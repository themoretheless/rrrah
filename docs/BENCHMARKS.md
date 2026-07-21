# Benchmark protocol

## Process-boundary harness

Для воспроизводимого запуска используйте `scripts/bench-harness.py`, а не
только `/usr/bin/time` вокруг одной команды:

```bash
scripts/bench-harness.py /path/to/a.CR2 /path/to/b.DNG \
  --repetitions 7 --workers 8 --backend wgpu-metal \
  --output target/bench/runs.jsonl
scripts/bench-report.py target/bench/runs.jsonl \
  --warmup 1 --json-out target/bench/report.json
```

Каждая строка JSONL содержит immutable sample и manifest запуска:

* SHA-256 RAW и исполняемого файла, размер и абсолютный путь fixture;
* commit, Rust toolchain, `RUSTFLAGS`, OS/kernel/arch и число CPU;
* mode (`no-persistent-cache` или `warm-persistent-cache`), iteration,
  process exit status и monotonic wall time;
* разобранные из `--inspect` dimensions/bit depth/pixel count, cache hit,
  reported decode/total timings и флаг embedded-JPEG invariant;
* stderr tail/error для неуспешных запусков.

`no-persistent-cache` не означает cold OS page cache: непривилегированный
процесс не может надёжно вытеснить страницы на macOS/Linux. Поэтому harness
помечает `os_page_cache_state` как `unknown (not flushed)`. Для настоящего cold
прогона нужен отдельный privileged protocol (reboot/`purge`/`drop_caches`) и
его результат должен быть отдельной серией, никогда не смешанной с warm.

Отчёт считает p50/p95/p99, MAD/outliers и детерминированный 95% bootstrap CI;
outliers только помечаются и не выбрасываются без `--exclude-outliers`.
Регрессионный gate можно запускать через `--baseline-json` и фиксированный
`--regression-threshold`; baseline должен быть получен на той же машине,
fixture и cache-state.

`--workers` и `--backend` являются частью ключа группы, даже если конкретный
исполняемый файл пока использует фиксированный worker pool. Это запрещает
смешать в одном p95 результаты CPU-only, Metal и Vulkan или разные точки
масштабирования. Когда scheduler начнёт читать `RRAH_WORKERS`, тот же harness
станет прямым scaling benchmark без изменения формата результатов.

Измерения должны выполняться на одной машине, с одинаковым файлом и без
embedded-JPEG shortcut. Команда:

```bash
scripts/bench-raw.sh /path/to/file.CR2
```

Она измеряет три разных сценария:

1. cold full RAW decode без кеша;
2. decode плюс запись persistent mosaic cache;
3. warm persistent-cache open.

## Baseline: Canon EOS 5DS CR2

Файл: `8896×5920`, 16-bit, 52,664,320 samples, 64 MiB compressed CR2.
Измерение выполнено на текущей рабочей машине 2026-07-21, release build.

| Сценарий | Pipeline time | wall time |
|---|---:|---:|
| cold decode, no cache | 396.6 ms | 0.40 s |
| decode + cache write | 402.5 ms | 0.51 s |
| warm persistent cache | 97.2 ms | 0.10 s |

Это не benchmark GPU render: headless environment не предоставляет swapchain.
Для production benchmark нужно отдельно измерять `upload_mosaic`, first frame,
steady-state frame time и p95 при zoom/pan.

## Сравнение с индустриальными ориентирами

В окружении benchmark не установлены darktable, RawTherapee, dcraw или LibRaw
CLI, поэтому для них нельзя честно привести локальные миллисекунды. Их нужно
собирать тем же harness и тем же RAW-файлом.

- **RawSpeed** — ориентир именно entropy-decoder: проект ставит целью скорость,
  близкую к оптимальной для loader-а, но не является готовым display pipeline.
- **LibRaw** — широкий C/C++ decoder API для CR2/DNG и множества камер; время
  полного RGB output включает дополнительные этапы, которых нет в нашем cold
  decode.
- **RawTherapee/darktable** — сравнивать следует отдельно по first preview и
  final-quality render: их advanced demosaic, lens/noise/highlight processing
  сознательно дороже bilinear first-frame path.
- **RapidRAW** — ближайший GPU-аналог (Rust + wgpu + WGSL), но это редактор с
  большим non-destructive processing pipeline, поэтому его first-open и
  steady-state нужно измерять отдельно от editor effects.

