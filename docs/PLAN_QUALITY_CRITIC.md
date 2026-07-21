# P1 quality, color and benchmark plan — independent critic

Дата аудита: **2026-07-21**. Этот документ является отдельным quality-контрактом
для P0/P1 работ. Он не считает RAW-derived bilinear preview полноценной
фотографической обработкой и не разрешает публиковать скорость без проверки
качества, памяти и отмены.

## 1. Вердикт по текущей архитектуре

Сильные стороны уже заложены правильно: rawler вызывается для sensor samples, а
не для встроенного JPEG; координаты CFA отделены от crop/orientation; есть
проверки finite metadata, singular matrix, halo и generation gate; telemetry и
quality benchmarks описаны отдельно от latency. Это хороший fast-preview
скелет, но не production RAW editor.

Критические недостающие части:

| Блокер | Текущий риск | Приёмка до production claim |
|---|---|---|
| Полный black/white grid | top-left 2×2 аппроксимация меняет экспозицию по кадру | synthetic 2×2, 4×4, row/column delta и camera grid дают одинаковый CPU/GPU result |
| DCP/ICC | 3×3 sRGB fallback не является color management | DCP dual-illuminant + ICC v4 3D-LUT differential tests |
| OpcodeList | DNG с lens/shading/bad-pixel opcodes имеет неверную семантику | исполнение только разрешённого подмножества с versioned oracle |
| Demosaic | bilinear визуально быстрый, но не quality tier | MHC/RCD/AMaZE tier-specific golden corpus |
| Float DNG | декодер отклоняет floating-point samples | explicit f16/f32 path, no integer reinterpretation |
| GPU residency | eager atlas упирается в 512 MiB и текстурные лимиты | bounded tile LRU, upload budget и eviction fence |
| Golden GPU | unit tests не проверяют реальный shader/adapter | headless wgpu render + readback против CPU oracle |
| Denoise | SNR можно улучшить уничтожением деталей | Poisson–Gaussian calibration + SNR/MTF Pareto gate |
| Export/sidecar | viewer output не воспроизводим и не сохраняет edit graph | deterministic 16-bit export, atomic XMP, source/profile ABI key |

До устранения этих блокеров допустимо говорить только «lossless RAW decode +
GPU bilinear preview», а не «полноценный RAW editor».

### Проверка на ложное срабатывание

В текущем `raw_view.wgsl` `clamp(0,1)` внутри `aces_fitted` находится уже в
финальном display-referred pass и поэтому сам по себе не является P0-багом.
`normalized_sample` ограничивает только нижнюю границу (`raw-black >= 0`) и не
режет верхний headroom. Исправить нужно именно контракт документации
(`P0_QUALITY_AUDIT.md` и `TILED_MATH.md`, где linearization всё ещё записана с
верхним clamp) и сохранить это различие при добавлении MHC/DCP/HDR: верхний
clamp допустим на display/export quantization, но не в scene-linear pipeline.

## 2. Неподвижный quality pipeline

Каждый tier должен использовать один и тот же порядок операций:

```text
probe → OpcodeList1 → linearization/black/bad-pixel → OpcodeList2
→ CFA demosaic → OpcodeList3 → WB → DCP/ICC camera transform → lens/shading/CA
→ Poisson–Gaussian denoise/detail → masks/local edits
→ exposure/tone/gamut → display transfer → export encoding
```

Смысловые правила:

1. `black(x,y,plane)` вычисляется в **глобальных sensor coordinates**, до crop и
   EXIF orientation. `white - black <= 0`, NaN/Inf и переполнение индексов —
   hard error, не silent clamp.
2. Значения выше 1.0 сохраняются до highlight/tone stage. Нельзя делать
   `clamp(0,1)` сразу после linearization: это уничтожает headroom для WB, HDR и
   highlight reconstruction.
3. Exposure — scene-linear: `L' = 2^stops * L`. Перевод в gamma/sRGB происходит
   только перед display/export.
