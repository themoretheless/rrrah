# Current decoder and app ordinary-test gate

2026-10-16. Command exited zero:
```
CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016 cargo +stable test -p rrrah-decode -p rrrah --lib --bins --locked --offline
```

App: 286 passed, 81 ignored, zero failures. Decoder: 216 passed, 5 ignored, zero failures. Total: 502 passed, 86 ignored. Includes the 17-case PNG/TIFF RAM/swap regression; the separate actual Metal gate remains documented in the linear wide-gamut report.

This is default-feature library/binary unit tests for two packages. It excludes integration-test targets, examples, doctests, all-feature builds, other workspace crates, and ignored hardware/corpus gates. It is not complete release or 100-format qualification. Existing formatting/strict workspace lint failures remain separate.

A source snapshot was captured during compilation, not before command launch. Its scope is crates plus vendor/image and vendor/wgpu-hal Rust files and Cargo.toml. Other shared-host Cargo processes were observed; no frozen-worktree claim is made. Changed paths between this mid-compilation observation and completion: 0. See source-observation.json for exact inventory and differences.
