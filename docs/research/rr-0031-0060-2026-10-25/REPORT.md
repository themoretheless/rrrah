# RR-0031–RR-0060: проверка гипотез и внедрение

**Исторический первый проход.** RR-0056 впоследствии закрыта: [ограниченное освобождение payloads](../rr-0056-bounded-2026-10-25/REPORT.md). Текущий итог диапазона: 4 verified, 26 rejected; ниже сохранены результаты до продолжения.

Дата: 2026-10-25. Проверен текущий dirty checkout; существующие изменения сохранены. VoiceOver, физический HDR, CUDA/NVIDIA, Windows и другое оборудование исключены по указанию пользователя. Системные настройки не менялись.

**Итог: 3 verified, 26 rejected, 1 partial.** Внедрены изменения RR-0034, RR-0036, RR-0055. RR-0056 требует безопасного по памяти продолжения. Это не 30 внедрённых оптимизаций.

`rejected` означает отклонение конкретного предположения исходного статического аудита. Где код уже делает exact reserve или коллекция владеет результатом, повторное измерение не превращает это в новую оптимизацию. Для таких пунктов сохранён анализ и выполнены correctness suites, но полный первоначальный allocation/RSS/performance acceptance не объявлен пройденным. Для опасных кандидатов сохранены отрицательные воспроизводимые проверки. Исходные acceptance/proposed_change и evidence_status сохранены в реестре; execution отражает отдельный результат проверки.

Машинные результаты: [qualification.json](qualification.json), [текущие основания](source-review.json), [исходный контекст этого прохода](source-audit.json). Команды и ограничения сохранены в журналах. Перегенерация Markdown реестра не изменяет исходные критерии задач.

## Внедрённые изменения

### RR-0034: deadline для внешних команд X3F qualification

`scripts/qualify-x3f-sd10.py` использовал синхронный subprocess без локального deadline. Теперь все команды проходят через общий process-tree watchdog (`qualification_command.run_logged` → `qualify-viewer-storage-gpu.run_suite`). Добавлен `--timeout` в секундах, default=1800 **на команду**, не на весь qualification run. Положительное конечное значение проверяется до fixture I/O. Timeout и ненулевой exit сохраняются в командном журнале и завершаются диагностической ошибкой; timeout не становится success даже если родитель успел вернуть 0. Вывод объединяет stdout/stderr как ранее. Публичные аргументы и report schema сохранены, флаг additive.

На локальном POSIX проверены нормальный процесс, exit=7 со stderr и зависающий процесс с SIGTERM-resistant потомком; после deadline потомок не выполняется. Watchdog имеет дополнительный срок завершения дерева после deadline. Принудительно отсоединившиеся от process group потомки не входят в доказанную область. Полный прогон pinned SD10/SD14 camera/oracle/Metal этим изменением не повторялся; Windows taskkill путь не квалифицирован.

### RR-0036: DICOM tag membership без дерева

До изменения strict-order decode одновременно хранил предыдущий tag и BTreeSet всех tags. При строго возрастающем порядке повтор и убывание определяются одним previous; четыре обязательных Part 10 meta tag учитываются битовой маской. Дубликаты/порядок, обязательные поля, pixel output, отмена и отказ по бюджету сохранены. Проверены все последовательности длины 3 из набора 8 граничных tags против прежнего правила и отдельные случаи пропущенных обязательных meta tags. Старый BTreeSet путь оставлен только под cfg(test) с thread-local переключением для сравнения в одном бинарнике; публичный API не менялся.

Benchmark — полный CPU decode явного float32 DICOM из fixture, дополненного 0/64/4096 упорядоченными private OB tags. Чередование small/large/small/medium, 120 decode на вариант, 3 warmup и 10 чередующихся A/A и A/B пар; две сохранённые серии. Перед измерением проверяется совпадение результата старого/нового пути; managed peak=96 B, final=0; tight budget=1 отвергается с final=0. Median candidate/baseline total ratios: **0.1318 / 0.1284** (в каждой серии 10/10 выигрышей), A/A **1.0018 / 0.9982**. Это измерение маленького pixel payload с большим metadata; ускорение обычного открытия, всего viewer или любого DICOM из него не следует.

