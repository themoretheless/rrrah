# Production RAW editor: 100-function map

Расширение pro-функций F101–F200 находится в
[EDITOR_101_200.md](EDITOR_101_200.md).

Это не обещание, что все функции уже реализованы. Это production backlog с
контрактом: каждая функция должна иметь deterministic output, memory budget,
CPU/GPU path и benchmark `Fxx`. Приоритеты: `P0` — фундамент, `P1` — pro
workflow, `P2` — расширенные возможности.

### Dependency и memory convention

Чтобы таблицы оставались читаемыми, зависимости задаются группами:
`F01–F10=I` ingest, `F11–F20=T` tiles/scheduler, `F21–F30=C` cache,
`F31–F40=D` develop, `F41–F50=K` color, `F51–F60=G` geometry,
`F61–F70=M` masks, `F71–F80=X` export, `F81–F90=N` metadata,
`F91–F100=U/B` UX/benchmark. Базовые зависимости: `T→I`, `C→I/T`,
`D→I/T`, `K→D`, `G→I/K`, `M→D/K/G`, `X→K/G/M`, `N→I/C`, `U→T/C/N`.

Базовые memory budgets считаются формулами, а не фиксированными мегабайтами:

```text
raw_mosaic      = width × height × bytes_per_sample
raw_tile        = (T + 2 × halo)² × bytes_per_sample
working_rgb     = width × height × 6 bytes       // FP16 RGB storage
mask_tile       = (T + 2 × mask_radius)² × 2 bytes
gpu_tiles       = floor(gpu_budget / raw_tile)
```

Каждый PR обязан указать, к какой группе относится функция, какие зависимости
разблокированы, и какой из этих budgets он увеличивает.

## F01–F10 — ingest и безопасность

| ID | Функция | Приоритет | Основной путь | Критерий |
|---|---|---:|---|---|
| F01 | File probe | P0 | CPU | metadata p95 |
| F02 | CR2 lossless-JPEG decode | P0 | CPU/SIMD | MP/s, p95 |
| F03 | DNG strip/tile decode | P0 | bounded CPU pool | scaling 1/2/4/8 |
| F04 | Float/LinearRaw decode | P1 | CPU | MP/s, RSS |
| F05 | CFA/black/white validation | P0 | CPU | reject malformed input |
| F06 | Metadata-only fast open | P0 | CPU | <20 ms p95 |
| F07 | Restart-marker partitioning | P1 | CPU | speedup vs serial |
| F08 | Predictor row-band postprocess | P1 | SIMD workers | GB/s |
| F09 | Malformed-file sandbox/limits | P0 | worker process | no OOM/crash |
| F10 | RawSpeed/LibRaw fallback | P1 | adapter | corpus pass rate |

## F11–F20 — tiles, preload и scheduling

| ID | Функция | Приоритет | Основной путь | Критерий |
|---|---|---:|---|---|
| F11 | Tile planner с halo | P0 | CPU | seam ΔE |
| F12 | Priority scheduler P0–P3 | P0 | CPU | visible tile p95 |
| F13 | Generation cancellation | P0 | atomics | stale publish = 0 |
| F14 | DNG tile worker pool | P0 | CPU | scaling efficiency |
| F15 | CR2 sequential entropy lane | P0 | CPU | serial fraction |
| F16 | Tile postprocess fan-out | P0 | SIMD/CPU | GB/s |
| F17 | Viewport residency prediction | P1 | CPU | tile hit rate |
| F18 | CFA-safe mip pyramid | P0 | GPU/CPU | build MP/s |
| F19 | GPU atlas/array packing | P0 | GPU | upload MB/s, VRAM |
| F20 | Seam/halo validator | P0 | CPU/GPU | max boundary error |

## F21–F30 — caches

| ID | Функция | Приоритет | Основной путь | Критерий |
|---|---|---:|---|---|
| F21 | Weighted RAM LRU | P0 | CPU | hit rate/eviction µs |
| F22 | TinyLFU/2Q admission | P1 | CPU | scan resistance |
| F23 | Persistent decoded mosaic | P0 | disk | warm-open p95 |
| F24 | Compressed tile cache | P1 | disk/CPU | ratio, decode MB/s |
| F25 | GPU LRU residency | P0 | GPU | hit rate/upload ms |
| F26 | ABI/semantic cache key | P0 | CPU | stale-hit = 0 |
| F27 | Full BLAKE3 fingerprint | P1 | SIMD CPU | hash GB/s |
| F28 | Crash-safe atomic commit | P0 | disk | corruption = 0 |
| F29 | RAM/VRAM pressure controller | P1 | CPU/GPU | budget adherence |
| F30 | Cache trace/observability | P1 | CPU | overhead <1% |

