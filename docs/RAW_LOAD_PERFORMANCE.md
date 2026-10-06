# Current native RAW viewing measurements

## Qualified NRW/RWL load measurements

The current `raw_view_timing` executable runs the three pinned corpus sources
on Metal Apple M4 Max with managed CPU/GPU limits of 256/128 MiB. Three warmups
precede fifteen measured loads; source mosaics are verified outside timing.
This uses a completed 1920x1080 offscreen frame and warm OS file cache.

| Source | Decode p50/p95 ms | Total completed-frame p50/p95 ms | Resident view p50/p95 ms |
| --- | ---: | ---: | ---: |
| P7800 NRW (1425) | 7.413/8.741 | 17.653/20.093 | 1.238/2.276 |
| P7700 NRW (5495) | 7.672/8.348 | 15.836/16.816 | 1.331/1.645 |
| D-LUX Typ 109 RWL (807) | 36.100/36.373 | 45.323/47.378 | 1.062/1.950 |

Tracked CPU allocation peak is 83,084,168 bytes, GPU peak 50,331,648 bytes;
both final usage counters are zero. This is tracked resource ownership, not
total process RSS or driver memory. CSV, logs, machine metadata and executable/
source SHA-256 hashes are in `target/bench/nrw-rwl-load`. Earlier fixture files
under target/qualification were unavailable after rebuilding, so this run uses
the pinned copies in `/tmp/rrrah-raw-corpus`. These observations are not an A/B
speedup, first-visible measurement, cold-storage result or universal RAW claim.

The same executable's `--cache-only` mode measures managed CPU decode,
persistent disk restore, session swap restore and RAM lookup for these sources.
Full sensor/metadata equality is checked outside timing; RAM lookup batches
1,000 operations and verifies shared allocation identity. CPU budget is 256 MiB.

| Source | Decode p50 ms | Disk p50 ms | Swap p50 ms | RAM p50 ms |
| --- | ---: | ---: | ---: | ---: |
| P7800 NRW | 6.42 | 16.45 | 16.04 | 0.000143 |
| P7700 NRW | 7.37 | 16.59 | 16.26 | 0.000139 |
| Leica RWL | 36.13 | 13.88 | 13.35 | 0.000141 |

Raw exact values and samples are in `target/bench/nrw-rwl-load/cache.log` and
`cache.csv`. All final root/restore usage is zero. These warm-cache CPU results
favor source decode for the two NRW files and swap/disk restore for this RWL.
They exclude GPU upload and presentation and do not establish a per-format
universal rule. Automatic source-versus-restore selection is not implemented
by these measurements; the production cache preference remains unchanged.

Measured 2026-10-05 on Metal Apple M4 Max / macOS 26.7.1, using the current optimized development build (workspace opt-level 1, dependency opt-level 3). Both Canon EOS R8 CR3 files decode the complete 6188×4120 sensor mosaic; no embedded JPEG or decoded-mosaic cache is used. Filesystem cache and renderer/shader setup are warm. These are absolute measurements of the current worktree, not an A/B speedup or complete RAW-family qualification.

## CPU decode

`RRRAH_CR3_BENCH_REPS=15 RRRAH_CR3_BENCH_WARMUPS=3 RRRAH_CR3_BENCH_VARIANT=current-native cargo run -p rrrah-decode --example cr3_end_to_end_timing -- tests/IMG_9043.CR3 tests/IMG_9074.CR3`

| Fixture | Full read/decode p50 ms | p95 ms | Workers |
|---|---:|---:|---:|
| IMG_9043.CR3 | 42.502 | 43.254 | 4 |
| IMG_9074.CR3 | 42.241 | 42.360 | 4 |

The harness checks full mosaic BLAKE3 identity outside timing on every repetition. This excludes application source/cache bookkeeping, GPU and presentation. Raw sample files are in `target/bench/raw-load/native-cpu.csv`.

## Read/decode through completed GPU frame

`cargo run -p rrrah --example raw_view_timing -- tests/IMG_9043.CR3 tests/IMG_9074.CR3`

