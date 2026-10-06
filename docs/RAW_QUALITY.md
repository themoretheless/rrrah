# RAW quality development

```sh
cargo run -p rrrah -- --raw-quality photo.dng
cargo run -p rrrah -- --raw-quality --tone-curve '0:0,0.5:0.3,1:1' photo.cr3
cargo run -p rrrah -- --inspect --raw-quality --managed-memory-mb 2048 photo.cr3
```

`--raw-quality` selects CPU AHD for Bayer sensors and spatial highlight recovery.
`--no-highlight-recovery` disables recovery and implies quality development.
`--tone-curve` also implies quality development. Supply 2–256 finite points in
[0,1], with increasing x (minimum spacing 1e-6) and nondecreasing y. The
Fritsch–Carlson Hermite spline cannot overshoot segment endpoints; Metal uses
its exact knots, including closely spaced near-black points.

The foreground worker develops immutable sensor samples into scene-linear
RGBA32F. Black levels (including periodic grids), white balance and distinct
second-green gains precede demosaic; calibrated camera conversion preserves
the existing strict color requirements. Crop and orientation follow full-sensor
development. Exposure, ACES display mapping and luminance curve run in the
raster shader. Thumbnail X-Trans uses cheaper color-wise area averaging;
filmstrip thumbnails do not apply the main view's opcodes or curve.

Highlight recovery uses a spatial pyramid and neighbouring chromaticity.
Clipping is measured before gains and white balance, so legitimate HDR values
are not automatically treated as clipped. Unclipped channels remain intact;
fully clipped regions fall back to neutral, not invented scene detail.

## DNG corrections

Supported lists automatically select CPU development, including sensor-cache
hits: the source opcode tags are read again rather than omitted from cached
metadata. The bounded parser accepts at most 1 MiB and 4096 opcodes per list.
Unknown required opcodes, malformed blobs and unsupported stages fail explicitly;
unknown optional opcodes are skipped.

| Stage | Supported operations | Current boundaries |
| --- | --- | --- |
| OpcodeList1 | FixBadPixelsConstant, FixBadPixelsList | Bayer phase must match; invalid areas rejected; combination with LinearizationTable rejected |
| OpcodeList2 | GainMap | Single-plane CFA maps; finite positive gains; full-sensor ActiveArea required |
| OpcodeList3 | FixVignetteRadial | Full-sensor ActiveArea required |

GainMap samples pixel-centre coordinates. Vignette normalization uses the
farthest sensor pixel from the supplied centre. Each operation clamps its
output to the stage's DNG logical range. Defect correction reads an immutable
snapshot and rejects defects without usable same-phase neighbours.

## X-Trans and qualification

6×6 RGB X-Trans uses directional color-difference interpolation. DNG supports
this CFA with the existing supported storage formats. RAF obtains the exact
pattern from directory tag 0x131 and supports 36-site black levels. Missing or
invalid authoritative patterns fail; proprietary compressed RAF and rotated
Super-CCD remain unsupported. Synthetic X-Trans decode/development and thumbnail
tests pass; no real X-Trans camera corpus or independent quality oracle has yet
qualified this path.

AHD has a sequential tile working set and cancellation between tiles; X-Trans
checks cancellation by row. Other development stages check cancellation at
stage boundaries. Scratch and output are admitted before allocation using a
conservative 64 bytes per sensor pixel plus 8 MiB; output reservations survive
handoff. This accounts for managed buffers, not physical RSS or driver memory.
The existing raster upload limits still apply.

One local dev-profile EOS R8 run developed the sensor into 6000×4000 RGBA32F
in 6.88 seconds, with 1,691,029,568 bytes peak managed reservations under a
2048 MiB limit. This is a single execution, not a throughput guarantee. Ordinary
Bayer viewing retains its GPU sensor path unless quality settings or supported
opcodes require CPU development.

## Provenance and checks

Adapted AHD, X-Trans, highlights, opcode structures and monotone spline from
[storytold/lightcraft](https://github.com/storytold/lightcraft/tree/294012742e277d95e59db0072c88bfd3d296f6cc)
under Apache-2.0. Attribution is in NOTICE and the license in
third_party/lightcraft/LICENSE-APACHE. Integration changes include immutable
samples, bounded admission/parsing, calibrated color, exact GPU Hermite curves,
sequential tiles and DNG coordinate corrections.

```sh
cargo test -p rrrah-core -p rrrah-decode -p rrrah --locked
RRRAH_GPU_BACKEND=metal cargo test -p rrrah-gpu --test raster_readback --locked
```

Coverage includes measured CFA sites, tile boundaries, spatial clipping masks,
monotone curves, opcode wire attacks and analytic corrections, source-cache hits,
RAF pattern phase, crop/orientation, memory refusal/release, and GPU exposure,
color and curve readback. Real CR3 inspect development was also exercised.

## Explicit scene-linear RAW output

`RawRenderer::set_output_mode(queue, RawOutputMode::SceneLinear)` bypasses
ACES display mapping and preserves signed scene-linear sRGB after camera color
conversion and exposure. It accepts only Rgba16Float/Rgba32Float target formats;
unorm/sRGB attachment refusal leaves the mode unchanged. DisplaySdr remains the
default. The linear Bayer path also bypasses SDR clipped-highlight neutral
reconstruction, preserving individual HDR camera channels. RGBE remains four
planes through its 3x4 color transform. The mode survives image upload and view
updates; callers can switch back to DisplaySdr explicitly.

`/tmp/rrrah-raw-linear-metal.log` confirms actual Metal Apple M4 Max Rgba32Float
readback. An independently authored neutral-transform Bayer case retains
[12,6,3,1] under one-stop exposure; RGBE matches the independent LibRaw 3x4
transform with bright and negative components within 2e-5. Mode round trips are
exact and tracked GPU atlas usage returns to zero. Float SDR assertions allow
1e-6 gamut-mapping roundoff. `/tmp/rrrah-raw-linear-regression.log` records 65
existing shader/RAW/color/RGBE checks passed.

This is an intermediate/render API, not a physical HDR presentation mode.
The current window keeps its SDR surface. Display transfer, target primaries,
reference white/peak luminance and actual HDR display qualification remain open.
Rgba16Float numerical range/precision and real-image HDR output still need
separate readback evidence.

### RAW RGBA16Float readback qualification

`/tmp/rrrah-raw-half-linear-metal.log` extends the signed linear RAW readback
to both Rgba16Float and Rgba32Float on actual Metal Apple M4 Max. Both formats
preserve above-one Bayer channels, negative RGBE color, opaque alpha and exact
SDR/linear mode round trips for the authored fixtures. Atlas credit returns to
zero after each renderer is dropped.

The binary16 reference expansion independently checks signed zero, normals,
subnormal 2^-24 and finite maximum 65504. Hardware numeric comparison permits
one binary16 ULP at the expected magnitude plus the independently checked f32
shader bound (2e-5). For the almost-exact Bayer red=12, Metal Rgba16Float output
is 11.9921875, the adjacent representable value; Rgba32Float retains its finer
precision. This is bounded quantization evidence, not an exact-bit claim for
arbitrary color transforms. The tested values are within binary16 range;
extreme exposure/overflow handling, real-image float-frame color, GPU buffer
footprint for caller-owned intermediate attachments and physical HDR presentation
remain outside this check.
