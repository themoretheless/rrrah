# RAM policy timing

## Runtime limit changes

`WeightedLru::set_limits`, `MosaicRamCache::set_limits` and
`RasterRamCache::set_limits` apply byte/count changes atomically. Pinned entries
are retained; an impossible limit returns `None` without changing limits,
membership or recency. Successful shrinking returns expired-first, then LRU
unpinned victims to the caller for optional spill. Returning victims does not
free their managed allocations until their last owner drops.

TTL changes apply to future insertions/replacements. Existing deadlines remain
fixed, including when TTL is disabled; old expired entries cannot become live
again. This API changes membership policy, not the immutable allocation budget.
It is not yet wired to interactive settings in the application.

Both image RAM caches also expose `set_limits_and_spill`: accepted shrinking
offers victims to the existing bounded asynchronous swap queue. Its boolean
reports policy acceptance, not guaranteed persistence. A full/disabled queue
drops its offered owner. A RAW integration test shrinks two frames to one,
retains the visible pin, waits for the disk write and restores background samples
and metadata exactly without displacing the visible frame; rejected shrinking
produces no writes.

Measured 2026-10-05 on the current macOS Apple M4 Max worktree, optimized dev
profile. Run:

```sh
cargo run -p rrrah-memory --example cache_policy_timing
```

The harness uses u32 keys/values with one-byte membership weights, one protected
entry and a recently accessed oldest key. Setup is outside timing. Every result
is checked outside timing for exact eviction order, retained protection, entry
count and byte weight. Three warmups precede twenty measured repetitions.
Hit samples average 10,000 lookups each. Single eviction inserts one unit-weight
entry; bulk eviction admits an entry weighing capacity minus one, evicting every
unprotected old entry. This is not a measurement of image navigation, RAW cache
keys, buffer allocation, decode, disk, GPU upload or physical display.

| Entries | Hit p50/p95 us | Single eviction p50/p95 us | Bulk eviction p50/p95 us |
| --- | --- | --- | --- |
| 64 | 0.005 / 0.006 | 1.042 / 2.792 | 3.125 / 6.333 |
| 1024 | 0.005 / 0.005 | 11.417 / 12.125 | 44.333 / 45.541 |
| 4096 | 0.005 / 0.006 | 44.333 / 46.042 | 182.750 / 185.709 |
| 16384 | 0.005 / 0.006 | 174.834 / 191.584 | 774.833 / 912.000 |

Evidence: `target/bench/memory-policy/current.csv` and `run.log`. This table records the pre-index implementation. The follow-up below is a
consecutive run, not an interleaved A/B measurement. Bulk admission now builds one sorted victim plan instead of repeatedly
scanning the whole map. The pre-index single-eviction path scans all eligible entries and allocates a
candidate list.
TTL-enabled lookup, expiration scanning, large/custom keys, concurrent callers
and process-memory pressure are outside this run.

## Indexed eviction follow-up

The unprotected-recency BTreeMap avoids the candidate-map scan for a sufficient
single victim when TTL is disabled. Membership changes and recency updates
maintain the index in logarithmic time; repeated hits on the most recent entry
do not change eviction order and bypass index updates. Pin/unpin, replacement,
removal and clock rollover are tested. TTL preference and large admissions keep
the full planning path.

| Entries | Hot-key hit p50/p95 us | Single eviction p50/p95 us | Bulk eviction p50/p95 us |
| --- | --- | --- | --- |
| 64 | 0.010 / 0.012 | 0.208 / 0.292 | 4.625 / 9.916 |
| 1024 | 0.010 / 0.010 | 0.292 / 1.125 | 72.292 / 75.916 |
| 4096 | 0.011 / 0.011 | 0.334 / 1.000 | 291.208 / 341.625 |
| 16384 | 0.010 / 0.010 | 0.958 / 1.542 | 1299.250 / 1732.417 |

