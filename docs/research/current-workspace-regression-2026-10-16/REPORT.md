

### Current workspace library regression (2026-10-16)

After RAW partial/cancel I/O, grayscale ICC regressions, and GPU capacity-aware atlas changes: 1642passed/100ignored/0failed across20library test binaries. Command `cargo +stable test --workspace --lib --offline`. Evidence: `research/current-workspace-regression-2026-10-16/workspace-lib.log`. Not all-targets/all-features or strict Clippy/release qualification; explicit NVIDIA and other ignored gates remain open.
