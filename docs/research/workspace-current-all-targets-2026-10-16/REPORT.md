# Workspace default-feature all-targets gate

2026-10-16. Process session 62377 completed with exit code zero.

```
CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016 cargo +stable test --workspace --all-targets --locked --offline
```

2387 tests passed, zero failed, 295 ignored. 307 test-result summaries, including 161 targets containing no tests. Counts are parsed from complete Cargo output, not inferred from target count. All-targets includes ordinary integration tests and benches; benchmark timing from this shared-host run is not a controlled performance comparison.

Source observation covers 1693 Rust/TOML/WGSL/Cargo.lock files discovered with rg, excluding .git/target/node_modules. The initial broad recursive inventory was explicitly terminated because it traversed build directories; Cargo then continued. The successful source snapshot was captured during compilation, not before command launch. All observed file hashes and inventory match at completion. This is not a full pre-command frozen-checkout claim. Both inventories and exact log are retained.

Two unused-import warnings in gallery test modules were reported through prefetch_scheduler and folder_scan bench compilation (duplicate diagnostics). This test success does not establish strict Clippy or formatting success; those existing failures remain separate. No code was auto-fixed during the run.

Scope: default workspace features, all targets, pinned locked dependencies, offline. Excludes doctests, all-feature combinations and all 295 ignored hardware/manual/corpus tests. Selected actual Metal tests run ordinarily are included, but this does not establish every Metal requirement, NVIDIA/CUDA qualification, physical HDR calibration or complete 100-format correctness. The catalog still records six pending viewer formats and no fully qualified format rows.

The separately completed 17-case PNG/TIFF transport gate is documented in ../png-linear-wide-gamut-metal-2026-10-16/REPORT.md. Source snapshots and test logs are evidence of the checked state, not proof of missing acceptance gates.
