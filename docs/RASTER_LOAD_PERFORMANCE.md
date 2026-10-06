# Raster loading performance

Priority: time to the first displayed image and latency when changing selected frames/mips, with retained pixel precision and color/alpha correctness.

## Measured CPU preparation

2026-10-05, Apple M4 Max, identical optimized development build profile. Three paired rounds alternate baseline/optimized order; each process uses three warmups and fifteen measured iterations. Values below are medians of the three round p50 values. File reading is included, with warm filesystem cache and no decoded-image cache. Pixel fingerprints are outside the timed interval. Every baseline/optimized display BLAKE3 matches exactly.

| Synthetic input | Selection | Before ms | After ms | Speedup |
|---|---:|---:|---:|---:|
| frames.apng | 0 | 27.233 | 0.713 | 38.18x |
| frames.apng | 23 | 27.285 | 0.714 | 38.19x |
| levels.ktx2 | 0 | 21.249 | 11.916 | 1.78x |
| levels.ktx2 | 8 | 4.019 | 0.012 | 337.31x |
| linear.ktx2 | 0 | 11.165 | 7.673 | 1.46x |
| still.png | 0 | 14.491 | 6.054 | 2.39x |

The APNG has 24 independent full-canvas SOURCE frames at 256x256. The ordinary PNG and KTX2 base image are 1024x1024; KTX2 contains 11 DEFLATE mips, and selected mip 8 is 4x4. The linear KTX2 is a 1024x1024 float32 HDR surface. These constant-color authored inputs isolate mechanisms and do not establish throughput for photographic data or every format.

Changes: cache the exact 256-entry sRGB conversion table; reuse validated linear float storage; decode only the selected KTX2 mip; borrow APNG compressed storage and decode only the dependency chain through the selected frame. Full-canvas SOURCE frames can restart that chain unless their PREVIOUS disposal requires earlier state. All container ranges/control fields and APNG CRCs remain checked. Unneeded compressed pixel streams are validated on selection.

This measures CPU-ready pixels, excluding process startup, cold disk access, GPU upload and first presentation. Next acceptance measurement should time file selection through first GPU presentation on real large files; GPU readback correctness alone is not a latency measurement.

Reproduce inputs with `python3 scripts/generate-load-speed-fixtures.py`; build `cargo build -p rrrah-decode --example raster_load_bench`; run `target/debug/examples/raster_load_bench FILE INDEX 15`. Preserve separate baseline/optimized executables before pairing. Local raw p50/p95 results: `target/bench/raster-load/paired.jsonl`.

Validation: core 50 unit + 3 precision tests; decoder 471 passed, 3 ignored; application 59 passed; GPU raster readback 3 passed. Formatting and diff whitespace checks passed.

## Selected Aseprite CPU preparation

The same CC0 speed-input generator now creates 24 independent full-canvas 1024x1024 RGBA Aseprite cels. On the same Apple M4 Max, three rounds alternate selection order 0/23 and use three warmups plus fifteen timed iterations. Median round-p50 CPU-ready time is 5.927 ms for frame 0 and 5.891 ms for frame 23. Display fingerprints are stable within each selection and differ between selections, as expected from their authored colors. This is an absolute warm read/decode/prepare measurement, not a before/after speedup or a cold/GPU first-presentation result. Constant-color compressed inputs do not establish photographic/worst-case throughput. Local results: `target/bench/raster-load/aseprite.jsonl`.

The parser inventories all frame/chunk bounds, but only inflates selected visible image storage. Linked cels resolve to their source bytes without retaining decoded other-frame canvases. A first full-canvas/full-opacity RGBA cel inflates directly into the output canvas. A regression corrupts an unrelated future cel: frame 0 still renders, selection of that corrupted cel fails, and a structural chunk-range corruption rejects even on frame 0.

## Shared routing header

The common image router now opens and reads the bounded 256-byte prefix once, sharing it between sensor signature detection and raster signature detection. Previously a non-TIFF raster required two separate opens and prefix reads before decoding. Strong sensor signatures, TIFF sensor-tag inspection and raster signatures overriding misleading RAW extensions retain their routing rules. Cancellation is checked before opening and after the prefix read. This removes redundant routing I/O; no end-to-end latency improvement has been measured for this change.
