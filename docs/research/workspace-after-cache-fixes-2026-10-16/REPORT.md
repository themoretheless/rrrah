# Workspace all-targets follow-up

Command: `CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016 cargo +stable test --workspace --all-targets --locked --offline`.

Terminal Cargo exit **0**: **2365 passed, 293 ignored, zero failed**, 307 test summaries (161 empty). Default features; includes ordinary libraries, integration tests, examples and benchmark targets. Ignored/manual corpora, doctests and all-features configuration are not covered. Compiler, initial source hashes and full logs are retained.

During execution, PCX external-palette validation was optimized and a WAL Metal regression was added. They passed separate texture/public/viewer and explicit native Metal gates. Some test executables were rebuilt while the broad run continued, so this result records a successful integration run, not a single immutable final-source release gate. A subsequent frozen-input run is recorded separately under `workspace-frozen-current-2026-10-16`.

Existing unused imports in test modules included by two benchmark targets remain warnings; strict format/Clippy gates are not proven. NVIDIA/CUDA and physical HDR calibration are not established by this command.
