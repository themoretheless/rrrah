# Validate external WAL palette PCX without constructing discarded pixels

The game-palette path formerly called full PCX decoding solely to validate the file before reading its final 256 RGB entries. It allocated RGBA output and a scanline, decoded the raster again, then discarded both. PCX now shares one validation path between ordinary decoding and palette validation: header, bounds, layout, stride, output-size policy, palette marker, every scanline/RLE packet, exact encoded end and cancellation remain checked. Palette validation returns before output/row allocation and pixel expansion. WAL still reads the external source under its original managed source budget.

Tests: complete texture library **96 passed / 3 ignored / zero failures**. The palette validator succeeds with a zero-byte memory budget, rejects every truncated fixture prefix and trailing raster bytes, and honors cancellation. A real game-directory fixture matches every existing WAL RGBA oracle byte with an exact **2241-byte** budget (1217-byte PCX source + 1024-byte WAL output); one byte less fails and releases all credit. Clone ownership retains 1024 bytes until the last drop. Caller-owned borrowed WAL source is excluded from this low-level measurement; public decode owns/account its source separately.

No wall-clock speedup is claimed. This removes measured unnecessary allocations and a duplicate raster pass; producer variant/GPU/HDR qualification remains separate.

Public decode library: **197 passed / 5 ignored**. Viewer binary: **285 passed / 79 ignored**, including external-palette RAM/swap invalidation, retained prefetch leases and thumbnail stamps. Zero failures in both. Logs: `public.log`, `viewer.log`.
