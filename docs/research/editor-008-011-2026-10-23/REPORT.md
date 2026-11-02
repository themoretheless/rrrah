# RR-0008 → RR-0009 → RR-0007 → RR-0011, 2026-10-23

Работа выполнена в существующем dirty checkout; чужие изменения не сбрасывались. RR-0007 и RR-0008 квалифицированы в описанной ниже среде. RR-0009 и RR-0011 остаются partial: границы исходной задачи сохранены. Это отчёт о корректности и абсолютных измерениях, не заявление об ускорении.

## XMP и восстановление

Добавлены типизированные ошибки I/O/блокировки/конфликта, native диалог выбора локальных или сохранённых правок. Выбор не переписывает XMP: запись требует Save и повторной проверки digest. Проверены сохранение неизвестного XML, лимиты, атомарные checkpoint/process-kill сценарии и конкурирующие writers. GUI: внешнее изменение exposure до -1, выбор сохранённой версии, проверка неизменности SHA XMP (`gui-xmp-conflict.png`, `gui-external-xmp-sha.txt`).

Полный source digest привязан к допуску foreground decoder до декодирования; отменённая загрузка не открывает устаревший путь. Стоимость полного чтения крупных/холодных файлов отдельно не измерена. Повторные кадры используют тот же допуск при совпадении decoder stamp; перед каждой записью исходник проверяется полностью. Недоступная идентичность запрещает редактирование. Последняя dirty история переносится между поколениями того же документа без второй аллокации; воспроизведённая потеря последней правки при старом Save в полёте закрыта регрессией (`carry-before-fix.log`).

Recovery запись проверяет digest прочитанных байтов под cooperative lock. Чужой, удалённый или неизвестный журнал не заменяется; последний локальный draft сохраняется отдельно. Собственные промежуточные записи узнаются по source identity и digest в bounded map. При изменении recovery во время reload нужен явный выбор; XMP-конфликт разрешает только recovery autosave до выбора. Ошибка открытия/отказ recovery не разрешает автоматически перезаписать отказавший документ.

Retained snapshots ограничены 32 копиями на документ и 256 KiB на запись; превышение даёт отказ с сохранением существующих файлов. Orphan cleanup удаляет только собственный временный файл с байтами, идентичными committed документу. Неизвестные/различающиеся файлы и symlinks сохраняются; 32 owned orphans запрещают следующую запись. 4100 посторонних фотографий больше не расходуют этот лимит (`large-directory-before-fix.log`). Ограничение shutdown persistence actor — 2 секунды, повторный вызов идемпотентен; зависший системный вызов может завершиться в отделённом владеющем данными потоке. Это не гарантия bounded shutdown всего viewer: foreground decoder по-прежнему имеет отдельное завершение.

В реальном GUI процесс с exposure -0.75 был принудительно завершён после записи recovery; XMP оставался -1. Перезапуск и выбор recovery восстановили -0.75 без изменения XMP (`gui-kill.json`, `gui-recovered-ax.txt`, `gui-recovered.png`). Это одна конкретная последовательность, а не полная kill-матрица всех GUI состояний.

**Остаток RR-0009:** schema-v1 остаётся checksummed snapshot, не persistent command journal. Нет GUI каталога/import retained drafts. Не квалифицированы Windows, network FS, power loss и вся GUI kill-матрица. Cooperative lock не исключает гонку с несотрудничающим writer в последнем rename; legacy lock намеренно не удаляется.

## История и кривая

T открывает native графический редактор монотонной кривой (до 32 точек), перетаскивание объединяется в одну undo-команду. Reset меняет только кривую. Проверены undo/redo, сброс redo новой правкой, смена документа/поколения, полный бюджет истории и сохранение/восстановление кривой. GPU сохраняет отдельные current/baseline кривые для before/after; alpha unpremultiply/reassociate, signed values, точный чёрный, HDR и f32::MAX проверены реальным Metal readback. Identity не меняет float bits.