Evidence: `target/bench/memory-policy/indexed.csv` and `indexed.log`. The index
improves the measured ordinary victim-selection case while increasing bulk
eviction cost and metadata memory. These figures cover repeated hits on one
key, not rotating/random-key recency traffic. They do not prove end-to-end viewer
latency improvements or universal workload superiority.

## Rotating-key follow-up and model qualification

A follow-up adds 10,000 sequential lookups cycling through the configured key
range per sample. Median/p95 microseconds per lookup were 0.049/0.074 for 64
entries, 0.070/0.073 for 1024, 0.083/0.085 for 4096 and 0.099/0.111 for 16384.
Hot-key medians stayed 0.010 us in this run. Evidence:
`target/bench/memory-policy/rotating.csv` and `rotating.log`. These remain u32-key
policy microbenchmarks with no decoded payload or physical presentation.

`indexed_policy_matches_sequence_model_under_mixed_pressure` compares 10,000
deterministic mixed operations against an independent oldest-to-newest sequence
model: admission, rejection, replacement, lookup promotion, pin/unpin, removal
and victim extraction. After each operation it checks values, resident weight,
count, protection and index count. Forced counter rollover exercises compaction.
The model run has count and byte limits together and TTL disabled; TTL boundaries
and expired-pinned behavior are covered separately by the existing limit tests.

## Explicit swap admission

`ImageSwapCache::try_enqueue` returns `SpillAdmission`: `Queued`,
`AlreadyPresent`, `AlreadyPending`, `Disabled`, `TooLarge`, `Full`, `Busy`, or
`WorkerStopped`. Admission is nonblocking and does not guarantee persistence.
The existing `enqueue` wrapper retains opportunistic behavior and discards the
result. Rejected payload ownership and queue reservations are released before
return; duplicate entries do not increment the dropped counter.

Tests distinguish a payload exceeding the whole queue budget from temporary
budget pressure, pending-index contention, disabled spill, accepted writes and
completed duplicates. They check released managed memory, pending-key cleanup
and exact write count. The cache library suite passes 104 tests.

## Consumer leases

`rrrah_memory::LeaseCache<K, V>` composes the existing weighted policy with
`CacheLease<V>`. Acquiring and cloning a lease shares an immutable value without
copying its payload. Held leases protect both eviction and replacement. Cache
mutations reconcile the last lease release without a lock or callback on the
consumer's drop path. New lookups still respect TTL; an existing lease remains
valid. Rejected insertions return the caller's value, and eviction/limit changes
return owned victims for optional spill. The caller supplies capacity weights;
allocation accounting remains in `MemoryBudget` and managed buffers.

The RAW RAM cache now uses this generic policy. Its explicit visible-frame pin
remains independent of consumer leases exposed by `MosaicRamCache::get_lease`.
Foreground RAW Ready events now carry an optional resident consumer lease.
Successful GPU upload transfers it to a completion callback; stale, rejected
or quality-converted events release their lease through ordinary ownership. Tests cover limit rejection, spill-owner accounting, protected admission,
replacement rejection, cloned leases released on another thread, and zero-TTL
lookup expiration without invalidating a previously held value.

An integration test holds two cloned consumer leases across visible-frame
navigation. Shrinking, replacing the leased key, background admission and
allocation-pressure eviction all preserve the held frame. After the last lease
release, shrinking writes exactly one swap entry; restoring it preserves pixels
without displacing the new visible frame. Explicit pins can protect expired
resident values, while consumer lookup continues to respect TTL.

## Viewer upload lifetime

All three foreground RAW routes (RAM/swap hit, persistent hit, native decode)
acquire a resident lease before publishing Ready when cache admission succeeds.
An oversized or otherwise rejected cache admission still publishes the decoded
frame without a lease. The UI transfers a successful upload's lease to
`submit_upload_lease`, which flushes pending texture writes with a queue
submission before attaching its completion callback. It protects CPU cache
membership through upload completion, not until physical scanout or replacement
of the GPU texture. GPU allocation lifetime has its separate budget.

