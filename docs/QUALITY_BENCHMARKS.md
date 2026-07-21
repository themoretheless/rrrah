# Фотографические quality benchmarks

Этот документ отделяет корректность RAW pipeline от его скорости. Быстрый
bilinear preview нельзя сравнивать с финальной демозаикой AMaZE/RCD, а
JPEG-thumbnail нельзя считать первым RAW-кадром.

## 1. Corpus и эталоны

### 1.1 Fixture-план

Corpus хранится вне git, каждый файл идентифицируется полным BLAKE3. Для каждого
fixture фиксируются: camera make/model, ISO, exposure, CFA, bit depth, active
area, black/white levels, orientation, DNG opcode tags, размер и frame index.

Минимальный набор:

| Группа | Состав | Назначение |
|---|---|---|
| D1 | 3 Bayer CR2/NEF (12/14/16 bit, малый/50 MP/100 MP) | entropy и scaling |
| D2 | DNG strip, tiled lossless-JPEG, uncompressed | независимые tile workers |
| D3 | X-Trans и 4-color CFA | generic CFA/phase |
| D4 | black-level grid, nontrivial crop/orientation, OpcodeList | metadata semantics |
| D5 | ISO 12233 chart, ColorChecker SG, Siemens star, dead-leaves, fabric, starfield | detail, color, moiré, noise |
| D6 | synthetic RAW generated from linear RGB + camera model | bit-exact ground truth |
| D7 | truncated, malformed, huge-dimension, NaN/Inf DNG | security и bounded memory |

Реальные фотографии нужны для camera-specific behaviour, но только D6 даёт
строго известную ground truth: формируем sensor mosaic из спектральных каналов,
применяем black/white/quantization и известную CFA. Это позволяет тестировать
decode/demosaic без субъективной оценки и без скрытого JPEG.

### 1.2 Reference pipeline

Для каждого D6 fixture сохраняются:

```text
raw_samples.u16          # до decoder
linear_camera_rgb.f64    # идеальный demosaic/reference RGB
xyz_d65.f64              # reference colorimetric image
srgb16.tiff              # export reference, tagged profile
metadata.json             # exact CFA, levels, matrices, orientation
```

Для реальных D1–D5 эталон строится двумя независимыми путями: LibRaw/dcraw и
darktable или RawTherapee с зафиксированными версиями, параметрами и профилем.
Это не «истина» для камеры; это cross-implementation oracle для регрессий.
Golden outputs должны быть версионированы вместе с алгоритмом, profile и ABI.

## 2. Метрики качества

Все pixel-wise метрики считают отдельно в linear light и в display-referred
sRGB. Перед сравнением геометрия и orientation нормализуются, края с halo
исключаются маской `valid = 2*tile_halo ... width-2*tile_halo`.

### 2.1 Decode exactness

Для synthetic RAW требование строгое:

```text
max_abs(sample_u16 - reference_u16) = 0
wrong_samples = 0
```

Для production camera RAW декодированный plane сравнивается с RawSpeed/LibRaw
только как differential test; disagreement помечается, а не «усредняется».

### 2.2 Ошибка цвета

Из linear RGB через зафиксированные camera matrix и Bradford adaptation получаем
XYZ, затем CIE Lab. Для каждой patch ColorChecker считаются:

```text
ΔE00 median, p95, max
neutral_gray = sqrt(a*² + b*²)
white_balance_error = |ΔC*ab| + λ|Δh°|
```

Публиковать median недостаточно: p95/max показывают выбитые каналы и неверную
матрицу. Патчи с clipping должны быть отдельной категорией, не исключаться из
результата молча.

### 2.3 Тональность и headroom

До tone mapping значения выше 1.0 сохраняются. На HDR synthetic ramp проверяем:

```text
monotonicity violations = count(y[i+1] < y[i])
clipped_area = count(y >= white_level) / pixels
highlight_hue_error = Δh° for patches near saturation
```

Для exposure stops используем:

\[
L' = 2^{e} L, \qquad
L_{display}=f(L'),
\]

и проверяем, что `e=+1` удваивает linear luminance до tone-map и не меняет
нейтральность серого.

### 2.4 Деталь и демозаика

На ISO 12233 slanted-edge считаем edge-spread function, derivative LSF и
получаем `MTF50`, `MTF10` в cycles/pixel. Для Siemens star измеряем радиус
первого alias/moire ring. Для dead-leaves — acutance и texture energy.

Отдельно считаются:

```text
edge_acutance = integral(absolute(LSF))
moire_energy = power(radial_band_alias) / power(signal_band)
zipper_score = high_frequency_energy(near_edge_mask)
```

Для tile atlas одна и та же synthetic диагональная линия должна иметь
непрерывный результат. Seam metric:

\[
E_{seam} = \operatorname{mean}_{x\in boundary}
\lVert I(x-1)-I(x+1)\rVert_2.
\]

Сравниваем `E_seam` с соседними внутренними границами, а не с нулём: допустимый
ratio `E_seam / E_interior` должен быть близок к 1.

### 2.5 Шум и flat-field

На плоских patches для каждой ISO/exposure считаются:

\[
SNR = 20\log_{10}(\mu/\sigma),\quad
PRNU = \sigma_{spatial}/\mu,
\]

раздельно для luminance и chroma. Denoise benchmark сравнивает SNR с потерей
MTF50: нельзя объявлять улучшение, если шум убран за счёт уничтожения деталей.

### 2.6 Геометрия и lens model

Для checkerboard вычисляется reprojection RMS в пикселях. Для CA chart считаем
межканальную регистрацию:

```text
CA_px = median(||edge_R - edge_G|| + ||edge_B - edge_G||)
```

Vignette измеряется как RMS отклонение flat-field после shading correction.

## 3. Качество и скорость: честные tiers

| Tier | Pipeline | Цель | Эталон |
|---|---|---|---|
| Fast preview | bilinear/EA, FP16/FP32 GPU | минимальный `T_first` | собственный CPU reference того же алгоритма |
| Balanced | MHC/EA или RCD, FP32, базовый highlight | хороший интерактивный просмотр | зафиксированный darktable/RawTherapee profile |
| Quality export | AMaZE/Markesteijn/quality CFA, FP32/f64 accumulation, ICC/DCP | финальный 16-bit output | versioned reference + chart metrics |

Не смешивать tiers в одном score. В отчёте всегда указывать algorithm, profile,
ISO и processing parameters. RawTherapee прямо отмечает, что AMaZE обычно
сильнее на низком ISO, LMMSE/IGV — на высоком, а для X-Trans важен Markesteijn;
это причина сравнивать алгоритмы по сценам, а не одним средним числом.

## 4. Reproducible protocol

Для каждого `(fixture, algorithm, backend, tier)`:

1. два warm-up запуска;
2. минимум 30 измерений для latency и 5 полных export runs;
3. p50/p95/p99 + 95% bootstrap CI;
4. CPU/GPU model, driver/API, OS, Rust/LLVM, compiler flags, power mode;
5. `cold-process`, `os-warm`, `disk-cache-warm`, `gpu-resident` — разные серии;
6. JSONL span trace: decode, tile, demosaic, color, upload, present, export;
7. quality metrics вычисляются из сохранённых linear TIFF/EXR, не из screenshot.

Для ColorChecker/flat-field/edge patches считаются paired bootstrap CI (resample
по patch, а не по отдельным пикселям, чтобы не завышать размер выборки). Для
двух алгоритмов используем paired ΔE/MTF differences и 95% CI; если интервал
пересекает ноль, результат объявляется `tie`, а не «победой» по третьему знаку.

Перед pixel comparison проверяем dimensions, orientation, ICC/DCP, crop и
white/black levels. Любое несовпадение metadata делает результат `invalid`, а
не «плохим качеством» реализации.

## 5. Regression gates

Порог зависит от tier и должен быть записан явно:

```text
decode exactness: 0 wrong u16 samples on D6
GPU-vs-CPU fast path: max_abs <= 1e-4 linear float, PSNR >= 70 dB
quality color: ΔE00 median/p95 does not regress > 0.1 / 0.3 from golden
geometry: reprojection RMS <= 0.25 px on synthetic chart
tile seams: seam_ratio <= 1.10 versus interior boundaries
interactive: frame p95 <= 16.7 ms, deadline miss < 1%
```

Абсолютные ΔE/MTF требования для camera RAW не универсальны: профиль камеры,
экспозиция и демозаика меняют oracle. Поэтому CI использует regression-to-golden,
а не скрытые «идеальные» числа.

## 6. Сопоставление open source

- **RawSpeed** — baseline entropy loader; тестировать byte correctness и
  `decode_Mpix/s`, но не выдавать его за display quality.
- **LibRaw** — decoder и dcraw-compatible processing API; каждый экземпляр
  processor может работать в отдельном thread, но память измеряется на instance.
- **darktable** — quality reference для pixelpipe; AMaZE/RCD и preview/final
  stages должны быть разнесены.
- **RawTherapee** — reference для сравнений demosaic и ISO-dependent quality:
  AMaZE, LMMSE/IGV, Markesteijn/X-Trans.
- **RapidRAW** — GPU/WGPU comparator: отдельно сравнивать first-frame,
  steady-state и final output, не смешивать с CPU quality path.

Любые цифры сторонних проектов публиковать только после запуска одинакового
fixture/harness. Версия проекта, параметры и cache state обязательны.

### Источники практик

- [RawTherapee Demosaicing](https://rawpedia.rawtherapee.com/Demosaicing)
- [darktable user manual](https://darktable-org.github.io/dtdocs/en/darktable_user_manual.pdf)
- [LibRaw C++ API overview](https://www.libraw.org/docs/API-overview.html)
- [RapidRAW repository](https://github.com/CyberTimon/RapidRAW)
