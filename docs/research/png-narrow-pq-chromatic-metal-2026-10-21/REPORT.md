# Narrow PQ PNG: chromatic and translucent Metal qualification

Two authored 64x48 PNG3 RGBA files use cICP 9/16/0/0 at 8/16 bits. Four RGB bands contain BT.2020 red/green/blue and asymmetric codes with below-black/above-white excursions. Four alpha rows contain zero, quarter, half and full opacity, retaining colored samples even when completely transparent.

The standard-library producer independently derives BT.2020-to-sRGB from both sets of xy primaries and D65 using 70-digit Decimal matrix inversion. It applies signed narrow-PQ transfer and writes a 16-cell per-source reference. No production matrix constant or decoder output is used to generate this oracle.

Required Apple M4 Max Metal test checks all 6,144 source pixels against the independent RGB/alpha references. A separately constructed reference raster yields identical complete SDR and float16 HDR readbacks. Additionally all 16 interior cell centers per source are checked against independent linear-over-background arithmetic (0.2% binary16 tolerance, floor 0.02), including the fully transparent colored row. Output alpha is opaque. Zoom=3 compensates the renderer's fixed 32-pixel fit margins at a 64x48 viewport, yielding one output pixel per source pixel.

Native bit patterns survive managed RAM pin/lease protection, eviction into count-one/byte-bounded swap, complete shared-root pressure refusal, cancellation after restore allocation and successful retry. Both Metal attachments remain exact after restore. Every source ends at root=0, one successful swap read/write and zero swap errors.

Initial analytical sampling failed because zoom=1 retained the normal fit margins; test coordinates were outside the intended cell interiors. rejected-unadjusted-fit.log preserves this test-geometry failure. No production rendering change was made. The corrected required-Metal run passes both chromatic-alpha and preceding neutral tests: 2 passed / 0 ignored / 0 failed (metal.log).

This is authored narrow-PQ PNG coverage, not an external producer corpus, physical HDR display, narrow HLG, all PNG variants or NVIDIA qualification.
