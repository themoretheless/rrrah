# CI, lint и toolchain audit

**Дата:** 2026-07-21  
**Область:** workspace `rrrah`, workflow `.github/workflows/ci.yml`,
`rust-toolchain.toml`, `Cargo.toml`, Python benchmark schema tests и dependency
policy.

Этот документ фиксирует не только зелёные проверки, но и намеренно красный
security gate. `cargo audit`/`cargo deny` нельзя превращать в «зелёный» статус
посредством бессрочных advisory ignores: текущий lockfile содержит два RustSec
DoS для `quick-xml 0.39.4` и unmaintained `ttf-parser`. См. также
[`SECURITY_DEPENDENCY_AUDIT.md`](SECURITY_DEPENDENCY_AUDIT.md) и
[`DEPENDENCY_UPDATE_AUDIT.md`](DEPENDENCY_UPDATE_AUDIT.md).

## Что изменено в CI

Workflow теперь состоит из независимых проверок:

* `checks` — форматирование, all-target/all-features check и Clippy,
  unit/integration tests без запуска benchmark binaries, отдельный короткий
  non-gating benchmark smoke, rustdoc с `-Dwarnings` и Python-тесты схемы;
* `msrv` — отдельная проверка объявленного Rust 1.89 (без подмены его latest
  stable); она компилирует all-targets и запускает unit/integration tests;
* `key-performance` — ручной opt-in p95 gate только на калиброванном
  self-hosted runner `rrrah-perf`; shared CI не принимает wall-clock решения;
* `fixtures` — проверка fixture manifest и decoder tests после успешного
  `checks`;
* `security` — единственный canonical `cargo-deny` job плюс `cargo audit`.
  Оба шага выполняются с `continue-on-error: true` только для того, чтобы
  загрузить JSON-отчёт; финальный `Enforce dependency gate` возвращает failure,
  если любой из них неуспешен.

Отдельный дублирующий `deny` job удалён. Это не ослабляет политику: известные
advisories остаются видимыми и блокируют workflow, но результаты тестов и
benchmark schema не скрываются за ранним падением dependency job. Security job
запускается также по weekly schedule и вручную, чтобы база advisory не
устаревала вместе с репозиторием.

`cargo-audit.json` загружается как artifact даже при failure. Это позволяет
разобрать точный advisory database commit, package path и remediation без
повторного запуска workflow; рядом сохраняются `rustc`, Cargo и cargo-audit
версии для воспроизводимости.

Workflow ограничивает `GITHUB_TOKEN` разрешением `contents: read`. Action refs
сейчас используют поддерживаемые major tags (`@v4`, `@v2`) для автоматического
получения security fixes; для supply-chain hardened release их следует
зафиксировать на commit SHA и обновлять через Dependabot/ручной review. SHA
pinning — отдельный hardening task, а не основание отключать текущие проверки.

## Локальный результат аудита

Проверено на macOS arm64, `rustc 1.96.0`, `cargo 1.96.0`, Python 3.14.6,
`cargo-deny 0.20.2`, `cargo-audit 0.22.2`:

| Проверка | Команда | Результат |
|---|---|---|
| Format | `cargo fmt --all -- --check` | **pass** |
| Feature/target check | `cargo check --workspace --all-targets --all-features --locked` | **pass** |
| Lint | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | **pass** после исправления `manual_assert` |
| Tests | `cargo test --workspace --all-features --locked` | **pass**; benchmark binaries не запускаются как тесты |
| Cache-key bench smoke | reduced-workload `cargo bench ... key_hashing` | **non-gating**; проверяет запуск, не скорость runner-а |
| Rustdoc | `RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --all-features --no-deps` | **pass** |
| Benchmark schema | `python3 -m unittest discover -s scripts -p 'test_*.py'` | **pass**, 6 тестов |
| Dependency policy | `cargo deny check advisories bans licenses sources` | **fail closed**: 2 quick-xml advisories + ttf-parser unmaintained |
| RustSec | `cargo audit --deny warnings --json` | **fail closed**: 2 vulnerabilities, 1 unmaintained warning |

Первый запуск Clippy на текущем stable выявил `clippy::manual_assert` в
fixture-only helper `configured_fixture`. Условие `if !path.is_file() { panic! }`
заменено на эквивалентный `assert!`; production decoder semantics не менялись.
Это важный сигнал: `RUSTFLAGS=-Dwarnings` в CI действительно обнаруживает
новые lint-диагностики toolchain, а не маскирует их crate-level allow.

Число тестов — наблюдаемое на дату аудита; добавление тестов должно увеличить
его, а не использовать фиксированное число как единственный gate.

## Toolchain и MSRV

`Cargo.toml` объявляет `rust-version = "1.89"`, потому что `rawler 0.7.2`
требует Rust 1.88 и workspace использует edition 2024. `rust-toolchain.toml`
остаётся на `stable` для разработки и получает `rustfmt`/`clippy`; это не
доказывает MSRV. Поэтому CI содержит отдельный pinned
`dtolnay/rust-toolchain@1.89.0` job.

Правила обновления:

1. новый crate или major/minor update сначала проверяется на 1.89;
2. exact pins (`rawler`, `wgpu`) не снимаются без RAW corpus, shader golden
   tests и benchmark comparison;
3. `Cargo.lock` меняется только resolver-командой Cargo, не ручным редактированием;
4. `rust-toolchain.toml` не переводится на nightly ради обхода lint или API
   ошибки; nightly допускается только для отдельного fuzz/coverage workflow;
