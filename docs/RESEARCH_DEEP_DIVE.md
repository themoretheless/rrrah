# Глубокий аудит RAW-viewer/editor: конкуренты, параллелизм и план до production

Дата среза: **2026-07-21**. Это сводный документ 50-ролевого исследования. В
среде доступно четыре параллельных слота, поэтому роли выполнялись волнами и
сведены в проверяемую матрицу, а не выдаются за 50 одновременно работающих
процессов. Матрица ролей и шкала доказательности находятся в
[`RESEARCH_50_ROLE_MATRIX.md`](RESEARCH_50_ROLE_MATRIX.md); подробные приложения:
[`RESEARCH_COMPETITORS.md`](RESEARCH_COMPETITORS.md),
[`RESEARCH_PAPERS.md`](RESEARCH_PAPERS.md),
[`RESEARCH_PRACTICE.md`](RESEARCH_PRACTICE.md).

## Итог в одном абзаце

Самая быстрая архитектура не пытается сделать «весь RAW в 20 потоков». Для CR2
lossless-JPEG entropy/predictor обычно является последовательным участком; для
DNG независимые strips/tiles дают настоящую параллельность. Поэтому идеальный
путь имеет разные гранулярности: быстрый probe и metadata без полного decode,
один CR2 entropy lane с SIMD и распараллеленной постобработкой, bounded pool для
DNG tiles, GPU compute для viewport и цветовых стадий, приоритетную tile
residency, prefetch соседних кадров и content-addressed cache. Первый настоящий
RAW-кадр, 60/120 Hz interaction и high-quality export — разные продукты и
должны иметь разные quality tiers и benchmark gates.

Главный вывод для `rrrah`: сейчас есть хороший архитектурный прототип с реальным
CR2/DNG full-RAW decode, GPU shader и persistent mosaic cache, но это ещё не
полноценный production editor. Критические недостающие блоки: metadata-only
open, независимый DNG tile API, streaming residency вместо eager atlas, MHC/RCD
quality path, DCP/ICC/OpcodeList/black-grid semantics, реальный GPU golden
corpus, scheduler с cancellation, export/sidecar/catalog и cross-device
benchmarks. Ниже эти пробелы разложены в порядок, который максимизирует скорость
и не жертвует фотографической корректностью.

## 1. Доказательность и границы утверждений

Каждый вывод помечается уровнем:

```text
E0  гипотеза или экспертная эвристика
E1  научная работа/прототип, не production proof
E2  открытая реализация
E3  повторяющаяся практика зрелых приложений
E4  воспроизводимо измерено в нашем harness на зафиксированном fixture
```

Решение допускается в P0 только при наличии E2, численного oracle и E4
benchmark. Форумные сообщения — полезные наблюдения о failure modes, но не
замена измерениям. Все внешние числа из статей и issues в этом исследовании не
переписываются как характеристики `rrrah`.

## 2. Сравнение конкурентов: что действительно следует перенять

| Решение | Сильная сторона | Ограничение/риск | Что берём в `rrrah` |
|---|---|---|---|
| RawSpeed | быстрый camera-specific C++ loader, SIMD | не display pipeline | отдельный decoder boundary и corpus coverage |
| LibRaw | широкая совместимость камер и fallback API | application сам решает cache/scheduler/GPU | compatibility backend, не архитектура UI |
| darktable | thumbnail/preview/export pixelpipes, selective pixelpipe cache, CPU+GPU overlap | OpenCL/driver variance, сложная memory policy | разные tiers, async queue, selective stage keys, tile RAM budget |
| RawTherapee | зрелый порядок цветового pipeline, AMaZE/RCD и отдельные preview/export paths | quality path дороже и в основном CPU/tiled | deterministic color contract и quality oracle |
| Lightroom/ACR | preview economics, Smart Preview/Camera Raw cache, Basic/Full GPU modes | закрытый engine, GPU benefit зависит от устройства | preview cache и честное разделение first-frame/final-export |
| Capture One | аппаратное распределение CPU/RAM/GPU и настраиваемый preview size | hardware acceleration не гарантирует одинаковую latency | cache-size policy и измерение разных interaction сценариев |
| FastRawViewer | очень быстрый culling и настоящий RAW вместо JPEG | не полный develop editor | priority первого кадра, соседний prefetch, мгновенные rating/reject |
| RapidRAW | современный Rust + `rawler` + wgpu/WGSL GPU-first pipeline | GPU-only/resource limits, driver/Wayland failures | cross-platform GPU adapter, CPU fallback, LRU/residency, ROI |
| digiKam | отдельный catalog, thumbnail, similarity и face DB | каталог не является pixel renderer | независимые DB/worker queues, не блокирующие viewer |

