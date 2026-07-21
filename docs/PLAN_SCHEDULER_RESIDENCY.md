# P0.1: priority scheduler, cache admission и GPU tile residency

Статус: архитектурный план перед реализацией. Дата: 2026-07-21.

Этот документ закрывает следующий обязательный слой после текущего прототипа:
`metadata/probe -> первый видимый RAW tile -> refinement -> готовый кадр`.
Задача не в том, чтобы запустить больше потоков, а в том, чтобы не выполнять
работу, которая уже не может попасть на экран. План учитывает CR2 с одним
последовательным lossless-JPEG потоком, tiled/strip DNG с независимыми
диапазонами, wgpu без переносимого универсального memory-budget API и
существующие `WeightedLru`/persistent mosaic cache.

## 1. Границы и цели

### Входит в P0.1

* единый `OpenSession` с monotonic generation и отменой устаревшей работы;
* bounded priority scheduler для probe, I/O, entropy decode, tile postprocess,
  upload и readback;
* резервирование байтов до запуска работы и освобождение после публикации или
  отмены;
* RAM 2Q/TinyLFU поверх byte-weighted cache с pin текущего кадра;
* GPU residency для видимых tiles вместо eager full-frame texture array;
* persistent triple staging ring с абстрактными fence-объектами, скрывающими
  различия Metal/Vulkan/DX12/WebGPU backend;
* восстановление после device loss: CPU/низкокачественный fallback, без
  публикации старого поколения и без бесконечного retry;
* телеметрия очередей, ожиданий, отмен, cache admission, uploads и
  residency-hit/miss;
* воспроизводимые benchmark-gates и failure-injection тесты.

### Не входит в P0.1

MHC/RCD/AMaZE, DCP/ICC/OpcodeList, полноценный denoise, экспорт, catalog/XMP,
HDR merge, ML ISP и sandbox-процесс декодера. Они должны использовать эти
порты, но не расширять scheduler скрытыми неблокирующими задачами.

## 2. Текущий baseline и обязательные изменения

Сейчас `rrrah-app` запускает один decode thread, не имеет generation gate и
материализует `DecodedMosaic`; `rrrah-cache` содержит корректный byte-weighted
LRU и атомарный full-mosaic blob; `rrrah-gpu` создаёт eager `R16Uint` texture
array и ограничивает его 512 MiB. Это хорошие предохранители, но:

1. `WeightedLru` нельзя считать residency manager: отсутствуют pin, in-flight
   protection, probation/protected очереди и admission policy.
2. eager atlas отказывает большим RAW, а не загружает только viewport tiles.
3. `spawn_load` не отличает работу старого открытия от текущего; поздний
   `LoadEvent::Ready` может заменить новый кадр.
4. `Queue::write_texture` синхронно создаёт временные буферы; нет bounded staging
   и backpressure относительно GPU.
5. телеметрия уже умеет JSONL/Chrome trace, но application/gpu этапы пока не
   обязаны отправлять стабильные `generation`, `tile`, `bytes`, `status` поля.

Изменения должны быть аддитивными: старый eager path остаётся compatibility
fallback для маленьких кадров, а scheduler/residency включаются feature flag-ом
до того, как будут удалены проверенные тесты eager пути.

## 3. SOLID-границы и владение

```text
OpenSession (app)
  ├─ WorkScheduler (core/app port)
  │    ├─ DecodePort (rrrah-decode)
  │    ├─ RamTileCache (rrrah-cache)
  │    ├─ DiskTileCache (rrrah-cache, later)
  │    └─ GpuResidency (rrrah-gpu port)
  └─ PublishGate (UI thread)
```

* `WorkScheduler` только планирует и резервирует ресурсы; он не знает TIFF,
  WGSL или winit.
* `DecodePort` обязан принимать `CancellationToken`, выдавать checked ranges и
  не владеть GPU handles.
* `RamTileCache` владеет только CPU bytes. `GpuResidency` владеет slots,
  bind-groups, staging и fence lifetime.
* `PublishGate` — единственная точка изменения `App.current_frame`. Любой
  completion сначала проверяет `(session_id, generation, tile_key, quality)`,
  затем публикуется.
* UI не ждёт lock decode/cache/GPU; event loop получает лишь bounded wakeups.