4. Crop/orientation меняют только display mapping. CFA parity и black-grid lookup
   не должны зависеть от UI viewport.
5. Cache key содержит source fingerprint, decoder/profile ABI, algorithm tier,
   opcodes ABI и semantic edit graph. Exposure/tone-only edits не инвалидируют
   immutable decoded mosaic.

### Tiers, которые можно честно сравнивать

| Tier | Реализация | Где разрешена | Reference |
|---|---|---|---|
| Fast preview | CFA-aware bilinear или EA, f32 GPU | первый кадр, навигация | собственный scalar bilinear |
| Balanced | MHC 5×5 или RCD с halo, f32 | интерактивный idle/refine | matching MHC/RCD CPU oracle |
| Quality | AMaZE-like/Markesteijn для соответствующего CFA, f32 accumulation | экспорт и final render | versioned oracle + chart metrics |
| Optional ML | joint denoise/demosaic/RAW restoration | opt-in, явно маркированный | fixed model hash + non-hallucination tests |

Нельзя сравнивать Fast preview с AMaZE по одним и тем же ΔE/MTF цифрам и
выдавать меньшую latency за «лучшее качество». Для X-Trans должен быть отдельный
Markesteijn tier; для высокой ISO — ISO-aware balanced policy (например,
LMMSE/IGV) по аналогии с практиками RawTherapee.

## 3. MHC: scalar oracle → SIMD → WGSL

### 3.1 Scalar oracle

Публикуемая реализация сначала делается на `f64`, без `fast-math`, с явной
функцией `sample(global_x, global_y, plane)` и border policy `clamp`. Для каждого
пикселя известной CFA-фазы вычисляется 5×5 коррекция цвета. Общая форма:

\[
  \hat C_p(x,y)=M_p(x,y)+\sum_{(d_x,d_y)\in[-2,2]^2}
       K_{p,\phi(x,y)}(d_x,d_y)\,D_{\phi}(x+d_x,y+d_y),
\]

где `M_p` — исходный sample этого цвета (если он есть), `D` — цветовая
разность между редким каналом и опорным каналом, `φ` — CFA phase. Набор
`K_{p,φ}` хранится как versioned coefficient table из Malvar–He–Cutler, а не
переписывается вручную в каждом backend. Для sanity check каждая таблица должна
иметь:

```text
sum(K) == 1 for direct interpolation kernels
DC(constant mosaic) == constant
horizontal/vertical kernels are exact rotations/reflections
all coefficients finite and bounded by declared max_abs
```

Пример зелёного канала на R/B sample (5×5, координаты `[-2..2]`) в MHC:

```text
 0    0  -1/8  0    0
 0    0   1/4  0    0
-1/8  1/4 1/2  1/4 -1/8
 0    0   1/4  0    0
 0    0  -1/8  0    0
```

Остальные R/B-at-G и R↔B kernels выбираются по horizontal/vertical phase и
проходят тот же DC/rotation тест. В reference pipeline accumulator — `f64`,
результат сохраняется в linear RGB до tone map. Для границы sensor применяется
тот же clamp, что и в tile path; не использовать zero padding.

Оракул обязан пройти:

* constant mosaic для всех RGGB/BGGR/GRBG/GBRG phases;
* red/green/blue impulse (no channel shift);
* diagonal edge through every tile seam;
* black/white grids and odd crop offsets;
* all eight EXIF orientations;
* `tile+halo == monolithic` до `1 LSB` integer / `PSNR >= 60 dB` float.

### 3.2 CPU SIMD

После bit-exact scalar oracle строится SoA tile layout: four CFA planes и
contiguous rows с halo `H=2`. Это предпочтительнее gather из interleaved mosaic.

* AVX2: 8 `f32` lanes; AVX-512: 16; NEON: 4/8 в зависимости от target.
* Разворачивать 5 rows × 5 taps; FMA разрешить только после differential test
  против scalar, с ULP budget, зафиксированным per target.
