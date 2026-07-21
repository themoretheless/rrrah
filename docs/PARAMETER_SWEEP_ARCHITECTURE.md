# Parameter-sweep architecture for GPU and benchmark work

**Дата:** 2026-07-21  
**Роль:** parameter-sweep architect  
**Ограничение:** этот документ не добавляет benchmark code и не подменяет
CR2/DNG corpus синтетическим RAW. Он определяет, что можно измерить сейчас,
что требует будущего scheduler/residency API, и как не получить ложные
speedup-цифры.

## Исполняемый контур сегодня

Текущий `RawRenderer` делает один full-screen fragment pass. При upload он
строит eager `R16Uint` texture-array: `tile_halo=1`, а `tile_size` вычисляется
из adapter limit и ограничивается 4096. `ViewParameters` задаёт physical
viewport, pan, zoom и exposure. `algorithm` присутствует в uniform, но пока не
выбирает алгоритм: shader всегда выполняет bilinear demosaic + color matrix +
ACES fitted tone map.

`crates/rrrah-gpu/examples/gpu_smoke.rs` уже поднимает no-display wgpu adapter и
рендерит deterministic synthetic mosaic. Он печатает CSV с фактическим
adapter/backend, upload enqueue/wait и render p50/p95/p99. Но его dimensions,
viewport, zoom, warmups и repetitions пока compile-time constants; он не даёт
CLI для `tile_size`, `halo`, workers или cache.

`scripts/bench-harness.py` запускает только процессный `rrrah --inspect` с
реальным fixture. Его `--workers` и `--backend` — **labels**, они не передаются
в приложение и не меняют scheduler/backend. Поэтому до добавления headless
GPU runner нельзя публиковать их как измеренную параллельность.

Без CR2/DNG файла можно честно измерять:

* Naga/WGSL parse и validation;
* synthetic `DecodedMosaic` upload/render через
  `crates/rrrah-gpu/examples/gpu_smoke.rs` (Metal/Vulkan/DX12 where an adapter
  exists);
* размер и feasibility tile-array, host-side row padding и halo invariants;
* synthetic `DiskMosaicCache`/`WeightedLru` read/write, eviction и byte budget;
* telemetry serialization/JSONL/Chrome trace overhead.

Нельзя измерить без реального corpus:

* CR2 entropy/predictor latency, restart-marker scaling и bytes read;
* DNG IFD/tile/strip decode, compression mix и metadata-only probe;
* production first-visible latency `probe + decode + post + upload`;
* camera-specific black/white grids, DCP/OpcodeList и quality against a real
  reference TIFF.

Синтетический GPU результат должен иметь fixture URI вида
`synthetic://mosaic/{width}x{height}/u16/{cfa}/seed-{seed}` и маркироваться
`synthetic`, не `CR2`/`DNG`.

## Десять sweep-параметров

| ID | Параметр | Рекомендуемые уровни | Измеримый статус | Основной эффект/метрика |
|---|---|---|---|---|
| P1 | `tile_size` | 256, 512, 1024, 2048; 4096 stress | **design-only:** сейчас выводится из adapter limit | число layers/upload calls, atlas bytes, upload time, feasibility |
| P2 | `halo` | 1, 2, 4; 0 только negative correctness case | **design-only:** сейчас захардкожен 1 | bytes overhead, seam ΔLSB; для radius `r` требуется `halo ≥ r` |
| P3 | synthetic RAW dimensions | 2048×1536, 4096×3072, 6000×4000; 8256×5504 stress | `gpu_smoke` fixed subset now; configurable headless runner future | host/GPU bytes, tile count, upload and frame scaling |
| P4 | physical viewport | 1280×720, 1920×1080, 2560×1440, 3840×2160 | `gpu_smoke` fixed subset now; app surface сейчас vsynced | fragment count, frame p50/p95/p99, dropped deadlines |
| P5 | zoom + pan | `fit`, 1×, 2×, 4×, 8×; center и corner | `gpu_smoke` fixed zooms, pan fixed; configurable runner future | viewport work stays roughly pixel-bound; edge pan exercises background branch |
| P6 | backend/adapter | Metal, Vulkan, DX12, GL, fallback — only detected adapters | `gpu_smoke` records actual adapter; cross-backend matrix future | shader/upload latency, limits, driver/device loss; never compare missing adapter as 0 |
| P7 | cache mode/repetitions | none, warm persistent; exploratory 5, release 30 samples | host-level microbench to add; current tests only | hit/miss, read/write latency, p95, cache bytes; OS page cache remains unknown |
| P8 | worker count | 1, 2, 4, 8, physical cores | **not operative today:** harness stores a label only | DNG tile/scheduler scaling after bounded worker API exists |
| P9 | prefetch depth / upload batch | 0, 1, 2, 4 neighbour tiles; batch 1, 4, 16, 64 | **future residency/scheduler** | first-visible latency vs wasted bytes/queue pressure |
| P10 | quality/CFA stratum | current `bilinear`; future MHC/RCD/AMaZE; RGGB/BGGR/GRBG/GBRG | bilinear + four Bayer patterns for synthetic correctness; future algorithms not present | frame cost and quality tier; quality comparisons require golden corpus |