[Серия 1](dicom-pairs-1.log), [серия 2](dicom-pairs-2.log), [отношения пар](dicom-summary.json). Предварительный запуск [dicom-pairs-preliminary.log](dicom-pairs-preliminary.log) был приостановлен во время внешнего окна квалификации и исключён; файл не удалён. Во время принятых CPU пар собственные тяжёлые correctness suites не запускались; A/A оставлен как контроль шума. Это локальные измерения, без обещания изоляции всей системы.

Allocation probes выполнялись отдельными процессами с 300 decode и DYLD interpose libc allocation APIs. Это process-wide call counts с общим setup/test overhead, **не число только Rust heap allocations и не число живых bytes**. `wait4` даёт per-child macOS peak RSS в bytes, отдельно от managed counter. Инструментированные прогоны исключены из timing. Сохранены C исходник, [драйвер](probe-process.py), SHA бинарников и 10 чередующихся process pairs для каждого сценария.

| Сценарий | Вариант | malloc / calloc / realloc / aligned, median | Peak RSS, median B |
|---|---|---|---|
| dicom | legacy | 54140 / 169 / 39 / 7 | 3104768 |
| dicom | candidate | 1190 / 169 / 39 / 7 | 2965504 |
| mng | legacy | 342 / 169 / 29 / 7 | 5931008 |
| mng | candidate | 343 / 169 / 30 / 7 | 5980160 |
| webp | legacy | 282 / 169 / 30 / 7 | 2736128 |
| webp | candidate | 283 / 169 / 31 / 7 | 2760704 |

MNG/WebP rows относятся к отрицательным вариантам предварительного выделения 4096 кадров, а не к внедрённому ускорению. Heap capacity нельзя приравнивать к RSS: страницы не обязаны быть затронуты, RSS содержит библиотеки/stack/setup и имеет шум. Основное основание отказа здесь — расход до валидации header без допуска бюджета; небольшое изменение RSS не интерпретируется как точный размер буфера.

### RR-0055: целая публикация JSON отчёта

Финальный `--json-out` читает следующий запуск `--baseline-json` и другие consumers. Прямая запись могла усечь старый документ до завершения новой записи. Теперь уникальный tempfile создаётся в том же каталоге, закрывается после flush/fsync, затем публикуется os.replace. Сохраняются прежние file mode и symlink-target semantics; новые файлы подчиняются umask. На POSIX постоянный per-destination flock inode сериализует cooperating writers, ожидание ограничено 5 с. Следующая публикация под lock удаляет только managed orphan names с точным 32-hex суффиксом; проверены имена с glob-метасимволами.

Реальные SIGKILL перед/после replace оставляют старый/новый целый JSON. Конкурентный reader при 49 публикациях не увидел частичный документ. Проверены replace failure, сохранность mode/старого JSON, bounded lock wait и запрет удаления temp активного writer. После SIGKILL tempfile может остаться **до следующего writer**; постоянный `.lock` оставлен намеренно. Это локальная process-interruption atomicity, не power-loss durability (directory fsync не добавлялся), не NFS/Windows qualification и не защита от стороннего writer, игнорирующего протокол.

[Новые Python regressions](python-regressions.log): 8 passed. [Существующие bench-report tests](bench-report-existing-tests.log): 7 passed.

## Отрицательные проверки