## F31–F40 — RAW develop и detail

| ID | Функция | Приоритет | Основной путь | Критерий |
|---|---|---:|---|---|
| F31 | Bilinear first-frame demosaic | P0 | GPU | first-frame ms |
| F32 | MHC 5×5 | P1 | GPU | ΔE00/reference |
| F33 | RCD/AMaZE | P2 | GPU/CPU | SSIM, MP/s |
| F34 | CFA-aware downsample | P0 | GPU/CPU | aliasing score |
| F35 | Black-level grids/deltas | P0 | GPU LUT | ΔE, shader µs |
| F36 | Linearization table | P1 | GPU LUT | highlight ΔE |
| F37 | Highlight reconstruction | P1 | GPU | clipped-area/ΔE |
| F38 | Sensor-noise denoise | P2 | GPU | SNR gain, ms/MP |
| F39 | Lens shading/vignetting | P1 | GPU | ΔE, uniformity |
| F40 | Sharpen/local contrast | P1 | GPU | acutance, ms/MP |

## F41–F50 — color management

| ID | Функция | Приоритет | Основной путь | Критерий |
|---|---|---:|---|---|
| F41 | Temperature/tint WB solver | P0 | CPU/GPU | ΔE00 |
| F42 | Camera/Forward Matrix | P0 | GPU | matrix ΔE |
| F43 | Bradford D50 adaptation | P1 | GPU | ΔE00 |
| F44 | ICC input profile | P1 | LUT | ΔE/LUT build |
| F45 | DCP dual-illuminant interpolation | P1 | CPU | CCT ΔE |
| F46 | Filmic/ACES/tone curve | P0 | GPU | highlight ΔE |
| F47 | HDR PQ/HLG | P2 | FP16 GPU | nits error/frame ms |
| F48 | Gamut mapping | P1 | GPU | out-of-gamut/ΔE |
| F49 | Soft proofing | P2 | GPU LUT | ΔE00 |
| F50 | Color-managed export transform | P0 | CPU/GPU | ΔE, MP/s |

## F51–F60 — geometry и lens

| ID | Функция | Приоритет | Основной путь | Критерий |
|---|---|---:|---|---|
| F51 | EXIF orientation | P0 | GPU | exact pixels |
| F52 | Crop/active area | P0 | CPU/GPU | crop error px |
| F53 | Zoom-to-cursor | P0 | CPU | anchor drift px |
| F54 | Fit/1:1 navigation | P0 | CPU/GPU | fit latency |
| F55 | Inertial pan/bounds | P1 | CPU | frame p95 |
| F56 | Lens distortion | P1 | GPU mesh | reprojection px |
| F57 | Chromatic aberration | P1 | GPU | edge ΔE |
| F58 | Perspective/keystone | P2 | GPU matrix | reprojection px |
| F59 | Arbitrary rotate/flip | P1 | GPU | frame ms |
| F60 | Variant optical-flow alignment | P2 | GPU pyramid | endpoint error |

## F61–F70 — masks и local adjustments

| ID | Функция | Приоритет | Основной путь | Критерий |
|---|---|---:|---|---|
| F61 | Parametric exposure mask | P0 | GPU | mask eval ms |
| F62 | Brush mask sparse tiles | P0 | CPU/GPU | stroke latency |
| F63 | Feather/blur mask | P1 | GPU | ms/MP |
| F64 | Linear/radial gradient | P0 | GPU | ns/pixel |
| F65 | Luminance range mask | P1 | GPU histogram | selectivity/MP/s |
| F66 | Color range mask | P1 | GPU LUT | selection ΔE |
| F67 | Mask boolean algebra | P0 | GPU | throughput |
| F68 | Mask invalidation DAG | P0 | CPU | recompute ratio |
| F69 | Local WB/HSL curves | P1 | GPU LUT | ΔE/frame p95 |
| F70 | Versioned edit history | P0 | CPU | undo latency/bytes |

## F71–F80 — export

