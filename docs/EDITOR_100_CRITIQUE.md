# Критический аудит следующего пакета из 100 функций

Этот документ не добавляет обещаний о готовности функций. Он задаёт условия,
при которых расширенный редактор можно развивать без потери скорости открытия
RAW, качества цвета и предсказуемого потребления памяти.

## Итог десяти критических ролей

| Роль | Риск | Решение |
|---|---|---|
| Архитектор систем | «100 функций» превращаются в монолитный pixel-pipe | capability-пакеты с портами `Decode`, `TileStore`, `Color`, `EditGraph`, `Export`; optional функции не должны попадать в hot path |
| Архитектор данных | metadata, edits и cache имеют разные жизненные циклы | immutable RAW identity + versioned edit document + derived cache; никакой записи в исходный RAW |
| CPU/perf инженер | CR2 entropy stream имеет последовательный predictor state | один sequential lane, SIMD/postprocess fan-out; workers не дублируют поток |
| GPU/perf инженер | full-frame texture недоступна на GPU с лимитом 8192 | tile residency, halo, bounded staging ring, eviction только вне viewport |
| Математик цвета | визуально быстрый shader может нарушить scene-linear инварианты | reference CPU oracle в `f64`, runtime `f32`, ΔE/PSNR gates до оптимизации |
| RAW/DNG алгоритмик | DNG OpcodeList, black grids и LinearRaw нельзя молча игнорировать | capability negotiation: `supported`, `degraded`, `rejected`; каждый режим маркируется в telemetry |
| Учёный по изображению | одна метрика SSIM не гарантирует photographic quality | ΔE00, neutral gray, clipping, MTF, zipper/moire, seam и round-trip checks |
| Security/privacy инженер | malicious RAW, MakerNotes и sidecar могут быть недоверенными | bounded parser, worker process, quotas, no network by default, redact paths/EXIF in telemetry |
| QA/fuzz инженер | combinatorial explosion форматов и профилей | corpus tiers, property tests, differential oracle, fuzz budget и quarantine queue |
| Release/performance tester | сравнение с OSS смешивает разные quality tiers | одинаковые fixture/cache/build/quality, p50/p95/p99 и отдельные first-frame/final-export scores |

## Capability-пакеты и границы

Функции должны поставляться пакетами, а не 100 независимыми переключателями:

```text
P0 viewer: probe → RAW decode → tile scheduler → cache → fast demosaic → export baseline
P1 pro-color: black grids → DCP/ICC → tone/gamut → quality demosaic
P1 workflow: catalog → sidecars → non-destructive history → batch export
P2 scientific: HDR PQ/HLG → spectral/four-color → optical-flow alignment
P2 plugin: external denoise, lens models, AI masks, vendor GPU backends
```

Правило: P1/P2 не могут увеличивать `T_first_frame` P0 более чем на 5% при
отключённом модуле. Их библиотеки загружаются лениво, а pipeline получает
явный feature bitset. Не включать одновременно MHC/RCD/AMaZE, HDR и heavy
denoise в обязательный путь первого кадра.

## Acceptance gates

### Correctness и quality

* `no NaN/Inf` после каждого stage, finite metadata и проверенный determinant.
* Tiled и monolithic reference дают `max_abs_error <= 1 LSB` для integer path;
  FP16 допускается только при заранее зафиксированном ΔE00.
* Neutral gray: ΔE00 p95 ≤ 0.5; camera chart: median ≤ 1.0, max ≤ 3.0.
* Highlight reconstruction не увеличивает clipped area более чем на 0.5 pp.
* Demosaic tier имеет отдельные gates: fast (latency), balanced (SSIM/false
  color), quality (MTF/ΔE); нельзя сравнивать их одной цифрой.
* Export round-trip сохраняет orientation, crop, ICC/EXIF/XMP согласно policy.

### Latency и throughput

На одной машине, одном fixture и фиксированном cache state:

```text
metadata-only open p95             ≤ 20 ms
warm visible tile p95              ≤ 150 ms
cold 50–70 MiB CR2 decode p95     ≤ 500 ms (exploratory target)
first interactive frame after tile ≤ 100 ms
steady pan/zoom p95                ≤ 16.7 ms (60 Hz)
cancel-to-new-generation p95       ≤ 20 ms
```

`T_open` и `T_warm` должны раскладываться на probe/decode/cache/upload/present.
Для CR2 benchmark отдельно показывает serial fraction; для tiled DNG — scaling
`workers=1,2,4,8` до насыщения storage/memory/GPU. Ни один результат `skip` не
считается speed result.

### Memory, thermal и energy

```text
raw_tile_bytes = (T + 2*halo)² * bytes_per_sample
rgb_tile_bytes = (T + 2*halo)² * channels * storage_bytes
resident_bytes = Σ pinned + Σ in_flight + scheduler_overhead
```

* RSS и VRAM обязаны оставаться в заданном budget ±5%; при pressure система
  должна сначала вытеснять prefetch, затем mips, но никогда не текущий viewport.
* In-flight upload bytes ≤ `2 × staging_ring_bytes`; allocator hot loop = 0.
* Benchmark фиксирует thermal state, power mode и governor; длительные тесты
  имеют warmup и steady-state, иначе turbo burst выдаёт ложное преимущество.
* `peak_rss`, `gpu_mem`, allocations, queue depth и dropped frames обязательны
  в JSONL, даже если функция отключена.

## Что должно оставаться optional

До появления reference corpus и профиля качества не включать в core:

* AI denoise, super-resolution, generative fill и cloud inference;
* vendor-specific CUDA/Metal/HIP backends;
* HDR PQ/HLG и 32-bit float export;
* X-Trans/four-color/spectral CFA, если нет camera fixtures;
* optical-flow alignment и multi-frame merge;
* геопоиск, cloud sync, facial recognition и similarity indexing;
* MakerNotes write-back (только read-only или sidecar);
* JPEG XL/AVIF/JXL, пока нет стабильных encoder licenses и round-trip tests.

Optional не означает «не тестировать»: capability должен иметь golden output,
memory budget, security review и explicit `unsupported/degraded` telemetry.

## Security, privacy и лицензии

* RAW, ICC, DCP, XMP и MakerNotes считаются untrusted input. Все lengths,
  offsets, tile counts и decompression ratios проверяются до allocation.
* Decoder worker можно завершить по quota/time limit; UI процесс не должен
  падать от panic/abort декодера. Cache не исполняет metadata и не раскрывает
  абсолютный путь в общий telemetry export.
* По умолчанию нет сетевого доступа; геоданные и серийные номера камер
  редактируются/хешируются в benchmark artifacts.
* Для каждого backend фиксируются SPDX-лицензии и notices. LGPL/ GPL/ patent
  obligations нельзя скрывать за feature flag; proprietary camera profiles
  хранятся отдельно от исходников.

## Open-source comparison policy

Сравнивать можно только одинаковые слои:

```text
RawSpeed/LibRaw       → entropy + metadata decode
darktable/RawTherapee → first preview и final-quality develop отдельно
RapidRAW              → GPU/editor latency при одинаковом quality tier
```

Для каждой системы фиксируются commit, compiler flags, CPU/GPU, fixture hash,
cache state, worker count, color profile и output bit depth. Не переносить
цифры из чужих README в локальный рейтинг. Отдельно публиковать:

* first RAW frame latency;
* warm reopen latency;
* sustained frames/s;
* final export throughput;
* peak RSS/VRAM;
* ΔE00/PSNR/SSIM/MTF.

## Stop conditions и triage

Работа над новой функцией останавливается, если выполняется любое условие:

1. regression gate превышен два последовательных запуска;
2. quality oracle не определён или fixture отсутствует;
3. memory budget неизвестен либо allocation зависит от входных offsets;
4. модуль меняет P0 first-frame без feature flag;
5. license/security review не завершён.

Тогда функция получает `blocked` или `experimental`, но не попадает в release
benchmark. Это защищает проект от номинального «ещё 100 функций» без измеримой
готовности.