GUI (`gui-curve.png`, `gui-undo.png`): добавление/перетаскивание точки, undo/redo и Save на линейном PFM. Developed RAW/compute RAW используют тот же raster path; direct fragment RAW требует перехода в linear raster и явно не допускает изменение кривой. Узкий синтетический DNG не является проверкой цвета реальных камер.

## Гистограмма

H показывает RGB/luminance source-linear plot. Для calibrated resident Bayer добавлен настоящий GPU compute histogram: до 65536 samples, 64 bins/channel, явные negative/zero/under/over/invalid/transparent counters, readback 1100 B, logical scratch/readback allocation 2232 B. В работе одна задача и один заменяемый pending запрос. Credits и source lease удерживаются до завершения GPU/readback. Проверены бюджетный отказ, позднее завершение после Drop сервиса, аналитические HDR patches и отмена устаревшего результата. Отдельный revision исключает старый кадр/страницу внутри одного decode generation (`histogram-before-fix.log`).

Исправлено направление столбцов HUD; отрицательный контроль сохранён (`panel-before-fix.log`). На Apple M4 Max реальный Bayer compute RAW + H показал 65536 samples, invalid=0 (`gui-raw-histogram.png`, `gui-raw-histogram-ax.txt`, `gui-final-raw.log`). Запуск квалифицирован с no-cache/prefetch=0, GPU budget 128 MiB, managed budget 256 MiB. Ранее отдельный cached RAW запуск не стал ready за ~90 секунд; stack/sample сохранены (`raw-compute-timeout.json`, `raw-compute-timeout-sample.txt`), причина не установлена. Не считать этот запуск успешным.

64 frame samples, 3 warmup, live Metal window (`live-bayer-before-native-counter.json`, binary hash `gui-before-native-counter-binary-sha.txt`): CPU preparation/submission median 0.441 ms, p95 8.881 ms; host-observed queue completion median 8.110 ms, p95 15.442 ms. Logical managed CPU peak 1,179,648 B; GPU raw/raster/strip peaks 2,361,624 / 1,048,576 / 262,144 B. RSS snapshot другого PFM прогона — 382,384 KiB (`live-app-rss.txt`), не peak RAW. Эти traces сняты до финальной recovery-CAS правки; функциональные Metal tests повторены после неё. Нет baseline A/B, утверждения о FPS, физической scanout latency или ускорении.

**Остаток RR-0011:** wgpu Metal allocator report возвращает None; физическая driver VRAM/полный process RAM peak и репрезентативная серия p95 по корпусу не квалифицированы. Попытка native counter не дала законченных данных и удалена из кода (`native-counter-result.json`). Direct fragment RAW/XTrans GPU histogram не реализованы; linear/developed путь поддержан. Float CPU/GPU совпадение проверено на аналитических patches, не на всех пограничных f32 bins. Физический HDR/CUDA/NVIDIA не проверялись.

## Проверки и артефакты

Итоговые native/Metal результаты и SHA текущих изменённых исходников — `qualification.json`; логи `final-*.log` относятся к финальным проверкам. Ignored и filtered тесты не являются успешными проверками. Clippy завершён с предупреждениями, не является clean lint gate: в edit_store остаются рекомендации о передаче 528-byte EditState по ссылке; vendor lifetime и прочие существующие предупреждения не исправлялись в рамках этих задач. Сохранены реальные отрицательные контроли и промежуточные неуспешные запуски; они не заменяют финальные результаты. `final-app-results.json` содержит актуальные exit codes; старые промежуточные журналы сохранены для аудита.

Реестр: ровно 1000 задач, исходные acceptance/ID/dependencies сохранены; изменены execution RR-0007/8/9/11 и текущие source anchors/hashes. Проверки структуры, Markdown parity и strict snapshot записаны в `registry-validation.log`, mutation tests — `registry-tests.log`.