* Каждая SIMD функция должна иметь `unsafe` boundary только вокруг aligned
  slices; размеры tile проверяются до векторного цикла, хвост идёт scalar.
* Benchmark должен разделять `load/transpose`, convolution и store; иначе
  «ускорение MHC» может оказаться выигрышем только на синтетическом aligned
  buffer.

### 3.3 WGSL

Базовая portability path — workgroup 16×16, shared tile `(16+2H)²`, один barrier
после загрузки halo. Рабочая группа пишет только interior; соседние tiles не
гоняются за общие пиксели. Для R16Uint использовать `textureLoad`, а не hardware
filtering. Subgroup operations — optional feature path с runtime capability
check; результат должен проходить тот же epsilon gate на Vulkan, Metal и DX12.

Два shader-а нужны явно:

1. `mhc_preview`: f32, быстрый path, без denoise;
2. `mhc_quality`: больше taps/precision, output в linear RGB storage/attachment.

Нельзя смешивать GPU queue submission time и shader time: trace должен включать
upload, command encoding, submit, fence/readback и present отдельно.

## 4. DNG semantics: grid, DCP, ICC, opcodes

### 4.1 Black/white и linearization

Планируется immutable `PhotometricModel`:

```text
raw_u = sample(global_x, global_y)
u = LinearizationTable[raw_u]                 // если присутствует
b = repeat_grid(x,y,plane) + row_delta[y] + column_delta[x]
L = (u - b) / max(white(x,y,plane) - b, 1)
```

Grid lookup — storage buffer или small texture с explicit dimensions; top-left
2×2 uniform approximation удаляется из production path. Индексы проверяются
через checked arithmetic и привязываются к `max_pixel_budget`.

Floating DNG samples (`f16/f32`) идут через отдельный `RawSample` representation;
запрещено трактовать float bytes как `u16`. Для float path white/black и
non-finite policy тестируются отдельно.

### 4.2 OpcodeList

Определить versioned, bounded subset:

* `OpcodeList1`: операции до linearization/ранний raw stage;
* `OpcodeList2`: операции после linearization, до demosaic;
* `OpcodeList3`: операции после demosaic.

Каждый opcode получает `id`, `version`, declared read/write footprint, max
scratch bytes и cancellation check. Неизвестный opcode — `UnsupportedOpcode`, а
не попытка «приблизительно пропустить». Spatial opcodes должны работать в
sensor coordinates и расширять tile halo; global opcodes (например, gain) можно
применять отдельным pass. Поэтапное внедрение:

1. identity + point/gain + bad-pixel map;
2. warp/lens shading с explicit tile dependencies;
3. remaining opcodes только после corpus и security review.

### 4.3 DCP

`ColorMatrix1/2` и `ForwardMatrix1/2` — это не один универсальный 3×3 fallback.
Для dual-illuminant profile интерполяция выполняется в reciprocal temperature:

\[
u=1/T,\qquad
\alpha=\operatorname{clamp}\frac{u-u_1}{u_2-u_1},\qquad
M=(1-\alpha)M_1+\alpha M_2.
\]

При `T=T1` это даёт `M1`, при `T=T2` — `M2`; близкие или одинаковые
illuminant temperatures обрабатываются как один профиль, а не через деление на
малый знаменатель.

Профильный builder должен:

* нормализовать white point и согласовать D50/D65 через Bradford;
* проверять determinant/condition estimate и finite coefficients;
* различать `ColorMatrix`, `ForwardMatrix`, `ReductionMatrix`;
* выбирать illuminant по camera metadata/WB и записывать choice в trace;
* иметь CPU f64 reference и GPU f32 implementation.

Если DCP отсутствует, явно показывать profile fallback в diagnostics; silent
identity не считать color-managed результатом.

### 4.4 ICC/OpenColorIO

