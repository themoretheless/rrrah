# Практическое исследование конкурентов и реальных проблем RAW-софта

Дата исследования: 2026-07-21. Фокус: что фотографы реально считают быстрым,
где ломаются существующие программы и какие решения следует перенести в rrrah.
Источники включают документацию и issue-трекеры проектов, а также обсуждения
pixls.us/Reddit. Форумные сообщения являются наблюдениями пользователей, а не
лабораторными измерениями; численные benchmark-утверждения должны быть
проверены нашим corpus на одинаковом железе.

## Короткий вывод

1. Главный UX-показатель — не throughput экспорта, а время до первого **RAW**
   кадра и скорость перехода между соседними кадрами. Пользователи готовы
   принять background quality refinement, но не blank/working экран.
2. Самый эффективный pre-load — многоуровневый: metadata → видимый preview →
   соседние кадры/tiles → quality render. Полный рендер каталога на import
   блокирует culling и создаёт ощущение «медленного приложения».
3. GPU не является безусловным ускорением. Ошибочные OpenCL/драйверы,
   компиляция kernel при первом запуске и VRAM pressure регулярно дают fallback
   на CPU или даже crash. Нужен capability probe, per-kernel fallback и
   горячее отключение GPU.
4. CR2 lossless-JPEG с зависимым predictor нельзя честно разделить на 20
   независимых декодеров. Параллелятся DNG tiles, restart-marker сегменты и
   postprocess; последовательный entropy lane надо сохранять.
5. Tile processing экономит память, но имеет цену: дополнительные halo,
   synchronization и повторные чтения. Darktable прямо предупреждает, что в
   некоторых модулях tiling может быть до 10x медленнее; его следует включать
   только при memory pressure или для ROI.
6. Качество демозаики видно прежде всего на foliage/тканях/тонких линиях и при
   100% zoom. Быстрый bilinear является preview tier, но его нельзя называть
   production-quality develop.

## Что практически работает у конкурентов

### darktable

Darktable использует несколько pixelpipe-вариантов: thumbnail pipe для
параллельной обработки множества маленьких изображений, укороченный pipe для
интерактивных инструментов и полноценный darkroom/export pipe. Это правильная
идея для rrrah: один универсальный «идеальный» pipeline всегда проигрывает
специализированным latency tiers.

Документация darktable описывает два уровня cache: RAM primary cache и disk
backend для thumbnails/full previews. При маленьком cache без disk backend
появляются повторная генерация thumbnails, flicker и зависание lighttable.
Для массовой генерации предусмотрен отдельный `darktable-generate-cache`, а
не блокировка UI во время импорта.

Для больших изображений используется tile-wise processing с host-memory
лимитом и минимальным размером buffer. Сами разработчики отмечают, что tiling
может быть значительно медленнее (в отдельных модулях до 10x), поэтому его
нельзя применять безусловно.

Практические уроки:

- отделить thumbnail/preview/full-quality pipes;
- хранить disk cache с явной очисткой и версией формата;
- background cache generation с backpressure;
- memory budget должен влиять на tile size и число одновременных задач;
- OpenCL должен иметь CPU fallback и диагностический режим.

