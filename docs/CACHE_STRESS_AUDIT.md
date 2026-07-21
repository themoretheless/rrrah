# Cache and scheduler stress audit

Дата: 2026-07-21. Область: `rrrah-cache` и контракты из
`PLAN_SCHEDULER_RESIDENCY.md`. Цель — отделить проверяемые сейчас инварианты
кеша от обещаний будущего scheduler/residency слоя.

## Что реально тестируется

`crates/rrrah-cache/tests/cache_stress.rs` не использует камеры, GPU, sleep или
нестабильный порядок потоков:

* 50 000 детерминированных операций над 64 ключами сравниваются с независимой
  моделью weighted-LRU; проверяются байтовая ёмкость, длина, hit/miss,
  replacement и точный eviction по recency;
* нулевой вес, запись близко к лимиту и повторная запись одного ключа проходят
  через тот же bounded цикл;
* oversized replacement не удаляет существующее значение, а remove дважды не
  уменьшает resident bytes ниже нуля;
* 1 024 image-index ключа и изменения `file_size`, `modified_ns`, sample hash
  не смешивают persistent cache domains;
* тестовый `PublishGate` показывает необходимое правило: generation проверяется
  под тем же lock, что и изменение published state, поэтому старый результат не
  может пройти между проверкой и публикацией;
* тестовый `ReservationLedger` проверяет checked admission и идемпотентное
  release, включая попытку переполнения `u64` и двойное освобождение.

Эти тестовые оракулы намеренно малы и детерминированы. Они не доказывают
потокобезопасность отсутствующего production scheduler и не являются
throughput benchmark. Их задача — зафиксировать арифметику и state-machine
инварианты до появления рабочих очередей.

## Критический разбор текущего production-кода

`WeightedLru` сейчас однопоточный и не имеет pin/in-flight состояния. Это
безопасная fallback-структура для теста, но не GPU residency manager:

1. `get` возвращает `&V`, поэтому concurrent eviction нельзя разрешить без
   внешнего lock или `Arc`/guard API;
2. нет pin guard для текущего viewport и нет защиты записи, которая ещё
   используется GPU submission;
3. `resident` обновляется через saturating arithmetic. Это не выпускает
   отрицательное значение, но в production-модуле маскировало бы double-release;
   будущий `Reservation` обязан иметь single-assignment terminal state и debug
   assertion;
4. `clock` использует wrapping increment. Переполнение практически недостижимо
   за одну сессию, но persisted/long-running cache не должен полагаться на
   уникальность времени: при вводе scheduler нужен epoch reset или сравнение с
   bounded generation;
5. `pop_lru` линейно сканирует `HashMap` (`O(n)` на eviction), что приемлемо для
   fallback и маленького теста, но не для шардированного 2Q/TinyLFU tile cache;
6. duplicate work coalescing и cancellation отсутствуют. Две одинаковые заявки
   могут декодировать один tile дважды — это correctness-neutral, но опасно для
   RAM/GPU budget и latency.

## Обязательные scheduler tests до production claims

После появления `WorkScheduler`, `Reservation`, `PinGuard` и
`GpuResidency` требуются отдельные tests (не simulation):

| Инвариант | Failure injection | Acceptance |
|---|---|---|
| stale publish | old generation completes after new open | 0 stale UI/page-table/cache commits / 10 000 transitions |
| reservation | cancel, panic/error, duplicate drop | resident + reserved never exceed pool cap; no double release |
| pin/eviction | viewport pin changes while upload is in flight | pinned and in-flight slots never evicted before fence retirement |
| duplicate work | same tile requested by 2–N viewport events | one producer, N subscribers, one cache admission |
| cancellation | new generation during CR2/DNG decode | stale bytes may finish only at safe boundary, never publish |
| GPU ABA | slot evicted/reused before old submission retires | page table generation mismatch prevents sampling stale slot |
| device loss | loss during upload and fence polling | slots/staging become `DeviceLost`; CPU fallback remains visible |
| integer bounds | `u64::MAX` dimensions/offsets/weights | typed rejection before allocation; no wrap or panic |

The tests must use a deterministic fake clock, fake fence and fake decoder. A
benchmark that merely launches more threads is not evidence of correct
parallelism: it must report queue wait, serial decode, cache wait, upload and
publish spans separately.

## Benchmark and lint gates

Run the current fast gate:

```bash
cargo fmt --all -- --check
cargo test -p rrrah-cache --all-targets --locked
cargo clippy -p rrrah-cache --all-targets --locked -- -D warnings
```

When scheduler code lands, add `criterion`/process benchmarks only after the
deterministic tests above. Report p50/p95/p99, bytes admitted/evicted,
duplicate-work ratio, stale drops, and queue wait. Never merge a result that
combines cache-hit and cache-miss cases or claims linear speedup for CR2's
serial entropy stream.

## Verdict

Current cache fallback has bounded byte admission and atomic disk persistence,
and now has a deterministic stress oracle. The scheduler/residency layer is not
implemented yet; generation gate, RAII reservations, pinning, duplicate-work
coalescing, fence retirement and device-loss fallback remain P0 blockers. The
new tests make those gaps explicit instead of presenting a false green
concurrency benchmark.
