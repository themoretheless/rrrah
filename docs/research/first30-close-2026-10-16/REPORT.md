# Первые 30: APNG и watchdog — 2026-10-16

## RR-0024: гипотеза отклонена

Проверено точное предварительное резервирование `Vec<Control>` по объявленному числу кадров. Production APNG не изменён. Измеренный baseline побайтно совпадает с текущим `crates/rrrah-animation/src/apng.rs` (SHA256 `fba9bea6778a1c16250833a21b7adcdf1d1b9fe2a0178a12db1061ae9c8c3497`). Изменение candidate — только `try_reserve_exact(count)` до валидации контролов. Оба варианта собраны в одном release binary из сохранённых исходников и locked dependencies. Публичный API не меняется.

Host: Apple M4 Max, 137438953472. rustc 1.98.1 (48a229cea 2026-09-01).

11 сценариев × 10 process-пар с чередованием порядка AB/BA; 3 warmup, затем 10 batches по 32 workloads на процесс. Workload — полный decoder, освобождение результата и black-box output; для alternating — большой 4096-frame и малый 16-frame входы последовательно, внутри одного процесса. Это 220 процессов и 100 batch samples на вариант/сценарий. Полный fingerprint каждого пикселя/ошибки проверяется отдельно от timing. Все 110 пар совпали по результату, пиковой managed памяти и финальному нулевому балансу.

| Кадры / сценарий | Baseline / candidate, median пар | Побед candidate / 10 | Realloc B → C | Peak Rust live B → C, bytes |
|---|---:|---:|---|---|
| 1 / valid | 1.0229 | 9 | 1 → 1 | 38326 → 38326 |
| 16 / valid | 1.0152 | 7 | 3 → 1 | 38326 → 38326 |
| 512 / valid | 1.0088 | 8 | 8 → 1 | 85389 → 85389 |
| 4096 / valid | 1.0027 | 6 | 11 → 1 | 486797 → 486797 |
| 512 / large | 1.0034 | 7 | 8 → 1 | 106402 → 106402 |
| 4096 / invalid | 1.0117 | 6 | 1 → 1 | 38326 → 196734 |
| 16 / truncated | 1.0006 | 5 | 1 → 1 | 120 → 120 |
| 512 / no_memory | 1.0028 | 6 | 9 → 2 | 57454 → 57454 |
| 512 / cancelled | 0.9917 | 5 | 0 → 0 | 147 → 147 |
| 1 / empty | 0.9744 | 4 | 1 → 1 | 120 → 120 |
| 4096 / alternating | 1.0063 | 6 | 14 → 2 | 486797 → 486797 |

На 4096 корректных кадрах отношение median пар ≈1.003, выигрыш лишь в 6/10 парах; alternating ≈1.006 и 6/10. Сокращение realloc само по себе не доказывает ускорение. Ранний некорректный fcTL приводит к пику 196734 вместо 38326 байт (≈5.13×), потому что candidate резервирует все объявленные контролы до их проверки. Это достаточное основание отвергнуть вариант, даже если отдельные короткие сценарии дают небольшую разницу времени.

[Сырые запуски](apng-raw.json), [median/p95 wall/CPU, peak RSS и счётчики](apng-summary.json), [scope и binary SHA](apng-run.json), [сохранённые snapshots](apng-probe/source.json), [locked build](apng-build.log). Managed peak для valid 1×1 — 94 bytes, large 32×32 — 36864, при отказе/отмене — 0; после каждой операции used=0. Allocation count измеряет Rust allocator в текущем потоке, без foreign allocs и временных внутренних копий realloc. RSS — peak отдельного процесса вместе с fixture/warmup, а не только decoder; managed budget не включает сами metadata allocations. Эти величины не взаимозаменяемы.

Синтетические PNG RGBA8 имеют SOURCE frames и выбран последний кадр; коллекция всех контролов разбирается, декодируется необходимый финальный кадр. Сценарии: 1/16/512/4096 frames, larger 32×32, invalid second control, truncated IEND, zero-budget refusal, stale generation, empty bytes и alternating. Это не полная квалификация APNG, холодного диска или viewer. Посторонний compiler был активен при подготовке; процесс не останавливался. Регрессия памяти и смешанные pair ratios не дают оснований заявлять ускорение.

Воспроизведение из корня репозитория:

```sh
cargo build --offline --locked --release --manifest-path docs/research/first30-close-2026-10-16/apng-probe/Cargo.toml --target-dir target/first30-apng-probe --jobs 2
python3 docs/research/first30-close-2026-10-16/run-apng.py
cargo test --offline -p rrrah-animation --lib apng --jobs 2 --target-dir target/first30-aligned
```

Нативные APNG регрессии: 10 passed, 0 failed, 1 ignored ([apng-tests.log](apng-tests.log)). Пропущен отдельный manual paired12MP APNG PREVIOUS storage benchmark; он не относится к проверенному reservation-варианту и не считается пройденным.

## RR-0028: проверка усилена, статус partial

Три теста проверяют normal exit 0/stdout, exit 7/stderr и реальный timeout. Последний child игнорирует SIGTERM; после возврата watchdog проверяется, что он отсутствует либо имеет zombie state и больше не исполняется. Срок возврата ограничен 5 секундами для timeout 1.5 секунды. Мутация, отключающая group SIGKILL, оставляет этот child исполняющимся и закономерно проваливает тест; cleanup затем завершает только созданный fixture child.

[Три теста: pass](watchdog-tests.log). [Мутация: ожидаемый failure](watchdog-mutation.log), runner завершается успешно лишь при одном ожидаемом failure с причиной `child still executing`. `run_suite` production в этом проходе не изменялся. Windows taskkill не выполнялся; весь RR-0028 поэтому не отмечен verified.

## Реестр и незакрытые условия

Генератор Markdown теперь берёт ссылку на отчёт из `execution.report` каждой задачи; раньше все ссылки вели на исходный отчёт и скрывали новые доказательства. Валидатор проверяет также соответствие ссылки на отчёт; мутационный тест отвергает возврат к устаревшей ссылке. Проверки: 1000 задач, 0 snapshot warnings, 6 тестов валидатора passed. Количество и критерии задач сохранены. RR-0024 переходит reviewed → rejected: проверенная отрицательная гипотеза, а не выполненная оптимизация. RR-0028 сохраняет partial.

Остальные условия первых 30 не были выполнены этим экспериментом: NVIDIA/CUDA runner, физический HDR с измерением, screen reader и GUI, RAW Metal pipeline/readback/latency, профили GPU/конкуренции, cold-disk условия. Предыдущие bounded benchmarks и исходные отчёты сохранены; новые результаты не подменяют эти условия. Полные оставшиеся требования находятся в `execution.remaining` реестра.