The benchmark uses the production `RawRenderer`, a 1920×1080 sRGB offscreen render target and `Device::poll` waiting for the submitted frame. It includes file read/decode, actual mosaic upload, view update, command encoding/submission and completion of queued upload/render work. Renderer/device/target construction precedes timing. Each source has three warmups and fifteen measured loads; every decoded mosaic is compared exactly with the first decode outside all timed intervals. Empirical p95 from fifteen samples has limited tail precision.

| Fixture | Decode p50 ms | Upload preparation/enqueue p50 ms | Submitted frame completion p50 ms | Total p50 ms | Total p95 ms |
|---|---:|---:|---:|---:|---:|
| IMG_9043.CR3 | 46.823 | 14.753 | 10.176 | 71.729 | 81.258 |
| IMG_9074.CR3 | 45.730 | 14.851 | 10.092 | 71.252 | 72.303 |

Stage medians are separate statistics and need not sum to the total median. Upload preparation/enqueue includes CPU packing and enqueueing, not GPU completion; the subsequent frame-completion interval also waits for queued texture copies. CPU-only and GPU runs have different workloads and are not a paired regression comparison.

After the last load, sixty completed resident zoom/pan frames follow three warmups. No reread, decode or mosaic upload is performed: only view uniforms and rendering change. IMG_9043 p50/p95 is 1.293/1.476 ms; IMG_9074 is 1.355/2.466 ms. These are serialized command-and-GPU completion latencies, not measured screen FPS or swapchain pacing.

This harness does not include process startup, cold file I/O, application request/source/cache bookkeeping, window scheduling, swapchain acquisition, vsync or physical display. First-visible and live interaction remain unmeasured. It therefore cannot claim a 71 ms request-to-visible delay. Output samples, adapter/summary log and environment metadata are in `target/bench/raw-load/gpu-view.csv`, `gpu-view.log` and `metadata.json`.

Sources: IMG_9043 is 22,382,226 bytes, SHA-256 `d06cbb10e84882cf130430bdc88ed2652c2622b96dea8d6a854c09e3bee37e59`; IMG_9074 is 21,368,466 bytes, SHA-256 `9c0d47cc2dd550eb6dc26c4ba2b2874b67f5c89e79dff5c472abf0ac6b2f6178`. The earlier July CR2/DNG/cache measurements cover different inputs and/or code and cannot establish a current CR3 viewing speedup.


## RAM and swap read timing (2026-10-05)

Run the existing viewer benchmark with `--cache-only`:

```sh
cargo run -p rrrah --example raw_view_timing -- --cache-only tests/IMG_9043.CR3 tests/IMG_9074.CR3
```

On this Apple M4 Max, optimized dev profile, warm OS file cache, three warmups
and fifteen measured iterations per fixture:

| Fixture | Native decode p50/p95, ms | Swap restore p50/p95, ms | RAM hit p50/p95, microseconds |
| --- | --- | --- | --- |
| IMG_9043.CR3 | 44.789 / 46.273 | 47.271 / 48.753 | 0.129 / 0.133 |
| IMG_9074.CR3 | 45.557 / 52.934 | 48.615 / 51.826 | 0.125 / 0.138 |

Each swap read allocates a fresh budgeted pixel buffer, streams the full mosaic,
and verifies BLAKE3 before publication. Serialization/setup are outside the read
interval. Complete metadata and samples are compared with native decoding outside
all timed intervals. RAM timing averages 1000 real lookups/clones per iteration;
shared allocation identity is checked outside timing. No GPU upload, physical
presentation, cold-storage access or end-to-end navigation is measured here.

The initial scratch-buffer implementation had higher median latency than native
decode for these two CR3s. The direct-read follow-up below supersedes this result.
Evidence: `target/bench/raw-cache/{timings.csv,run.log,metadata.json}` (90 samples).


### Direct managed-buffer reads

A follow-up run on the same two fixtures reads little-endian pixel bytes directly
into initialized budgeted u16 storage using a safe byte slice. It removes the
scratch-buffer copy and scalar reconstruction on little-endian hosts; big-endian
hosts convert each sample in place. BLAKE3, length validation and cancellation
remain active. All metadata and pixels matched native decode outside timing.

