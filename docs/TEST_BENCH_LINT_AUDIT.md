# Test, benchmark, dependency and lint audit

Дата проверки: 2026-07-21. Этот документ фиксирует не только зелёный локальный
`cargo test`, но и границы того, что он действительно проверяет. В проекте
пока нет полноценного scheduler/DNG tile backend и нет GPU oracle на CI; поэтому
отсутствующие функции не маскируются микробенчмарками.

## Что проверяется сейчас

| Область | Команда/тест | Результат | Ограничение |
|---|---|---|---|
| Форматирование | `cargo fmt --all -- --check` | green | не проверяет семантику |
| Компиляция | `cargo check --workspace --all-targets --locked` | green | не запускает тесты и не поднимает GPU |
| Unit/invariant | `cargo test --workspace --locked` | green на дату аудита | synthetic frames, без камерного корпуса; benchmark binaries исключены |
| Cache-key bench smoke | reduced-workload `cargo bench ... key_hashing` | non-gating | компилирует/запускает target, но не оценивает shared-runner wall clock |
| Lint | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | green | локальные `#![allow]` требуют периодического пересмотра |
| Dependency policy | `cargo deny check advisories bans licenses sources` | **не green** | см. security findings ниже |
| Rust advisories | `cargo audit` | **не green** | две уязвимости в quick-xml и unmaintained ttf-parser |
| Process benchmark | `scripts/bench-harness.py` + `scripts/bench-report.py` | runnable с fixture | без fixture нельзя называть результат production |
| GPU benchmark | отдельный headless/device runner | не доступен на CI | нужен одинаковый adapter/driver и timestamp queries |

## Добавленные/обязательные инварианты

Unit-тесты должны оставаться детерминированными и дешёвыми. Они проверяют
границы до запуска дорогих RAW corpus tests:

* CFA и black-level grid отклоняют пустые, неполные и non-finite данные;
* `RawMetadata` отклоняет неверные rectangles, white-level и calibration;
* `DecodedMosaic` отклоняет несовпадение `width*height*cpp` и проверяет overflow;
* все восемь EXIF orientation сохраняют нормированный диапазон UV и корректно
  сообщают swap dimensions;
* Bradford/3x3/exposure/tone-map не выпускают NaN/Inf в preview path;
* cache checksum, schema/key mismatch, truncation, trailing bytes и объявленный
  слишком большой payload не приводят к panic или неограниченной аллокации;
* weighted LRU измеряет бюджет в bytes, удаляет старый элемент и не принимает
  entry больше capacity;
* GPU tile halo совпадает с monolithic sensor samples на внутренних границах,
  край clamp-ится, а каждый upload row выравнивается по WebGPU 256-byte rule;
* generation token делает stale работу отменённой до публикации результата;
* telemetry JSONL и Chrome trace остаются валидными, sequence монотонен,
  а медленный live listener не блокирует producer.

Property/fuzz следующий обязательный слой, но он намеренно не подменён
случайными unit-тестами: seed и corpus должны сохраняться как артефакты.
Минимальные targets:

```text
cache_header_mutation   10k mutations, no panic, bounded allocation
metadata_json_mutation  10k serde inputs, only typed error/ok
gpu_tile_dimensions     dimensions {0, 1, max_texture, max+1}, no panic
orientation_grid        8 orientations × 17×17 UV samples
report_jsonl_mutation   malformed rows never produce NaN in release JSON
```

Для RAW decoder fuzzing worker обязан быть process-bounded: `catch_unwind` уже
переводит panic в `DecodeError`, но memory exhaustion внутри стороннего decoder
не остановить этим механизмом. Нужны отдельный worker, wall-time/byte quota и
kill/restart policy; `unsafe` в workspace запрещён.

Fixture CI запускает `scripts/fetch-fixtures.sh` с
`RRRAH_REQUIRE_FIXTURES=1`: отсутствие лицензированного manifest теперь даёт
явный failure. Локальный режим без manifest остаётся видимым `skip`, но не
может сделать release fixture job зелёной.

