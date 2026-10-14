# Первые 30: реализация и проверка, 2026-10-14

Все 30 задач получили результат исследования в реестре. Полное выполнение критериев не заявляется: `verified` — завершённый проверенный результат; `partial` — внедрена часть; `reviewed` — анализ без принятой оптимизации; `rejected` — конкретная гипотеза отвергнута; `measured_no_change` — baseline без принятого альтернативного алгоритма; `blocked` — необходимая среда недоступна.

## Принятые изменения

- RR-0001: repository URL и наследование manifest всех десяти пакетов, проверено cargo metadata.
- RR-0002: отдельные Chrome trace lanes, сохранение происхождения перенесённого span и чтение старых JSONL. Новое поле публичного Rust TelemetryEvent требует миграции внешних struct literals; JSONL совместимость проверена, Rust source compatibility не заявляется.
- RR-0005: ручной self-hosted NVIDIA CI с проверкой выполненного аппаратного теста, сохранением stdout/stderr и характеристик среды. На NVIDIA не запускался.
- RR-0007–0009: bounded native история экспозиции/кривой, viewer undo/redo, атомарный XMP с сохранением чужого XML, checksummed snapshot journal и проверка идентичности исходника. Правки до завершения чтения recovery блокируются; различающиеся XMP/journal не выбираются молча, оба сохраняются. Диалог разрешения остаётся незавершённой частью RR-0009. История 64 состояния на документ, 16 документов и общий бюджет 1 MiB. Изображения не переписываются. Перед сохранением проверяется полный hash исходника, включая замену с прежними длиной/mtime; не-файловые и oversized sidecar отклоняются. Подробные границы в [EDITING.md](../../EDITING.md).
- RR-0010: AccessKit tree, Tab/ShiftTab и доступные действия. Screen reader не квалифицирован.
- RR-0011: фиксированная log HDR histogram, bounded sampling, отдельный отменяемый worker и диагностическое действие H для подготовленного линейного float raster. Нет RAW GPU histogram или графического plot.
- RR-0012: B удерживает исходную экспозицию, C включает split. Один resident texture/atlas, общий viewport. Metal readback обеих сторон совпадает с отдельными baseline/edit эталонами SDR/HDR и RAW.
- RR-0013: устранены две лишние копии prefetch списка; RR-0019: фильтрация поддерживаемого расширения до filesystem запроса и DirEntry.file_type без следования symlink.
- RR-0027: неизменные параметры view не переписывают GPU uniform. Сравнение точных float bits сохраняет реальные изменения и начальную установку текстуры.
- RR-0028: deadline каждой subprocess suite, bounded POSIX process-group cleanup; timeout не может стать verified. Windows ветка добавлена, но не исполнена.

## Измерения

Microbenchmarks не устанавливают скорость всей навигации или FPS. Синтетические входы, три warmup и десять чередующихся пар; сырые результаты сохранены.

- [operations-ab.json](operations-ab.json), [operations-ab-v2.json](operations-ab-v2.json): prefetch new примерно 2.1–2.19×, duplicate 3.05–3.565× по median ratio; выбор обложки каталогов 1000/10000 файлов примерно 5.18–7.81×. Повторный прогон подтверждает направление. Эти числа относятся только к измеренной операции, без cold disk/decoder/UI затрат.
- [uniform-ab.log](uniform-ab.log): реальный Metal Apple M4 Max, контроль — прежняя запись uniform, вариант — guard, 1000 одинаковых обновлений в каждой паре; 10/10 пар в пользу guard. Включены submit/completion; это не frame-time benchmark.
- [swap-warm-baseline.json](swap-warm-baseline.json): 0, 65539 и 16 MiB, точный round-trip, reader calls, освобождение всех кредитов. Наибольший input: median write 19.622 ms/read 11.712 ms/segmented 45.102 ms в данном прогоне. Backend — warm temporary filesystem; cold disk не измерялся. Альтернативный алгоритм не принят.
- [amaze-ab.json](amaze-ab.json): interior offsets вместо clamp, точное сравнение 2752808 байт на boundary входах и 18874376 байт на 1024×768. Median ratio 1.0642, 7/10 выигрышей: недостаточно для принятия. Экспериментальная правка откатана; probe оставлен. Существующий переход проекта на f64 сохранён.

## Проверки и пределы

[native-tests.log](native-tests.log): memory/swap/CUDA/bench — 74 прошли, 2 ignored; ignored hardware/benchmark не считается проверкой. [editor-current-tests.log](editor-current-tests.log) — тест контроллера на текущем workspace. [components-final.log](components-final.log) — финальные дополнительные тесты изолированного harness; harness не заменяет сборку приложения.

[metal-comparison-current.log](metal-comparison-current.log) — сравнение RAW/raster на текущем core с f64, реальный Metal Apple M4 Max. `metal-comparison-aligned-core.log` относится к временному согласованному старому core и не используется как подтверждение текущего workspace. [live-workspace-check.log](live-workspace-check.log) и [viewer-build.log](viewer-build.log) — успешные проверки текущего приложения на момент запуска. Финальная регрессия сохраняется отдельно в `app-current-tests.log`; ориентироваться на завершающую строку, незавершённый лог не является успехом.

