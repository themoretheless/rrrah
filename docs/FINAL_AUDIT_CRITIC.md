# Финальный adversarial-аудит

Дата: 2026-07-21  
Область: dependency update, RustSec/cargo-deny, decoder boundary, cache stress,
Naga/WGSL validation, benchmark harness, CI и lint. Это критический аудит
текущего прототипа, а не сертификат безопасности произвольных RAW-файлов.

## Вердикт

**RED — релиз и заявление «доведено до идеала» блокированы.**

1. `cargo audit` остаётся ненулевым: `quick-xml 0.39.4` имеет
   `RUSTSEC-2026-0194` и `RUSTSEC-2026-0195`, а `ttf-parser 0.25.1` помечен
   unmaintained (`RUSTSEC-2026-0192`). Это build-time Wayland path, но CI
   правильно рассматривает advisories как fail-closed release blocker.
2. `RawlerDecoder` вызывает полный `raw_image(..., false)` в UI-процессе.
   `catch_unwind` ловит только Rust panic: OOM, stack exhaustion,
   бесконечный decompressor и лимиты RSS/wall-time пока не изолированы.
3. Metadata-only probe, независимый DNG `TilePlan`, production scheduler,
   GPU residency/page table, fence/pin lifetime и device-loss CPU fallback
   ещё не реализованы. Eager atlas — ограниченный fallback, не residency.

**YELLOW — проверено частично и требует исправления до performance/compatibility
claims.**

- Камерные corpus tests opt-in; без `RRRAH_*_FIXTURE` они печатают skip и
  завершаются успешно. Это допустимо для обычного локального smoke, но release
  fixture job теперь запускает `RRRAH_REQUIRE_FIXTURES=1` и падает без
  `tests/fixtures/SHA256SUMS`; до этой правки зелёный job не означал реальный
  CR2/DNG decode.
- `scripts/bench-harness.py --workers` и `--backend` пока только записывают
  labels; worker count не передаётся приложению. Это не benchmark scaling.
  При ошибке warm-cache seed harness печатает предупреждение и продолжает
  серию (`bench-harness.py:226-236`), а reporter отбрасывает non-zero rows.
  Риск — частичный или cache-miss результат будет выглядеть как валидная
  группа без явного hard failure.
- Reporter сознательно пропускает битые JSONL строки (`bench-report.py:82-96`).
  Для exploratory runs это удобно, для release gate нужен ожидаемый sample
  count и ошибка при truncation/missing rows.
- До последней правки документация расходилась с CI: `TEST_BENCH_LINT_AUDIT`
  указывала 36 тестов и отсутствие Python-проверки; теперь зафиксированы 54
  Rust-теста и 6 Python-тестов, а workflow действительно запускает `unittest`.
- Широкие crate-level `#![allow(...)]` остаются в app/cache/bench/core/decode/gpu
  (включая `cast_*`, `too_many_lines`, `missing_errors_doc`). Это не ошибка
  Clippy, но снижает чувствительность lint gate; новые allow должны быть
  локальными, с rationale и issue.

**GREEN — текущие проверяемые инварианты.**

- `cargo fmt`, workspace test, Clippy `-D warnings`, Rust docs и Python schema
  tests проходят; WGSL проверяется Naga `30.0.0`, совпадающим с `wgpu 30.0.0`.
- Cache header/payload ограничены до allocation, проверяются schema/key,
  dimensions, checksum и trailing bytes; weighted-LRU stress сравнивается с
  независимой моделью и не превышает byte budget.
- Stale generation, tile halo, row-pitch 256-byte alignment и отсутствие
  embedded JPEG в adapter зафиксированы deterministic tests.
- CI не маскирует advisories через `deny.toml ignore=[]`; security job
  публикует JSON report и затем явно падает при неуспешном deny/audit.

## Воспроизводимые команды и наблюдения

Запуск из корня workspace `/Users/themoretheless/Documents/Sources/rrrah`:

```text
cargo fmt --all -- --check                         exit 0
cargo test --workspace --all-targets --locked      exit 0, 54 tests passed
cargo clippy --workspace --all-targets --locked -- -D warnings
                                                     exit 0
RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --all-features --no-deps
                                                     exit 0
python3 -m unittest discover -s scripts -p 'test_*.py'
                                                     exit 0, 6 tests passed
cargo update --workspace --dry-run --verbose        exit 0, 0 packages changed
```

