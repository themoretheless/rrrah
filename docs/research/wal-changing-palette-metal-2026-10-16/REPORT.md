# Current WAL palette/cache path on native Metal

Actual adapter: **Metal Apple M4 Max** (`RRRAH_GPU_BACKEND=metal`).

The existing 2019-case legacy raster pipeline passed after external WAL dependency-key and allocation-free PCX-validation changes: 12114 SDR/half-float GPU readbacks, 2019 RAM hits under full root pressure, 2019 swap writes and reads, blocked DecodeGate during restoration, and final root use zero. This authored/producer corpus includes the two WAL fixtures and many other supported raster subsets. It does not qualify all variants of those formats.

A new explicitly ignored hardware regression reuses the same-size/same-mtime palette-change regression in two modes (RAM and spilled swap). The original independent fixture RGBA oracle supplies the black frame; the red oracle changes only pixels whose original WAL index is zero. Both old held and new decoded frames match their respective oracle through SDR and half-float shader readbacks, 16 actual/reference render outputs in total. The WAL source fingerprint stays unchanged, the cache key changes, no old palette swap entry is read, missing palette fails, held old pixels remain immutable, and final managed use is zero. Hardware test passed in 2.30 seconds.

This tests color-source cache coherence and GPU-buffer preservation, not physical HDR luminance, native window presentation, concurrent file writes, NVIDIA/CUDA, or full format qualification.

Ordinary viewer suite after adding the hardware regression: **285 passed, 80 ignored, zero failures** (`viewer.log`). The added hardware test was executed separately and passed, as recorded above.