5. если upstream поднимает MSRV, меняются `Cargo.toml`, CI matrix и release
   notes одним review.

## Линтерная политика

В workspace установлены `unsafe_code = "deny"`, Clippy `all + pedantic` и
`RUSTFLAGS=-Dwarnings`. `module_name_repetitions` и `must_use_candidate`
разрешены как осознанные noise exceptions. Новые `#[allow]` разрешаются только
на узкой функции с комментарием о причине и ссылкой на benchmark/issue; широкие
crate-level allow не должны использоваться для скрытия производительных или
security diagnostics.

`RUSTDOCFLAGS=-Dwarnings` добавлен в CI для broken intra-doc links и прочих
rustdoc warnings. Public decoder/cache/GPU errors должны иметь actionable
документацию: что означает ошибка, можно ли retry, и является ли вход
attacker-controlled.

## Benchmark и schema gate

В pre-merge выполняются детерминированные Python schema tests и короткий
cache-key benchmark smoke без временных assert-ов: они не притворяются RAW
performance benchmark. `scripts/bench-harness.py` требует
явных CR2/DNG fixtures, сохраняет SHA-256, toolchain, CPU/OS, backend, cache
mode и сырые samples; отсутствие fixture должно быть `skip`, а не нулевое
время. Реальный performance job должен запускаться на labelled hardware и
загружать JSONL + report как artifact.

Минимальные правила для будущего benchmark workflow:

* не смешивать CR2 serial entropy с DNG tile scaling;
* фиксировать `fixture_sha256`, compiler, `workers`, API/device/driver,
  power mode и cache state;
* считать p50/p95/p99 и confidence interval, а outliers помечать вместо
  тихого удаления;
* не сравнивать GPU и CPU без quality-tier и golden-pixel результата;
* regression gate должен иметь baseline того же hardware label, иначе статус
  `exploratory`, не `pass`.

## Dependency security и обновления

`cargo deny` показывает:

* `RUSTSEC-2026-0194` — квадратичная проверка duplicate attributes в
  `quick-xml 0.39.4`;
* `RUSTSEC-2026-0195` — неограниченная аллокация namespace bindings в
  `quick-xml 0.39.4`;
* `RUSTSEC-2026-0192` — `ttf-parser 0.25.1` unmaintained.

Они приходят через build-time Wayland scanner (`winit → wayland-scanner`), а не
через RAW parser, но dependency gate всё равно остаётся красным. Fixed
`quick-xml >= 0.41` нельзя добавить прямой зависимостью: текущий
`wayland-scanner` требует `^0.39`. Разрешённый путь — upstream release/patch с
полной compatibility matrix, а не ручное изменение lockfile. Удаление Wayland
feature — platform decision и должно быть отдельным artifact profile.

Локальный pre-release набор:

```text
cargo metadata --locked --format-version 1
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
RRRAH_KEY_BENCH_SOURCE_BYTES=65536 RRRAH_KEY_BENCH_SOURCE_ITERS=1 \
RRRAH_KEY_BENCH_KEY_ITERS=1000 RRRAH_KEY_BENCH_SAMPLES=3 \
  cargo bench -p rrrah-cache --bench key_hashing --features bench-internals --locked
RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --all-features --no-deps
python3 -m unittest discover -s scripts -p 'test_*.py'
cargo deny check advisories bans licenses sources   # должен fail при текущем lockfile
cargo audit --deny warnings --json                  # должен fail при текущем lockfile
```

## Критический проход

1. **Не делать CI falsely green.** `continue-on-error` используется только для
   сбора отчёта; финальный gate проверяет оба `outcome`. Advisory не добавлен в
   `deny.toml` `ignore`.
2. **Не запускать dependency check дважды.** Оставлен один cargo-deny action;
   отдельный audit дополняет его RustSec database, а не повторяет cargo-deny.
3. **Не принимать latest stable как MSRV.** Stable job и pinned 1.89 job имеют
   разные цели. Обновление `rust-toolchain` не освобождает dependency от
   объявленного `rust-version`.
4. **Не путать тесты с production corpus.** Unit/integration tests не
   доказывают поддержку всех CR2/DNG камер; нужен labelled fixture corpus и
   decoder worker quota из [`FUZZ_HARDENING_AUDIT.md`](FUZZ_HARDENING_AUDIT.md).
5. **Не превращать lint allow в API.** Узкое исключение допустимо только с
   причиной; `-Dwarnings` должен оставаться включённым.
6. **Не скрывать отсутствующие инструменты.** Если scheduled fuzz/coverage/GPU
   runner не установлен, статус должен быть visible `skip` с owner и причиной,
   а не `echo ok`.

## Definition of Done

CI/lint этап можно считать завершённым, когда:

* checks, MSRV, rustdoc, Python schema и fixture jobs зелёные;
* security job либо полностью зелёный после upstream fixes, либо красный с
  сохранённым artifact и назначенным owner/expiry; бессрочный ignore запрещён;
* dependency update проходит `metadata`, test, clippy, audit/deny, RAW corpus,
  shader validation и labelled benchmark;
* lint/toolchain versions и advisory database commit присутствуют в CI
  artifact или summary;
* benchmark numbers не публикуются как индустриальный baseline без manifest,
  uncertainty и одинакового quality tier.