A hardware-required test on Metal uploads the pinned Sony DSC-R1 SR2, verifies
that shrinking the cache to zero is rejected while the consumer holds the
lease, waits for the exact upload submission, then checks successful shrinking
and zero retained managed CPU bytes. The normal application suite passes 104
tests with six optional tests ignored; the Metal upload lease test was also run
explicitly and passed. The isolated upload/frame comparison below measures the extra submission;
end-to-end window navigation remains unmeasured.

## Paired Metal upload submission timing

Measured on Apple M4 Max with `raw_view_timing --compare-upload-submit`,
Metal explicitly selected, managed CPU limit 256 MiB and atlas limit 128 MiB.
Each fixture uses 4 warmup pairs followed by 20 measured pairs, alternating
combined/split order. Native decode and cache setup occur before timing; both
modes upload the same leased resident mosaic and render a completed 1920x1080
offscreen frame. Split mode flushes upload writes and attaches the lease-release
callback before frame submission. Every split callback must have run after the
exact frame submission completes. The baseline uses combined upload/frame
submission. Cache lease acquisition and callback-status allocation are outside
timing in both modes. Results include upload, view uniform update and completed
frame, excluding decoding, navigation, surface presentation and physical scanout.

| Fixture | Combined p50/p95 ms | Split lease p50/p95 ms | Paired split-minus-combined p50/p95 ms | Split submit median ms |
| --- | --- | --- | --- | --- |
| Sony DSC-R1 SR2 | 8.265 / 9.965 | 8.486 / 9.859 | -0.022 / 1.949 | 0.01894 |
| Minolta Dynax 7D MRW | 4.992 / 5.524 | 4.865 / 6.182 | 0.159 / 0.789 | 0.01904 |
| Epson R-D1 ERF | 4.087 / 6.949 | 4.013 / 5.423 | -0.178 / 1.220 | 0.01404 |

The noisy paired differences do not establish a universal speed advantage or
regression. Keep the split submission for the verified completion lifetime;
these measurements do not justify claiming faster navigation. Both managed
budgets return to zero at run completion. Evidence is retained in
`target/bench/lease-upload/{sony,extra}.{csv,log}`. This timing harness does not
read back pixels or qualify ERF display color; independent GPU pixel checks are
separate from performance timing.

## Raster lease integration

`RasterRamCache` now uses the same `LeaseCache` policy as RAW and exposes
resident consumer leases. `get_visible_lease_for` checks pixel-allocation identity,
dimensions and color space, preventing a failed admission from attaching the
previous visible frame's lease to the new output. Raster Ready events carry this
optional lease through the UI to the shared upload-completion helper. Quality
RAW-to-raster conversion currently has no resident raster-cache lease.

A managed HDR+ICC swap test checks complete 19-byte ownership (16-byte float
pixels plus a 3-byte profile), cloned protection across navigation, rejection of
replacement/pressure eviction and shrinking until the last lease drops. The
subsequent spill writes exactly once; restore preserves float bit patterns
including negative zero and the exact ICC bytes. Managed memory returns to zero.
A separate Metal-required upload test now covers both the pinned full SR2 sensor
and a managed linear HDR float raster: admission to zero remains blocked while
leased, then succeeds after the exact upload submission completes. These are
storage/upload-lifetime checks, not physical HDR-display qualification.

Validation: cache library 106 tests plus 5 integration tests passed; application
104 tests passed (6 optional ignored); the Metal upload test was explicitly run
and passed after adding the raster case.

## Model lease integration

The viewer's model RAM cache now uses `LeaseCache` for STL, OBJ, PLY and OFF.
Model Ready events acquire a visible resident lease only when the actual shared
geometry allocation matches the output. A failed foreground admission therefore
cannot attach the previous model's lease. Successful geometry upload passes the
lease to the common completion helper; stale and rejected events drop it.
Explicit visible pins remain independent of consumer leases.