Ожидаемо красные dependency gates:

```text
cargo audit --json
  exit 1; vulnerabilities=2, unmaintained warnings=1
  quick-xml 0.39.4 -> RUSTSEC-2026-0194, RUSTSEC-2026-0195
  ttf-parser 0.25.1 -> RUSTSEC-2026-0192

cargo deny check advisories bans licenses sources
  exit 1; advisories FAILED, bans/licenses/sources OK
```

`cargo update -p quick-xml --precise 0.41.0` нельзя считать remediation:
`wayland-scanner 0.31.10` требует `quick-xml ^0.39`; ручная правка lockfile или
прямая зависимость нарушит resolver contract. Нужен совместимый upstream
Wayland release, reviewed patch или осознанный product profile без Wayland.

## Приоритет оставшихся блокеров

### P0 — перед выпуском

1. Выделить decoder worker process с input/output/RSS/CPU/wall quotas,
   no-network policy и kill/restart на timeout/OOM. `catch_unwind` оставить как
   диагностический guard, но не считать sandbox.
2. Сделать metadata-only probe и bounded DNG `TilePlan` с 64-bit checked
   offsets/counts, overlap/file-range checks и tile-vs-monolithic oracle.
3. Ввести реальный generation-aware scheduler, byte reservations, duplicate
   coalescing, pin/fence lifetime и stale publish gate; подключить CPU fallback
   при device loss/OOM.
4. Разрешить dependency gate только после upstream quick-xml fix или
   time-bounded exception с owner, issue и expiry. Permanent advisory ignore
   запрещён.

### P1 — перед speed/quality claims

1. Обязать fixture manifest/hash/license/expected metadata; отсутствие manifest
   должно быть visible `skip`/failure в CI, а не silent green.
2. Подключить `--workers` к фактическому scheduler и сделать harness fail при
   seed/sample error или неполной серии; reporter должен проверять expected
   count и не скрывать malformed JSONL на release path.
3. Hardware-labelled GPU smoke на Metal/Vulkan/DX12 с adapter/driver/limits,
   cache state, RSS/VRAM и quality tier; host-only Naga test не заменяет этот
   smoke.
4. Сократить crate-level Clippy allows и исправить stale documentation (54
   теста; Python tests уже есть в CI).

## Что не следует объявлять доказанным

- Наличие `raw_image(..., false)` доказывает sensor RAW вместо embedded JPEG,
  но не metadata-only open, tile decode или production first-visible latency.
- Naga parse/validate доказывает WGSL и host layout, но не device limits,
  shader compilation на всех backend и корректность цветов.
- 50 000 операций weighted-LRU — deterministic accounting oracle, не
  потокобезопасность scheduler и не throughput benchmark.
- `no-persistent-cache` не является cold OS-page-cache: harness честно пишет
  `os_page_cache_state=unknown`.
- Синтетический mosaic/halo test не является камерной demosaic/color oracle;
  нужны лицензированные CR2/DNG corpus и независимый linear-sensor reference.

## Exact release stop conditions

Релизный gate остаётся красным, если выполнено хотя бы одно:

- advisory high/critical без upstream fix либо ограниченного owner/expiry
  exception;
- hostile RAW может вызвать in-process OOM/timeout;
- отсутствуют DNG tile bounds и tile-vs-full oracle;
- stale generation публикует результат или GPU slot evicts до fence retirement;
- benchmark не содержит fixture hash, hardware/backend/driver, cache state,
  quality tier и p50/p95/p99 с размером выборки;
- fixture/test отсутствует, но job выглядит зелёной из-за skip;
- malformed benchmark rows silently удаляются на release path;
- изменения объявляются «быстрее» при фактически изменённом quality tier.

## Итог для следующего merge

Текущее состояние — добротный и честно ограниченный прототип: fast checks и
границы кеша зелёные, но безопасность произвольного RAW, независимое
параллельное ingest и production GPU residency ещё не доказаны. Следующий
merge должен закрывать P0 в порядке `probe -> DNG TilePlan -> bounded worker /
scheduler -> GPU residency/fallback`; новые benchmark numbers до этого считать
exploratory, а не индустриальным сравнением.
