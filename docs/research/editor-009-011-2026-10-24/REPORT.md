# RR-0009 и RR-0011 — завершение проверки, 2026-10-24

Исходные критерии и IDs реестра сохранены. Проверенная среда: macOS / Apple M4 Max, локальная файловая система, SDR Metal surface. Существующая незакоммиченная работа сохранена; коммиты и push не выполнялись.

## RR-0009: командный журнал v2

`edit_journal.rs` хранит контрольную точку и последовательность команд изменения экспозиции/кривой. Каждая команда содержит sequence, previous digest и собственную checksum; весь документ имеет checksum, source identity и page. Replay проверяет порядок, каждый промежуточный EditState и итог. Undo/redo и coalesced autosave сохраняются как переход к соответствующим значениям полей; session undo buffer остаётся отдельным.

Лимит — 64 команды и 256 KiB на документ. При заполнении 32 старых команды сворачиваются в новую контрольную точку с сохранением chain anchor и sequence; оставшийся хвост и новая команда воспроизводят тот же последний целый набор правок. Это ограниченная история recovery, а не бесконечное persistent undo. Весь bounded log заменяется атомарно под существующим cooperative lock и digest compare-and-exchange, с fsync временного файла и каталога. Частичный импорт повреждённой цепочки не допускается.

Совместимость: XMP и `EditState` остаются schema 1. Recovery v1 читается без перезаписи и мигрирует в checkpoint v2 при следующем checked Save; retained snapshots v1 продолжают читаться. Старый бинарник v1 не понимает новый command log и должен отказать, сохранив его байты. Неизвестная схема не мигрируется автоматически; явный clean restart сначала сохраняет точный refused документ в retained backup, затем создаёт v2. Проверки source identity, внешнего изменения, лимита backups и orphan policy сохранены.

Проверены 1000 последовательных команд с compaction/replay, legacy migration на 200 сохранениях, перестановка/пропуск/изменение команд, неверный EditState при пересчитанном outer checksum, truncation, чужой source/page и отказ без уничтожения данных. Kill-матрица атомарного документа расширена до 12 сочетаний: v1→v2, v2→v2, compaction и XMP × temporary_synced/persisted/directory_synced. Существующие lock/retained/source-change проверки сохранены.

Реальный GUI: исходный XMP с exposure 0; затем +0.50 и трёхточечная кривая, 5 committed команд v2. После SIGKILL изолированного viewer перезапуск показал конфликт; выбор recovery восстановил +0.50 и форму кривой. XMP SHA не изменился. Артефакты: `recovery-kill.json`, `recovery-v2-before-kill.json`, `recovery-v2-after-choice.json`, `recovery-verification.json`, `recovery-v2-restored-ax.txt`, `recovery-v2-restored.png`. Снимок `recovery-v2-before-repaint.png` — промежуточный кадр до repaint, не доказательство результата.

Границы: OS process kill не моделирует отключение питания; Windows/network FS не квалифицированы. Cooperative locks не исключают несотрудничающего writer при финальном rename. Deadline относится к persistence actor, не всему decoder shutdown. GUI каталог retained drafts остаётся возможным расширением; последнее целое состояние текущего документа восстанавливается существующим выбором recovery/XMP.

## RR-0011: расширенные проверки

Добавлена явная проверка crop extents, переполнения координат и размера GPU source buffer до dispatch. Неверный внутренний crop resident input теперь явно отвергается на границе dispatch; штатный producer уже ограничивает геометрию. Это защитное усиление, не воспроизведённый сбой доступного пользователю сценария. Проверены три геометрии (нечётный crop с отступом, крайний одиночный пиксель, полный нечётный sensor), 36 повторов, f32::MAX, очень малые положительные значения, отрицательные каналы, NaN/Inf, прозрачность и alpha вне [0,1]. Все RGB/Y counters совпали с CPU oracle на этих входах. Четыре неверные геометрии отвергнуты, credits после каждого завершения — 0.

Повторены Metal тесты generation/revision cancellation, замены pending, budget refusal, drop-before-completion и аналитических patches. Это конечная матрица, не битовое доказательство для всех пограничных f32 log bins. Direct fragment RAW/XTrans не получили отдельный GPU histogram: для них остаётся linear/developed путь. Физический HDR, CUDA и NVIDIA не квалифицированы.

## Память и p95 в реальном viewer

