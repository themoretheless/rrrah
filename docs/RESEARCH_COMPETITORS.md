# Конкуренты, научные работы и практическая архитектура RAW-редактора

Дата исследования: 2026-07-21. Цель: определить, какие решения реально применяются в быстрых RAW viewer/editor, какие идеи можно перенести в `rrrah`, а какие считаются устаревшими. Основной критерий — latency от выбора файла до первого корректного RAW-кадра, затем качество финального рендера и предсказуемое потребление RAM/VRAM.

## Краткий вывод

Главный вывод рынка не в том, что «20 потоков всегда быстрее». Лучший результат даёт конвейер с разной гранулярностью параллелизма:

```text
probe/metadata             один быстрый поток
RAW entropy                последовательный lane либо независимые DNG tiles
postprocess                bounded CPU pool
preview/mips               GPU async compute
visible tiles              priority scheduler
cache/hash/upload          overlapped I/O + staging ring
```

CR2 lossless-JPEG часто ограничен зависимостью Huffman/predictor state; DNG strips/tiles масштабируются гораздо лучше. Поэтому для `rrrah` нельзя обещать линейный speedup от количества thread-ов для всех форматов. Нужны отдельные cost models для CR2 и tiled DNG.

Самые зрелые практики конкурентов:

1. Несколько pipeline tiers: thumbnail/preview/full-quality, а не один одинаково дорогой pipeline.
2. Persistent preview/thumbnail cache с versioned invalidation.
3. GPU используется для интерактивного pixelpipe, CPU и GPU работают одновременно над разными viewport-ами.
4. Tile-wise processing и memory budgets вместо загрузки полного RGB кадра на каждый модуль.
5. Разделение immutable исходника и неразрушающих edit instructions.
6. Профилирование стадий и отдельные latency gates для first-frame, interaction и export.

## Сводное сравнение