Swap restore p50/p95 was 34.468/35.513 ms for IMG_9043 and 35.014/37.918 ms for
IMG_9074, versus 47.271/48.753 and 48.615/51.826 ms in the earlier run. These are
consecutive runs, not an interleaved A/B experiment. Native decode in the follow-up
run remains about 46 ms. This demonstrates faster warm-cache restoration for
these two fixtures only; no cold-storage or physical-display claim follows.
Evidence: `target/bench/raw-cache/direct-read/{timings.csv,run.log,metadata.json}`.
The big-endian branch has not been executed on this little-endian machine.

## Managed full-resolution view qualification (2026-10-05)

The existing raw_view_timing example now accepts RRRAH_VIEW_CPU_MB and
RRRAH_VIEW_GPU_MB for view mode. The same CPU budget admits native decode output
and RAW tile/row packing; GPU admission includes atlas replacement overlap.
After completed render/pan/zoom work it drops the renderer and asserts both
managed budgets have no retained reservations. Invalid/overflowing environment
values fail; --cache-only uses its separate RRRAH_CACHE_CPU_MB budget (see the managed tier follow-up).

On Metal Apple M4 Max, tests/IMG_9043.CR3 (6188x4120), with 512 MiB per budget,
15 measured samples after three warmups gave total completed offscreen frame
p50/p95 72.573/77.482 ms. Decode was 45.413/46.554 ms, upload enqueue
17.056/17.482 ms, and completed rendering 10.248/13.058 ms. Sixty resident pan/zoom
frames after three warmups gave 1.336/2.194 ms p50/p95. These are warm OS-cache
1920x1080 offscreen measurements, not physical presentation or cold-file latency.

CPU managed peak was 124360466 bytes; GPU atlas peak was 146800640 bytes.
The benchmark retains the first mosaic allocation as a repeatability reference,
so CPU peak includes that extra owner plus current decode/packing reservations.
A second full run with CPU 119 MiB and GPU 140 MiB succeeded, retaining the same
peaks and releasing both to zero; its total p50/p95 was 72.798/76.565 ms.
GPU 139 MiB refused replacement with requested=73400320, used=73400320,
limit=145752064, proving old/new atlas overlap is enforced on the real file.
Timing configurations were consecutive, not a counterbalanced performance A/B.

Fixture SHA256: d06cbb10e84882cf130430bdc88ed2652c2622b96dea8d6a854c09e3bee37e59.
Executable SHA256: 1226d2a61efa6e0ff8a3b597a97e4c4983df5e5be2a94da686fb5729e856d80c.
Raw samples/logs: target/bench/managed-raw-view. Pixel repeatability is checked
outside timed intervals; this run does not replace the independent LibRaw color
qualification. Managed counters exclude remaining decoder scratch, queue staging,
driver overhead and in-flight resource retention; they are not RSS/VRAM readings.

## Managed RAW cache tiers, rotating order (2026-10-05)

Run on Apple M4 Max/macOS with the current optimized dev-profile example:

```sh
cargo run --locked -p rrrah --example raw_view_timing -- --cache-only tests/IMG_9043.CR3
```

Cache-only now uses `RRRAH_CACHE_CPU_MB`, default 512 MiB, for native source/output
and restored pixels. Queue occupancy is independent; disk and swap restores share
a child capped at twice the decoded pixel capacity. Source fingerprint and native
recipe form the same key used by the viewer. Setup writes both tiers outside read
timing. Four warmup rounds precede sixteen measured rounds. Decode, disk, swap and
RAM rotate positions each round, giving every tier four appearances per position.
All full results are checked outside timing for exact metadata/sample equality;
restores must be managed, and RAM hits must share the reference allocation.
RAM samples batch 1000 hits and report time per hit. No GPU work is included.

| Path | p50 ms | p95 ms |
| --- | ---: | ---: |
| Native decode | 45.239667 | 45.994875 |
| Verified persistent-cache restore | 34.529042 | 37.551875 |
| Verified temporary-swap restore | 35.063208 | 35.893625 |
| RAM lookup/shared clone | 0.000147 | 0.000169 |

