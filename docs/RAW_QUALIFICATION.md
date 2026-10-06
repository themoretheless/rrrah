# Native RAW qualification

Build with `cargo build -p rrrah-decode --example raw_fixture_dump --locked`, then run
`python3 scripts/qualify-raw-corpus.py /path/to/corpus --fetch --report /tmp/raw-report.json`.
Add `--include-local-cr3` to require both repository EOS R8 fixtures as well.
The manifest in `tests/fixtures/raw-color-corpus.json` pins nine public CC0 files
from raw.pixls.us by SHA256 and two locally supplied CR3 files. The local files
have no asserted redistribution license and are not downloaded by the script.

All eleven cases match independent full-sensor samples from LibRaw 0.22.2,
camera white balance and final camera-to-linear-sRGB transforms. The cases are
Pentax K-x DNG/PEF, Nikon D750 lossless NEF, Canon EOS 80D CR2, Sony a7 III
compressed/uncompressed ARW, Olympus E-M5 ORF, Fuji X-A2 RAF, Panasonic GH2 RW2,
and two Canon EOS R8 CR3 captures. This qualifies these exact files and modes.
Unsupported metadata layouts and missing color calibration remain explicit errors.

The independent oracle is `scripts/raw-fixture-oracle.cpp`. LibRaw is an external
qualification tool, never a production decoder dependency. The verifier checks
source hashes, complete sensor hashes, dimensions, CFA, WB and all nine final
matrix coefficients; it does not regenerate references or skip unavailable cases.
LibRaw's `cam_xyz` can differ from its final `rgb_cam`: the latter is the display
transform reference, including the embedded K-x DNG profile.

## Metadata and normalization policy

The manifest also pins source-tag crop/black values and the declared white-level
policy. Source crops use full-sensor coordinates: Sony [12,12,6000,4000], Canon
80D [276,46,6000,4000], Pentax [10,10,4288,2848], Fuji [28,12,4896,3264], Olympus
[8,8,4608,3456], Panasonic [8,4,4608,3456]. LibRaw's initial active area is a
different contract and is not used as the file's default-crop oracle.

