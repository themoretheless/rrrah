# RR-0056 — освобождение TTL payloads фиксированными пакетами

2026-10-25. Продолжение проверки RR-0031–RR-0060. **RR-0056 verified в области фонового TTL-worker.** В диапазоне теперь 4 verified и 26 rejected; rejected означает отклонённую конкретную гипотезу, не внедрённую оптимизацию. Исторические результаты первого прохода сохранены отдельно.

## Реализация и границы

Раньше TTL-worker вызывал `WeightedLru::prune_expired` под общим index mutex: последний SwapHandle мог удалять файл, блокируя читателей. Испытанный ранее полный `drain_expired` сокращал ожидание, но добавлял второй O(N) heap Vec payloads. Этот вариант не принят.

Теперь worker сохраняет прежний O(N) план ключей, затем извлекает значения **пакетами до 64** в `[Option<StoredPayload>; 64]` на стеке. Guard отпускается до Drop массива и filesystem cleanup. Между пакетами проверяется отмена; следующий проход способен найти остаток прерванного плана. Таблица сканируется при планировании один раз: нет повторного полного обхода для каждого пакета. Рабочий массив на этой сборке занимает 1536 B; это размер массива, не полный stack high-water. Дополнительной heap-коллекции payloads нет. Существующий key Vec остаётся O(N) и, как раньше, не включён в MemoryBudget: это не обещание O(1) всей памяти очистки или полного RSS budget.

`WeightedLru` получает additive API `expired_keys_for_prune(cutoff)` и `remove_if_expired_at(key, cutoff)`. После планирования срок и protected status перепроверяются. Новая запись с тем же ключом и более поздним deadline не удаляется старым планом; pin после планирования сохраняется. Планирование оставляет консервативную expiry hint, чтобы отмена/перекрытие планов не скрыли оставшиеся expired entries. Старые policy методы сохраняют поведение.

Оптимизация применяется **только в background TTL-worker**. Он уничтожает весь пакет и завершает очистку до обработки следующей queued write, сохраняя disk-quota ordering. Публичный `ImageSwapCache::prune_expired`, lazy get expiry и spill eviction сохраняют прежнюю синхронизацию. Перенос публичной очистки за mutex мог бы допустить worker к записи до фактического освобождения дисковой квоты и вызвать новый transient quota refusal; этот вариант исключён. Наличие коротких файловых операций под lock в других путях не объявляется устранённым.

Код: [swap.rs](../../../crates/rrrah-cache/src/swap.rs), [policy.rs](../../../crates/rrrah-memory/src/policy.rs). Производственные изменения не требуют VoiceOver, GPU, HDR или другого оборудования; системные настройки не менялись.

## Проверки корректности

- Policy: брошенный/повторный план; точный cutoff; fresh replacement; pin/unpin; повторное удаление; resident weight и завершение expiry.
- Реальные 199 swap objects: отмена после первого пакета 64, остаток обнаруживается повторным планом; в конце index пуст. Внешний held handle остаётся читаемым; quota=1 object до его release и 0/0 после release.
- Другой поток получает index mutex, пока detached файлы ещё существуют. Удаление detached массива освобождает quota уже вне index guard.
- Все memory/cache unit suites, включая существующие idle/busy TTL-worker, cancellation, queue/count/byte limits, stalled writer, restore и lease/admission tests. Public prune сохранён и проверен на финальном коде.

[Команды и результаты](checks.json), [memory](memory.log), [cache final](cache-final.log). Первоначальный cache run до восстановления прежнего public-prune пути сохранён как [cache.log](cache.log); финальный run проверяет совместимую реализацию.

| Suite | Passed | Failed | Ignored |
|---|---:|---:|---:|
| memory | 45 | 0 | 0 |
| cache | 157 | 0 | 5 |

Ignored benchmarks не засчитаны как автоматически выполненные tests; три необходимых benchmark запуска выполнены явно и имеют отдельные exit/timeout/summary/SHA в [benchmarks.json](benchmarks.json).