A four-format integration test retains cloned leases across navigation, verifies
that background admission and allocation-pressure eviction cannot displace the
held model, then releases both consumers and observes exactly one native swap
write. Restoration bypasses the decoder and produces byte-identical native
payloads, covering vertices, indices and format attributes. Managed allocation
credit returns to zero after dropping the cache. Existing model swap/retry tests
still pass. `LeaseCache::remove` supports explicit invalidation, refuses active
consumer leases and clears the owner pin; a regression test checks that reusing
the key does not retain stale protection.

The required Metal upload test now additionally uploads each of the four model
formats, verifies held-lease limit rejection, waits for the exact submission,
and checks successful eviction with zero retained CPU allocation credit. This
qualifies upload completion lifetime; GPU pixel readback and physical display
are separate evidence. Application validation: 105 tests passed (6 optional
ignored); the expanded Metal test passed explicitly. Library validation: memory
30, cache 106 and cache integration 5 tests passed.

## Completion progress without visible rendering

The viewer tracks pending upload callbacks across RAW, raster and model paths.
`about_to_wait` performs nonblocking `Device::poll(PollType::Poll)` only while
callbacks remain. Pending completion schedules a 10 ms event-loop wake;
once all callbacks have released their leases, control flow returns to `Wait`.
No redraw or successful surface acquisition is required. Poll failure is logged
and stops scheduling completion wakeups for that iteration; device-loss recovery
is not qualified by this change.

The Metal-required RAW upload test now completes by nonblocking polling with a
bounded test deadline, without drawing or waiting on a frame submission. It
checks zero pending callbacks before evicting the source allocation. Raster and
four-format model cases still verify exact-submission completion. Application
105 tests passed with six optional ignored; the hardware test passed explicitly.

A current native-window run on Metal Apple M4 Max logged surface acquisition as
occluded before RAW Ready, then `GPU upload consumer lease released; pending=0`,
followed by `first window frame presented generation=0 total_ms=2280.779`.
That last event records CPU surface-present submission, not GPU completion or
physical scanout. The run therefore does not prove that the surface remained
occluded during upload completion; the surface-independent path is covered by
the separate headless nonblocking-poll test. The owned viewer process was
terminated after bounded observation. Evidence:
`/tmp/rrrah-hidden-lease-window.log`.

## Hasselblad X1D 3FR cache-tier measurement

On 2026-10-05, `raw_view_timing --cache-only` compared the full 105,705,472-byte sensor on Apple M4 Max with a 512 MiB managed CPU budget. Four warmup rounds precede sixteen measured rounds; tier order rotates each round. Full pixels and metadata are checked after each decode/restore outside timing. RAM lookup shares existing pixels. Source, cache and swap files are warm; this excludes GPU upload, first physical display, cold storage, and OS/allocator overhead.

| Path | p50 ms | p95 ms |
| --- | --- | --- |
| Native decode | 79.044000 | 96.940333 |
| Persistent restore | 110.434959 | 129.557250 |
| Swap restore | 103.687792 | 122.224417 |
| RAM lookup | 0.000163 | 0.000170 |

Evidence: `target/bench/3fr-cache/current.csv` and `current.log`. The managed root peak was 321,789,952 bytes because the expected sensor stays resident during the comparison; the restore child peak was 105,705,472 bytes. Both return to zero after teardown. This fixture supports using the existing explicit decode-first policy when RAM misses; it does not establish an automatic policy or universal speed advantage.

## Hasselblad X1D completed Metal frame