| ID | Кандидат | Наблюдение |
|---|---|---|
| RR-0033 | reserve PLY offsets по недоверенному объявлению | invalid body, managed peak 1151→4001151 B, final=0 |
| RR-0038 | reserve MNG 4096 frames до проверки header | capacity 0→4096, requested heap 0→819200 B; та же ошибка; managed peak=0 |
| RR-0045 | второй 16 KiB stream buffer поверх PLY scalar blocks | managed peak 2900→19284 B; прежний tight budget пропускает baseline и отвергает буфер |
| RR-0049 | reserve всех CacheKey prefetch plan | при cache count limit=0 и 4096 paths requested heap 0→131072 B до первого decode; managed final=0 |
| RR-0051 | reserve WebP 4096 frames до проверки header | capacity 0→4096, requested heap 0→163840 B; та же ошибка; managed peak=0 |
| RR-0053 | второй 16 KiB stream buffer поверх OBJ blocks | managed peak 1039→17423 B; tight admission нарушен |

Эти кандидаты присутствуют только под cfg(test), production-путь сохранён. Для OBJ/PLY проверены roundtrip/wire bytes, short/Interrupted reads, framing с NEXT sentinel, усечение на нескольких позициях, ошибка отмены после 24 B и corrupt magic; credits возвращаются в 0. Это memory-stream/admission проверки, не замер cold/warm диска. MNG/WebP считаются структурными отказами, не доказательством нулевой heap памяти всего decode.

## Воспроизведение проверок

```sh
python3 scripts/test-improvements-31-60.py
python3 scripts/test_bench.py
python3 docs/research/rr-0031-0060-2026-10-25/run-native-checks.py
python3 docs/research/rr-0031-0060-2026-10-25/run-dedup-checks.py
cargo test -p rrrah-scientific --lib dicom::rr36_qualification::full_decode_tag_set_pairs -- --ignored --show-output --test-threads=1
cargo test -p rrrah-cache --lib rr56_qualification::ttl_release_outside_lock_pairs -- --ignored --show-output --test-threads=1
cc -dynamiclib -O2 docs/research/rr-0031-0060-2026-10-25/allocation-calls.c -o /tmp/rrrah-31-60-allocation-calls.dylib
python3 docs/research/rr-0031-0060-2026-10-25/probe-process.py
RRRAH_PROBE_NAME=mng python3 docs/research/rr-0031-0060-2026-10-25/probe-process.py
RRRAH_PROBE_NAME=webp python3 docs/research/rr-0031-0060-2026-10-25/probe-process.py
```

Перед allocation probe должны существовать соответствующие native binaries и журналы Running unittests: драйвер сохраняет SHA и не скачивает fixtures. Повторный запуск этих исследовательских драйверов перезаписывает одноимённые журналы; для независимого повторения сначала сохраните текущую папку результатов. При пересборке native бинарника нужно обновить журнал его запуска, иначе это новая идентичность измерения.

## RR-0056: подтверждённое ожидание, реализация не принята

TTL maintenance действительно уничтожает StoredPayload под общим mutex, а Drop SwapHandle может удалить файл. В [benchmark](swap-ttl-pairs.log) используются реальные SwapStore файлы (8×32 B), истёкшие TTL и 1/2/4 читателя, синхронно ожидающих index; 3 warmup, затем 10 alternating A/A и A/B пар по 10 rounds. Candidate вызывает drain_expired, отпускает mutex, затем уничтожает payloads. Проверено: index пуст после публикации, внешний held handle остаётся читаемым, store usage=1 object/32 B до release и 0/0 после него.

Median p95 wait ratios candidate/baseline: 0.0335 / 0.0271 / 0.0455; A/A: 0.871 / 0.820 / 0.985. Выигрыш синхронизированных readers большой, но это не production throughput. Дополнительная capacity Vec=256 B на 8 entries, растёт O(expired count) и не имеет admission. Cleanup p95 ratios 1.072 / 1.016 / 0.835 с нестабильными хвостами, в том числе отдельными длинными candidate samples; raw samples сохранены в полном виде. Поэтому production worker пока не изменён; **partial**. Следующий шаг — bounded extraction/release protocol без произвольного дополнительного heap и проверка больших batches, worker put/get, отмены/ошибок, lease/pin/quota и парные замеры. Нельзя закрыть задачу только по уменьшению времени mutex.