EOS R8 frame: 6188x4120, 50,989,120 retained sensor-pixel bytes. Managed root peak
124,360,466 bytes; restore child peak 50,989,120 bytes. Both used counters return
to zero after all frame/cache owners drop. These are managed reservations, not
RSS. The reference frame stays allocated throughout comparisons. OS/file caches
are warm. Timed decode/read intervals exclude result destruction, validation,
source fingerprinting, initial serialization, GPU upload and physical presentation.
This measures cache-tier primitives, not complete navigation or preload scheduling.
The first fixed-order run had a decode p95 outlier; the table reports the revised
rotating-order run and does not infer a cause for that outlier.

Evidence: `target/bench/managed-cache-tiers/eos-r8-rotating.csv` and
`eos-r8-rotating.log` (64 measured CSV rows). Source SHA-256:
`d06cbb10e84882cf130430bdc88ed2652c2622b96dea8d6a854c09e3bee37e59`.
Tested executable SHA-256:
`db6d9a053a5204749367dc1e8ef29d75bfac7e4dcbf0ed80d9d7ec16c1897f30`.

## Swap limit and cancellation follow-up (2026-10-05)

Repeated the same managed rotating-order benchmark after adding cancellable,
coalesced swap writes, configurable waiting-job counts and enforced local budget
caps. The benchmark exercises completed-object restores; it does not measure
queue contention, cancellation latency or physical navigation.

| Path | p50 ms | p95 ms |
| --- | ---: | ---: |
| Native decode | 45.407333 | 45.562333 |
| Verified persistent-cache restore | 34.278834 | 34.595875 |
| Verified temporary-swap restore | 34.338083 | 35.457291 |
| RAM lookup/shared clone | 0.000146 | 0.000157 |

All 64 measured rows (16 per tier) use the same 50,989,120-byte EOS R8 mosaic.
Exact metadata/pixel checks passed on every full decode/restore. Root peak remains
124,360,466 bytes and restore-parent peak 50,989,120 bytes; both used counters
return to zero. Warm OS caches and the exclusions above still apply. A single
repeat does not establish a statistically significant performance improvement.

Evidence: `target/bench/swap-limits-followup/eos-r8.csv` and `eos-r8.log`.
Source SHA-256 is unchanged from the preceding record. Tested executable SHA-256:
`a81fb6db9f21d3a52f8f979e6046ac2122cd42a3b33008fe9c2b7e6d45243e92`.
# Panasonic FZ50 native RAW, cache and Metal timing

Apple M4 Max / Metal, corrected mode 34828, pinned raw.pixls.us object 2234.
Current `raw_view_timing` executable rebuilt from source; CPU cap 256 MiB,
GPU cap 128 MiB. Fifteen loads after three warmups at 1920 x 1080:
decode p50/p95 8.062/8.250 ms, upload enqueue 5.676/6.437 ms,
frame completion 3.299/3.896 ms, total 16.953/18.388 ms.
Sixty resident zoom/pan frames: p50/p95 1.166/1.452 ms.
Tracked CPU peak 60,908,676 bytes and GPU atlas peak 50,331,648 bytes;
both final usage counters zero. This excludes physical display latency,
source fingerprinting, total process RSS and driver overhead.

Separate rotated cache-only samples verify every reconstructed sensor and its
metadata outside timing: decode p50/p95 7.976/8.113 ms, persistent disk
13.723/14.018 ms, swap 13.182/13.638 ms, RAM shared-owner lookup
0.000137/0.000145 ms. Disk/swap restoration was slower than decoding this
particular warmed source. Automatic selection was not implemented at measurement time;
the later optional `auto` policy still needs physical navigation qualification.
Root/restore budget usage returned to zero.

Evidence: `/tmp/rrrah-panasonic-view-timing.csv`,
`/tmp/rrrah-panasonic-view-timing.log`,
`/tmp/rrrah-panasonic-cache-timing.csv`,
`/tmp/rrrah-panasonic-cache-timing.log`.

## Sony DSC-R1 SR2, 2026-10-05

Pinned CC0 object 3221 (SHA256
`921c5f2513dd1671a9089e73fc778fc3573bc4cb70a3a555d0414661d0e62274`),
3984×2608 sensor, ARW backend revision 6. Current optimized dev build, Metal
Apple M4 Max. View and cache runs were executed sequentially, with warm OS file
cache. Commands after `cargo build --locked -p rrrah --example raw_view_timing`:

```sh
RRRAH_GPU_BACKEND=metal RRRAH_GPU_VENDOR=any RRRAH_VIEW_CPU_MB=256 RRRAH_VIEW_GPU_MB=128 target/debug/examples/raw_view_timing /path/to/3221.sr2
RRRAH_CACHE_CPU_MB=256 target/debug/examples/raw_view_timing --cache-only /path/to/3221.sr2
```

| Operation | p50 ms | p95 ms |
| --- | ---: | ---: |
| Decode in view run | 7.852 | 7.904 |
| Upload enqueue, including packing | 4.769 | 5.046 |
| Completed GPU frame | 6.804 | 7.655 |
| Decode through completed 1920×1080 frame | 19.546 | 20.499 |
| Resident zoom/pan | 1.230 | 1.280 |
| Decode in cache comparison | 7.842459 | 7.871791 |
| Persistent cache restore | 13.801042 | 13.876500 |
| Swap restore | 13.380667 | 13.594167 |
| RAM lookup/shared ownership clone | 0.000157 | 0.000161 |

View: 15 samples after 3 warmups, resident view: 60 frames after 3 warmups.
Cache: 16 measured samples after 4 warmups with operation order rotation.
All restored samples/metadata are checked outside the read timing. The RAM
lookup result does not include upload or drawing. Managed CPU peak was
62,596,576 bytes, atlas texture peak 50,331,648 bytes; both counters returned to
zero. These counters exclude allocator/driver bookkeeping and the render target.

The comparison supports preferring native decode over persistent/swap reads for
this warmed fixture. It does not establish a universal RAW policy or cold-disk
performance. The default application policy remains cache-first; an adaptive selector
remains to be implemented and qualified. Offscreen GPU
completion is measured, not physical display latency or HDR presentation.

## Foreground RAW source policy

`RRRAH_RAW_LOAD_POLICY=decode-first` makes the window's foreground RAW loader
try resident RAM first, then native decode, bypassing swap and persistent-cache
reads. Cache admission/write-back stays enabled, so subsequent RAM hits still
benefit. `cache-first` (the default) retains RAM/swap/persistent-cache lookup.
Invalid values fail loader initialization explicitly. The policy is fixed at
loader creation and carried across latest-wins navigation requests; it does not
change raster/model loading or background prefetch policy. Native failures are
reported normally rather than silently replaced with a cached result.

```sh
RRRAH_RAW_LOAD_POLICY=decode-first cargo run --locked -p rrrah -- /path/to/3221.sr2
```

This manual choice is available to apply measured results such as DSC-R1's faster
native decode. A later optional `RRRAH_RAW_LOAD_POLICY=auto` learns successful
decode/restore costs per source/recipe with bounded history and hysteresis; see
`STORAGE_LIBRARY_CONTRACT.md`. It is not yet qualified for physical navigation latency.

### Window runtime observation

The current binary was launched with `decode-first`, an already populated test
cache, a 256 MiB managed budget and Metal on the DSC-R1 fixture. Debug output
confirmed generation 0 selected native decode and the window surface selected
Apple M4 Max. The first-present debug event was not observed. A process sample
showed the main thread in the AppKit event loop, not an active decode or GPU
initialization call. The standalone executable was not selectable through the
available native UI tool. Therefore this run does **not** qualify visible
presentation or navigation. The test process was terminated after inspection.

New debug events record foreground policy, native selection, RAM/restore hits
and the first successful CPU surface-present request. That request event is
not GPU completion or physical display scanout. Offscreen SR2 readback remains
independently qualified in `sony_sr2_readback`.

A subsequent instrumented launch confirmed `RAW Ready ... GPU available=true`
and `viewer surface acquisition deferred: occluded` for both redraw attempts.
The deferred presentation is therefore an observed Metal surface visibility
condition, not evidence of missing decode/upload. The window event handler now
explicitly requests redraw on `Occluded(false)`, retaining the existing uploaded
frame and pending first-present timing. Surface validation rejection is logged
separately. A real visibility transition and visible presentation remain
unqualified because the available UI tool cannot select this executable.