| Решение | RAW engine | Pipeline/cache | GPU/parallelism | Практический урок для rrrah |
|---|---|---|---|---|
| [RawSpeed](https://github.com/darktable-org/rawspeed) | Специализированный C++ loader, camera-specific fast paths | Обычно cache и UI строятся в darktable | SIMD, threading там, где формат допускает | Быстрый entropy decode — отдельный слой, не пытаться сделать из него весь редактор |
| [darktable](https://github.com/darktable-org/darktable) | RawSpeed + pixelpipe | Thumbnail cache, pixelpipe intermediate cache, tile memory budget | OpenCL; preview CPU и center GPU параллельно; async kernels | Разные pipe-и и overlap важнее одной «суперфункции» |
| [RawTherapee](https://github.com/RawTherapee/RawTherapee) | LibRaw/dcraw-derived ingest + mature algorithms | Отдельные thumbnail/preview/saved pipelines, batch queue | CPU thread pool и tile processing | Quality path должен быть отдельным от fast preview; порядок модулей — часть color contract |
| [LibRaw](https://www.libraw.org/) | Широкий C/C++ decoder API | Не навязывает cache | CPU-oriented, application decides scheduling | Хороший fallback и corpus coverage, но не display architecture |
| [Adobe Lightroom](https://helpx.adobe.com/lightroom/desktop/kb/lightroom-gpu-faq.html) | Закрытый Camera Raw engine | Camera Raw cache, previews, Smart Previews | GPU режимы Basic/Full, обычно одна GPU; CPU/GPU split скрыт | User-visible speed depends on prebuilt previews and cache policy, а не только shader throughput |
| [Capture One](https://support.captureone.com/hc/en-us/articles/360002412798/comments/360001134797) | Закрытый engine | ImageCore/preview cache, configurable preview size | Hardware Acceleration: preview, fit, process используют CPU/GPU/RAM | Нужен cache size, соответствующий рабочему дисплею; expensive recompute нельзя делать на каждый UI event |
| [FastRawViewer](https://www.fastrawviewer.com/about-and-features-1-2) | Прямой RAW display с очень быстрым culling | Decoded-file cache и small previews | Фокус на sequential browsing/keyboard latency | Идеальный reference для first-frame и каталогов; простая функция часто быстрее полноценного editor pipe |
| [RapidRAW](https://github.com/CyberTimon/RapidRAW) | `rawler` в Rust | GPU pipeline + AI connector cache | WGPU/WGSL, весь processing pipeline на GPU | Хороший современный Rust/WGPU пример, но GPU-only fallback и ресурсные требования нужно контролировать |
| [digiKam](https://docs.digikam.org/en/getting_started/database_intro.html) | LibRaw/native или external darktable/RawTherapee | SQLite/MySQL metadata DB; PGF thumbnail DB; similarity/face DB | Основной упор на каталог, импорт и background workers | Catalog/search должны быть отдельным сервисом, а не блокировать pixel renderer |

## Архитектурные детали конкурентов

### darktable: зрелый pixelpipe и асинхронный GPU

Документация darktable описывает несколько pipe-ов: thumbnail, preview и export. Thumbnail pipe специально оптимизирован для обработки множества маленьких изображений одновременно. Для OpenCL есть профиль, где GPU занимается центральным изображением, а CPU в это же время считает navigation preview ([pixelpipe](https://docs.darktable.org/usermanual/development/en/darkroom/pixelpipe/the-pixelpipe-and-module-order/), [scheduling](https://darktable-org.github.io/dtdocs/en/special-topics/opencl/scheduling-profile/)).

Критически важные настройки:

- `opencl_async_pixelpipe`: уменьшает количество synchronizing interrupts;
- `opencl_synch_cache`: может сохранять intermediate GPU buffers, чтобы изменять только хвост pipeline;
- pinned host memory: на некоторых AMD перенос ускоряется в 2–3 раза;
- `opencl_micro_nap`: оставляет GPU время для GUI, иначе интерактивность становится рваной;
- tile host-memory limit: крупные кадры режутся на tiles при ограниченном RAM.

Это практическое подтверждение модели `rrrah`: один asynchronous queue и priority residency лучше, чем блокирующий `poll(Wait)` после каждого модуля. Однако cache промежуточных буферов надо включать selective: storage каждого stage может съесть RAM/VRAM быстрее, чем повторный пересчёт.

### RawTherapee: quality pipeline с жёстким порядком

RawTherapee документирует фиксированный порядок: dark frame/flat field/bad pixels/black point, lens distortion и chromatic aberration, white point, demosaic, highlight recovery, WB, crop, color conversion ([Toolchain Pipeline](https://rawpedia.rawtherapee.com/Toolchain_Pipeline)). Есть отдельные пути для main preview, saved image и thumbnail.

Практические выводы:

- нельзя произвольно переставлять color operations ради GPU throughput: изменится результат;
- preview должен иметь собственный quality tier и собственный budget;
- сложные demosaic и denoise не должны блокировать первое изображение;
- финальный export запускается отдельным batch pipeline с более высоким quality/precision.

### Lightroom/ACR: acceleration plus preview economics

Adobe официально разделяет GPU на Basic и Full: Basic ускоряет display/zoom, Full переносит часть pixel processing на GPU. В Lightroom обычно используется одна GPU; сама функция ускорения не отменяет стоимость генерации preview и Camera Raw cache ([GPU FAQ](https://helpx.adobe.com/lightroom/desktop/kb/lightroom-gpu-faq.html), [GPU preview generation](https://helpx.adobe.com/lightroom-classic/desktop/kb/gpu-preview-generation.html)).

Практический урок: для first-frame нужно строить preview заранее (import/idle), но UI должен сразу показывать RAW fast tier, если preview устарел. Нельзя полагаться на «автоматическую GPU оптимизацию» как на единственную стратегию: драйверы, VRAM и режимы питания сильно меняют latency.

### Capture One: hardware acceleration без гарантии одинаковой пользы

Документация Capture One прямо указывает, что hardware acceleration распределяет работу между CPU cores, RAM и GPU: preview updates, sorting/rating, fit-to-screen и processing имеют разные профили нагрузки. Размер preview cache — пользовательский параметр и влияет на latency ([hardware acceleration](https://support.captureone.com/hc/en-us/articles/360002412798/comments/360001134797), [preview cache](https://support.captureone.com/hc/en-us/articles/360002484457-Capture-One-Preferences-Settings-Image-tab)).

Это подтверждает необходимость в `rrrah` отдельного benchmark по:

```text
metadata/thumbnail → preview generation → fit display → 100% zoom → export
```

Один итоговый «ms/file» не отражает реального опыта.

### FastRawViewer: минимальный pipeline, максимальный browsing latency

FastRawViewer сознательно показывает настоящий RAW, а JPEG-превью оставляет отдельным режимом. Основная ценность — быстрый просмотр и culling больших серий, а не полный develop pipeline ([features](https://www.fastrawviewer.com/about-and-features-1-2), [manual](https://updates.fastrawviewer.com/data/FastRawViewer2-Manual-ENG.pdf)).

Для `rrrah` это эталон UX:

- открытие соседнего кадра должно быть priority P0;
- стрелка/Space должны работать без ожидания full-quality render;
- embedded JPEG допустим только как явно помеченный fallback/сравнение;
- rating/reject должны записываться сразу, не после завершения decode.

### RapidRAW: GPU-first Rust/WGPU

RapidRAW заявляет GPU-ориентированный pipeline через WGPU и custom WGSL, включая сложные edits/masks; для AI передаётся один раз full image, последующие запросы используют маленькую mask и текст, что уменьшает transfer ([repository](https://github.com/CyberTimon/RapidRAW)).

Сильные стороны: единый cross-platform shader path, низкая стоимость повторных edits, Rust ownership. Риски: GPU-only design может деградировать на integrated/старых GPU; большие texture limits требуют atlas/tiles; AI integration должна быть optional и sandboxed.

### digiKam: каталог как самостоятельная система

digiKam использует отдельные базы: core metadata, compressed thumbnail DB (PGF), similarity DB и face DB. Core DB может быть SQLite или MySQL/MariaDB ([database overview](https://docs.digikam.org/en/getting_started/database_intro.html), [features](https://www.digikam.org/about/features/)).

Урок: каталог нельзя строить поверх RAM cache редактора. Нужны:

- индекс файлов/metadata с WAL;
- thumbnail store с независимым ABI;
- background import workers;
- similarity/face jobs с pause/resume;
- collection path identity, чтобы removable volumes не ломали ключи.

## Параллелизм: что масштабируется, а что нет

### CR2/lossless JPEG

В entropy scan без restart markers состояние Huffman/predictor переносится между строками. Поэтому безопасный baseline:

```text
один sequential bitstream decoder
→ ring buffer decoded rows
→ N параллельных postprocess workers
```

Если присутствуют restart markers, поток можно разбивать по интервалам после предварительного scan. Общий принцип параллельного JPEG decode с restart intervals описан в [JParEnt](https://onlinelibrary.wiley.com/doi/10.1002/cpe.4111) и стандарте JPEG; без markers попытка независимого decode создаёт неверные predictor states.

### DNG strips/tiles

Каждый tile/strip имеет собственный offset/byte count и обычно может декодироваться независимо. Планировщик должен использовать bounded credits:


\[
N_{workers}=\min\left(N_{cpu},\left\lfloor\frac{RAM_{budget}}{tile\_bytes+halo}\right\rfloor, N_{io}\right)
\]

Полезны tile states:

```text
Absent → Reading → Decoding → CPUReady → Uploading → Resident
                           ↘ Failed
```

Приоритеты: visible tile > halo neighbor > next frame > background mip. Generation token аннулирует результат после смены файла/viewport.

### GPU

Грубый CPU/GPU split:

```text
CPU: parse, entropy, metadata, cache index, scheduling
GPU: demosaic, WB, matrix, tone, masks, resize, scopes
```

GPU kernels должны быть tile-local с halo; если kernel зависит от глобального histogram или noise model, использовать reduction pass и явный barrier, а не читать обратно framebuffer на CPU.

## Научные результаты и новые направления

### Demosaic + denoise

Работа [How to Best Combine Demosaicing and Denoising?](https://arxiv.org/abs/2408.06684) показывает, что при умеренном шуме выгоднее сначала demosaic, затем denoise; при высоком шуме лучше частичное CFA denoise, затем demosaic и второй RGB denoise. Это аргумент против обязательного тяжёлого neural joint pipeline на каждом preview: нужен fast classical path и quality/high-noise path.

Работа [Low Cost Edge Sensing](https://arxiv.org/abs/1806.00771) показывает, что edge-guided методы могут приблизиться к качеству сложных алгоритмов при существенно меньшей цене. Это хороший кандидат для `balanced` GPU tier.

Нейросетевые методы (например, [Retinex-RAWMamba](https://arxiv.org/abs/2409.07040)) интересны для low-light, но требуют camera/domain validation, model cache и memory budget. Их следует запускать только по явному quality/AI режиму.

### Lossless RAW cache compression

Свежая работа [RAWIC](https://arxiv.org/abs/2603.28105) предлагает bit-depth adaptive lossless compression Bayer RAW и сообщает среднее преимущество над JPEG XL. Это исследовательское направление, не готовая production-зависимость: модель требует валидации, обучения и лицензирования. В `rrrah` его разумно изучать для L3 disk cache после deterministic raw mosaic; не использовать в P0 hot path.

### Progressive codecs

JPEG XL поддерживает lossless JPEG recompression и progressive/region-oriented design ([JPEG XL whitepaper](https://ds.jpeg.org/whitepapers/jpeg-xl-whitepaper.pdf), [2025 overview](https://arxiv.org/abs/2506.05987)). Для persistent preview/mip cache это интереснее, чем хранить множество независимых JPEG: можно получать coarse preview раньше и уменьшать дисковый объём. Но DNG/JPEG XL support и native decoder availability должны быть проверены на каждой платформе.

### Недавние изменения в зрелых проектах

В darktable 4.4 были переписаны pixelpipe caching strategies и добавлено внутреннее кеширование для OpenCL highlight reconstruction; в darktable 5.6 отдельно подчёркнуто устранение лишних pixelpipe runs при редактировании history ([4.4 release](https://www.darktable.org/2023/06/darktable-4.4.0-released/), [5.6 release](https://www.darktable.org/2026/06/darktable-5.6.0-released/)). Это важная тенденция: современная оптимизация — не только ускорять kernel, но и не запускать его вообще, если входные параметры и ROI не изменились.

Для `rrrah` это означает content-addressed stage keys:

```text
stage_key = hash(raw_fingerprint, roi, tile, mip, edit_subgraph, shader_abi)
```

Изменение exposure не должно инвалидировать entropy decode, demosaic input или metadata. Изменение camera profile инвалидирует только color stages и downstream tiles. Такой selective invalidation даст больший эффект, чем увеличение worker count.

## Что устарело или опасно

### Устаревшие подходы

- Декодировать embedded JPEG и выдавать его за RAW viewer.
- Полностью materialize `RGBA32F` кадр до первого frame.
- Один глобальный mutex вокруг decoder/cache/GPU.
- Синхронный `map → decode → upload → wait` в UI thread.
- Unlimited thread pool и отсутствие backpressure.
- Перегенерировать full-resolution preview на каждое движение slider.
- Хранить cache только по filename/mtime.
- Считать histogram/quality metrics в gamma-encoded sRGB.
- Смешивать fast preview с финальным quality score.
- Использовать FP16 без error budget и golden tests.
- Делать face/AI embeddings обязательными при импорте.

### Практики, которые всё ещё полезны

- Embedded JPEG как мгновенный thumbnail, если явно обозначен как preview.
- Full RAW decode для camera compatibility fallback.
- CPU path для старой/неподдерживаемой GPU.
- Fixed-point/u16 на sensor stage для экономии bandwidth.
- Persistent thumbnail cache и WAL catalog DB.

## Приоритеты для rrrah

### P0: довести до промышленного viewer-а

1. Реальный tiled DNG/strip planner; не materialize весь кадр перед первым visible tile.
2. CR2 sequential lane + row-band postprocess fan-out.
3. Priority scheduler с generation cancellation и bounded credits.
4. CPU tile cache + GPU residency LRU/2Q + staging ring.
5. Metadata-only probe <10 ms для локального SSD; первый visible tile <100–150 ms warm.
6. Fast/balanced/quality tiers с deterministic fallback.
7. Full DNG semantics: linearization, black-level grids, OpcodeList policy, floating point.
8. Cross-platform shader validation Metal/Vulkan/DX12; GPU-limit-aware atlas.
9. Golden RAW corpus с ΔE00/PSNR/SSIM/seam/highlight metrics.
10. Crash-safe cache, malformed-file sandbox, fuzz corpus и memory caps.

### P1: качество и редактор

1. Edge-guided/MHC/RCD quality demosaic.
2. DCP/ICC, CAT16/Bradford, gamut mapping и soft proof.
3. Highlight reconstruction, denoise/sharpen, lens/CA correction.
4. Non-destructive edit DAG, masks и history checkpoints.
5. Export TIFF/PNG/JPEG/AVIF/JXL/DNG с reproducible metadata.
6. Live benchmark HUD и Chrome trace spans без pipeline stalls.

### P2: исследовательские возможности

1. Optional neural low-light/demosaic/denoise models.
2. RAWIC/JXL cache experiments.
3. Multi-frame HDR, focus/blur stacking и panorama.
4. Multi-GPU scheduling только после доказанной single-GPU bottleneck.

## Оценка текущего rrrah

| Область | Статус | Оценка |
|---|---|---:|
| Full CR2 decode без embedded JPEG | Работает | 7/10 |
| Persistent RAW mosaic cache | Работает, sampled fingerprint | 7/10 |
| GPU full-resolution atlas | Работает как prototype | 6/10 |
| Tile residency/prefetch | Архитектура описана, не production | 3/10 |
| DNG advanced semantics | Частично | 3/10 |
| Demosaic quality | Bilinear fast tier | 3/10 |
| Color management | Базовая matrix/WB | 4/10 |
| Editor graph/masks/export | Backlog | 1–2/10 |
| Benchmark harness/statistics | Хороший foundation | 7/10 |
| Security/fuzz/limits | Базовые проверки | 5/10 |

Итог: текущий проект сильнее всего как исследовательский fast RAW viewer foundation. До уровня darktable/RawTherapee по качеству и до Lightroom/Capture One по полноте редактора ещё далеко. Самое ценное направление — не добавлять ещё 100 UI-функций, а завершить P0 tile scheduler, cache residency, DNG semantics и quality tiers. Именно эти четыре слоя определяют измеримую скорость, память и correctness.

## Benchmark contract

Каждый конкурент и каждый `rrrah` commit следует измерять одинаково:

```text
C1: 24 MP Bayer CR2
C2: 52 MP Canon CR2
C3: tiled DNG 100+ MP
C4: floating-point Linear DNG
C5: Fuji X-Trans
C6: noisy high-ISO RAW
C7: 10k-file catalog
```

Регистрировать:

```text
T_probe, T_first_visible, T_full_decode, T_upload, T_first_present
p50/p95/p99, RSS_peak, VRAM_peak, cache hit ratio
frame misses at 60 Hz, CPU/GPU utilization, bytes read/written
ΔE00, PSNR/SSIM, highlight clipping, tile seam error
```

Нельзя сравнивать «время открытия» без фиксации cache state, preview size, quality tier, GPU backend, OS page cache и количества кадров в серии.