## Benchmark contract

### Не смешивать разные вопросы

1. `probe` — metadata-only latency и bytes read.
2. `decode` — CR2 serial entropy/predictor и DNG independent tiles отдельно.
3. `postprocess` — demosaic/color math на synthetic deterministic mosaic.
4. `cache` — RAM/disk hit/miss, checksum и admission при фиксированном budget.
5. `GPU` — upload, first-visible, steady p95 frame и dropped frames на одном
   adapter/driver/API.
6. `quality` — PSNR/SSIM/CIEDE2000/seam ΔLSB; нельзя оптимизировать скорость,
   изменив oracle или quality tier.

`scripts/bench-harness.py` создаёт immutable manifest (fixture/binary hashes,
Rust/toolchain/CPU/OS, cache mode); `scripts/bench-report.py` считает p50/p95/p99,
MAD/outliers и deterministic bootstrap CI. `no-persistent-cache` означает лишь
отсутствие cache file, а не cold OS page cache. Не называть его `cold`.

### Микро- и process-benchmarks

* Microbench должен держать input и параметры в `black_box`, иначе LLVM удалит
  вычисления. Минимум 30 samples для release gate; n<10 — exploratory.
* Process benchmark запускает каждую репетицию отдельным process и пишет raw
  rows; warmup отделён от измерений. В каждой группе ключи
  `(fixture, mode, workers, backend)` обязательны.
* CR2 scaling обязан показывать serial fraction: по Amdahl при `s=0.85` восемь
  workers дают максимум `1/(.85+.15/8)=1.16×`; дополнительные threads идут в
  postprocess/prefetch.
* DNG scaling — workers `1,2,4,8,physical`; отдельные кривые tile decode,
  cache wait, upload и first-visible. Никаких средних между CR2 и DNG.
* GPU результаты маркируются API/device/driver и не сравниваются с CPU-only.
  Если timestamp-query недоступен, wall span остаётся CPU observation.

## Dependency/update review

Проверены lockfile и crates.io 2026-07-21:

| Crate | Lock | Available/обсуждение | Решение |
|---|---:|---:|---|
| `rawler` | 0.7.2 | latest 0.7.2, rust-version 1.88 | оставить pinned до отдельной decode compatibility matrix |
| `wgpu` | 30.0.0 | latest 30.0.0, rust-version 1.87 | оставить exact; major API/driver change требует GPU corpus |
| `winit` | 0.30.13 | latest stable 0.30.13, 0.31 beta | не брать beta в production |
| `pollster` | 0.4.0 | 1.0.1 | major update не нужен для текущего synchronous call |
| `tempfile` | 3.27.0 | 3.27.0 | locked |
| `serde_json` | 1.0.151 | 1.0.151 | locked |

Решение об обновлении должно быть benchmark- и compatibility-driven, а не
`cargo update` вслепую: сначала changelog, MSRV, feature graph, затем
workspace test/clippy/deny/audit и GPU smoke на Metal/Vulkan/DX12.

## Security findings and mitigation

`cargo audit` и `cargo deny` в текущем lockfile находят:

* **RUSTSEC-2026-0194** — quadratic duplicate-attribute check в `quick-xml
  0.39.4`;
* **RUSTSEC-2026-0195** — unbounded namespace declarations в `quick-xml
  0.39.4`;
* **RUSTSEC-2026-0192** — `ttf-parser 0.25.1` объявлен unmaintained.

Путь: `winit 0.30.13 → smithay-client-toolkit 0.19.2 → wayland-scanner
0.31.10 → quick-xml 0.39.4`; `ttf-parser` приходит через
`winit → sctk-adwaita → ab_glyph`. `quick-xml >=0.41` исправляет первые два,
но текущий `wayland-scanner` требует `^0.39`, поэтому простое `cargo update -p`
невозможно. Варианты:

