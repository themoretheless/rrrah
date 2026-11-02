# Первые 30: доступные локальные проверки

Начато 2026-10-24, завершено 2026-10-25 (Asia/Tbilisi). Среда: macOS arm64, Apple M4 Max, настоящий Metal, локальная APFS. Незакоммиченные изменения сохранены. Результаты относятся к указанным операциям и проверенным гипотезам; отказ от ошибочной оптимизации не означает доказательство оптимальности всего метода.

## Внедрённое

**RR-0015: объединены clear и HUD render панели телеметрии.** Раньше `TelemetryState::render` создавал отдельный clear pass, затем HUD pass с Load. Теперь оба действия выполняются одним render pass. Это панель processing pipeline, а не image renderer.

Добавлен совместимый метод `HudRenderer::encode_with_clear(encoder, target, color)`. Старый `encode` сохраняет Load/overlay semantics и прежний no-op при пустой геометрии. Новый метод обязан очищать target даже при пустой панели. Код `TelemetryState` использует новый метод; остальные overlay callers не менялись.

Проверка на настоящем Metal: полное совпадение всех readback bytes для пустой панели и 12 карточек при размерах 640×480, 1248×900 и 320×240. В двух отдельных performance сериях: 3 warmup + 10 alternating A/A и A/B пар, 30 кадров на вариант, target 1248×900 Rgba8UnormSrgb. CPU submission, host queue completion и GPU timestamps измеряются отдельно. Resolve timestamp query на Metal выполняется отдельным submit после завершения render.

| Median candidate/baseline по парным p95 | Серия 1 | Серия 2 |
|---|---:|---:|
| A/A CPU submission | 0.995 | 0.973 |
| A/A GPU | 0.999 | 0.993 |
| A/A host wall | 1.025 | 0.936 |
| A/B CPU submission | 0.611, 10/10 выигрышей | 0.717, 10/10 |
| A/B GPU | 0.851, 8/10 | 0.826, 8/10 |
| A/B host wall | 0.847, 8/10 | 0.844, 8/10 |

Медианы CPU p95 в серии 1: 66.917→41.313 μs, GPU 59.938→50.000 μs. В серии 2 CPU 61.042→42.292 μs, GPU 60.604→49.251 μs. Median paired ratio отличается от отношения медиан. Проигранные пары сохранены. Native sampled Metal allocations одинаковы: 13,467,648 B у обоих вариантов. После удаления тестовых renderer/target/query ресурсов осталось 1,736,704 B внутренних allocations устройства; нулевая native память не заявляется. Счётчик сопоставлен единственному адаптеру по имени и проверен реальной 8 MiB allocation challenge. Это per-device allocatedSize unified memory, не dedicated physical VRAM или lifetime residency.

В живом окне новый standalone pass показал idle pipeline карточку. Окно изменило размер с 3200×2224 до 4112×2580 физических пикселей через native zoom, F6 вернул фокус в изображение. Снимки и binary SHA сохранены. Этот GUI binary предшествует следующей правке opacity; readback последней правки выполнен отдельно. Live снимок не доказывает все реальные RAW pipeline стадии или FPS.

**RR-0010: исправлена зависимость контраста editor panel от HDR сцены.** Подложка с alpha 0.98 пропускала 2% scene-linear фона: при half-float ceiling 65504 это около 1310, намного ярче UI текста около 1. До изменения настоящий Metal regression завершился ошибкой уже на белом фоне (`panel-contrast-before.log`). Подложка Conflict/Curve/Histogram сделана непрозрачной; геометрия, текст, график и расположение не менялись.

Финальная проверка сравнивает каждый пиксель внутри подложки на шести фонах −16, 0, 1, 16, 1024 и 65504 в Rgba16Float: 3 панели × 6 фонов. Пиксель вне панели обязан менять яркость, что исключает ложный проход от неработающего clear. Это проверка независимости контраста от изображения, не WCAG сертификация, физический HDR дисплей или VoiceOver. RR-0010 остаётся partial: реальный screen reader не запускался; запрошенное временное изменение системной настройки не было подтверждено. Системные настройки не менялись. Все форматы/режимы этим узким тестом не квалифицированы.

## Квалифицированы прежние изменения

**RR-0013: verified для настоящего submit path.** Расширен cfg(test) benchmark `ForegroundLoader::submit_request_with_prefetch` до 1/2/4 одновременных producers плюс consumer с чтением реального source stamp. 3 warmup и 10 alternating A/A/A/B пар; по 2000 submit на producer. Последний generation, flight identity и реально потреблённый request проверяются точно. Отдельный regression нагружает submit/cancel при 1/2/4 producers, затем проверяет окончательный запрос, отмену старых токенов, пустую очередь после cancel и явный отказ disconnected channel.