Non-performance correctness strata should still run: EXIF orientations 0–7,
exposure −10/−2/0/+2/+10, black/white levels and singular camera matrices.
They must not be mixed into a headline speedup because the current shader uses
the same branch structure and only clamps the controls.

## Memory and geometry model

For sensor dimensions `W×H`, tile size `t`, halo `h`, and 16-bit one-plane
samples (`b=2`), the eager atlas upper bound is:

\[
  n_x=\lceil W/t\rceil,\quad n_y=\lceil H/t\rceil,\quad
  B_{atlas}=n_x n_y (t+2h)^2 b.
\]

The ideal full-mosaic payload is `2WH` bytes. Ignoring edge effects, halo
overhead is approximately `(1+2h/t)^2−1`:

| `t` | `h=1` | `h=2` | `h=4` |
|---:|---:|---:|---:|
| 256 | 1.57% | 3.15% | 6.35% |
| 512 | 0.78% | 1.57% | 3.15% |
| 1024 | 0.39% | 0.78% | 1.57% |
| 2048 | 0.20% | 0.39% | 0.78% |

Small tiles lower working-set size and improve viewport residency, but increase
`n_x n_y` layers, `queue.write_texture` calls and texture-array-limit risk.
Large tiles amortize upload calls but waste memory when only a small viewport is
visible. Current 512 MiB eager-atlas cap and adapter `max_texture_array_layers`
must be reported as a feasibility result, never as a timing sample.

For a fixed physical viewport, fragment work is approximately proportional to
`V_w V_h`, not `W H`. Each interior fragment currently performs nine
`textureLoad`s for bilinear demosaic plus matrix/tone-map math. Pan at the image
center is the worst case; corner pan adds background fragments that skip RAW
loads. Record the derived `fit_scale`, because a numeric zoom is not comparable
between different sensor/view dimensions.

## Concrete staged matrix

An all-factorial sweep is intentionally rejected. Even a modest
`4 tile × 3 halo × 3 raster × 4 viewport × 5 zoom × 2 pan × 3 backend` grid is
4320 cells before repetitions, and it invites thermal drift and multiple-
comparison noise. Use screening, then a small interaction matrix.

### S0 — static/host-only checks (runnable without fixtures)

| Case | Input | Repetitions | Output |
|---|---|---:|---|
| S0.1 | WGSL parse + Naga validation | 30 process runs | parse time, pass/fail, shader hash |
| S0.2 | `mosaic_bytes` widths 1, 3, 127, 128, 255, 256, 1024 | 1000 calls | row pitch, allocations, 256-byte alignment |
| S0.3 | halo seam/edge synthetic ramps | every `t∈{256,512,1024}`, `h∈{1,2,4}` | max seam ΔLSB, clamp correctness |
| S0.4 | `WeightedLru` traces with budgets 64/256/512 MiB | 30 traces | hit ratio, evictions, resident bytes |
| S0.5 | telemetry JSONL/trace | 30 runs | event/s, allocation count if instrumented, dropped listener count |

S0 is valid as a unit/microbenchmark suite, not as RAW-open latency. The
workspace currently has only invariant tests for these private helpers; a
dedicated benchmark target is required before the repetition counts above are
actually runnable.

### S1 — synthetic headless GPU screening

Use one deterministic 4096×3072 RGGB 14-bit ramp/noise mosaic, viewport
1920×1080, center pan, `fit` and 2× zoom. Prewarm shader/pipeline twice, then
five screening samples. Run each detected backend in a separate process. Measure CPU encode,
queue submit, GPU timestamp span (if timestamp queries are available), upload
bytes and peak host/GPU allocation.

