# P0 photographic-quality audit

This audit is the acceptance contract for the full-RAW display path. It is
deliberately separate from performance benchmarks: a fast wrong demosaic or a
one-pixel CFA phase shift is not an improvement.

## Invariants

1. Sensor coordinates are authoritative. Crop and orientation are display
   transforms; they must not change CFA phase or the coordinates used to sample
   a black-level grid.
2. For a tile of interior width `T` and demosaic radius `r`, the physical tile
   contains `(T + 2r)^2` samples. The halo is copied from the full mosaic with
   the same border policy as the monolithic path (currently clamped edges).
3. Every displayed sample is normalized before color conversion. The scene-linear
   value keeps highlight headroom; only the lower bound is removed at this stage:

   `L(x,y) = max((raw(x,y) - black(x,y)) /
   max(white(x,y) - black(x,y), 1), 0)`.

   The upper clamp belongs to the final display/export tone and quantization
   pass, never to linearization. A future highlight-reconstruction pass may
   consume values above one.

4. Exposure is scene-linear: `L' = 2^stops * L`. Tone mapping is the final
   bounded preview operation; it must never be applied before WB or the camera
   matrix.
5. A tiled render and a monolithic render must differ by at most one sensor LSB
   before tone mapping for an integer RAW fixture. The same test is performed
   around every tile boundary, not just at the image center.

## Required golden corpus

The quality suite must contain deterministic synthetic fixtures in addition to
camera files:

* four Bayer phases (RGGB, BGGR, GRBG, GBRG), with a neutral ramp and a single
  red/green/blue impulse;
* a 2x2, 4x4 and non-uniform black-level grid;
* a diagonal edge crossing every tile seam;
* saturated highlights and a sub-black patch;
* all eight EXIF orientations;
* a camera matrix with two distinct green planes (G1/G2);
* a DNG fixture with OpcodeList1/2/3, linearization and floating-point samples.

For camera fixtures, store the source hash, decoder version, expected metadata,
and a 16-bit linear reference image. Do not use an embedded JPEG as an oracle.

## Metrics and gates

* Linear-light PSNR >= 60 dB for the fast path against the synthetic oracle.
* CIEDE2000 median/p95 thresholds are tracked against the selected reference
  renderer; the threshold is per camera profile, not a universal constant.
* Seam ratio `mean(abs(error_boundary)) / mean(abs(error_interior)) <= 1.10`.
* Neutral patch chroma error `sqrt(a*^2 + b*^2) <= 1.0` in the balanced profile.
* Orientation corner coordinates must match the CPU transform to < 0.5 pixel.
* Export bytes are deterministic for identical source hash, profile ABI and
  edit graph. Metadata ordering is canonicalized before hashing.

Fast bilinear is a preview tier. A quality tier must publish its demosaic
algorithm and compare only against the matching oracle (for example MHC/RCD
against an MHC/RCD reference), never mix it with the preview score.

## Current implementation status

Implemented and unit-tested:

* canonical EXIF orientation mapping;
* 2x2 Bayer phase validation;
* tile halo extraction and boundary replication;
* padded WebGPU upload rows;
* finite metadata and matrix singularity checks.

Not yet production-complete:

* runtime WGSL/device golden renders;
* arbitrary black-level grids in the shader (the current uniform is a 2x2
  approximation);
* four-plane camera matrices/G1-G2 combination;
* DNG linearization and OpcodeList execution;
* MHC/RCD quality demosaic;
* streaming GPU residency (the atlas currently uploads all decoded tiles).

These are release blockers for a claim of full photographic quality, even if
the fast path is visually useful and the RAW decode itself is lossless.