## Парные замеры

Локальный macOS/APFS, реальные SwapStore files 8×32 B и 1/2/4 синхронизированных читателя. По 3 warmup и 10 alternating A/A, затем 10 alternating A/B pairs, 10 rounds на вариант. Baseline удаляет файлы под mutex; candidate использует тот же fixed-stack detach, что worker. A/A сравнивает candidate с самим собой. Проверяются пустой index после detach, сохранность внешнего handle, возврат physical file quota. Это замер критической секции при синхронизированных contenders, не throughput всего worker или viewer. Сторонняя нагрузка на машине не запрещалась; все controls сохранены.

`pairs-1` собран до восстановления прежнего публичного prune: код измеряемого detach идентичен. `pairs-2` и scaling — финальный код. Для итогового вывода используется финальная серия; первая сохранена как дополнительный контроль. Median ratios candidate/baseline:

| Серия | Readers | A/A p95 wait | A/B p50 wait | A/B p95 wait | A/B hold p95 | A/B cleanup p95 | Wait wins / 10 |
|---|---:|---:|---:|---:|---:|---:|---:|
| [pairs-1.log](pairs-1.log) | 1 | 0.8972 | 0.0212 | 0.0298 | 0.0070 | 0.9089 | 9 |
| [pairs-1.log](pairs-1.log) | 2 | 2.3018 | 0.0272 | 0.0297 | 0.0087 | 0.8821 | 10 |
| [pairs-1.log](pairs-1.log) | 4 | 0.6311 | 0.0443 | 0.0561 | 0.0100 | 1.0046 | 10 |
| [pairs-2.log](pairs-2.log) | 1 | 0.8315 | 0.0240 | 0.0279 | 0.0083 | 0.9103 | 10 |
| [pairs-2.log](pairs-2.log) | 2 | 0.9302 | 0.0230 | 0.0296 | 0.0095 | 0.8068 | 10 |
| [pairs-2.log](pairs-2.log) | 4 | 0.6394 | 0.0358 | 0.0408 | 0.0085 | 0.8078 | 10 |

Все raw tails остаются в журналах. Уменьшение ожидания mutex не интерпретируется как ускорение filesystem cleanup; cleanup time отдельно включает Drop после unlock. Нет обещания выигрыша на любом размере каталога или filesystem.

Дополнительно [scaling.log](scaling.log): реальные 129/1025 файла, несколько пакетов, alternating baseline/candidate, один warmup и 3 пары; index и quota в конце 0. Это конечная проверка масштабирования/стоимости очистки, не строгий latency release gate (3 пары). Выделения/metadata setup происходят до timed cleanup.

| Entries | Cleanup median ratio | Pairs |
|---|---:|---:|
| 129 | 0.9689 | 3 |
| 1025 | 0.9559 | 3 |

## Воспроизведение и реестр

```sh
python3 docs/research/rr-0056-bounded-2026-10-25/check.py
python3 docs/research/rr-0056-bounded-2026-10-25/bench.py
cargo test -p rrrah-cache --lib -- --show-output --test-threads=1
python3 scripts/test-improvements-1000.py
python3 scripts/validate-improvements-1000.py --strict-snapshot
```

Драйверы имеют deadline 300/240 с и сохраняют команды, exit и timeout. Повторный запуск перезаписывает одноимённые исследовательские журналы — сначала сохраните текущий снимок. SHA исходников и native binaries в [qualification.json](qualification.json) и benchmarks.json. Реестр содержит итог RR-0056 и новый отчёт, исходный acceptance сохранён. Счётчик execution_batches обновлён; первые 30 и аппаратные ограничения не объявляются заново выполненными.

[Строгая проверка реестра](registry-validation.log), [регрессионные проверки валидатора](registry-tests.log). Структурная валидация документа не заменяет runtime checks выше.