Главная измеряемая характеристика rrrah — не максимальное качество финального
экспорта, а latency до настоящего RAW frame:

```text
T_open = T_probe + T_entropy_decode + T_cache_write + T_gpu_upload + T_first_frame
```

Для warm open:

```text
T_warm = T_cache_read + T_gpu_upload + T_first_frame
```

Целевые acceptance thresholds для viewer-а:

- warm open p95 ≤ 150 ms;
- cold CR2 decode p95 ≤ 500 ms для 50–70 MiB файла;
- first interactive frame ≤ 100 ms после готовности mosaic;
- steady-state pan/zoom: p95 frame time ≤ 16.7 ms (60 Hz).

Пороговые значения — инженерные цели, а не утверждение о характеристиках
сторонних проектов.

## Scaling benchmark для tiled backend

После появления `TileScheduler` каждый RAW должен прогоняться при
`workers = 1, 2, 4, 8` (и при числе физических ядер), отдельно для CR2 и DNG:

```text
probe_ns
entropy_decode_ns
tile_postprocess_ns
cache_wait_ns
gpu_upload_ns
first_visible_tile_ns
all_visible_tiles_ns
stale_task_count
peak_cpu_bytes / peak_gpu_bytes
```

Для CR2 ожидается ограниченный speedup: Huffman bitstream и predictor имеют
последовательную зависимость по строкам. Для DNG independent tiles могут
масштабироваться почти линейно до насыщения SSD, memory bandwidth или GPU
upload queue. Это нужно показывать двумя кривыми, а не одним средним числом.

Оценка верхней границы берётся из Amdahl:

```text
speedup(P) = 1 / (serial_fraction + parallel_fraction / P)
```

Например, при 85% последовательного CR2 entropy path восемь CPU workers дают
теоретически не более `1 / (0.85 + 0.15/8) ≈ 1.16×`; дополнительные threads
полезнее направить на соседние tiles, postprocess или следующий кадр.

## Production benchmark matrix (100 функций)

Каждая функция редактора должна иметь хотя бы один machine-readable benchmark.
Функции распределяются по пяти классам; номер функции используется в JSON
результатах и в CI-regression gate.

```text
F01-F20  ingest/probe/decode       (CR2, DNG, TIFF/LinearRaw, metadata)
F21-F40  cache/prefetch/scheduler  (RAM, disk, cancellation, tiles/mips)
F41-F60  demosaic/color            (bilinear, MHC, RCD, WB, matrices, tone)
F61-F80  GPU/display               (upload, residency, pan/zoom, effects)
F81-F100 export/editor             (16-bit TIFF/PNG, sidecar, undo, batch)
```

Canonical case names (the implementation may expose them as subcommands or
criterion benches, but IDs must remain stable):

```text
F01 probe_header  F02 probe_ifd  F03 probe_cr2  F04 probe_dng  F05 metadata_validate
F06 ljpeg_huffman F07 ljpeg_predictor F08 cr2_full_decode F09 dng_strip_decode
F10 dng_tile_decode F11 dng_float_decode F12 opcode_parse F13 opcode_apply
F14 black_grid F15 white_level F16 cfa_phase F17 crop_orientation F18 wb_extract
F19 matrix_extract F20 malformed_input

F21 ram_lru F22 ram_tinylfu F23 disk_cache_read F24 disk_cache_write F25 cache_checksum
F26 cache_fingerprint F27 cache_restart F28 cache_corruption F29 cache_eviction F30 cache_budget
F31 prefetch_forward F32 prefetch_backward F33 prefetch_cancel F34 priority_queue
F35 generation_drop F36 dng_tile_pool F37 cr2_row_pool F38 mip_build F39 gpu_residency
F40 staging_ring

F41 bilinear F42 edge_aware F43 mhc F44 rcd F45 amaze F46 xtrans F47 four_color
F48 black_normalize F49 wb_apply F50 camera_matrix F51 xyz_transform F52 gamut_map
F53 exposure F54 contrast F55 tone_curve F56 aces F57 srgb_encode F58 highlight_reconstruct
F59 noise_model F60 lens_shading

F61 texture_upload F62 padded_rows F63 atlas_build F64 tile_upload F65 tile_eviction
F66 mip_resolve F67 shader_compile F68 pipeline_cache F69 bind_update F70 first_present
F71 steady_60hz F72 steady_120hz F73 pan F74 zoom F75 zoom_cursor F76 rotate
F77 flip F78 viewport_resize F79 hdr_surface F80 device_lost_recovery

F81 export_tiff16 F82 export_tiff8 F83 export_png16 F84 export_jpeg F85 sidecar_read
F86 sidecar_write F87 nondestructive_stack F88 undo F89 redo F90 history_compact
F91 batch_export F92 batch_decode F93 batch_cache F94 color_profile_load F95 icc_transform
F96 metadata_write F97 atomic_output F98 cancel_export F99 crash_recovery F100 session_restore
```