Четыре отдельных запуска одного frozen debug executable: source-linear HDR 513×259 и 2048×1024 без кэша, Bayer 256×256 с пустым и затем заполненным mosaic cache. В каждом H показал histogram; RAW использовал реальный Bayer compute path. Для каждого сохранены argv, binary/source hashes, AX, screenshot, viewer log, kernel usage и 64 frame samples; 3 warmup отброшены, nearest-rank p95 рассчитан по 61 sample. Полный машинный результат — `memory-results.json`.

| Сценарий | Max RSS, MiB | Peak footprint, MiB | Sampled Metal allocations, MiB | CPU p95, ms | Queue-completion p95, ms |
|---|---:|---:|---:|---:|---:|
| HDR 513×259 | 180.5 | 571.2 | 83.9 | 9.31 | 16.98 |
| HDR 2048×1024 | 245.9 | 738.9 | 151.3 | 18.87 | 32.12 |
| RAW cold cache | 296.3 | 753.8 | 194.8 | 18.37 | 29.14 |
| RAW warm cache | 240.9 | 759.7 | 229.8 | 17.27 | 24.57 |

Max RSS и peak memory footprint получены из Darwin `/usr/bin/time -l` за весь lifetime дочернего viewer, включая загрузку и нормальное закрытие. Это разные kernel метрики, их нельзя складывать. Инструментированный процесс включает однократный transient allocation challenge 8 MiB; это известная добавка к наблюдаемому lifetime RSS, а не вычтенный из результата расход.

GPU метрика — `MTLDevice.currentAllocatedSize` внутри того же процесса. Разрешён только единственный Metal device с именем выбранного adapter; затем настоящий wgpu allocation challenge проверяет рост и освобождение счётчика. Отдельный native тест показал 999,424 → 17,776,640 → 999,424 B для выделения/освобождения 16 MiB (`native-counter-test.log`). Это безопасный binding API без нового unsafe-кода. Неоднозначная/неподдержанная среда даёт явную ошибку и не засчитывается сборщиком результатов.

Metal peak — максимум **выборок** при 64 histogram frames, не доказательство глобального lifetime peak GPU residency. На unified-memory Apple Silicon это учёт ресурсов Metal, а не отдельная физическая VRAM. RSS/footprint уже могут учитывать shared GPU allocations: складывать их с Metal нельзя. wgpu allocator report по-прежнему None, но native counter теперь проверен реальным выделением. Управляемые бюджеты (CPU 256 / GPU 128 MiB) относятся к учитываемым ресурсам приложения; surface/pipeline/driver allocations в этот лимит целиком не входят. Поэтому измеренный Metal максимум выше 128 MiB не скрыт как успешная проверка глобального hard cap.

CPU p95 описывает подготовку/submission кадра; queue completion включает ожидание callback, scheduling и работу очереди. Это не scanout latency, FPS или GPU timestamp. Замеры сделаны на общем рабочем desktop, без A/B baseline; ускорение и универсальный performance budget не заявляются. Начальная allocation challenge попадает в warmup, но lifetime memory метрики включают её.

Warm RAW действительно сообщил `full RAW ready from mosaic cache in 14.43ms` в native UI, histogram — 65536 samples, invalid=0. Это сообщение decoder readiness, не полная launch-to-display latency. Старый cached startup timeout из отчёта 2026-10-23 не воспроизвёлся на этом корпусе; его первопричина не установлена и исправление того инцидента не заявляется.

Воспроизведение: `scripts/prepare-editor-memory-probe.py <small|large|raw-cold|raw-warm>` подготавливает изолированный bundle, сохраняет один frozen executable и не запускает GUI. Запустить показанный bundle, нажать H, дождаться trace, закрыть только его окно. `raw-warm` запускается после `raw-cold` на том же cache-dir. Перед повторной серией следует выбрать отдельный research/cache каталог и не подменять уже запущенный Mach-O. Fixtures в ignored target; их hashes записаны в `fixtures.json` и manifests. Recovery GUI использует отдельный edited.pfm.

## Итоговые проверки

Итоговые результаты сборки, тестов, Clippy и SHA исходников находятся в `qualification.json`. Clippy — с предупреждениями, не clean gate. Ignored/filtered тесты не засчитаны как прошедшие. Финальные registry strict-snapshot/Markdown parity и 6 mutation tests записаны в `registry-validation.log` / `registry-tests.log`; записи и исходные критерии всех 1000 задач сохранены.

Frozen GUI binary включает command log v2, crop validation и native counter. Последующие отличия до финального binary относятся к test-only kill matrix и cfg(test) legacy fixture writer; runtime поведения GUI они не меняют. Хэши обоих binaries сохранены отдельно.
