# Native EPS dash state and stroking

The former closed-corner discrepancy is resolved against its unchanged pixel
reference. Its strict comparison is now an ordinary automatic test with no
case exclusion or ignore. Sixteen independent pixel scenarios match, up from
the previous seven of eight. This is the scope of the captured evidence, not a
claim of full PostScript/AI or every dash/device configuration.

## State, geometry and ownership

Native Rust setdash/currentdash use a validated 32-entry maximum numeric pattern.
Negative/nonfinite/all-zero nonempty arrays refuse; empty patterns are solid.
Odd patterns preserve on/off parity across repetitions and phase restarts per
subpath. Original VM array handles are kept separately from cached numeric
values: mutating the input array affects currentdash queries but not the geometry
captured by setdash. Graphics saves, VM rollback and initgraphics preserve/reset
both as appropriate. Scene stroking owns its numeric pattern and does not need
the VM array after execution.

Counting/writing passes use admitted buffers and a shared work/cancellation
allowance. Corners inside on-runs survive subdivision, and closed seam runs join.
Affine-aware flattening provides the curve geometry. Hairlines measure dash
distance in stroke-user coordinates but retain a one-raster-pixel device pen.
Singular dashed-hairline metrics refuse rather than substituting device-space
distance. Fixed inline pattern metadata increases paint/save-stack backing
stride, including for solid styles; the root budget charges that capacity. No
allocator/RSS or speed improvement is claimed.

## The resolved terminal fragment

At an on-run ending exactly at an interior vertex, the reference retains a
zero-length on-fragment with the next segment's tangent. Its butt-cap rendering
policy gives that oriented fragment a one-dominant-raster-pixel visible length.
The old subdivision advanced phase at the vertex and lost it. The raster path
now retains it without changing logical dash distance. Mapping the tangent into
raster space determines the length; linewidth and coordinate translation do not
substitute for that metric. The public mathematical centerline subdivision
remains independent of this device policy.

The [witness](closed-corner.json) preserves the old four difference coordinates
and native map, the unchanged RGB reference hash and the current zero-difference
result. Independent follow-up controls include zero on-elements, repeated and
collinear vertices, affine scaling, translation and a wider pen. A native
analytic control verifies the diagonal/nonuniform one-device-pixel metric.
No arbitrary epsilon or width-dependent extension is used to fit the old image.

The behavior was isolated using separate Ghostscript strokepath/path-coordinate
and pixel probes. The test-only reference implementation is pinned to
[Ghostscript 10.05.1](https://github.com/ArtifexSoftware/ghostpdl/tree/7f64bc848f6b67ae204182e6e088eec110061490).
Production contains native Rust subdivision/outline code, not that interpreter
or its C source. Language contract reference:
[Adobe PLRM3](https://pdfa.org/norm-refs/PLRM.pdf), setdash/currentdash and stroke.

## Executed gates

- 20 independent top-level state/error traces match Ghostscript 10.05.1, including
  array aliasing, snapshots, phase, zero values and typed refusals.
- All 16 pixel scenarios match every RGBA component: even/odd/negative phase,
  corners, closed paths, affine spacing, cached-array mutation, device-visible
  zero fragments and wider pens. The oracle explicitly uses false setstrokeadjust
  and GraphicsAlphaBits=1; this does not qualify its default stroke-adjustment
  profile or arbitrary curved/antialiased content.
- Native geometry controls cover exact quota/one-byte-short refusal, last-owner
  credit, every exposed cancellation point and all insufficient work allowances,
  including temporary dash/outline storage. An analytic quarter-circle control
  checks user-space arc length under a nonuniform transform; a separate hairline
  control checks device pen width and dash spacing.
- The full vector regression passes 284 tests with 11 previously ignored gates.
  Viewer regression passes 364 with 109 ignored. The former new ignored dash gate
  is gone; it now participates in the automatic pixel test.
- All 16 independent cases pass native decode/RAM/pressure/cancellation/disk
  restore and exact SDR/HDR offscreen rendering on Metal Apple M4 Max, ending at
  root credit zero. [Hardware log](metal.log). This is not physical-HDR-display,
  NVIDIA, every HDR value or general AI qualification.
- The authored dash-prolog.ai also passes the public decode_raster file route:
  8x8, image 0 of 1, all 64 native pixels composited over white match the frozen
  reference. [Route evidence](ai-file-route.json). This does not qualify arbitrary AI.
- Production check, public decoder example build, formatting and whitespace
  checks pass. [Results and source pins](result.json). Repeated full regression
  logs are represented by terminal summaries and hashes instead of duplicates.

The oracle generator is scripts/generate-eps-dash-reference.py and requires
Docker. Source, generator, image/executable and output hashes are pinned in the
reference fixture and result. The same external original Illustrator Tiger
loads setdash and still refuses at currentflat, instruction 142:
[diagnostic](original-tiger.json). Artwork remains outside the checkout.
Native flatness, further Illustrator operators, full AI support and the original
100-format/cache/GPU objective remain open. Format counts do not change.