Explicit file white/linearity endpoints take priority. Panasonic's three RGB
LinearityLimit values are preserved independently; partial, zero or overflowing
sets reject display. GH2 declares 3967 while LibRaw uses 3989. Without a supported
file endpoint, normalization uses the declared sample precision's full code range
(e.g. Olympus 12-bit 4095 versus LibRaw's empirical 4065). These are explicit
normalization policies, not measurements of physical sensor saturation. Black
metadata can likewise differ by one code from LibRaw's combined channel offsets.

EOS R8 black levels and all four WB gains come from each capture's CTMD ColorData.
Gains are [R/G1, 1, B/G1, G2/G1]. NormalWhiteLevel and SpecularWhiteLevel are retained
and range-validated separately. NormalWhiteLevel 12735 is no longer substituted
for the 14-bit code range 16383. The two local fixtures also pass the opt-in CR3
full-pixel regression against independent oracle files.

## Rendering qualification

`cargo test -p rrrah-gpu --test camera_color_readback --locked -- --nocapture`
requires a real GPU and fails without one. It checks the nine public profiles
against independent final transforms on neutral, colored and shadow fields,
including CFA phase, black normalization, per-photosite G1/G2 WB, camera conversion,
tone mapping and sRGB encoding. CPU color thumbnails use the same contracts.
The gate ran on Metal Apple M4 Max; the constant-field display tolerance is two
8-bit code values. `RRRAH_GPU_OPTIONAL=1` allows an explicitly logged skip for
ordinary CI without a GPU; such a run does not qualify GPU behavior.

CPU thumbnails and GPU minification integrate complete Bayer cells across each
pixel's sensor footprint with rectangle-overlap weights. They average linear
camera RGB before the camera matrix and tone mapping, avoiding the previous
single-sample stride aliasing. A checkerboard fixture verifies the independent
linear mean through both paths. Crop and orientation remain part of the footprint.
Magnified views retain bilinear Bayer interpolation.

EXIF orientation is independently checked against CIPA DC-008-2019 Figure 12
(https://www.cipa.jp/std/documents/e/DC-X008-Translation-2019-E.pdf). Tags 6 and 8
were reversed in CPU and WGSL inverse transforms; both are corrected.
`cargo test -p rrrah-gpu --test raw_orientation_readback --locked` checks all eight
fixed corner permutations through a non-square crop in GPU and thumbnails.

Real Canon/Sony previews exposed pink blown skies missed by below-white fields.
The preview now neutralizes a sensor-clipped neighborhood only when all three
WB-corrected camera channels reach white, at their lowest corrected intensity.
Unclipped HDR and chromatic clipped highlights retain color. GPU and CPU regression
fixtures exercise both cases; real previews confirm removal of large pink regions.
Area filtering also removes the former strongly aliased thumbnail noise.

These checks establish sensor decoding, supported metadata interpretation and
specified rendering behavior. Bilinear demosaicing can still produce fine-edge
false color. The baseline camera profiles and preview tone curve do not reproduce
a camera JPEG's creative look or certify every camera, illuminant, display or
physical saturation threshold.

## Managed source/output qualification

On 2026-10-05 the current native dumper was rebuilt with `--locked` and all eleven
pinned files passed the same independent sensor/color/metadata checks with a
512 MiB managed budget:

```sh
python3 scripts/qualify-raw-corpus.py /tmp/rrrah-raw-corpus --include-local-cr3 --memory-mb 512 --report /tmp/rrrah-managed-raw-qualification.json
```

The dumper rejects unmanaged decoder output when a budget is requested and emits
limit, used, peak and pixel-capacity bytes in the JSON metadata. The verifier
requires managed ownership, exact retained pixel capacity, peak within limit and
agreement with the requested budget. Whole sensor and independent color oracle
checks remain enabled; no reference was regenerated. Managed peak reservations
ranged from 39,164,316 to 98,683,904 bytes. The source reservation is released on
return; the result still holds its pixel reservation. These counters exclude
codec scratch, metadata, allocator bookkeeping and GPU memory; they are not RSS.
The report records the native executable SHA256 and per-case memory evidence.

## NRW P7800 integrated corpus qualification

The required manifest now includes raw.pixls.us CC0 object 1425 (COOLPIX
P7800 NRW). Full sensor bytes/digest, CFA, WB, camera-to-linear-sRGB transform,
black/white levels and crop are pinned to independent LibRaw 0.22.2 output.
The existing qualifier downloads this case with `--fetch` and verifies its
source hash; it does not regenerate its reference.

All ten public cases pass with `--memory-mb 128`. All twelve cases including
the two local EOS R8 CR3 fixtures pass with `--include-local-cr3 --memory-mb 256`.
Reports with native executable hash and per-case ownership/peak metadata are
in `target/qualification/nrw/full-corpus-report.json` and
`target/qualification/nrw/full-corpus-with-cr3-report.json`; logs are
`/tmp/rrrah-nrw-full-corpus.log` and `/tmp/rrrah-nrw-full-corpus-cr3.log`.
These runs qualify the pinned RAW contracts, not all camera variants, physical
photographic color accuracy, CUDA/NVIDIA or HDR display.

The CC0 COOLPIX P7700 sample (raw.pixls.us object 5495) is now required too.
Its source hash and independent LibRaw sensor/color reference are pinned in
the shared manifest. The explicit mode-7 decoder/level profile admits P7700
as well as P7800; unknown models still reject. All 13 corpus cases including
local CR3 pass with managed 256 MiB admission, and all 563 decoder tests pass
(four ignored). Evidence: `target/qualification/nrw/p7700-full-corpus-report.json`,
`/tmp/rrrah-p7700-corpus.log`, `/tmp/rrrah-p7700-tests.log`. P7700 GPU display
is not yet qualified; the existing real GPU regression still covers P7800.

NRW admission now checks the exact qualified model before generic NEF metadata
resolution. Duplicate Nikon storage-mode tags reject even when both values
agree; existing curve-tag priority for compressed NEF is unchanged. Both
maker-note byte orders are covered. The 27 Nikon tests and all 13 real corpus
cases pass after stricter admission (`/tmp/rrrah-nrw-mode-ambiguity.log`,
`target/qualification/nrw/strict-mode-corpus-report.json`). The subsequent
two-camera GPU qualification is recorded in IMAGE_FORMATS_100.md.

The NRW and RWL real-corpus GPU regressions additionally exercise persistent
disk storage keyed by the production source fingerprint and mosaic recipe.
For P7700, P7800 and Leica Typ 109, budgeted disk restore preserves all metadata
and sensor samples and produces the same Metal RGBA output as the independent
reference. Both external-corpus test targets pass explicitly in
`/tmp/rrrah-new-raw-disk-metal.log`. Recreated LibRaw dumps under
`/tmp/rrrah-view-corpus` match the already pinned manifest hashes; no reference
expectations were regenerated. These are offscreen persistent-cache contracts,
not physical navigation or crash durability tests.

## Phase One processing-stage oracles

The independent `raw-fixture-oracle.cpp` retains its default unpack behavior.
Optional `--phaseone-black` and `--phaseone-corrected` call LibRaw's actual
pre-demosaic Phase One stages after `raw2image_start`, preserving a full-sensor
dump before active-image assembly. They are qualification-only; LibRaw is not a
production fallback. JSON records the selected `processing_stage`.

Pinned CC0 P20+ IIQ object 4366 demonstrates why unpack metadata alone is
insufficient: native format-3 entropy matches every raw sample, but the file's
1024 black correction is applied at a later stage, and full calibration changes
16,748,675 samples relative to black-only output. Native black subtraction and
three integer flat-field grids match independent full-sensor stage oracles.
Flat-field isolation uses a documented source mutation setting only the defect
packet length to zero. It does not qualify original-source defect repair.
Original calibration includes 1862 defective pixels and three columns. Container
and viewer integration remain pending. Details and hashes are in
`tests/fixtures/iiq-investigation.json`.

Native P20+ individual-defect repair now also matches the independent full sensor
with only the three column entries disabled (types 131 changed to 0 in a documented
source variant). All 1862 pixel records retain their original order before the
three flat-field tables. The exact full-sensor channel indices are `[3,2,0,1]`;
G1 and G2 must remain distinct during this repair even though display CFA labels
both are green. The operation uses eight stack-held neighbour offsets, rounds only
valid neighbours and rejects malformed/out-of-bounds records. 1859 samples differ
from the flat-only oracle; 12347 differences before column repair belong to columns.

Native isolated-column correction now matches all 17,065,152 samples of the
original source after LibRaw black and full calibration stages. Columns 746,
2855 and 3180 are repaired after the gain grids using directional gradients and
same-color horizontal bounds. Synthetic coverage checks constant-sensor recovery,
truncation, cancellation and rejection before mutation of adjacent column tables.
Adjacent columns (distance at most four) are explicitly unsupported pending their
separate interpolation qualification. Backend/container/color/cache/GPU integration
remains open; IIQ retains Pending catalog status.

The native IIQ private-directory reader now borrows payloads without copying and
checks directory counts, offset arithmetic, lengths, requested duplicate tags and
inline scalar representation. Calibration offsets are scoped to the calibration
block itself. Both synthetic malformed-directory coverage and pinned real-source
sensor/row-offset/defect/gain-grid extraction pass. This is container access
evidence, not yet a registered production decoder or viewer qualification.

The assembled native P20+ pipeline now parses the original private tables, applies
black/pixel/gain-grid/column stages in order, and matches the full original
corrected sensor oracle. Native three-channel WB and the exact Phase One/P20+
profile match independent WB and camera-to-linear-sRGB values (matrix tolerance
1e-5). The stored top margin 21 is advanced to Bayer row 22, preserving the bottom
for crop `[26,22,4096,4093]`, matching the stage oracle. Zero/NaN WB, unknown model,
unknown calibration operations and truncated container are errors. Production
registration and managed-memory/cache/swap/GPU qualification remain pending.

IIQ now has public native backend ID 20/revision 1 and shared RAW/gallery
routing. The public decoder preserves every original corrected oracle sample
under a 64 MiB managed budget. Its retained output is 34,130,304 bytes, remains
charged through a cloned consumer owner and returns the budget to zero after
the last owner drops. A 32 MiB budget produces a typed memory refusal and also
returns to zero. Cache/swap and actual Metal readback remain unqualified; the
format catalog stays Pending until that integration evidence is gathered.

IIQ integration qualification completed for the pinned P20+-H format-3 subset:
RAM shared-owner leases, disk and bounded swap preserve full pixels and metadata;
whole 128x96 Metal Apple M4 Max readback equals the independent corrected-sensor
reference across all four routes. Pressure refusal preserves the swap entry and
retry succeeds after pressure release. Seven malformed mixed-router cases
(including uppercase `.IIQ`) and an 8 MiB refusal leave managed usage at zero.
The required RAW manifest now includes object 4366 with the original corrected
LibRaw-stage sensor hash; all 27 required cases passed under 256 MiB. This updates
the prior Pending integration notes for this subset only. Other IIQ cameras,
adjacent-column interpolation and physical photographic/display qualification
remain open. Evidence: `/tmp/rrrah-iiq-metal.log`, `/tmp/rrrah-iiq-rejection.log`,
`/tmp/rrrah-iiq-required-report.json`.

Clustered-column IIQ correction is now also qualified by controlled source
variants, replacing only the three column coordinates with `[746,748,2855]`,
`[746,747,2855]` and `[0,1,2855]`. Each matches all 17,065,152 independently
corrected LibRaw samples. The algorithm selects rounded same-color diagonal
averages for columns within four pixels of another defect, preserving sorted
sequential repair order. It skips out-of-bounds neighbours and leaves a pixel
unchanged when no neighbourhood exists. Duplicate columns remain typed errors.
This supersedes the earlier adjacent-column rejection limitation for the P20+
subset. Source/oracle hashes are recorded in the investigation fixture.

Pinned CC0 Sony DSC-F828 SRF object 1351 now has standalone bounded native
sensor extraction: every 8,265,600 sample matches LibRaw unpack output. The
rolling decryption state is retained across rows without a second sensor copy;
source extents, sample bit range, cancellation and allocation are checked.
This file uses RGBE CFA indices `[3,0,2,1]`; the E channel is not an ordinary
second green. LibRaw unpack WB is `[0,1,0,0]`, so it is insufficient as an as-shot
color oracle. Native WB, explicit four-channel color/rendering, backend/cache/GPU
integration remain required. SRF remains Pending in the 100-format catalog.

Native DSC-F828 SRF as-shot RGB WB now reads `[397,256,728]` from encrypted
SRF2 inline SHORT tags 0xd0–0xd2. It matches independent ExifTool 13.59 output
(commit recorded in the investigation fixture), also repeated in SRF3–SRF5.
The reader decrypts only a fixed bounded directory window, validates both key
directory and WB types/counts/duplicates/positive values, and requires all three
coefficients. This supersedes the missing three-channel WB investigation step.
The fourth RGBE coefficient and four-channel color/rendering contract remain
unqualified; green is not silently duplicated for E. SRF stays Pending.

Core now exposes an explicit four-distinct-plane color transform API,
`camera4_to_linear_srgb_precise`, using all four profile rows and a normalized
least-squares left inverse. The DSC-F828 3x4 output matches LibRaw full rgb_cam
within 1e-6 and NumPy SVD pseudoinverse independently agrees with LibRaw within
2.48e-8. Unit coverage proves neutral preservation, reconstruction of basis and
HDR vectors, nonzero fourth-plane contribution, and rejection of incomplete or
rank-deficient/nonfinite profiles. The existing Bayer API continues rejecting
divergent fourth planes. No implicit G/E merge is added. CFA/WB binding and
shader/demosaic/cache integration remain pending, so SRF still is not routed.
The qualification oracle now also records full `camera_to_rgb4` without altering
its existing three-column field, preserving prior format comparisons.

The domain CFA registry now includes explicit Emerald (wire code 7), appended
after existing serde variants to preserve previous variant indices. RGBE quad
validation requires four distinct R/G/B/E sites, returns `[3,0,2,1]` for the
pinned sensor, and remains refused by the Bayer quad API. Cache payload code 7
now round-trips the fourth plane with four WB entries and all matrix rows.
Existing wire codes remain unchanged; old readers explicitly reject new Emerald
payloads. This test uses authored four-plane coefficients and does not establish
the fourth as-shot WB value. CPU/GPU demosaic and source/render integration still
remain before routing SRF. Core and cache tests pass (185 total).

The allocation-free RGBE CPU reference `rgbe::interpolate_planes` now reconstructs
four separate two-pixel lattices with bilinear interpolation and same-plane edge
extension. Analytic tests cover constant independent planes, HDR values, odd
geometry, affine plane ramps and exact measured photosite preservation; invalid
channels/geometry and nonfinite samples are rejected. Input normalization requires
explicitly supplied black/white levels and four WB gains, so this does not fill
the missing Emerald as-shot coefficient. It supplies a mathematical reference
for subsequent GPU comparison, not a photographic rendering qualification.

The RGBE compute shader now runs on actual Metal Apple M4 Max and matches the
CPU four-lattice reference within 1e-5 for all 2,880 channel values (720 pixels)
in nine geometry/layout cases. Sizes 2x2, 3x5 and 17x13 and three CFA orders cover
small/odd edges and overdispatch; an extra output sentinel remains unchanged.
Four float planes remain separate and values above one survive. This is kernel
verification with analytic normalized input, not full-source photographic or
viewer qualification. Production host validation, memory admission, dispatch
segmentation and source WB/levels/color binding remain required. Evidence:
`/tmp/rrrah-rgbe-compute.log`.

RGBE row-dispatch admission now checks plane layout, shader indexing arithmetic,
buffer/workgroup limits and storage-offset alignment before pixel allocation.
The plan limits dispatch metadata to 4096 entries. The F828 full-sensor plan
requires 33,062,400 input bytes, 132,249,600 output bytes and 297,561,632 total
GPU buffer bytes including readback and uniforms. This is an allocation plan,
not a full-sensor GPU execution claim. Metal tests now also force 1024-byte
output bindings for a 16x13 image, yielding aligned 4/4/4/1 row dispatches.
Across twelve cases all 5,376 channel values match the CPU reference within
1e-5, with no output-tail writes. Managed reservation/completion ownership and
actual full-source execution remain required. Logs: `/tmp/rrrah-rgbe-plan.log`,
`/tmp/rrrah-rgbe-segmented-metal.log`.

Managed RGBE execution now reserves planned GPU buffer bytes and CPU output
before GPU allocations. It validates sample count/finite values and retains GPU
accounting with a queue-completion owner, including the error return path after
submission. Tracked CPU output persists through cloned owners. Zero CPU/GPU
limits and invalid inputs are tested without retained reservations. Actual Metal
Apple M4 Max full-F828 execution matches all 33,062,400 four-plane channel values
to the CPU lattice reference within 1e-5. CPU input/output and GPU buffer peaks
are 165,312,000 and 297,561,632 bytes; both return to zero. This uses explicitly
authored four-channel gains, not inferred camera Emerald WB, and is not a
photographic color qualification. Source WB/color/viewer binding remains open.
Evidence: `/tmp/rrrah-rgbe-full-managed.log`.

The DSC-F828 SRF four-plane WB interpretation now explicitly follows the
independent LibRaw 0.22.0 `parseSonySRF` source: SRF2 green tag 0xd1 also assigns
the fourth camera multiplier. Thus native coefficients `[397,256,728,256]`
normalize to `[1.55078125,1,2.84375,1]`. This shares a gain and preserves separate
E samples/matrix rows; it does not invent an additional source tag or use a
missing-metadata fallback. Three file values remain independently verified with
ExifTool. Five controlled encrypted-source mutations (zero R/G/B, duplicate tag,
wrong type) all fail. This resolves the explicitly declared parser-policy step
from earlier notes, but not photographic color certification or viewer binding.
Source: https://github.com/LibRaw/LibRaw/blob/0.22.0/src/metadata/sony.cpp#L2403-L2409
Evidence: `/tmp/rrrah-srf-rgbe-wb-policy-final.log`.

### DSC-F828 source calibration binding

The private SRF metadata adapter validates the LE TIFF root, exact camera model,
3360x2460 geometry, sensor byte count, orientation and complete file extent before
binding the four-plane profile. Sensor-phase black levels are [492,558,491,621]
for E/R/B/G; white is 16368. WB uses the explicit SRF policy described above.
The RGBE normalization API validates source pixel length before allocation,
preserves above-white values, supports cancellation, and retains a managed
reservation until the last output owner drops. Validated calibration fields are
immutable through its public API.

`/tmp/rrrah-srf-metadata.log` confirms the pinned real source metadata binding,
wrong-model/truncated-container rejection and every decrypted sensor sample
against the independent unpack oracle. `/tmp/rrrah-rgbe-calibration-verified.log`
records core normalization/admission/lifetime regressions. These are foundations;
SRF remains pending public routing, full color/frame readback and cache integration.

### SRF public routing milestone

The native/shared image router now accepts case-insensitive SRF and selects
the DSC-F828 full-sensor backend (ID 21, revision 1). Other models/storage
layouts fail explicitly. The color resolver validates a four-plane matrix
without using the Bayer-only profile path. `/tmp/rrrah-srf-public-route.log`
qualifies all sensor samples through `decode_image`, managed last-owner
release, five malformed source cases and input-plus-output admission refusal
at 32 MiB. Normal decoder regression: 584 passed, 25 external tests ignored.

The RAW viewport now supports RGBE directly on its u16 texture atlas.
`/tmp/rrrah-rgbe-view-metal.log` confirms Metal Apple M4 Max completed frame
readback on three CFA layouts with differing Emerald values and HDR, at native
and reduced sizes; `/tmp/rrrah-rgbe-view-regression.log` records 64 existing
GPU checks. The synthetic frame qualification does not yet establish real SRF
whole-frame color, odd-size/cropped/tiled RGBE coverage or CPU thumbnails.
Real cache/swap integration and photographic color qualification remain open;
the catalog row stays pending until the integration gate is satisfied.

### RGBE CPU thumbnail milestone

CPU thumbnails now select the validated RGBE calibration before the Bayer/X-Trans
paths. They reconstruct four independent lattices at native size and integrate
complete CFA-cell areas during reduction, with same-phase extension at odd sensor
edges. Only the bounded thumbnail output is allocated; no full normalized sensor
or RGBA intermediate is created. Color conversion precedes the existing ACES/sRGB
display mapping. Wrong source pixel counts and invalid calibration produce no
thumbnail.

`/tmp/rrrah-rgbe-thumbnail-metal.log` validates synthetic CPU thumbnails and
completed Metal Apple M4 Max frames against the numerical color oracle for three
CFA layouts, changing E/HDR samples, 32x24/native and 8x6/reduced views, plus odd
17x13 native edges. `/tmp/rrrah-srf-thumbnail-real.log` verifies that the pinned
real SRF public sensor route produces an opaque non-achromatic 128x96 thumbnail
and retains the existing source rejection/admission/lifetime checks. Real-image
thumbnail color and the complete cache/swap/frame path still need qualification.

### Real SRF storage and completed-frame qualification

`/tmp/rrrah-srf-cache-metal.log` qualifies the pinned DSC-F828 through the public
native decoder, all 8,265,600 independent LibRaw sensor samples, RAM shared lease
identity/eviction protection, disk metadata/sample restoration, asynchronous swap
write, RAM-pressure refusal followed by successful retry, and managed zero usage
after last-owner teardown. RAM, disk and swap produce identical completed 128x96
Metal Apple M4 Max frames. The complete native frame also matches an independently
written CPU viewport reference using oracle sensor samples and LibRaw's 3x4 matrix
with maximum byte difference 1. CPU thumbnails match those from the oracle sensor
and explicit source metadata.

Viewport cover framing and thumbnail sizing differ; their raw pixel arrays are
not an interchangeable numerical reference. The CPU frame oracle uses the actual
viewport geometry explicitly. The SRF catalog now records a DSC-F828 native subset
(79 implemented subsets, 21 pending), not universal SRF support or photographic
color accuracy. Tiled RGBE, arbitrary odd crops, rotated RGBE and physical HDR
remain outside this milestone.

### RGBE cropped/oriented atlas qualification

`/tmp/rrrah-rgbe-tiling-metal.log` confirms completed Metal Apple M4 Max
frame equality between normal atlas selection and forced 32/33-sample tiles
with one-sample halos. The authored nonconstant four-plane sensor is 85x77,
with an odd-origin/odd-size crop [3,1,75,69]. All eight EXIF orientations run
at native size, 3x magnification and approximately 3x reduction: 48 forced
atlas frames match their corresponding default frames byte-for-byte. Odd tile
size 33 also crosses sensor CFA phase. CPU thumbnails preserve dimensions and
opacity in each orientation. This establishes atlas equivalence for these
cases; it does not independently qualify arbitrary crop/orientation color
accuracy or physical display presentation.

### Required RGBE corpus gate

The required corpus now contains 28 cases including both local CR3 fixtures and
the CC0 DSC-F828 SRF. The native dumper emits E CFA cells and a full 3x4 transform
for RGBE, while preserving legacy 3x3 output for Bayer cases. The verifier requires
an explicit four-plane declaration and independently sourced four-gain WB policy;
the LibRaw unpack WB [0,1,0,0] remains recorded in the oracle rather than overwritten
with inferred coefficients. SRF source WB [397/256,1,728/256,1] is a separate
ExifTool/native-file and documented LibRaw SRF-policy contract.

`/tmp/rrrah-srf-required-report.json` and `/tmp/rrrah-srf-required-corpus.log`
record all 28 passing with managed 256 MiB admission. Full sensor hashes, CFA,
all four calibration rows, all four RGB transform columns, levels, crop and four
WB gains are mandatory for SRF. `/tmp/rrrah-srf-validator-negatives.log` records
a passing original and rejection after changing Emerald WB, zeroing its matrix
contribution, deleting the fourth matrix column, or replacing E CFA with G.
These negative checks alter native output only; independent oracle data is kept
unchanged.

### MEF Mamiya ZD managed sensor foundation

The existing non-CC0 external object 562 has one 4016x5344 packed 12-bit
sensor strip at 4,383,052 with 32,192,256 source bytes. The private native
sensor adapter validates the exact camera ASCII fields (including their
multiple trailing NUL padding), big-endian TIFF declaration, geometry,
compression, bit-depth array and strip layout. It reserves output before
allocation, checks cancellation per row and preserves last-owner accounting.
Every one of 21,461,504 samples matches the independent LibRaw oracle.
`/tmp/rrrah-mef-sensor-native.log` records real-source/truncation and synthetic
boundary/admission/cancellation/ownership checks. Source bytes are borrowed;
caller-owned input accounting is explicitly outside this private adapter.

ExifTool 13.59 revalidation reports only automatic WB mode, with no gain
coefficients; LibRaw unpack WB remains [0,1,0,0]. MEF is still Pending for
public color viewing; no unity WB is substituted. The external licensed
image is not copied into Git or the CC0 required corpus.

### Kodak DCS520C original TIFF / memory / cache / Metal

CC0 raw.pixls.us object 2573 is pinned by SHA-256 in
`tests/fixtures/dcs-investigation.json`; source and independent LibRaw 0.22.2
unpack oracle stay external. The bounded classic TIFF identity probe reads at
most 128 root entries plus two 64-byte strings and excludes DNGVersion.
Original `.TIF` selects backend 22 revision 1. Native camera/storage/CFA,
orientation 9 interpretation, WB, exact-model black 178/white 4095 and matrix
are independently checked. Kodak tag 2317 supplies the 4096-entry response
curve. All 2,013,760 linear sensor values match the independent oracle.

The pinned stream has a redundant trailing category-6 difference +49 and
three one-fill bits. Only exact 11-bit tail `0x38f`, fixed JPEG geometry
868x1160/12-bit/two components, predictor 1, no point transform/restart and
immediate EOI are admitted in the DCS-specific path. Generic JPEG still
rejects this extra entropy. Changed tail bits, truncation, geometry mismatch,
output admission denial and pre-cancellation are tested; no general Kodak
encoder policy is inferred from one source.

`/tmp/rrrah-dcs-cache-metal.log` records original-source decode and independent
sensor/metadata Metal readback on Apple M4 Max, exact RAM lease identity,
lease-protected limit reduction refusal, persistent disk and swap restoration,
restoration refusal under full memory pressure followed by successful retry,
and last-owner budget release. Frames compare byte-exact at 128x96 before and
after restoration. This is offscreen SDR proof, not physical HDR or navigation
latency qualification. Broader camera/tail variants remain open.

Required corpus after DCS registration: 27 public CC0 cases plus two local
EOS R8 sources, all 29 mandatory cases pass with managed 256 MiB admission.
`/tmp/rrrah-dcs-required-report.json` and `/tmp/rrrah-dcs-required-corpus.log`
record this run with native binary SHA-256. The dumper was built with
`cargo build -p rrrah-decode --example raw_fixture_dump --locked --target-dir /tmp/rrrah-required-corpus-target`;
the qualifier used that directory's debug example via `--native`, together
with `--include-local-cr3 --memory-mb 256`. DCS keeps `.tif` to exercise
content identity routing. The qualifier refuses a missing/non-executable
native binary before assigning results to individual cases.

### Shared in-memory camera pipeline regression

After extracting the camera pipeline into source-owner-based decoding for explicit
EIP sensor import, `/tmp/rrrah-eip-shared-required-report.json` and
`/tmp/rrrah-eip-shared-required-corpus.log` record all 29 mandatory RAW cases
passing again under managed 256 MiB admission. The native dumper was rebuilt
from current sources in `/tmp/rrrah-required-corpus-target`. Source owners are
released before adaptation for both normal file decode and EIP-managed inflation.
The EIP authored ZIP checks independently verify full corrected IIQ sensor values
for both stored and DEFLATE packaging, with no filesystem source-path read.
This does not qualify Capture One-authored adjustment appearance or normal EIP
viewer opening.

After adding shared DNG in-memory source decoding for explicit EIP sensor import,
all 29 required RAW cases pass again under managed 256 MiB admission.
`/tmp/rrrah-eip-dng-required-report.json` pins native binary SHA-256 and per-case
sensor/color/geometry/budget results; `/tmp/rrrah-eip-dng-required-corpus.log`
records the run. Ordinary decoder regression passes 596 tests with 32 ignored
external/hardware cases in `/tmp/rrrah-eip-dng-regression.log`. The separate EIP
stored/DEFLATE checks include both IIQ and DNG independent full-sensor oracles.

### CR3 source-owner / authored EIP import regression

CR3 now has one source-owner decoding pipeline shared by native file decode and
explicit EIP sensor import. After the change, all 29 mandatory RAW cases pass
under managed 256 MiB admission in `/tmp/rrrah-eip-cr3-required-report.json` and
`/tmp/rrrah-eip-cr3-required-corpus.log`. The report pins the current native binary.
Full decoder regression passes 596 tests (33 external/hardware checks ignored)
in `/tmp/rrrah-eip-cr3-regression.log`. Eleven explicit EIP checks pass in
`/tmp/rrrah-eip-cr3-all.log`, including authored IIQ/DNG packages and four local
EOS R8 Stored/DEFLATE imports. Every CR3 sensor sample, per-image WB, CTMD levels
and IAD1 crop match their independent/source contracts; input release, output
lease retention and insufficient combined capacity are checked. These packages
are local ZIP fixtures and do not certify Capture One adjustment appearance.