Контроль A/B возвращает только старую лишнюю копию neighbour paths; действующий production алгоритм её не выполняет. Измерены ожидание mutex, удержание, p50/p95 операции и throughput. Test-only TLS clocks/сбор samples добавляют overhead обоим вариантам и не присутствуют в production. Точность clock около 41–42 ns; нулевые либо равные этому шагу lock samples не доказывают отсутствие затрат. В JSON ratios для нулевого знаменателя исключаются; absolute values сохранены.

| Producers | A/B total ratio, серия с hold instrumentation 1 / 2 | A/B operation p95 ratio, серия 2 | A/A operation p95 ratio, серия 2 |
|---|---:|---:|---:|
| 1 | 0.735 / 0.707 | 0.735 | 0.980 |
| 2 | 0.760 / 0.764 | 0.756 | 0.975 |
| 4 | 0.813 / 0.819 | 0.746 | 0.993 |

В повторной серии 4-producer operation p50 не улучшился устойчиво (3/10 выигрышей); выигрыш throughput и хвоста не выдаётся за улучшение каждой метрики. Lock p95 1/2/4: 42→42 ns, 10,146→7,687.5 ns, 32,208→24,042 ns; hold p95 708→458 ns, 1354.5→917 ns, 3000→2313 ns. Это submit queue/ownership path с настоящей конкуренцией, не decoder latency, camera corpus или открытие изображения целиком. Новый production алгоритм сверх ранее устранённого clone не внедрялся.

**RR-0019: verified в локальной APFS области.** Новый regression проверил замену источника той же длины через rename с новым fingerprint, удаление выбранного cover с выбором следующего, symlink к файлу, broken symlink, cancellation, chmod 000 для source и каталога. Перед отказом проверяется фактический PermissionDenied, поэтому root bypass не считается проходом; права восстанавливаются RAII.

Benchmark вызывает настоящий `first_supported_image_with_cancel`; baseline повторяет прежний алгоритм, включая no-follow metadata перед extension filter и тот же `smallest_paths`. Изолированный одноразовый APFS volume: 10000 файлов, 1000 поддерживаемых png / 9000 junk, плюс каталог и symlink с поддерживаемым расширением. Для каждой из 10 alternating A/B пар варианты запускались после отдельных detach/attach, затем сразу повторялись тёплыми. Фиксируются native test executable SHA и полный вывод. Remount-cold VFS не означает физически холодный SSD/controller cache.

В принятом проходе 20 вариантов без ошибок: remount-cold ratio 0.168 (10/10 выигрышей), медианы 99.476→16.688 ms; warm ratio 0.255 (10/10), медианы 26.752→6.944 ms. Selected cover совпал во всех случаях. Это каталог заданной структуры, не все filesystem layouts и не end-to-end viewer.

Отдельная instrumentation без root через test-only macOS libc interposition зафиксировала `lstat` 10002→0, stat 0→0 для fixture paths; fstatat по всем путям процесса 0→0, fstat по всем FD 2→2. Относительные fstatat пути не отфильтровываются. Счётчик перехватывает эти metadata API, а не все kernel I/O, readdir или неизвестные fallback вызовы. Проверен positive control на 100 файлах: 100→0. C helper не входит в production и не является зависимостью Rust приложения; `cc -dynamiclib -O2 -Wall -Wextra` без warnings. Instrumented probes не включены в performance выборку.

Первый APFS проход пересекался с compilation и сохранён как preliminary. Следующий повтор превысил 20 s timeout после первой пары; ошибка и неполные результаты сохранены отдельно, причина не установлена, они не включены в принятые пары. Повтор с сохранением sample при задержке завершился без задержек и без ошибок. Это не исправление причины timeout и не универсальная гарантия отсутствия filesystem stalls. Все созданные образы/тома штатно отсоединены; чужие mount points не затрагивались. Никакой глобальный cache purge не выполнялся.

## Закрытые отрицательные гипотезы

Эти статусы `rejected` означают отказ от конкретного проверенного варианта, без production изменения и без обещания, что других оптимизаций не существует. При нарушении correctness/admission performance comparison непригоден и не используется для оправдания изменения.