A separate 2026-10-05 `raw_view_timing` run rendered 1920×1080 on Metal Apple M4 Max with CPU 512 MiB and GPU 256 MiB managed limits. Three warmup iterations precede fifteen measured decode/upload/completed-frame iterations. Sixty resident zoom/pan frames follow three resident warmups. This is an offscreen submission-completion measurement, not window presentation or scanout. It checks repeated sensor identity outside timing; independent pixel readback is covered by `hasselblad_3fr_readback` separately.

| Stage | p50 ms | p95 ms |
| --- | --- | --- |
| Decode | 48.634 | 49.353 |
| Upload enqueue | 29.436 | 30.431 |
| Completed frame after enqueue | 15.469 | 19.975 |
| Total decode to completed frame | 94.227 | 98.576 |
| Resident zoom/pan | 2.705 | 3.268 |

Evidence: `target/bench/3fr-cache/view.csv` and `view.log`. CPU managed peak is 321,789,952 bytes and GPU managed peak is 264,241,152 bytes; both return to zero. GPU accounting excludes driver staging/overhead. Decode timings differ from the separate cache-tier run; the two runs do not establish a causal speed change. Resident frames reuse uploaded sensor data and do not repeat decode or disk restoration.

## CFV-50 FFF cache-tier baseline

2026-10-05, Apple M4 Max, optimized dev profile, `raw_view_timing --cache-only`, CPU managed limit 512 MiB. Four warmups and sixteen measured rotating-order rounds compare the 103,359,360-byte decoded sensor. Full pixels/metadata are checked outside timing after every decode/restore. All files are warm; GPU/window presentation is excluded.

| Path | p50 ms | p95 ms |
| --- | --- | --- |
| Predictor-8 decode | 1232.098250 | 1382.579208 |
| Persistent restore | 69.959333 | 81.827750 |
| Swap restore | 69.498041 | 80.612583 |
| RAM lookup | 0.000171 | 0.000198 |

Evidence: `target/bench/fff-cache/baseline.csv` and `baseline.log`. The root peak is 286,158,234 bytes and the restore child peak is 103,359,360 bytes; both finish at zero. This supports cache-first for this compressed FFF sample. The earlier uncompressed X1D 3FR sample favored decode-first; file extension alone is insufficient to establish a universal policy. The current native Huffman decoder reads categories bit by bit; optimizing it requires a fresh paired or alternating measurement and full-sensor regression qualification.

### Predictor-8 prefix lookup follow-up

The decoder now resolves codes of up to eight bits through a 256-entry u16 prefix table (512 bytes), falling back to canonical bit-by-bit parsing for longer codes and short final tails. Full 51,679,680-sample comparison, truncation/cancellation, signed pair/reset fixtures, long-code fallback and one-bit final-tail tests pass.

The follow-up `target/bench/fff-cache/prefix.csv` / `prefix.log` records decode p50 880.116583 ms and p95 1560.226333 ms; disk p50 69.693833 ms and swap p50 67.282709 ms. The root/restore peaks remain unchanged and both finish at zero. This is a sequential follow-up with a brief concurrent test compilation, not a paired isolated comparison: lower median does not prove a universal improvement and the higher p95 remains unresolved. Cache-first still wins on this fixture.

### Paired predictor-8 confirmation

`paired_prefix_vs_bitwise_full_sensor_timing` specializes both entropy paths in the same optimized test binary. Twelve alternating-order pairs use the same resident input: four warmups then eight measured pairs. Each pair compares every decoded sample outside timing. Source loading, metadata, cache, GPU and physical presentation are excluded. No concurrent compilation was launched during sampling.

Bitwise p50/p95: 1233.224833/1243.255833 ms. Prefix p50/p95: 844.488208/854.250209 ms. All eight paired differences favor prefix by 376.358375–398.358708 ms. Evidence: `/tmp/rrrah-fff-paired.log`; the executable benchmark remains in the entropy module as an explicitly ignored external-fixture test. This confirms improvement for the pinned CFV-50 sample, not other cameras, distributions or cold system states. It resolves the noisy earlier sequential result for this measured workload.