The first pass may mark unsupported cases as `skip` with an explicit reason; a
missing implementation must never be reported as a fast result.

Для каждой функции harness пишет одну строку JSONL:

```json
{
  "function":"F43_mhc_demosaic",
  "backend":"wgpu-metal",
  "fixture":"canon_5ds.cr2",
  "mode":"cold|warm|steady|export",
  "workers":8,
  "repeat":30,
  "p50_ms":12.4,
  "p95_ms":13.8,
  "p99_ms":15.1,
  "throughput":"81.2 Mpix/s",
  "peak_rss_mib":742,
  "gpu_mem_mib":318,
  "cache_hit_rate":0.94,
  "dropped_frames":0,
  "quality":{"psnr_db":48.2,"ssim":0.998,"delta_e2000":0.42},
  "git":"<commit>","cpu":"<model>","gpu":"<model>"
}
```

### Corpus and reproducibility

The corpus must include at least 12 files, not just one CR2:

* 3 lossless-JPEG CR2 (small/medium/large; one with restart markers, one without);
* 3 DNG (uncompressed strips, tiled lossless-JPEG, floating-point/LinearRaw);
* 2 cameras with non-Bayer CFA (Fuji X-Trans and 4-color if licensing permits);
* 2 files containing black-level grids, DNG OpcodeList and nontrivial orientation;
* 2 stress files (120+ Mpix and truncated/malformed copies for security tests).

Fixtures are identified by full BLAKE3 and stored outside git. The benchmark
records camera model, byte size, dimensions and `raw_frame_index`; embedded JPEG
is prohibited. The runner reports whether the operating-system page cache was
flushed. A true cold run requires a reboot or privileged cache flush; otherwise
label the run `os-warm` instead of claiming cold.

Hardware and software controls are part of the result: CPU model, physical and
logical core count, RAM, GPU/driver/API, OS, Rust/LLVM version, compiler flags,
power mode, display resolution and color profile. Pin CPU workers to physical
cores for scaling tests and run single-process, single-GPU jobs. Never compare a
debug build with a release build.

### Timing definitions

Use a monotonic clock and emit spans through `tracing`/Chrome trace format:

```text
T_open      = T_probe + T_decode + T_cache_read/write + T_upload + T_first_present
T_first     = T_probe + T_visible_tiles + T_upload_visible + T_first_present
T_steady    = p95(frame_present - frame_request) over 300 frames
T_export    = decode + process + encode + fsync
```

`T_first` is the primary editor metric; a preview generated from the embedded
JPEG is an invalid result. Report p50/p95/p99 and min/max, not only arithmetic
mean. Discard the first measurement only when explicitly labelled warm-up; keep
allocator and shader-compilation times in a separate `cold-process` series.

### Statistical protocol (v2)

Use `scripts/bench-report.py` as the canonical aggregator. It accepts the CSV
written by `bench-matrix.sh` and JSONL emitted by the future live telemetry
runner:

```bash
scripts/bench-report.py target/bench/results.csv \
  --warmup 2 --bootstrap 10000 \
  --json-out target/bench/report.json
```

Keep every raw sample in the report. For latency/open suites use at least 30
samples after two warm-ups; for expensive final exports, 10 samples after one
warm-up is the minimum. Cold, OS-warm, process-warm and cache-warm are separate
groups and must never be averaged together. A run whose page cache was not
flushed is labelled `os-warm`, never `cold`.

Percentiles use linear interpolation of the ordered sample:

```text
q(p) = (1-a) x[floor((n-1)p)] + a x[ceil((n-1)p)]
a = (n-1)p - floor((n-1)p)
```

Headline latency is p50/p95/p99. A deterministic percentile-bootstrap gives the
95% confidence interval (`B=10000`, fixed seed):

