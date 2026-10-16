# All eight PNG EXIF orientations through RAM, swap and Metal

2026-10-16. Extends the seven-case PNG/TIFF metadata transport corpus with eight independently authored PNG orientation cases (15 total).

The test builds 2x3 RGBA PNGs with six explicit colors/alpha values, sRGB declaration, a valid pre-IDAT eXIf TIFF Orientation entry, and a single stored DEFLATE block. PNG CRC and zlib Adler checksums are computed in the test fixture builder. Explicit six-pixel permutations are independent of image orientation implementation. Orientations 1–4 retain 2x3; 5–8 require 3x2. Alpha includes zero, intermediate values and opaque pixels. Exact prepared float pixel comparison includes all samples, including transparent RGB.

CPU test passed; actual Metal Apple M4 Max test passed. Fifteen exact RAM hits under full available root-budget pressure, fifteen swap writes and reads, zero errors/drops, background restores while the foreground DecodeGate is held. Ninety full SDR/half-float reference/decoded/restored output comparisons. Root accounted memory returns to zero after all owners drop.

Commands are the same as the preceding [seven-case report](../png-tiff-metadata-metal-2026-10-16/REPORT.md). Adjacent logs record this 15-case run and test source SHA256. Previous seven-case evidence is retained without rewriting it.

This proves bounded authored orientation and transport behavior. The ICC metadata/CMM oracle remains shared for the other seven cases; this is not independent ICC accuracy, arbitrary producer PNG support, full codec heap accounting, physical HDR calibration or NVIDIA qualification. No broad workspace gate was run for this test-only change.