| ID | Функция | Приоритет | Основной путь | Критерий |
|---|---|---:|---|---|
| F71 | JPEG 8-bit | P0 | CPU/GPU | MP/s, SSIM |
| F72 | JPEG XL/AVIF | P1 | CPU | ratio, MP/s |
| F73 | 16-bit TIFF | P0 | CPU/GPU | MP/s, RSS |
| F74 | Linear DNG round-trip | P2 | CPU | ΔE round-trip |
| F75 | PNG | P1 | CPU | MP/s |
| F76 | Export crop/resize/sharpen | P0 | GPU/CPU | edge score |
| F77 | ICC/EXIF/XMP preservation | P0 | CPU | tag retention |
| F78 | Bounded batch export | P1 | CPU pool | jobs/min, p95 |
| F79 | Atomic export/resume | P1 | disk | corruption = 0 |
| F80 | Print proof/export intent | P2 | CPU/GPU | ΔE00 |

## F81–F90 — metadata и catalog

| ID | Функция | Приоритет | Основной путь | Критерий |
|---|---|---:|---|---|
| F81 | EXIF/TIFF/IFD parser | P0 | CPU | parse ms/bounded alloc |
| F82 | Canon/Nikon/Sony MakerNotes | P1 | CPU | fields parsed |
| F83 | XMP sidecar read/write | P0 | CPU | round-trip tags |
| F84 | Non-destructive edit schema | P0 | CPU | migration pass |
| F85 | Stars/labels/flags | P0 | CPU | update latency |
| F86 | SQLite catalog/index | P1 | CPU | query p95 |
| F87 | GPU/CPU perceptual thumbnails | P0 | GPU/CPU | grid first paint |
| F88 | Duplicate/similarity search | P2 | CPU/GPU | recall/scans/s |
| F89 | Geo/time search | P2 | CPU | query p95 |
| F90 | Corruption/quarantine report | P1 | CPU | scan MB/s |

## F91–F100 — UX и observability

| ID | Функция | Приоритет | Основной путь | Критерий |
|---|---|---:|---|---|
| F91 | Non-blocking open/cancel | P0 | scheduler | cancel latency |
| F92 | Keyboard/gesture navigation | P0 | CPU | event→frame |
| F93 | Histogram/waveform/vectorscope | P1 | GPU compute | update ms |
| F94 | Zebra/clipping/focus peaking | P1 | GPU | overlay ms |
| F95 | Before/after comparison | P0 | GPU | frame p95 |
| F96 | Virtualized filmstrip/grid | P0 | CPU/GPU | scroll FPS |
| F97 | Presets/settings profiles | P1 | CPU | apply ms |
| F98 | Crash recovery/autosave journal | P0 | CPU/disk | recovery seconds |
| F99 | Accessibility/color-blind UI | P1 | CPU | accessibility tests |
| F100 | Benchmark HUD/trace export | P0 | CPU/GPU | overhead <2% |

## Общий математический контракт

Порядок операций:

```text
entropy → linearization → black/bad-pixel → demosaic → WB
→ camera matrix → working RGB/Bradford → lens/vignetting
→ denoise/detail/masks → exposure/tone → gamut/ICC/transfer → export
```

Нормализация сенсора:

\[
s=\max\left(\frac{r-b(x,y)}{\max(w(x,y)-b(x,y),1)},0\right)
\]

До tone mapping значения выше 1 не clamp-ятся, чтобы сохранять highlight
headroom. Exposure в scene-linear:

\[
RGB' = 2^{e}\,RGB
\]

Для каждого алгоритма явно задаётся halo: bilinear `1`, MHC `2`, RCD `2..3`,
AMaZE `4..6`. Tile пишет только interior. CFA, crop и black-level grid всегда
вычисляются в глобальных sensor coordinates.

Color matrices строятся в `f64`, runtime выполняется в `f32`; `f16` разрешён для
storage, но не для критичных matrix/demosaic вычислений. Проверяются finite,
determinant и condition number. Для reference kernels используется `f64` CPU
oracle.

Критические invariants:

```text
black → 0
one-stop → 2× scene-linear radiance
M × inverse(M) error < 1e-5 (f32)
neutral gray ΔE00 < 0.5
tiled vs monolithic max error ≤ 1 LSB
no NaN/Inf
```

## Приоритет реализации

Сначала вертикальный P0-срез: F01–F03, F11–F16, F21/F23/F25, F31, F41/F42,
F51–F54, F61/F64/F67/F68, F71/F73/F77, F81/F83/F84, F91/F95/F96/F98/F100.
P1/P2 функции нельзя добавлять до появления reference image tests, tile seam
tests, quality corpus и regression gates из [BENCHMARKS.md](BENCHMARKS.md).