ICC v4/Display-P3/HDR pipeline должен иметь CPU oracle (LittleCMS/OpenColorIO
version pinned) и GPU 3D LUT path. Нельзя сводить ICC к одной matrix: TRC,
black point, A2B/B2A LUT и gamut mapping значимы на saturated patches. LUT
resolution (например, 33³/65³) выбирается по ΔE00 budget, а не только по памяти;
в тесте измеряются interpolation error и out-of-gamut policy.

## 5. Poisson–Gaussian denoise и detail

### 5.1 Модель

Для нормализованного RAW sample:

\[
  \operatorname{Var}(X\mid\mu)=a\mu+b,
\]

где `a` — shot-noise slope, `b` — read-noise variance, измеренные по flat-field
серии для camera/ISO/temperature. Для стабилизации можно использовать
generalized Anscombe:

\[
  Z(X)=\frac{2}{a}\sqrt{aX+\frac{3}{8}a^2+b},
\]

с inverse transform, проверенным на low-signal bias. Параметры не брать из
одной фотографии: минимум три flat/exposure levels и confidence interval.

### 5.2 Реализация и порядок

P1 baseline — CFA-aware edge-preserving 5×5/7×7 filter в linear sensor domain,
с strength, зависящим от `a*mu+b`. Он должен сохранять green detail и не смешивать
несовместимые CFA planes. Joint demosaic+denoise и RawMamba/learned restoration
оставляются opt-in P2: модель, веса, backend и precision являются частью cache
key и export provenance.

Приёмка не по одной SNR:

```text
SNR gain, MTF50 loss, dead-leaves texture energy, chroma blotch score,
flat-field PRNU, highlight hue error, latency, peak RSS/VRAM
```

Denoise улучшение принимается только на Pareto frontier: нельзя получить «pass»
простым blur, который разрушает MTF или texture energy.

## 6. GPU golden corpus и numerical gates

### Corpus

Минимальный deterministic corpus должен содержать:

* четыре Bayer phase, neutral ramp, color impulses, saturated/sub-black patches;
* 2×2, 4×4 и non-uniform black grids, row/column deltas, linearization table;
* DNG strip/tile, OpcodeList1/2/3, uncompressed/lossless-JPEG, f16/f32 sample;
* diagonal/vertical edge across every tile seam, odd tile sizes 257/511;
* eight EXIF orientations, crop offsets 0/1, incomplete edge tiles;
* real CR2/DNG camera set with source hash, decoder version, ISO and profile;
* malformed/truncated/huge dimensions/unknown opcode/security cases.

Embedded JPEG запрещён как oracle. Для synthetic fixture хранить source `u16/f32`,
linear camera RGB, XYZ D65, expected metadata и tagged export. Для реальных RAW
использовать два независимых pinned references (например, LibRaw и
darktable/RawTherapee); disagreement помечать как differential finding, а не
усреднять.

### Headless adapter test

На CI запускать wgpu headless для доступных adapters. Для каждого shader:

1. upload known tile;
2. render в float storage/texture;
3. readback после fence;
4. сравнить с scalar f64 oracle и сохранить failure tile/parameters.

Разные adapters могут отличаться на ULP из-за FMA/precision. Допустимый budget
фиксируется отдельно: fast GPU `max_abs <= 1e-4` linear RGB, quality path —
per-kernel ULP/ΔE budget. `NaN`, `Inf`, negative after declared clamp или stale
generation — безусловный fail.

### Метрики

Линейная ошибка:

\[
MSE=\frac1N\sum_i (x_i-y_i)^2,\qquad
PSNR=10\log_{10}\frac{MAX^2}{MSE}.
\]

Для normalized linear RGB `MAX=1`; для integer RAW `MAX=white_level`, явно
указанный в отчёте. PSNR не заменяет цветовые и структурные метрики.

CIEDE2000 считается через стандартное преобразование XYZ→Lab (D50 или D65
явно фиксируется), chroma/hue weighting `S_L,S_C,S_H` и rotation `R_T`. Отчёт
содержит median/p95/max по ColorChecker patches; clipped patches не выкидываются
молча, а идут отдельной категорией.

Seam metrics:

\[
E_{seam}=\operatorname{mean}_{p\in\text{boundary}}
\|I(p-\mathbf n)-I(p+\mathbf n)\|_2,
\quad
R_{seam}=E_{seam}/E_{interior}.
\]

Ожидается `R_seam <= 1.10`, `max_abs(tile-monolithic) <= 1 LSB` для integer
preview и `PSNR >= 60 dB` для float quality path. Дополнительно измерять
gradient continuity, чтобы одинаково-серые tiles не скрывали seam.

## 7. Export и sidecar milestones

### P1 minimum

1. Export 16-bit TIFF (linear or tagged sRGB/P3), PNG для display-referred,
   deterministic rounding-to-nearest-even и final clamp только перед quantization.
2. Профиль/black-white/WB/exposure/demosaic algorithm записываются в output
   metadata; ICC/DCP embedding policy видима пользователю.
3. XMP sidecar пишется во временный файл в той же директории, `sync_data` плюс
   atomic rename; source RAW никогда не переписывается автоматически.
4. Sidecar содержит schema version, source fingerprint, edit graph, profile/model
   hashes, timestamps в canonical form. Повторный export одинакового входа даёт
   одинаковые pixel bytes и canonical metadata hash.
5. Export job bounded/cancellable: отмена не оставляет частичный final path,
   stale generation не публикует файл.

### P2 extension

Gain-map HDR по ISO 21496-1, OpenEXR, AVIF/JPEG XL progressive output, panorama/
stacking и learned restoration provenance. Gain map не смешивать с обычной
gamma curve: base image, gain map, headroom и display metadata тестируются
отдельно.

## 8. Adversarial review: где легко обмануться

### Ложные speed claims

* Сравнение с embedded JPEG — это не RAW decode и не full-quality render.
* Warm cache, predecoded persistent mosaic или уже resident GPU tile должны быть
  отдельными режимами; нельзя выдавать их за cold open.
* Shader-only время не включает disk I/O, entropy decode, staging upload, queue
  submit, fence и present. Async overlap считается через critical path, а не
  простой суммой span durations.
* Маленький synthetic DNG с независимыми tiles не доказывает ускорение CR2:
  Canon lossless-JPEG entropy stream имеет serial dependency.
* FPS без dropped-frame/deadline miss и input latency скрывает jank.
* RSS не измеряет VRAM, mapped pages и compressed-cache; нужны process RSS,
  resident decoded bytes, GPU allocated/resident/evicted и peak staging.
* Fast bilinear нельзя сравнивать по latency с AMaZE/RCD и заявлять «быстрее
  quality». В отчёте обязательны tier, algorithm, profile, fixture, cache state.

### Отсутствующие или опасные fixtures

Нельзя считать corpus достаточным без 4 CFA phases, distinct G1/G2, active-area
offsets, all orientations, non-uniform grids, OpcodeList, float DNG, saturated
headroom, bad-pixels, odd tile boundaries, truncated/huge inputs и реальных
camera profiles. JPEG previews и screenshots — только UI smoke tests.

### Numerical traps

* `clamp(0,1)` до highlight reconstruction; gamma-space exposure/HDR merge.
* Division by `white-black`, singular/ill-conditioned matrix, non-finite DCP/ICC.
* F16 underflow/overflow в dark/highlight pixels, unchecked f32 FMA drift,
  backend-specific relaxed precision и non-deterministic parallel reductions.
* Незаявленная channel order (G1/G2), crop-relative CFA parity, top-left grid
  approximation и zero-padding halo.
* Independent channel clipping вместо luminance/chroma gamut mapping.
* ΔE00 на screenshot без фиксированных white point/profile; PSNR по gamma RGB,
  где ошибка не пропорциональна scene-linear radiance.

### Cancellation and cache traps

Generation token проверяется после probe, read, entropy chunk, tile decode,
upload, submit и before publish/cache insert. `rawler` panic/slow call может
пережить отмену, поэтому process boundary/timeout остаются security milestone.
Evicted GPU resource нельзя уничтожать до submission fence. Cache insertion
после cancellation запрещён; key должен включать algorithm/profile/opcode/model
ABI, иначе старый quality result выдаётся как новый.

