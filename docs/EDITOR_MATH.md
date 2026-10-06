# Production editor math contract

The editor evaluates nodes in this order:

```text
RAW decode -> linearization -> black/bad pixel -> per-photosite WB -> CFA demosaic
-> camera/working matrix -> lens/vignette -> denoise/detail -> local masks
-> exposure/tone -> gamut/ICC/transfer -> export
```

The decoded mosaic cache is immutable and independent of all artistic settings.

## Core equations

Sensor normalization:

`s = (u - black) / max(white - black, 1); s_linear = max(s, 0)`.

Do not clamp values above one before highlight reconstruction. Reject non-finite
metadata and `white <= black`.

Exposure is scene-linear: `E = 2^stops`, `rgb' = E * rgb`.

WB uses a diagonal matrix in camera space. Format backends provide resolved
multiplicative correction gains `g` such that
`diag(g) * AsShotNeutral = [k, k, k]`, conventionally with green gain equal to
one. G1 and G2 gains apply to their respective photosites before demosaic,
so the second green gain is preserved during interpolation. The renderer uploads those gains unchanged. Their common scale defines the
camera-neutral exposure convention; exposure adjustment remains a separate
scene-linear operation.

Do not apply Rec.709 luminance weights to camera-space gains. Those weights are
defined only after the camera-to-linear-sRGB transform and would introduce an
illuminant-dependent exposure shift here.

Color matrix:

`M_rgb_to_cam = M_xyz_to_cam * M_sRGB_to_XYZ`,
`M_cam_to_rgb = inverse(row_normalize(M_rgb_to_cam))`, where row normalization
divides each row by its sum to preserve the camera-neutral white.

Bradford adaptation is `B^-1 * diag(BW_destination / BW_source) * B`.
Build and invert profiles in f64, upload f32 uniforms, reject determinant below
`1e-8` and non-finite coefficients.

Bayer bilinear (at an R sample):

`R=C`, `G=(N+S+E+W)/4`, `B=(NW+NE+SW+SE)/4`.

Use absolute sensor coordinates for CFA parity and clamp borders. MHC 5x5 needs
halo radius two and normalized weights (`abs(sum(weights)-1) < 2^-20`).

For local mask `m`:

`out = (1-m)*base + m*adjusted`,
`local_exposure = base * 2^(m*delta_stops)`.

ACES preview fit:

`f(x) = x*(2.51*x + .03) / (x*(2.43*x + .59) + .14)`.

Production tone mapping should map luminance and preserve chroma. Gamut mapping
reduces chroma in OKLCH/JzAzBz instead of independently clamping channels.

Lens correction uses Brown-Conrady radial terms
`rd = r*(1+k1*r^2+k2*r^4+k3*r^6)` and tangential terms; inverse GPU mesh
reprojection error target is below `0.05 px`.

## Precision invariants

* black maps exactly to zero;
* +1 exposure stop doubles linear radiance;
* constant mosaic remains constant after demosaic;
* matrix round trip error `<1e-5` in f32 runtime and `<1e-10` f64 reference;
* neutral gray remains neutral (`DeltaE00 < 0.5`);
* tile+halo equals full-frame output within `1e-4` linear RGB;
* every kernel has unit DC gain and never emits NaN/Inf;
* 16-bit export uses round-to-nearest-even and clamps only at final quantization.

## Benchmark gates

Measure p50/p95 on the same corpus, cold and warm OS cache: metadata probe,
first RAW frame, complete viewport, full decode, quality refinement, 60 Hz
pan/zoom, export and peak RSS/GPU memory. Suggested desktop targets are
metadata `<20 ms`, warm first frame `<100 ms`, cold first frame `<500 ms`,
frame p95 `<16.7 ms`, and peak decoded RAM `<=1.5x` mosaic plus tile budget.

Before camera conversion, a sensor-clipped Bayer neighborhood whose three
WB-corrected camera channels all reach normalized white is rendered neutral
at the lowest corrected level. This is the preview's conservative treatment
of unrecoverable near-white highlights. It leaves unclipped HDR and chromatic
clipped highlights unchanged; it does not globally clamp camera RGB to one.

For minification with a sensor footprint of at least two photosites along either axis,
integrate complete Bayer cells using their exact overlap with the cropped pixel
footprint. Apply phase-specific black/white/WB and the clipped-highlight rule per
cell, then average linear camera RGB before camera conversion and tone mapping.
Do not average already tone-mapped samples or choose a single stride sample.