```text
x*_k ~ sample_with_replacement(x, n)
CI95(stat) = [quantile(stat(x*_k), .025), quantile(stat(x*_k), .975)]
```

The report also includes mean, standard deviation, trimmed mean, MAD, min/max
and the complete `samples_ms` array. MAD outlier detection is diagnostic, not a
license to hide stalls:

```text
MAD = median(|x_i - median(x)|)
z_i = 0.67448975 * (x_i - median(x)) / MAD
outlier iff |z_i| > 3.5
```

Outliers remain in headline metrics by default and are counted in
`outlier_count`; `--exclude-outliers` adds filtered metrics while retaining the
unfiltered values in `all_samples`.

### Effect sizes and regression gates

Compare reports using the stable key `fixture/mode/workers/backend`:

```text
relative_change = (new - old) / old
speedup         = old_p50 / new_p50
```

For latency, `relative_change > 0.05` on p50 or p95 is a regression and exits
CI with status 1; `< -0.05` is an improvement. Throughput uses the opposite
sign (`relative_change < -0.05` is a regression). A 5% threshold avoids making
normal timer noise a failure. Reviewers should also inspect absolute milliseconds
and both 95% CIs. If the CI for a ratio crosses 1.0 (or the difference crosses
zero), mark the comparison `inconclusive` unless the change exceeds the gate.

The machine-readable schema is `rrrah.benchmark-report.v2`:

```json
{
  "schema": "rrrah.benchmark-report.v2",
  "statistics": {"warmup": 2, "bootstrap_samples": 10000,
    "confidence": 0.95, "outlier_rule": "modified_z_score > 3.5 using MAD"},
  "groups": [{"fixture": "canon_5ds.cr2", "mode": "warm-cache",
    "workers": 8, "backend": "wgpu-metal", "n": 30,
    "p50_ms": 91.2, "p50_ci95_ms": [89.7, 93.1],
    "p95_ms": 106.4, "p95_ci95_ms": [101.0, 113.8],
    "outlier_count": 1, "samples_ms": [90.1, 91.2]}],
  "regressions": []
}
```

Reports without sample count, raw samples, commit and hardware fingerprint are
incomplete and must not be used for speed claims.

Каждый baseline comparison также содержит `absolute_change_ms`, `ratio`,
`ratio_ci95` и статус `regression|improvement|inconclusive|ok`. Ratio CI
строится консервативно как `new_ci_low/old_ci_high` …
`new_ci_high/old_ci_low`; это шире paired-bootstrap и не создаёт ложной
уверенности, когда process-boundary samples независимы.

For throughput use both pixels and bytes:

```text
decode_Mpix_s = width * height * frames / decode_seconds / 1e6
io_GB_s       = compressed_bytes / decode_seconds / 1e9
```

For interactive quality, report frame deadline misses:

```text
miss_rate = frames_over_16.67ms / rendered_frames
```

Acceptance gates for a high-end desktop are `T_first p95 <= 250 ms` from an
OS-warm source, `warm-cache p95 <= 150 ms`, and steady-state `p95 <= 16.7 ms`
with miss-rate below 1%. The gate is hardware-labelled, not universal.

### Parallel scaling and scheduler efficiency

Run workers `1,2,4,8,16` and physical-core count. Capture wall time, CPU time,
speedup, parallel efficiency and queue behaviour:

```text
speedup(P)     = T(1) / T(P)
efficiency(P)  = speedup(P) / P
utilisation    = busy_worker_time / (P * wall_time)
tail_ratio     = p95_tile_time / p50_tile_time
```

For tiled DNG also vary tile size (256, 512, 1024, 2048) and queue depth. Stop
increasing workers at the first point where throughput improves <5% or RSS/
tail latency grows >10%. CR2 entropy decode is measured as a sequential lane;
parallelism is expected in row-band postprocess, hashing, prefetch and GPU upload.

### Memory/cache benchmark

Measure peak RSS, resident decoded bytes, mapped bytes, GPU allocated bytes,
staging bytes and cache metadata separately. The expected lower bound for a full
single-plane 16-bit mosaic is:

```text
mosaic_bytes = width * height * 2
```

For a tiled image with one-pixel demosaic halo:

```text
tile_bytes = (tile_w + 2h) * (tile_h + 2h) * 2
atlas_bytes = resident_tiles * tile_bytes
```