The current `gpu_smoke` example covers only its fixed dimensions/viewports/zooms
and the renderer's automatic tile (`t`) and `h=1`; the tile/halo rows below
require a future immutable render configuration.

| Sweep | Levels | Cells per backend |
|---|---|---:|
| tile/halo screening | `t={256,512,1024,2048}`, `h={1,2,4}` | 12 |
| viewport/zoom screening | `V={1280×720,1920×1080,3840×2160}`, `z={fit,1,2,4}`, pan={center,corner} | 24 |
| quality/CFA correctness | CFA four Bayer phases, orientation 0 and 4 | 8 |

Run S1 with `n=5` exploratory samples, retain raw rows, and promote only the
two best feasible tile/halo choices per adapter to S2. A failed texture-limit
combination is `skip:limit`, not a zero or infinite speedup.

### S2 — confirmation and interactions

For each adapter, use the promoted tile choices and run `n=30` samples after
two warmups:

1. `tile_size × halo` at 4096×3072 and 1920×1080;
2. `viewport × zoom × pan` at 6000×4000 and promoted tile choice;
3. 2D backend comparison on the identical synthetic seed;
4. cache mode `none` vs `warm persistent` for synthetic cache payloads,
   repetitions 5 (smoke) and 30 (release).

Report p50/p95/p99, MAD/outliers, deterministic bootstrap 95% CI, and raw
samples. Keep adapter/driver/API/cache state in every row.

### S3 — future scheduler/residency matrix

Do not run this as if it existed today. Once `TilePlan`, bounded worker pool,
GPU residency and generation cancellation are implemented:

```text
workers      = [1, 2, 4, 8, physical]
prefetch     = [0, 1, 2, 4]
tile_size    = [256, 512, 1024]
upload_batch = [1, 4, 16, 64]
cache_budget = [64, 256, 512] MiB
scenario     = [visible-first, pan, next/prev, rapid-cancel]
```

Use DNG tiled fixtures for scaling and CR2 fixtures only for serial entropy +
postprocess overlap. `--workers` must become an actual scheduler input before
its curve is accepted.

## Exact command matrix and artifacts

The repository exposes a fixed-matrix `gpu_smoke` example, but not a
parameterized tile/halo/worker sweep binary. The commands below separate the
currently runnable smoke from the reproducible contract for a future runner;
they are not a claim that a complete RAW/UI sweep exists. Every invocation
uses the same deterministic synthetic Bayer mosaic and a fresh process for one
backend.

### Static and host-only commands (all platforms)

```bash
cargo fmt --all -- --check
cargo metadata --locked --format-version 1 >/dev/null
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test -p rrrah-gpu --all-targets --locked
cargo test -p rrrah-bench --all-targets --locked
python3 -m unittest discover -s scripts -p 'test_*.py'
```

These commands validate WGSL/Naga, host layout, halo/padding invariants and
telemetry schema. They do not measure GPU execution.

### Current fixed synthetic smoke

The example currently uses RAW sizes 1024², 2048×1536 and 4096×3072, viewports
1280×720, 2560×1440 and 3840×2160, zooms 0.5/1/2/8, two warmups and twenty
samples. It is useful for adapter/device smoke and a baseline CSV, but it is not
yet a tile/halo/worker/cache sweep. Its CSV keeps `upload_enqueue_ms` (CPU
allocation/queue enqueue) separate from `upload_wait_ms`; `render_*` waits for
GPU completion. The target is per-viewport `Rgba8UnormSrgb`, and numbers are
synthetic GPU evidence—not CR2/DNG decode or UI first-present latency:

```bash
# Actual adapter/backend must be read from the CSV header and stderr.
WGPU_BACKEND=metal \
  cargo run --release -p rrrah-gpu --example gpu_smoke --offline \
  > target/bench/gpu-smoke-metal-$(date +%Y%m%d).csv \
  2> target/bench/gpu-smoke-metal.stderr
```

The 2026-07-21 Apple M4 Max artifact is
`target/bench/gpu-smoke-metal-20260721.csv` (adapter limits are in stderr and
CSV fields). `--offline` is useful for a dependency-cache-only local run; the
current lockfile already includes the dev-only `pollster` edge, so CI may use
`--locked` directly.