## 9. Приоритизированный delivery plan

### P0.1 — correctness before speed

* `PhotometricModel` с полной grid/linearization semantics;
* scalar f64 bilinear + MHC oracle, all CFA/orientation/halo tests;
* headless GPU bilinear golden path и generation/fence assertions;
* real corpus manifest, hashes, metadata validator и malformed fixtures.

### P0.2 — first useful quality

* DNG independent tile planner, bounded pool, visible-first scheduling;
* MHC SIMD and WGSL with identical coefficient tables;
* GPU tile residency/LRU replacing eager atlas;
* cold/warm benchmark matrix with critical-path telemetry.

### P1.1 — color and denoise

* DCP dual-illuminant + Bradford, full ICC/OCIO CPU oracle and GPU LUT;
* OpcodeList identity/point/bad-pixel subset with explicit unsupported errors;
* calibrated Poisson–Gaussian baseline, SNR/MTF/PRNU gates;
* CIEDE2000/PSNR/seam/MTF report generation.

### P1.2 — export contract

* deterministic TIFF/PNG 16-bit export, ICC embedding, atomic XMP/edit graph;
* cancellation, source/profile ABI provenance and reproducibility hash;
* only after these gates: camera-specific MHC/RCD/AMaZE tuning and UX defaults.

### P2 — research features

Gain-map HDR, JPEG XL/AVIF progressive output, joint learned demosaic/denoise,
stacking/panorama and plugin model. Они не должны блокировать стабильный
CPU/GPU reference path.

## 10. Release gates

Release candidate может называться «full RAW quality» только если одновременно:

```text
synthetic decode: 0 wrong samples
GPU-vs-CPU fast: max_abs <= 1e-4 linear, PSNR >= 70 dB
tile seam: R_seam <= 1.10; integer <= 1 LSB
neutral ColorChecker: ΔE00 median/p95 regression <= 0.1/0.3
geometry: orientation/crop error <= 0.5 px; lens RMS <= 0.25 px where enabled
interactive: frame p95 <= 16.7 ms, deadline miss < 1%
first RAW frame: cold/warm reported separately, no JPEG path
memory: decoded + compressed + staging + VRAM all under declared budgets
cancellation: stale result never published; export leaves no partial final path
```

Абсолютные camera-RAW thresholds не должны быть универсальными: profile, ISO,
exposure и algorithm входят в oracle identity. CI использует paired regression
against golden; confidence interval, пересекающий ноль, считается `tie`, а не
победой по третьему знаку.

## Источники, которые следует закрепить в manifest

* [RawTherapee demosaicing](https://rawpedia.rawtherapee.com/Demosaicing)
* [Malvar–He–Cutler demosaicing paper](https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/Demosaicing_ICASSP04.pdf)
* [IPOL MHC reference implementation](https://www.ipol.im/pub/art/2011/g_mhcd/revisions/2011-08-14/g_mhcd.htm)
* [RawTherapee processing pipeline](https://rawpedia.rawtherapee.com/Toolchain_Pipeline)
* [darktable pixelpipe](https://docs.darktable.org/usermanual/development/en/darkroom/pixelpipe/the-pixelpipe-and-module-order/)
* [LibRaw API](https://www.libraw.org/docs/API-overview.html)
* [RapidRAW](https://github.com/CyberTimon/RapidRAW)
* [OpenColorIO 2.2](https://opencolorio.readthedocs.io/en/latest/releases/ocio_2_2.html)
* [ISO 21496-1 gain maps](https://www.iso.org/standard/86775.html)
* [RawMamba](https://arxiv.org/abs/2409.07040)
* [Adobe DNG SDK stage/opcode implementation](https://android.googlesource.com/platform/external/dng_sdk/+/refs/heads/android14-prebuilt-test/source/dng_negative.cpp)