## Leaf Aptus 22 MOS cache tiers

2026-10-06, Apple M4 Max, optimized dev profile, `raw_view_timing --cache-only`,
managed CPU cap 256 MiB. Four warmup rounds and sixteen measured rounds rotate
path order. Full sensor samples and metadata are compared outside timing. Source,
disk cache and swap files are warm; GPU upload, window presentation, cold storage,
allocator overhead and total RSS are excluded.

| Path | p50 ms | p95 ms |
| --- | --- | --- |
| Native decode | 333.715417 | 334.738708 |
| Persistent restore | 28.811584 | 29.240292 |
| Swap restore | 27.804666 | 28.100583 |
| RAM lookup | 0.000163 | 0.000180 |

Evidence: `target/bench/mos-cache/current.csv` and `current.log`. Sensor payload is
42,837,504 bytes; managed root peak 109,098,641 bytes, restore peak 42,837,504 bytes;
both return to zero. This supports cache-first for the pinned Aptus 22 sample,
not a universal MOS policy. Separate actual Metal Apple M4 Max integration checks
full 128x96 frame equality against independent sensor/profile reference, RAM
shared-buffer leases and atomic limit refusal, persistent metadata/pixel equality,
swap restoration under pressure and retry, and zero managed usage after teardown.
Six malformed-source variants and an 8 MiB admission refusal release reservations.
Logs: `/tmp/rrrah-mos-metal.log`, `/tmp/rrrah-mos-rejection-integration.log`.
Physical navigation and HDR presentation remain outside this qualification.


## Automatic RAW policy under managed memory pressure

On 2026-10-06, the actual foreground recovery helper was exercised with the pinned
Hasselblad X1D 3FR. A 256 MiB budget prepares the expected native sensor and stores
it persistently. The same source under 128 MiB returns typed allocation admission
failure and releases its reservations. Disk recovery under that 128 MiB root then
returns identical full pixels and metadata. A separate empty persistent-cache
root exercises the swap branch, also retaining full identity and releasing usage
after the returned buffer drops. Initial cancellation returns no object.

Policy regression covers temporary decode deferral after memory refusal, periodic
probes, successful decode clearing deferral, and exclusion of failed operations
from successful cost samples. Four policy/helper tests pass in
`/tmp/rrrah-auto-pressure-real.log`. This does not measure physical navigation,
GPU/display latency, cold storage, or an automatic-policy speed advantage.

## Automatic policy live-window observation

On 2026-10-06, the actual `rrrah` window ran with `RRRAH_RAW_LOAD_POLICY=auto`,
forced Metal, a 512 MiB managed CPU cap, zero RAM cache and disabled prefetch.
The runtime confirmed Metal Apple M4 Max and recorded 27 first-frame present
calls across observed generations, including cached Ready states. Initial
startup-to-present was 2056.892 ms and included device/window startup and an
occluded-surface deferral. It is not comparable to steady-state fixture timings.
The owned process subsequently exited successfully.

Navigation during this run was uncontrolled; this does not qualify per-source
learning, a paired latency improvement, physical scanout or HDR output. Snapshot
evidence: `/tmp/rrrah-auto-window-observation.log` and
`/tmp/rrrah-auto-window-observation.json`. The policy now emits debug records with
source/recipe cache key, sample counts, EWMA costs, deferral and probe decisions;
these support a subsequent controlled learning audit. Debug tracing is off unless
the logger enables it. Application regression remains 109 passing tests with
seven explicit external-fixture/device tests ignored in the ordinary run.


## Phase One P20+ IIQ cache and view timing

2026-10-06, Apple M4 Max, optimized dev profile. The pinned CC0 object 4366
uses native format-3 entropy and all calibrated sensor correction stages.
`raw_view_timing --cache-only` ran under a 256 MiB managed CPU cap with four
warmup rounds and sixteen measured rounds, rotating tier order. Full samples
and metadata are checked outside timing. Source, cache and swap files are warm.