- **RR-0014:** ранний выход из общей flight critical section перед enqueue/publication. Детерминированный test-only hook остановил generation 1, generation 2 успел опубликоваться, затем generation 1 затёр очередь и flight. В очереди остался отменённый старый request; актуальное намерение потеряно. Настоящий сериализованный path прошёл 1/2/4 load/cancel проверки. Сохранён общий профиль critical section RR-0013; независимого доказанного сокращения enqueue не найдено. Двухфазный новый протокол не квалифицирован.
- **RR-0016:** пропуск callback polling при скрытом/неперерисовываемом окне. Настоящий Metal upload 16 B плюс completion callback после 20 ms без submit/poll/redraw всё ещё удерживает весь бюджет; следующий upload отказан. Production poll освобождает credits до 0, GPU readback содержит точные 16 байт 0x5a. Изначальный idle fast-path pending=0 уже не вызывает device.poll. Снижение частоты/объединение с poll_loss — другие, не измеренные стратегии; удаление обязательного сервиса отвергнуто.
- **RR-0017:** откладывание drop старого completed upload до подготовки/публикации нового. При budget 72 B, resident 36 + obsolete 36 новая подготовка отказана, после drop obsolete принимается; final credits 0. Вновь прошли 1000 latest-wins replacements, stale completion/caller и cancel/source ownership тесты. Публичный submit имеет `&mut self`: 2/4 независимых вызывающих потока несовместимы с этим API без внешней сериализации; реальная конкуренция — UI и один upload worker. Безопасный двухфазный release-before-wake алгоритм — отдельная непроверенная стратегия.
- **RR-0030:** переиспользование видимого RAW atlas in-place до принятия нового результата. В test-only candidate записан новый слой, затем получена cancellation до обновления uniforms/bind group. Uniforms и logical budget остались прежними, но readback старого кадра изменился в SDR и HDR. После удаления renderer credits 0. Production candidate не включён. Дополнительный completion-safe pool потребует самостоятельного доказательства memory/admission/GPU lifetime и A/B; его ускорение здесь не заявляется.

## Остаток

На этой машине локальные части закрыты/обновлены в указанной области. Неполными остаются RR-0003/0004/0005 (NVIDIA/CUDA, включая два GPU), RR-0006 (физический HDR тракт и измеритель), RR-0010 (screen reader и полная квалификация доступности), RR-0028 (реальная Windows taskkill ветка). Их статусы не повышены по Metal или mock результатам. CUDA, Windows, network FS, power-loss и физическая HDR яркость этим проходом не проверены.

## Проверки и воспроизведение

- `cargo test -p rrrah --bin rrrah first30_remaining::foreground_submit_contention_pairs -- --exact --ignored --show-output --test-threads=1`
- `cargo test -p rrrah --bin rrrah first30_remaining:: -- --show-output --test-threads=1 --skip metal` — safety checks, benchmark остаётся ignored.
- `cargo test -p rrrah-gpu --lib telemetry_pass_qualification::telemetry_clear_pass_pairs_metal -- --exact --ignored --show-output --test-threads=1`. Для одного correctness pass: `RRRAH_TELEMETRY_CORRECTNESS_ONLY=1`.
- `cargo test -p rrrah-gpu --lib telemetry_pass_qualification::editor_panels_do_not_inherit_extreme_hdr_background_metal -- --exact --ignored --show-output --test-threads=1`
- `cargo test -p rrrah-gpu --lib raw_reuse_qualification::cancelled_in_place_reuse_changes_the_retained_frame_metal -- --exact --ignored --show-output --test-threads=1`
- `run-final-gpu.py` запускает реальные suite под существующим POSIX process-group watchdog с deadline 180 s; timeout не является проходом.
- `run-cover-volume.py` требует собранный native probe (`cover-binary.json`) и `RRRAH_COVER_TRACE_LIBRARY`, создаёт только одноразовый том. Если test binary пересобран, manifest нужно заново зафиксировать, старый SHA не соответствует новым bytes. `summarize.py` сохраняет все pair ratios, включая проигранные.

Финальные результаты и manifest — в `qualification.json`; source snapshot не является build attestation всего dirty checkout. Прежние execution evidence не удалены. Количество 1000, ID/поля/зависимости/ссылки и Markdown parity проверяются отдельно от корректности приложения.

Финальные gates: app 328 passed / 0 failed / 48 ignored / 69 filtered; GPU library 77 passed / 0 failed / 7 ignored. Реальные Metal worker 2, idle callback 1, raw reuse negative 1, public HUD API/readback 1 и HDR contrast 1 тест (18 случаев) прошли отдельно. Ignored/filtered не считаются проходом. Финальная сборка debug завершилась; build warnings сохранены, чистый Clippy gate не заявляется.

Реестр: strict validation прошла для ровно 1000 задач и 30 execution records, 0 snapshot warnings; 6 mutation tests прошли; git diff --check чист. Состояние первых 30: 11 verified, 10 rejected для конкретных гипотез, 3 measured_no_change, 3 partial и 3 blocked.
