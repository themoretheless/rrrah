# Texture and swap strict Clippy audit

`cargo +stable clippy -p rrrah-texture --all-targets --locked --offline -- -D warnings`: **passed**. This includes current PCX/WAL changes and dependencies checked by this command.

Workspace default-feature all-targets strict Clippy: **failed**, complete log retained. Dedup and swap have independent diagnostics; no global lint exclusions were added.

Segmented swap production code now uses checked u64-to-usize conversions for block lengths/offsets, preserving the existing Length error contract on conversion failure. Public reader/writer/restore methods document their error boundaries. Two test literals gained readable separators. These bounded conversions do not change normal x64 block sizes or wire representation.

Full rrrah-swap tests including integration/doctests: **24 passed / 1 ignored / zero failures**. Production Clippy errors reduced from15 to2; remaining diagnostics concern intentional64KiB stack buffers. Test build still reports17 diagnostics (including those same2), so swap all-targets Clippy is **not green**. Buffers were not moved to unmanaged heap storage and block sizes were not changed without a measured performance/admission design.

The earlier immutable-input workspace result remains evidence for its recorded source hashes. These subsequent localized changes are supported by this new swap test gate; no new whole-workspace/current-release claim is made. Formatting, all-features, pending formats, NVIDIA/CUDA and calibrated physical HDR remain unqualified.
