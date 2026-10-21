# EPS graphics reset: public decode, swap and Metal

The independent fixture generator now has an exclusive --resets mode. Test-only Ghostscript renders five8x8 RGB references with pure colors and axis-aligned geometry: initmatrix retains clipping; initgraphics resets clip/color/matrix; grestore recovers clip/color; initgraphics clears a built path; initmatrix retains a built path. Version, binary hash and source hash are retained in the fixture. Reproduce with python3 scripts/generate-eps-pixel-ghostscript-reference.py --resets.

The new ignored hardware test is explicitly executed through decode_image, RasterRamCache count eviction, RasterSwapCache, full-root restore refusal, cancelled restore and successful background retry. All64native pixels per case equal the independently rendered reference. Prepared32x32 SDR and binary16 HDR GPU outputs equal separately constructed golden raster outputs; HDR components are finite. Final root ownership is0 for every case.

The common EPS oracle helper now verifies HDR as well as SDR for its existing cases. Required actualMetal full eps_lifecycle run:10passed/0failed/0ignored on AppleM4Max. Independent groups executed:5import,66gray/RGB,5arc,10clip,5reset cases. The four known midpoint quantization cases remain explicitly excluded from the ramp and ellipse remains excluded from arc; this run does not resolve those compatibility gates. Separate lifecycle tests exercise TTL/unpin and empty-image replacement.

This proves selected native EPS decoding and storage/GPU transport. It is not live navigation/window validation, arbitrary producer EPS, all100formats, physical HDR luminance/calibration or NVIDIA qualification. Production uses native Rust; Ghostscript only generates test references.