Источники: [pixelpipe](https://darktable-org.github.io/dtdocs/en/darkroom/pixelpipe/the-pixelpipe-and-module-order/),
[CPU/GPU/memory](https://docs.darktable.org/usermanual/3.6/en/preferences-settings/cpu-gpu-memory/),
[thumbnail cache](https://docs.darktable.org/usermanual/3.6/en/lighttable/digital-asset-management/thumbnails/),
[tiling/performance](https://docs.darktable.org/usermanual/development/de/special-topics/mem-performance/).

### RawTherapee

RawTherapee имеет отдельную очередь экспорта: тяжёлый batch не должен
отнимать CPU у интерактивного редактора. Количество потоков ограничивается
числом hardware threads и памятью; документация предупреждает, что каждый
дополнительный thread увеличивает RAM, особенно у noise reduction. Это важнее
простого правила «создать 20 workers».

RawTherapee показывает embedded JPEG как быстрый 1:1 preview, но для честного
RAW-viewer это лишь навигационный слой. В community обсуждениях пользователи
хвалят качество AMaZE/RCD и IGV, но жалуются на slow first render и сложность
настройки. На практике полезен явный переключатель `fast/balanced/quality`, а
не скрытая смена алгоритма при zoom.

Демозаика и sharpening должны проверяться на foliage, волосах, ткани и
диагональных линиях: обсуждения pixls.us показывают, что артефакты (zipper,
false color, halos) заметны даже при одинаковом разрешении и особенно после
сильного sharpening.

Источники: [RawPedia threads](https://rawpedia.rawtherapee.com/RawPedia.pdf),
[floating-point engine](https://rawpedia.pixls.us/the_floating_point_engine/),
[demosaic discussion](https://discuss.pixls.us/t/iridient-vs-markenstijn-demosaicing/16923),
[PhotoFlow SIMD benchmark](https://discuss.pixls.us/t/photoflow-optimizations-and-benchmarks/5171).

### RawSpeed и LibRaw

RawSpeed — специализированный C++ decode layer (CR2/NEF/DNG и др.), а не
готовый viewer. Его ценность — оптимизированный entropy decode и camera
specific parsing. LibRaw даёт широкий API и совместимость, но cache, scheduler,
GPU upload и UI остаются обязанностью приложения.

Практический вывод: rrrah должен иметь `RawDecoder` abstraction и corpus
fallback (rawler → RawSpeed/LibRaw adapter), но не смешивать decode benchmark с
качеством develop pipeline.

Для DNG нельзя считать metadata «опциональной косметикой»: отсутствие
OpcodeList/linearization/black-level semantics даёт purple или неверную
экспозицию. Issue RawSpeed #506 показывает реальный случай, где неполная
поддержка DNG opcodes меняет изображение.

Источники: [RawSpeed](https://github.com/darktable-org/rawspeed),
[LibRaw API](https://www.libraw.org/docs/API-overview.html),
[RawSpeed DNG opcode issue](https://github.com/darktable-org/rawspeed/issues/506).

### RapidRAW и современные GPU editors

RapidRAW — ближайший практический аналог по Rust/WGPU/WGSL. Его changelog
показывает, что реальный порядок оптимизаций был таким:

1. thumbnail workers и visible-viewport-only generation;
2. LRU image cache и GPU image cache;
3. separate preview worker/backpressure;
4. ROI rendering при zoom;
5. race fixes при быстрой смене изображения;
6. staging/direct WGPU renderer и triple buffering;
7. только затем сложные masks/AI/lens/noise функции.

Это подтверждает приоритет rrrah: сначала residency/scheduler/cache и
отменяемые задачи, затем расширенный editor graph.

RapidRAW также документирует практические сбои: auto backend может выбрать
нерабочий GPU, Wayland + NVIDIA/WebKit требует environment workaround,
старые/интегрированные GPU могут давать instability/artifacts. Значит, наш
renderer обязан показывать выбранный backend/adapter, иметь safe CPU mode и
проверять shader capabilities до открытия файла.

Источники: [RapidRAW repository](https://github.com/CyberTimon/RapidRAW),
[RapidRAW docs](https://www.getrapidraw.com/docs/),
[Wayland/WebKit issue](https://github.com/CyberTimon/RapidRAW/issues/289).

## Что пользователи ценят в первую очередь

Приоритет подтверждается повторяющимися жалобами в discussions:

1. мгновенная filmstrip/culling навигация;
2. prefetch следующего/предыдущего кадра;
3. histogram и basic exposure/WB без полного rerender;
4. стабильный 100% zoom без blank/flicker;
5. predictable keyboard shortcuts и отмена устаревшего перехода;
6. sidecar edits без изменения RAW;
7. только после этого — сложные AI masks, generative fill и cloud.

Darktable пользователи отдельно жалуются на импорт тысяч DNG без embedded
thumbnail: приложение сканирует и обрабатывает каждый RAW, что занимает часы.
Другие отмечают, что FastRawViewer выигрывает culling именно prefetch-ом.
Наше требование: embedded JPEG можно использовать в **очень раннем** catalog
thumbnail tier, но никогда не подменять им RAW при открытии/редактировании.

Источники: [slow import discussion](https://www.reddit.com/r/DarkTable/comments/1ufe0z1/importing_speeds/),
[prefetch/culling discussion](https://www.reddit.com/r/DarkTable/comments/1up85wm/rating_label_pics/),
[pixls cache troubleshooting](https://discuss.pixls.us/t/darktable-troubleshooting-cache-speed/44858),
[darktable thumbnail zoom issue](https://discuss.pixls.us/t/anyone-else-having-problems-with-zoomable-table-mode-in-dt-3-2-1/19716).

## Типовые failure modes и контрмеры

| Failure mode | Наблюдение в OSS | Контрмера rrrah |
|---|---|---|
| GPU startup crash | OpenCL/driver/library mismatch, startup segfault | capability probe, shader smoke test, CPU fallback, `--disable-gpu` |
| first-run kernel compile stall | OpenCL/ROCm compilation; AMD first inference slow | persistent pipeline cache, asynchronous warmup, report compile time |
| VRAM exhaustion | mipmap/full preview cache and tiling pressure | byte budget, LRU residency, watermark eviction, no giant atlas |
| stale/corrupt cache | cache survives app upgrades or network disconnect | ABI+semantic key, checksum, atomic commit, bounded load |
| UI starvation | thumbnail generation on import and batch export | priority queues, cooperative cancellation, separate export queue |
| wrong DNG colors | missing OpcodeList/linearization/black semantics | capability report; reject or mark degraded, never silent JPEG fallback |
| tile seams | local CFA phase or insufficient halo | global sensor coordinates, halo validator, seam golden tests |
| too many workers | RAM grows with noise/demosaic buffers | memory-aware worker admission, `workers <= min(cpu, budget/working_set)` |
| network/NAS stalls | disk cache/RAW on slow NAS blocks preview | local metadata/thumbnail cache, async I/O, prefetch only with bandwidth budget |
| unsafe malformed RAW | parser allocation/cycle/offset bugs | sandbox worker, size/offset limits, fuzz corpus, kill/restart |

OpenCL troubleshooting documentation также отмечает, что CPU fallback может
быть быстрее эмулированного OpenCL, поэтому «GPU enabled» нельзя считать
метрикой производительности без фактического timing.

Источники: [darktable OpenCL problems](https://darktable-org.github.io/dtdocs/en/special-topics/opencl/problems-solutions/),
[OpenCL fallback](https://darktable-org.github.io/dtdocs/en/special-topics/opencl/still-doesnt-work/),
[RapidRAW backend guidance](https://github.com/CyberTimon/RapidRAW#common-problems).

## Устаревшие или вредные подходы

- **Всегда показывать embedded JPEG.** Быстро, но не RAW и вводит в заблуждение
  по цвету/экспозиции.
- **Запускать полный quality pipeline до первого кадра.** Хорошо для export,
  плохо для culling; нужен progressive refinement.
- **Безлимитный thread pool.** Приводит к memory pressure, swap и худшему
  p95, особенно в denoise.
- **Одна гигантская GPU texture.** Ломается на лимитах 8192/16384 и съедает
  VRAM; нужен tiled residency.
- **OpenCL как единственный GPU путь.** Слишком много driver variance; оставляем
  backend adapter, но не связываем correctness с GPU.
- **Только count-based LRU.** Большие full-res entries вытесняют полезные
  thumbnails; нужен weighted admission (2Q/TinyLFU).
- **Тихий fallback на JPEG при ошибке RAW.** Скрывает DNG/decoder bugs и
  повреждает доверие к редактору.
- **Сравнение только среднего времени.** Пользователь чувствует p95/p99,
  dropped frames и stale result; benchmark обязан сохранять распределения.

## Приоритеты для rrrah (P0 → P2)

### P0: довести core до конкурентоспособного viewer

1. `TilePlan` для DNG strips/tiles и CR2 sequential lane.
2. Priority scheduler P0 visible/P1 adjacent/P2 next frame/P3 idle.
3. Generation cancellation и stale-publish counter = 0.
4. CPU weighted cache + GPU residency LRU + staging ring.
5. Metadata-only open и catalog thumbnails с bounded workers.
6. Full BLAKE3/ABI/semantic cache keys и cache pressure controller.
7. GPU capability probe, pipeline cache, CPU fallback, visible diagnostics.
8. DNG linearization, black-level grids, OpcodeList support or explicit
   degraded status.
9. Fast/balanced/quality demosaic tiers with golden quality corpus.
10. Live benchmark HUD: first-frame, queue depth, cache hit, VRAM/RSS,
    dropped frames, backend.

### P1: quality parity

- RCD/MHC/AMaZE (или validated equivalents), X-Trans;
- ICC/DCP, camera matrices, highlight reconstruction, noise profiles;
- lensfun distortion/TCA/vignette;
- non-destructive edit graph and sidecar recovery;
- export correctness and metadata preservation.

### P2: optional differentiators

- AI masks/denoise, panorama/stacking, cloud/automation, generative tools.

## Как проверять «доведено до идеала»

Feature считается готовой только если одновременно выполняется:

```text
functional corpus pass
+ deterministic CPU reference
+ GPU/CPU image diff within tolerance
+ p50/p95/p99 benchmark gate
+ bounded RSS/VRAM under stress
+ cancellation and crash recovery test
+ documented degraded path
+ no silent JPEG substitution
```

Минимальный release corpus: CR2 с/без restart markers, DNG tiled/uncompressed/
lossless-JPEG, OpcodeList 1/2/3, X-Trans, dual-gain, 12/14/16-bit, huge
dimensions, NAS latency, corrupt/truncated files. Benchmark matrix должна
разделять `first RAW frame`, `warm open`, `next-frame`, `100% zoom`, `final
quality export` и `memory pressure`.

## Рекомендация архитектуре

```text
Probe/metadata (CPU, <20 ms target)
  → scheduler (priority + generation)
  → decode lanes (CR2 serial / DNG tile parallel)
  → immutable CPU tile cache
  → GPU residency + staging ring
  → fast preview pipe
  → asynchronous quality pipe
  → persistent result cache/export
```

Главная инновация для rrrah — не «запустить больше потоков», а уменьшить
последовательную часть и не считать невидимые данные. В терминах Amdahl:

\[
S(P)=\frac{1}{s+(1-s)/P},
\]

где `s` для CR2 entropy остаётся большим, а для DNG tile decode уменьшается
до scheduler/I/O overhead. Поэтому 20 agents полезны для design review, но
runtime должен иметь bounded workers, data locality и измеряемый ROI.
