# Local renderer changes

Based on hayro 0.7.1, under the accompanying Apache-2.0 or MIT licenses.

- Device-space luminosity soft masks use PDF coefficients (0.30, 0.59,
  0.11), instead of the CSS/SVG coefficients used by vello's luminance mask.
- Alpha masks retain the original path. Calibrated group-space luminosity
  still uses the existing renderer path and is not fully qualified.

Fixtures and decoder regressions live in
`crates/rrrah-decode/src/pdf_device_default_tests.rs`.