Первичные источники: [darktable pixelpipe](https://docs.darktable.org/usermanual/development/en/darkroom/pixelpipe/the-pixelpipe-and-module-order/),
[darktable OpenCL scheduling](https://darktable-org.github.io/dtdocs/en/special-topics/opencl/scheduling-profile/),
[RawTherapee pipeline](https://rawpedia.rawtherapee.com/Toolchain_Pipeline),
[RawTherapee demosaic](https://rawpedia.rawtherapee.com/Demosaicing),
[Adobe GPU FAQ](https://helpx.adobe.com/lightroom/desktop/kb/lightroom-gpu-faq.html),
[Capture One acceleration](https://support.captureone.com/hc/en-us/articles/360002412798/comments/360001134797),
[FastRawViewer features](https://www.fastrawviewer.com/about-and-features-1-2),
[RapidRAW](https://github.com/CyberTimon/RapidRAW),
[digiKam database](https://docs.digikam.org/en/getting_started/database_intro.html).

### Практический приоритет UX

Сначала пользователь хочет увидеть и выбрать следующий кадр, затем быстро
проверить exposure/WB/histogram на 100%, и только потом ждать сложный export.
Повторяющаяся ошибка проектов — тратить всю машину на thumbnails/full preview и
оставлять UI без scheduler budget. Правильная очередь:

```text
P0 visible tile/current frame
P1 halo tiles и 100% cursor ROI
P2 next/previous кадр
P3 thumbnail/metadata catalogue
P4 idle mips, AI, similarity, export
```

Embedded JPEG разрешён только как явно помеченный catalog thumbnail/fallback в
режиме диагностики; он никогда не является главным RAW-путём и oracle.

## 3. Параллелизм: математическая модель и практический дизайн

### 3.1 CR2 и lossless JPEG

Для обычного CR2 entropy lane хранит позицию Huffman bitstream, predictor state и
состояние предыдущей строки. Независимый запуск декодера с произвольного байта
создаёт неверные значения. Без restart marker нельзя обещать линейное ускорение
от числа worker-ов.

Безопасная схема:

```text
probe + slice/marker scan
        ↓
один sequential entropy/predictor lane (read-ahead)
        ↓ bounded row ring
N workers: predictor-adaptation, black/linearization, CFA tile packing, checksum
        ↓
GPU upload видимых row/tile regions
```

Если restart intervals доказаны parser-ом, scan можно разбить по marker
границам. Иначе распараллеливаются только postprocess и соседний кадр. Верхняя
граница по Amdahl:

\[
S(P)=\frac{1}{s+(1-s)/P}.
\]

При `s=0.85` восемь потоков дают не более `1/(0.85+0.15/8)=1.16×` для всей
задачи. Поэтому `RRAH_WORKERS=20` само по себе является анти-метрикой: оно может
увеличить RSS, contention и p95 без ускорения decode.

### 3.2 DNG strips/tiles

`TileOffsets`/`TileByteCounts` или независимые strips — естественная единица
планирования. Worker admission должен учитывать не только CPU, но и RAM,
compressed I/O и GPU staging:

\[
N=\min\left(N_{cpu},\left\lfloor\frac{B_{ram}}
 {B_{compressed}+B_{mosaic}+B_{halo}+B_{scratch}}\right\rfloor,N_{io},N_{upload}\right).
\]

Каждая tile проходит `Absent → Reading → Decoding → CPUReady → Uploading →
Resident`; ошибки не должны оставлять lock или бесконечный retry. Соседняя
halo-region имеет более высокий приоритет, чем background mip. Для MHC radius 2
физический tile `T×T` требует `(T+4)²` samples; для directional quality tier
радиус и scratch должны быть частью cost model, а не константой UI.

### 3.3 GPU: bandwidth-first, portable

RAW display чаще упирается в memory traffic, а не FLOPs. Базовый shader path:

```text
R16Uint mosaic load
→ black/white/linearization
→ CFA-aware demosaic
→ WB + camera matrix
→ exposure/tone/gamut
→ sRGB/HDR surface
```

Для bilinear/MHC используют workgroup tile и shared/local memory с halo; размер
16×16 — стартовая эвристика, которую надо подтвердить per-device benchmark.
Subgroup reductions полезны для histogram/scopes, но должны иметь feature-gated
fallback: portability `wgpu`/WebGPU важнее vendor-specific maximum. Один
asynchronous queue + staging ring + in-flight fence обычно надёжнее попытки
эмулировать несколько очередей на каждом backend. CPU↔GPU readback в interaction
path запрещён; histogram выполняется reduction-ом и возвращает небольшой buffer.

## 4. Cache, preload и selective invalidation

Полная cache-иерархия:

```text
L0  OS page cache / async file reads
L1  RAM decoded tiles (weighted 2Q/TinyLFU, pinned visible tile)
L2  GPU residency (byte-weighted LRU, fence-aware eviction)
L3  persistent immutable tiles/mips (ABI + source hash + checksum)
```

Ключ pipeline stage:

```text
stage_key = H(source_fingerprint, frame, roi, tile, mip,
               edit_subgraph, decoder_abi, shader_abi, semantic_version)
```

Exposure change не должен инвалидировать entropy/decode. Camera profile change
инвалидирует color и downstream, но не исходную mosaic tile. Такой selective
invalidation обычно приносит больше, чем увеличение worker count.

Cache admission должен быть byte-weighted, учитывать стоимость повторного
вычисления и pin-ить текущий viewport. Простая count-LRU опасна: один 200 MB
full-res entry может вытеснить сотни горячих маленьких tiles. Persistent blob
должен писаться во временный файл с checksum и atomic rename; malformed cache
удаляется только после проверки magic/schema/payload/finite metadata.

Preload не означает загрузить весь каталог. Нужна модель вероятности:

\[
value(tile)=P_{visible}(tile)\cdot latency_{saved}(tile)-cost_{io+ram+gpu}(tile).
\]

Сначала visible/halo, затем соседний кадр в направлении навигации, только потом
idle jobs. При отмене viewport generation старые задачи могут завершиться, но
stale result обязан быть отброшен до cache publish и GPU upload.

## 5. Математика качества, которая должна оставаться детерминированной

### Demosaic tiers

| Tier | Алгоритм | Радиус | Назначение |
|---|---|---:|---|
| fast | bilinear | 1 | первый RAW-кадр, filmstrip, fallback |
| balanced | Malvar–He–Cutler | 2 | интерактивное 100%/fit |
| quality | RCD/AMaZE-подобный directional | 2–4+ | idle/export, foliage/диагонали |
| experimental | learned joint ISP | model-dependent | opt-in low-light/burst |

MHC — лучший следующий production kernel: фиксированные 5×5 коэффициенты,
малый halo, SIMD/WGSL-friendly и scalar golden reference. Известны формулы и
фазовые фильтры в [Malvar–He–Cutler](https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/Demosaicing_ICASSP04.pdf)
и [IPOL reference implementation](https://www.ipol.im/pub/art/2011/g_mhcd/revisions/2011-08-14/g_mhcd.htm).
RCD/AMaZE дают лучшую детализацию, но дороже и сложнее для tile fusion.

### Linear RAW и color

Все операции до tone mapping выполняются в scene-linear:

\[
L=\frac{\max(raw-B(x,y),0)}{W(x,y)-B(x,y)},\qquad L'=2^eL.
\]

`B` и `W` должны поддерживать repeat-grid, per-channel и row/column deltas;
clamp до tone map не должен съедать highlight headroom. Camera matrix и CAT
считаются в f64 при построении профиля, runtime может быть f32 при проверке
ошибки. Для HDR bracket merge:

\[
E(p)=\frac{\sum_i w_i(p)\,f^{-1}(R_i(p))/(t_i g_i)}
              {\sum_i w_i(p)+\epsilon}.
\]

Баланс белого, matrix, tone curve и gamut mapping не должны быть зашиты в один
непроверяемый shader. Каждая стадия имеет version и scalar oracle.

### Denoise и learned ISP

Минимальная физическая модель:

\[
Var(y|x)=a x+b,
\]

где `a` зависит от shot noise/gain, `b` — read noise. При умеренном шуме
demosaic→denoise обычно дешевле и достаточно качественно; при высоком ISO
полезен частичный CFA denoise перед demosaic. Learned models (RAWMamba,
joint burst ISP) остаются optional: domain shift, веса, лицензии, tile seam и
hallucination делают их неприемлемыми default-path без camera corpus и
confidence gate. Свежие paper результаты — E1, а не автоматический P0.

## 6. Новые инновации 2024–2026 и production status

| Направление | Польза | Статус для rrrah |
|---|---|---|
| DNG independent tiles + async staging | viewport-first и масштабирование | **P0**, production |
| selective pixelpipe/content-addressed cache | не пересчитывать неизменившиеся stages | **P0**, production |
| GPU subgroup reductions | histogram/scopes и меньший traffic | **P1**, feature-gated |
| JPEG XL progressive/region cache | компактный persistent preview/mip | P1/P2, проверить decoder/license |
| ISO 21496-1:2025 gain maps | корректный HDR SDR/HDR mapping | P1, export/display contract |
| calibrated Poisson–Gaussian denoise | предсказуемый high-ISO результат | P1 |
| burst joint demosaic/denoise | качество при серии кадров | P2, heavy compute |
| learned RAW restoration/RAWMamba | low-light/creative quality | P2, opt-in only |
| RAWIC-style lossless compression | потенциально меньше L3 cache | P2 research, не hot path |
| vendor-specific CUDA/Metal imageblocks | максимум на одной GPU | optional backend, не correctness path |

Источники: [ISO gain map](https://www.iso.org/standard/86775.html),
[JPEG XL whitepaper](https://ds.jpeg.org/whitepapers/jpeg-xl-whitepaper.pdf),
[RAWIC](https://arxiv.org/abs/2603.28105),
[RAWMamba](https://arxiv.org/abs/2409.07040),
[joint demosaic/denoise](https://arxiv.org/abs/2408.06684),
[Vulkan subgroups](https://docs.vulkan.org/guide/latest/subgroups.html).

### Что устарело или опасно

- embedded JPEG как основной кадр;
- полный high-quality render до первого present;
- 20+ неограниченных `spawn_blocking` worker-ов;
- произвольное разбиение CR2 bitstream без restart proof;
- одна гигантская GPU texture/atlas;
- count-only LRU без byte budget и fence-aware eviction;
- OpenCL-only GPU path без capability probe/CPU fallback;
- mean-only benchmark без p95/p99, RSS/VRAM, dropped frames и quality;
- gamma-space HDR merge и независимый RGB clipping;
- усреднение Bayer samples для mip без CFA-aware reduction;
- silent JPEG fallback при ошибке RAW;
- default learned model без camera validation и hallucination tests;
- сравнение алгоритмов по одному JPEG preview вместо linear RAW oracle.

## 7. Аудит текущего `rrrah`

### Уже есть и является хорошей базой

- Rust workspace с SOLID-разделением `core/decode/cache/gpu/app/bench`;
- `rawler` вызывается с `raw_image(..., false)`, embedded JPEG не подменяет RAW;
- CR2/DNG full-RAW decode в `u16` mosaic;
- checked dimensions, finite metadata, cache checksum и atomic rename;
- Bayer phase/orientation/halo/color math и unit tests;
- WGSL viewport path: normalize → demosaic → WB → matrix → tone map;
- eager GPU atlas имеет hard cap, чтобы не повторять ошибку texture limit 8192;
- monotonic generation/token и зачаток cancellation/stale-publish protection;
- weighted RAM LRU, persistent mosaic cache, JSONL/Chrome telemetry;
- process-boundary benchmark harness с p50/p95/p99 и cache-state labels;
- baseline Canon 5DS CR2: около 396.6 ms decode и около 97.2 ms warm cache
  pipeline на текущей машине (это локальная точка, не индустриальный claim).

### Release blockers (не скрывать статусом “GPU enabled”)

| Блокер | Почему критичен | Приёмочный тест |
|---|---|---|
| eager atlas вместо residency | 45–120 MP и 8192/16384 limits ломают upload | visible tile present при ограниченном VRAM |
| rawler materializes full frame | DNG viewport-first не достигается | `first_visible_tile` без full-frame allocation |
| нет metadata-only open | UI ждёт дорогой decode | metadata/filmstrip не блокируют RAW worker |
| CR2/DNG scheduler не production | workers/generation не связаны с budget | cancellation p95 ≤20 ms, stale=0 |
| shader только 2×2 black approximation | цвета/границы неверны на DNG grids | grid/opcode corpus, linear diff oracle |
| нет MHC/RCD quality path | bilinear недостаточен для editor quality | MHC scalar vs SIMD/WGSL <=1 LSB |
| нет DCP/ICC/OpcodeList contract | camera/display color mismatch | ColorChecker ΔE00 + explicit degraded state |
| нет runtime GPU golden renders | unit tests не видят driver/shader issues | headless/reference backend corpus |
| нет export/sidecar/catalog | это viewer prototype, не editor | deterministic TIFF/PNG + XMP round trip |
| corpus слишком узок | одна Canon Bayer не покрывает камеры | ≥12 fixtures, X-Trans/float/malformed |

### Оценка зрелости

| Область | Состояние | Уровень |
|---|---|---:|
| RAW decode correctness для поддержанного CR2 | рабочий прототип | E2/E4 на ограниченном corpus |
| first RAW display | есть GPU path, но eager upload | E2 |
| DNG independent tiles | спецификация, не end-to-end | E1/E2 |
| cache integrity/weighted RAM | реализовано и протестировано | E2/E4 |
| color/demosaic quality | bilinear + базовая математика | E2, не editor parity |
| scheduler/cancellation | частично | E1/E2 |
| telemetry/bench harness | schema + scripts, app integration неполная | E2 |
| production editor functions | отсутствуют export/catalog/history | E0/E1 |

## 8. План до “идеала”: порядок, а не список хотелок

### P0 — fast, correct, bounded viewer

1. `ProbeResult` с TIFF/IFD/CR2/DNG metadata без materialize full mosaic.
2. Безопасный parser/decoder worker process: quotas, timeout, fuzz corpus,
   deterministic errors, no panic/OOM/UB.
3. DNG `TilePlan` + independent decode; CR2 sequential lane + row ring.
4. Priority scheduler visible→halo→adjacent→background, generation gate,
   backpressure и byte-based admission.
5. GPU tiled residency, staging ring, fence-aware LRU; удалить eager atlas как
   default и оставить его только для малых fixtures.
6. Persistent tile/mip blob с BLAKE3 source+ABI+semantic key, checksum и atomic
   commit; 2Q/TinyLFU weighted RAM policy.
7. Full black/white/linearization grids, CFA phase, orientation, DNG OpcodeList
   support или явный degraded status.
8. Bilinear fast + MHC balanced; scalar CPU golden, SIMD/WGSL equality и seam
   tests.
9. GPU capability probe, shader/pipeline cache, CPU fallback, device-loss
   recovery, visible diagnostics.
10. Live HUD: `T_metadata`, `T_first_raw`, `T_first_present`, `T_visible_complete`,
    queue depth, cache hit, RSS/VRAM, dropped/stale frames и backend.

### P1 — editor quality and repeatable color

1. RCD/AMaZE-like quality tier и quality corpus (foliage, hair, text, stars).
2. DCP dual-illuminant, ICC/OCIO, Bradford/CAT16, D50/P3/HDR surfaces.
3. Calibrated Poisson–Gaussian denoise, highlight reconstruction, lens shading
   и lensfun metadata.
4. Histogram/scopes on GPU reduction; curves, levels, tone, WB solver, crop,
   rotate, masks/brushes с dirty-ROI invalidation.
5. TIFF/PNG/JPEG/AVIF export, XMP/sidecar round trip, deterministic metadata.
6. Catalog DB + filmstrip thumbnails и import workers, которые не блокируют
   pixelpipe.

### P2 — advanced/optional

Burst alignment/merge, HDR brackets, panorama/focus stack, super-resolution,
learned denoise/RAWMamba, RAWIC/JPEG XL persistent compression, plugin/IPC
backend и vendor-specific CUDA/Metal optimizations. Каждый P2 модуль должен
иметь explicit opt-in, model/codec license, memory quota и fallback.

## 9. Benchmark contract: как доказать скорость

Нельзя сравнивать одним числом `rrrah`, darktable, RawTherapee и FastRawViewer.
Нужны одинаковый fixture, режим cache и определения событий:

```text
T_meta       = path select → validated metadata
T_first_raw  = path select → first pixel derived from sensor mosaic
T_present    = path select → first visible GPU present
T_complete   = path select → all visible tiles ready
T_steady     = p95 frame latency over 300 zoom/pan events
T_export     = decode + process + encode + fsync
```

Для каждого результата хранить: полный SHA/BLAKE3 fixture, размер/модель/
разрешение, OS/CPU/RAM/GPU/driver/API, release flags, power mode, backend,
workers, cache state, p50/p95/p99, RSS/VRAM, dropped/stale count, throughput и
качество (`PSNR`, `SSIM`, `ΔE00`, neutral drift, seam ratio). Cold OS page-cache
run без привилегированного flush помечается `unknown/os-warm`, а не называется
cold.

Минимальный corpus: три CR2 (с/без restart), три DNG (strips/tiles/float),
два non-Bayer, два black-grid/Opcode/orientation, два stress/malformed; плюс
synthetic four Bayer phases and tile seams. Embedded JPEG запрещён как oracle.

### Реальные gates

Это engineering targets для одной фиксированной машины, не обещания для любого
железа:

```text
metadata ready                         p95 ≤ 20 ms
warm first RAW present                 p95 ≤ 150 ms
cold 50–70 MiB CR2 decode             p95 ≤ 500 ms
steady interaction 60 Hz               p95 ≤ 16.7 ms, dropped < 1%
stale published results                0
cancel after new generation            p95 ≤ 20 ms
tile-vs-monolithic linear diff         ≤ 1 sensor LSB interior/seam
balanced neutral chroma drift          ΔE00/ab threshold per profile
DNG 8-worker scale efficiency          report, target ≥ 0.5 until I/O saturation
RSS/VRAM                                explicit budget, no swap/device loss
```

Индустриальные приложения следует запускать тем же harness. Пока локально
установлены не все конкуренты, честный результат — “not measured”, а не
синтетическая таблица. Existing scripts:

```bash
scripts/bench-harness.py /path/to/file.CR2 --repetitions 9 --output target/bench/runs.jsonl
scripts/bench-report.py target/bench/runs.jsonl --json-out target/bench/report.json
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## 10. Decision log

1. **Rust остаётся orchestration/core language.** Ownership, bounded channels,
   deterministic cache and wgpu portability дают лучший баланс; raw decoder
   compatibility may remain an isolated C/C++/Rust backend.
2. **GPU не является correctness oracle.** CPU scalar reference defines output;
   GPU is accepted only after numerical and seam comparison.
3. **CR2 and DNG get separate cost models.** One generic “parallel RAW decoder”
   is architecturally false.
4. **Preload is a scheduler policy, not a bulk read.** Visible and adjacent have
   priority; idle jobs are cancellable.
5. **Quality tiers are explicit.** Fast bilinear may be first; it must not be
   silently presented as final-quality AMaZE/RCD.
6. **No silent degraded color.** Missing DNG opcodes/profile is shown in
   diagnostics and never hidden behind embedded JPEG.
7. **Measured claims are hardware-labelled.** p95/p99 and memory budgets matter
   more than a single average ms/file.

## Verdict

`rrrah` уже прошёл наиболее важный conceptual barrier: он действительно строит
первый кадр из RAW mosaic, а не из встроенного JPEG, и имеет измеримый cache/GPU
контур. До идеала по запросу пользователя осталось не “добавить ещё 100
эффектов”, а закрыть десять системных blockers из раздела 7, после чего
добавлять quality/editor functions в P1/P2. Следующая безопасная реализационная
итерация — **P0.1: metadata-only probe + DNG TilePlan + priority scheduler + GPU
residency**, а не нейросетевой фильтр и не увеличение количества потоков.