Use `WGPU_BACKEND=vulkan` or `WGPU_BACKEND=dx12` on supported hosts. If the
requested backend is unavailable, preserve the stderr and mark the artifact
`skip:adapter-unavailable`; never replace it with a zero-time row.

На audit host smoke был реально запущен на Apple M4 Max / Metal (release,
36 CSV rows: 3 synthetic RAW sizes × 3 viewports × 4 zooms, 2 warmups + 20
samples). Observed ranges: `upload_enqueue_ms=13.39–15.86`,
`render_p95_ms=0.255–2.149`, `render_p99_ms=0.322–2.334`; adapter reported
`max_texture_dimension_2d=16384` and `max_texture_array_layers=2048`.
Это synthetic bilinear smoke с explicit GPU completion, не benchmark открытия
CR2/DNG, не tile/halo scaling и не индустриальный baseline. Исходный CSV и
stderr должны сохраняться как artifacts вместе с manifest.

### Release synthetic headless matrix

The **future** `rrrah-gpu-bench` runner should accept `--backend`, `--adapter`, `--seed`, `--width`,
`--height`, `--tile-size`, `--halo`, `--viewport`, `--zoom`, `--pan`,
`--repetitions` and `--output`. If an adapter/backend is absent, it must exit
with a visible `skip:adapter-unavailable` record and non-zero status only for a
required matrix cell; it must not emit zero timings.

The following blocks are intentionally marked future: `rrrah-gpu-bench` is not
yet a workspace target, so CI must not execute them until that target and its
headless surface are added.

```bash
# Apple Silicon/macOS; wgpu Metal
WGPU_BACKEND=metal \
  cargo run --release -p rrrah-gpu-bench --locked -- \
  --backend metal --seed 0x52524148 --width 4096 --height 3072 \
  --tile-size 256,512,1024,2048 --halo 1,2,4 \
  --viewport 1920x1080 --zoom fit,2 --pan center \
  --warmups 2 --repetitions 30 \
  --output target/bench/metal/rrrah-sweep.jsonl

# Linux; Vulkan (pin a device with the runner's adapter selector)
WGPU_BACKEND=vulkan \
  cargo run --release -p rrrah-gpu-bench --locked -- \
  --backend vulkan --adapter "$RRAH_VULKAN_ADAPTER" \
  --seed 0x52524148 --width 4096 --height 3072 \
  --tile-size 256,512,1024,2048 --halo 1,2,4 \
  --viewport 1920x1080 --zoom fit,2 --pan center \
  --warmups 2 --repetitions 30 \
  --output target/bench/vulkan/rrrah-sweep.jsonl

# Windows; DX12
WGPU_BACKEND=dx12 \
  cargo run --release -p rrrah-gpu-bench --locked -- \
  --backend dx12 --adapter "$RRAH_DX12_ADAPTER" \
  --seed 0x52524148 --width 4096 --height 3072 \
  --tile-size 256,512,1024,2048 --halo 1,2,4 \
  --viewport 1920x1080 --zoom fit,2 --pan center \
  --warmups 2 --repetitions 30 \
  --output target/bench/dx12/rrrah-sweep.jsonl
```

`WGPU_BACKEND` is only a selection hint; the runner must record the actual
`adapter.get_info()` and backend. Do not infer Vulkan/Metal/DX12 from the
environment variable. On macOS, use Metal as the supported production path;
on Linux run Vulkan and optionally GL as a separate compatibility experiment;
on Windows run DX12 and optionally Vulkan. Never average different APIs,
drivers or power modes into one headline number.

For the existing RAW process harness, use honest mode names and explicit
fixture hashes:

```bash
cargo build --release --locked
scripts/bench-harness.py /licensed/path/sample.CR2 \
  --binary target/release/rrrah --repetitions 30 \
  --workers 0 --backend cpu \
  --output target/bench/raw/cr2-cpu.jsonl
python3 scripts/bench-report.py target/bench/raw/cr2-cpu.jsonl \
  --json-out target/bench/raw/cr2-cpu.report.json
```

`--workers 0` means application default, not one measured worker. Do not use
this command to report scheduler scaling until the binary accepts and applies a
worker count.

### Artifact naming

Use deterministic names with no spaces:

```text
target/bench/{date}/{os}-{arch}/{backend}/{adapter_slug}/
  sweep-{seed}-{WxH}-v{viewport}-z{zoom}-t{tile}-h{halo}.jsonl
  sweep-{same-key}.report.json
  sweep-{same-key}.trace.json
  manifest.json
```