Публичные границы (псевдо-Rust, до конкретного async runtime):

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Ord, PartialOrd)]
pub struct Generation(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TileKey { pub frame: u32, pub mip: u8, pub x: u32, pub y: u32 }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkClass { Probe, VisibleDecode, VisibleUpload, Refinement,
                     NeighborPrefetch, Catalog, Export }

pub struct WorkItem {
    pub id: u64,
    pub session: u64,
    pub generation: Generation,
    pub class: WorkClass,
    pub tile: Option<TileKey>,
    pub estimate: CostEstimate,
    pub deadline_ns: Option<u64>,
    pub cancel: CancellationToken,
}

pub struct CostEstimate {
    pub compressed: u64,
    pub decoded: u64,
    pub staging: u64,
    pub gpu: u64,
    pub cpu_ns: u64,
}

pub trait WorkScheduler: Send + Sync {
    fn submit(&self, item: WorkItem) -> Admission;
    fn cancel_generation(&self, session: u64, generation: Generation);
    fn poll(&self, limit: usize) -> Vec<WorkItem>;
}

pub trait GpuResidency: Send {
    fn request(&mut self, key: TileKey, bytes: TileBytes, generation: Generation)
        -> Result<UploadTicket, GpuError>;
    fn evict(&mut self, key: TileKey) -> Eviction;
    fn poll_fences(&mut self) -> FenceProgress;
    fn recover(&mut self, device: DeviceHandle) -> Recovery;
}
```

`Admission` must be an explicit `Granted(Reservation)` or `Rejected(reason)`;
never silently enqueue work whose buffers cannot be reserved.

## 4. Session и state machines

### 4.1 Open session

```text
Idle
  -> AcquiringHandle -> Probing -> Planned
  -> CacheLookup -> VisibleRequested
  -> FirstRawPresent -> Refining -> Ready
       |                  |
       +-> Failed          +-> Cancelled (new generation / close)
```

`OpenSession::open(path)` atomically increments generation before submitting
`Probe`. A new open does **not** destroy old buffers synchronously: it cancels
the token, unpins old frame, and lets workers release reservations. The UI may
continue showing the old frame until the new frame publishes or fails.

### 4.2 Work-item lifecycle

```text
Created -> Admitted -> Queued -> Running -> Completed
                                  |            |
                                  +-> Cancelled +-> Published
                                  +-> Failed
```

The terminal transition is single-assignment. `cancel` is cooperative at
boundaries (before read, after read, before decode, before upload, before
publish); a decoder may finish an uninterruptible entropy block, but its bytes
must be discarded if generation is stale. Every result carries a reservation
lease; dropping the lease returns all four byte reservations exactly once.

### 4.3 Tile residency lifecycle

```text
Missing -> Requested -> Decoding -> CPUResident
                         -> UploadQueued -> Uploading
                         -> GPUResident -> EvictPending -> Evicted
                         |                     |
                         +-> Cancelled         +-> DeviceLost -> CPUResident
```

`CPUResident` is not sufficient for `Published`: the tile must be in a slot with
a completed submission fence. A tile marked `EvictPending` remains sampled until
the corresponding submission retires. Slot reuse requires `(slot_generation,
submission_complete)` checks to prevent ABA/stale texture reads.

## 5. Priority scheduler

### 5.1 Queues and workers

Use one bounded priority heap per resource domain, not one unbounded global
executor:

* `probe_io`: 1–2 threads, max 2 concurrent file probes;
* `decode_cpu`: `min(max(1, physical_cores - 1), configured_decode_workers)`;
* `postprocess_cpu`: separate semaphore, normally `max(1, physical_cores/2)`;
* `gpu_submit`: one owner thread/event-loop lane;
* `background`: catalog/export only when no visible deadline is at risk.

CR2 gets one sequential reader per source and parallel row-band/postprocess work;
its entropy bitstream is not split at arbitrary offsets. DNG tile requests may
run independently, but total parallelism remains bounded by file handles,
compressed-byte budget and output-byte budget.

`poll(limit)` is deterministic: highest score, then earliest deadline, then
smallest id. Each worker checks the token before taking the next item.

### 5.2 Integer priority score

Do not sort by floating point or wall time. Compute a signed fixed-point score
(`i64`, Q16.16) every time an item is popped:

\[
S_i = 8C_i + 12V_i + 10D_i + 4Q_i + 3A_i - 5B_i - 2R_i.
\]

Terms are clamped to `[0, 65_535]`:

* `C`: class urgency (`Probe=65535`, `VisibleUpload=60000`,
  `VisibleDecode=58000`, `Refinement=30000`, `Neighbor=12000`,
  `Catalog/Export=2000`);
* `V = 65535/(1 + manhattan_distance_to_viewport)` for visible/halo tiles,
  zero for non-visible work;
* `D = clamp((frame_budget - slack_ns) * 65535 / frame_budget)` where
  `slack = deadline - now`; overdue work saturates at 65535;
* `Q`: quality tier (`bilinear=30000`, `MHC/RCD=50000`, export=65535), but
  quality never outranks a visible first-frame deadline;
* `A = min(age_ns / 1ms, 65535)`, giving bounded anti-starvation aging;
* `B = log2(1 + bytes / 64KiB) * 8192`, discouraging one giant tile from
  starving small visible work;
* `R`: retry count and known driver/decoder risk, capped at 65535.

Weights are configuration, not API semantics. Production tuning must use the
telemetry score breakdown, and changing weights increments the benchmark
configuration hash. Admission never uses score to exceed a hard byte budget.

### 5.3 Deadlines и fairness

The first-visible tile gets `deadline = open_start + target_first_visible`
(default target 150 ms for a warm NVMe run). Refinement tiles use the next frame
deadline (`now + 16.7 ms` at 60 Hz); prefetch has no hard deadline. A queue is
starved if its oldest item age exceeds `max_wait` (default 500 ms); aging raises
its score but cannot bypass a full reservation budget. Background work is paused
while `visible_backlog > 0` or frame miss rate exceeds 1%.

No worker may recursively submit unbounded follow-up work. A DNG tile may submit
at most its known halo dependencies; duplicate keys are coalesced by the
in-flight map.

## 6. Byte admission и backpressure

Every item reserves all worst-case resources before execution:

\[
N_i = B_{compressed} + B_{decoded} + B_{staging} + B_{gpu} + B_{metadata}.
\]

For each pool `p`, grant iff:

\[
resident_p + reserved_p + N_{i,p} \leq cap_p.
\]

`N_i` is checked with `u64::checked_add`; overflow is an immediate rejection.
`compressed` is bounded by checked TIFF/CR2 ranges, never by an untrusted count.
If a single tile exceeds a pool cap, reject with a user-visible “too large for
this backend” reason; never evict the entire current frame to admit it.

Recommended initial budget model (overridable per device):

```text
RAM cap       = min(user_cap, 0.25 × available_memory)
GPU logical   = min(user_cap, 0.50 × conservative_device_budget)
staging cap   = min(256 MiB, 0.10 × RAM cap)
compressed    = min(512 MiB, 0.10 × RAM cap)
```

If the backend does not expose device memory, `conservative_device_budget` is
`max(64 MiB, 2 × max_texture_dimension² × bytes_per_sample)` capped by the user
setting; allocation errors lower it monotonically. Never infer free VRAM from a
single successful allocation.

Reservations are RAII leases held through result publication/fence retirement.
An admission failure increments `scheduler.admission_rejected{pool,reason}`
and can retry at a lower quality/mip, but not at the same size indefinitely.

## 7. RAM cache: 2Q/TinyLFU, byte weighted

The current `WeightedLru` remains the deterministic fallback and test oracle.
The production tile cache should use byte-weighted 2Q with a TinyLFU admission
filter:

```text
A1-in (probation) 20–25%  : new/one-hit tiles, FIFO by bytes
Am (protected)     75–80%  : tiles hit at least twice, LRU by bytes
Ghost metadata               : keys only, no payload, bounded by entry count
Pinned set                   : current visible frame/halo, excluded from evict
```

For an insertion of weight `w`, compare the candidate frequency against the LRU
victim frequency in `Am`. Admit if:

\[
\hat f(candidate) + \log_2(1+w/64KiB) \geq \hat f(victim).
\]

The size penalty avoids admitting a 48 MiB full-frame tile because it was seen
once. TinyLFU uses a 4-row Count-Min Sketch with saturating 4-bit counters;
halve counters every `2^20` accesses to avoid ancient history. Hash seed is
recorded in benchmark manifests for deterministic tests.

Required API semantics:

```rust
pub trait RamTileCache<K, V> {
    fn lookup(&self, key: &K) -> Option<Arc<V>>; // increments frequency
    fn admit(&self, key: K, value: Arc<V>, weight: u64) -> Admission;
    fn pin(&self, key: &K, owner: PinOwner) -> Result<PinGuard, PinError>;
    fn invalidate(&self, predicate: InvalidatePredicate);
    fn stats(&self) -> CacheStats;
}
```

Lookups must not return a reference whose eviction can race with use; use
`Arc<V>` or a guard. `PinGuard` is non-copy and unpins on drop. A debug counter
must assert `resident_bytes <= cap_bytes` and `pinned_bytes <= cap_bytes`; if a
pin leak would exceed the cap, admission is rejected rather than evicting a
pinned tile. Shard maps by `hash(key) % shard_count`; each shard owns its queues,
so a global lock is never taken by the decode hot path.

Persistent blobs remain immutable and atomic. P0.1 may cache complete mosaic
payloads, but the disk format must carry `tile_schema`, `tile_size`,
`semantic_pipeline_abi` and source fingerprint before a tiled format is enabled.

## 8. GPU tile residency

### 8.1 Logical/physical mapping

A logical `(frame,mip,x,y)` maps to a physical array layer or atlas slot through
a small page table. The shader never assumes `layer = y * grid_x + x` once
residency is enabled:

```text
page_table[logical_tile] = { slot, mip, valid, tile_generation }
slot_texture[slot]      = R16Uint tile + halo
```

The page table is a storage/uniform buffer updated only after a `copy_buffer`
or queue submission that contains the tile upload. Invalid entries sample the
coarsest resident mip or a neutral checkerboard, never uninitialized memory.
MHC/RCD halo radius is part of the tile key (`halo=1` bilinear, `halo=2` MHC,
larger algorithms use a different pipeline key). A tile is not a cache hit if
the halo or semantic pipeline ABI differs.

### 8.2 GPU LRU and pinning

Slots use the same protected/probation principle as RAM, but eviction is gated
by GPU completion:

```text
Missing -> Uploading -> Resident(protected/probation)
Resident -> EvictPending(submission_id) -> Free
```

The current viewport and one halo ring are pinned for at least one frame. A
zoom/pan transaction updates the pin set atomically; old pins retire after the
next present. Eviction score is:

\[
E_i = 4\,age_i + 2\,distance_i + upload\_cost_i - 8\,pinned_i,
\]

where larger `E` is evicted first; `pinned` is a hard prohibition, not merely a
penalty. If no slot is evictable, the scheduler backpressures uploads and asks
CPU to keep the tile resident. It must not create another texture implicitly.

### 8.3 wgpu constraints

The renderer must retain a compatibility path for adapters that cannot expose
storage textures, timestamp queries or a sufficiently large texture array.
`max_texture_dimension_2d`, `max_texture_array_layers`, alignment, format
features and device limits are captured in a `GpuCapabilities` snapshot. A
logical budget is enforced even where the API cannot report actual VRAM.

No cross-thread `wgpu::Device`, `Queue`, `Texture`, `BindGroup` or surface
ownership is allowed. The GPU owner serializes resource mutations; workers only
return CPU tile bytes and an upload command description.

## 9. Persistent staging ring и fences

### 9.1 Ring layout

Use a persistent mapped/upload buffer split into `N` segments (`N=3` minimum,
`N=4` when frame latency is >16.7 ms). Each row pitch is:

\[
pitch = 256 \cdot \lceil ((T+2H)\cdot 2)/256 \rceil,
\qquad B_{tile}=pitch\cdot(T+2H).
\]

The segment state is `Free -> Reserved -> Submitted(index) -> Retired`. A
segment is reusable only after the backend fence reports completion. Fence
polling is non-blocking on frame ticks; if all segments are busy, upload work is
queued rather than allocating a temporary unbounded `Vec`.

Abstract the backend uncertainty:

```rust
pub trait GpuFence: Send {
    fn is_complete(&self) -> bool;
    fn wait_bounded(&self, budget: Duration) -> FenceWait;
}

pub struct StagingLease { offset: u64, len: u64, segment: u16,
                          submission: SubmissionId }
```

The wgpu implementation binds `SubmissionId` to its submission-completion
callback/polling mechanism; it must not pretend that `queue.submit` itself is a
fence. `Device::poll` is driven from the GPU owner, and a lost device retires
all unresolved segments as `DeviceLost`, not `Free`.

### 9.2 Upload batching

Batch adjacent tiles from one generation when the total staging bytes fit the
current segment. One queue submission per frame (or ≤2 per 16.7 ms frame) is the
default. A visible tile may bypass batching when its deadline would be missed.
Each batch records `bytes`, `tile_count`, `row_pitch`, `generation`, and
`submission_id` in telemetry.

## 10. Generation cancellation и stale publish defense

The publish gate is deliberately boring and synchronous:

```rust
fn publish(result: Result<TileResult, Error>, stamp: Stamp) {
    if session.current_generation() != stamp.generation
        || session.is_closed()
        || !session.accepts(stamp.tile, stamp.quality)
    {
        drop(result); // RAII releases reservation; no UI mutation
        telemetry.cancelled_stale(stamp);
        return;
    }
    session.commit(result);
}
```

`Generation` wraps with checked increment; if it approaches `u64::MAX`, the
session is reset while the UI is blocked for one event-loop turn. A stale task
may finish CPU work, but it cannot mutate page tables, cache pin counts, current
frame, histogram, or metadata. Deduplicate in-flight keys by `(session,
generation, tile, pipeline_abi)`; cancellation removes only the subscriber,
not a shared decode still required by a newer generation.

## 11. Device loss и fallback

Classify failures instead of retrying all errors:

```text
Recoverable surface lost/outdated -> reconfigure, keep residency metadata
Device lost / allocation failure   -> invalidate GPU slots, keep RAM tiles
Unsupported feature                 -> CPU bilinear or eager compatibility path
Out-of-memory twice                 -> lower GPU budget/quality, no retry loop
Fatal adapter init                  -> CPU-only viewer with explicit status
```

On loss, stop new uploads, mark all staging leases `DeviceLost`, preserve RAM
and disk keys, then perform at most two recovery attempts with exponential
backoff (`50 ms`, `250 ms`). Recreate device/surface on the GPU owner thread,
re-probe capabilities, and upload only visible tiles first. If recovery fails,
switch to CPU bilinear display and continue navigation; a blank window is not a
valid fallback. A subsequent manual “retry GPU” starts a new generation and
clears only GPU state.

The CPU fallback must be separately bounded: at most one full-frame conversion
or `K` viewport tiles, never a hidden unbounded allocation. Its quality/backend
is recorded so CPU and GPU timings are not combined.

## 12. Telemetry contract additions

Use existing bounded JSONL/Chrome telemetry, but add required attributes to all
scheduler and residency events:

```text
scheduler.enqueue          class, priority, generation, estimated_bytes
scheduler.admit/reject     pool, requested, resident, reserved, reason
scheduler.start/end        queue_wait_ns, run_ns, status, generation
scheduler.cancel           reason={new_generation,closed,deadline,device_lost}
cache.lookup/admit/evict   tier, hit, weight, pinned_bytes
gpu.residency              tile, mip, slot, state, generation, bytes
gpu.staging                 segment, offset, bytes, submission, fence_state
gpu.device                  backend, limits, loss_reason, recovery_attempt
publish.first_visible      tile, quality, generation, elapsed_ns
publish.stale_drop         tile, work_generation, current_generation
```

Counters are emitted non-blockingly. Dropped live events do not block decode,
but correctness benchmarks fail if durable `telemetry.dropped_events > 0`.
Include `reservation_id` so leaked leases and double releases can be detected.
Never log RAW pixels or full paths in a default trace; use fixture hash and
basename policy appropriate for privacy.

## 13. Benchmark gates

All gates are per CPU/GPU/API/driver/fixture class; numbers from different
devices are not collapsed. The target is a gate, not a universal speed claim.

| Gate | Measurement | Initial acceptance |
|---|---|---:|
| first visible | open→first valid RAW tile, warm NVMe | p95 ≤150 ms |
| stale safety | old generation published after new open | 0 / 10,000 transitions |
| frame pacing | 60 Hz viewport with pan/zoom | p95 ≤16.7 ms, missed <1% |
| cancellation | new generation→old work marked cancelled | p95 ≤20 ms at boundary |
| RAM budget | peak RSS over configured cap | ≤10% over cap; no OOM |
| GPU budget | logical resident + in-flight bytes | never above cap |
| upload | aligned staging tile upload | 0 alignment/row corruption failures |
| DNG scaling | 1→2→4→8 decode workers | efficiency `T1/(n·Tn)` ≥0.50 at 8 |
| CR2 behavior | workers 1→N | no claimed linear speedup; postprocess ≥0.70 efficiency |
| cache | repeated viewport sweep | ≥80% RAM tile hit after warmup |
| cache correctness | tile vs monolithic reference | ≤1 linear 16-bit LSB seam error |
| device loss | injected loss during upload | visible CPU fallback, no stale publish |
| telemetry | deterministic corpus | zero dropped durable events |

For scale efficiency:

\[
\eta_n = \frac{T_1}{n\,T_n},
\quad T_n = T_{probe}+T_{serial}+T_{parallel}(n)+T_{upload}.
\]

Report serial fraction using Amdahl, not only aggregate wall time. CR2 entropy
must expose `T_serial`; otherwise a misleading “8-thread” result is rejected.

Failure-injection suite must cover: cancelled read, duplicate tile request,
full RAM cache, all GPU slots in-flight, fence never completing, surface lost,
device lost, malformed offset, and telemetry consumer disconnect.

## 14. Implementation sequence

1. Add `Generation`, `CancellationToken`, `WorkItem`, `Reservation` and a
   deterministic single-thread scheduler with unit tests for score/order.
2. Put `publish` behind the generation gate; add stale completion tests to
   `rrrah-app` before enabling parallel workers.
3. Add pool-specific byte accounting and RAII reservations; instrument every
   rejection and release.
4. Extract `RamTileCache` trait; implement byte-weighted 2Q and retain
   `WeightedLru` as a feature-gated fallback/oracle.
5. Implement `GpuResidency` with a small fixed slot array, page-table indirection
   and visible-tile pinning. Keep eager atlas for adapters without residency.
6. Implement staging ring/fence abstraction and alignment tests (256-byte rows,
   halo edges, submission retirement).
7. Add device-loss state machine and CPU bilinear fallback; inject failures in
   a headless GPU test where available.
8. Integrate telemetry schema/events; add benchmark harness and acceptance
   gates above. Only then enable DNG viewport tiles and CR2 row-band refinement.

## 15. Critic review: failure modes, races и осознанные отсрочки

### Critical risks

* **Stale publish race.** Generation check performed only when a worker starts is
  insufficient; check again immediately before page-table/UI mutation. `Stamp`
  must be immutable and publish must be single-owner.
* **ABA slot reuse.** A slot can be evicted and reused while an old submission
  still samples it. Require completed submission plus slot generation in the page
  table; never use an eviction request as a fence.
* **Pin leak.** A lost UI event or panic can leave current tiles pinned forever.
  `PinGuard` drop, debug accounting and session-close cleanup are mandatory.
* **Double release/underflow.** Reservation release must have an id and atomic
  terminal state. Saturating subtraction can hide a bug; assert in debug and
  return an error in release telemetry.
* **Device-loss storm.** Recreating a device on every `SurfaceError` can spin and
  leak resources. Classify errors, cap retries, exponential-backoff, then CPU
  fallback.
* **Fence ambiguity.** A queue submission index is not completion. Treat fences
  as backend-specific and keep in-flight bytes reserved until explicit progress.
* **Priority starvation.** Pure visible priority starves catalog/ghost queues;
  bounded aging and a maximum wait are required. Aging cannot override byte caps.
* **Duplicate decode amplification.** Two viewport events can enqueue the same
  tile. Coalesce by key and share the result; cancellation removes a subscriber,
  not necessarily the producer.
* **Untrusted ranges.** Scheduler cost estimates must be generated only after
  checked `offset + length`; never trust a TIFF count to reserve/allocate directly.
* **Unified-memory mismeasurement.** macOS may report no useful VRAM budget.
  Enforce a conservative logical cap and count staging + textures + in-flight
  bytes; do not claim physical VRAM precision.

### Deliberately deferred or impossible in P0.1

* Exact global optimal ordering is impossible with unknown decode cost and GPU
  driver scheduling; use an observable heuristic and tune from traces.
* Hard cancellation inside every third-party entropy decoder is not guaranteed;
  cancellation is cooperative at safe boundaries and stale output is discarded.
* Portable GPU timestamp and memory queries do not exist across all wgpu
  backends; report unavailable rather than synthesizing numbers.
* True tile-first CR2 decode cannot be obtained by splitting arbitrary byte
  ranges. It requires upstream restart-marker/slice-aware support or one
  sequential reader plus postprocess parallelism.
* A no-copy zero-allocation path is not promised: alignment, driver staging and
  format conversion may require bounded copies.
* TinyLFU frequency is approximate. It is an admission hint, never a correctness
  decision; cache misses remain valid.

**Critic verdict:** implement generation gate, byte reservations, in-flight/fence
protection and device-loss fallback before optimizing score weights or adding
more decoder threads. Any benchmark that cannot prove these invariants is a
performance anecdote, not a production result.