[timeout-tests.log](timeout-tests.log): POSIX timeout parent/child и сохранение исходной ошибки subprocess. Windows cleanup не исполнялся.

[clippy.log](clippy.log): общий строгий gate не пройден из-за существующих предупреждений в f64/color/develop/raster/thumbnail. [components-clippy-final.log](components-clippy-final.log): собственные компоненты проверяются изолированным Clippy с разрешениями, соответствующими workspace. Успех определяется завершающей строкой лога. Это не полный Clippy pass проекта.

CUA попытка открытия собственного синтетического viewer bundle завершилась server error -10005 timeoutReached. Screenshot, keyboard smoke и screen reader не подтверждены. Физический HDR требует измерителя: [протокол](../../PHYSICAL_HDR_QUALIFICATION.md). CUDA context reuse/device matching требуют NVIDIA Linux/Windows; speculative production изменения не применялись.

RR-0014–0018, RR-0023–0025, RR-0029–0030 просмотрены по текущему вызывающему контексту. Mutex/Vec/device poll сами по себе не доказывают проблему: try_take уже отпускает guard, idle GPU poll уже возвращается, header probe bounded и переиспользуется, EMF vector уже выделен точно, RAW scratch переиспользуется. Отвергнутые предпосылки отделены от открытых измерений в JSON.

## Состояние реестра

Оригинальные основания сохранены в baseline_evidence и audit_source_sha256. Текущие локации и SHA обновляются для ссылок; это не повторный аудит остальных 970 задач. Тип доказательности исходной задачи и execution.status независимы. Незавершённые критерии перечислены в execution.remaining; первоначальные acceptance не ослаблены.

Рабочее дерево содержит параллельные изменения других задач. Они сохранены. Коммиты, публикация, dispatch внешнего CI и изменение приватных изображений не выполнялись в рамках этой работы.

Проверка source identity читает исходник потоково в background после готовности изображения; event loop файловое чтение не выполняет. Дополнительная нагрузка на накопитель при навигации и ожидание shutdown на медленной файловой системе ещё не измерены. Полная интеграция редактора поэтому остаётся partial; отсутствие GUI-проверки не означает готовность к выпуску.

Машинная сводка прогонов и текущие SHA: [qualification.json](qualification.json). Контрольные суммы артефактов: [artifacts.json](artifacts.json).

Финальные app/GPU команды используют `--target-dir target/first30-aligned` только как отдельный каталог сборки; manifest и core берутся из текущего корня rrrah. Это не временный manifest `/tmp/rrrah-first30-aligned` из ранней проверки согласованной старой версии.

Регрессия f64 JSON: [воспроизведение до исправления](f64-edit-roundtrip-before.log) показывает 1 ULP на 0.9991956459614251. App явно требует serde_json/float_roundtrip; 1024 кривых и реальный XMP/journal round-trip после исправления проходят в components-final.log (12 тестов). Это воспроизведение в минимальном harness без случайных transitive features, не заявление о повреждении пользовательских файлов. Финальный app прогон с явной зависимостью сохраняется в app-final-tests.log.

RR-0023: [реальный cache A/B](cache-insert-ab.json), 3 warmup и 10 чередующихся пар по 20000 insert, PathBuf с 512-байтовым компонентом, 32 идентичности, один shared float raster. На одну K::clone меньше, pin/aliasing/последний владелец проверены; median baseline/candidate 0.88085 и только 3/10 выигрышей. Производственный участок откатан. Вариант сохранён только в cfg(test), новый запуск probe сравнивает приватные тела операций. Во время замера продолжалась компиляция; эти данные не устанавливают универсальное замедление и не дают основания внедрять оптимизацию. Cache unit suite: 140 прошли, 2 ignored; повтор после отката отражён в cache-final-tests.log.

## Финальный исход (2026-10-15, местное время)

App-final-tests.log: 230 прошли, 0 ошибок, 62 ignored, включая f64 round-trip на locked serde_json 1.0.151. App-final-check.log: текущая сборка после отката RR-0023 прошла. Cache-final-tests.log: 140 прошли, 0 ошибок, 2 ignored после отката; prototype находится только в cfg(test).

Metal повтор: raster split прошёл на Apple M4 Max (12.60 s). RAW повтор остановлен после 20 минут: [raw-timeout.json](raw-timeout.json), [стек](raw-metal-compiler-sample.txt). Стек находится в Metal create_render_pipeline/MTLCompiler, не в ожидании завершения GPU-команд. Причина затянувшейся компиляции не установлена. Предыдущий успешный RAW-readback не заменяет этот неуспешный ретест; холодное создание RAW pipeline необходимо проверить до признания сравнения готовым к выпуску. Общий GPU-ретест не объявляется успешным. Остановлен только собственный синтетический тест; shared Metal service и чужие процессы не тронуты.

Ещё незавершённая часть RR-0009: автоматическое обновление identity/истории уже открытой in-memory сессии при замене исходника по тому же пути. Запись при отличающемся полном hash запрещена, но обновление preview/session и диалог решения требуют доработки. Наличие защитного отказа не объявляется полной реализацией этого сценария.
