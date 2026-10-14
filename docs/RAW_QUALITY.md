# RAW quality development

```sh
cargo run -p rrrah -- --raw-quality photo.dng
cargo run -p rrrah -- --demosaic amaze-vng --false-color-passes 2 photo.cr3
cargo run -p rrrah -- --demosaic rcd-lmmse --tone-curve '0:0,0.5:0.3,1:1' photo.cr3
cargo run -p rrrah -- --demosaic xtrans-markesteijn3-pass photo.raf
cargo run -p rrrah -- --gpu-demosaic rcd photo.dng
cargo run -p rrrah -- --inspect --demosaic amaze --false-color-passes 1 --managed-memory-mb 2048 photo.cr3
```

`--raw-quality` selects CPU quality development (defaulting to AHD on 2×2 Bayer,
directional colour-difference on 6×6 X-Trans, sub-cell edge-directed on 4×4 Quad /
6×6 Nona / 8×8 Hexadeca Bayer, and universal edge-aware interpolation on
arbitrary/complementary CFAs) and spatial highlight recovery.
`--demosaic <auto|nearest|bilinear|smooth-hue|laroche-prescott|hamilton-adams|mhc|ppg|vng|rcd|ahd|lmmse|dcb|igv|amaze|amaze-vng|amaze-lmmse|rcd-vng|rcd-lmmse|dcb-vng|xtrans-directional|xtrans-markesteijn|xtrans-markesteijn3-pass|xtrans-dual|quad-bayer|nona-bayer|hexadeca-bayer|generic>`
explicitly selects the CPU demosaicing algorithm (28 total variants/modes) and implies `--raw-quality`.
`--false-color-passes <0..4>` enables 1–4 iterations of 3×3 median colour-difference
false-colour and zipper suppression (preserving measured CFA channel values bit-for-bit)
and implies `--raw-quality`.
`--gpu-demosaic <bilinear|smooth-hue|malvar-he-cutler|directional|vng|rcd>` selects the GPU fragment
and compute shader demosaicing kernel for fast 2×2 Bayer rendering (`SmoothHue`, `MalvarHeCutler`,
`Directional`, and `Vng` use 5×5 stencils and automatically raise `tile_halo` to `2`; `Rcd` uses a
7×7 stencil and raises `tile_halo` to `3` so tile boundaries never clip the filter footprint).
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

6×6 RGB X-Trans uses directional color-difference interpolation, 1-pass or 3-pass
Markesteijn CIELab homogeneity-directed interpolation, or contrast-blended
`XTransDual`. DNG supports this CFA with the existing supported storage formats.
RAF obtains the exact pattern from directory tag 0x131 and supports 36-site black
levels. Missing or invalid authoritative patterns fail; proprietary compressed RAF
and rotated Super-CCD remain unsupported. Synthetic X-Trans decode/development and
thumbnail tests pass; no real X-Trans camera corpus or independent quality oracle
has yet qualified this path.

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

## Exhaustive CPU & GPU demosaicing library

`rrrah_core` (`DemosaicAlgorithm`, `DemosaicConfig`, `demosaic_f32`, `demosaic_u16`,
`demosaic_bayer_superpixel`) and `rrrah_gpu` (`GpuDemosaicAlgorithm`,
`BayerDemosaicCompute`) provide an exhaustive, standalone and integrated
demosaicing library covering 28 CPU algorithms/modes and 6 GPU algorithms across
all periodic CFA layouts:

| Algorithm | Target CFA | Key characteristics |
| --- | --- | --- |
| `Nearest` (CPU) | 2×2 Bayer (all 4 phases) | Zero-blur 2×2 quad replication for sub-millisecond inspection |
| `Bilinear` (CPU & GPU) | 2×2 Bayer (all 4 phases) | Fast 3×3 axial/diagonal linear interpolation |
| `SmoothHue` (CPU & GPU) | 2×2 Bayer (all 4 phases) | Cok (1987) constant-hue-ratio (`R/G`, `B/G`) interpolation |
| `LarochePrescott` (CPU) | 2×2 Bayer (all 4 phases) | Laroche–Prescott (1994) second-derivative edge classifier + colour-difference R/B |
| `HamiltonAdams` (`Directional` on GPU) | 2×2 Bayer (all 4 phases) | Hamilton–Adams (1997) 5×5 first+second-order Laplacian gradient-corrected interpolation |
| `MalvarHeCutler` (`mhc`, CPU & GPU) | 2×2 Bayer (all 4 phases) | Malvar–He–Cutler (2004) 5×5 gradient-corrected linear filter with local range clamping |
| `Ppg` (CPU) | 2×2 Bayer (all 4 phases) | Patterned Pixel Grouping: 4-direction cardinal gradients + Hamilton–Adams R/B at G + diagonal R/B at B/R |
| `Vng` (CPU & GPU) | 2×2 Bayer (all 4 phases) | Variable Number of Gradients: 8-direction 5×5 compass gradients with adaptive threshold subset accumulation |
| `Rcd` (CPU & GPU) | 2×2 Bayer (all 4 phases) | Ratio-Corrected Demosaicing: low-pass directional ratios + second-derivative colour-difference & diagonal weights |
| `Ahd` (CPU) | 2×2 Bayer (all 4 phases) | Adaptive Homogeneity-Directed: H/V directional interpolation + CIELab 3×3 homogeneity selection |
| `Lmmse` (CPU) | 2×2 Bayer (all 4 phases) | Linear Minimum Mean-Square Error (Zhang–Wu 2005): directional difference + 5-tap smoothing + empirical MMSE variance blending |
| `Dcb` (CPU) | 2×2 Bayer (all 4 phases) | Jacek Gozdz DCB: directional green + iterative colour-difference refinement + local gradient arbitrations |
| `Igv` (CPU) | 2×2 Bayer (all 4 phases) | Ingo Weyrich IGV: anisotropic Gaussian variance-weighted colour-difference solver for high-ISO/noisy sensors |
| `Amaze` (CPU) | 2×2 Bayer (all 4 phases) | Emil Martinec AMaZE: multi-scale cardinal + diagonal sub-lattice colour-difference variance solver |
| `AmazeVng`, `AmazeLmmse`, `RcdVng`, `RcdLmmse`, `DcbVng` (CPU) | 2×2 Bayer (all 4 phases) | Contrast-guided dual demosaicing: S-curve local contrast mask blends ultra-sharp edge solver with smooth low-variance solver on flat/noisy areas |
| `XTransDirectional` (CPU) | 6×6 X-Trans | Inverse-gradient weighted 5×5 colour-difference solver |
| `XTransMarkesteijn` (CPU) | 6×6 X-Trans | 1-pass directional green candidates + CIELab homogeneity selection |
| `XTransMarkesteijn3Pass` (CPU) | 6×6 X-Trans | 3-pass iterative green & colour-difference refinement + 5×5 CIELab homogeneity selection |
| `XTransDual` (CPU) | 6×6 X-Trans | Contrast-guided dual X-Trans blending `XTransMarkesteijn3Pass` on edges with `XTransDirectional` on flat areas |
| `QuadBayer` (CPU) | 4×4 Quad Bayer / TetraCell | Exploits 2×2 homogeneous sub-cells for intra-cluster detail and inter-cluster directional colour differences |
| `NonaBayer` (CPU) | 6×6 Nona Bayer / Nonacell | Exploits 3×3 homogeneous sub-clusters (108 MP / 200 MP sensors) with radius-4 directional colour differences |
| `HexadecaBayer` (CPU) | 8×8 Hexadeca Bayer / ChameleonCell | Exploits 4×4 homogeneous sub-clusters (16-in-1 sensors) with radius-5 directional colour differences |
| `GenericCfa` (CPU) | Any periodic CFA (`RGB`, `RGBW`, `RGBE`, `CYGM`) | Universal inverse-distance/gradient kernel with panchromatic `RGBW` luminance fusion and complementary (`CYGM` → linear `RGB`) decoding |

- **Standalone public demosaic API (`rrrah_core`)**: `demosaic_f32`, `demosaic_u16`,
  and `demosaic_bayer_superpixel` operate directly on raw slices (`&[f32]` / `&[u16]`)
  and `CfaPattern` with `DemosaicConfig` and optional `MemoryBudget` admission,
  without requiring `DecodedMosaic` or camera matrices.
- **Pre- and post-demosaic conditioning**:
  - `equilibrate_bayer_greens`: equalizes G1/G2 crosstalk imbalance on 2×2 Bayer sensors before demosaicing while preserving real edges.
  - `suppress_cfa_impulse_noise`: removes isolated salt-and-pepper hot-pixel spikes on same-colour CFA neighbours within a 5×5 window.
  - `suppress_false_color`: 1–4 configurable passes of 3×3 median filtering on `(R - G)` and `(B - G)` colour differences while re-pinning the measured CFA channel bit-for-bit.
  - `suppress_chromatic_fringe`: attenuates narrow purple/magenta/cyan axial/lateral fringes along steep luminance edges while keeping measured CFA sites exact.
- **Managed GPU compute (`BayerDemosaicCompute`)**: Standalone and resident WGSL
  `@workgroup_size(8, 8)` compute pipeline supporting all 6 GPU algorithms (`Bilinear`,
  `SmoothHue`, `MalvarHeCutler`, `Directional`, `Vng`, `Rcd`) with pre-allocation
  `MemoryBudget` admission (`execute_resident`, `execute_managed`, `execute_managed_with_cancel`).