Cache tests use sequential scan, random browse, forward/backward browse and
reopen-after-restart. Report hit rate, bytes read, eviction count, duplicate
work and stale-task count. A prefetcher passes only if it improves `T_first` or
next-image latency without increasing peak RSS beyond the configured budget.

### GPU benchmark

Use timestamp queries around upload, demosaic, color transform and copy-to-
swapchain. Record texture-array residency, staging-ring occupancy, bytes/frame,
pipeline/shader compilation and device-lost events. Benchmark viewport sizes
1080p, 4K and 6K, zoom levels 0.25x/1x/4x/16x and effects enabled/disabled.
Compare full-atlas, resident-tile and mip fallback paths. A tile seam test must
render the same crop once as a monolithic CPU reference and once split at every
tile boundary; the max absolute channel error must be below 1 LSB in linear
16-bit space.

### Roofline and bottleneck classification

Every hot stage is classified as compute- or bandwidth-bound. Estimate arithmetic
intensity and compare it with measured memory bandwidth and peak FLOP rate:

```text
AI = floating_point_ops / bytes_transferred
roofline_perf = min(peak_FLOP_s, AI * sustainable_bandwidth_B_s)
efficiency = measured_perf / roofline_perf
```

For RAW decode, count compressed input bytes and decoded mosaic writes; for
demosaic count texture reads, interpolations and RGB writes; for color count
matrix multiply/FMA operations. If a kernel is below 60% of the appropriate
roofline, optimize memory layout/coalescing before adding threads. Record CPU
cache-miss, branch-miss and SIMD-width counters when available (`perf`, `xctrace`,
or vendor profiler); GPU counters include occupancy, cache hit rate, DRAM
throughput and wave/warp divergence. This prevents a misleading benchmark where
20 workers merely contend for the same memory bus.

### Image-quality benchmark

Performance is invalid if the image silently degrades. Generate a reference
render with a trusted high-quality path (darktable/RawTherapee, fixed profile)
and compare the same crop and exposure:

```text
MSE  = (1/N) * Σ (I - R)^2
PSNR = 10 * log10(MAX^2 / MSE)
SSIM = standard luminance/contrast/structure metric
ΔE00 = CIEDE2000 after converting to CIE Lab/D50
```

Report PSNR/SSIM/ΔE00 for flat fields, edges, saturated highlights, shadows and
high-frequency foliage. Also report clipping rate and false-color pixel rate.
The fast preview path may use bilinear demosaic, but the final-quality path must
declare its algorithm (MHC/RCD/AMaZE), profile, matrix and tone curve so results
are comparable.

### Open-source comparison protocol

Run the same corpus and measurements for rrrah, RawSpeed (decode-only), LibRaw
(unprocessed mosaic and processed RGB), darktable, RawTherapee and RapidRAW.
Do not compare unlike stages: publish separate columns for probe, full RAW
decode, first real RAW frame, final-quality render and export. Capture command,
version and configuration in the JSONL record. External tools that cannot expose
GPU timestamps are measured with wall-clock and clearly marked.

### Harness layout

```text
bench/
  fixtures.toml          # paths, BLAKE3, expected metadata
  cases/*.toml           # F01..F100 parameters and acceptance gates
  runner.rs              # invokes rrrah bench subcommands, JSONL output
  adapters/{libraw,darktable,rawspeed,rawtherapee,rapidraw}.rs
  stats.rs               # quantiles, bootstrap CI, regression test
  quality.rs             # PSNR/SSIM/ΔE00 and crop comparisons
scripts/
  bench-matrix.sh        # build, pin CPU, run cases, collect traces
  bench-compare.sh       # external adapters, normalized report
  bench-report.py        # plots speedup, RSS, p95, quality frontier
```

CI runs a small deterministic corpus on every change and a full corpus nightly.
The regression gate fails when p95 latency regresses >10%, peak RSS >10%, cache
hit rate drops >5 percentage points, or quality metrics cross the per-algorithm
threshold. Raw benchmark artifacts (JSONL, traces, GPU counters and plots) are
kept with the commit so optimization claims remain reproducible.

The current process-boundary smoke harness is available immediately:

```bash
BENCH_REPS=9 scripts/bench-matrix.sh sample.cr2 sample.dng
```

It seeds a persistent cache, measures cold/no-cache and warm-cache opens, and
writes `target/bench/results.csv`. This is deliberately a baseline harness;
stage spans and GPU timestamps are added by the Rust runner described above.
