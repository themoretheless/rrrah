# Linear P3 / BT.2020 PNG cache and Metal transport

2026-10-16. Extends the 15-case PNG/TIFF corpus with two authored RGB16 PNGs declaring cICP full-range linear P3 and BT.2020 (17 total). Source generator specifies three RGBA samples: black, red, white, each alpha 32768/65535.

The test constructs raw sample oracles explicitly, without decoding source pixels or extracting source color metadata. Prepared RGB is checked against fixed matrix coefficients (2e-6 absolute tolerance), without calling the production xy matrix derivation for the expected values. Alpha is checked exactly. Saturated red must retain a red value above one and negative green/blue. These checks cover the selected primaries and samples, not arbitrary ICC color accuracy or all CICP transfer functions.

CPU and actual Metal Apple M4 Max passed. Seventeen pressure RAM hits and seventeen swap writes/reads, zero errors/drops; restores succeed while the foreground DecodeGate is held. Prepared float values are compared bit-for-bit on load and restore. Metal compares 102 full SDR/half-float outputs across reference/decoded/restored frames. Accounted memory returns to zero. Half-alpha compositing can reduce output values below one: this is extended source-gamut preservation and render equivalence, not a physical HDR luminance test.

Commands:
```
CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016 cargo +stable test -p rrrah --bin rrrah --locked --offline png_tiff_metadata_ram_swap_and_blocked_prefetch -- --nocapture
RRRAH_GPU_BACKEND=metal CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016 cargo +stable test -p rrrah --bin rrrah --locked --offline png_tiff_metadata_ram_swap_and_metal -- --ignored --nocapture
```

Adjacent logs and hashes identify this run. Prior 7/15-case evidence remains historical. Shared ICC/CMM oracle limitations apply to those earlier ICC cases. NVIDIA remains deferred; full 100-format qualification and a current broad workspace gate remain incomplete.