| Path | p50 ms | p95 ms |
| --- | --- | --- |
| Native decode | 470.140667 | 716.734542 |
| Persistent restore | 36.535167 | 64.561417 |
| Swap restore | 35.290959 | 42.851000 |
| RAM lookup | 0.000259 | 0.000289 |

Evidence: `target/bench/iiq-cache/current.csv` and `current.log`. Managed root
peak is 89,756,494 bytes with the expected sensor retained for comparison;
restore peak is 34,130,304 bytes. Both return to zero after teardown. This
supports cache-first for this fixture and workload; it does not establish a
universal IIQ policy or a cold-storage advantage.

Separate Metal Apple M4 Max view timing completed 1920x1080 offscreen frames,
with three warmups and fifteen measured decode/upload/render runs. Total
p50/p95 is 521.451/1110.616 ms; decode 499.087/1094.017 ms; upload enqueue
9.689/13.252 ms; frame completion 8.867/11.648 ms. Sixty measured resident
zoom/pan frames give 3.282/6.526 ms p50/p95. The substantial decode tail is
reported rather than hidden by averages. These separate runs do not support
paired timing comparisons between CPU/cache and GPU workloads.

Evidence: `target/bench/iiq-view/current.csv` and `current.log`. Managed CPU/GPU
peaks are 137,414,528 and 68,425,984 bytes; both report zero after teardown.
These caps cover managed buffers and renderer-owned textures, not total process
RSS or driver allocations. Cold storage, physical scanout, window latency and
physical HDR are outside this measurement. Independent full sensor and Metal
readback equality were qualified separately in `tests/fixtures/iiq-investigation.json`.


## IIQ native stage timing

The ignored `real_pipeline_stage_timing_preserves_oracle` benchmark isolates
native stages after reading the pinned source, with three warmups and twelve
measured runs. Each complete output is compared to the independent corrected
sensor outside timing. This is sequential warm CPU work, not a full load or
GPU benchmark, and the stage percentiles must not be summed as an end-to-end
percentile.

| Stage | p50 ms | p95 ms |
| --- | --- | --- |
| Private tables, metadata, row offsets | 0.096333 | 0.108750 |
| Entropy decode | 297.270292 | 304.172208 |
| Black subtraction | 9.192250 | 10.412959 |
| Individual pixel repair | 0.175084 | 0.451750 |
| Luma grid 0x416 | 65.298958 | 67.345958 |
| Luma grid 0x410 | 71.953916 | 74.617791 |
| Chroma grid 0x40b | 101.564125 | 109.435750 |
| Column repair | 5.799958 | 6.425417 |

Evidence: `/tmp/rrrah-iiq-stage-timing.log`; all fifteen full-sensor outputs
match the independent oracle. These observations identify entropy and gain-grid
loops as the next CPU optimization targets. Metadata/profile lookup is small
in this fixture. No production speed improvement is claimed by this profiling
change; future loop changes need paired measurements and full sample equality.


## IIQ flat-field specialization: paired verification

The native gain-grid code now specializes channel count at compile time (luma
and red/blue chroma), allowing constant loop bounds and branches to be removed.
Arithmetic order, bounds and cancellation points are preserved. The former
runtime-channel implementation remains test-only as the paired baseline.

Two warmup pairs and eight measured pairs alternate implementation order. The
same sensor after black/pixel repair is cloned outside timing; timing covers
only the three gain grids. Subsequent column repair and full corrected-sensor
comparison are outside timing. Every output matches the independent original
LibRaw corrected oracle, including all twenty warmup/measured outputs.