## Проверки по подсистемам

Все команды перечислены в [native-results.json](native-results.json) и соответствующих `*-tests.log`. Watchdog: 240 с на основную suite, 600 с на feature-enabled dedup run. Ни ignored tests, ни zero-test runs не являются проверкой заявленной функциональности. Основные тесты CPU; Metal/device tests не запускались.

| Suite | Passed | Failed | Ignored | Квалифицирован ненулевой набор |
|---|---:|---:|---:|---|
| texture | 96 | 0 | 3 | да |
| mesh | 57 | 0 | 0 | да |
| scientific | 58 | 0 | 10 | да |
| develop | 102 | 0 | 2 | да |
| vfx | 52 | 0 | 0 | да |
| layered | 78 | 0 | 2 | да |
| animation | 36 | 0 | 1 | да |
| cr2 | 10 | 0 | 0 | да |
| mng | 16 | 0 | 0 | да |
| heif | 8 | 0 | 0 | да |
| wmf | 7 | 0 | 0 | да |
| pdf | 46 | 0 | 1 | да |
| cache-swap | 30 | 0 | 1 | да |
| dedup-model | 0 | 0 | 0 | нет |
| dedup-gradient | 9 | 0 | 2 | да |
| dedup-anchor | 0 | 0 | 0 | нет |
| dedup-collection | 0 | 0 | 0 | нет |
| gallery | 53 | 0 | 17 | да |

Decode-feature dedup run: exit=0, timed_out=False; [журнал](dedup-decode-tests.log), [команда/результаты](dedup-decode-results.json).

| Integration target (в порядке фактического выполнения) | Passed | Failed | Ignored |
|---|---:|---:|---:|
| anchor_rank | 5 | 0 | 0 |
| anchor_rank_file | 3 | 0 | 0 |
| bank_partition | 6 | 0 | 0 |
| gradient | 9 | 0 | 0 |
| gradient_collection | 1 | 0 | 0 |
| gradient_distinct_locations | 4 | 0 | 0 |
| local_collection | 20 | 0 | 0 |
| model_compare | 47 | 0 | 0 |
| model_projected_tiles | 8 | 0 | 0 |
| model_tiles | 21 | 0 | 0 |
| opposite_native | 6 | 0 | 0 |

Нулевые default-feature `dedup-model`, `dedup-anchor`, `dedup-collection` оставлены как диагностические попытки и исключены из coverage. Они заменяются отдельными integration targets с `--features decode`; это CPU providers/fixtures, без CUDA/LoFTR hardware claim. Native suites подтверждают указанные конечные наборы, не универсальность всех форматов, бюджетов или публичных API.

## Индивидуальные результаты

### RR-0031 — rejected

