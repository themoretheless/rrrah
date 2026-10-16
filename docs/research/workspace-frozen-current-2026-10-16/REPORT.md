# Current immutable-input workspace verification

`CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016 cargo +stable test --workspace --all-targets --locked --offline`

Cargo terminal exit **0**. **2367 passed, 294 ignored, zero failures** across 307 summaries (161 empty). Compiler: rustc 1.98.1 (48a229cea 2026-09-01). Full log SHA256 and counts are in `result.json`.

Before the run, SHA256 was recorded for 1101 Rust/TOML source/config files under crates/vendor/.cargo and root Cargo.toml/Cargo.lock/rust-toolchain.toml. After the run, every recorded hash matched and the inventory had no added source files. `inputs.json` preserves the complete inventory. This supersedes the broad preceding integration run whose source changed during execution.

The run includes ordinary libraries, integration tests, examples and benchmark targets under default features. It covers the current WAL dependency key, palette-only PCX validation, cache count preflight, prefetch startup wiring, and layered deflate state/cancellation changes. Explicit native Metal evidence remains in `../wal-changing-palette-metal-2026-10-16/REPORT.md` (2019 cases /12114 readbacks plus dynamic palette replacement).

Limitations: ignored/manual tests were not executed by this command; no all-features, doctests, strict formatting/Clippy, NVIDIA/CUDA hardware or physical HDR calibration claim. Existing unused imports in test modules included by folder_scan/prefetch_scheduler benchmarks remain warnings. Fixture-wide original-producer fidelity and remaining Pending formats are not established by a passing workspace command. Catalogue remains 94 implemented subsets /6 Pending /0 fully qualified.

Performance numbers emitted by benchmark targets are observations under shared host load, not a paired before/after speedup claim. No further broad rerun is needed without new code changes, failures or a concrete uncovered concern.
