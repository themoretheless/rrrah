# Animation playback clock, swap and actual Metal

Extended the real animation/cache playback qualification with an explicitly enabled Metal test. GIF disposal-3 (three total plays), APNG composition and WebP animation (infinite) each execute twelve selected-frame states driven by the production playback clock. The test checks timing/repeat metadata and the finite GIF stop, forces eviction with RAM count=1, waits for swap writes, and counts decodes/restores. Every state renders through actual Metal to SDR and RGBA16Float targets; restored outputs compare exactly against that frame's first-pass output. Both output paths must distinguish frames for each source.

Actual adapter: Metal Apple M4 Max. Result: 36 states, 12 decodes, 24 swap reads, 72 readbacks. Root memory usage returns to zero after each source. Full parallel viewer suite: 273 passed / 73 ignored; the new hardware test is among those ignored by default and was separately enabled and passed.

This proves frame stability through selected-image decoding, prepared cache, swap and GPU transport for these three fixtures. It does not establish independent color accuracy, physical HDR display output, native window presentation timing, every producer variant, or NVIDIA/CUDA operation.