Baseline p50/p95: 291.127459/335.507458 ms. Specialized p50/p95:
85.596084/92.688000 ms. All eight paired savings are positive, ranging from
194.044707 to 244.347750 ms. Evidence: `/tmp/rrrah-iiq-flat-paired.log`;
`real_flat_field_specialization_paired` is the repeatable ignored benchmark.
This proves improvement of these grids for the pinned P20+ workload, not full
load/window latency or other IIQ cameras. Earlier full-load timing above predates
this change and is retained as historical evidence rather than relabeled current.


## IIQ after specialization: current load and view observations

The optimized production code was remeasured with the same pinned source and
managed caps. Cache timing uses four warmup rounds and sixteen measured rounds
with rotating path order. Full sensor and metadata checks remain outside timing.

| Path | p50 ms | p95 ms |
| --- | --- | --- |
| Native decode | 218.991542 | 269.342958 |
| Persistent restore | 27.164917 | 33.911542 |
| Swap restore | 26.261375 | 33.373708 |
| RAM lookup | 0.000192 | 0.000227 |

Evidence: `target/bench/iiq-cache-specialized/current.csv` and `current.log`.
Root/restore peaks remain 89,756,494/34,130,304 bytes, both zero after teardown.
These current warm observations still support cache-first for the pinned P20+.
Compilation of an independent integration test overlapped part of this cache
run; the separate paired gain-grid experiment above is the causal speed evidence.

The subsequent isolated Metal Apple M4 Max view run uses three warmups and fifteen
measured completed 1920x1080 offscreen frames. Total p50/p95 is
248.387/271.292 ms; decode 235.951/258.640 ms, upload enqueue 8.198/8.602 ms,
frame completion 6.535/9.454 ms. Sixty resident zoom/pan frames measure
1.127/1.509 ms p50/p95. Evidence: `target/bench/iiq-view-specialized/current.csv`
and `current.log`. CPU/GPU managed peaks remain 137,414,528/68,425,984 bytes,
both returning to zero. The old and new whole-load observations are not paired,
so no exact end-to-end speedup factor is inferred from them.

Actual independent full-sensor and whole-frame Metal comparisons through native,
RAM, disk and swap were rerun after specialization and passed:
`/tmp/rrrah-iiq-specialized-metal.log`. Physical display, HDR scanout, cold storage
and total RSS remain outside this qualification.

## Sony DSC-F828 SRF warm tier/view timing (2026-10-06)

Pinned CC0 object 1351, release binary, Apple M4 Max. The tier run uses four
warmup rounds and sixteen measured rotating-order rounds, checking complete
sensor/metadata identity outside timed intervals. 128 MiB managed CPU root.
Logs/CSV: `target/bench/srf-cache-current/current.{log,csv}`.

| Warm path | p50 ms | p95 ms |
| --- | ---: | ---: |
| Native decode | 6.017292 | 6.227833 |
| Persistent disk restore | 9.067708 | 10.495042 |
| Temporary swap restore | 8.669958 | 8.820167 |
| RAM shared lease | 0.000139 | 0.000165 |

Root peak 50,455,744 bytes; restore child peak 16,531,200 bytes. Both returned
to zero. This source favors repeated native decode over warm serialized restore
when RAM misses; it does not justify an extension-wide loading policy.

A subsequent separate forced-Metal run renders a completed offscreen 1920x1080
frame: three warmups, fifteen measured samples. Decode p50/p95 6.060/6.601 ms,
upload enqueue 4.576/6.192 ms, frame completion 5.719/6.520 ms, total
16.429/19.977 ms. Sixty resident zoom/pan frames: 1.362/1.950 ms. Managed CPU
peak 50,455,744, GPU atlas peak 50,331,648 bytes; both zero after teardown.
Logs/CSV: `target/bench/srf-view-current/current.{log,csv}`.

Build finished before either series; the two timing runs were sequential.
These are warm single-source timings, not cold-storage, sustained navigation,
physical scanout, HDR display or CUDA/NVIDIA evidence. RAM lease acquisition is
not a completed-frame latency. GPU limits cover tracked atlas ownership, not
driver allocations.
