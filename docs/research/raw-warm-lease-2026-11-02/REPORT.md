# RAW lease handoff and retained swap recovery — 2026-11-02

The resident RAW prefetch path now obtains its consumer lease in one lookup.
Previously it cloned the resident mosaic, discarded the temporary clone, and
then requested a consumer lease. A TTL boundary between the two calls could
lose retention. The direct lookup returns the initial lease and checks generation
cancellation before handing it to the worker. Source identity/recipe validation
still precedes the lookup; no pixel decoder or cache format was changed.

An original local EOS R8 CR3 was decoded once with the owned native backend.
Its warmed mosaic was put into a one-entry cache with a one-second TTL. The
test then occupied all remaining managed root capacity and held the foreground
decode permit. Warm prefetch returned before that gate was released, with the
same sensor allocation, samples and metadata; the visible pin remained unset.
This does not claim zero source-identity I/O or measure navigation latency.

After the TTL elapsed, new consumer lookups failed while the already acquired
lease stayed valid and prevented count-based replacement. Releasing that lease
allowed admission. Final managed usage was zero. The five-second response timeout
is a test guard, not a performance result. The original stays outside this evidence
directory; [pins.json](pins.json) records its size/hash and the test binary/source
hashes without copying the photograph.

The current required original-source suite passes **7/0 failed/0 ignored** in [raw.log](raw.log).
The ordinary viewer suite passes **354 tests/104 manual ignored** in
[viewer.log](viewer.log); [production check](production-check.log), formatting
and diff whitespace checks pass. The manual case is among those skipped by the
ordinary filter and was executed separately; it is not counted as an additional
ordinary-suite pass.

```sh
cargo test --locked -p rrrah --bin rrrah real_cr3_prefetch_returns_first_ram_lease_under_pressure_and_ttl -- --ignored --nocapture --test-threads=1
```

This closes the double resident lookup for this RAW path. It does not qualify
every camera, swap/cold RAW prefetch under all pressures, calibrated display,
NVIDIA/CUDA, or the separate pending three-hour raster-worker run.

## Retained swap recovery

`MosaicRamCache::try_get_retained_with_cancel` is an explicit retained-consumer path. It checks byte/count admission before restoration, may reclaim unpinned/unleased LRU entries on recoverable memory refusal, and fences active spills at most once with cancellation before retrying. Visible pins and consumer leases remain protected. The legacy background-clone API keeps its refusal-without-eviction behavior. RAW worker prefetch receives the returned lease directly. Cold-source and persistent disk-cache recovery are described below.

The new original CR3 case uses three actual file copies with distinct timestamps/keys. One native decode supplies their identical expected sensor payload; three swap objects are written and read. A restore pool of two sensor allocations is full while the managed root's remaining capacity is reserved. Two held leases refuse the third restore without a pixel read (reads=2). Dropping the old neighbor's lease lets retained prefetch reclaim that neighbor and restore the third (reads=3), with exact samples/metadata, the same selected allocation still pinned, and final managed usage zero. This is controlled managed admission pressure, not physical OS memory exhaustion or slow-disk qualification.

The original-source tests also cover both prefetch directions and count/byte limits, partial protected-budget admission, and warm TTL handoff. [Cache suite](cache.log): 160 passed, 5 ignored, zero failures, including a small managed-pool regression with cancellation and protected refusal/replacement. Current ordinary viewer:354 passed/104 ignored; required original-source:7 passed/zero ignored. Production check covers the current implementation. Format and diff checks pass.

```sh
cargo test --locked -p rrrah-cache --lib
cargo test --locked -p rrrah --bin rrrah gpr_preload_tests::real_cr3 -- --ignored --nocapture --test-threads=1
```

These are CPU sensor/cache proofs. They do not add new physical HDR, Metal shader, camera-format-wide, or NVIDIA qualification. Logs and pins above replace the earlier positive checkpoint rather than duplicate it.

## Persistent RAW cache recovery

The original CR3 scenario also runs with three persistent disk-cache objects and an empty swap index. Two restored sensor allocations fill the shared restore pool; the remaining managed root is reserved. With both consumer leases held the candidate is refused. After the old neighbor's lease drops, the previous implementation still returned `admitted=false`: [original failure](disk-before.log), [reverse patch](disk-before.patch), exact before-source hash in [pins.json](pins.json).

Persistent prefetch now uses the validated header's allocation size to check both the individual memory limit and RAM retention feasibility. It releases only an unpinned/unleased LRU neighbor on a recoverable memory refusal, then retries the persistent read. At most one cancellable active-spill fence is used when no RAM victim remains. Cancellation remains terminal, impossible retention/oversized allocations do not trigger reclamation, and transient persistent-cache memory errors do not fall through to native decode.

The repaired original-source case passes with exact sensor samples and metadata, the selected allocation still resident, no swap reads, root never exceeding its configured limit, and final managed usage zero. Seven required original CR3 tests and the full ordinary viewer suite pass on the pinned binary. This proves this authored native sensor/cache scenario, not slow storage, physical OS memory exhaustion, every camera, arbitrary cold-source working sets, or physical HDR. The three-hour raster-worker run remains separate and pending.

## Cold native CR3 recovery

An additional original-source test exposed a cold sensor allocation refusal: requested50,989,120 bytes, used536,870,912, limit536,870,912. This occurred with enough room for the compressed source but both neighbor leases held. Previously the native allocation error escaped prefetch rather than yielding a protected refusal: [original failure](cold-before.log), [reverse patch](cold-before.patch), before-source hash in [pins.json](pins.json).

Cold prefetch now reclaims unpinned/unleased neighbors before reading a compressed source that lacks headroom. For native CR3, the admitted prepared source supplies the sensor size without entropy decoding; RAM retention feasibility and source-plus-sensor fit are checked first, then sensor headroom is recovered while retaining that same prepared source. Each headroom stage waits for at most one cancellable spill fence. Individually oversized requirements refuse without reclamation at that stage. Actual allocations still enforce their managed budgets; this is a headroom check, not a reservation against concurrent unrelated owners.

Two original CR3 cases pass:

- **ColdSensor**: the available root space equals the22,382,226-byte source. Two restored sensor leases protect the selected/old neighbors. The candidate refuses cleanly; releasing the old lease permits reclaiming that neighbor before allocating the new sensor.
- **ColdSource**: the root is initially full, with three restored sensor leases and a separate reference sensor. Releasing two old leases permits reclaiming one neighbor for compressed input and another for the sensor. The selected allocation remains resident; final steady root usage is one sensor below the limit.

Both candidates have no persistent or swap object. All sensor samples/metadata match the native reference; swap-read counters remain at the initial restored-neighbor count, proving the candidate did not come from swap. Final managed usage is zero. [Current original-source suite](raw.log):7 passed/zero ignored. [Current ordinary viewer](viewer.log):354 passed/104 ignored/zero failed. Production check, formatting and diff checks pass. The cache implementation itself is unchanged from the160-test checkpoint above.

These focused EOS R8 cases do not establish other cameras' working-set requirements, performance under slow disks, physical memory exhaustion, or new GPU/HDR qualification. Source-headroom recovery is shared by RAW prefetch; the sensor-size preflight is specifically native CR3. The long Metal raster-worker run uses a frozen earlier binary and remains separate; its observed low-power system sleep must be retained when reporting duration.

The preserved reverse patches use zero context; validate/apply them with `git apply --unidiff-zero` against the pinned current source. They reconstruct their respective failed source snapshots, including test-only changes.