1. обновить winit/Smithay после upstream release и прогнать Wayland smoke;
2. для macOS-only artifact отключить default Wayland feature (`winit` с
   `default-features=false`, оставить нужные backend features), явно записав
   потерю Wayland в release matrix;
3. временно принять advisory только как **build-time** risk для vendored XML,
   но не скрывать его в deny-конфигурации и не выпускать без owner/expiry.

`rawler 0.7.2` содержит deprecated SPDX `LGPL-2.1`; license policy должна
нормализовать это как `LGPL-2.1-only` или upstream должен исправить metadata.
Несколько старых platform crates (`bitflags`, `getrandom`, `windows-sys`,
`objc2`, `toml_edit`) дублируются из-за winit/wgpu/rawler; это размер и время
сборки, но не повод ломать backend compatibility.

До решения upstream обязательно:

* RAW decoder process boundary + byte/pixel/time quota;
* cache header/payload size guard до allocation и checksum (уже есть);
* no XML parser on user-supplied sidecars without size/depth/attribute limits;
* `cargo audit --deny warnings` и `cargo deny check ...` в release preflight;
* advisory owner, expiry date и CI report, если временное exception станет
  необходимым. Бессрочный `ignore` запрещён.

## Lint policy / critic

Workspace lint baseline (`unsafe_code=deny`, Clippy `all+pedantic`,
`RUSTFLAGS=-Dwarnings`) — хороший minimum. Но критик отмечает:

* широкие crate-level `#![allow(clippy::...)]` в `app`, `core`, `decode`,
  `gpu`, `bench` скрывают регрессии; новые allow допустимы только на tight
  function scope с issue/benchmark rationale;
* `missing_errors_doc` сейчас allow, хотя публичные security-sensitive errors
  должны иметь actionable docs;
* CI не запускает `cargo doc --workspace --no-deps` и не проверяет links;
* CI запускает Python harness tests/schema validation, но пока не запускает
  реальные RAW/GPU performance runs;
* нет `cargo nextest`, `cargo llvm-cov` и fuzz job. Это не обязательные nightly
  gates, но они нужны в scheduled security workflow;
* `cargo deny` дублирован отдельным job и check job; оставить один canonical
  invocation с lockfile artifact, иначе результаты расходятся.

Рекомендуемый pre-merge набор:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo doc --workspace --all-features --no-deps
cargo deny check advisories bans licenses sources
cargo audit --deny warnings
python3 -m unittest discover -s scripts -p 'test_*.py'
```

Nightly/security:

```bash
cargo fuzz run cache_header -- -runs=10000
cargo fuzz run metadata -- -runs=10000
cargo llvm-cov --workspace --all-features --locked --summary-only
cargo nextest run --workspace --all-features --locked
```

Если инструмент отсутствует, job должен быть `skip` с версией/причиной, а не
зелёным fake pass. GPU и full RAW corpus остаются hardware-labelled jobs.

## Adversarial critic — stop conditions

Работу нельзя объявлять «доведённой до идеала», пока не выполнены все пункты:

1. `cargo audit` не имеет unowned high/critical advisory, либо есть короткое
   documented exception с owner/expiry и ограниченным runtime exposure;
2. malformed cache/metadata/tiles дают typed error без panic/OOM;
3. tile/monolithic seam ≤1 linear 16-bit LSB и GPU/CPU oracle CIEDE2000 p95
   проходит на каждом поддержанном CFA;
4. benchmark manifest содержит fixture hash, backend/device/driver, cache state,
   compiler и power mode; цифры без этого считаются exploratory;
5. stale generation не публикует GPU/cache result, dropped telemetry = 0 в
   correctness run;
6. любой claimed speedup имеет baseline, uncertainty и одинаковый quality tier;
7. отсутствие fixture/GPU не превращается в нулевое время: результат `skip` с
   причиной должен быть виден в отчёте.