[crates/rrrah-texture/src/pvr.rs:167](../../../crates/rrrah-texture/src/pvr.rs#L167), `decode`. PVR сначала считает metadata records, затем try_reserve_exact(records); reservation корректируется по фактической capacity. Повторный рост этой коллекции отсутствует; возвращаемые metadata принадлежат результату.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [texture-tests.log](texture-tests.log).

### RR-0032 — rejected

[crates/rrrah-vector/src/wmf.rs:61](../../../crates/rrrah-vector/src/wmf.rs#L61), `decode`. WMF object table уже создаётся vec![None; count] после ограничения count≤4096. Предлагаемое точное резервирование дублирует действующий путь; смена владельца/pool потребовала бы отдельной API-гипотезы.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [wmf-tests.log](wmf-tests.log).

### RR-0033 — rejected

[crates/rrrah-mesh/src/ply.rs:470](../../../crates/rrrah-mesh/src/ply.rs#L470), `decode_bytes_budgeted`. Проверен кандидат reserve(element.count+1) для PLY list offsets. На усечённом теле с объявлением 1 000 000 элементов пик managed bytes вырос 1151→4001151 при одинаковом отказе и final=0. Текущий постепенный рост оставлен.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [mesh-tests.log](mesh-tests.log).

### RR-0034 — verified

[scripts/qualify-x3f-sd10.py:85](../../../scripts/qualify-x3f-sd10.py#L85), `main`. Все внешние команды X3F qualification переведены на общий watchdog дерева процессов. Добавлен --timeout (по умолчанию 1800 с на команду), запрещены неположительные/NaN/Inf. Проверены успешный exit, exit=7 со stderr и зависший SIGTERM-resistant потомок; timeout не считается успехом.

Границы: Проверен локальный POSIX watchdog и CLI. Полный camera/oracle/Metal прогон X3F и Windows process cleanup этим изменением повторно не квалифицированы; настройки VoiceOver не менялись.

Проверки: [source-review.json](source-review.json), [python-regressions.log](python-regressions.log), [x3f-help.log](x3f-help.log).

### RR-0035 — rejected

[crates/rrrah-develop/src/raster.rs:771](../../../crates/rrrah-develop/src/raster.rs#L771), `to_linear_srgb_with_budget_and_cancel`. Выход linear sRGB уже имеет try_reserve_exact(count), фактическую capacity и credit; LUT ограничен и допускает scalar fallback. Выходной Vec удерживается raster, а не является временным scratch.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [develop-tests.log](develop-tests.log).

### RR-0036 — verified

[crates/rrrah-scientific/src/dicom.rs:70](../../../crates/rrrah-scientific/src/dicom.rs#L70), `decode`. Убрано BTreeSet всех DICOM tags: строгий порядок проверяет previous, четыре обязательных meta tag — битовая маска. Совпадают ошибки/пиксели и бюджет. Две серии по 10 чередующихся пар: total ratio 0.132/0.128 при A/A≈1; malloc 54140→1190 за 300 декодирований в отдельном allocation probe.

Границы: Выигрыш относится к CPU full-decode корпусу с 0/64/4096 private tags и малым pixel payload, не ко всем DICOM/просмотрщику. Managed peak=96 B, per-process RSS измерен отдельно. API, допустимые форматы и pixel output сохранены; физический HDR не проверялся.

Проверки: [source-review.json](source-review.json), [scientific-tests.log](scientific-tests.log), [dicom-pairs-1.log](dicom-pairs-1.log), [dicom-pairs-2.log](dicom-pairs-2.log), [dicom-summary.json](dicom-summary.json), [dicom-allocation-results.json](dicom-allocation-results.json).

### RR-0037 — rejected

[crates/rrrah-vfx/src/dpx.rs:111](../../../crates/rrrah-vfx/src/dpx.rs#L111), `decode`. Обе DPX output ветви уже заранее резервируют полное количество samples и учитывают capacity в credit. Коллекция становится владельцем выходных пикселей; повторного роста отмеченного Vec нет.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [vfx-tests.log](vfx-tests.log).

### RR-0038 — rejected

[crates/rrrah-decode/src/mng.rs:57](../../../crates/rrrah-decode/src/mng.rs#L57), `decode`. MNG: предварительный reserve 4096 кадров на одном 8-byte signature выделяет 819200 B вместо 0, хотя header отвергается одинаково. Managed counters=0 не покрывают этот heap. Сохранён отрицательный regression и отдельные allocation/RSS пробы.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [mng-tests.log](mng-tests.log), [mng-allocation-results.json](mng-allocation-results.json).

### RR-0039 — rejected

[crates/rrrah-texture/src/ktx.rs:61](../../../crates/rrrah-texture/src/ktx.rs#L61), `decode`. KTX заранее вычисляет output/level sizes, резервирует выход точно и проверяет бюджет. Указанный decode anchor не доказывает временный growing scratch; новое предварительное резервирование повторяет существующее.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [texture-tests.log](texture-tests.log).

### RR-0040 — rejected

[crates/rrrah-layered/src/psd.rs:212](../../../crates/rrrah-layered/src/psd.rs#L212), `decode`. PSD raw planes заимствованы из входа; compressed planes выделяются по известному total, RGBA output сразу получает pixels*4 capacity. Эти выделения удерживают данные результата, не независимый повторно используемый scratch.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [layered-tests.log](layered-tests.log).

### RR-0041 — rejected

[crates/rrrah-dedup/src/model_banks.rs:355](../../../crates/rrrah-dedup/src/model_banks.rs#L355), `search_model_banks_pixel_selected`. Pixel-selected model banks используют фиксированные 25 slots и предварительно рассчитанные/допущенные размеры. Повторный рост отмеченного массива отсутствует; перераспределение кредитов не даёт доказанного выигрыша.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [dedup-decode-tests.log](dedup-decode-tests.log).

### RR-0042 — rejected

[crates/rrrah-dedup/src/gradient.rs:1144](../../../crates/rrrah-dedup/src/gradient.rs#L1144), `match_gradients_selected_with_scratch`. Gradient matcher получает явный scratch budget, заранее выделяет forward/reverse, учитывает distance/runners. Наличие Vec::new само по себе не доказывает рост: границы и capacity заданы до заполнения.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [dedup-gradient-tests.log](dedup-gradient-tests.log), [dedup-decode-tests.log](dedup-decode-tests.log).

### RR-0043 — rejected

[crates/rrrah-dedup/src/anchor_rank.rs:141](../../../crates/rrrah-dedup/src/anchor_rank.rs#L141), `compare_anchor_rank`. Anchor-rank aw/bw резервируются по числу points; их reservations переходят в сохраняемое evidence. Пул потребовал бы изменить владельца результата, точное резервирование уже реализовано.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [dedup-decode-tests.log](dedup-decode-tests.log).

### RR-0044 — rejected

[crates/rrrah-animation/src/jng.rs:236](../../../crates/rrrah-animation/src/jng.rs#L236), `decode`. JNG z.write_all пишет 2/4 байта локального metadata carrier 1×1 в память через ZlibEncoder. Файловый backend здесь отсутствует; объединение disk I/O и cold/warm benchmark к отмеченному вызову неприменимы.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [animation-tests.log](animation-tests.log).

### RR-0045 — rejected

[crates/rrrah-mesh/src/ply_swap.rs:475](../../../crates/rrrah-mesh/src/ply_swap.rs#L475), `read_ply_swap_payload`. PLY swap уже читает scalar blocks по 16 KiB. Дополнительный BufReader повышает managed peak 2900→19284 B и не проходит прежний tight budget. Проверены roundtrip, framing NEXT, короткие/Interrupted чтения, усечение, отмена, corrupt magic; final=0.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [mesh-tests.log](mesh-tests.log).

### RR-0046 — rejected

[crates/rrrah-raw/src/camtiff/cr2.rs:367](../../../crates/rrrah-raw/src/camtiff/cr2.rs#L367), `decode_pixels`. CR2 decode_pixels заранее резервирует total и передаёт выделение в sensor raster с credit. Отмеченный Vec — выход, а не забытый growing scratch; предлагаемое exact reserve уже есть.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [cr2-tests.log](cr2-tests.log).

### RR-0047 — rejected

[crates/rrrah-dedup/src/local_collection.rs:2769](../../../crates/rrrah-dedup/src/local_collection.rs#L2769), `scan_gradient_collection_with_confirmation_selected`. Gradient collection заранее резервирует entries и owners по числу файлов и принимает фактическую capacity в бюджет. Время жизни owners требуется для сохранения результатов; общий пул не является локальной заменой Vec.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [dedup-decode-tests.log](dedup-decode-tests.log).

### RR-0048 — rejected

[crates/rrrah-scientific/src/fits.rs:407](../../../crates/rrrah-scientific/src/fits.rs#L407), `decode`. decode_fits — ограниченная read/decode обёртка. parse_header делает count pass, заимствует cards из bytes и резервирует budget buffer(count); metadata упорядочены. Предположение о неизвестном growing temporary не подтверждено.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [scientific-tests.log](scientific-tests.log).

### RR-0049 — rejected

[crates/rrrah-app/src/gallery.rs:323](../../../crates/rrrah-app/src/gallery.rs#L323), `with_budget`. Prefetch protected содержит CacheKey, не pixel leases. При cache count limit=0 текущий worker прерывается до первого push. Eager reserve плана 4096 путей выделяет 131072 B вместо 0; managed final=0 в обеих ветках. Кандидат отклонён.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [gallery-tests.log](gallery-tests.log).

### RR-0050 — rejected

[crates/rrrah-animation/src/raster_color.rs:148](../../../crates/rrrah-animation/src/raster_color.rs#L148), `prepare_raster_for_display_with_budget_and_cancel`. Одноэлементный Vec gamma/TRC передаётся во владеющий ICC profile, не растёт в цикле; gamma=1 использует пустую identity кривую. Предложенное повторное использование scratch не сохраняет эту семантику владения.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [animation-tests.log](animation-tests.log).

### RR-0051 — rejected

[crates/rrrah-animation/src/webp_animation.rs:92](../../../crates/rrrah-animation/src/webp_animation.rs#L92), `decode`. WebP: eager reserve 4096 frames выделяет 163840 B до отказа на неполном VP8X input; текущая capacity=0 и та же ошибка. Managed counters не видят это выделение. Сохранены regression и отдельные allocation/RSS пробы.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [animation-tests.log](animation-tests.log), [webp-allocation-results.json](webp-allocation-results.json).

### RR-0052 — rejected

[crates/rrrah-dedup/src/anchor_rank_file.rs:1132](../../../crates/rrrah-dedup/src/anchor_rank_file.rs#L1132), `search_anchor_rank_opposite_files_selected`. Anchor-rank opposite swapped points заранее резервируются по точному числу proposals; reservation переносится в retained evidence. Исходный набор освобождается после передачи; локальный growing scratch не обнаружен.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [dedup-decode-tests.log](dedup-decode-tests.log).

### RR-0053 — rejected

[crates/rrrah-mesh/src/obj_swap.rs:537](../../../crates/rrrah-mesh/src/obj_swap.rs#L537), `read_obj_swap_payload`. OBJ swap уже объединяет scalar/record чтения блоками 16 KiB. Дополнительный BufReader повышает managed peak 1039→17423 B и нарушает прежний tight admission; негативные streaming/cancel/framing сценарии проходят без него.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [mesh-tests.log](mesh-tests.log).

### RR-0054 — rejected

[crates/rrrah-develop/src/thumbnail.rs:145](../../../crates/rrrah-develop/src/thumbnail.rs#L145), `develop_with_cancel`. slots создаются заново внутри develop_with_cancel; AtomicUsize.fetch_add выдаёт каждому worker уникальный idx. Каждый mutex принадлежит одному worker, конкурирующие вызовы имеют разные slots. Удержание lock не создаёт предполагаемую очередь между запросами.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [develop-tests.log](develop-tests.log).

### RR-0055 — verified

[scripts/bench-report.py:301](../../../scripts/bench-report.py#L301), `main`. JSON отчёт публикуется через уникальный tempfile в том же каталоге, flush/fsync и os.replace; прежний mode сохраняется. POSIX flock сериализует writers с deadline 5 с и позволяет следующему writer удалить собственные orphan temp. Реальные SIGKILL до/после replace и конкурентный читатель не видят частичный JSON.

Границы: Квалифицирован локальный POSIX/macOS filesystem. SIGKILL может оставить tempfile до следующей публикации; постоянный .lock намеренно сохраняется. Нет заявления о power-loss durability, NFS, Windows или несогласованных сторонних writers. CLI/report schema сохранены.

Проверки: [source-review.json](source-review.json), [python-regressions.log](python-regressions.log), [bench-report-existing-tests.log](bench-report-existing-tests.log).

### RR-0056 — partial

[crates/rrrah-cache/src/swap.rs:289](../../../crates/rrrah-cache/src/swap.rs#L289), `new_with_budgets`. Подтверждён unlink swap payload внутри TTL index mutex. Реальные файлы, A/A и A/B при 1/2/4 читателях: p95 wait ratio 0.033/0.027/0.046 при drain+drop после unlock. Этот кандидат не внедрён: дополнительный Vec всех expired payloads не допущен бюджетом, а cleanup tails нестабильны. Held handles остаются читаемыми; quota возвращается в 0.

Границы: Нужен ограниченный по памяти протокол извлечения/удаления expired payloads и повторные парные замеры с большими TTL batches, конкурирующими put/get, отменой и admission gates. Малый синхронизированный benchmark не квалифицирует production worker или общее ускорение. Production TTL путь пока сохранён.

Проверки: [source-review.json](source-review.json), [cache-swap-tests.log](cache-swap-tests.log), [swap-ttl-pairs.log](swap-ttl-pairs.log), [swap-ttl-summary.json](swap-ttl-summary.json).

### RR-0057 — rejected

[crates/rrrah-vector/src/pdf.rs:170](../../../crates/rrrah-vector/src/pdf.rs#L170), `decode`. curve_error mutex создаётся на один PDF decode; lock выполняет только присвоение ошибки budget callback. В нормальной ветке lock не берётся, независимые decode не делят mutex. Гипотеза длинной общей критической секции не подтверждена.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [pdf-tests.log](pdf-tests.log).

### RR-0058 — rejected

[crates/rrrah-decode/src/heif.rs:181](../../../crates/rrrah-decode/src/heif.rs#L181), `decode`. HEIF RGBA output уже создаётся with_capacity(width*height*4) после проверки размера и reservation. Выход удерживает pixel storage; точное резервирование не новое изменение. Нативные libheif tests выполнены на этой машине.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [heif-tests.log](heif-tests.log).

### RR-0059 — rejected

[crates/rrrah-texture/src/sti.rs:88](../../../crates/rrrah-texture/src/sti.rs#L88), `decode`. STI заранее резервирует samples, reconciles capacity и credit; выход удерживается raster. Отмеченный Vec::new не означает повторных аллокаций.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [texture-tests.log](texture-tests.log).

### RR-0060 — rejected

[crates/rrrah-develop/src/raster.rs:298](../../../crates/rrrah-develop/src/raster.rs#L298), `hdr_to_linear_srgb`. HDR CPU conversion заранее резервирует output(count). PQ LUT ограничен бюджетом, scalar fallback сохраняет работоспособность при тесном бюджете. Повторное exact reserve уже реализовано; тесты не требуют физического HDR дисплея.

Границы: Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.

Проверки: [source-review.json](source-review.json), [develop-tests.log](develop-tests.log).

## Реестр и совместимость

По всем 30 ID записаны execution status/result/remaining/evidence/report без подмены исходных acceptance. Первые 30 summary сохранены отдельно; добавлена execution_batches секция, её счётчики, ID и report проверяются валидатором. RR-0010 остаётся partial и помечена deferred_by_user: VoiceOver сейчас не нужен пользователю, повторного разрешения не ожидаем. Этот проход не меняет прежние результаты оборудования.

Проверки: точные 1000 ID, уникальность, обязательные поля, зависимости/циклы, source anchors/SHA, execution references и batch counters; тесты валидатора включают искажение batch summary, duplicate/unknown IDs, отсутствие execution и unsafe report path. [Registry tests](registry-tests.log), [strict validator](registry-validation.log). Проверка документа не заменяет runtime proof. Новых публичных Rust API нет; изменения CLI описаны выше. Производственные изменения ограничены X3F watchdog, DICOM tag bookkeeping и bench-report publication.