The CI upload name should include the run id and backend, for example
`rrrah-sweep-20260721-metal-m1max-${{ github.run_id }}`. `manifest.json` must
contain commit, Cargo/Rust versions, profile, target triple, OS/kernel, GPU
vendor/device/driver, API, timestamp, power mode, seed, tile/halo/viewport
levels, warmups/repetitions, cache state and whether timestamp queries were
available. Compress JSONL only after hashing the uncompressed artifact.

## Measurement contract and risks

| Risk | Why it invalidates a result | Required control |
|---|---|---|
| Harness `workers`/`backend` are labels | changing CLI labels does not change execution | include actual scheduler/backend ID in row; otherwise mark `not_operative` |
| Windowed `AutoVsync` | frame wall time becomes refresh/desktop compositor time | offscreen target or fixed `Immediate` mode; use GPU timestamps |
| Async `queue.write_texture` | host call measures enqueue, not GPU copy completion | separate CPU enqueue and GPU fence/timestamp; never `poll(Wait)` per tile |
| Fixed `tile_size`/`halo` in renderer | nominal sweep values are not applied | expose immutable `RenderConfig` and persist it in manifest |
| Adapter limits/layers/512 MiB cap | some cells cannot allocate | record `skip:limit` with limits, never average failures |
| Synthetic flat/ramp mosaic | unlike CR2/DNG entropy, cache locality and branch mix | call it GPU/geometry benchmark; do not claim open RAW speed |
| Missing camera metadata | DCP/black-level/OpcodeList quality absent | use synthetic only for finite/seam/oracle checks |
| Thermal/clock drift | later cells look slower | randomize cell order, pin power mode, one process per adapter, record temperature if available |
| Pipeline compilation | first frame includes one-time shader cost | report `compile_ms` separately; warm up twice before steady-state |
| High zoom quality path | current shader passes constant `raw_pixel_footprint=1.0` | treat zoom as viewport-coordinate stress only; don't claim adaptive downsample quality |
| Halo under-radius | `halo=0` crosses tile boundary with wrong layer samples | `h=0` is negative correctness test; minimum valid preview halo is 1 |
| No GPU in CI | Naga pass does not prove driver execution | labelled hardware smoke on Metal/Vulkan/DX12; visible skip if unavailable |
| OS page cache | `--no-persistent-cache` does not flush OS pages | name mode honestly; use separate process and state `os_page_cache=unknown` |

## Acceptance and reporting

Every row must contain:

```text
schema, run_id, synthetic_or_fixture, source_hash/seed,
width, height, cfa, tile_size, halo, viewport_px, fit_scale, zoom, pan,
backend, adapter, driver, limits, workers, prefetch, upload_batch,
cache_mode, repetition, warmups, compile_ms, enqueue_ms, gpu_ms,
frame_p50/p95/p99, upload_bytes, host_peak_bytes, gpu_peak_bytes,
status, skip_reason, quality_gate
```

Quality gates for synthetic output are finite/no-NaN, monotonic ramp, Bayer
phase correctness, orientation transform and tile-vs-monolithic seam ≤1 linear
16-bit LSB. Performance gates are hardware-labelled: 60 Hz `p95≤16.7 ms` and
120 Hz `p95≤8.3 ms` only for steady render, not upload/open. No number from S0/S1
is an industry comparison until the same quality tier and a licensed CR2/DNG
corpus are used.

## Critic verdict

* The highest-value immediate sweep is synthetic GPU geometry (tile/halo,
  viewport/zoom, backend), not fake CR2 decode. It isolates the part that can be
  measured without proprietary files.
* `workers`, prefetch, cache budget and upload batch are architecture parameters,
  not current runtime knobs. Publishing curves for them now would be a false
  benchmark.
* Keep tile/halo screening one-factor-at-a-time plus selected interactions; a
  full factorial would spend more time on thermal and adapter noise than on
  useful evidence.
* Treat adapter-limit failures and missing fixtures as explicit status values.
  Zero-filling them makes averages look faster and hides the exact GPU texture
  limit that previously caused the blank viewer.
* A final claim of RAW-open speed still requires C1–C7 from
  [`BENCHMARK_MATRIX.md`](BENCHMARK_MATRIX.md), especially CR2 serial entropy,
  DNG tile scaling, cache state and first-visible timing.
