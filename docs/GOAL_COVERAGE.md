# Current coverage of the requested viewer scope

Current demosaicing & GPU compute checkpoint (2026-10-14): Exhaustive multi-algorithm CPU/GPU demosaicing library implemented and verified (`rrrah_core::{DemosaicAlgorithm, DemosaicConfig, demosaic_f32, demosaic_u16, demosaic_bayer_superpixel}`, `rrrah_gpu::{BayerDemosaicCompute, GpuDemosaicAlgorithm}`). CPU `develop_raw` and standalone `demosaic_f32`/`demosaic_u16` support 28 variants/modes (`Auto`, `Nearest`, `Bilinear`, `SmoothHue`, `LarochePrescott`, `HamiltonAdams`, `MalvarHeCutler` 5×5, `Ppg`, `Vng` 5×5, `Rcd`, `Ahd`, `Lmmse`, `Dcb`, `Igv`, `Amaze`, contrast-guided dual modes `AmazeVng`, `AmazeLmmse`, `RcdVng`, `RcdLmmse`, `DcbVng`, 6×6 X-Trans `XTransDirectional`, `XTransMarkesteijn`, `XTransMarkesteijn3Pass`, `XTransDual`, clustered sub-cell `QuadBayer` 4×4, `NonaBayer` 6×6, `HexadecaBayer` 8×8, and universal `GenericCfa` covering `RGB`/`RGBW`/`CYGM`) plus pre-demosaic G1/G2 green equilibration (`green_equilibration`), CFA hot-pixel impulse denoising (`impulse_threshold`), post-demosaic median colour-difference false-colour suppression (`false_color_passes: 0..=4`), and high-contrast chromatic fringe suppression (`suppress_fringe`), plus half-resolution `demosaic_bayer_superpixel`, multi-threaded via `std::thread::scope` with reusable per-worker scratch buffers, cooperative cancellation, and `MemoryBudget` pre-allocation admission (`cargo test -p rrrah-core --locked`: 130 passed). GPU fragment shader (`raw_view.wgsl`) and compute shader (`bayer_demosaic.wgsl`) support 6 algorithms (`Bilinear`, `SmoothHue`, `MalvarHeCutler` 5×5, `Directional` 5×5, `Vng` 5×5, and `Rcd` 7×7) with automatic `tile_halo` elevation (`2` for 5×5, `3` for 7×7) across tile boundaries, verified on Apple M4 Max Metal (`RRRAH_GPU_BACKEND=metal cargo test -p rrrah-gpu --lib --test raw_linear_readback --locked`: 63 passed). CLI supports `--demosaic` (all 28 choices), `--false-color-passes`, and `--gpu-demosaic` (`cargo test -p rrrah --test raw_quality --locked`: 3 passed).

Current Metal checkpoint (2026-10-11): 142 selected tests across 14 suites passed; source/log hashes and workload validation in `research/combined-metal-current-2026-10-11.json`. Includes full-sensor compute, 12 MP HDR, compute/cache/swap, TTL/preload, linear imports, model worker and model swap. Ordinary application regression separately passes 208 tests (54 ignored). Live window/resize recovery, physical HDR, real NVIDIA and full 100-format qualification remain open. Entries below retain historical dates and counts.

Audit date: 2026-10-06. This is a scope ledger, not completion certification.

Current-state Metal storage regression passes all five tests in
`hdr_raster_swap` and `ttl_swap_metal`, with no ignored tests or optional-device
skip, on Apple M4 Max. This includes the new profiled/oriented TIFF fixtures,
positive TTL for real CR3 and HDR, active-owner retention, corruption refusal and
memory-pressure retry. Exact command, logs and source hashes are recorded in
`research/current-metal-storage-2026-10-06.json`. Evidence remains SDR offscreen
readback; physical display, CUDA/NVIDIA and full format qualification are open.

Foreground idle TTL integration now services RAW/raster/model RAM caches after
one second without requests; no periodic wake occurs if all RAM TTLs are disabled.
Timer-path tests cover exact native/prepared profiled raster payloads and four
model formats. The separately executed real EOS R8 CR3 test verifies exact sensor
values/metadata, managed RAM release after spill, refusal under exhausted RAM,
cancellation and subsequent restoration from the same swap key. The short-interval
timer tests do not measure physical one-second scheduling or navigation latency.
See `STORAGE_LIBRARY_CONTRACT.md`; latest viewer suite has 123 passes/12 ignored,
and the new ignored RAW case separately passes in
`/tmp/rrrah-idle-raw-pressure.log`. Earlier entries describing app idle TTL as
pending are historical and superseded by this integration evidence.

Latest decoder regression after cancellation/index admission: 691 tests pass (35 ignored).
Latest viewer regression is 129 tests passing (13 ignored).
Both are recorded in `/tmp/rrrah-model-index.log`.
Small TIFF ICC metadata is read before image's pixel-sized tag allocation limit
is applied; pixel decode limits remain enabled. A Pillow-generated ICC TIFF now
prepares successfully and its neighbour preload reuses managed float pixels
under an exhausted remaining RAM budget without assuming untagged sRGB.
Seven raster readback tests pass on Metal Apple M4 Max, recorded in
`/tmp/rrrah-post-icc-metal-raster.log`; this is not physical display qualification.
Invalid float ICC input is refused before output-memory admission, and the app
regression preserves the existing visible cache entry and its buffer identity.
These are the decoder library and viewer
binary suites; external ignored qualification and GPU integrations require their
own runs. Latest cache regression:
111 cache unit tests and five cache integration tests pass. The ignored real-AI
viewer RAM/prefetch and swap/Metal tests were separately executed successfully
against pinned sources; the combined AI qualification still fails because two
of three pages differ from Poppler. See
`research/ai-combined-qualification-2026-10-06.json` for the distinction between
transport preservation and independent decoded-pixel correctness.

XCF remains Pending for viewer support. Native parsing now includes layer masks,
channels and group paths, and pixel-level decoding reserves output and tile scratch
under the shared RAM budget. A real GIMP fixture exposed empty main levels stored
as one null tile pointer; these now produce managed zero-filled planes. Tile input
is bounded by the next tile pointer, with duplicate/decreasing offsets refused;
the final tile is still bounded by the end of the source.

`scripts/qualify-xcf-gimp.py` verifies two full mask/selection planes and six layer
metadata comparisons against the pinned GIMP test construction. The independent
Python oracle in `scripts/qualify-xcf-groups.py` verifies 22 native layer/mask planes
and 48 tree/group/attribute comparisons across three external version-13 files. A fourth
version-11 high-precision group fixture has structural/raw-byte decode evidence,
not normalized-sample qualification. Dependency/source hashes are pinned in
`research/xcf-groups-python-pixels-2026-10-06.json`.

The 29 XCF unit tests include all 65,536 half patterns: 63,488 finite patterns
match an independent Python IEEE numerical oracle bit-for-bit, and 2,048 nonfinite
patterns are refused. Sample conversion preserves finite HDR values without
gamma/ICC conversion; ambiguous high-precision pre-v12 byte order is refused.
Normal source-over, legacy RGBA expansion and shared-buffer preparation are
standalone primitives. Full group/blend/color flattening, modern indexed alpha,
display integration, swap/Metal readback and physical presentation remain open.
The catalog stays at 86 implemented subsets and 14 Pending entries; this evidence
does not qualify all XCF files or complete the 100-format objective.
The exported xcf_palette API borrows validated RGB triplets directly from the
image colormap property without allocation. Tests connect both accepted property
length encodings to managed indexed expansion, verify pointer identity/release,
and refuse duplicate palettes, every truncation and malformed following metadata.
Log: `/tmp/rrrah-xcf-palette-final.log` (28 passed). This metadata/pixel primitive
does not change XCF's Pending viewer status.
The diagnostic exporter now preserves native indexed planes and separately
exports palette-expanded encoded RGBA. Two authored v1 XCF files (indexed and
indexed-alpha) exactly match native bytes and supplied RGBA, including legacy
alpha 127/128 threshold; source, tile scratch and output credits all release.
Sources/expectations are pinned in `tests/fixtures/xcf/indexed-v1-manifest.json`,
and actual exporter output/binary hash in
`research/xcf-indexed-v1-native-2026-10-06.json`. Modern indexed alpha is deliberately
not exported via the legacy-alpha path. No flattening, color conversion or viewer
qualification is claimed by these layer diagnostics.

| Requirement | Current evidence | Remaining work |
| --- | --- | --- |
| 100 image formats | IMAGE_FORMATS_100.md has 100 entries, 94 implemented subsets and 6 pending; pending AI has PDF-compatible routing and real RAM/swap/Metal transport evidence | Implement missing families and qualify variants, color and display; AI color comparison still fails, and the 94 implemented subsets are not blanket correctness proof |
| 3D | Native STL/OBJ/PLY/OFF decoding, streamed model swap codecs, viewer spill/restore and real swap Metal readback | Additional formats, materials, thumbnails, persistent model disk cache and physical navigation qualification |
| RAM size/count/TTL | RAW/prepared-raster LRU adapters and model viewer RAM cache; independent RAW/raster CLI limits and pin/transition regressions | Broader admission coverage and physical RSS qualification; managed budgets account for tracked allocations and external buffer owners, not total process RSS |
| Disk size/count/TTL | Persistent RAW policy and real CLI regression with two CR3s | Raster/model persistence and broader concurrent policy qualification |
| Buffers | RAW and raster PixelBuffer carry lifetime reservations; generic raster RGBA output and sRGB/ICC display output reserve before allocation; ICC row buffers are budgeted; disk/swap restore reserves before pixel allocation; MemoryBudget::child supports local caps under shared ancestor budgets | Optional --managed-memory-mb shares allocation admission across managed buffers and RAW restore; spill/write queue occupancy is independent to avoid charging shared pixels twice; native CR3/DNG/Camera TIFF source/output reserve before read/assembly (EOS R8, synthetic DNG and seven real camera fixture exact-pixel/peak-budget tests); extend to remaining viewer pools and decoder scratch/output allocation admission; specialized raster and model pre-allocation admission; ModelBuffer now retains geometry reservations; GPU staging accounting |
| Swap | Shared ImageSwapCache engine for RAW/raster streaming codecs; independent optional RAW/raster viewer spill/restore, independent queue occupancy, disk bytes/count/TTL, checksum/cancellation and managed restore; raster HDR/metadata/pressure regressions | STL/OBJ/PLY/OFF model swap is also integrated with independent disk bytes/count/TTL, queue and restore budgets; end-to-end physical navigation, cold-storage and sustained pressure measurements remain |
| Previous/next preloading | Configurable navigation-relative RAW RAM/disk warming with shared managed-budget admission and foreground priority; prepared raster/model neighbour RAM warming and bounded generation work | Broader raster subimage preload; physical navigation qualification |
| GPU/shaders/Metal | RAW/raster/model render paths, explicit auto/Metal/Vulkan/DX12/GL API selection and shared test mask; 131 ordinary GPU tests passed with forced actual Metal (13 explicitly ignored; separate manual suites have their own evidence); explicit NVIDIA numeric-vendor/surface filter and no-fallback absence regression | Selected live main-viewer SDR/HDR presentation, page navigation, resize and managed model-budget refusal/recovery now have targeted screenshots/logs; broader devices and OS-wide pressure remain unqualified; driver/in-flight physical retention remains outside tracked caps |
| Compute/CUDA/NVIDIA | LinearExposureCompute WGSL storage-buffer pass; actual Metal Apple M4 Max readback verifies HDR RGB, unchanged alpha and dispatch tail | Viewer integration, sustained resource/latency qualification, CUDA path and NVIDIA hardware execution evidence |
| HDR | Native float samples and RGBA16F raster intermediate retain values above one; explicit scene-linear RAW mode has Rgba32Float Metal readback; RRRAH_HDR_SURFACE=1 selects advertised Rgba16Float/ExtendedSrgbLinear or explicitly refuses; live Metal configuration validated | Selected live native RAW probe and main-viewer float TIFF page/resize paths presented RGBA16Float / ExtendedSrgbLinear. Probe reported EDR headroom 11.983057, potential 16; this is OS reporting, not calibrated luminance. Independent physical color/luminance qualification remains |

Current preserved evidence is recorded under `research/ico-cur-viewer-selected-cache-2026-10-12.json`, `research/metal-live-hdr-viewer-resize-2026-10-12.json` and `research/metal-live-model-budget-recovery-2026-10-12.json`. Passing readbacks and selected live windows validate those paths, not all formats/devices or physical HDR color/luminance. CUDA/NVIDIA hardware execution is deferred at the user's request.

Compute qualification: `cargo test -p rrrah-gpu --test compute_readback -- --nocapture`
passed on Metal Apple M4 Max on the audit date. The pass multiplies linear RGB by
2^stops without tone mapping; it is a standalone API and is not yet used by the
viewer. Caller-owned RGBA32F input/output buffers are processed through aligned
subrange bindings within storage and dispatch limits. A constrained 1024-byte
binding test verifies three dispatches and exact per-pixel offsets. A separate optional 6188x4120 test runs the full dispatch range on Metal M4 Max,
with exact readback at three positions; see GPU_COMPUTE.md for timings and scope.
Buffer
allocation must still fit the device max-buffer-size and available GPU memory. This test provides no CUDA, NVIDIA or physical HDR display evidence.

Next integration priorities: managed
allocation admission before decoding, GPU memory ownership, and a qualified HDR
output path. Format work must preserve the full 100-entry objective rather than
counting partial readers or 3D subsets as completed image-format qualification.

## Decoder and cache regression after model block readers

Current locked test run across rrrah-decode, rrrah-cache, rrrah-memory and
rrrah-swap passes 689 tests: decoder 559 (four ignored), cache 95 unit plus
five integration, memory 23 and swap seven. Log:
`/tmp/rrrah-cache-codec-regression.log`. The run covers the available test
contracts; it does not qualify all 100 formats or physical GPU presentation.
Model restore performance controls and real Metal swap readback are recorded
in MODEL_FORMATS.md. CUDA, NVIDIA execution and HDR window output remain open.

Gallery window regression: thumbnail job enumeration now saturates the radius
addition and returns no jobs for an invalid center, including an empty gallery.
Tests include `usize::MAX` radius/center alongside existing directional windows,
generation replacement and shared-budget recovery. The targeted gallery run
passes 19 tests with one ignored (`/tmp/rrrah-gallery-window.log`). This validates
the tested scheduling contracts, not physical navigation or all decoder pools.

The thumbnail prefetch convenience path also now rejects an out-of-range
selection before indexing previous neighbours. Tests cover the first invalid
index, `usize::MAX` and an empty gallery. This aligns it with the guarded
generation plan and avoids panic after a gallery/selection mismatch. The full
application binary suite passes 104 tests with three ignored in
`/tmp/rrrah-app-current.log`; this includes loader, cache and CLI regressions.

Limit edge qualification: `rrrah-memory` passes 25 tests in
`/tmp/rrrah-cache-limit-edges.log`. Added policy regressions prove an
unrepresentable `Duration::MAX` deadline rejects insertion/replacement without
evicting existing values, and logical `u64::MAX` byte capacity preserves exact
accounting and atomic pin-aware rejection. The latter uses weighted scalar
entries, not actual enormous allocations; it qualifies arithmetic only.

Current application/cache regression: the full locked application, cache,
memory and swap test run passes after separating RAW-quality error assertions.
The invalid curve and unknown required opcode cases now each require their
own error category; either failure can no longer mask the other. NRW recipe
tests also include its backend ID/revision and distinct cache identity.
Evidence: `/tmp/rrrah-viewer-cache-current.log`,
`/tmp/rrrah-raw-quality-current.log`, `/tmp/rrrah-camera-recipe-current.log`.
# Directional prefetch queue verification

PTX extension compatibility groundwork: case-insensitive `.ptx` requests use
the existing native PEF backend and are admitted to gallery candidate paths.
TIFF camera classification avoids full-source admission and never falls back
to a preview after a sensor error. Explicit local qualification copies pinned
CC0 PEF object 831 to lower/upper PTX suffixes and compares all managed sensor
samples, metadata and recipe identity; memory returns to zero afterwards.
This is a renamed-PEF routing test, not an independent camera-produced PTX
oracle. The catalog PTX row remains Pending and the subset count remains 68.
No PTX source was found in the locally cached raw.pixls.us catalog; primary
source inspection did not establish a distinct PTX storage specification.
Evidence: `/tmp/rrrah-ptx-router.log`, `/tmp/rrrah-ptx-image-router.log`,
`/tmp/rrrah-ptx-alias-real.log`.

ICC-aware RAM limits now have explicit tests with independent byte and count
constraints: background admission and oversized visible transitions preserve
the old pin; successful visible transitions retain full old pixel+ICC memory
only while an external owner exists. Cache resident weight stays 19 bytes,
while shared managed usage correctly reaches 38 for two externally live
allocations. TTL expiration drops cache ownership without prematurely releasing
an externally held profile or pixels. No sleeps are needed for the TTL check.
Evidence: `/tmp/rrrah-icc-ram-limits.log`.

Real ICC fixture integration: profiled PNG and the corresponding ORA/KRA
containers now pass decode -> managed ICC preparation -> asynchronous raster
swap -> pressure/retry restore -> ICC preparation -> actual Metal readback.
Restored profile bytes remain identical. Original/restored 64x64 frames match
exactly; independent sRGB fixture images differ by at most one channel unit.
The root holds pixel+ICC capacity while decoded owners exist and only the
float output after decoded owners release; final managed usage is zero.
Qualification uses actual forced Metal M4 Max, optional skipping disabled.
Evidence: `/tmp/rrrah-icc-swap-metal.log` (three tests, including HDR and six
corrupt-raster cases; real ICC loop covers PNG, ORA and KRA).

ICC raster ownership/accounting correction: color metadata now uses immutable
shared ownership instead of copying its profile Vec on every raster clone.
Managed raster adoption admits profile capacity; swap restoration reserves
profile memory before allocation/read and transfers that reservation to the
shared metadata owner. `DecodedRaster::capacity_bytes` includes pixels and ICC
capacity and is used by RAM admission and asynchronous swap queue accounting.
Explicit tests reject profile-only and combined one-byte-short budgets, verify
profile pointer sharing through clones, and release credit at the last owner.
RGBA32F plus a three-byte ICC profile weighs 19 bytes, not the old 16-byte
pixel-only weight. PNG/ORA corpus tests distinguish complete-raster ownership
from a retained pixel-only owner, which correctly releases profile memory.
This fixes managed adoption/restoration and cache weighting; allocation peaks
inside every external image decoder are not globally certified by this change.
Validation log: `/tmp/rrrah-icc-all-tests.log`.

Raster swap corruption/recovery is now covered through the real asynchronous
store for RGBA8, RGBA16 and RGBA32F, with both byte mutation and truncation.
Corrupt reads remove the entry, release managed credit, and increment errors
only once; a subsequent lookup is a clean miss. Re-enqueueing the same key
restores the exact canonical payload including frame selection. Float payloads
preserve HDR magnitude, negative zero and NaN payload bits (codec fidelity,
not a claim that non-finite values are suitable for display). Final queue and
managed memory usage return to zero. The existing linear Metal HDR readback
test also passes in the same target.
Evidence: `/tmp/rrrah-hdr-raster-corruption.log` (2 tests passed; 6 corruption
cases across 3 representations).

FZ8 storage validation is shared by metadata inspection and pixel decoding.
Exact single-strip size, compression, compatible RowsPerStrip and source
bounds are validated before metadata is admitted. A cancelled pixel request
is rejected before reserving the output Vec. Synthetic tests require malformed
declared size and truncated source to fail in both paths. Real-source tests
mutate Panasonic format, RowsPerStrip and StripByteCounts independently and
verify explicit errors and zero managed usage, while both genuine Panasonic
sources still pass cache/swap/Metal qualification.
Evidence: `/tmp/rrrah-panasonic-layout-full.log` (565 passed, 4 ignored),
`/tmp/rrrah-panasonic-layout-metal.log` (3 explicitly executed tests passed).

FZ8 follow-up resolves the formerly rejected profile: LibRaw identifies
`packed_load_raw` with flags 81, not Panasonic predictor compression. Native
Rust now decodes five little-endian 12-bit sample pairs plus a separator in
each 16-byte block (10 pixels). Admission requires exact FZ8 model, mode
34316, Panasonic format 2, twelve-bit precision, width divisible by ten,
single exact-size strip and compatible RowsPerStrip. Truncation and invalid
layouts fail; cancellation is checked per row. Other legacy profiles still
fail explicitly. Black is 0 and calibrated white 3967 for this profile.
RW2 recipe revision 8 invalidates prior cache results.

All 7,258,470 FZ8 samples match the independent LibRaw dump. The required
success corpus now contains 16 cases; historical mismatch provenance moved
to `tests/fixtures/panasonic-raw-qualification.json`. Both FZ50 and FZ8 pass
persistent cache, pressure/retry swap, metadata and actual Metal pixel readback.
A mutated format-3 source verifies explicit rejection and released memory.
Evidence: `/tmp/rrrah-panasonic-fz8-tests.log` (133 passed),
`/tmp/rrrah-panasonic-fz8-corpus.log` (16 passed),
`/tmp/rrrah-panasonic-fz8-metal.log` (3 passed),
`/tmp/rrrah-panasonic-fz8-full-decode.log` (full decoder regression suite).
References: LibRaw 0.22.2 `src/decoders/generic.cpp::packed_load_raw` and
`src/metadata/tiff.cpp`, inspected locally; no LibRaw is linked into the viewer.

Second Panasonic RAW camera audit: CC0 raw.pixls.us object 2282 (DMC-FZ8)
uses mode 34316 without RawDataOffset, unlike qualified RW2/RWL packed sources.
The prior decoder silently returned sensor hash
`2c222c799fa0300ff5b94fd03123631268b6bb8ebb5dc28f705a7d4675c2c55c`, while
independent LibRaw 0.22.2 returns
`ee843204346f627ae5cca8583fb73fc795a48b1c8f075d70b732404ac967113b`.
Black/white also disagreed (native 15/4095 versus oracle 0/3967).
This unqualified storage variant now fails explicitly in metadata and pixel
paths; correct decoding remains pending. RW2 recipe revision 7 invalidates
the former cache behavior. Source/oracle provenance is pinned in
`tests/fixtures/panasonic-raw-rejected.json`, separate from the success corpus.
Real FZ8 rejection releases its managed source allocation; the same integration
target still verifies positive FZ50 cache/swap/Metal output.
Evidence: `/tmp/rrrah-panasonic-positive-negative.log` (2 passed),
`/tmp/rrrah-panasonic-legacy-guard-tests.log` (132 passed),
`/tmp/rrrah-panasonic-guard-corpus.log` (15 success cases passed).

Panasonic FZ50 native RAW now has explicitly executed persistent-cache,
asynchronous-swap and Metal readback qualification. All sensor samples match
the pinned LibRaw dump; original/cache/restored frames are identical at
128 x 96. Full RAM pressure rejects restore without corrupting the swap
entry, retry succeeds, metadata survives, and managed memory is released.
Evidence: `/tmp/rrrah-panasonic-cache-swap-metal.log`.
Load/cache timing and its scope are recorded in `RAW_LOAD_PERFORMANCE.md`.

Panasonic `.RAW` progress: CC0 raw.pixls.us object 2234 (DMC-FZ50 4:3) exposed
a real sensor-domain bug in the existing IIU/RW2 path. Legacy tag 0x000B mode
34828 stores twelve sample bits left-aligned in a 16-bit word; nonzero low
nibbles are discarded, matching LibRaw 0.22.2 across all 10,151,190 samples.
This mode also uses its declared black level without the packed-mode +15.
RW2 recipe revision is now 6, invalidating previously decoded cache entries.
The RAW suffix is admitted to gallery routing, with content-based backend
selection; it is not blindly assigned to Panasonic for other camera brands.
The pinned required corpus includes source and full-sensor oracle hashes.
Crop follows Panasonic tags (16,7,3648,2736), intentionally distinct from
LibRaw's initial active crop (4,0,3671,2748). Subsequent native GPU/swap
qualification is recorded above; 68 catalog rows have subsets, 32 remain Pending.
Evidence: `/tmp/rrrah-panasonic-camtiff-test.log` (132 passed),
`/tmp/rrrah-panasonic-decode-full.log` (564 passed, 4 ignored),
`/tmp/rrrah-panasonic-corpus.log` (15 required cases passed, local CR3 included).

Protected-write TTL/cancellation regressions: an expired protected entry is
removed under the write lock and does not block a one-entry cache admission.
Cancellation at each of the first four checks preserves the live resident,
publishes no incoming entry, and leaves the write lock available. Focused
tests: `/tmp/rrrah-protected-ttl-cancel.log` (4 passed).
Combined RAM/cache/swap verification: `/tmp/rrrah-memory-swap-ttl-check.log`.

Byte-driven protection is now implemented for each RAW prefetch command:
present or successfully stored higher-priority neighbours are passed to
`DiskMosaicCache::store_with_cancel_preserving`. Under the write lock, admission
accounts for their actual stored sizes and count before evicting any live
unprotected candidates. Impossible admissions fail without live eviction.
Protection applies to this writer, not unrelated concurrent writers; TTL
expiration still applies. Ordinary foreground writes retain existing behavior.
Cache regressions cover byte/count rejection without partial eviction,
duplicate/missing protected keys, unprotected eviction and protected replacement.
Real EOS R8 tests cover an already-present nearest RAW retained while the next
RAW exceeds the shared byte budget. Test waits require both jobs and released
memory, rather than mistaking one completed neighbour for a finished command.
Evidence: `/tmp/rrrah-protected-cache.log` (102 passed),
`/tmp/rrrah-protected-prefetch.log` (7 passed, including local fixtures).

RAW prefetch also caps the processed plan to the disk cache's configured
maximum entry count. This prevents the lower-priority tail of a single plan
from evicting the neighbours just warmed when the cache is count-limited.
The subsequent byte-protection implementation above extends this count cap;
protection against unrelated concurrent writers remains unimplemented.
An explicitly executed EOS R8 integration test verifies a one-entry cache
retains the nearest upcoming RAW, skips lower-priority paths, and releases the
shared memory budget. The real-fixture pressure/recovery test also passes.
Evidence: `/tmp/rrrah-prefetch-real-count.log` (6 passed, none skipped).

Neighbour prefetch now puts upcoming files first during forward navigation,
nearest first, followed by the configured previous files. Backward navigation
keeps previous files first and swaps the configured window extents. Initial
selection preserves its existing ordering. Queue generation cancellation and
configured counts are unchanged. Worker-command tests check the actual queued
order, including direction reversal and bounded gallery edges.

Validation: `cargo test --locked -p rrrah gallery::tests` and
`cargo test --locked -p rrrah --bin rrrah`; logs:
`/tmp/rrrah-prefetch-direction.log`, `/tmp/rrrah-direction-app.log`.
This verifies ordering and regressions, not a measured latency improvement.

## Sony SR2 DSC-R1 integration qualification

The CC0 raw.pixls.us object 3221 is pinned in `raw-color-corpus.json` and
`sr2-qualification.json`. The native SR2 route is qualified for this camera's
14-bit uncompressed mode. `sony_sr2_readback` explicitly executed on Metal Apple
M4 Max compares all 10,390,272 sensor samples, uses independent WB/black/matrix
values for its rendered reference, and checks exact whole-frame readback at
128×96 before and after persistent-cache and asynchronous-swap roundtrips.
Full RAM pressure rejects restoration without marking the disk entry corrupt;
retry succeeds and managed usage returns to zero. This is one camera/storage
profile, not universal SR2 or physical HDR display certification.

## MRW native container and sensor foundation

`MrwLayout` validates big-endian MRM block extents, unique PRD/WBG/TTW blocks,
12-bit packed mode 0x59 and RGGB CFA. It decodes to managed immutable pixels with
pre-allocation admission and periodic cancellation. The explicit real-fixture
test matches all 6,056,128 Dynax 7D object 1826 samples against the independent
LibRaw dump. Tests cover every byte truncation of a synthetic fixture,
unqualified storage, oversized block lengths, memory refusal, cancellation and
last-owner release. Native metadata adaptation, RAW routing, cache/swap/viewer
integration and additional MRW storage modes remain pending; row 60 is Pending.

MRW follow-up: `NativeMrwDecoder` now reads embedded TTW camera identity and
orientation, admits only the qualified DYNAX 7D profile, resolves its calibrated
matrix, adapts WBG channel order and outputs full-sensor CFA metadata. Backend
id 12 revision 1 separates persistent keys from TIFF/other RAW decoders.
`.mrw`/`.MRW` route through `NativeRawDecoder`; MRM magic selects sensor in the
mixed-image router. The required corpus now contains 18 cases including local
CR3, all passing with a 128 MiB managed source/output budget. Cache/swap/Metal
roundtrips and further MRW camera/storage qualification remain pending.

MRW Dynax 7D integration: `minolta_mrw_readback` explicitly runs on Metal Apple
M4 Max and compares the native whole-frame 128×96 readback with independent
sensor/WB/black/white/matrix values. Persistent cache and asynchronous swap
preserve every sensor sample and all metadata. Full RAM pressure refuses restore
without corrupting/removing the disk entry; retry succeeds and managed usage
returns to zero. Mutated real-fixture cases verify errors and zero retained
managed memory for block overflow, unsupported precision, zero WB, unknown
camera and truncated sensor payload. This does not certify other cameras,
physical HDR display or visible window presentation.

MRW DiMAGE A2 follow-up: pinned CC0 object 4419 qualifies all 8,036,032 sensor
samples, RGGB CFA, native WBG gains, calibrated camera matrix, black 0/white 3983
and full-sensor crop. The same persistent-cache/swap/Metal tests explicitly pass
for Dynax 7D and A2. Qualified identities include exact make/model and sensor
geometry; unsupported WBG scaling is rejected. Required RAW corpus now has 19
cases. Further cameras and storage modes remain unqualified.

MRW content routing: MRM is now a strong `SniffedFormat` in the common sniffer
and overrides filename extensions in `NativeRawDecoder`, including unsupported
extensions. An explicitly executed real-fixture integration test copies the same
Dynax 7D bytes under jpg/dng/bin/MRW names and verifies sensor selection,
identical backend recipe, full samples and metadata. A four-byte MRM file under
jpg remains a sensor request, fails decode and releases managed memory. Four
MRW integration tests (including both Metal camera cases) pass; the decoder
suite passes 568 tests, with five fixture-dependent tests ignored by default.
This does not add unsupported suffixes to extension-based folder discovery.

## ERF Epson R-D1 color foundation

The native `read_erf_maker_metadata` API resolves Epson MakerNote 0x0401 black
values and 0x0e80 white balance; payload reads must remain inside the declared
MakerNote. Explicit object 2680 test matches independent LibRaw WB and spatial
RGGB black [61,64,63,60], rejects zero WB and truncated metadata. The previously
verified 10-sample/16-byte sensor layout still needs production decoding/routing,
calibrated matrix adaptation and cache/swap/Metal integration. ERF remains Pending.

## Epson R-D1 ERF end-to-end subset qualification

The native ERF backend (recipe backend 13, contract revision 1) is now included
in canonical recipe revision/uniqueness tests. CC0 raw.pixls object 2680, pinned
in `tests/fixtures/erf-qualification.json`, was independently compared with
LibRaw 0.22.2: all 6,152,960 samples (3040x2024), RGGB, full sensor crop,
spatial black [61,64,63,60], white 4095, camera WB and matrix. The decoder only
admits the qualified Epson R-D1 profile and storage contract.

`epson_erf_readback` was run explicitly with Metal selected and GPU optional
skipping disabled on Apple M4 Max. Native/persistent-cache/swap frames match
independent full sensor pixels and the complete 128x96 GPU frame. Restore under
exhausted managed memory preserves the swap entry and retries correctly without
counting corruption. Metadata and samples roundtrip exactly; write/read counts
are one each and managed memory returns to zero. Zero WB, invalid black values,
unqualified model identity, truncated sensor and truncated metadata all fail the
mixed image router without preview fallback or retained managed allocations.

Current verification: decoder 568 tests passed (6 optional ignored), ERF hardware
integration 2 tests passed explicitly, and all 20 required independent RAW corpus
cases passed with a 128 MiB managed limit, including both local CR3 cases.
Evidence: `/tmp/rrrah-erf-current-corpus.json`,
`/tmp/rrrah-erf-current-decoder.log`, `/tmp/rrrah-erf-metal-qualified.log`.
The 100-format catalog now records 71 implemented subsets and 29 pending rows;
this does not qualify every variant, other ERF cameras, physical display, CUDA,
NVIDIA hardware or physical HDR output. The overall goal remains incomplete.

## Kodak P880 KDC native subset

A new native camera backend (ID 14, revision 1) routes `.kdc`/`.KDC` through
Kodak-specific TIFF directories, not the embedded JPEG. Sensor geometry comes
from private tags 0xfa13/0xfa14 at the root 0xfe00 IFD pointer; storage offset is
the checked sum of raw 0xfd04 words at offsets 16 and 48. Packed12 MSB sample
pairs end exactly at the declared JPEG preview boundary. Decode checks bounds,
cancellation and reserves sensor capacity before allocating. Strict identity
admits EASTMAN KODAK COMPANY / KODAK P880 ZOOM DIGITAL CAMERA only; unknown
profiles fail. BGGR comes from 0xfd09; calibrated black 0, white 3963 and the
exact camera matrix are independently pinned. Native WB selection 0xfa0d chooses
one of five coefficient tables, requiring nonzero u32 RGB values and normalizing
by green; unsupported selectors and malformed types fail explicitly.

CC0 raw.pixls object 2339, source SHA256
8138d068d14e9ce90c6877b175759fc92358024303b8cd5e9b9eb5ae11e8c4b1,
matched LibRaw 0.22.2 for all 8,049,120 full-sensor values (3280x2454), CFA,
crop, black/white, WB and matrices. All five WB selections (0/1/2/3/6) were also
independently checked on source mutations with exact sensor equality. The
original case is required in the RAW qualification corpus.

The actual Metal Apple M4 Max integration test compares the full 128x96 frame
against independently pinned sensor/color parameters, then verifies identical
persistent-cache and swap-restored samples, metadata and render. Exhausted
restore memory leaves the swap entry retryable; final retained memory, queued
bytes and error count are zero. Eight negative cases cover zero WB, invalid
geometry/selector/CFA/profile, invalid descriptor and WB types, and truncated
sensor storage; the mixed router never falls back to preview.

Verification: decoder 568 tests passed (6 optional ignored), KDC integration
2 passed explicitly with Metal and optional GPU skipping disabled, all 21
required RAW corpus cases passed at 128 MiB including both local CR3 cases.
Evidence: `/tmp/rrrah-kdc-tests.log`, `/tmp/rrrah-kdc-metal.log`,
`/tmp/rrrah-kdc-corpus-report.json`, `/tmp/rrrah-kdc-wb-modes.json` and
`tests/fixtures/kdc-qualification.json`. Catalog: 72 implemented subsets,
28 pending rows. Universal KDC coverage, photographic color, NVIDIA/CUDA and
physical HDR remain incomplete.

### Samsung EX1 SRW required regression coverage

CC0 object 1204 is now a mandatory entry in `tests/fixtures/raw-color-corpus.json`, with pinned source SHA-256, full sensor SHA-256 and LibRaw 0.22.2 color metadata. The current 22-case required corpus, including both local CR3 cases, passes with a 128 MiB managed budget. `samsung_srw_readback` independently verifies full pixels, persistent cache, bounded swap restoration after memory-pressure refusal, exact 128×96 Metal output and malformed-source rejection without retained managed memory. Qualification is limited to EX1 storage; other SRW camera layouts remain pending.

## Explicit EIP sensor transport / storage / Metal

EIP remains one of the 20 Pending image families for ordinary viewer and authored
Capture One appearance. The separate explicit sensor-only library API now handles
registered camera TIFF families, DNG and native CR3 from managed in-memory sources.
Authored ZIPs around qualified IIQ/DNG and two local EOS R8 RAWs verify all pixels
and source color/geometry metadata. `/tmp/rrrah-eip-cache-metal.log` records eight
Stored/DEFLATE packages passing RAM lease, persistent disk, swap pressure/retry,
last-owner release and exact 128x96 SDR Metal readback on Apple M4 Max. This proves
the tested sensor-only transport/storage path; it does not satisfy EIP settings,
ICC/LCC/mask interpretation, real Capture One fixture, physical HDR or CUDA/NVIDIA.

EIP MRW storage follow-up: the explicit sensor test now covers 12 packages
from six RAW sources, including Minolta 1826 and 4419. Actual Metal Apple M4 Max
128x96 frames, full pixels/metadata, CPU thumbnails, RAM leases, persistent
disk and swap restoration remain exact. Memory-pressure refusal preserves the
swap entry for retry and all managed output credits release after the last owner.
Evidence: `/tmp/rrrah-eip-mrw-metal.log`. This does not qualify Capture One
adjustments, ordinary EIP viewer opening, physical HDR or CUDA/NVIDIA.

After MRW source-owner extraction, all 29 mandatory RAW corpus cases pass
at a 256 MiB managed budget, including both local CR3 cases. Report:
`/tmp/rrrah-eip-mrw-required-report.json`; normal decoder tests: 596 passed.

## RAW queued upload admission

The viewer now shares its managed allocation budget with RAW halo/row packing
and aligned atlas queue occupancy. Queue credit is reserved before texture
creation and retained by a completion callback; the completion guard flushes
pending texture writes even on early return. Actual Metal M4 Max readback
passes all 10 RAW checks, including queue quota refusal preserving the loaded
image, multilayer admission, and credit release after GPU completion when the
renderer has already dropped. Log: `/tmp/rrrah-queue-readback-all.log`.
This accounts logical upload payload, not physical driver staging or all GPU
resources; raster/filmstrip queue accounting remains open. Model geometry uses
mapped-at-creation buffers rather than queued upload writes.

## Raster queued upload admission

`RasterRenderer::with_upload_queue_budget` admits RGBA32F uploads before texture
creation, using 256-byte aligned row footprints, and retains occupancy through
GPU completion with the shared RAW upload completion guard. The viewer connects
this pool to `--managed-memory-mb`, shared with RAW and other tracked allocations.
All seven raster readbacks pass on Metal Apple M4 Max, including HDR signed/bright
channels and a new queue regression: a 17-pixel row requires 512 logical bytes,
a 511-byte quota refuses replacement while preserving the loaded image, and
GPU completion releases credits after renderer destruction. Application
`cargo check -p rrrah --locked` passes. Logs:
`/tmp/rrrah-raster-queue-readback.log`, `/tmp/rrrah-raster-queue-app-check.log`.
Filmstrip uploads, uniforms, driver overhead and physical staging RSS remain
outside these queue caps. Model vertices are written directly into a mapped
vertex buffer; broader in-flight resource retention remains unqualified.

## Filmstrip thumbnail queue admission

Filmstrip RGBA8 uploads now admit aligned queue bytes before texture creation
and release their credit after submitted work completes. The viewer uses the
fallible `try_upload_tile` API with its shared managed budget. Invalid dimensions,
wrong payload length and memory refusal do not consume or replace texture slots.
Actual Metal M4 Max testing verifies overflow/zero/payload rejection, pressure
refusal, row alignment, slot reuse and completion cleanup after renderer drop.
This is GPU admission evidence, not a pixel readback or physical presentation
qualification. Logs: `/tmp/rrrah-filmstrip-queue.log`,
`/tmp/rrrah-filmstrip-app-check.log` (application check passed).
A refused thumbnail currently retains its placeholder; automatic pressure retry,
filmstrip texture residency budgets, vertex/uniform queues and driver overhead
remain open. The legacy infallible upload wrapper remains for compatibility;
budget-aware callers must use the fallible API.

## Bounded thumbnail pressure retry

The viewer now retains at most one ready thumbnail after a temporary queue
admission refusal. Existing bounded worker channels provide upstream backpressure;
its managed pixel reservation remains attached to the retained buffer. Retry is
25 ms with a fixed two-second deadline, without source re-decoding. A request
exceeding the pool's total quota is discarded immediately. Folder/window job
replacement clears the deferred item; retry revalidates folder, cover path and
source stamp before upload. Expiration releases the pixels.
The event loop polls GPU callbacks and wakes for the retry deadline without
requiring a redraw. Successful strip uploads register completion polling.
Application tests pass 112 cases (nine ignored at that checkpoint), including
managed ownership, changed-source rejection and exact deadline behavior:
`/tmp/rrrah-thumb-retry-app-all.log`. Filmstrip texture residency, broader physical
navigation and retry after pressure lasting longer than two seconds remain open.

Actual Metal completion test also passes without a surface or redraw:
`/tmp/rrrah-thumb-retry-metal.log`. It submits a budgeted thumbnail, destroys
the renderer, advances callbacks through nonblocking device polling and verifies
all queue credits released. This is callback/admission evidence, not physical
window or pixel-color qualification.

## Filmstrip texture residency

Filmstrip RGBA8 textures now use a dedicated child of the shared GPU resource
budget. Admission precedes texture creation and queue writes; removing a tile
or destroying the renderer releases its logical residency credit. Texture and
queue admission errors are distinct. The viewer evicts one old thumbnail on
texture-pressure refusal before its bounded retry, while queue pressure does
not evict the resident cache. Existing entry-count LRU remains independent.
Two actual Metal M4 Max admission tests pass: shared-parent competition with a
raster texture, local thumbnail quota, refusal preserving slots, removal, slot
reuse, duplicate removal and renderer cleanup, plus aligned queue completion.
Log: `/tmp/rrrah-filmstrip-residency-metal.log`. This is resource admission
evidence; thumbnail color readback, in-flight texture retention, vertex/uniform
accounting, physical driver RSS and broader navigation remain open.

Application residency integration passes 113 normal tests (10 ignored),
including shared GPU/local limits and recency-aware pressure eviction with
source inventory cleanup: `/tmp/rrrah-filmstrip-residency-app.log`.

## Submitted frame resource ownership

GPU resource credits now have shared lifetime ownership. `GpuResourceLease`
snapshots RAW atlas, raster texture, model vertex/depth and resident filmstrip
tile reservations. The viewer snapshots the active render path plus strip,
submits the frame, retains the snapshot through queue completion and schedules
event-loop polling. Resource replacement or renderer destruction cannot release
these credits while a submitted frame lease still owns them. Filmstrip snapshots
conservatively cover every resident tile, including undrawn ones.
An actual Metal M4 Max validation test encodes all four paths into a real target,
destroys renderers before submission, checks exact retained budget and refusal
of conflicting admission, then verifies completion releases all credits with
no GPU validation error. This is ownership/validation evidence, not a color
readback or physical window proof. Logs:
`/tmp/rrrah-gpu-resource-lease-metal.log`; broader Metal regressions pass 27 tests
in `/tmp/rrrah-gpu-resource-lease-regression.log`; application tests pass 113
(10 ignored) in `/tmp/rrrah-gpu-resource-lease-app.log`.
Uniforms, surface targets, driver overhead, pre-frame upload texture ownership
and broader queue-retention/physical memory qualification remain open.

## Destination texture ownership during upload

The shared upload completion guard now retains destination GPU texture credits
for RAW atlases, RGBA32F raster and RGBA8 strip thumbnails, even when queue-byte
admission is disabled. The renderer and guard share the same reservation; no
double charging occurs. Pending texture writes are submitted before callback
registration, and early returns drop the guard through the same completion path.
Successful RAW/raster uploads register event-loop polling independently of
optional cache leases; filmstrip already does this.
An actual Metal test removes the uploaded thumbnail and destroys its renderer
before any frame encoding, verifies credit remains unavailable for new
admission, then polls completion and checks release. Six relevant Metal
regression targets pass 28 tests in `/tmp/rrrah-upload-texture-regression.log`;
113 application tests pass (10 ignored) in `/tmp/rrrah-upload-texture-app.log`.
Existing cleanup tests now wait for completion before asserting retired
texture release. This does not certify driver overhead, uniform/vertex staging,
CUDA/NVIDIA execution or physical HDR/window presentation.

MNG progress: native full-canvas opaque VLC frames now route through the common
image/gallery path with explicit selection, 8/16-bit PNG precision, color and
managed source/output ownership. Three authored integer frame oracles and
strict malformed/pressure/routing regressions pass. FFmpeg matches every
embedded PNG sample plane independently. Decoder suite: 599 passed, 34 ignored
(`/tmp/rrrah-mng-regression.log`). This moves row 15 to an implemented subset,
not complete qualification: 81 subsets / 19 Pending, none universally qualified.
Outer MNG composition/global inheritance/transparent layers/controls/loops,
external MNG oracle, cache/GPU/live viewer proof remain open.

MNG transport/readback follow-up: all three authored frames pass native and
prepared swap transport with exact values, precision, color and image selection,
managed restore pressure refusal preserving retry, prepared RAM lease identity
and pin-aware zero-limit refusal. Actual Metal M4 Max 96x64 frames equal the
prepared references from pinned FFmpeg PNG planes, and managed credit returns
to zero. `/tmp/rrrah-mng-cache-metal.log`; fixtures and oracle generation:
`tests/fixtures/mng/png-oracle-qualification.json`,
`scripts/qualify-mng-png-fixtures.py`. This covers six representation/frame
combinations in the supported subset, not independent full MNG semantics,
physical window presentation or universal color qualification.

MNG static composition follow-up: zero-tick full-canvas opaque layers form
one output frame, represented by the last layer; index 1 is refused. New
two-layer fixture output and frame metadata match the explicit source contract.
600 decoder tests pass (34 ignored), `/tmp/rrrah-mng-still-regression.log`.
The expanded Metal M4 Max cache/swap test checks four visible frames in native
and prepared forms (eight combinations), including the final static layer.
All samples, precision, color/index/count, RAM lease/pin policy, swap pressure
retry and 96x64 Metal frames match, with final managed usage zero. Evidence:
`/tmp/rrrah-mng-still-cache-metal.log`. Five embedded PNG planes independently
match FFmpeg via the reproducible fixture oracle script. This does not qualify
transparent/partial layers, background controls, global inheritance, external
MNG composition engines or physical viewer presentation.

MNG color inheritance: bounded top-level cHRM/gAMA/iCCP/sRGB defaults now
snapshot per embedded layer, respect local color override, empty-chunk
nullification and global sRGB vs gamma/chromaticity precedence. Shared PNG
validation/preparation handles the reconstructed metadata; gamma/chromaticity
interpretation is still unqualified rather than treated as proven sRGB.
602 decoder tests pass (34 ignored), including ICC last-owner accounting and
state/override/reset/error checks. The global-sRGB fixture expands actual Metal
RAM/swap/readback proof to six visible frames / 12 native-prepared variants;
all exact, with pressure retry and zero final managed usage. Evidence:
`/tmp/rrrah-mng-global-regression.log`, `/tmp/rrrah-mng-global-cache-metal.log`.
Background, palette/sBIT/pHYs inheritance, alpha/partial composition, controls,
external MNG engine qualification and physical display remain open.

MNG partial layers: native integer composition now restores a covering base and
overwrites only each opaque layer's origin rectangle. Same qualified color
space is required, and 8/16-bit mixtures promote without quantization. Canvas
admission precedes allocation; source/decoded layer reservations are transient,
and cancellation checks bound copy work. Missing background, insufficient
canvas/source capacity and mixed/unqualified color refusal preserve cleanup.
Fixtures cover horizontal and vertical rectangles, RGB16 and static layering.
604 decoder tests pass (34 ignored); 13 presentations / 26 native-prepared
RAM/swap/Metal combinations are exact with zero final managed usage:
`/tmp/rrrah-mng-partial-regression.log`,
`/tmp/rrrah-mng-partial-cache-metal.log`. Alpha, backgrounds, controls,
qualified mixed-profile composition and physical viewer proof remain open.

MNG alpha follow-up: RGBA8/RGBA16 partial layers now compose in linear-sRGB
float storage with managed canvas admission and last-owner accounting. Three
CC0 fixture streams (seven visible presentations) have independently FFmpeg
qualified embedded PNG samples and Python float64 sRGB/source-over references.
Decoder samples match within 2e-7. Actual Metal on Apple M4 Max passes RAM lease,
swap pressure/retry, exact restored samples/metadata and exact restored rendered
frames; independent-reference rendered frames differ by at most one 8-bit code.
This does not qualify an external MNG compositor, physical display, arbitrary
profiles, tRNS variants, BACK/TERM/control chunks or full MNG conformance.
Evidence: scripts/generate-mng-alpha-fixtures.py,
tests/fixtures/mng/alpha-manifest.json, mng::tests and mng_readback.

MNG local-transparency qualification adds RGB8 tRNS, grayscale8 tRNS,
palette8 tRNS and grayscale-alpha8: four additional CC0 streams whose PNG
samples are FFmpeg-qualified and linear float presentations use independent
Python references. Eleven alpha presentations now traverse RAM/swap/Metal.
Initial background validation also rejects a transparent/partial first
presentation when the profile forbids background transparency, even if a later
opaque canvas would replace it. Global palettes/transparency, 16-bit tRNS,
BACK/TERM and other MNG controls remain unqualified.

MNG-LC global-palette subset: top-level PLTE/tRNS snapshots are inherited only
through an empty embedded PLTE; an embedded tRNS overrides inherited alpha.
The parser admits the simple-profile bit while still refusing unsupported
control chunks. Missing/nullified palettes and global palette use under a VLC
profile are refused. Two CC0 global-palette fixtures reuse FFmpeg-qualified
source samples plus independent Python linear references, and pass actual
Metal, RAM lease and pressure-preserving swap restoration. Thirteen alpha
presentations are now exercised. This is not full MNG-LC qualification; framing,
object placement, backgrounds, loops, TERM and other control semantics remain.
Specification: https://www.libpng.org/pub/mng/spec/mng-lc.html section 4.2.2/4.2.3.

MNG palette follow-up: a three-layer regression verifies palette snapshots,
changed global alpha and tRNS nullification without recoloring preceding layers.
RGB16 tRNS, grayscale16 tRNS and grayscale-alpha16 now have FFmpeg-qualified
native source samples and independent Python linear float references. Neighboring
16-bit values straddling the transparency key and alpha=1/65535 are included.
All sixteen file-backed alpha presentations pass RAM leases, swap admission
pressure/retry and actual Metal Apple M4 Max readback. Thirteen MNG unit tests
and two MNG integration tests pass. These checks do not establish complete
MNG-LC control/timing behavior or physical HDR display support.

MNG-LC filter64 subset: native RGB/RGBA8/16 now restores modular intrapixel
R=S0+G and B=S2+G after standard PNG unfiltering. RGB tRNS keys are mapped into
the differenced domain before decoding. Mutation requires an exclusive native
pixel allocation and avoids a second full sample buffer. Four authored streams
(eight presentations) compare against separately FFmpeg-qualified ordinary PNG
sources and independent Python linear references, and pass actual Metal,
RAM/swap pressure/retry and exact restored samples/rendered frames. The authored
filter64 IDAT rows use PNG filter0; other adaptive filters/interlace variants
and an external MNG decoder oracle remain unqualified. Regression: core 78
passed; decode 610 passed/34 ignored; MNG Metal integration 2 passed.

MNG filter64 qualification now adds non-interlaced 2x5 RGBA8/RGBA16 fixtures
with all five PNG row filters and 7x6 Adam7 RGBA8/RGBA16 fixtures (pass rows use
filter0). FFmpeg independently checks undifferenced PNG reference samples;
Python independently encodes modular differencing/filter predictors/pass layout
and linear source-over references. All 28 file-backed alpha presentations pass
managed decoding, RAM leases, swap pressure/retry and actual Metal readback.
Fourteen MNG unit tests and both MNG integration tests pass. Exclusive native
slice mutation also has a shared-owner refusal and last-owner budget test.
Combined adaptive filters within Adam7 passes and an external MNG compositor
remain unqualified; full MNG control/timing and physical display claims remain
open.

STI/STCI native foundation: indexed8 planes and ETRLE subimages now route through
strong magic and common raster/image APIs. Every subimage directory range and
dimensions are bounded, selected RGBA output is admitted before allocation,
rows/run lengths/palette indices/exact selected-stream termination are validated,
and cancellation is checked during decode. Authored tests cover selection,
transparent runs, truncated prefixes, insufficient budget and last-owner credit.
Header/layout reference: JA2-Stracciatella src/sgp/ImgFmt.h and STCI.cc.
RGB/zlib layouts, application-data/placement interpretation, independent fixtures
and RAM/swap/GPU qualification remain open. The format matrix is now 82
implemented subsets and 18 Pending; no full-format qualification is implied.

STCI RGB follow-up: uncompressed packed 16/24/32-bit RGB layouts now validate
non-overlapping contiguous channel masks and matching declared depths (up to
16 bits per channel). Samples normalize to RGBA16 without an intermediate
8-bit reduction; optional stored alpha and whole-pixel transparency keys are
retained. Output admission precedes allocation. Authored RGB565/RGBA32 tests
cover normalized samples, alpha, invalid masks, RAM refusal and last-owner
credit. Full decoder suite: 612 passed, 34 ignored. Independent producer files,
RGB24-specific fixtures, zlib and RAM/swap/Metal qualification remain open.

STCI zlib follow-up: indexed8 and packed RGB zlib payloads now admit a bounded
expanded staging buffer before inflation, validate original dimensions/size,
checksum, exact expanded length and compressed-stream consumption, and poll
cancellation between 64KiB output chunks. Combined zlib+ETRLE remains refused.
Application bytes are bounded and preserved through staging but not interpreted.
Authored indexed and RGB24 tests cover pixels, app-data bounds, checksum failure,
trailing compressed bytes, invalid declared sizes, truncated prefixes and RAM
refusal/cleanup. Decoder regression: 614 passed, 34 ignored. Independent STI
producer files and end-to-end RAM/swap/Metal qualification remain open.

STCI file-backed storage/GPU qualification: nine CC0 authored files contain ten
selected images across indexed8, ETRLE, RGB565, RGB24 and RGBA32 plus indexed/RGB
zlib variants. Native and prepared samples match independent Python authored
references. All twenty native/prepared combinations retain exact samples,
selection and color through RAM leases and streaming swap, preserve swap entries
under restore RAM pressure, and render identically on actual Metal Apple M4 Max.
A misleading .mrw suffix still routes STCI magic and selected subimages through
the common image API. Source/generator and sample planes are checked in under
scripts/generate-sti-fixtures.py and tests/fixtures/sti; two sti_readback tests
pass. This is authored-reference evidence, not independent STI producer/decoder
qualification. Sprite placement and application data semantics remain open.

Independent STCI ETRLE producer evidence: seven real JA2-Stracciatella assets
at commit c527753eb635216c21bfcbeb058b6ed86823072a contain twenty subimages.
An unmodified standalone upstream Rust STCI parser qualifies palette/indices and
metadata. Coverage is qualified by the extracted unmodified engine function
Blt8BPPDataTo16BPPBufferTransparent using identity palette values and an unwritten
sentinel: a literal palette index zero is opaque, unlike a transparent run.
Native samples, subimage dimensions/count, prepared linear pixels and actual
Metal frames all match. The initial oracle wrapper incorrectly treated literal
zero as transparent; it was corrected without changing production decoding.
Source/plane hashes and pinned URLs are in sti-independent-qualification.json;
assets remain external because redistribution rights are unqualified. Opt-in
sti_readback test uses RRRAH_STCI_EXTERNAL_DIR and passes. Independent RGB/zlib,
application-data/placement interpretation and physical color remain open.

Independent STCI storage extension: all twenty real ETRLE subimages, both native
and prepared, now pass RAM lease protection, streaming swap exact restoration,
shared-root restore pressure/refusal/retry and exact Metal frames. Forty tested
combinations retain selection/color and release managed credit at final owner
drop. Opt-in independent_stci_producer_images_match_external_parser_and_metal
passes; the qualification JSON records this storage scope. This still does not
qualify independent RGB/zlib producers or sprite placement semantics.

STCI measured CPU baseline: 100 warm-process/warm-filesystem repetitions per
independent ETRLE subimage (20 images, 2,000 measurements) produce CPU-ready
p50 ranges 0.01025-0.164667 ms and maximum per-image p95 0.175709 ms on Apple
M4 Max. Profile is Cargo dev (local opt-level=1, dependencies opt-level=3), not
release. Each iteration verifies the prepared pixel fingerprint. These small
15x15 to 575x30 assets are not representative of large RAW or cold storage;
this timing excludes GPU upload/presentation and decoded RAM/swap hits.
Full per-image evidence: docs/research/sti-load-timing.json.

HDR window configuration foundation: explicit RRRAH_HDR_SURFACE=1 now selects
only an advertised RGBA16Float/ExtendedSrgbLinear pair, configures that color
space, sets RAW SceneLinear output and instantiates the raster linear-HDR
renderer. Unsupported HDR pairs fail initialization. Default SDR behavior remains.
Format/color-space pairing unit test passes; app regression 114 passed/10 ignored,
actual Metal raster/RAW linear readback regressions pass. No live-window or
physical-display HDR proof is claimed: display state/headroom, surface activation
and UI luminance still require runtime qualification.

Live HDR surface probe: hdr_surface_probe creates a main-thread winit window,
queries actual Metal surface capabilities/display state, configures
RGBA16Float/ExtendedSrgbLinear and creates a linear raster pipeline. On this
Apple M4 Max environment the pair is advertised and configuration/pipeline
validation returns None (no validation error). Display headroom reports current
1.0, potential 16.0, coarse HDR false before/after configuration. Drawable
acquisition remained Occluded through 120 redraw attempts, so the probe exits
with an error and explicitly reports no presented frame. This is configuration
and capability evidence only; live HDR presentation/physical luminance/color
qualification remain open. Run cargo run -p rrrah --example hdr_surface_probe
in a drawable desktop environment to continue this check.

SVG output-admission correction: resvg/tiny-skia rasterization now reserves final
RGBA capacity before Pixmap allocation, transfers that reservation into returned
managed pixels without copying, and polls cancellation during alpha unassociation.
A 10x10 fixture refuses a 399-byte output budget before any output reservation
(peak zero), admits exactly 400 bytes and retains credit through raster clones
until the final owner drops. Decoder regression: 615 passed, 34 ignored. SVG
parsing/tree/renderer scratch remains outside this final-output accounting;
this is not a total RSS bound. WMF specification review located this shared
vector-rendering issue; WMF implementation remains Pending.

### Placeable WMF foundation (2026-10-06)

`wmf.rs` now validates the placeable checksum, declared metafile size, bounded
object table, record lengths and EOF. The admitted geometry subset includes
anisotropic window coordinates, solid/null brushes and pens, rectangle, ellipse,
and move/line commands; unknown records are refused. Nonzero image selection is
explicitly refused. The shared SVG renderer reserves its final managed output
before pixmap allocation. Parser/tree scratch is not covered by that reservation.

Three CC0 Python-authored 10x10 opaque rectangle files have mathematical RGBA
references. The application integration test checks their exact pixels through
`decode_image`, including misleading `.mrw` suffixes, managed ownership release,
invalid image selection and uppercase gallery discovery. Decoder regression:
617 passed, 34 ignored; WMF routing integration: one passed. These fixtures do
not establish independent WMF renderer agreement, stroke/mapping equivalence,
RAM/swap or actual GPU readback qualification. WMF remains Pending in the full
format qualification matrix until that evidence and broader record support exist.

WMF storage/GPU follow-up: all three authored rectangle images now pass native
RGBA8 and prepared float32 stream-swap round trips (six combinations). Restore
under exhausted managed RAM refuses admission without treating the entry as
corrupt; retry after releasing pressure succeeds with exact native samples and
color metadata. An active RAM cache lease prevents a zero-byte limit change.
Reference, prepared, leased and restored displayed frames agree exactly on actual
Metal Apple M4 Max. Both WMF application integration tests passed in 0.17s
(`/tmp/rrrah-wmf-metal.log`). All managed CPU credits return to zero after owners
are dropped. This supersedes the preceding missing RAM/swap/GPU evidence for
these authored fixtures only; independent WMF renderer and broader commands
remain unqualified, so the full-format matrix status remains Pending.

WMF polygon follow-up: META_POLYGON and META_POLYLINE consume signed bounded
point counts and exact PointS arrays (x,y), and SETPOLYFILLMODE selects even-odd
or nonzero fill. Polylines ignore the brush and do not update the current point.
An authored full-canvas polygon adds a fourth mathematical-reference fixture;
all four images pass the RAM/swap/Metal pipeline above (eight native/prepared
swap combinations), two app tests in 0.18s on Metal Apple M4 Max. Negative,
undersized and mismatching point counts are refused; a null-pen polyline leaves
transparent pixels despite an active solid brush. Decoder regression: 618 passed,
34 ignored. Stroke rasterization and independent renderer qualification remain
open. Record layout reference:
https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-wmf/0982bbfc-feb7-4f06-a8fb-ad03b465ffea

WMF independent renderer follow-up: libwmf 0.2.16 was built unchanged from its
release source archive in `/tmp` with bundled GD. Four authored 10x10 files render
to PNG with `wmf2gd --maxwidth=10 --maxheight=10 --maxsize`; independent FFmpeg PNG
decoding matches every mathematical RGBA sample exactly. The source archive and
fixture/reference hashes are recorded in
`tests/fixtures/wmf-independent-qualification.json`; the reproducible comparison
entrypoint is `scripts/qualify-wmf-libwmf.py`. Explicitly created null pens replace
stock-object references in these file fixtures because libwmf rejects the latter
as out-of-range object indices. App RAM/swap/Metal tests were rerun and both pass
(0.20s). This establishes independent renderer agreement for these basic fills
only, superseding the preceding missing independent-renderer evidence there.
Independent producers, stroke fidelity, full mapping and broader WMF commands
remain unqualified, and the full-format matrix stays Pending.

### Measured RAW cache route comparison (2026-10-06)

The existing `raw_view_timing --cache-only` entrypoint measured native CR3, MRW
and DNG input on Apple M4 Max: four warmup and 16 measured rounds, rotating route
order, exact metadata/sensor sample comparisons after each decode or restore.
The percentile calculation now uses nearest rank (16-sample p95 is rank 16).
`docs/research/raw-cache-routes-2026-10-06.{json,csv}` retain source hashes, profile,
scope and individual measurements. CPU-ready p50 milliseconds, decode versus
stream-swap: CR3 43.598 / 33.370; MRW 5.230 / 7.900; DNG 201.599 / 16.359.
RAM hit averages are 0.000156–0.000163 ms per operation (1000 hits per round),
not a foreground frame latency measurement. Every root/restore managed budget
returned to zero. These warm filesystem/dev-profile results support choosing
routes by measured source cost: swap is slower than decode for this MRW, faster
for these CR3/DNG cases. They exclude cold I/O, swap write/setup, GPU upload,
presentation and navigation scheduling, and do not establish a universal RAW
speedup or user-visible load time.

RAW automatic-policy foreground correction: the RAM resident lookup now precedes
`RawLoadCosts::decode_first`. A resident hit therefore neither advances the
alternative-route probe counter nor initializes/changes decode-vs-restore
history; only a RAM miss selects those routes. The duplicate resident lookup in
the decode-first branch is removed. Lookup timing still starts before the
resident query. This aligns the existing every-16-request alternative sampling
with actual nonresident loads rather than arbitrary RAM navigation hits.

WMF saved-context follow-up: SAVEDC retains pen, brush, window mapping, current
point and polygon fill rule, with a 256-context bound. RESTOREDC supports signed
relative and positive absolute saved indices and removes the restored state and
all newer stack entries; zero/out-of-range indices are refused. A fifth authored
fixture changes brush and window origin inside a saved state and restores the
original full red fill. It agrees exactly with unchanged libwmf 0.2.16, and all
five fixtures pass RAM/swap/Metal tests (ten native/prepared swap combinations;
two application tests, 0.23s). Decoder tests verify positive index restoration,
invalid extreme indices and stack overflow: 619 passed, 34 ignored. This stores
only currently admitted properties; unknown state commands are still refused,
and broader/full WMF qualification remains Pending.

WMF object lifecycle follow-up: DELETEOBJECT removes only existing unselected
objects and frees their indices for lowest-slot allocation. Active or saved
context references, absent slots and invalid indices explicitly fail. The sixth
authored fixture switches to a replacement brush, deletes the prior brush,
reuses its index and selects it again; unchanged libwmf output matches every
sample. All six images pass application RAM/swap/Metal qualification (twelve
native/prepared swap combinations; two tests 0.22s). Decoder regression: 620
passed, 34 ignored. The external release's `examples/arrow01.wmf` now decodes
successfully with stable repeated prepared-image digest; its source hash and
limited smoke scope are recorded in `wmf-external-producer-smoke.json`. No pixel
agreement claim is made for that external producer file yet. Selected-object
release semantics, stroke fidelity and remaining command support stay open.

External WMF pixel comparison is now performed for `arrow01.wmf`: libwmf's
170x159 PNG disagrees with native output in 14,266 of 27,030 pixels. A major
contributor is external opaque white background versus native transparent
background; compositing native samples over explicit white still leaves
mismatches recorded in `wmf-external-producer-smoke.json`. Stroke/mapping and
rasterization causes are not yet isolated. The opt-in
`external_arrow01_matches_libwmf_exactly` test requires source/reference env vars
and currently fails, preserving the exact-agreement requirement; it is ignored
in normal runs because the external release files are not redistributed here.
This contradicts full independent-producer qualification and does not invalidate
the separate six authored solid-fill reference comparisons.

WMF viewport follow-up: explicit viewport origin/extents now participate in
logical-to-device mapping and horizontal pen width scaling. Signed extents allow
reflections; zero extents fail. SAVEDC/RESTOREDC also retain viewport state.
Three mathematical tests cover positive scale, reflected axes and translated
origin with exact opaque samples. Decoder regression: 621 passed, 34 ignored.
This extends coordinate support but does not explain the external arrow01
mismatch, whose input has no explicit viewport records. Other map modes and
independent producer/stroke fidelity remain open.

### RAW completed Metal-frame timing (2026-10-06)

The existing `raw_view_timing` entrypoint measured native decode, renderer upload
and completed offscreen 1920x1080 SDR frames on Metal Apple M4 Max, with separate
256 MiB managed CPU/GPU budgets. Three warmup and 15 measured loads per source,
plus 60 measured resident zoom/pan frames, are recorded in
`docs/research/raw-metal-view-2026-10-06.{json,csv}`. Full-load p50/p95 milliseconds:
CR3 72.353/74.690, MRW 9.392/9.986, DNG 219.376/221.286. Resident interaction p50:
1.373, 0.656, 0.640 ms respectively. Managed peak CPU 124,360,466 bytes and GPU
146,800,640 bytes; both used counters return to zero. Each sensor decode is
compared exactly and every GPU submission is waited to completion. The timing
entrypoint does not read back pixels, exercise RAM/swap routes, present a
swapchain or establish physical display/HDR latency; warm filesystem/dev profile
and offscreen scope prevent a universal viewer-speed claim.

WMF ROUNDRECT follow-up: signed corner-ellipse width/height are transformed through
window/viewport scale, converted to radii and clamped to the bounding rectangle.
The shared SVG backend renders the resulting rounded rectangle. Mathematical
unit checks cover zero diameter, full-diameter, oversized and negative diameters,
opaque center and transparent rounded corners. Decoder regression: 622 passed,
34 ignored. The attempted independent GD oracle is unsuitable: libwmf 0.2.16's
`src/ipa/xgd/draw.h::wmf_gd_draw_rectangle` calls plain filled/outline rectangle
functions without corner dimensions; its diameter-10 output keeps the corner
opaque red. `wmf-roundrect-qualification.json` records this contradiction and
marks curved-corner qualification false. No independent curved-edge agreement
is claimed, and full WMF qualification remains Pending.

Performance evidence consistency follow-up: raster_load_bench, decode_timing and
cr3_end_to_end_timing now use explicit nearest-rank percentile calculations,
matching raw_view_timing and the memory policy bench. A regression example test
covers p95 selecting the final rank in three/sixteen samples and one-sample
boundaries; all decoder examples compile. The prior 100-repeat STI p50/p95 ranks
are unchanged by this correction; short old runs may have understated p95.
Six authored 10x10 WMF cases were measured with the corrected raster_load_bench:
100 repeats each with stable full prepared-sample digest. CPU-ready p50 range
0.013917–0.015917 ms, maximum per-case p95 0.024583 ms. The complete report/source
hashes are in `docs/research/wmf-load-timing.json`; these warm CPU/dev-profile
small-fill measurements exclude GPU, caches, presentation, complex geometry
and external producer qualification.

### Real RAW prefetch limit qualification (2026-10-06)

Three existing opt-in worker tests were executed with current native decoder,
Canon EOS R8 CR3 files and the pinned Sony DSC-F828 SRF corpus. Byte-budget
pressure preserves an already retained priority neighbour while refusing another
admission. A count cap of one stops lower-priority missing-file work after the
nearest source is stored. A missing first neighbour does not consume retained
entry count: the following real SRF is stored and its restored RGBE layout is
checked. All three passed (byte case 0.59s, two count cases 0.42s), managed memory
returns to zero. Source hashes and assertion scope are recorded in
`docs/research/raw-prefetch-limits-2026-10-06.json`. These are actual background
worker/storage tests, not physical UI navigation/latency proof; tests remain
opt-in because required external/local camera files are not generally available.

Gallery scan memory correction: supported directory entries are now selected
through a bounded max-heap while iterating, retaining at most MAX_ITEMS=10,000
candidates rather than collecting every image before truncation. Cover selection
uses the same streaming selection with limit one. Case-folded filename ordering
is preserved, with original path as deterministic tie-breaker. A 30,002-path
regression checks equality with a full-sort prefix, mixed-case ties, empty/zero
limits and one-entry cover ordering. This bounds candidate count, not path byte
lengths or total process RSS; directory traversal remains O(N), selection is
O(N log MAX_ITEMS). Existing gallery filtering and symlink rules remain applied
before admission.

Catalog reconciliation (2026-10-06): the current IMAGE_FORMATS_100 implementation
column now records the WMF geometry foundation as an implemented subset, while
its completion column retains external-producer mismatch and missing-command
limits. STI RGB/zlib/external blitter/storage/GPU and MNG alpha/LC evidence are
also reflected in their matrix rows. Exactly 100 numbered rows remain: 83
implemented subsets, 17 Pending ordinary viewer families, zero fully qualified.
EIP retains explicit import APIs but its ordinary viewer status remains Pending.
The machine-readable consistency audit is
`docs/research/format-matrix-audit-2026-10-06.json`. Historical statements that WMF
is Pending refer to full-format qualification; the latest table distinguishes
implementation presence from that unachieved qualification. This is bookkeeping
against current source/evidence, not a claim that 83 formats are fully correct.

EMF foundation: the base 88-byte header admits signature/version/exact file size,
no-description/no-palette subset and bounded record count; every record is
32-bit aligned and bounded, EOF validates final count/length. Stock white/black/
null brushes and pens plus rectangles/ellipses translate to shared SVG output.
Nonzero image indices and all other records explicitly fail; output and SVG
sizes are capped and managed output is admitted before pixmap allocation.
Own authored stock-black null-pen rectangle checks exact opaque samples,
399-byte refusal versus 400-byte admission, every truncated prefix, record
count/alignment/unknown-record/EOF corruption. Decoder regression: 624 passed,
34 ignored. File-backed external oracle, cache/swap/Metal and broader EMF records
remain pending; the matrix is not advanced on this unit-only foundation yet.

EMF file/storage/GPU follow-up: CC0 base-header black/white null-pen rectangles
have separate mathematical RGBA planes and SHA-256 manifest, generated by
`scripts/generate-emf-fixtures.py`. Both pass common decode_image routing under
misleading MRW suffix, exact sample/selection/color checks and managed lifetime
release. Native and prepared variants pass RAM lease protection, streamed swap
pressure refusal preserving entries and successful retry, exact samples and
actual Metal frame readback against mathematical references (four swap combos).
Two app tests pass in 0.32s; all tracked CPU credits return to zero. The matrix
now has 84 implemented subsets / 16 Pending ordinary viewer families, with zero
full-format qualification. Independent EMF producer/renderer, extended headers,
object/transforms/clipping/fonts/bitmaps and color semantics remain open.

EMF independent-renderer follow-up: wmf2svg 0.10.4 Java2D PNG was run through a
checksum-verified portable Temurin JRE in `/tmp`. Header Bounds are
inclusive-inclusive, so native dimensions now use right-left+1/bottom-top+1;
fixture header bounds are 0..9 while drawing rectangle right/bottom are 10.
Physical Frame corners are 0..225 hundredths of mm for reference device 100px /
25mm (9*25), matching the inclusive endpoint contract. Both authored black/white
10x10 frames match the independent Java PNG's dimensions and every RGBA sample,
via separate FFmpeg PNG decoding. Renderer/runtime/fixture hashes and scope are
in `tests/fixtures/emf-independent-qualification.json`. Decoder regression 624
passed/34 ignored; RAM/swap/Metal tests rerun, two passed (0.22s). Independent
producer and broader EMF/physical viewer qualification remain open.

EMF custom-object follow-up: CREATEBRUSHINDIRECT admits solid/null RGB brushes;
CREATEPEN admits solid/null RGB pens, nonnegative x widths and zero y width.
Header handles bound the table to 4096 slots; zero/reserved, out-of-range,
uncreated and duplicate indices fail. Three authored red/green/blue fills extend
the corpus to five images, all matching the independent Java2D PNG renderer
exactly and passing RAM/swap/Metal (ten native/prepared swap combinations, two
app tests 0.22s). A custom null-pen test verifies brush fill remains exact; bad
indices/style/palette colors/table limits fail. Decoder regression: 625 passed,
34 ignored. Independent non-null pen stroke rasterization, object deletion,
transforms, clipping and broader EMF remain unqualified.

EMF oracle reproducibility: `scripts/qualify-emf-java.py` now reproduces every
manifest entry against a separately supplied Java renderer. It verifies source
SHA-256, rendered PNG dimensions, expected RGBA length and exact decoded samples,
and records renderer hash/runtime/PNG-decoder versions. Running it with the
pinned wmf2svg 0.10.4 JAR and portable Temurin reproduced all five comparisons,
with identical renderer/fixture/reference hashes to the earlier saved report.
The catalog EMF row now reflects custom objects and this evidence while keeping
full qualification pending. The script does not fetch/install a runtime or
replace an external producer/pen-stroke qualification.

EMF context follow-up: SAVEDC/RESTOREDC preserve currently admitted fill, stroke
and pen width; restore indices must be negative and relative per MS-EMF. State
stack is limited to 256; zero/positive/underflow and excessive stack depth fail.
A sixth authored fixture switches to black within a saved red context and then
restores red: independent Java2D dimensions/samples agree exactly, and all six
cases pass native/prepared RAM/swap/Metal (twelve swap combinations). App tests
pass in 0.27s. Saved state supports only admitted properties; transforms/clipping/
fonts/bitmaps and broader/full EMF remain pending.

### EMF polygon follow-up (2026-10-06)

EMR_POLYGON/POLYLINE and their 16-bit variants now validate exact array lengths and a 16,384-point admission cap, check cancellation while traversing points, and preserve polygon fill mode across saved contexts. Open lines ignore the brush. Eight authored fixtures agree pixel-for-pixel with the pinned independent Java2D renderer; all eight pass native/display RAM leases, pressure-preserving streamed swap, and actual Metal readback. Decoder regression: 627 passed, 34 ignored; EMF application integration: 2 passed. These examples do not qualify independent producers, non-null pen rasterization, transforms, clipping, or complete EMF. The 100-format objective remains incomplete.

### EMF overlapping fill modes (2026-10-06)

Three new double-wound polygon fixtures verify even-odd versus nonzero fill and SAVE/RESTORE fill-mode state against mathematical RGBA references. All 11 authored fixtures pass native routing/admission and prepared/native RAM, streamed swap and actual Metal frame readback (2 integration tests). Independent Java2D comparison passes 10/11, with all 100 winding pixels differing. Pinned AwtGdi polygon() constructs java.awt.Polygon without consulting the fill mode, so this oracle cannot qualify nonzero direct polygons. The strict comparison script now saves mismatches before returning failure; expected pixels remain unchanged. Full EMF qualification remains incomplete. See tests/fixtures/emf-fill-qualification.json.

### EMF independent nonzero fill qualification (2026-10-06)

The unmodified pinned wmf2svg SVG backend emits the winding polygon with explicit fill-rule=nonzero. Separate CairoSVG 2.8.2/Cairo rasterization matches all 11 authored mathematical RGBA fixtures exactly, including even-odd and restored fill mode. scripts/qualify-emf-java.py now supports --svg-via-cairo without changing its strict failure behavior; tests/fixtures/emf-svg-cairo-qualification.json records versions, package pins, library hash and reproduction instructions. The original Java2D 10/11 mismatch remains preserved. This qualifies these authored cases, not arbitrary independent producers or full EMF. No production-source changes required in this follow-up.

### EMF current drawing point (2026-10-06)

MOVETOEX and LINETO now validate exact 16-byte records, use signed logical coordinates with header-origin mapping, update the current point even with a null pen, and preserve it through SAVE/RESTORE. Two full-frame, width-10 solid pen fixtures distinguish restored-current-position and null-pen advancement. All 13 authored cases match unmodified independent EMF-to-SVG/Cairo output exactly and pass native/prepared RAM leases, streamed swap under pressure and actual Metal readback (2 integration tests). Decoder regression: 627 passed, 34 ignored. Endpoint exclusion and arbitrary GDI cosmetic/non-null stroke rasterization remain unqualified; these aligned wide-line examples do not prove full EMF.

### RAM cache TTL-history eviction (2026-10-06)

WeightedLru now counts resident deadline-bearing entries rather than retaining a permanent historical TTL flag. Removing, expiring, evicting or replacing the last such entry returns eviction/pruning to the indexed no-deadline path; pin and fixed insertion-age TTL semantics stay intact. Regression covers pinned expiry, replacing with no deadline and explicit removal. An older test now constructs its deadline via public limit/insertion behavior rather than mutating a private entry and bypassing accounting. rrrah-memory: 31 unit + positive TTL ownership integration + doc test passed; rrrah-swap: 7 passed; application regression passed (see current log). Sequential policy-only before/after measurements at 16,384 entries changed single-victim admission p50 from 180.625 us to 0.500 us. These timings exclude pixels, decode, disk, GPU and display and do not establish viewer speed. Recorded in docs/research/cache-ttl-history-2026-10-06.json and raw CSVs.

### Expired RAM values transferable to swap (2026-10-06)

WeightedLru and LeaseCache expose drain_expired(), returning owned key/value victims without cloning payloads or performing I/O. Owner pins and active consumer leases retain expired entries; deadlines remain fixed insertion age. Existing prune_expired() keeps its discard behavior without allocating an additional victims vector. A real 1-second TTL integration uses a 96-KiB managed allocation: pointer identity survives cache-to-caller transfer, root budget remains charged while the writer owns the buffer, streamed swap succeeds, RAM-pressure restore refuses without losing the disk handle, and retry after releasing the victim restores exact bytes. Last-owner drops release both RAM and disk quotas. Explicit owner-pin expiry has a deterministic unit regression. Memory 32 units + TTL integration + doc test; swap 7 units + cache/spill integration; application 115 passed, 10 ignored. This adds the reusable library contract; automatic app TTL-to-swap scheduling remains a separate integration task.

### Loader TTL spill integration (2026-10-06)

MosaicRamCache, RasterRamCache and ModelDisplayCache drain expired unleased/unpinned values into their existing bounded swap queues before ordinary resident lookup or admission. RAW resident-only lookup performs TTL maintenance without restoring disk itself. Queue admission remains asynchronous and can refuse under count/byte pressure; pending writes do not block foreground loading or count as saved. Without swap, expired victims drop. Existing expiry deadlines are unchanged; visible owner pins and live leases stay protected. New regressions prove TTL spill despite spare RAM capacity for RAW pixels/metadata and HDR float bit patterns, and completed streamed restore without decoder calls for STL/OBJ/PLY/OFF with exact serialized payload and final root-credit release. Cache tests: 111 unit + 5 integration passed; app 116 passed, 10 ignored. TTL-enabled cleanup currently scans resident entries; no claimed viewer-latency improvement from this integration. Physical UI navigation, broader formats and hardware qualification remain open.

### TTL maintenance fast path (2026-10-06)

WeightedLru tracks a conservative earliest unprotected deadline and skips whole-cache expiry traversal before it is due. A due scan recalculates the next live unprotected deadline; pinned expired entries no longer force repeated scans, and unpinning restores their deadline immediately. Replacements/removals may leave only an earlier stale hint, causing one harmless recalculation. Deterministic boundary tests cover future deadlines, pinned expiry, unpin and exact deadline equality. Regression: cache 111 unit + 5 integration; memory 33 unit + positive TTL integration + doc test; swap 7 unit + TTL spill integration; app 116 passed, 10 ignored. At 16,384 entries policy-only drain+hit p50 changed from 42.641 to 0.042 us; raw timings and scope in docs/research/cache-ttl-maintenance-2026-10-06.json. This excludes LeaseCache reconciliation and full viewer timing.

### Positive TTL + streamed swap + actual Metal (2026-10-06)

New ttl_swap_metal integration passed both real IMG_9043.CR3 and synthetic linear-float HDR cases on Metal Apple M4 Max. A one-second TTL expires while an active lease still protects pixels; readback remains identical, no spill occurs until last lease release, then completed swap releases managed CPU credit. Restored RAW samples/metadata and HDR float bit patterns are exact, restored Metal frames equal original. HDR admission pressure retains swap for successful retry. RAW/raster get_lease() now performs TTL spill maintenance before lookup instead of allowing an unleased expired value to be discarded by lookup. Cache regression 111 unit + 5 integration passed. This is SDR offscreen readback with HDR input, not physical HDR/swapchain or persistent GPU-residency qualification; full 100-format goal remains open. See docs/research/ttl-swap-metal-2026-10-06.json.

### EMF object lifetime (2026-10-06)

EMR_DELETEOBJECT validates a created nonzero/nonstock index, clears the slot for reuse and resets selected brush/pen defaults. Selection identities now travel with saved contexts; deleting an object also clears stored selections so a restored context cannot refer to a reused index. Authored deselected brush index reuse matches independent EMF-to-SVG/Cairo pixels; all 14 authored fixtures pass RAM, streamed swap and Metal readback. Decoder 628 passed, 34 ignored; app EMF integration 2 passed. Invalid delete indexes and selecting a cleared slot fail explicitly; selected brush deletion has a mathematical default-white regression. MS-EMF 2.3.8.3 specifies default restoration for selected-object deletion; both pinned independent renderer backends ignore that side effect, so that behavior and saved-context deletion interactions are not independently qualified. Full EMF and broader goal remain incomplete.

### EMF external corpus and header extensions (2026-10-06)

58 unmodified LibreOffice regression EMF files pinned to commit 65bbb1691f022226390e15b7ea65567970bbc12e were fetched outside the repository and tested with the existing native raster-load entrypoint. Initially 50 failed header admission and 8 failed unsupported records. Fixed 100/108-byte extension headers without description/pixel-format/OpenGL metadata are now admitted, with bounded record start and explicit malformed/GL refusals. Physical micrometer fields are accepted as device metadata; output continues to use declared pixel bounds. Sixteen authored fixtures independently match EMF-to-SVG/Cairo and pass native/prepared RAM/swap/Metal; decoder 629 passed, 34 ignored. After extension support the real corpus still has zero full successes, now 11 header failures and 47 unsupported-record failures. This is negative compatibility evidence, not qualification. Source hashes, URLs and before/after decoder errors are in docs/research/emf-libreoffice-corpus.json; next work must cover variable description headers and drawing/state commands used by real producers.

### EMF UTF16 description admission (2026-10-06)

Variable header sizes now validate alignment, exact declared file bounds and bounded UTF16 description offsets/lengths/terminal NUL without interpreting descriptions as extension fields. Description offsets determine the fixed-header extent; description text is inert metadata and not decoded into drawing commands. Empty optional description fields follow the documented either-field-zero absence rule. Explicit pixel-format/OpenGL refusal remains. Three new base/extension1/extension2 description fixtures bring exact independent EMF-to-SVG/Cairo and RAM/swap/Metal cases to 19. Decoder 630 passed, 34 ignored; EMF integrations 2 passed. The pinned 58-file external corpus was retested with source hashes and before/after errors preserved; zero full successful external decodes remain, so admission progress is not full format qualification.

### EMF mapping modes and reference-device coordinates (2026-10-06)

External record inventory found SETMAPMODE first in 14 files. Fixed modes TEXT/LOMETRIC/HIMETRIC/LOENGLISH/HIENGLISH/TWIPS and ANISOTROPIC plus WINDOW/VIEWPORT origin/extent records are now implemented, with signed-axis affine drawing transforms saved/restored by DC state. Physical modes derive pixel/mm scale from reference device dimensions, with upward Y; isotropic mode remains an explicit unsupported error. Invalid modes, zero extents and physical modes with invalid device dimensions are refused. Five new full-frame mathematical fixtures cover low/high metric, anisotropic scale, restored extent and reflection; all 24 pass native/prepared RAM/swap/Metal readback. Decoder 631 passed, 34 ignored. Independent SVG/Cairo agrees on 21/24: the pinned SvgDc fixed metric factors and reflected rectangle origin differ from these references. Strict failure report preserved at tests/fixtures/emf-mapping-qualification.json; no changed expected pixels or broad qualification claim. General cosmetic pen scaling and mode-transition extent semantics still need qualification, as do arbitrary external producers.

### EMF Bezier and continuation records (2026-10-06)

POLYBEZIER/POLYBEZIER16, POLYBEZIERTO/POLYBEZIERTO16 and POLYLINETO/POLYLINETO16 now validate bounded exact arrays and complete cubic control groups, emit unfilled stroked paths under the active mapping and update current point only for continuation commands. Four continuation regressions use a null pen followed by a reversed visible line, proving endpoint advancement independent of drawing. Six new wide-line fixtures pass native/prepared RAM/swap/Metal; all 30 authored examples pass those paths. Independent SVG/Cairo compares 25/30 exactly: three prior mapping mismatches and two new single-point continuation mismatches. Pinned EmfParser routes POLYLINETO to plain polyline without the current point, yielding empty SVG for those fixtures. Strict failures/source hashes are preserved in tests/fixtures/emf-curves-qualification.json. Bezier independent fixtures are collinear and do not qualify arbitrary curved antialiasing or full EMF. Decoder 633 passed, 34 ignored; app integration 2 passed.

Basis LDR implementation evidence (2026-10-06): pinned basis-universal 0.3.1
transcodes ETC1S/UASTC into managed RGBA8. Four authored constant-white files
provide 16 selected mips. The ordinary router, native/prepared RAM leases,
32 native/prepared streamed swap roundtrips including pressure retry, and
actual Metal Apple M4 Max offscreen readback pass. Decoder regression: 635
passed, 34 ignored; application binary: 116 passed, 10 ignored; Basis Metal
integration: 1 passed, none ignored. These counts cover their test suites, not
all format variants. Same encoder/transcoder and mathematical white reference
do not establish independent codec qualification. Complex textures, alpha,
independent producers, array/cube/Y-flip, KTX2 Basis and direct compressed GPU
upload remain unqualified. Native codec scratch is outside managed output
accounting; this is not an RSS or physical presentation measurement.

Basis colored-alpha extension (2026-10-06): two additional authored 7x5
ETC1S/UASTC files exercise four RGBA block colors with alpha 255/170/85/0,
non-multiple-of-four dimensions and cropped block padding. Every native channel
agrees with the source within fixed absolute error 8/255. The expanded GPU test
covers six files and 18 selected images/mips, with 36 native/prepared streamed
swap restores whose samples are bit-exact. Colored GPU frames are compared to
the admitted initial render, not an independent color-rendering oracle. Actual
Metal Apple M4 Max integration passes; decoder suite is 636 passed, 34 ignored.
Independent producers and spatially complex alpha remain unqualified.

Basis multi-image selection extension (2026-10-06): ETC1S and UASTC authored
files contain distinct red 8x8 and green 4x4 images with complete mip chains.
Image-first indices 0..3 select red mips and 4..6 green; index 7 is rejected
without retained managed memory. Expanded eight-file test covers 32 selections
and 64 native/prepared streamed swap restores on actual Metal Apple M4 Max.
Source colors have fixed per-channel error <=8; cache/swap samples are bit-exact
and offscreen frames match their admitted baseline. Decoder suite: 637 passed,
34 ignored; GPU integration: 1 passed, none ignored. This validates independent
2D images in one file, not array/cube texture semantics or independent producers.

Basis external producer corpus (2026-10-06): eight unchanged upstream files
from BinomialLLC/basis_universal commit
99f52d63aa6799cbdaecfe977111dc5ec3b31d47 were downloaded outside the repository.
URLs and SHA256 pins are in research/basis-upstream-corpus-2026-10-06.json.
Four ETC1S/UASTC files (alpha3, kodim01_mipmapped, kodim03, kodim03_uastc)
decode and pass index-zero RAM lease, native/prepared bit-exact streamed swap,
pressure retry and actual Metal Apple M4 Max offscreen frame preservation.
Four newer codec files (base, kodim23_hdr, desk_6x6, desk_6x6i) explicitly fail
unsupported-format admission. This is external producer compatibility and
preservation evidence, not an independent pixel/color oracle or full Basis
coverage. Warm 20-iteration decode p50 is 1.113..2.702 ms for successful files;
these timings exclude GPU, physical presentation, cold storage and decoded cache.

Basis external all-mip extension (2026-10-06): SHA256-verified upstream corpus
passes scripts/qualify-basis-upstream.py on actual Metal Apple M4 Max. All 10
kodim01 mip levels and three single-level files are tested: 13 selections, 26
native/prepared streamed swap roundtrips, exact samples/metadata and baseline
offscreen frames. Mip dimensions halve to minimum 1; out-of-range selections
refuse without retained managed credit. The runner checks all eight pinned
source hashes before execution and optionally fetches missing files. Successful
preservation still does not establish independent pixel/color correctness.

Lease reconciliation change (2026-10-06): LeaseCache scans consumer Weak
strong counts only after a release notification. CacheLease drops its value
owner before publishing the atomic notification, avoiding a missed final
release; releases during reconciliation remain pending for the next operation.
Cross-thread clone/final release regression passes, along with 34 memory unit
tests, positive TTL integration and doctest. Cache (111 unit + 5 integration)
and swap (7 unit + expired spill integration) pass. Actual Metal Apple M4 Max
TTL CR3/HDR preservation tests pass (2). No end-user latency measurement or
physical RSS claim is made; see research/lease-release-reconciliation-2026-10-06.json.

Lease concurrent-release regression (2026-10-06): 64 repetitions release 16
worker leases concurrently with owner limit changes. Held keeper always blocks
eviction; final release enables transfer of one intact victim, whose managed
64-byte credit is held until victim drop. All 35 memory unit tests, positive
TTL integration and doctest pass. Thread stress exercises observed schedules,
not exhaustive concurrency model checking or measured viewer performance.

Public swap ownership contract (2026-10-06): rrrah-swap crate documentation
now includes a passing executable example for independent RAM/disk lifetimes.
Cloned handles share one disk quota reservation, failed RAM admission permits
retry, last handle drop releases disk quota, and restored RAM remains charged
until its last owner drops. TTL/eviction remain explicitly the responsibility
of the composing cache, not SwapStore. Verified with cargo test -p rrrah-swap
--doc --target-dir /tmp/rrrah-required-corpus-target (1 passed); output at
/tmp/rrrah-swap-contract-doc.log. This is a public API contract check, not
crash durability or performance evidence.

MEF public-route revalidation (2026-10-06): the SHA256-pinned Mamiya ZD
source is rejected by the common TIFF classifier because SubIFD 116920 links
to next IFD 29704. This occurs before unresolved WB admission. An explicit
external-source regression passes and confirms no preview fallback or retained
managed memory. Private sensor qualification is not public viewer support;
MEF remains one of the 15 pending families. See tests/fixtures/mef-investigation.json.

Generic TIFF linked-SubIFD classification (2026-10-06): generic sensor sniffing
now follows linked SubIFDs using existing directory-count, depth and repeated
offset guards. Strict DNG decoding keeps its no-SubIFD-chain contract. Synthetic
linked-tail admission, cycle rejection and strict-DNG refusal pass; full decoder
suite is 639 passed, 35 ignored. Revalidated Mamiya ZD still refuses, now at
unsupported TIFF field type 4084 after the link is followed, with no preview
fallback or retained managed allocation. The external refusal regression passes.
MEF remains pending; no valid photographic color route has been established.

TIFF directory fanout admission (2026-10-06): strict DNG parsing and generic
TIFF sensor classification now share a 256-directory scheduled-work cap.
Completed, queued, current and next-linked directories are counted before
materializing the declared SubIFD offset vector. A root declaring 256 child
offsets refuses DirectoryLimit in both paths; full decoder suite passes
640 tests with 35 ignored. This bounds this traversal allocation, not all
decoder scratch memory or physical RSS. Evidence: research/tiff-directory-fanout-2026-10-06.json.

TIFF directory cap boundary (2026-10-06): a synthetic root with 255 distinct
children (256 total directories) passes strict DNG parsing and generic TIFF
sensor classification. Adding one next-IFD link rejects DirectoryLimit in both
paths. The focused boundary regression passes; production code is unchanged
since the preceding 640-test decoder run. This verifies admission at the exact
cap as well as refusal above it.

Prefetch stale-result budget regression (2026-10-06): existing model/raster
generation-cancellation test now verifies managed allocation peaks and final
release. Real OFF and GIF decodes allocate before generation changes; neither
stale result enters the cache and root/raster child usage returns to zero.
Already-cancelled requests still do not invoke decode. Focused app test passes;
this is adapter-level cancellation evidence, not physical navigation latency
or exhaustive concurrent scheduling coverage. See research/prefetch-cancel-budget-2026-10-06.json.

RAW prefetch terminal disk cancellation (2026-10-06): preload_raw now rechecks
generation after persistent restore and treats CacheError::Cancelled as terminal
instead of falling through to native decode as a cache miss. Application tests
pass (116, 10 ignored) and the real EOS R8 RAM/disk preload regression passes
explicitly. Existing tests preserve valid RAM/disk hits and visible pin behavior;
the exact cancellation boundary during disk-to-decode transition is not
deterministically injected and no latency improvement is claimed.

Lease lookup timing (2026-10-06): executable lease_lookup_timing compares
steady get_cloned lookups with 1/64/1024/16384 live consumers. Current p50
is 19.129/24.275/25.238/28.558 ns; source-identical temporary counterfactual
forcing reconciliation on every lookup is 20.146/65.179/767.188/14208.825 ns.
Twenty samples of 10000 operations follow three warmups, with checked results
and ownership validation. This isolates scan avoidance under no releases;
frequent releases still scan. Sequential runs are not a paired randomized
benchmark; this does not measure decode, GPU, UI latency or physical memory.
Raw CSVs and protocol are in research/lease-lookup-timing-2026-10-06.json.

PDF renderer feasibility (2026-10-06): isolated pinned hayro 0.7.1 project
scripts/pdf-render-feasibility builds and renders two authored 10x10 PDF pages
with exact opaque red/green RGBA. Independent Poppler RGB planes also agree
exactly. This is renderer feasibility only; ordinary PDF routing, managed
pre-allocation admission, alpha conversion, cancellation, selected pages and
RAM/swap/Metal integration remain pending. PDF stays in the 15 pending families.
Evidence: research/pdf-render-feasibility-2026-10-06.json.

PDF ordinary decoder integration (2026-10-06): pinned Hayro 0.7.1 is now
connected through signature/extension routing and selected page indices.
Geometry must be finite, 1..65535 per dimension, RGBA <=512 MiB, pages <=4096.
Major PDF source copy, pixmap and final RGBA ownership reserve managed credit
before allocation; straight-alpha conversion follows premultiplied rasterization.
Two authored opaque pages match their mathematical/Poppler oracle through
decode_image; budget release and insufficient-budget/index refusal pass.
Full decoder suite: 642 passed, 35 ignored. Native rendering is only cancellable
before/after and during conversion; parser/interpreter scratch remains outside
these major-buffer reservations. Fonts/color/transparency, wider producers,
RAM/swap/Metal and physical presentation remain unqualified. Catalog now has
86 implemented subsets / 14 pending / zero fully qualified families.

PDF RAM/swap/Metal integration proof (2026-10-06): both authored opaque
selected pages route through decode_image even under misleading .mrw suffix.
RAM leases protect frames against zero-limit shrinking; four native/prepared
streamed swap restores retain samples bit-exact, page indices/count/color,
and successful retry after managed-budget pressure. Actual Metal Apple M4 Max
offscreen frames agree with independent mathematical red/green float reference.
Managed root usage returns to zero. GPU test: 1 passed, none ignored; app binary:
116 passed, 10 ignored. Scope remains two simple opaque pages, with wider
PDF color/fonts/transparency and physical presentation unqualified.

PDF constant alpha qualification (2026-10-06): authored ExtGState ca/CA 0.5
red fill renders straight RGBA [255,0,0,128]. Its white composite agrees with
independent Poppler over all 100 pixels ([255,127,127]). Focused decoder test
passes; expanded actual Metal test passes for three page selections and six
native/prepared streamed swap restores, with exact pixel bits and frames.
This covers constant alpha only; masks, blend/isolation/knockout and arbitrary
transparency remain unqualified. Production decoder code is unchanged.

PDF fallible copy allocation (2026-10-06): parser source copy and final RGBA
copy use try_reserve_exact and admit actual Vec capacity before filling.
Allocation failure propagates as typed memory error; final ownership remains
managed. Both PDF decoder tests and expanded three-page RAM/swap/actual Metal
test pass. Hayro internal allocations remain outside this fallible-copy
contract; this does not establish total RSS or arbitrary input qualification.

PDF external-generator strict comparison (2026-10-06): CairoSVG 2.8.2/Cairo
produced an authored PDF containing rectangles, circle, triangle and embedded
text. Ordinary decoder renders 48x36 RGBA8. White RGB composite differs from
independent Poppler in 173/1728 pixels, maximum channel difference 128. Exact
qualification fails and tolerances are not relaxed; edge/font/rendering cause
remains unisolated. Reusable raster_fixture_dump exports native RGBA8 for
independent comparisons. Evidence: research/pdf-cairo-poppler-2026-10-06.json.

PDF Cairo component isolation (2026-10-06): separate Cairo-generated
rectangle agrees exactly with Poppler; circle differs in 51 pixels, triangle
in 63, text in 59. These sum to the combined 173 differences. Causes remain
unisolated; exact comparison stays strict. The external-generator rectangle
additionally passes full-plane Poppler sample comparison, RAM lease, two
native/prepared streamed swap restores with pressure retry, and actual Metal
frame equality. Both PDF GPU tests pass. This qualifies the rectangle fixture
only; it does not qualify font or curved/diagonal rasterization.

PDF antialias diagnostic (2026-10-06): disabling Poppler text/vector
antialiasing does not eliminate differences. Default comparisons differ only
where at least one renderer has partial coverage, but nonwhite bounding boxes
differ by one row; this suggests rasterization differences without proving
font/geometry correctness. scripts/qualify-pdf-poppler.py reproduces strict
full-plane comparison, saves all results and correctly exits 1 for the three
mismatching components (0/51/63/59 pixels for rectangle/circle/triangle/text).
No tolerance was changed. Evidence: research/pdf-poppler-antialias-2026-10-06.json.

PDF malformed-source ownership regression (2026-10-06): a compact invalid
PDF graph returns typed InvalidPdf after managed source allocation and leaves
zero retained credit. A source budget one byte too small refuses before
allocation with zero peak and usage. All three focused PDF tests pass. This
is ownership/error-path evidence for these inputs, not arbitrary malformed
PDF or parser scratch qualification; production behavior is unchanged.

PDF warm load timing (2026-10-06): existing raster_load_bench measures five
selected PDF cases with 3 warmups and 20 samples each; display fingerprints
are stable across repetitions. Large authored Cairo page is verified 1536x1152:
p50 decode 4.542 ms, display preparation 6.060 ms. Small simple pages decode
p50 0.021..0.026 ms; small Cairo text/shapes 0.084 ms. Results exclude decoded
cache, GPU, cold storage and physical presentation. Complex/large page pixel
correctness remains unqualified; timings do not establish correctness. Full
nearest-rank p50/p95 and source hashes: research/pdf-load-timing-2026-10-06.json.

PDF rotation sample (2026-10-06): authored MediaBox 10x20 with Rotate90
renders 20x10 with red left/green right halves. Native RGBA agrees exactly
with mathematical reference and independent Poppler full-plane RGB, and
managed credit is released after drop. All four focused PDF tests pass.
This covers Rotate90 only, not crop boxes/user units/arbitrary transforms
or rotated-page GPU integration. Evidence: research/pdf-rotation-2026-10-06.json.

PDF Rotate90 RAM/swap/Metal proof (2026-10-06): the 20x10 rotated source
now joins the independent Poppler-reference GPU preservation test. RAM lease
protection, two native/prepared bit-exact streamed swap restores, pressure
retry, exact primary-color offscreen frames and final managed release pass
on Metal Apple M4 Max. Both PDF GPU tests pass (none ignored). Physical
presentation and further crop/unit/rotation variants remain unqualified.

PDF geometry corpus and UserUnit correction (2026-10-06): authored
0/90/180/270 rotations and CropBox match Poppler full-plane samples. Initial
UserUnit=2 agreement concealed both renderers ignoring the scale. Decoder now
reads/validates the page UserUnit, scales render dimensions and rendering
transform before managed admission, and refuses nonpositive/nonfinite values.
Unit=2 produces mathematical 20x40 green/red halves, preserved through RAM,
two native/prepared pressure-retry swap restores and Metal reference frames.
Full decoder tests: 646 passed, 35 ignored; both PDF GPU tests pass. Unit
reference is mathematical, not the unadjusted Poppler 72dpi output.
Evidence: research/pdf-geometry-corpus-2026-10-06.json.

PDF invalid UserUnit admission (2026-10-06): zero, negative and name-valued
UserUnit return the explicit invalid-UserUnit error; excessive positive scale
returns page-geometry error. Focused test passes and requires peak credit to
equal only the source copy, with final usage zero, proving no output reservation
occurs for these rejected cases. Production decoder is unchanged.

PDF application page-key/preload proof (2026-10-06): actual two-page PDF
loads page0 visibly and page1 through preload_raster. Subsequent hits use
distinct prepared allocations without decode; metadata/colors remain correct.
Count shrink to one removes the background page while retaining visible page0.
Dropping all owners/cache returns managed usage to zero. Focused app regression
passes; physical page navigation UI is not covered. Evidence:
research/pdf-preload-page-keys-2026-10-06.json.

Indexed XCF correction: the initial v0 file fixtures were invalid as indexed
compatibility evidence because GIMP treats v0 indexed files as grayscale. Valid
v1 replacements now have independent gimpformats native-plane and colormap
checks. The legacy RGBA/alpha expectations remain authored; the Python reader
does not apply those semantics. The original report is explicitly superseded.
Diagnostic RGBA expansion excludes v0 indexed and modern indexed-alpha layouts;
XCF viewer support remains Pending. Source: https://developer.gimp.org/core/standards/xcf/

Valid indexed-v1 fixture decoding is now an automated library regression: both
files preserve header/layer/palette/native planes and authored RGBA. A seven-byte
output child refuses the eight-byte result without consuming source ownership;
an eight-byte child succeeds under the shared root, pre-cancellation has zero
output peak, and last-owner release returns credits to zero. Fixture source bytes
are borrowed and outside this pixel budget. XCF suite: 29 passed,
`/tmp/rrrah-xcf-indexed-budget.log`. Full flattening/display remains Pending.

### XCF selected-layer native loading (2026-10-06)

`decode_xcf_layer` now unifies selected-index admission, image compression
validation, layer properties, hierarchy decode and stored mask decode under a
shared managed budget and cancellation callback. `XcfDecodedLayer` retains
borrowed metadata and native samples, including high-precision/indexed byte
representations; it does not evaluate layer effects, blends or color. The API
rejects out-of-range indices explicitly and releases partial outputs on failure.
Two authored indexed v1 fixtures verify exact native bytes and ownership. The
pinned GIMP 2.6 fixture verifies two selected layers, its 25x251 zero mask, retained
byte accounting and cancellation after allocation with zero retained memory.
Full decoder: 695 passed, 36 ignored; the new external-fixture test separately
passes. Evidence: `research/xcf-selected-layer-2026-10-06.json`. XCF remains Pending
for flattened-image viewer integration and full color/render qualification.

### XCF selected API external exporter/oracle integration (2026-10-06)

The existing xcf_structure_dump now obtains native layer pixels and masks through
`decode_xcf_layer`; stored masks are exported directly instead of decoded again
by a separate channel path. Independent structural diagnostics remain intact.
The pinned Python/GimpFormats group oracle passes all 22 native layer/mask planes
and 48 metadata comparisons on the updated binary; the separate pinned GIMP
fixture checker passes both native mask/channel planes and all six metadata
checks. Every exporter invocation reports zero managed retention at completion.
Reports record source and executable hashes: `research/xcf-selected-api-groups-2026-10-06.json`
and `research/xcf-selected-api-gimp-2026-10-06.json`. This validates native-layer
API integration with external sources; flattening, group blend/color semantics,
viewer routing and physical presentation remain unqualified.

### Legacy XCF normal flatten foundation (2026-10-06)

`flatten_xcf_legacy_normal` now produces managed straight RGBA8 for legacy
v0-v3 8-bit normal-mode layers, visiting the topmost-first table in reverse,
applying stored masks, visibility, opacity and signed offsets through the native
source-over primitive. Work occurs in encoded channel space. ICC is borrowed
and preserved, not applied. Transparent RGB is canonicalized to zero. Groups,
effects, modern composition attributes, non-normal modes and v0 indexed
interpretation are refused before allocating the working canvas. The float
canvas has an explicit 512 MiB cap; output and every layer/mask/scratch allocation
share the supplied root. Failures/cancellation return no partial flattened image.

An authored two-layer raw-tile RGB fixture checks bottom-to-top alpha overlay
and unaffected pixels against independently stated arithmetic. Two authored
indexed v1 fixtures verify binary alpha and final retained ownership. Admission
and post-allocation cancellation tests verify zero retained memory. The pinned
GIMP fixture's non-normal mode is explicitly refused with zero managed peak,
while its native selected-layer/mask API remains usable. Full decoder: 697 passed,
36 ignored, `/tmp/rrrah-xcf-flatten-full.log`; external GIMP refusal/selected-layer
regression separately passes, `/tmp/rrrah-xcf-blend-refusal.log`. This remains a
restricted foundation: no independent GIMP flattened-color oracle, full blend/
group/effect renderer, modern precision flatten or viewer routing is established.

### XCF common raster / swap / Metal integration (2026-10-06)

The supported legacy normal-layer subset now routes through decode_raster and
decode_image, including XCF under misleading CR3 suffixes. Gallery extension
recognition includes XCF. Native ICC is reserved before copying, retained with
shared metadata and applied by existing display preparation; untagged output
remains Unspecified unless the caller explicitly requests an sRGB assumption.
No byte precision is silently substituted for modern precision. Unsupported
composition remains an explicit InvalidXcf error; cancellation/memory failures
retain the common typed source errors.

Authored normal-overlay fixtures verify exact pixels, source/profile ownership,
last-owner release, explicit untagged preparation, insufficient ICC admission
rollback and nonzero-index refusal. The existing ICC raster swap/Metal test adds
a sixth fixture case: profiled XCF. Actual Metal Apple M4 Max output matches the
authored sRGB overlay within its one-byte tolerance, and native/prepared swap
transport, pressure retry and last-owner zero usage pass. Full decoder 698 passed,
36 ignored; viewer 137 passed, 13 ignored. Evidence and exact GPU command:
`research/xcf-raster-route-metal-2026-10-06.json`. Catalog: 87 implemented subsets,
13 Pending; no complete 100-format qualification claim. Full XCF GIMP color/
flatten oracle, modern blend/group/effect handling and physical display remain.

### XCF legacy normal Auto defaults and independent flatten oracle (2026-10-06)

The external xcf-rs `1x1-violet-legacy.xcf` exposed stored Auto defaults that
initially caused a composition refusal: composite mode -1, composite space -2,
blend space 0. The supported normal legacy path now explicitly accepts absent/
Auto/Union modes (None,-1,0,1), absent/Auto/encoded-RGB spaces (None,-2,0,2), and
absent/Auto blend space (None,0), retaining mode 0 and legacy version restrictions.
Unknown modes/spaces and linear/Lab interpretations still refuse before canvas
admission. This mapping follows pinned GIMP mode defaults and enum/load semantics:
https://raw.githubusercontent.com/GNOME/gimp/025352c5745884086d7095969bc5942bc6fbff49/app/operations/layer-modes/gimp-layer-modes.c
and https://raw.githubusercontent.com/GNOME/gimp/025352c5745884086d7095969bc5942bc6fbff49/app/operations/operations-enums.h.
A mutation regression exercises supported explicit values and unsupported/unknown
values, checking exact output or refusal with zero managed peak.

`xcf_flatten_dump` exports encoded output through the common raster route and
checks last-owner release. `qualify-xcf-legacy-flatten.py` verifies all source
hashes and pinned Python dependency/source versions before decoding. Independent
GimpFormats layer parsing plus Pillow alpha composition matches all three
outputs exactly: one external opaque 1x1 source and two authored 2x1 RGBA overlays.
The authored fixtures now carry explicit visible=true properties because the
independent parser treats omitted visibility differently; hashes are updated.
A wrong exporter that changes one output byte is rejected. Positive/negative
reports: `research/xcf-legacy-flatten-python-2026-10-06.json` and
`research/xcf-legacy-flatten-negative-2026-10-06.json`. Scope explicitly excludes
masks, variable opacity, indexed, groups, modern blend modes, ICC transforms and
GIMP-rendered/presented output. This is not complete XCF qualification.

Full decoder: 699 passed, 36 ignored, `/tmp/rrrah-xcf-flatten-auto-final.log`.
Viewer and actual Metal hdr_raster_swap revalidated with the revised fixtures:
`/tmp/rrrah-xcf-auto-viewer-metal.log`. All three integration tests pass, including
the six-fixture ICC test on Metal Apple M4 Max.

### XCF binary masks, disabled masks and negative clipping oracle (2026-10-06)

Three reproducible CC0 legacy fixtures generated by
`make-xcf-normal-mask-fixtures.py` add enabled/disabled stored 0/255 masks and
negative-X layer clipping. Independent GimpFormats layer/mask parsing plus Pillow
alpha composition now matches six encoded outputs exactly (one external opaque
single-pixel source and five authored overlays). The oracle rejects intermediate
mask values so endpoint coverage cannot be mistaken for general mask-rounding
qualification. Source/dependency hashes are checked before native invocation;
a deliberately corrupted-byte exporter fails the expanded gate. Reports:
`research/xcf-legacy-flatten-masks-python-2026-10-06.json` and
`research/xcf-legacy-flatten-masks-negative-2026-10-06.json`.

Managed regression checks all three exact outputs, zero retention after last
owner, and cancellation at mask reservation after canvas/layer allocation,
releasing all partial managed output. Decoder 700 passed, 36 ignored,
`/tmp/rrrah-xcf-mask-flatten-full.log`. A new live Metal integration checks strict
untagged-color refusal, explicit AssumedSrgb preparation, authored reference
readback, native swap byte/color-marker preservation, equal restored readback,
one write/read, empty retained queue and final managed usage zero for all three
fixtures. All four hdr_raster_swap tests pass on Metal Apple M4 Max,
`/tmp/rrrah-xcf-mask-swap-metal.log`. These are SDR offscreen checks; intermediate
mask/opacity rounding, full GIMP rendering, modern/groups/effects and physical
HDR/CUDA/NVIDIA remain unqualified.

### Invisible XCF layers avoid native decoding (2026-10-06)

Legacy flatten now checks the prevalidated layer visibility/opacity before
native layer/mask loading. Hidden or zero-opacity planes do not reserve/decode
pixels; metadata, unsupported composition admission and cancellation remain
checked. The separate selected-layer API still extracts hidden native planes.
A new valid 128x1 hidden masked layer over a 2x1 visible canvas demonstrates
working-memory behavior: explicit hidden extraction refuses a 64-byte working
budget before allocation, while flatten succeeds within that cap and retains
only the final 8 bytes. Source fixture bytes are test-owned outside this working
budget; this does not prove whole-source admission under 64 bytes or total RSS.
Hidden pixel streams are not decoded/validated by flatten; no corrupted-hidden
file compatibility or complete-file validation claim is made.

The fixture generator now emits proper multiple 64-pixel tile pointers for the
wide hidden layer. All eight mask fixture/manifest files regenerate exactly.
Independent pinned GimpFormats/Pillow comparisons match seven flattened images,
including the wide hidden layer; report
`research/xcf-legacy-flatten-hidden-python-2026-10-06.json`. Native decoder:
701 passed, 36 ignored, `/tmp/rrrah-xcf-hidden-budget-corrected.log`. All four
actual Metal raster/swap tests pass, with the masked-XCF test now exercising
four fixtures (including hidden layer), `/tmp/rrrah-xcf-hidden-swap-metal.log`.
This verifies pixel/ownership behavior; no measured decode-time speedup, physical
presentation, intermediate-mask rounding or full XCF qualification is claimed.

### XCF zero-opacity work exclusion (2026-10-06)

The generator adds an explicitly visible 128x1 masked layer with opacity zero
over the 2x1 visible canvas. The managed working-budget regression now checks
both hidden and visible-zero-opacity metadata independently: explicit extraction
of either wide plane refuses the 64-byte budget; flattened output succeeds,
retains eight bytes and releases them on final drop. This exercises both branches
of the earlier work-exclusion change. Source bytes are borrowed outside this
working-budget test; no whole-file/RSS cap or measured timing gain is claimed.

The independent parser/Pillow oracle accepts only zero or unit opacity, extending
its exact comparison to eight images. Report:
`research/xcf-legacy-flatten-zero-opacity-python-2026-10-06.json`. All ten generated
mask fixture/manifest files reproduce exactly. The Metal native/swap/color test
now includes a fifth XCF case, checking strict color refusal, explicit assumption,
reference output, byte/color-marker restoration, readback equality and managed
release. All four raster/swap integration tests pass:
`/tmp/rrrah-xcf-zero-opacity-metal.log`. Decoder full regression: 701 passed,
36 ignored, `/tmp/rrrah-xcf-zero-opacity.log`; the enhanced metadata/budget
assertions additionally pass in `/tmp/rrrah-xcf-zero-opacity-budget.log`. Variable
opacity/intermediate mask rounding and full XCF/GIMP rendering remain unqualified.

### XCF authored half-opacity endpoint-color check (2026-10-06)

An authored PROP_OPACITY=128 fixture with binary mask and endpoint RGB values
adds an exact green-over-blue result of [0,64,191,255], retaining the masked-off
red pixel. The pinned Python decoder exposes integer opacity without scaling,
so the independent comparison normalizes integer PROP_OPACITY by 255 explicitly.
Pillow uses rounded alpha bytes for this one endpoint-color case; this is not a
general oracle for float opacity/intermediate RGB rounding. The scope remains
explicitly restricted. Nine images now match exactly, with dimensions and final
managed ownership checked: `research/xcf-half-opacity-python-2026-10-06.json`.
Native regression: 702 passed, 36 ignored, `/tmp/rrrah-xcf-half-opacity.log`.
The actual Metal masked-XCF native/swap test passes all six fixture cases,
including half-opacity, with strict color refusal, explicit sRGB assumption,
reference/restore readback equality, one write/read and managed release:
`/tmp/rrrah-xcf-half-opacity-metal.log`. General intermediate-color/mask/opacity
rounding, GIMP-rendered output and full XCF qualification remain unproved.

The expanded nine-image opacity oracle was additionally checked with the
deliberately corrupted-byte exporter: it fails as required, with failed pixel
comparisons preserved in `research/xcf-half-opacity-negative-2026-10-06.json`.
All twelve generated mask fixture/manifest files, including half-opacity,
regenerate byte-identically. This verifies fixture reproducibility and verifier
sensitivity; it does not enlarge the declared rounding/render qualification scope.

### XCF hidden-only global compression admission (2026-10-06)

Fixed a validation hole introduced by work exclusion: an all-hidden/zero-opacity
canvas could previously bypass image compression validation because no selected
layer loader ran. Compression validation is now a shared no-allocation helper
used by both native layer loading and flatten admission before working-canvas
allocation. Invalid/duplicate/incorrect-length compression properties retain
explicit refusal regardless of visibility. The regression makes both layers
hidden, verifies valid input produces transparent RGBA, and verifies compression
3/255 returns the specific structure error even under a zero-byte budget, with
zero managed peak. Full decoder: 703 passed, 36 ignored,
`/tmp/rrrah-xcf-global-compression.log`; diff check passes. Hidden pixel payloads
remain deliberately undecoded by flatten; this fix validates global metadata and
does not establish complete-file validation or full XCF qualification.

### Managed compute exposure cooperative cancellation (2026-10-06)

LinearExposureCompute adds `execute_managed_with_cancel` and typed Cancelled
errors, retaining the original API as an uncancelled wrapper. Checks precede
validation, repeat per 4096 input pixels, precede admission/upload/submission,
and reject an obsolete result after readback/unmapping. Submitted work still
completes before this blocking API returns; queue completion retains its GPU
reservation independently. Cancellation does not interrupt native driver waits.
Actual Metal Apple M4 Max regression verifies pre-allocation cancellation with
zero peaks, cancellation after reservations with zero final usage, injected
post-readback cancellation with both budgets released, and subsequent usable
exposure. Existing HDR/alpha/tail and multiple-dispatch checks remain passing.
One GPU integration test passes; two large/full-sensor tests remain ignored.
All 47 GPU library tests pass. Report with source hashes and logs:
`research/compute-cancellation-metal-2026-10-06.json`. This does not establish
viewer integration, measured cancellation latency, physical HDR or CUDA/NVIDIA.

Current EOS R8 CR3 performance was refreshed after staged memory admission.
On Metal Apple M4 Max, warm uncached decode-to-completed-offscreen-frame p50/p95
is 75.449/77.006 ms; resident zoom/pan is 1.511/1.948 ms. Separate rotating
cache-tier measurements give decode 44.106/45.539 ms, persistent disk
33.484/35.177 ms, swap 32.517/32.998 ms and shared RAM lookup
0.000175/0.000188 ms. RAM lookup excludes rendering/upload and warm disk results
exclude cold I/O. Exact restored pixels/metadata and final managed credit zero
were checked. Evidence/raw samples: research/current-raw-performance-2026-10-06.json
and current-{cr3-timing,raw-view,raw-cache}-2026-10-06.csv. Single-file offscreen
evidence does not establish all-camera performance or physical HDR presentation.

Real EOS R8 RAW-view admission negative controls explicitly fail with a typed
Memory(Capacity) error: 1 MiB CPU refuses the 22,382,226-byte source; 1 MiB GPU
refuses the 73,400,320-byte atlas admission. The same fixture succeeds under
512 MiB budgets in the current performance control. These process-level failures
do not independently establish all error-path last-owner cleanup; renderer
resource tests cover their own ownership contracts. Target/driver memory is
outside managed atlas accounting. Evidence:
research/current-raw-view-admission-2026-10-06.json.

The current rebuilt common XCF raster decoder re-passes the pinned independent
GimpFormats/Pillow legacy flatten gate: nine images have exact pixels, dimensions
and zero retained managed credit. A separate byte-corrupting decoder wrapper
fails every pixel comparison and exits nonzero, confirming the gate rejects
incorrect output. Reports: research/xcf-current-legacy-flatten-2026-10-06.json
and research/xcf-current-legacy-negative-2026-10-06.json. The scope still excludes
modern modes/precision, groups, intermediate masks, general float-opacity
rounding, ICC transform and physical GIMP/viewer presentation.

Legacy grayscale and grayscale-alpha XCF now have reproducible CC0 raw v1
fixtures, native exact levels/alpha and last-owner accounting regression, and
independent pinned GimpFormats/Pillow flatten evidence. The expanded gate passes
11 images with exact dimensions/pixels and managed ownership zero:
research/xcf-gray-legacy-flatten-2026-10-06.json. Generator:
scripts/make-xcf-gray-fixtures.py; native log: /tmp/rrrah-xcf-gray-native.log.
This extends legacy color-layout qualification only; modern/groups/effects,
general float-opacity and physical presentation limitations remain.

Both newly qualified legacy grayscale XCF files also pass the actual Metal
raster/swap test, alongside six mask/visibility/opacity cases. Strict untagged
color rejection, explicit sRGB interpretation, exact native/restored bytes,
AssumedSrgb marker retention, identical pre/post-swap Metal readback, zero final
managed credit and one write/read are checked per case. Evidence:
research/xcf-gray-swap-metal-2026-10-06.json. This is eight offscreen SDR cases,
not modern XCF qualification or physical HDR presentation.

Full decoder library regression after grayscale XCF: 704 passed, zero failed,
36 intentionally ignored. The four grayscale source/manifest files regenerate
byte-identically. Evidence: research/xcf-gray-full-decode-2026-10-06.json.
Ignored corpus requirements and all-format qualification remain independent.

Authored legacy v1 indexed/indexed-alpha XCF palette files now pass the native
raster/swap/actual-Metal path, bringing that loop to ten cases. Encoded indices
0/1 expand to RGB (17,43,91)/(239,181,7); indexed alpha128 follows the existing
legacy binary-alpha threshold. The attempted independent GimpFormats comparison
failed because that parser exposes index bytes as L/LA and omits colormap
application. Its mismatch report is preserved as historical evidence, not a
passing indexed qualification; indexed inputs were removed from that oracle's
admitted corpus. Independent indexed GIMP flatten remains open. Reports:
research/xcf-indexed-swap-metal-2026-10-06.json and
research/xcf-indexed-legacy-flatten-2026-10-06.json.

The indexed oracle limitation is narrowed for opaque kind4 v1 only: pinned
GimpFormats independently parses the document colormap and raw indices, and
Pillow expands that palette before normal composition. The expanded gate passes
12 exact images; the corrupt-output negative gate rejects all twelve. This
qualifies one opaque indexed source using explicit oracle glue, not the parser's
default image conversion. Indexed-alpha threshold semantics, v0 interpretation
and independent GIMP presentation remain open. Evidence:
research/xcf-opaque-indexed-oracle-2026-10-06.json and
research/xcf-opaque-indexed-negative-2026-10-06.json.

Legacy indexed alpha boundary127/128 now has an authored fixture and passes
native/swap/Metal exact transport (eleven cases in the loop). Alpha127 produces
transparent output with canonical zero RGB; alpha128 produces opaque palette
color. This matches the historical rule in https://developer.gimp.org/core/standards/xcf/ .
That document explicitly flags uncertainty about newer GIMP behavior; the
fixture is a legacy contract test, not independent indexed-alpha renderer
qualification. No GIMP/ImageMagick renderer was available locally. Evidence:
research/xcf-indexed-boundary-metal-2026-10-06.json.

The legacy XCF independent gate now verifies each authored manifest's expected
encoded RGBA against the independently reconstructed image before calling the
native decoder for that source. All eleven authored expectations agree, alongside
one external source (twelve exact results). A deliberately incorrect manifest
injected only in memory is rejected; workspace manifests stay unchanged.
Fixture construction is now reported accurately instead of labeling every
source as an overlay. Evidence: research/xcf-manifest-oracle-2026-10-06.json.

The complete default GPU test suite re-passes after staged-buffer and cache
ownership changes: 87 passed, zero failed, 3 intentionally ignored.
This includes unit tests and actual Metal offscreen RAW/color, raster/model,
filmstrip, compute, RGBE and resource-submission ownership checks. Ignored cases
are enumerated in research/current-full-gpu-tests-2026-10-06.json; no physical
HDR presentation or CUDA/NVIDIA qualification is implied.

Both default-ignored full-sensor exposure tests were explicitly executed and
pass on current Metal Apple M4 Max. Resident compute on 6188x4120 synthetic
float pixels has five post-warmup samples, median2.400ms; first/middle/last
readbacks agree. Managed execution checks every one of 25,494,560 pixels,
GPU peak1,223,738,992 bytes, CPU peak815,825,920 bytes, clone ownership and
final credit zero. Its single allocation/upload/readback-inclusive sample is
248.171ms, not a latency distribution or demosaic benchmark. Evidence:
research/current-full-sensor-compute-2026-10-06.json. The remaining default
GPU-suite ignored corpus test needs the independent Sony F828 full-sensor dump.

A local SRF and matching-size 3360x2460 u16 dump were rediscovered. Explicit
execution of the full RGBE GPU test on that dump passes 33,062,400 GPU/CPU
channel comparisons within1e-5 and final CPU/GPU credit zero. Dump and source
hashes are recorded in research/current-full-rgbe-compute-2026-10-06.json.
The dump producer provenance was not verified, so this qualifies interpolation
parity on supplied data only; the independent camera-decode/as-shot WB/color
prerequisite remains unproven despite the historical test name.

Rediscovered local Sony F828 SRF also passes two explicitly executed application
tests: missing neighbour does not consume count1 retention quota and the next
valid neighbour stores/restores; thumbnail source/preview share the managed root
with low-budget refusal and higher-budget success. Source hash/logs:
research/current-srf-cache-thumbnail-2026-10-06.json. These close available
resource-flow checks previously excluded for missing corpus, but do not prove
source provenance or independent camera color/RAW qualification.

RGBE cancellation API has actual Metal coverage at entry, post-admission before
allocation, dispatch preparation, pre-submission and completed readback. Every
cancelled call releases owned CPU/GPU credit; the same test subsequently
produces exact output. Full supplied-dump33,062,400 channel parity re-passes
after the cancellation production change. Evidence:
research/rgbe-cancellation-metal-2026-10-06.json. Submitted work/driver waits
remain blocking and viewer integration is separately unproven; camera dump
provenance and as-shot color limitations remain.

All ten currently available default-ignored application tests explicitly
re-pass together with the rediscovered SRF source: RAW/raster-neighbour
prefetch, byte/count/shared pressure, source-kind visible preservation,
RAW RAM/disk preload, idle TTL spill/restore, thumbnail root admission and
actual Metal budget retry without surface redraw. Three unrelated corpus
tests are explicitly excluded and enumerated, not reported passed. Evidence:
research/current-available-app-special-2026-10-06.json. This closes current
resource-flow execution gaps, not all-format, physical HDR or provenance gates.

All thirteen default-ignored application cases have now explicitly executed
and passed without exclusions after rediscovering local Sony SR2, Hasselblad
3FR and AI corpora. This additionally confirms Metal raw-lease retention until
submission completion, real3FR disk/swap recovery after admission refusal and
AI multi-page shared RAM hits/independent keys. Logs and all five input hashes:
research/current-all-app-special-2026-10-06.json. Earlier missing-corpus
exclusions are historical; origin authentication and broad color/format/physical
presentation qualification remain separate.

Three more default-ignored decoder contracts explicitly pass on rediscovered
local sources: Hasselblad3FR embedded precision/zero-neutral rejection, Epson
ERF known black/WB parameters with malformed-WB/truncation rejection, and
external GIMP XCF selected layer/mask partial-cancellation rollback. Hashes/logs:
research/current-external-decode-contracts-2026-10-06.json. These are scoped
contracts, not general camera or full XCF qualification.

Current common RAW qualification re-passes all29 pinned cases under256MiB
managed admission, including both local EOS R8CR3 sources. Full sensor hashes,
geometry/CFA, camera matrix/WB and declared metadata contracts are checked by
the existing independent-LibRaw gate without reference regeneration. Evidence:
research/current-managed-raw-corpus-2026-10-06.json. The rediscovered F828SRF
and u16 dump also exactly match corpus case1351 source and independent sensor
reference hashes, resolving the earlier provenance-identity uncertainty:
research/current-srf-oracle-identity-2026-10-06.json. This does not convert the
authored-gain GPU interpolation test into as-shot color/presentation proof.

Current strict AI/Poppler gate exits1: VectorApple.ai differs in33,054 pixels
(max channel213), one.ai page0 differs in2,790 pixels(max115), page1 is exact.
The RAM-page and actual Metal/swap preservation tests both pass, proving
transport consistency rather than independent renderer correctness. The
VectorApple source now produces comparable output where the historical report
contained no pixel comparison, so the discrepancy is explicit, not silently
qualified. Report: research/current-ai-poppler-2026-10-06.json. PDF-compatible
AI rendering fidelity remains open; PostScript AI is still unsupported.

AI discrepancy localization: VectorApple has17,320 pixels with max-channel
error>10 and984 with error<=2, bounding box[55,13,274,240]; one.ai page0
has2,294>10 and96<=2, bounds[194,189,1186,606]. These are materially larger
than simple rounding; no tolerance was relaxed. Saved native/Poppler images
are in /tmp/rrrah-ai-current-diff. Lexical content survey finds VectorApple
DeviceCMYK, type2/7 shading, soft masks and several blend modes; one.ai has
a soft mask. Presence alone is not causal proof. Diagnostic ledger:
research/ai-current-content-diagnostics-2026-10-06.json. Next isolation requires
minimal color/shading/soft-mask fixtures rather than declaring native/swap
consistency to be independent renderer correctness.

Minimal PDF soft-mask isolation now provides two reproducible CC0 files:
a blue control and DeviceGray luminosity mask with0/0.5/1 vertical bands.
Both white-composited outputs match Poppler exactly; native regression checks
every RGBA pixel and managed release. This excludes that simple mask scenario
as reproduction of the current AI discrepancy; it does not establish that all
soft masks work or identify a causal fix. Generator:
scripts/generate-pdf-soft-mask-fixtures.py. Independent report:
research/pdf-soft-mask-isolation-2026-10-06.json; native log:
/tmp/rrrah-pdf-soft-mask-native.log. More complex color/blend/mask isolation
remains required; the strict AI gate still fails.

Further AI localization: Poppler reports no images or fonts for one.ai; its
first page contains two black ellipses. All2,790 differing pixels lie within
two pixels of an oracle color boundary; none lie in locally uniform interiors.
This narrows that page's discrepancy to contour/rasterization behavior; lexical
SMask presence was not evidence it caused the difference. The strict gate
still fails and no tolerance is relaxed. Pixel-band diagnostic:
research/ai-one-edge-localization-2026-10-06.json. VectorApple remains a separate
complex color/shading/mask discrepancy and cannot inherit this diagnosis.

A reproducible CC0 16x16 black cubic-circle PDF now independently reproduces
contour disagreement without soft masks, color profiles, fonts or images:
47 pixels differ from Poppler, max channel error36. This isolates a renderer
coverage/rasterization discrepancy; it does not prove every AI difference has
the same cause and is not a passing qualification. Generator:
scripts/generate-pdf-curve-fixture.py; source: tests/fixtures/pdf/curve-circle.pdf;
report: research/pdf-curve-circle-poppler-2026-10-06.json.

Numerical cubic-area isolation uses polygon clipping with128 and512
subdivisions per segment; quantized pixel coverage agrees at both resolutions.
For the minimal circle, native alpha differs from area coverage by up to52
levels and Poppler by up to18. Thus this is not evidence of native superiority
over the external oracle. The observed difference requires actual coverage
algorithm investigation, not relaxed acceptance. Diagnostic:
scripts/diagnose-pdf-curve-area.py (uses saved /tmp minimal-fixture outputs),
research/pdf-curve-area-oracle-2026-10-06.json. This area oracle only covers
that authored path and does not replace the strict AI comparison.

Isolated Vello0.0.8 flatten-tolerance probe: lowering tolerance0.25 to0.01
in a temporary standalone dependency reduces maximal numerical-area alpha
error on the minimal cubic circle from52 to2 levels. Production registry and
workspace dependencies remain unchanged. This establishes a causal improvement
for that fixture; it is not broad PDF correctness, strict Poppler parity or a
measured performance tradeoff. Report:
research/pdf-curve-tolerance-probe-2026-10-06.json; probe source/build remain
in /tmp/rrrah-pdf-tolerance-probe and /tmp/rrrah-pdf-tolerance-target.

Controlled tolerance probe completes: one.ai page0, with matching extents,
reduces max Poppler channel error115->60 and differing pixels2790->2551
when tolerance0.25->0.01. Dev render medians154.287->155.524ms; sequential
samples are not a paired performance claim. Strict equality still fails.
VectorApple probe defaults produced different viewport extents from production
ceil dimensions, so its pixel metrics are invalid and explicitly flagged; it
must be rerun with identical explicit viewport settings. Production dependencies
remain unchanged. Report: research/ai-tolerance-comparison-2026-10-06.json.

Corrected VectorApple tolerance probe now matches production ceil viewport
and exact integer alpha transport. Baseline white-composite output equals
production byte-for-byte and reproduces33,054 differing Poppler pixels(max213).
Tighter0.01 tolerance yields33,065 differing pixels(max210): no overall pixel
agreement improvement for this complex document. Thus curve-tolerance improvement
cannot serve as its color/shading/mask fix. One-output comparison, no timing
claim; production unchanged. Report:
research/ai-viewport-tolerance-comparison-2026-10-06.json.

Opaque axis-aligned RGB blend isolation now has four reproducible CC0 PDFs:
Normal/Multiply/Screen/HardLight red-blue overlaps. All four compare exactly
with Poppler at12x8. This rules out those simple RGB blend cases as the
VectorApple reproduction, not CMYK blending, gradients or soft-mask groups.
Generator: scripts/generate-pdf-blend-fixtures.py; independent report:
research/pdf-rgb-blend-poppler-2026-10-06.json. Hayro0.7 fixed DeviceCMYK
profile still has no public interpreter setting for replacement, and prior
shared-profile Poppler refusal remains documented. Strict AI gate stays open.

Minimal axial gradients now isolate another PDF disagreement:12x8 RGB
linear ramp differs from Poppler at96 pixels(max11), while native samples
exactly match authored pixel-center values(x+0.5)/12 and Poppler samples
match edge-position behavior. This is not an admission to relax the strict
Poppler gate or broad correctness proof. CMYK counterpart differs at96
pixels(max45), combining sampling and color-policy effects that still need
separation. Sources/generator: scripts/generate-pdf-gradient-fixtures.py;
report: research/pdf-gradient-poppler-2026-10-06.json.

Center-sample strip isolation separates axial interpolation from CMYK policy:
RGB gradient and authored center strips match natively exactly, and RGB strips
match Poppler exactly. CMYK native gradient also matches native CMYK center
strips exactly, but constant strips differ from Poppler by up to57 levels.
Thus this fixture's large CMYK disagreement persists without gradient sampling
and lies in color-policy/conversion, not its native axial interpolation. This
does not prove all complex AI discrepancies have the same cause. Generator:
scripts/generate-pdf-gradient-strips.py; report:
research/pdf-gradient-center-strips-2026-10-06.json.

### Same-profile independent CMYK diagnostic (2026-10-06)

`scripts/qualify-pdf-cmyk-profile.py` pins the authored 12x8 opaque CMYK
strip PDF and the exact 8464-byte CGATS001Compat-v2-micro ICC profile. Independent
Pillow/LittleCMS transforms with intents 0 through 3 differ from native output
by at most one byte per RGB channel (32 of 96 pixels differ for each intent).
Versions, source/profile/output hashes and input samples are saved in
`research/pdf-cmyk-littlecms-2026-10-06.json`. The independent input is quantized
CMYK8. A later source inspection (2026-10-07) established that Hayro DeviceCMYK
also quantizes its parsed float operands to CMYK8 before ICC conversion; thus
this comparison matches that input precision and is not a direct float-ICC oracle.
The earlier same-strip Poppler maximum difference of 57 therefore does not by
itself establish a broken native ICC transform. Color policy remains a separate
qualification question. The strict AI Poppler gate remains failed; this fixture
does not qualify arbitrary colors, masks, blend groups or rendering intents.

### Decoder and independent encrypted SRF regression (2026-10-06)

The current locked decoder library run passes 705 tests with 36 external tests
ignored. The ignored `real_full_encrypted_sensor_matches_independent_oracle`
was then explicitly executed against the pinned Sony F828 source and independent
LibRaw unpack buffer and passes. Its assertions cover full sensor equality,
RGBE phase/black levels, independent 3x4 color matrix, as-shot WB and malformed
metadata rejection. The separate execution does not imply the other 35 ignored
cases have passed in this run. Logs, hashes and counts are recorded in
`research/current-decode-and-srf-regression-2026-10-06.json`.

### CRW/CIFF directory foundation (2026-10-06)

The private `ciff` module now reads bounded borrowing directories in both byte
orders, inline records and nested directory payloads. Synthetic tests exercise
all fixture prefixes and malicious header/table/payload offsets and counts.
Two focused tests and the full decoder suite (707 passed, 36 ignored) pass.
No camera-produced CRW source, sensor entropy or color has been qualified;
CRW remains Pending and the 87/13 catalog counts are unchanged. Reference
URLs, source hashes and log hashes are recorded in
`research/ciff-directory-foundation-2026-10-06.json`.

### Camera-produced D30 CIFF verification (2026-10-06)

CC0 raw.pixls.us object 1307 was downloaded and its advertised SHA256 verified.
The explicitly executed ignored CIFF test traverses seven directories and 39
records, locates 2,824,648 compressed sensor bytes and decoder-table index 1.
Image geometry 2160x1440, sensor geometry 2224x1456 and Canon EOS D30 identity
match independent ExifTool metadata. Source provenance, tool version, fields
and test evidence are pinned in `tests/fixtures/crw-d30-investigation.json`.
Entropy and color decoding remain unimplemented; CRW stays Pending.

### Independent CRW sensor references (2026-10-06)

The existing LibRaw fixture oracle was rebuilt from current source and used to
unpack CC0 objects 1307 (D30) and 2027 (10D), both verified against repository
source hashes. Full sensor lengths/hashes and color metadata are pinned in
`tests/fixtures/crw-sensor-oracles.json`. D30 has inadmissible negative red WB;
10D reports positive gains [1742,832,1241,832], so it is the next candidate for
full native entropy/color qualification. LibRaw crop and CIFF ImageInfo dimensions
are distinct, and must not be conflated. No native sensor equality is claimed.

### Bounded CRW entropy bit reader (2026-10-06)

The borrowing MSB-first CRW bit reader enforces FF00 byte stuffing, rejects
invalid widths and truncated streams, and decodes signed JPEG-style amplitudes.
Exhaustive tests cover all 131,071 amplitudes for widths zero through 16,
cross-byte reads and all 255 invalid FF followers. Current full decoder tests
pass 709 with 37 external cases ignored (including the separately executed D30
CIFF test). Camera Huffman tables, predictors, low-bit merging and native full
sensor equality remain pending. Report: `research/crw-entropy-bits-2026-10-06.json`.

### CRW canonical Huffman foundation (2026-10-06)

The entropy module now builds bounded canonical prefix indexing from sixteen
length counts and borrowed symbols. It rejects empty/oversubscribed trees and
incorrect symbol counts, with explicit invalid-code/truncated-stream errors.
Authored mapping and sixteen-bit depth regressions pass; full decoder run is
711 passed, 37 ignored. Camera tables and sensor reconstruction remain pending;
CRW status is unchanged. Report: `research/crw-huffman-foundation-2026-10-06.json`.

### CRW block and predictor foundation (2026-10-06)

The entropy module reconstructs 64-difference blocks with first/rest trees, EOB,
run bounds and DC carry. Alternating ten-bit predictors reset at row boundaries;
invalid samples and arithmetic overflow do not advance predictor state. Authored
block and refusal tests pass; current full decoder run: 713 passed, 37 ignored.
Camera table hookup, low-bit merging and full sensor comparison remain pending.
Report: `research/crw-block-predictor-2026-10-06.json`.

### CRW low-plane layout and bounded merge (2026-10-06)

The low-two-bit merge checks geometry, length and all high-bit samples before
mutating. Synthetic tests cover all 4096 combined values and the 2672-width
correction boundary. Every low bit of the 6,518,336-sample EOS 10D reference
matches the CRW plane at offset 26. High bits in this test are oracle-derived,
so this does not prove native entropy reconstruction. The external test passes;
default decoder suite: 714 passed, 38 ignored. Report:
`research/crw-low-plane-2026-10-06.json`. CRW remains Pending.

### Complete native CRW sensor equality (2026-10-06)

The native bit reader, canonical Huffman decoder, block/predictor reconstruction
and low-plane merge reproduce every full sensor sample for CC0 EOS 10D object
2027 and D30 object 1307 against pinned LibRaw unpack buffers. Tables 0/1 are
external numerical format data supplied only to tests; no LibRaw implementation
is linked or embedded. The ignored full-sensor test was explicitly executed for
both cases. Default suite: 715 passed, 39 ignored. Production managed admission,
validated source spans, shipped table provisioning, metadata/color, and viewer
integration remain pending; CRW stays Pending. Report:
`research/crw-full-sensor-2026-10-06.json`.

### Managed CRW reconstruction (2026-10-06)

CRW reconstruction now reserves output memory before allocation and returns a
shared managed pixel buffer. Cancellation at block/low-plane row boundaries,
malformed entropy and insufficient memory retain no output reservation. Full
10D sensor equality passes with an exact 13,036,672-byte output budget; clones
retain that credit until last-owner release. Input/table ownership is external.
Current decoder tests: 716 passed, 39 ignored. Public route and color remain
pending. Report: `research/crw-managed-reconstruction-2026-10-06.json`.

### CIFF-derived bounded sensor decode (2026-10-06)

Full sensor tests now obtain geometry from unique CIFF 0x1031 and confine entropy
and low-bit reads to unique 0x2005 payload, excluding previews and metadata.
Managed/unmanaged reconstruction still matches all D30 and 10D oracle samples.
Bounded unique tree lookup rejects duplicates and supports cancellation; depth
16 and 4096-record caps are implemented but cap-boundary fixtures remain open.
Default suite: 717 passed, 39 ignored. External table provisioning and public
metadata/color/viewer integration remain pending. Report:
`research/crw-bounded-container-2026-10-06.json`.

### Native 10D Auto-WB (2026-10-06)

CIFF 0x10a9 ColorBalance and 0x102a ShotInfo now supply 10D Auto-WB coefficients
[1742,832,1241,832] in RGBG order, matching pinned independent LibRaw metadata.
The private resolver rejects zero gains, malformed lengths and unqualified
nonzero preset modes. The real-source test was explicitly executed successfully;
default suite: 718 passed, 40 ignored. Color calibration, table provisioning
and viewer integration remain pending. Report:
`research/crw-10d-native-wb-2026-10-06.json`.

### CIFF traversal cap boundaries (2026-10-06)

Synthetic container tests now verify 4096 records and 16 nesting levels are
admitted, while the next record/level returns the corresponding resource-cap
error. This supersedes the earlier untested-cap limitation of bounded CIFF
lookup. Current decoder run: 719 passed, 40 ignored. CRW public route remains
pending. Report: `research/ciff-cap-boundaries-2026-10-06.json`.

### CIFF-to-managed-sensor entrypoint (2026-10-06)

The private sensor entrypoint validates qualified header, geometry and declared
decoder table, then reconstructs only the sensor span into managed storage.
Mismatched table IDs reject before output admission; D30 and 10D full sensor
equality and last-owner accounting pass. Trees/low-plane policy remain explicit
caller inputs; shipped automatic selection and color/viewer admission remain
pending. Default suite: 719 passed, 40 ignored. Report:
`research/crw-container-managed-entry-2026-10-06.json`.

### CRW entropy admission preflight (2026-10-06)

Managed reconstruction now rejects entropy spans shorter than the unavoidable
two-symbol-per-block lower bound before output memory reservation. Tiny-source
fixtures with huge declared geometry retain zero budget peak; the four-block
one-byte exact boundary succeeds. Full 10D equality still passes. Default
suite: 720 passed, 40 ignored. This is a necessary length bound, not validation
of all entropy. Report: `research/crw-entropy-preflight-2026-10-06.json`.

### CIFF/CRW static analysis (2026-10-06)

Clippy found conversion/control-flow/chunking diagnostics in the new entropy
module. These were corrected; the final library Clippy run has no diagnostics
located in `ciff.rs` or `crw_entropy.rs`. Other existing decoder warnings remain.
Default decoder regression passes 720 tests, 40 ignored. External sensor tests
were not repeated for this lint-only cleanup. Report:
`research/crw-static-analysis-2026-10-06.json`.

### Pinned 10D CRW color calibration (2026-10-06)

A private CRW 10D calibration uses the independent LibRaw 0.22.2 coefficients.
The core camera-to-linear-sRGB transform matches all nine independent RGB matrix
coefficients within 1e-6. The older general catalog profile differs and remains
unchanged; differing provenance does not establish invalidity. This calibration
is not yet a public color route. Default tests: 721 passed, 40 ignored. Report:
`research/crw-10d-calibration-2026-10-06.json`.

### Native 10D optical-black calibration (2026-10-06)

Per-phase optical-black means are now computed from native full sensor pixels
in the qualified masked region (rows 12..2068, columns 18..62). Result RGGB
[127,126,127,126] equals independent LibRaw absolute RGBG [127,126,126,127]
after phase mapping. Full native sensor/black test and cancellation check pass.
This does not qualify other camera masks or public color/viewer support.
Report: `research/crw-10d-optical-black-2026-10-06.json`.

### Internal calibrated 10D domain mosaic (2026-10-06)

Native sensor, Auto-WB, calibrated matrix, optical black, pinned crop/white
point and normal orientation now assemble into a validated `DecodedMosaic`.
Real 10D pixels and metadata assertions pass without copying the managed sensor
allocation. Identity, geometry, preset WB and orientation remain strict subset
checks. Default suite: 721 passed, 40 ignored. This remains an internal path with
external trees, not public viewer support. Report:
`research/crw-10d-domain-mosaic-2026-10-06.json`.

### Managed 10D CRW thumbnail development (2026-10-06)

The real 10D domain mosaic now passes the existing managed RAW thumbnail API:
128-edge dimensions follow the qualified crop, alpha is opaque, color samples
are non-gray, exact preview budget ownership persists until release, second
admission under pressure refuses, and cancellation retains no preview credit.
This is pipeline/ownership proof, not independent thumbnail color or GPU/window
qualification. Report: `research/crw-10d-managed-thumbnail-2026-10-06.json`.

### Complete internal 10D decode with early color admission (2026-10-06)

The internal `decode_10d` entry validates camera/color metadata before sensor
admission, then returns the complete calibrated managed mosaic. A zero-WB
source mutation refuses with zero output-budget peak. Full 10D pixels retain
independent LibRaw equality and final budget release. Default suite: 721 passed,
40 ignored. External trees and public viewer/GPU route remain pending. Report:
`research/crw-10d-complete-internal-2026-10-06.json`.

### Independently qualified 10D WB mode selection (2026-10-06)

Ten source variants modify only the CIFF shot WB mode and were processed by
LibRaw 0.22.2; every full sensor hash stays identical. Native mode selection now
matches positive reference coefficients for modes 0,1,2,3,5,6,8,9. Modes 4/7
return zero independent coefficients and remain explicit refusals. The external
coefficient test and default decoder suite pass. This supersedes the Auto-only
resolver limitation, but is not a corpus of ten photographed conditions or
public CRW viewing. Report: `research/crw-10d-wb-modes-2026-10-06.json`.

### Native 10D orientation (2026-10-06)

CIFF angles 0/90/180/270 are now mapped to native orientation. Independently
processed source variants produce LibRaw flips 0/6/3/5 with unchanged full
sensor hashes. Native metadata and crop-adjusted display dimensions agree;
real-source and default decoder tests pass. The oracle tool now also emits
`orientation_flip`. This supersedes normal-only metadata admission, but does
not prove rotated GPU/display pixels. Public CRW route remains pending. Report:
`research/crw-10d-orientation-2026-10-06.json`.

### Reproducible pinned CRW internal acceptance (2026-10-06)

`scripts/qualify-crw-internal.py` verifies all source/sensor/table lengths and
hashes from the fixture manifest before native execution. Both required
D30/10D tests pass on current code, with command/log hashes recorded in
`research/current-crw-internal-2026-10-06.json`. A one-byte external-table
mutation is rejected before any native execution or report publication.
This remains internal sensor/domain proof with external trees, not public
format, GPU or physical display qualification.

### Explicit budgeted CRW import API (2026-10-06)

`decode_crw_10d_with_tables` now exposes the qualified 10D path with explicit
external table-zero data and required request RAM budget. Source and sensor
share the root; real-source peak equals source+output, returned usage equals
output, and final release returns to zero. Full independent sensor equality
passes. The API validates table shape; caller must verify identity/provenance.
Automatic routing and shipped trees remain absent; CRW stays Pending. Report:
`research/crw-explicit-import-api-2026-10-06.json`.

### Explicit CRW import, swap and Metal preservation (2026-10-06)

The ignored `crw_swap_metal` integration test was explicitly executed on Metal
Apple M4 Max against pinned EOS 10D and table-zero data. Full sensor/metadata
and the 128x96 GPU frame survive swap exactly; managed RAM reaches zero after
spill and final restoration release. Root-pressure refusal leaves the stored
object retryable. This is native result preservation, not an independent color
or physical-display oracle. Automatic CRW viewer routing is still absent.
Report: `research/crw-swap-metal-2026-10-06.json`.

### CRW lease TTL/size/count to swap and Metal (2026-10-06)

The real 10D import now exercises a one-entry byte-limited lease cache before
swap. Protected replacement and tighter byte/count limits refuse. Expiry blocks
new lookups, but cloned leases retain the sensor and unchanged Metal render.
Last lease release yields the owned victim; completed swap releases RAM and
restoration preserves all pixels, metadata and frame after pressure refusal.
Executed on Metal Apple M4 Max. This is explicit composition, not automatic
CRW navigation/preload. Report: `research/crw-ttl-swap-metal-2026-10-06.json`.

### CRW API table identity and admission (2026-10-06)

The explicit import API now enforces pinned table-zero BLAKE3 before source
I/O, superseding the earlier caller-only identity requirement. Admission tests
use a nonexistent path to prove table/index/root refusals and cancellation take
precedence over I/O with zero root peak. Both pinned D30/10D internal cases pass
again; default suite: 722 passed, 40 ignored. External table provisioning and
automatic viewer routing remain pending. Report:
`research/crw-api-table-identity-2026-10-06.json`.

### Unified pinned CRW sensor and Metal acceptance (2026-10-06)

The reproducible CRW qualifier now has `--metal`, which requires the explicit
CRW import/cache/TTL/swap/actual-Metal test in addition to both pinned full
sensor cases. The current combined run passes two sensor cases plus the
Apple M4 Max integration. Test success requires one executed test with zero
failures/ignores and a Metal adapter in the integration log. Commands and log
hashes: `research/current-crw-internal-metal-2026-10-06.json`. External trees,
automatic viewer routing, independent rendered color and physical HDR remain
open; this is not all-100 qualification.

### Four CRW orientations survive swap and Metal (2026-10-06)

The actual-Metal CRW integration now renders Normal/Rotate90/Rotate180/Rotate270
variants before and after bounded swap, comparing complete frame bytes, sensor
pixels and metadata. Five writes/reads complete; shared source credit stays
owned until last release and final managed RAM is zero. The combined pinned
D30/10D sensor plus Metal gate passes. This verifies transport/presentation
preservation, not independently correct rotated pixels or color. Current logs:
`research/current-crw-internal-metal-2026-10-06.json`.

### Bundled CRW table-zero and standalone library import (2026-10-06)

The qualified 209-byte table-zero data is now bundled under the upstream
CDDL-1.0 option with license, attribution and hexadecimal source representation
in `crates/rrrah-decode/data/crw`. Bytes match both pinned LibRaw 0.22.2 arrays
and the qualified table hash. `decode_crw_10d` uses them directly; absent a caller
budget it creates a bounded 256 MiB root. Real 10D import and actual Metal
cache/TTL/swap integration now use this bundled API, so they need no external
table file. The D30 exploratory sensor test still uses external table-one.
Combined sensor/Metal gate and 722 default decoder tests pass (40 ignored).
Automatic viewer routing and broader CRW qualification remain pending.

### CRW native routing (2026-10-06)

NativeRawDecoder now dispatches CRW to the qualified EOS 10D bundled-table
backend (recipe backend ID 23, revision 1). CIFF signature detection routes
unknown extensions to sensor handling; CRW extensions are gallery candidates.
Unqualified cameras/layouts still refuse. The real pinned 10D route returns
every sensor pixel and metadata field identical to explicit import. Logs:
/tmp/rrrah-crw-route-real.log and /tmp/rrrah-crw-route-full.log.
This establishes public decoder routing, not live navigation/prefetch or full
CRW camera coverage. Timing is coarse total import time, not separate I/O
and entropy phases. The overall 100-format/device objective remains open.

### CRW routed worker/cache/Metal verification (2026-10-06)

The actual-Metal CRW lease/TTL/size/count/swap regression now imports through
NativeRawDecoder and checks sensor classification and recipe identity. It passes
on Metal Apple M4 Max. A separately executed ignored gallery test sends a real
EOS 10D neighbour from selected PNG to RawPrefetcher, verifies one successful
disk write, exact restored sensor/metadata, and final managed RAM zero.
Evidence: research/crw-router-prefetch-metal-2026-10-06.json. This is worker and
offscreen integration, not physical UI navigation or broad CRW qualification.

### CRW routed admission under competing ownership (2026-10-06)

A separately executed real EOS 10D regression sets the root exactly to source
bytes plus 13,036,672 output bytes. A competing one-byte owner causes output
admission refusal, releasing source credit while preserving that owner. After
its release the same request succeeds with exact source-plus-output peak.
Cloned mosaic ownership retains output credit until the last drop, then RAM
usage is zero. A root one byte below source length refuses with zero peak.
Log: /tmp/rrrah-crw-route-pressure.log (one pass). This verifies managed
allocation accounting, not total RSS or a navigation latency bound.

### CRW public sensor oracle (2026-10-06)

The ordinary raw_fixture_dump entrypoint uses NativeRawDecoder with a 64 MiB
managed root. Its complete EOS 10D output (6,518,336 samples) is byte-exact to
the pinned independent LibRaw 0.22.2 sensor dump, SHA256
3aecb01756492569734885710dafd6ffc26421ac0b5a7fa1c151973d48cd389a.
Evidence: research/crw-public-sensor-oracle-2026-10-06.json. This upgrades the
public route's sensor evidence from native self-comparison to independent
comparison; no broader-camera, physical-display or latency claim is implied.

### CRW producer timing phases (2026-10-06)

The producer now reports table validation/selection, managed source read, and
combined sensor/metadata decode separately. raw_decode equals decoder_select
plus raw_image and excludes source_open. total includes all producer phases;
router sniffing is outside these backend timings, so end-to-end navigation
measurements still require an external wall clock. The explicit APIs share
the same implementation. Four CRW tests, including both real-source route
and exact-budget pressure cases, pass with explicit ignored-test execution.
The route test also checks the timing sum contract.
Log: /tmp/rrrah-crw-timing-contract.log. This supersedes the coarse-timing note
in the initial routing entry; no performance improvement is claimed.

### CRW warm-source import timing (2026-10-06)

The new raw_import_timing example measures native router wall time before
verification hashing, checking full sample digest/metadata and last-owner RAM
release on each iteration. With one warmup and 20 EOS 10D imports, dev optimized
with debuginfo, warm OS file cache: wall p50 129.149 ms / p95 138.953 ms;
source read p50 0.541 ms; sensor/metadata decode p50 128.560 ms. Managed
peak is 20,824,002 bytes. It excludes render/upload/display and does not
establish cold-storage performance. Evidence: research/crw-import-timing-2026-10-06.json.

### CRW chunked-bit experiment (2026-10-06)

Reading amplitude bits in byte-sized groups passed ten entropy tests and both
full independent D30/10D sensor cases, but did not establish a speed benefit.
Sequential warm import p50 was 132.488 ms versus baseline 129.149 ms; this
is not an interleaved causal comparison. The algorithm change was reverted.
Evidence: research/crw-chunked-bits-oracle-2026-10-06.json and
research/crw-chunked-bits-timing-rejected-2026-10-06.json. Production retains
the previously qualified bit reader.

### Catalog after public CRW integration (2026-10-06)

CRW row 44 now describes the implemented EOS 10D subset and its source/worker/
cache/Metal evidence rather than Pending. The 100-row catalog has 88 implemented
subsets and 12 Pending viewer families, with zero universally qualified rows.
The prior format-matrix audit remains historical; current consistency evidence
is research/current-format-matrix-audit-2026-10-06.json. Application regression
after routing/timing changes passes 138 tests with 14 ignored external-fixture
cases (the new CRW worker case was executed separately).
Log: /tmp/rrrah-post-crw-app.log. This does not complete the goal.

### MEF linked-directory byte investigation (2026-10-06)

Pinned ZD bytes show sensor SubIFD 116920 next=29704 and preview SubIFD
117164 next=29912. Interpreting those targets as classic directories produces
large counts and repeated near-4095 words as tags/types, rather than ordinary
TIFF entries. This suggests camera-specific data/link semantics but does not
establish them. Type 4084 must not be added as a generic TIFF field merely
to bypass this error. Recorded original entries/target words and source hash:
research/mef-linked-ifd-byte-audit-2026-10-06.json. Private sensor qualification
and invalid zero WB remain unchanged; MEF public color/viewer support pending.

### Independent MEF metadata audit (2026-10-06)

ExifTool 13.59 verbose parsing independently reports "Ignored bad IFD linked
from SubIFD" on the pinned ZD file. MakerNote directory at offset 800 contains
zero-filled integer tags 0x82/83/84/88 and entire zero-filled 2048-byte tags
0x85/86/87; checking complete payloads confirms their displayed leading zeros
are not hiding positive gains later in those arrays. These fields cannot
establish valid native WB. Recorded entry offsets/types/counts and payload
hashes: research/mef-independent-metadata-audit-2026-10-06.json. The prior LUT
interpretation remains inference, and public MEF support remains Pending.

### Reproducible public CRW gate (2026-10-06)

qualify-crw-internal.py now also requires all four public CRW tests, including
real-route timing identity and competing-owner admission/retry. With --metal
it checks the routed cache/TTL/swap readback on an actual Metal adapter.
Pinned D30/10D independent full-sensor tests, public-route suite and Metal
all pass in research/current-crw-public-metal-gate-2026-10-06.json. Stale
claims that bundled tables/public routing are absent were corrected in newly
generated reports; historical experiment reports remain unchanged.
The cached raw.pixls.us repository contains only the already investigated
Mamiya ZD object 562, so another MEF requires a different source collection.

### MEF alternate source deduplication (2026-10-06)

Downloaded the original rawsamples.ch Mamiya ZD file linked by its Mamiya page.
Its 36,575,308 bytes and SHA256 exactly match raw.pixls.us object 562, so
it is the same problematic fixture and cannot count as a second camera source
or new WB qualification. Evidence: research/mef-alternate-source-dedup-2026-10-06.json.
No production parser relaxation was made.

### PICT bounded framing foundation (2026-10-06)

Private pict module parses the ten-byte Picture record framing with explicit
resource/scrap versus file-data-fork representation. Based on Apple Inside
Macintosh Pictures pp. 7-5/7-7, a file's arbitrary 512-byte application header
is skipped only when explicitly selected; signed origin/dimension differences
are checked and pixel limits precede any image allocation. picSize is retained
without treating it as a valid version-2 stream length. Two tests cover all
short file/record prefixes, negative origins, empty/inverted rectangles and
full signed-coordinate extent with exact pixel-budget boundary.
Log: /tmp/rrrah-pict-header.log. No opcode rendering/public PICT route is
implemented, so PICT stays Pending. Primary specification:
https://developer.apple.com/library/archive/documentation/mac/pdf/Imaging_With_QuickDraw/Pictures.pdf

### PICT version-prefix foundation (2026-10-06)

Based on Apple Appendix A p. A-3, the private parser admits exact version-1
11/01 framing and version-2 0011/02ff followed by mandatory 0c00 and a
complete 24-byte HeaderOp payload. It distinguishes -1/-2 header revisions
without allocation and refuses unknown prefixes/header revisions. Tests cover
every truncated version-2 prefix and missing header; all three PICT tests pass
in /tmp/rrrah-pict-version.log. Header resolution/payload semantics, commands,
rendering and public route remain unimplemented; PICT stays Pending. Source:
https://developer.apple.com/library/archive/documentation/mac/pdf/Imaging_With_QuickDraw/Appendix_A.pdf

### PICT bounded borrowed command stream (2026-10-06)

Private v2 walker borrows operands and recognizes NOP, RGB foreground/background,
pen size/mode/pattern, fill pattern, origin, rectangle/repeated-rectangle
operations and end marker. Unknown operations refuse; byte ranges are checked
before visiting, command count includes end marker, trailing data refuses,
and cancellation is observed before every command. This is parsing only, not
semantics/rendering. Tests cover complete color+rectangle stream, every
truncation, count bound, unsupported bitmap opcode, trailing data, cancellation
and visitor refusal. All five PICT tests pass in /tmp/rrrah-pict-commands.log.
PICT stays Pending; clipping/bitmap/text and renderer integration remain.

### PICT managed rectangle-rendering prototype (2026-10-06)

Private renderer preflights the entire borrowed command stream before allocating
a managed RGBA buffer. It implements opaque RGB8-equivalent foreground colors
and paintRect/paintSameRect on a white canvas, clipping to signed picFrame
bounds. Other admitted parser operations refuse at renderer admission rather
than being silently omitted. Per-command/row cancellation releases partial
output. Synthetic exact red/white pixels, byte budget refusal, shared last-owner
credit, unsupported-op zero-peak admission and early cancellation pass; all
six PICT tests pass in /tmp/rrrah-pict-render-tested.log. Header payload semantics
and independent real-producer/rendering evidence are still unqualified, so
this private prototype does not enable public viewing or change Pending status.

### PICT v2 fixed-box admission (2026-10-06)

The private renderer now validates HeaderOp's full LONG -1 marker, zero reserved
long and fixed-point left/top/right/bottom exactly mapping to picFrame before
output allocation. Other mappings refuse until scaling semantics are implemented.
The renderer fixture now has a valid matching header rather than arbitrary
header filler; a nonzero reserved field refuses with zero RAM peak. Six PICT
tests pass in /tmp/rrrah-pict-mapping.log. Apple Appendix A Listing A-6
provides the layout. Extended-v2, public route and independent rendering remain
pending; this is a restrictive admission contract, not full PICT support.

### PICT rectangular clipping (2026-10-06)

The private v2 command walker admits Clip only for its exact ten-byte rectangular
region representation; complex regions refuse before allocation. Renderer
coordinates clip in signed picFrame space, intersects with canvas bounds and
applies each replacement clip to subsequent paintRect/paintSameRect commands.
A synthetic test verifies excluded pixels, included pixels, changing the clip
and repeating the prior rectangle, final managed owner release, and complex
region refusal with zero allocation peak. All seven PICT tests pass in
/tmp/rrrah-pict-clip.log. Public viewing and independent PICT producer/rendering
qualification remain pending; no catalog status change.

### PICT extended-v2 72dpi mapping (2026-10-06)

Private renderer admits extended-v2 HeaderOp only with -2 revision, zero
reserved fields, 72dpi horizontal/vertical fixed-point resolution and an exact
srcRect-to-picFrame mapping. This enables equivalent version-2/extended-v2
rectangle/clip images without inventing scaling. A synthetic comparison checks
whole RGBA equality and owner release; 144dpi refuses with zero peak. A minimal
version-1 input refuses before attempting to read a v2 payload, avoiding a
short-input slice panic. All eight PICT tests pass in
/tmp/rrrah-pict-extended-final.log. Independent rendering/public viewer
qualification, other resolutions and wider commands remain pending.

### PICT cancellation with allocated output (2026-10-06)

Renderer adds a final cancellation checkpoint before freezing/publishing its
output. A 1000-row synthetic fill cancels after allocation and several row
checks under a root exactly covering 12,000 output bytes plus a competing
one-byte owner. Failure releases only its output; competitor credit remains,
and retry under the same budget produces every expected row before final owner
release to zero. All nine PICT tests pass in /tmp/rrrah-pict-cancel.log.
This proves cooperative managed-output rollback, not a wall-clock cancellation
latency bound, live navigation or public PICT qualification.

### Authored PICT file and system color mismatch (2026-10-06)

Added CC0 authored file-data-fork fixture tests/fixtures/pict/red-rectangle-v2.pict
with exact expected RGBA and SHA256 manifest. Native managed renderer matches
all expected bytes; ten PICT tests pass in /tmp/rrrah-pict-file-fixture.log.
macOS sips successfully converts the same file to PNG, but its two rectangle
pixels are black instead of RGBFgCol red; geometry/background agree. Extracted
PNG pixels through FFmpeg and recorded the failed color comparison in
research/pict-sips-color-mismatch-2026-10-06.json. Whether this is system basic
QuickDraw interpretation or a fixture/semantic issue remains unresolved; no
independent native color qualification or public-route admission is claimed.

### PICT system color probe matrix (2026-10-06)

Six authored v2 variants change only RGBFgCol red/green/blue/white/black, or
replace it with legacy FgColor red. sips renders the same rectangle black in
all six cases; only the black case agrees. This rules out using this path as
a color-sensitive oracle for these fixtures. Probe opcode offset, source
hashes and exact expected/actual RGBA are in
research/pict-sips-color-probes-2026-10-06.json. It does not establish why
color state is ignored or independently qualify native color semantics. A
separate color-capable PICT decoder is needed; PICT remains Pending.

### Independent PICT color decoder probe (2026-10-06)

TwelveMonkeys imageio-pict 3.13.0 successfully decodes the authored v2 file
using its independent Java reader. All RGB channels agree with native output,
including the two red rectangle pixels; four untouched-background alpha bytes
are zero in Java versus 255 in the native white-canvas prototype. Thus full
RGBA does not agree, and background/canvas semantics still need an explicit
contract. Source/jar hashes and all pixels: research/pict-twelvemonkeys-oracle-2026-10-06.json.
An authored adapter is saved in scripts/PictOracle.java (class
PictOracle). This establishes an
external color-sensitive reader for further qualification, not full PICT support.

### PICT transparent canvas contract (2026-10-06)

Private rendering now preserves untouched pixels with alpha zero; hidden RGB
is white for deterministic equality, and each painted pixel becomes opaque.
Viewing background remains a separate composition choice. Initialization
checks cancellation each 4096 pixels. The file fixture's full RGBA matches
TwelveMonkeys 3.13.0 output; all ten tests, including active-fill cancellation
and last-owner accounting, pass in /tmp/rrrah-pict-transparent-final.log.
Evidence: research/pict-transparent-oracle-2026-10-06.json. Earlier opaque
canvas mismatch reports remain historical. This single authored fixture does
not qualify all PICT commands/producers; public route/cache/GPU remain pending.

### PICT public raster adapter (2026-10-06)

PICT/PCT extension candidates now enter decode_raster's native file-data-fork
adapter. The rectangle/rectangular-clip subset preserves managed RGBA and
explicit Unspecified color; no untagged sRGB assumption is introduced.
Public adapter checks cancellation/image index and does not fall back from
unsupported operations. Fixture regression verifies exact red/transparent
pixels, source-buffer release and final output release to zero. All eleven
PICT tests pass in /tmp/rrrah-pict-public.log. Content sniffing under RAW/unknown
suffixes, display color policy, typed restore-pressure integration and
viewer/cache/swap/Metal remain pending; catalog status unchanged until those
integration gates are qualified. Renderer currently wraps allocation refusal
as InvalidPict text; preserving typed Memory is a required next correction.

### PICT typed admission and cancellation (2026-10-06)

Renderer now returns RasterDecodeError with Source(Memory) for buffer refusal,
Source(Cancelled) for direct and command-walker cancellation, and
Source(DimensionOverflow) for arithmetic overflow. Invalid/unsupported format
data remains InvalidPict. Public adapter preserves those errors. A root exactly
covering source plus output with a competing one-byte owner produces typed
output refusal and releases source credit; after owner release, the same
request succeeds and final RAM becomes zero. Twelve PICT tests pass in
/tmp/rrrah-pict-typed.log. This corrects the earlier allocation-as-text limitation;
viewer color policy, swap/cache/GPU and wider format qualification remain open.

### PICT explicit display color policy (2026-10-06)

Common decode_image correctly selects the PICT extension as raster. Default
Unspecified color refuses display preparation while retaining only its native
24-byte output. Explicit DecodeRequest.assume_untagged_srgb tags it AssumedSrgb
and allows managed linear-sRGB float preparation, preserving opaque red and
transparent background; native+prepared ownership accounts 120 bytes until
last release. All thirteen PICT tests pass in /tmp/rrrah-pict-color-policy.log.
This is an explicit interpretation, not camera/producer color calibration;
GPU/cache/swap transport and broader PICT commands remain unqualified.

### PICT lease, swap pressure and actual Metal transport (2026-10-06)

The public decode_image route for the authored PICT v2 rectangle fixture passes
the ignored hdr_raster_swap integration test on Metal Apple M4 Max. A live RAM
lease prevents a zero-byte limit from evicting its payload. After release, the
payload is written once to disk swap. A full root budget refuses restoration;
releasing the competing owner permits retry without losing the stored entry.
Restored RGBA and explicit AssumedSrgb metadata match the decoded payload,
and the rendered 64-by-64 GPU frame matches the frame before swapping exactly.
All native and prepared allocations return to zero after last release.
Evidence: /tmp/rrrah-pict-metal-swap.log (one passed, zero failed), and
/tmp/rrrah-pict-decode-regression.log (736 passed, 42 ignored, zero failed).
This proves transport for the authored rectangle subset, not independent GPU
color accuracy, external-producer PICT coverage or physical HDR output.
PICT remains Pending in the 100-format catalog pending broader qualification.

### PICT neighbour preload under shared RAM pressure (2026-10-06)

The application foreground-neighbour predicate accepts PICT and uppercase PCT.
Its existing preload_raster path now has a fixture regression with a 4096-byte
shared root and count-one prepared raster cache. Full competing ownership
refuses a cold preload and leaves the cache empty. After pressure release,
preloading succeeds using explicit untagged-sRGB interpretation. Filling the
remaining root memory still permits a foreground cache hit without invoking
the decoder: prepared managed float storage is shared by pointer identity,
opaque red and transparent pixels are checked, and last-owner release returns
root usage to zero. Evidence: /tmp/rrrah-pict-prefetch.log.
This checks the actual preload and foreground cache paths, not asynchronous
gallery scheduling or rapid-navigation cancellation for PICT.

### PICT preload cancellation through latest-wins navigation (2026-10-06)

A regression uses ForegroundLoader's real submission queue and generation
tokens, with PICT speculative preloading on a spawned thread while a real
foreground decode permit is held. Repeated equivalent submission joins the
existing generation; PNG then GIF submissions invalidate its token and leave
only the latest GIF request pending. The stale preload refuses work and leaves
its prepared cache empty. All source/output ownership returns to zero, and
real GIF preload succeeds after releasing foreground admission. This covers
queue replacement, generation cancellation and permit recovery; it does not
run the window event loop or establish cancellation during PICT painting.
Logs: /tmp/rrrah-pict-navigation.log and /tmp/rrrah-pict-navigation-app.log.
The normal window loader currently constructs strict untagged-color requests:
PICT display remains refused without explicit color interpretation. Tests that
set assume_untagged_srgb do not establish ordinary window PICT display support.

### Explicit viewer untagged-color policy (2026-10-06)

CLI --untagged-color strict|srgb defaults to strict and is wired through
run_viewer and ForegroundLoader to both foreground execute_load and neighbour
preload DecodeRequests. Example: rrrah --untagged-color srgb drawing.pict.
The inspect path does not accept this viewer-only option. Unknown values are
rejected. Existing source profiles remain decoder-owned; this supplies an
explicit request assumption rather than fabricating a producer profile.
An application regression verifies default strict PICT preparation refusal,
explicit CLI-selected sRGB preload success, and rejection of the warmed assumed
frame after resetting the request to strict. Cache keys retain the assumption
flags, and all managed ownership returns to zero after cache release.
Evidence: /tmp/rrrah-untagged-policy-final.log. Wiring is source-verified;
window presentation and external-producer PICT correctness remain unproven.

### Actual PICT window smoke on Metal (2026-10-06)

Built and launched the ordinary rrrah binary with --untagged-color srgb
--no-cache tests/fixtures/pict/red-rectangle-v2.pict. The real macOS window
title reports image 3x2 ready and assumed sRGB; runtime reports Metal Apple M4
Max. After bringing the test process forward, a window-only screenshot visibly
shows the red painted region blended over the viewer's dark background.
Evidence: docs/research/pict-window-srgb-2026-10-06.json and matching PNG,
including source/screenshot hashes and command. The owned process was terminated
after capture. This establishes ordinary window presentation for one authored
subset fixture; it is visual smoke, not a screen pixel/color oracle. The title's
13.19ms measures loading and does not establish first-presentation latency or
physical HDR correctness. Broader external-producer PICT remains Pending.

### Filmstrip receives explicit viewer color policy (2026-10-06)

Inspection found that foreground/preload requests received --untagged-color
but thumbnail_loader_with_cancel still created strict requests. App::new now
passes the same startup policy to the thumbnail worker. A real authored PICT
thumbnail regression verifies strict refusal without retained memory, explicit
sRGB success at 3x2, exact opaque red and transparent-background compositing
to [36,36,36,255], and ownership of only the final 24-byte thumbnail. Cloning
that pixel owner retains the reservation after dropping ThumbnailReady;
last-owner release returns root usage to zero. Application regression:
/tmp/rrrah-thumbnail-policy-final.log, 142 passed, 14 ignored, zero failed.
This resolves a real policy mismatch; it does not qualify all PICT producers.

### PICT content routing through misleading suffixes (2026-10-06)

The bounded shared header read now covers 552 bytes, sufficient for PICT's
512-byte application header, picture frame and v2 version/header prefix.
Conservative PICT recognition requires positive bounded frame dimensions and
a complete recognized v2 header prefix. image_source_kind uses that signature
to override a misleading RAW suffix; decode_raster also dispatches by content.
A real fixture renamed to .cr3, .png and .unknown is decoded as raster with
3x2 dimensions and preserved Unspecified color. Every prefix shorter than
552 bytes and a zero-filled 552-byte header fail recognition. Last output
release returns managed RAM to zero in all cases. Decoder regressions:
/tmp/rrrah-pict-sniff.log, 737 passed, 42 ignored, zero failed.
Recognition is a candidate decision; full renderer validation still rejects
unsupported mappings/commands. This does not add extensionless gallery discovery
or qualify every PICT variant.

### External TwelveMonkeys PICT corpus probe (2026-10-06)

Downloaded four externally supplied corpus files into /tmp/rrrah-pict-external
from TwelveMonkeys imageio-pict test resources, preserving URLs and SHA-256
in docs/research/pict-external-corpus-probe-2026-10-06.json. No source binaries
were added to the repository. Independent TwelveMonkeys 3.13.0 decodes 6.pict
(335x300), mire16.pict and mire32.pict (both 64x64), with oracle output hashes
recorded. Native inspection refuses 6.pict at opcode 0x001e and both mire files
at resolution/bounding-box validation. Their header tails contain nonzero
unused bytes, requiring specification review before relaxing validation.
8.pict fails both native and independent decoding; the independent error is
unknown opcode 0x9876, so this source is not a valid positive oracle case.
External producer identity and corpus file licensing are not established by
repository placement. These probes demonstrate actual coverage gaps and do
not promote PICT from Pending. Next renderer work must address real command
streams and packed bitmap operations rather than only authored rectangles.

### PICT unused header tail compatibility (2026-10-06)

Apple Inside Macintosh Appendix A, listing A-5 and HeaderOp layout, labels the
last four header bytes unused/reserved. The reader no longer requires these
bytes to be zero; geometry, 72dpi resolution and supported version checks
remain. Regression compares exact rendered RGBA with zero, 0x00007fff and
0xffffffff extended-header tails. The malformed-mapping regression now changes
the actual fixed-point bottom rather than the unused field.
An ignored external-corpus test with RRRAH_PICT_CORPUS=/tmp/rrrah-pict-external
verifies both mire16 and mire32 now reach typed InvalidPict Unsupported(30),
with zero peak output allocation; logs /tmp/rrrah-pict-external-header.log and
/tmp/rrrah-pict-unused-header-final.log. Neither external image is yet rendered.
Primary specification: https://developer.apple.com/library/archive/documentation/mac/pdf/Imaging_With_QuickDraw/Appendix_A.pdf

### PICT DefHilite state compatibility (2026-10-06)

The borrowed command walker admits zero-data opcode 0x001e DefHilite. Its reset
of default highlight color does not affect admitted opaque paint rectangles;
the renderer explicitly permits it while HiliteMode and invert/highlight drawing
remain unsupported. A regression checks identical painted RGBA with DefHilite
and zero-allocation refusal after replacing it with HiliteMode.
Both external mire files now progress to Unsupported(154), DirectBitsRect,
without allocating output. Their command is at file offset 566: mire16 uses
rowBytes 128, packType 3, pixelSize 16, three 5-bit components; mire32 uses
rowBytes 256, packType 4, pixelSize 32, three 8-bit components. These require
bounded packed-row handling before external render qualification is possible.
All 16 PICT tests including the external ignored case pass in
/tmp/rrrah-pict-defhilite.log; full regression log is
/tmp/rrrah-pict-defhilite-regression.log. PICT remains Pending.

### Packed PICT direct-pixel row foundation and external oracle (2026-10-06)

Added unpack_direct_row: allocation-free decoding into caller-owned row storage
for packType 3 two-byte pixel runs and packType 4 one-byte component runs.
Literal/repeat lengths are checked against both source and destination, -128
is a no-op, malformed runs refuse, and cancellation is checked between runs.
Canary regressions cover truncated/oversized byte and word runs, invalid unit
sizes and cancellation before mutation.
The external ignored test decompresses all 64 rows of mire16 and mire32 using
only managed 128/192-byte scratch storage. Planar RGB8 and RGB555 expanded by
bit replication match all 4096 RGBA pixels per file against independent
TwelveMonkeys 3.13.0 outputs. Initial rounded RGB555 scaling disagreed by one
at an intermediate shade; bit replication resolves the full oracle comparison.
Peak scratch credit equals one row and drops to zero at last release.
Evidence: /tmp/rrrah-pict-external-packed-row-final.log (three passed),
source/oracle hashes in pict-external-corpus-probe-2026-10-06.json and
/tmp/rrrah-pict-packed-row-regression.log. This helper is not yet connected to
the command walker/renderer; the public decoder still refuses DirectBitsRect.

### Public PICT DirectBitsRect render qualification (2026-10-06)

DirectBitsRect is now connected to the borrowed walker and native renderer.
The admitted subset validates direct-pixel PixMap framing, RGB555 packType 3
or planar RGB8 packType 4, exact row pitch, positive bounds, full source bounds,
unscaled destination dimensions and srcCopy mode. Packed row framing honors
one/two-byte lengths and zero word-alignment padding. Other variants refuse.
The preflight determines maximum scratch size; rendering allocates one managed
RGBA canvas and one managed reusable row, applies destination translation and
rectangular clip, and checks cancellation during scans/rows. No whole decoded
source bitmap is retained alongside the canvas.
The public decode_image route now matches every RGBA byte of independent
TwelveMonkeys outputs for external mire16 and mire32. Exact measured peak
ownership is source length plus 16384-byte output plus 128/192-byte scratch;
only output remains on return and last release returns root use to zero.
Unsupported transfer mode refuses before allocation, and every truncated
suffix beginning at the DirectBits header refuses with no retained ownership.
All 19 PICT tests including external cases pass in
/tmp/rrrah-pict-directbits-oracle.log. Full decoder regression evidence:
/tmp/rrrah-pict-directbits-regression.log. These two cases establish public
packed-raster subsets, not universal PICT coverage; catalog remains Pending.

### PICT DirectBits scratch admission and post-allocation cancellation (2026-10-06)

The public external-file regression now sets the root to exactly source bytes
plus RGBA canvas plus one packed-row scratch. A competing one-byte managed
owner allows source/canvas allocation but refuses the scratch with typed
Source(Memory); source and canvas reservations are released, leaving only the
competing byte. Releasing it permits the same public request to succeed with
exact independent RGBA identity and final root usage zero for both mire files.
A separate renderer cancellation predicate triggers only after canvas and
scratch are both allocated, during the second framing pass; typed cancellation
releases both allocations, and retry again matches the oracle. This is not a
claim of cancellation during actual pixel copy. Evidence:
/tmp/rrrah-pict-directbits-pressure.log, one external test passed covering both
RGB555 and planar RGB8. Broad allocation-failure coverage remains incomplete.

### External packed PICT lease/swap/Metal integration (2026-10-06)

hdr_raster_swap now shares its PICT transport qualification helper between
the authored rectangle fixture and both external mire packed-raster cases.
Public native RGBA matches the independent TwelveMonkeys oracle before cache
admission and after disk restore. A live RAM lease resists reducing its limit
to zero; after last lease release, one disk swap write preserves pixel payload
and explicit AssumedSrgb color metadata. Filling the shared root refuses a
restore, releasing the competing owner permits retry, and managed native plus
display-prepared ownership returns to zero after last release.
Actual Metal Apple M4 Max renders a nonuniform image-detail frame for each
external case, with exact 64x64 frame identity before/after swap. This establishes
transport stability, not independent GPU color accuracy or physical HDR.
Both ignored PICT integration tests pass in
/tmp/rrrah-pict-directbits-metal-final.log; external sources/oracle hashes remain
in pict-external-corpus-probe-2026-10-06.json. CUDA/NVIDIA and broader PICT
variants remain unqualified, and the overall goal remains incomplete.

### Extended external PICT oracle corpus exposes resolution/cropping gap (2026-10-06)

Downloaded external 16bit.pict, 32bit.pict and FLAG_B24.PCT into the existing
/tmp corpus and decoded with independent TwelveMonkeys 3.13.0. Source URLs,
source hashes, header bytes and oracle hashes are recorded in
docs/research/pict-extended-corpus-oracle-2026-10-06.json. Both 16bit/32bit sources
produce identical 269x269 RGBA (oracle SHA e7beff811a64e21918115a7b1eabfa8b5fc0865bbb5228f7e1373b7eb96dd9e9),
while their picFrame is 201x201, extended hRes/vRes are 96dpi and srcRect is
269x269. The DirectBits PixMap bounds cover 270 columns, with only 269 selected
by srcRect; row pitch includes this extra column. Current native 72dpi/full-bounds
validation cannot support these legitimate external variants. Correct extension
requires choosing the appropriate extended-header pixel coordinate domain and
cropping source bounds while retaining full row pitch, not merely skipping
header validation. FLAG_B24's independent 124x124 output uses an unexpected
header revision 0x00001022; provenance/semantics need investigation before
admission. No new native coverage is claimed by these oracle probes.

### PICT packed source-rectangle cropping (2026-10-06)

DirectBitsRect now admits a positive srcRect contained within PixMap bounds
when destination dimensions match that selected source region. Stored full
row pitch and scratch size still derive from PixMap bounds; every packed row
is consumed/validated, while only selected source rows/columns are copied with
the proper destination offset. Planar channel offsets retain the full stored
width rather than the cropped width. A synthetic RGB555 case selects columns
1..4 of a four-column row: the distinctive excluded column does not leak into
the 3x2 output, exact RGB is checked, and peak credit is 24-byte canvas plus
8-byte row. Out-of-bounds source selection refuses before allocation.
All 20 PICT checks including previous external oracle/pressure cases pass in
/tmp/rrrah-pict-crop.log. Full regression log:
/tmp/rrrah-pict-crop-regression.log. Extended 96dpi coordinate-domain handling
remains outstanding; the new 269x269 external files are not yet admitted.

### Extended PICT resolution and public 96dpi oracle (2026-10-06)

canvas_header now derives the pixel coordinate domain from extended-v2 srcRect,
requiring positive fixed-point resolutions, positive bounded source dimensions,
and agreement with picFrame after resolution conversion to 72dpi logical units.
The renderer and public DecodedRaster use the same resulting pixel dimensions
and origin. Inconsistent geometry/resolution still refuses. The existing 72dpi
cases and malformed-resolution regression remain covered.
Both external 16bit.pict and 32bit.pict now decode through decode_image as
269x269 RGBA with every byte matching their independent TwelveMonkeys output.
The stored extra PixMap column is excluded through source cropping; managed
scratch is released before return, only 289444-byte output remains, and last
output release returns root usage to zero. All 21 PICT checks including external
cases pass in /tmp/rrrah-pict-96dpi.log. Full decoder regression log:
/tmp/rrrah-pict-96dpi-regression.log. This qualifies these 96dpi packed subsets,
not arbitrary QuickDraw mapping or external window/Metal handling at 96dpi.

### 96dpi PICT swap/Metal and catalog implementation status (2026-10-06)

Expanded the actual Metal transport test to all four independently decoded
external packed-raster PICT files, including both 96dpi 269x269 cases. A 4MiB
shared root accommodates native and prepared pixels. Native/restored RGBA
matches oracle bytes, RAM lease prevents forced eviction, full-root pressure
refuses restoration, release permits retry, nonuniform Metal frames match
exactly before/after swap and final managed ownership returns to zero.
Both integration tests pass on Metal Apple M4 Max in
/tmp/rrrah-pict-96dpi-metal.log. Catalog row 86 now reports an implemented
native subset with explicit limitations, consistent with other partial rows.
The current matrix is 89 implemented subsets and 11 Pending out of 100;
zero rows are universally qualified. External corpus licensing/producer color,
broader PICT commands, CUDA/NVIDIA and physical HDR remain incomplete.

### Reproducible pinned PICT qualification gate (2026-10-06)

scripts/qualify-pict.py verifies SHA-256 of all four external packed sources
and independent RGBA outputs plus exact oracle dimension/length before invoking
tests. It runs all 21 PICT native checks including ignored corpus cases;
--metal additionally requires both transport tests, five actual Metal adapter
announcements and zero skipped/failed checks. Captured command/log hashes and
explicit qualification limits are saved in the requested JSON report.
Current gate passes with --corpus /tmp/rrrah-pict-external --target-dir
/tmp/rrrah-required-corpus-target --report
docs/research/current-pict-native-metal-gate-2026-10-06.json --metal.
Negative validation changes one oracle byte in a temporary corpus copy and
confirms rejection before native/GPU checks without creating a success report;
evidence is pict-gate-negative-2026-10-06.json. Original corpus remains intact.
Pinned inputs make this subset acceptance repeatable; they do not resolve
external licensing, producer attribution or universal PICT correctness.

### Independent positive restore cap survives cloned PICT owners (2026-10-06)

The shared PICT transport helper now verifies the positive local restore cap,
not only shared-root exhaustion. With a restored object occupying the configured
one-payload restore limit and ample parent headroom, another restore returns
typed Capacity with limit equal to that payload weight. A clone retains the
same allocation after the original and prepared frame are dropped; another
restore still refuses until the clone is released. The same stored entry then
restores successfully with exact pixels. Last release returns shared usage to
zero; successful read counts are exactly two and write count remains one.
This executes for the authored rectangle plus four external RGB555/RGB8 cases,
including 96dpi sources, on actual Metal Apple M4 Max. The complete pinned gate
passes in docs/research/current-pict-restore-limit-gate-2026-10-06.json with
21 native tests and both Metal integration tests. Other payload families and
dynamic budget replacement are not newly qualified by this result.

### Restore-budget rebind aggregate-cap bypass reproduced (2026-10-06)

The standalone restore_rebind_probe example demonstrates a real remaining
library gap: with restore_bytes=4 and a 1024-byte shared root, restore one
four-byte object, keep it alive, call set_restore_budget with the same root,
then restore again. A fresh local child counter permits eight live restored
bytes despite the configured four-byte restore cap. The example exits nonzero
when this invariant is violated and checks that final release returns root
ownership to zero. Log: /tmp/rrrah-restore-rebind-probe.log. Thus old ownership
is still accounted at the root, but the local cap can be reset by rebinding.
Existing static-budget tests do not prove dynamic aggregate-cap correctness.
This requires an API/accounting correction; it is not yet fixed or qualified.

### Restore rebind aggregate-cap correction (2026-10-06)

ImageSwapCache retains old restore budget generations while their allocations
remain live. New restoration uses a child allowance equal to the configured
limit minus live current/retired usage, under serialized admission. Replacement
does not transfer old allocations to a new parent or lose their local charge.
New regression covers same-parent then different-parent rebind, cloned old
owners, refusal while those owners live and exact successful retry after last
release. A previous RAM test that relied on reset-cap behavior now explicitly
releases the old resident payload before retry.
All 112 cache library tests pass in /tmp/rrrah-restore-rebind-tests-final.log.
The original reproduction now exits successfully with restore_limit=4,
live_restored_bytes=4 and rebind_bypassed_limit=false in
/tmp/rrrah-restore-rebind-fixed.log. Concurrent restore throughput and all
dynamic application-setting combinations remain unqualified.

The complete pinned PICT gate also passes after this correction: 21 native
checks and both actual-Metal transport tests, with all five adapter observations.
Evidence: research/current-pict-rebind-fixed-gate-2026-10-06.json. Capacity
assertions accept the refusing ancestor's cap or the temporary child's remaining
allowance; they still require typed refusal and exact retry/owner behavior.

### Concurrent restore allowance after budget rebind (2026-10-06)

A barrier-started eight-thread regression restores a two-byte payload while
a one-byte retired-generation owner occupies a three-byte aggregate cap.
Exactly one thread succeeds; its returned allocation remains owned while all
seven competitors report typed Capacity. Original/current shared roots report
one/two bytes respectively, never exceeding the aggregate cap. Releasing all
returned/retired owners returns both roots to zero; a subsequent restoration
of the same disk entry succeeds with exact pixels. All 113 cache library tests
pass in /tmp/rrrah-restore-concurrent.log. This proves the targeted concurrent
admission invariant, not restore throughput or all possible interleavings.

### Cancellation while waiting for serialized restore admission (2026-10-06)

try_get now checks cancellation before attempting restore admission and during
one-millisecond try_lock retries rather than blocking unconditionally. A test
holds the admission mutex while another thread requests a real stored payload,
checking immediate cancellation and cancellation after repeated wait checks.
Both requests return Ok(None) before the mutex is released, with zero reads
and no retained managed allocation. After release, the same entry restores with
exact pixels. Existing corrupt-old-entry/replacement-key regression still passes;
the uncontended path retains the previous cancellation-check order.
All 114 cache library tests pass in /tmp/rrrah-restore-wait-cancel-final.log.
The polling interval is not a benchmarked end-to-end cancellation bound.

Application regression after the restore-wait change also passes:
/tmp/rrrah-restore-wait-app.log, 142 passed, 14 ignored, zero failed.

### Retired restore pressure remains eligible for foreground eviction (2026-10-06)

The temporary remaining-allowance child could report limit zero while old
restored owners consumed the cap, incorrectly marking retry as intrinsically
impossible in foreground RAM caches. MemoryBudget::allocation_limit now exposes
the minimum configured cap across ancestors, independent of current occupancy.
Swap admission normalizes retired/current live pressure to aggregate occupancy
and that full cap, while true zero ancestor caps still refuse without eviction.
The RAM regression verifies that a visible retired frame remains protected;
after removing its visible pin, a foreground restore releases it automatically
and succeeds without manually clearing RAM. The old shared root returns to zero.
All 114 cache and 38 memory tests pass in
/tmp/rrrah-retired-pressure-retry-final.log. This corrects pressure retry
classification, not allocation admission itself; aggregate caps still bind.

Application regression also passes after this pressure classification change:
/tmp/rrrah-retired-pressure-app.log, 142 passed, 14 ignored, zero failed.

### Unpacked and short DirectBits PICT rows (2026-10-06)

DirectBitsRect now supports packType 1 RGB555 and 32-bit xRGB, plus the
specified raw-storage rule for rowBytes below eight even when packed type is
declared. Framing consumes raw full-pitch rows without length prefixes;
rendering uses managed row scratch, preserves RGB555 expansion and ignores
the 32-bit high pad byte rather than treating it as alpha. Packed RGB8 retains
its existing planar layout. Four synthetic cases cover short RGB555/xRGB,
explicit unpacked full-width RGB555/xRGB, exact red/opaque RGBA, truncated rows
and final managed release. All 22 PICT checks including previous independent
external cases pass in /tmp/rrrah-pict-unpacked-final.log. Full regression log:
/tmp/rrrah-pict-unpacked-regression.log. New unpacked variants currently have
synthetic evidence only; wider independent producer qualification remains open.
The pinned qualifier now expects all 22 native checks.

### Authored unpacked PICT interoperability (2026-10-06)

Four CC0 binary fixtures and their full oracle outcomes are recorded in
`tests/fixtures/pict/unpacked-manifest.json`. macOS sips produces exact RGBA
for short and explicit unpacked 32-bit xRGB, including a nonzero ignored pad
byte. Both RGB555 fixtures produce white pixels through sips, while
TwelveMonkeys 3.13.0 rejects three fixtures and misreads the unpacked xRGB
channels. These disagreements remain recorded; RGB555 unpacked interoperability
is not qualified. The public native regression checks authored pixels, release
of managed memory, and the recorded macOS pixels for both xRGB fixtures.
The qualifier now expects 23 native checks; four packed external oracles and
the two Metal transport tests retain their existing scope.

The unpacked nonuniform xRGB fixture also exercises the full public RAM lease,
streamed swap, ancestor/local restore pressure and actual Metal readback path.
Its native samples are compared with the recorded macOS oracle before GPU
preparation; pre/post-swap GPU frames must agree. The qualifier expects three
transport tests and six Metal adapter observations.

TwelveMonkeys 3.13.0 source inspection shows DirectBits row framing and pixel
interpretation selected by packType: types above two always consume a packed
length prefix; RGB555 conversion is only selected for type three; other RGB
paths use planar channel offsets. This explains that oracle's short-row,
unpacked RGB555 and xRGB outcomes, but does not resolve the macOS RGB555
disagreement or establish universal correctness. Source:
https://github.com/haraldk/TwelveMonkeys/blob/twelvemonkeys-3.13.0/imageio/imageio-pict/src/main/java/com/twelvemonkeys/imageio/plugins/pict/PICTImageReader.java

### Drop-pad DirectBits RGB (2026-10-06)

PICT packType 2 now admits 32-bit PixMaps with RGB24 interleaved storage when
rowBytes is at least eight. Short rows preserve the specified raw XRGB rule.
Framing, scratch admission and rendering use the stored RGB byte count, while
the PixMap pitch remains four bytes per pixel. A checked-in CC0 two-color
fixture matches macOS sips full RGBA exactly. Existing public fixture checks
cover it and synthetic cases exercise both short and full-pitch type 2 rows,
truncation and managed release. Transport qualification now includes this
fixture alongside unpacked xRGB; the qualifier expects seven Metal observations.
RGB555 disagreement persists after explicit Clip and declared-size experiments.
Reference: https://dev.os9.ca/techpubs/mac/QuickDraw/QuickDraw-461.html

### Directional raster neighbour admission with independent limits (2026-10-06)

`pict_neighbour_windows_obey_count_bytes_and_ttl_without_losing_visible_frame`
uses five distinct real PICT files and the actual neighbour planner/preload/
foreground cache paths. Forward and backward plans each select three neighbours
for behind=1/ahead=2 and prioritize the direction of travel. Separate policies
(two entries with ample bytes, or 192 bytes with no count cap) retain exactly
two 96-byte prepared frames while repeated neighbours cannot displace the
visible frame or force its re-decode. Switching policy to zero TTL preserves
existing insertion deadlines; a newly inserted neighbour then expires and
releases its managed storage while the visible frame stays cached. Dropping
all owners releases the root budget to zero in both configurations.
Evidence: /tmp/rrrah-pict-neighbour-limits.log. This is synchronous planner and
loader integration, not a live window latency or asynchronous scheduler proof.

### Navigation supersession between decode and color preparation (2026-10-06)

The real-source post-decode cancellation regression now covers profiled PNG,
GIF, PICT rectangles, unpacked xRGB and drop-pad RGB. Each case decodes into
managed storage, supersedes its GenerationToken, and verifies typed color
preparation cancellation without any increase of peak allocation. All input
storage releases to zero. A replacement token for the current generation then
decodes and prepares that same source successfully, and final ownership release
returns the root budget to zero. This exercises the actual generation token,
native decoder and preparation boundary; the generation change is injected
deterministically and does not prove live-window scheduling or latency.
Full application regression: 143 passed, 14 ignored in
/tmp/rrrah-pict-postdecode-regression.log.

### Current real CRW cache-route timing (2026-10-06)

The existing raw_view_timing --cache-only benchmark was rerun against pinned
Canon EOS 10D CRW on the current sources with 512 MiB and 64 MiB managed
root caps. Full pixels/metadata agree for each decode/disk/swap iteration;
final root/restore usage is zero. Sixteen retained rounds rotate tier order
after four warmups. At 64 MiB, decode p50/p95 is 123.993/125.899 ms and swap
8.095/8.382 ms, with 33,860,674 bytes peak managed root storage. These are
warm-OS CPU-only timings, not first displayed frame or universal RAW evidence.
RAM timings average 1000 hits per round, so their p95 is a batch-mean statistic.
Pinned source hash, samples, logs, both budgets and limitations are recorded in
research/current-crw-cache-routes-2026-10-06.json.

### Current bounded CRW Metal view timing (2026-10-06)

The existing RAW view benchmark runs the pinned EOS 10D source with 64 MiB
managed CPU and 128 MiB renderer atlas caps on Metal Apple M4 Max. Fifteen
retained warm-OS samples after three warmups give decode-to-completed-offscreen
frame p50/p95 133.628/136.860 ms; 60 resident pan/zoom frames give
0.794/1.259 ms. Complete sensor pixel identity is checked outside timings.
Final managed CPU/GPU usage is zero. CPU peak 66,154,376 bytes includes a
retained sensor oracle; GPU peak 39,790,864 covers renderer atlas textures,
not all wgpu allocations. This is neither physical display latency nor an
end-to-end swap-to-GPU measurement. Full samples, log hashes and scope are in
research/current-crw-metal-view-2026-10-06.json.

### Direct swap-to-Metal RAW timing (2026-10-06)

The existing raw_view_timing now supports --swap-view. It seeds actual bounded
MosaicSwapCodec storage, retains the original native decoded sensor samples as
the oracle, and performs 18 separate managed restores followed by atlas upload
and GPU submission completion. Full pixels and metadata compare outside every
timed interval; reads=18/errors=0 and final managed CPU/GPU usage zero are
asserted. With 64 MiB CPU/128 MiB atlas caps, 15 retained warm-OS samples give
complete restore-to-frame p50/p95 15.981/16.576 ms on Metal Apple M4 Max.
This measures the complete path directly; it is not a sum of medians. Seed
decode/write, startup, swapchain and physical presentation remain excluded.
Full evidence: research/current-crw-swap-metal-2026-10-06.json.

### CR3 complete swap/decode GPU routes (2026-10-06)

Pinned Canon EOS R8 IMG_9074.CR3 was measured with the same existing view
benchmark, Metal adapter and 256 MiB CPU/atlas caps. Swap-to-completed-frame
p50/p95 is 67.596/94.556 ms; separate decode-to-frame is 74.335/110.735 ms.
This modest observed median difference is from sequential runs, not a paired
controlled comparison; a general speed advantage is unproven. Both release
managed CPU/GPU usage to zero. Original decoded pixel/metadata checks and
18 successful swap restores remain enabled. Detailed scope, source and
log/sample hashes: research/current-cr3-swap-versus-decode-metal-2026-10-06.json.

### Swap block copy experiment rejected (2026-10-06)

A direct-read branch bypassed the internal 64 KiB copy for large initialized
destinations, preserving bounded read/hash/cancel behavior. rrrah-swap tests
passed, but real CR3 swap p50 32.922 ms versus repeated buffered 33.529 ms
does not establish a meaningful causal speedup amid broader run variation.
The experimental branch was removed; original reader remains authoritative.
Evidence and samples: research/cr3-direct-read-experiment-rejected-2026-10-06.json.
Future optimization needs CPU profile or paired controlled evidence rather
than attributing the previous 37.943 ms sample directly to the block copy.

### Real CR3 swap CPU sampling (2026-10-06)

A 2-second /usr/bin/sample capture of the existing cache benchmark isolates
320 observed stacks in the swap restore subtree: 232 in BLAKE3 update, 67 in
managed allocation/resize_with, 17 in file read and four in memmove. This is
short stack sampling, not exact CPU time percentages. The extra block copy
is not the dominant sampled path. Replacing resize_with with Vec::resize
passed 114 cache and 38 memory tests, but swap p50 32.987 ms versus sampled
baseline 33.110 ms gave no convincing benefit; the experiment was reverted.
Checksum verification and initialized managed allocation remain intact.
Evidence and full profile: research/cr3-swap-cpu-profile-2026-10-06.json.

### Cropped odd-length drop-pad PICT fixture (2026-10-06)

The authored corpus includes a 3x3 stored RGB24 PixMap cropped to a 2x2
output: one source row and one column are excluded, all three nine-byte
rows are consumed, and the 27-byte total requires a zero alignment byte
before EndPic. The public native fixture check now uses manifest heights
and compares all four output RGBA pixels with authored coordinates. A
nonzero alignment byte must fail preflight with peak/final budget zero.
macOS sips returns white pixels for this cropped fixture; the full outcome
is preserved in unpacked-manifest.json. This case is authored-only evidence,
not independently qualified cropping interoperability. MEF/EIP remain Pending.

### X3F borrowed container foundation (2026-10-06)

Public inspect_x3f supports bounded container inventory for versions 2.0–2.2
from the Foveon external specification. It borrows all section payloads, rejects
invalid header/directory framing, dimensions/rotation, count above 1024,
unaligned/out-of-range/overlapping sections, and checks cancellation during
all bounded overlap comparisons. Unknown entry kinds remain available without
rendering or preview fallback. Synthetic tests cover borrowed pointer identity,
extended headers, every truncated prefix, overlap/range/count/version errors
and cancellation. Sensor decompression, properties/color and external camera
qualification are missing; X3F remains Pending and catalog counts unchanged.
Reference: https://libopenraw.freedesktop.org/formats/x3f/x3f-raw-format.pdf

### Real Sigma SD10 X3F section inventory (2026-10-06)

Downloaded raw.pixls.us object 571, verified published SHA256, and inspected
five directory entries independently using Python struct. Native image_info
reports borrowed section headers with bounded positive dimensions, version
validation and row alignment. Output header 2268x1512 differs from raw sensor
2304x1531 type=3/encoding=6; type=2 previews are 567x378 encoding=11 and
189x126 encoding=3 with 568-byte rows. An external ignored native regression
pins these distinctions and keeps sensor compression/color unsupported.
Source attribution/license, exact directory ranges and command are recorded
in research/x3f-sigma-sd10-inventory-2026-10-06.json; binary not redistributed.
No rendered pixel oracle or complete X3F support is claimed.

### X3F legacy Huffman framing (2026-10-06)

Source inspection of pinned LibRaw 0.22.2 identifies type=3/encoding=6 as
legacy 10-bit mapped Huffman deltas. X3fEntry::legacy_huffman borrows 1024
signed mapping entries, 1024 length/code words, the entropy stream and
the trailing row-offset table. It bounds every row independently and rejects
invalid code widths, duplicate/prefix-conflicting codes, empty streams,
nonmonotone/aliased/out-of-range rows and cancellation, without allocation.
Tests cover malformed/cancelled framing and the real pinned SD10 layout.
Predictor wrapping, automatic legacy offset, actual sensor samples and color
remain unqualified; X3F still Pending. Layout measurements/source hash are in
research/x3f-sd10-legacy-layout-2026-10-06.json.

### Native legacy X3F channels (2026-10-06)

X3fLegacyHuffman::decode_channels admits the Huffman tree and complete three
channel output through MemoryBudget, bounds entropy reads by row, checks
cancellation, accumulates signed wrapping predictors and performs the legacy
minimum-based second pass. It returns camera-channel samples, not display RGB.
Five checks include synthetic negative offset, typed zero-cap refusal, tree
cancellation, final managed release and real SD10 decode. SD10 produces
10,582,272 samples with offset 202; reproducible native hash is recorded in
research/x3f-native-channels-2026-10-06.json. The installed LibRaw 0.22.2
reports X3FTOOLS=0 and refuses this file, so independent sensor equality is
unproven. A test-only x3f-channel-oracle.cpp is ready for an enabled build.
CAMF/WB/color and public viewer support remain Pending; no identity fallback.

### Independent legacy X3F sensor equality (2026-10-06)

An isolated test-only LibRaw 0.22.2 build with USE_X3FTOOLS enabled now
unpacks the pinned Sigma SD10 source. All 10,582,272 native u16 channel
samples match the independent oracle exactly. Five scoped tests pass,
including malformed input, cancellation, memory pressure and managed release.
Hashes and the test command are recorded in
research/x3f-independent-channels-2026-10-06.json. The earlier disabled-oracle
report is retained as historical evidence. Production remains native Rust.
X3F remains Pending: CAMF/WB/color, public viewing and broader variants
are not yet qualified.

### X3F properties and calibration framing (2026-10-06)

Native borrowed PROP parsing validates bounded tables, UTF-16 offsets,
termination and Unicode, with per-property cancellation. The pinned SD10
checks all 26 properties, including manufacturer, model, ISO and Auto WB.
A borrowed CAMF header now exposes encoding and four parameters, rejecting
truncated/empty/version-mismatched sections. SD10 has encoding 2 and a
52,520-byte calibration payload; calibration decoding/color remain Pending.
The six scoped tests, including full independent sensor equality, pass.

### Native X3F CAMF type-2 deobfuscation (2026-10-06)

Calibration bytes now decode through managed Rust storage, with cancellation
checks before admission, every 1024 bytes and before returning. Unknown
encodings are refused. The real SD10 yields 52,520 bytes beginning CMbT.
Seven scoped tests pass, including full independently qualified sensor
channels, zero budget, cancellation/release and a synthetic reversible
transform. Calibration correctness still needs an independent byte oracle;
entry/matrix parsing, WB/color and public viewing remain Pending. Evidence:
research/x3f-camf-type2-2026-10-06.json.

### Independent X3F calibration bytes and record traversal (2026-10-06)

An external test-only enabled LibRaw x3f_load_data oracle exactly matches
all 52,520 native SD10 calibration bytes. Borrowed record traversal now
bounds sizes/name/value offsets and string termination, checks cancellation
and caps records at 1024. All 54 real records are visited; malformed zero
size and cancellation are refused. Seven scoped tests pass including full
sensor equality. Evidence: research/x3f-camf-independent-2026-10-06.json.
Matrix/property values, WB/color and public viewer integration remain Pending.

### Bounded native X3F CAMF matrices (2026-10-06)

All 47 SD10 matrix records now parse as borrowed numerical payloads.
The parser bounds dimension tables/data, checks multiplication overflow,
validates supported element widths and rejects missing/duplicate/out-of-range
axes. SpatialGain and LinLUTS preserve their actual permuted axis order.
AutoRGBNeutral is checked as three finite float32 values. Seven scoped
tests pass with independently matched sensor/calibration bytes. Numerical
calibration interpretation and final display color remain unqualified.

### X3F matrix corruption regression and color inventory (2026-10-06)

Synthetic matrices now exercise every truncated prefix, unsupported element
types, missing/excessive dimensions, duplicate/out-of-range axes, zero
extents, invalid name/data offsets and numerical product overflow. All six
admitted element types and reversed axis order have positive coverage.
Full decoder regression: 750 passed, 46 ignored, no failures. Color-related
SD10 calibration values are inventoried from independently matched CAMF
bytes in research/x3f-sd10-color-calibration-inventory-2026-10-06.json.
These extracted matrices do not yet qualify their application or display color.

### Typed finite X3F calibration access (2026-10-06)

CAMF matrices expose bounded finite float32 access and logical axis sizes
without copying storage or assuming dimension order. Wrong source type,
NaN/infinities, out-of-range indices and truncated payloads return errors.
Nine scoped tests pass, including exact real AutoRGBNeutral values and
independently matched complete sensor/calibration bytes. AutoRGBNeutral
extraction does not yet prove its application or final image color.

### Native CAMF white-balance property links (2026-10-06)

Borrowed CMbP traversal validates property counts/table/string-base offsets,
NUL termination and per-property cancellation without allocation. Synthetic
checks cover truncation, invalid offsets and cancellation. The pinned SD10
WhiteBalanceCorrections has eight links; Auto explicitly references its
embedded WBCorrection_Identity matrix. This is file-provided calibration,
not a default fallback. Ten scoped tests pass, including complete independent
sensor/CAMF equality. Calibration application and display color remain Pending.

### Explicit X3F WB correction resolution (2026-10-06)

Native x3f_wb_correction resolves a requested mode through exactly one
WhiteBalanceCorrections table/link and exactly one finite 3x3 float matrix.
Missing/duplicate/wrong-type/malformed calibration is refused. SD10 Auto
resolves to its explicitly embedded identity matrix; this is not a fallback.
Ten scoped tests pass with unknown-mode, duplicate-corpus and cancellation
checks, plus independent full sensor/CAMF equality. This extracts source-order
correction coefficients; the full Foveon sensor correction/color pipeline
and public viewer integration remain Pending.

### X3F illuminant resolution and full-color requirements (2026-10-06)

Native linked matrix resolution now also extracts WhiteBalanceIlluminants
with identical missing/ambiguous/finite-3x3 checks. Pinned SD10 Auto and
Flash explicitly resolve the same CamToXYZ_Flash coefficients. Ten scoped
tests pass. Full-color processing requirements were inspected against the
dcraw Foveon implementation and recorded with source hash in
research/x3f-foveon-color-requirements-2026-10-06.json. Dark shielding/drift,
spatial gain, polynomial correction, filtering and crop are still required;
WB/illuminant extraction alone does not qualify native display color.

### Native SpatialGain interpolation (2026-10-06)

CAMF float matrices now support bilinear sampling of row/column/channel
SpatialGain storage at normalized grid coordinates without allocation.
Dimensions, products, finite/range coordinates and positive finite corner
gains are checked. Synthetic corners/center and invalid inputs are tested;
real SD10's first triple and center/end positivity are checked. Eleven scoped
tests pass. Sensor pixel-to-grid mapping, dark correction and full application
are still unqualified; this primitive alone does not establish display color.

### Native Foveon post polynomial primitive (2026-10-06)

PostPolyMatrix evaluation now uses bounded signed fixed-point square, cross
and cubic terms, finite float coefficients and checked result conversion.
Tests cover zero coefficients, explicit square/cross correction, negative
input, oversized cubic products and NaN refusal, plus real SD10 matrices.
Twelve scoped tests pass. Accumulation uses f64; independent whole-image
color/rounding equality is not established. Integration with dark correction,
gain/WB/filtering and public viewer remains Pending.

### Native DarkDrift interpolation (2026-10-06)

A bounded finite float DarkDrift primitive interpolates top/bottom slope
and offset for each camera channel over actual sensor rows. Shape, height,
row bounds and nonfinite coefficients/results are checked. Synthetic top,
center and bottom values and invalid rows/nonfinite inputs are covered.
Real SD10 coefficients are checked alongside sensor/CAMF byte equality.
Thirteen scoped tests pass. Dark shielding estimates, black smoothing,
sensor application and whole-image color qualification remain Pending.

### Native X3F shield means and coordinate mismatch (2026-10-06)

Signed legacy sensor channel means now support explicitly mapped inclusive
shield rectangles without allocation, with bounds checks and row/256-pixel
cancellation. Synthetic signed means, invalid ranges and cancellation/retained
lease release are tested; real SD10 top shield [10,2,2294,8] is admitted.
CAMF bottom shield [10,1528,2294,1534] exceeds decoded height 1531 and is
explicitly refused. Its coordinate mapping/calibration meaning must be
qualified before use; no clipping or fabricated black fallback is applied.
Fourteen scoped tests pass. Whole-image black/color integration remains Pending.

### X3F IncludeBlocks semantics and shield branch clarification (2026-10-06)

Native x3f_block_enabled checks key presence, including empty string values,
and rejects ambiguous tables/duplicate keys. SD10 explicitly enables DarkDrift,
ColumnFilter and AutoRGBNeutral with empty values. The reference Foveon
algorithm uses embedded DarkDrift when present, so the out-of-frame bottom
shield is not needed on this branch. The prior coordinate mismatch remains
a valid refusal for fallback shield sampling, not a blocker for this file's
embedded-drift path. Fourteen scoped tests pass including real inclusion,
missing blocks, duplicate corpus, cancellation and independent byte equality.
Full black smoothing/application and display color remain Pending.

### Native shield-column filtered means (2026-10-06)

Signed legacy channels now support bounded filtered column means per row,
with prior-column differences, two-sample averaging and min/max trimming
for longer ranges. Row/range/filter/output validity and cancellation are
checked without allocation. Synthetic outlier, signed negative channels,
filter response and malformed/cancelled inputs pass. Scoped tests: 14
passed, 1 real-corpus test ignored in this run. Calibration-to-column mapping,
black smoothing and whole-image application remain Pending.

### Managed initial X3F black rows (2026-10-06)

Native black_rows combines two filtered shield-column means with interpolated
DarkDrift into managed per-row/channel storage. Real SD10 uses explicit
calibration ranges [6,14]/[2292,2300] and ColumnFilter 0.8 in the test.
All 4,593 initial values are finite; zero-budget admission is refused and
midstream cancellation releases the result allocation. Fifteen scoped tests
pass, including complete independent sensor/CAMF equality. Black smoothing,
independent processing-value oracle and whole-image application remain Pending.

### X3F initial black arithmetic comparison (2026-10-06)

All 4,593 initial black-row/channel values exactly match a separately
implemented C++ formula using independent LibRaw sensor and CAMF dumps.
Observed max absolute error is zero; test tolerance is 0.00005. This checks
arithmetic with an independent implementation, not a complete converter
image oracle. Fifteen scoped tests pass. Helper/input/report hashes are in
research/x3f-black-independent-2026-10-06.json. Black smoothing and whole
sensor/display processing remain Pending.

### Native X3F black row smoothing (2026-10-06)

In-place black-row smoothing now includes edge replication, local outlier
clamping and forward/reverse exponential passes with finite validation and
cancellation. It returns the forward mean for the later scene adjustment.
Tests cover constant rows, isolated spikes, short/nonfinite buffers and
cancellation. Fifteen scoped tests pass, one real-corpus check ignored in
this run. Independent smoothing arithmetic and final scene adjustment are
still unqualified; cancellation may leave caller-owned scratch partially changed.

### Independent X3F black smoothing arithmetic (2026-10-06)

A separate C++ arithmetic implementation exposed two native discrepancies:
unconditional median replacement versus conditional extrema clamping, and
f64 versus float32 intermediate precision. Both were corrected. All 4,593
SD10 smoothed black values now match exactly (max absolute error zero).
Sixteen scoped tests pass, including independent initial/smoothed black and
full sensor/CAMF comparisons. Evidence: research/x3f-smooth-independent-2026-10-06.json.
This is arithmetic qualification, not whole-image color qualification.

### Native X3F scene black adjustment (2026-10-06)

Scene adjustment samples signed legacy channels every fourth row/column
starting at (2,2), accumulates checked integer sums and combines the sample
mean with the forward smoothing mean. Buffer dimensions, finite values and
cancellation are validated; caller scratch can be partially updated on late
cancellation. Synthetic signed/stride/formula checks pass. Scoped tests:
16 passed, one real-source test ignored. Real-source arithmetic/rounding
comparison and whole sensor/color integration remain Pending.

### Independently compared X3F scene-adjusted black (2026-10-06)

Scene ratio/final addition precision now follows the reference expression
without prematurely rounding the correction. All 4,593 SD10 adjusted black
values match separate C++ arithmetic exactly; initial and smoothed values
still match exactly. Seventeen scoped tests pass including complete sensor
and CAMF equality. Evidence: research/x3f-scene-independent-2026-10-06.json.
Whole-image correction, filtering, display color and viewer remain Pending.

### Managed combined X3F black processing (2026-10-06)

processed_black_rows now combines admitted initial shield/drift estimates,
qualified smoothing and scene adjustment in one cancellable native call.
Both immutable initial and mutable scratch allocations share the supplied
MemoryBudget; initial storage is released before later stages. Pinned SD10
output exactly matches the separately compared staged result. Tests prove
retained clone accounting, insufficient scratch admission with complete
release and mid-pipeline cancellation without leaked allocations. Seventeen
scoped tests pass. Full sensor corrections/filtering/display remain Pending.

### File-resolved X3F black processing (2026-10-06)

processed_black_from_camf resolves unique typed DarkShieldColRange, enabled
DarkDrift and optional enabled ColumnFilter directly from calibration bytes.
The shield-derived drift branch fails explicitly until implemented. Missing,
malformed and ambiguous matrices return errors; no SD10 numeric parameters
are hardcoded in this path. Pinned SD10 automatic parameter resolution exactly
matches the independently compared staged output. Seventeen scoped tests
pass including duplicate corpus, cancellation and final managed release.
Full sensor correction/filtering/display remains Pending.

### Combined native X3F sensor correction stage (2026-10-06)

correct_sensor_channels combines file-resolved processed black, interpolated
DarkDrift, column difference filter, PostPolyMatrix, integer-spaced SpatialGain
sampling and explicitly supplied positive neutral response. Output is managed
signed i32 camera channels, not display RGB. The full SD10 frame processes
with correct output length and upper saturation bound; intermediate black
storage is released and dropping output returns its budget to zero. Invalid
neutral and cancellation release checks pass. Seventeen scoped tests pass.
Independent corrected-channel arithmetic/rounding, neutral extraction, bad
pixels, chroma/color filters and final color/viewer integration remain Pending.

### File-resolved X3F Auto neutral sensor stage (2026-10-06)

correct_sensor_auto requires the explicit AutoRGBNeutral inclusion and unique
three-element finite float matrix, then runs the managed sensor correction
stage. All SD10 corrected channel values exactly match the explicitly supplied
neutral route. Missing renamed calibration is refused with zero allocation;
cancellation and result release are checked. Seventeen scoped tests pass.
This comparison establishes routing consistency, not independent corrected
channel/color correctness; other neutral branches, filtering and final
color/viewer integration remain Pending.

### Reproducible pinned X3F qualification gate (2026-10-06)

scripts/qualify-x3f-sd10.py now verifies the pinned source hash, compiles
external test-only sensor/CAMF oracles against an explicitly enabled isolated
LibRaw 0.22.2 build, generates separate C++ black arithmetic references and
runs all 17 X3F tests with fresh temporary outputs. It refuses unexpected
oracle versions, disabled support, skipped checks and nonzero observed black
arithmetic error. Reports retain source/helper/output hashes and command logs.
The actual run passed: research/x3f-reproducible-gate-2026-10-06.json.
Corrected-channel arithmetic and final color/viewer qualification remain Pending.

### Independent full X3F corrected sensor arithmetic (2026-10-06)

A separate C++ sensor-stage arithmetic implementation initially exposed
949 mismatched values (maximum difference 2). Native polynomial accumulation
was corrected to float32 and SpatialGain interpolation now follows vertical
then integer-spaced horizontal weighting. All 10,582,272 corrected SD10
channel values now match exactly (zero mismatches and maximum difference).
The reproducible gate includes fresh sensor-stage output and requires exact
equality: research/x3f-corrected-gate-2026-10-06.json. Seventeen scoped tests
pass. This qualifies the sensor stage only; bad pixels, filtering, crop,
final color, viewer integration and other variants remain Pending.

### Native X3F encoded bad-pixel repair primitive (2026-10-06)

repair_x3f_bad_pixels now decodes packed coordinates and eight-neighbor masks,
applies an explicit coordinate origin and averages admitted signed channels
in-place without allocation. Noninterior coordinates are skipped, malformed
code lengths/frame sizes and out-of-i16 neighbor samples are refused. Tests
cover diagonal masks, negative means/truncation, translated origin, borders
and cancellation. Seventeen scoped tests pass, one real-source test ignored.
File-origin resolution, independent real-frame repair comparison and filters
remain Pending; this primitive is not yet in the public viewer path.

### File-resolved independently compared X3F bad pixels (2026-10-06)

Native bad-pixel repair now resolves KeepImageArea coordinate origin and
BadPixels codes from unique typed CAMF matrices. SD10 repairs 148 pixels.
Every one of the 10,582,272 resulting camera-channel values exactly matches
a separate C++ repair implementation operating on the independently
compared sensor-stage output. The reproducible gate now generates and
requires this reference: research/x3f-badpixel-gate-2026-10-06.json.
All 18 scoped tests pass. Color/chroma filters, final crop/color, viewer
integration and other X3F variants remain Pending.

### Native X3F red sharpening stage (2026-10-06)

Red sharpening uses separable 5x5 Gaussian estimates and a five-row managed
ring; only interior red samples are changed. Checks cover signed source
range, dimensions, budget admission, cancellation and final scratch release.
Synthetic constant/impulse behavior, unchanged two-pixel borders and green/blue
channels are tested. Eighteen scoped tests pass, one real-source test ignored.
Independent full-frame red-stage comparison and subsequent brighter-pixel
linearity/chroma/color filtering remain Pending.

### Independently compared full X3F red sharpening (2026-10-06)

A separate C++ implementation precomputes all horizontal Gaussian rows,
independently of the native managed five-row ring. Every one of 10,582,272
SD10 output channel values matches exactly after red sharpening. Native
scratch usage returns to zero. The reproducible gate generates and requires
this oracle, and now requires all 19 scoped tests: research/x3f-red-gate-2026-10-06.json.
Brighter-pixel linearity, remaining chroma/color filters, crop/final color and
viewer integration remain Pending.

### Native Foveon highlight linearity primitive (2026-10-06)

Highlight adjustment now derives the transition threshold from explicit
saturation/normalized neutral response and applies legacy fixed-point channel
convergence. Synthetic below-threshold, transitional and fully bright samples
match analytical values. Invalid calibration, malformed channel length and
cancellation are refused. Nineteen scoped tests pass, one real-corpus test
ignored. File-resolved saturation and independent full-frame highlight
comparison remain Pending, along with chroma/color filters and final display.

### File-resolved independently compared X3F highlights (2026-10-06)

Native highlight linearity now resolves u16 SaturationLevel and explicitly
enabled finite AutoRGBNeutral from unique CAMF matrices. Every one of
10,582,272 full-frame SD10 channel values matches a separate C++ arithmetic
reference after highlight processing. The reproducible gate generates and
requires this comparison: research/x3f-highlights-gate-2026-10-06.json.
All 20 scoped tests pass. Hue/chroma filters, final crop/color, viewer
integration and broader X3F variants remain Pending.

### Managed native Foveon noise curves (2026-10-06)

X3fNoiseCurve builds cosine/tanh legacy noise response in managed i16
storage with finite/domain/value validation and cancellation. Domain length
is stored separately and limited to the positive legacy i16 range. Signed
application is symmetric and handles i32::MIN without overflow. Tests cover
domain bounds, zero filter's specified 0.8 behavior, invalid parameters,
zero-budget admission and cancellation/release. Twenty scoped tests pass,
one real-source test ignored. File-resolved curve parameters, independent
curve arithmetic and actual chroma filtering remain Pending.

### File-resolved managed Foveon noise bank (2026-10-06)

Eight noise curves now resolve ColorDQ/ColorDQCamRGB selection, ChromaDQ,
ColumnFilter and explicit AutoRGBNeutral from unique CAMF matrices. The
transform luminance scale is supplied explicitly, pending the final color
transform; test luminance 1.0 is not a production fallback. All eight real
SD10 curves admit finite bounded storage and release completely on drop;
zero-budget and cancellation checks pass. The reproducible gate passes
21 scoped tests with all previously qualified stages still matching.
Independent noise-bank values, chroma filtering and final color remain Pending.

### Native first Foveon hue-noise pass (2026-10-06)

A managed five-row channel ring now supports the first 3x3 hue-noise pass,
applying a supplied noise curve to local channel deviations and the legacy
shared adjustment. Two-pixel borders are preserved; dimensions, signed
source values, budget and cancellation are checked. Synthetic constants,
color impulses, unchanged borders and scratch release pass. Scoped tests:
21 passed, one real-source test ignored. Independent curve/whole-frame hue
comparison, remaining filters and final color/viewer remain Pending.

### Independently compared X3F noise-bank arithmetic (2026-10-06)

Every table length and signed sample of all eight SD10 noise curves now
exactly matches a separate C++ implementation, with explicit test luminance
1.0. The reproducible gate requires this oracle and all 22 scoped tests;
research/x3f-curves-gate-2026-10-06.json records source/helper/output hashes.
This qualifies noise-bank arithmetic for the supplied luminance, not the
actual final transform luminance. Independent hue-frame comparison, remaining
filters, crop, final transform and viewer integration remain Pending.

### Independently compared first X3F hue pass (2026-10-06)

Every one of the 10,582,272 SD10 channel values after the first hue-noise
pass exactly matches a separate C++ full-horizontal-buffer implementation.
This independently checks the native five-row ring and signed curve
application; temporary ring credit is released while noise curves remain
held, and final release returns budget usage to zero. The reproducible gate
passes 22 scoped tests: research/x3f-hue-gate-2026-10-06.json. The curve
luminance remains explicitly set to 1.0 for this stage qualification.
Further hue/color filters, actual transform luminance, crop and viewer remain Pending.

### Native second Foveon hue pass (2026-10-06)

The second hue pass uses 5x5 channel estimates and total-signal normalization
with a supplied noise curve in a managed five-row ring. Integer products,
source/frame range, budget and cancellation are bounded. Synthetic constant
and color impulse behavior, unchanged borders and complete scratch release
pass. Scoped tests: 22 passed, one real-source test ignored. Independent
full-frame second-pass comparison, final transform, further color smoothing,
crop and viewer integration remain Pending.

### Independently compared second X3F hue pass (2026-10-06)

All 10,582,272 SD10 camera-channel values after the second 5x5 hue pass
exactly match a separate C++ full-buffer reference. Native ring storage
and fixed-point total normalization are checked; final scratch and curve
leases release to zero. The reproducible gate passes all 23 scoped tests:
research/x3f-wide-hue-gate-2026-10-06.json. Qualification still uses explicit
test transform luminance 1.0. Actual color-transform derivation, subsequent
color smoothing, crop and public viewer integration remain Pending.

### Native X3F transform and derived luminance (2026-10-06)

Auto color-transform construction now resolves explicit illuminant/WB and
AutoRGBNeutral matrices, combines an explicitly supplied target matrix,
normalizes channel row response and derives the luminance for noise curves.
Missing/nonfinite/degenerate calibration and target rows are refused. Real
SD10 construction and derived-luminance curve admission/release pass; the
identity target is a test input, not a production fallback. All 23 scoped
tests pass with previously qualified stages unchanged. Independent transform
arithmetic, actual display-target convention, complete derived-luminance
filtering and final image/viewer qualification remain Pending.

### Independent X3F transform arithmetic (2026-10-06)

The separate test-only C++ transform reference exactly matches all nine
native float32 coefficients and the float64 derived luminance for the
explicit identity target on the pinned SD10 file. The reproducible gate
passes 23 tests, zero failed or ignored:
research/x3f-transform-arithmetic-gate-2026-10-06.json. Existing full-frame
sensor correction and both hue-pass comparisons also remain exact.
This establishes transform arithmetic only: actual display-target convention,
derived-luminance full filtering, pixel color transformation, subsequent
chroma smoothing, crop and viewer integration remain Pending. X3F remains
Pending in the 100-format catalog; the external oracle is not a runtime
dependency.

### Real X3F cancellation after processing allocation (2026-10-06)

An application generation-cancellation test now starts actual SD10 display
loading in a worker, waits until root usage exceeds source+unpacked sensor
storage (observed 50,484,616 bytes), then supersedes its generation. The
in-flight load returns the public Cancelled error and releases all leases
to zero. The latest generation subsequently decodes/prepares the same
2267x1513 source successfully and also releases to zero. Opt-in test passes:
research/x3f-inflight-cancellation-2026-10-06.log. This proves cancellation
after real processing allocation, rather than only gate waiting; it does
not claim exhaustive cancellation coverage of every individual stage.
Whole app-to-GPU opening and wider format/hardware qualification remain
outstanding.

### Native X3F orientation (2026-10-06)

Legacy container decode now applies the header's clockwise 90/180/270 degree
orientation before RGBA raster transport, using managed RGB16 scratch.
Non-square synthetic tests cover all four rotations, swapped dimensions,
invalid angles, budget rejection and cancellation. A pinned SD10 fixture
with only its header rotation changed to 90 degrees exactly matches every
channel of the complete independent converter's 1513x2267 linear-sRGB PPM.
All 30 scoped tests pass without failures/skips:
research/x3f-rotation90-gate-2026-10-06.json. This supersedes the earlier
nonzero-rotation refusal. Real-source independent 180/270 comparisons,
queued mid-processing cancellation, complete application-to-GPU opening,
other X3F generations and full 100-format qualification remain outstanding.

### Real X3F queued-navigation cancellation (2026-10-06)

The foreground-loader generation/queue test now also uses copied real SD10
sources. A stale speculative preload waiting behind the foreground decode
gate is cancelled by newer navigation requests, leaves no cached entry or
managed leases, and only the latest request remains queued. That latest
generation then decodes and prepares the real 2267x1513 X3F successfully;
all leases release after cache/output drop. Opt-in test passes:
research/x3f-navigation-cancellation-2026-10-06.log. This proves cancellation
while waiting for the gate and subsequent recovery, not cancellation at every
in-flight processing stage. Mid-processing cancellation and the whole app
GPU path remain separate qualifications, as do rotated/other-generation X3F.

### Real X3F directional neighbour preload (2026-10-06)

The gallery extension candidate list now includes X3F for the supported
raster legacy decoder path. Five copied pinned SD10 sources exercise actual
application preload with previous=1, next=2 in both navigation directions.
All planned neighbours are accepted, prefetched frames share managed float
storage with subsequent RAM hits, a two-entry cache preserves the visible
frame, and dropping frames/cache releases all root memory leases.
The opt-in test passes: research/x3f-prefetch-2026-10-06.log.
This uses synchronous preload through the existing planning/loading helpers;
queued navigation cancellation and whole application-to-GPU opening are
separate outstanding checks. Rotated X3F and wider camera generations
remain incomplete.

### Real X3F application RAM and swap round-trip (2026-10-06)

The SD10 source now runs through application display preparation and
`load_cached_raster_mode`. Its second opening is a RAM hit with a closure
that panics if decoding occurs. Lowering entry count to zero spills the
unprotected frame; the writer completes one swap write and RAM leases
release to zero. Restore performs one swap read with decoding forbidden,
and the entire serialized raster payload exactly matches before eviction.
Dropping restored output/cache releases all root-budget leases. The opt-in
real-source test passes: research/x3f-app-ram-swap-2026-10-06.log.
Full-frame limits used are 128 MiB swap, 64 MiB queue/restore, count 4 and
60s swap TTL. An initial mini-fixture configuration refused the oversized
frame as expected; test configuration was adjusted, not admission policy.
X3F prefetch, rotated images and full app-to-GPU opening remain unqualified.

### X3F application dispatch and memory-pressure retry (2026-10-06)

Content detection now identifies FOVb as Raster, enabling the existing
application raster loading/cache/prefetch branch to reach native X3F decode.
The real-source public-opening test verifies this classification; all 29
scoped checks pass: research/x3f-dispatch-gate-2026-10-06.json.
`RasterLoadError` now recognizes typed X3F memory errors for the existing
memory-pressure retry policy. Application compilation and its focused
raster retry/visible-retention regression pass:
research/x3f-app-retry-tests-2026-10-06.log. Actual X3F cache/prefetch/swap
round-trip and end-to-end application GPU opening still need qualification;
nonzero rotations and broader X3F variants remain incomplete.

### Public legacy X3F raster opening (2026-10-06)

The public raster decoder now routes FOVb content to native legacy Auto
processing, using request cancellation, caller memory budget (bounded default
when absent) and a typed X3F error. Image selection other than zero is refused.
Pinned SD10 opens through `decode_raster` as 2267x1513 LinearSrgb RGBA16,
and output drop releases usage to zero. All 29 scoped checks pass:
research/x3f-public-opening-gate-2026-10-06.json. Nonzero rotation is explicitly
refused pending oriented output; no incorrectly oriented image is returned.
Application format dispatch, RAW development/exposure policy, rotated files,
other camera generations and end-to-end viewer/cache/GPU opening still need
qualification. The 100-format objective remains incomplete.

### Native container-to-raster X3F Auto decode (2026-10-06)

`decode_x3f_legacy_auto` now composes bounded container selection, legacy
Huffman unpack, type-2 CAMF decoding, explicit Auto calibration, file-derived
active crop, legacy Foveon linear output and managed RGBA16 transport.
Duplicate/missing sensor or CAMF entries and unsupported generations fail
explicitly. On pinned SD10 the API returns 2267x1513 LinearSrgb raster whose
RGB values all exactly match the complete converter's linear sRGB PPM,
with opaque alpha and zero budget usage after drop. The gate passes 29
tests without failures/skips and includes the container-to-raster marker:
research/x3f-container-raster-gate-2026-10-06.json. This API is explicitly
Auto; ordinary decode/viewer routing, orientation/policy integration and
other camera/encoding variants remain incomplete.

### Full SD10 linear-sRGB fixture displayed on Metal (2026-10-06)

The physical Metal Apple M4 Max raster path uploads the independently
qualified SD10 2267x1513 linear-sRGB fixture and reads back the complete
display frame. At explicitly compensated 1:1 scale, every rendered channel
is within one uint8 value of CPU sRGB transfer and alpha is 255 throughout.
CPU raster/float leases release to zero. The ignored opt-in test requires
the fixture and GPU and refuses silent adapter skips:
qualified_sd10_linear_raster_full_frame_display_readback. It passed in
research/x3f-gpu-readback-2026-10-06.log. Initial uncompensated fit-to-window
comparison failed because the renderer reserves 32 viewport pixels; this
was corrected in the test's zoom, without changing the shader or tolerances.
This tests qualified fixture transport/rendering, not an end-to-end ordinary
X3F viewer opening. CUDA/NVIDIA/HDR hardware qualification remains separate.

### Real SD10 linear raster transport equality (2026-10-06)

The independently matched SD10 linear-sRGB output now passes through the
managed RGBA16 adapter and the existing core linear-float preparation path.
Every RGB float equals its original uint16 channel divided by 65535, every
alpha is exactly 1, and all transport allocations release to zero. The
complete real-source gate passes 29 tests without failures or skips:
research/x3f-linear-raster-gate-2026-10-06.json. This verifies CPU transport
without double transfer decoding; actual GPU upload/readback and ordinary
X3F viewer routing remain Pending.

### X3F linear raster transport (2026-10-06)

`x3f_linear_raster` now packs qualified linear-sRGB RGB16 channels into
managed RGBA16 with opaque alpha and explicit LinearSrgb metadata. This
preserves precision and uses the existing raster display/thumbnail color
path rather than applying another transfer function in the decoder. Focused
value/alpha/metadata, dimension, budget rejection and lease-release tests
pass. This creates the transport adapter only; real-source CPU/GPU display
qualification and ordinary X3F viewer routing remain unfinished.

### Native SD10 final linear sRGB matrix output (2026-10-06)

`x3f_linear_output` applies an explicit final matrix in float32, truncates
and clamps to uint16 using a managed output lease and bounded cancellation.
Synthetic truncation/clamp and invalid-input release pass. All 10,289,913
native final linear sRGB channels exactly match full dcraw `-4 -W -o 1`
PPM when the reference is compiled with `-ffp-contract=off`, consistent with
the prior arithmetic oracle configuration. A reference compiled with default
contraction first exposed a one-unit difference; it was not accepted as exact.
The complete gate passes 28 tests without failures/skips:
research/x3f-linear-srgb-gate-2026-10-06.json. Screen transfer encoding,
exposure policy and viewer/GPU integration remain incomplete.

### Native SD10 linear output matches independent PPM (2026-10-06)

Direct comparison of the native 2267x1513 cropped frame with the complete
converter's 16-bit big-endian PPM (`-4 -W -o 0`) exactly matches all
10,289,913 channel values. This explicitly selected linear/no-auto-bright
configuration does not remap exposure or transfer after the crop, independently
verified in research/x3f-linear-output-identity-2026-10-06.json. The native
optional PPM assertion and all existing checks pass:
research/x3f-linear-ppm-gate-2026-10-06.json; 27 tests without failures/skips.
Screen sRGB conversion, exposure controls and GPU/viewer integration are
still separate unfinished requirements; linear output equality alone does
not qualify display color or other X3F cameras.

### Full-converter SD10 equality and crop gate passed (2026-10-06)

The repeat promotion-fix gate passes all 27 tests and releases sensor leases
to zero: research/x3f-filter-promotion-gate-2026-10-06.json. A subsequent
strict comparison against a standalone converter dump taken after its own
crop exactly matches all 10,289,913 channels in the native 2267x1513 crop
[18,8,2285,1521]. That gate also passes 27 tests without failures or skips:
research/x3f-full-crop-gate-2026-10-06.json. External instrumented source
and cropped artifact hashes are in research/x3f-full-crop-artifacts-2026-10-06.json.
This proves the pinned SD10 linear channel arithmetic and crop, not final
display exposure/transfer encoding, public viewer integration or wider
X3F variants. The optional full-converter fixtures must be supplied explicitly
for these additional assertions; default separate-stage checks alone do not
establish these stronger results.

### Full SD10 converter arithmetic equality after promotion fix (2026-10-06)

The drift expression's 0.5 constant promotes the remaining subtraction to
double. Native correction and the separate C++ reference now preserve that
promotion rather than prematurely rounding to float32. The full converter
comparison reports zero corrected-sensor mismatches and zero final-frame
mismatches across all 10,582,272 values. The first run reached those markers
but failed the final sensor-budget assertion because the test's explicit
stage-input alias retained its lease. That alias is now explicitly dropped.
A repeat qualification run is in progress; do not claim a fully passing
gate until its terminal result is inspected. Display encoding, final crop
integration, public viewer and other X3F variants still remain incomplete.

### X3F gain/drift precision correction (2026-10-06)

Native vertical gain coordinates and dark-drift interpolation now follow
double intermediate evaluation before float32 storage. Weighted gain remains
float32, but signal multiplication, spacing division and neutral division
are evaluated in double before floor, matching the full C converter's
promotion caused by floor(). Test-only C++ references also explicitly use
double floor to avoid the C++ float overload changing the reference semantics.
Full-converter corrected-sensor differences dropped from 450 to 27 values,
and final-frame differences from 3846 to 117; respective maximum differences
remain 4 and 10. Exact equality still fails:
research/x3f-gain-drift-precision-gate-2026-10-06.json (26 passed, one failed).
The remaining first-stage differences still require investigation; no
tolerance was introduced.

### Remaining full-converter X3F divergence precedes bad-pixel repair (2026-10-06)

The independent full converter now also dumps its corrected sensor frame
immediately before bad-pixel repair. Native correction on unshifted samples
differs in 450 channel values, maximum absolute difference 4. The later
full-frame output still differs in 3846 values, maximum 10. Thus the next
required investigation is sensor/black/gain arithmetic rather than display
encoding or crop. Diagnostic output is retained in
research/x3f-full-converter-corrected-diagnostic-2026-10-06.json; the strict
whole-frame test intentionally still fails. No comparison tolerance was
introduced and X3F remains Pending.

### Native calibration offset removal reduces full-converter divergence (2026-10-06)

`process_auto` now restores the signed predictor samples before camera
calibration through a managed modular-subtraction buffer; unpacked data and
its reported offset remain intact. Zero-offset inputs share their existing
lease, and normalized scratch is dropped after sensor correction. Separate
arithmetic fixtures explicitly mark their deliberately shifted input as
offset-free; real full-converter comparisons retain the real unpack offset.
The strict full-converter comparison improved from 10,520,375 mismatches,
maximum 261, to 3,846 mismatches, maximum 10. It still fails exact equality:
research/x3f-unshifted-full-converter-gate-2026-10-06.json (26 passed, one
failed). Remaining stage precision differences must be localized; no
tolerance or qualification status was relaxed.

### Full-converter divergence located at sensor offset (2026-10-06)

Dumping the complete dcraw converter immediately before CAMF processing
shows that all sensor values differ from LibRaw/native unpack by the known
legacy offset 202 modulo uint16. Removing that offset modulo uint16 gives
exact equality for every one of 10,582,272 samples, including negative
predictor values. Evidence:
research/x3f-full-converter-sensor-comparison-2026-10-06.json. The unpack
contract may retain its explicit offset, but calibration must consume the
unshifted signed predictor values. This identifies a required production
correction; the full-converter assertion remains failing until normalization
and corresponding independent stage references are updated and rerun.

### Full converter pixel comparison exposes unresolved X3F difference (2026-10-06)

The standalone dcraw converter was instrumented only to dump signed channel
values immediately before its crop. Against native managed legacy-target
processing it differs in 10,520,375 of 10,582,272 values, maximum absolute
difference 261. The new strict optional full-converter assertion fails:
research/x3f-full-converter-gate-2026-10-06.json (26 passed, one failed).
Source/instrumented-source/output hashes are retained in
research/x3f-full-converter-evidence-2026-10-06.json. Prior separate-stage
equality does not establish whole-converter correctness. The next required
step is identifying the first divergence using full-converter stage dumps;
X3F stays Pending and is not routed into the viewer.

### SD10 full converter crop observed (2026-10-06)

The independent dcraw source compiled as a standalone test-only converter
and processed the original pinned SD10 with `-4 -W -o 0 -c`. Its actual
16-bit PPM header is 2267x1513, and payload size exactly matches that frame.
This confirms the legacy CAMF crop formula's dimensions on this file, despite
the identify command advertising the uncropped sensor before processing.
Source/output hashes and exact build/processing options are recorded in
research/x3f-full-converter-crop-2026-10-06.json. Native crop rectangle
[18,8,2285,1521] gives those dimensions. Independent full-converter pixel,
exposure/transfer and final color equality still need comparison; this is
not a claim of complete X3F qualification or viewer integration.

### Full legacy-target X3F frame comparison (2026-10-06)

All 10,582,272 channel values from managed native `process_auto` using the
explicit legacy Foveon target and its derived luminance exactly match the
separate C++ post-highlight filter chain. The gate hashes each legacy-target
intermediate and requires exact full-frame equality and output-lease release:
research/x3f-legacy-frame-gate-2026-10-06.json; 27 tests passed without
failures or skips. This extends arithmetic evidence beyond identity target.
It still does not prove agreement with a full independent converter's final
display image, exposure/transfer encoding or crop policy. Viewer integration
and broader X3F variants remain Pending.

### Explicit legacy Foveon target convention (2026-10-06)

Inspection of the independent dcraw reference establishes that Foveon
identification selects `simple_coeff(0)` before interpolation; its target is
[[1.4032,-0.2231,-0.1016],[-0.5263,1.4816,0.017],[-0.0112,0.0183,0.9113]],
not identity. The test-only transform oracle now accepts this explicit
reference convention. Native transform coefficients and luminance exactly
match it on SD10. The reproducible gate requires this additional marker
and passes 27 tests without failures/skips:
research/x3f-legacy-target-gate-2026-10-06.json. These coefficients are only
explicit test inputs; production target selection is still caller-owned.
Prior identity-target frame comparisons establish arithmetic, not display
color. Full legacy-target frame/converter qualification, output transfer,
crop policy and viewer remain incomplete.

### X3F complete processing failure-path memory evidence (2026-10-06)

Real SD10 complete processing now explicitly checks budgets 0, one signed
frame, and one signed frame plus 32768 bytes. Each rejected admission returns
the typed memory error and releases usage to zero. Cancellation is triggered
after observing an actual allocated lease, rather than only before work;
it returns Cancelled and releases usage to zero. The qualification script
requires this marker alongside full-frame equality:
research/x3f-managed-failures-gate-2026-10-06.json; 27 tests passed without
failures or skips. Successful frame output and previous stage comparisons
remain exact. Display color/crop policy, viewer/cache integration and wider
format/hardware qualification remain incomplete.

### Managed complete legacy X3F processing (2026-10-06)

`X3fLegacyChannels::process_auto` now composes sensor correction, bad-pixel
repair, red sharpening, highlights, derived-luminance curves, both hue passes,
pixel transform, final chroma smoothing and an explicit crop into one API.
All intermediate image/scratch buffers use the caller's memory budget;
the full-frame case freezes scratch directly without a crop copy. Target
matrix and half-open crop remain explicit caller decisions. On SD10 the
managed full-frame output exactly equals the independently compared chain,
only its output lease remains after return, and dropping it releases usage
to zero. Initial cancellation also releases to zero. The reproducible gate
requires the new marker and passes 27 tests without failures or skips:
research/x3f-managed-processing-gate-2026-10-06.json. Output still uses
legacy linear transform scale; display encoding/color, crop convention,
public viewer and other X3F variants remain Pending.

### SD10 crop policy discrepancy verified (2026-10-06)

An enabled isolated LibRaw 0.22.2 unpack run reports active 2266x1510,
origin [20,8], on the 2304x1531 sensor. Its source camera-size adjustment
table explicitly specifies these dimensions; this is a separate camera
crop policy rather than proof of CAMF endpoint semantics. The legacy CAMF
reference formula gives half-open [18,8,2285,1521], output 2267x1513,
while the container advertises 2268x1512. Evidence and pinned source hash:
research/x3f-sd10-crop-policies-2026-10-06.json. No production crop was
silently forced to any of these differing policies. Final crop policy and
display color require explicit full-converter qualification before viewer
integration; native rectangular extraction itself is tested.

### Managed X3F crop primitive (2026-10-06)

`crop_x3f_channels` now copies an explicit half-open active rectangle into
a managed channel buffer, validates source dimensions and rectangle bounds,
and checks cancellation every 256 pixels. Output budget rejection and
cancellation release the allocation; retained clones retain its lease.
Exact synthetic row selection, invalid rectangles, insufficient budget and
cancellation pass: 26 scoped tests passed, one real-file test ignored;
research/x3f-crop-tests-2026-10-06.log. SD10 CAMF reports KeepImageArea
[0,4,2303,1534] and ActiveImageArea [18,12,2285,1523]. Their endpoint
convention and final crop must still be independently qualified against
the container's advertised 2268x1512 dimensions before viewer routing.

### Full derived-luminance X3F filtering comparison (2026-10-06)

The complete native post-highlight sequence (first hue pass, wide hue pass,
pixel color transform and final chroma pass) now runs with curves built from
the transform's calculated luminance. All 10,582,272 final SD10 channel
values exactly match independently executed C++ stages. The gate hashes
all derived intermediates, requires the full-frame equality marker and
passes 26 tests without failures or skips:
research/x3f-derived-frame-gate-2026-10-06.json. Scratch and curve leases
release to zero. The target is still explicitly identity for arithmetic
qualification; actual display-target/color convention, crop, public viewer
and other X3F variants remain Pending.

### Independent derived-luminance X3F noise bank (2026-10-06)

All eight native noise-curve lengths and samples now exactly match the
separate C++ curve reference using luminance read from the independently
computed transform, rather than fixed test luminance 1.0. The target remains
explicit identity. The reproducible gate requires this additional equality
and hashes the derived bank: research/x3f-derived-curves-gate-2026-10-06.json;
26 tests passed without failures or skips. Full-frame filtering using this
derived bank, actual display-target convention, crop and viewer remain Pending.

### Independent final X3F chroma arithmetic (2026-10-06)

The SD10 quarter guide and all 10,582,272 final chroma channel values exactly
match a separate C++ reference that stores full intermediate filter frames,
independent of the native three-row scratch layout. The complete reproducible
gate passes 26 tests without failures or skips, hashes guide and full-frame
outputs and requires exact equality markers:
research/x3f-chroma-gate-2026-10-06.json. Target remains explicitly identity
and curve luminance explicitly 1.0; display color is not qualified. Actual
target convention, full derived-luminance pipeline, crop, viewer and broader
X3F variants remain Pending.

### Native final X3F chroma smoothing (2026-10-06)

`smooth_x3f_chroma` now uses the managed quarter-scale guide, reverse and
forward horizontal recurrences, vertical recurrence and three supplied
chroma curves. Int64 intermediates avoid legacy fixed-point overflow;
only complete four-pixel blocks are adjusted. Three managed scratch rows
are admitted before frame mutation. Cancellation checks run per row and
every 256 columns, and late cancellation invalidates the scratch frame.
Zero-frame behavior, trailing borders, insufficient scratch budget and
cancellation/release pass. Scoped tests: 25 passed, one ignored, zero failed;
research/x3f-chroma-tests-2026-10-06.log. Independent nonzero full-frame
comparison, derived-luminance full pipeline, actual display-target convention,
crop and viewer remain Pending.

### Native X3F quarter-scale chroma guide (2026-10-06)

The next color smoothing stage now has a native bottom-up quarter-resolution
guide: each complete 4x4 block is accumulated in int64, the last guide row
is averaged and earlier rows use the specified 1840/141 fixed-point recurrence.
Trailing incomplete blocks are excluded. Input range, dimensions, allocation
budget and cancellation are checked; retained output clones keep their lease.
Synthetic two-row recurrence, trailing borders, insufficient budget, invalid
input and cancellation/release pass. Scoped results: 24 passed, one ignored,
zero failed; research/x3f-chroma-guide-tests-2026-10-06.log.
Independent full-frame guide comparison and subsequent horizontal/vertical
chroma adjustment, final crop and viewer remain Pending.

### Native X3F pixel color transformation (2026-10-06)

`transform_x3f_pixels` applies the three explicit channel noise curves,
weighted luminance correction and supplied color matrix in place. Products
use float32 with float64 accumulation, rounding and output clamp 0..24000.
Input range and finite bounded coefficients are validated before mutation;
cancellation is checked every 256 pixels. Late cancellation invalidates the
scratch frame. No additional image allocation is needed. Scoped tests pass:
23 passed, one real-file case ignored, zero failed; log:
research/x3f-pixel-transform-tests-2026-10-06.log. Independent full-frame
pixel-transform comparison, actual display-target convention, subsequent
chroma smoothing, crop and public viewer integration remain Pending.

### Independent full-frame X3F pixel transform (2026-10-06)

The pinned SD10 pixel transform now exactly matches a separate test-only
C++ reference for all 10,582,272 output channel values. Input is the independently
compared second hue-pass frame; target is explicitly identity and the supplied
curve bank explicitly uses luminance 1.0. This is arithmetic qualification,
not qualification of display color or a complete derived-luminance pipeline.
The reproducible gate includes the new oracle, hashes its complete output,
requires the exact-comparison marker and passes 24 tests without failures
or skipped tests: research/x3f-pixel-transform-gate-2026-10-06.json.
Actual target convention, full derived-luminance filtering, subsequent chroma
smoothing, final crop and public viewer remain Pending.

### SD10 native raster to completed Metal frame (2026-10-06)

The existing `raw_view_timing` application example now accepts `--raster-view`.
On Apple M4 Max / Metal, the pinned Sigma SD10 X3F completed native public
raster decoding, linear display preparation, GPU upload and an offscreen
1920x1080 submitted frame in p50 932.374 ms / p95 942.220 ms (15 measured
iterations after 3 warmups, optimized development profile). Managed CPU peak
was 113,978,248 bytes and GPU peak 109,759,072 bytes; both returned to zero.
Evidence: `research/x3f-raster-metal-timing-2026-10-06.log`.
This measures completed offscreen rendering, not UI navigation latency,
physical display/HDR or pixel readback. It qualifies this SD10 fixture only.

### Reproducible independent SD10 complete-output gate (2026-10-06)

`qualify-x3f-sd10.py --dcraw-source` now verifies a pinned standalone
converter source hash, builds it with floating-point contraction disabled,
generates complete linear sRGB PPM oracles for the original SD10 and header
rotation 90, and hashes both outputs in the report. It clears inherited
`RRRAH_X3F_*` variables before supplying its own generated fixtures.
Full native container output, rotation and public raster opening markers are
mandatory in this mode; PPM comparisons check complete payload lengths as well
as every channel. The gate passed 30 tests with zero failures or ignored tests:
`research/x3f-reproducible-full-output-2026-10-06.json`. External converters
remain test-only. Other X3F cameras, encodings and white-balance modes remain
unqualified; this evidence does not establish physical HDR display correctness.

### All SD10 header rotations independently checked (2026-10-06)

The reproducible complete-output gate now generates and hashes separate
independent PPMs for 90, 180 and 270 degree X3F header rotations. Native
container decoding matches every RGB16 channel in each complete image;
alpha remains opaque, exact dimensions/payload lengths are required, and
managed leases return to zero after each orientation. All three equality
markers are mandatory. 30 tests passed with zero failures/ignored tests:
`research/x3f-all-rotations-2026-10-06.json`. This remains one pinned SD10
Auto-WB fixture, not universal X3F qualification. The catalog now accurately
records this implemented subset: 90 implemented rows, 10 Pending rows;
no row is claimed universally qualified.

### Current independent cache policies and application regressions (2026-10-06)

Current-source review confirms independent CLI byte/count/TTL settings for
RAW RAM, prepared raster RAM, model RAM, persistent disk and temporary swaps;
swap write queues and live restore memory have separate budgets. Directional
preload windows have independent behind/ahead counts. Corrected CLI help to
describe files rather than RAW alone: real raster preload also uses this window.
The cache crate passed 114 unit and 5 invariant tests. The actual PICT
application decode/preload test independently exercises count and byte
admission, then zero-TTL expiry, while retaining the visible frame and
releasing the managed root budget. The complete ordinary application test
run passed 143 tests, with 18 external-fixture/hardware tests ignored; those
ignored tests are not completion evidence. Logs: `research/cache-current-limits-2026-10-06.log`,
`research/app-prefetch-limits-2026-10-06.log`,
`research/app-current-tests-2026-10-06.log`. These checks do not qualify
every format, device/backend or live window behavior.

### Metal compute full-resolution and live HDR presentation (2026-10-06)

Four current GPU readback binaries passed 13 ordinary tests on explicitly
selected Metal / Apple M4 Max, with 4 external/full-resolution checks ignored.
They exercise exposure compute, RGBE compute, signed/HDR float RAW targets
and raster textures/readback. The full managed exposure check was then run
explicitly: all 25,494,560 pixels exactly matched the authored CPU formula,
including HDR/negative channels and unchanged alpha. End-to-end execution
was 251.979 ms in one run (not a statistical benchmark); tracked GPU peak
1,223,738,992 bytes and CPU peak 815,825,920 bytes, both released to zero.

The existing main-thread `hdr_surface_probe` now completed a real window
surface submission and presentation using RGBA16Float / ExtendedSrgbLinear,
with no validation errors. Display API headroom was current 1.0 / potential
16.0 and coarse HDR false, luminance/chromaticity unavailable: successful
presentation does not prove physical highlight brightness or color accuracy.
This supersedes the earlier occluded probe as current presentation evidence.
Logs: `research/metal-hdr-compute-current-2026-10-06.log`,
`research/metal-full-managed-compute-2026-10-06.log`,
`research/hdr-surface-presented-2026-10-06.log`. CUDA/NVIDIA execution, physical
HDR measurements and compute integration into the interactive viewer remain open.

### Current XCF public composition independently requalified (2026-10-06)

Current authoritative source already contains public XCF raster routing and
legacy normal-layer flattening, superseding early Pending notes. Rebuilt the
existing native flatten example and verified its output against the existing
pinned independent Python parser and Pillow alpha-composition gate. All 12
complete compositions match exact pixels/dimensions and release managed
ownership; dependency versions, oracle source hashes and input hashes were
verified. The current XCF library suite passed 39 tests with one ignored
external source check. Evidence: `research/xcf-current-full-flatten-2026-10-06.json`
and `research/xcf-current-library-tests-2026-10-06.log`. Scope is explicitly
bounded to the gate's legacy normal RGB/RGBA/gray/gray-alpha and one opaque
v1 indexed case, binary mask endpoints and one half-opacity endpoint-color
fixture. This is not a GIMP rendered-presentation oracle and does not qualify
groups, general fractional masks/opacity, indexed-alpha, modern blending or
physical display/color accuracy. Catalog row 30 now records this exact evidence.

### SD14 sensor and IMA2 public-routing correction (2026-10-06)

Pinned CC0 raw.pixls.us object 4462 was downloaded and its advertised SHA-256
verified. Native legacy Huffman decoding matches all 14,450,688 independent
LibRaw 0.22.2 X3FTOOLS-enabled sensor values (2688x1792, offset 70). Type-2
CAMF decodes 39,220 bytes, but AutoRGBNeutral is not enabled. Investigation
found the public selector only admitted IMAG records despite the existing
image parser supporting IMA2; SD14 uses IMA2 for sensor and previews. Public
sensor selection now admits both and still requires type 3. Missing Auto
neutral produces UnsupportedAutoWhiteBalance instead of a generic malformed
error; the SD14 regression proves exact unpack and no retained lease after
this refusal. Full SD14 WB/color/viewing remains unsupported.

Evidence: `research/x3f-sd14-sensor-2026-10-06.json` and
`research/x3f-sd14-native-unpack-2026-10-06.log`. The SD10 reproducible full
output and all-rotation gate passed 30 tests after this change:
`research/x3f-sd10-after-ima2-2026-10-06.json`. Its camera-specific skip of
the separate SD14 external test is explicitly recorded, not claimed as SD14
qualification. Production remains independent of LibRaw.

### Native calibrated X3F neutral resolver (2026-10-06)

Added public `x3f_neutral_response` for explicit WB modes. Enabled embedded
RGBNeutral is preserved exactly; otherwise unique linked WB correction and
camera-to-XYZ matrices are combined, cofactor/D65 response computed with
legacy float32/double promotion semantics, and finite positive responses
required. Unknown links, singular matrices, malformed mode names and
cancellation fail; no identity/unity substitution. This resolver does not
yet change public SD14 processing admission.

All eight SD14 modes (Auto, Custom, Sunlight, Shade, Overcast, Incandescent,
Fluorescent, Flash) match independent C++ arithmetic bit-for-bit. Reference
source, compiler flags, binary, CAMF and response hashes are recorded in
`research/x3f-sd14-neutral-2026-10-06.json`, with runtime log alongside.
SD10 embedded-neutral preservation and full independent output/all-rotation
gate passed 30 tests: `research/x3f-sd10-neutral-resolver-2026-10-06.json`.
Next required work is shared neutral propagation through sensor, highlights,
noise and color plus independent full SD14 processing qualification. Current
public SD14 refusal is intentional until these stages are qualified.

### Shared calibrated Auto neutral through processing stages (2026-10-06)

Sensor Auto correction, highlight linearization, noise curves and color
transform now call the same native calibrated neutral resolver instead of
requiring an enabled AutoRGBNeutral matrix independently at each stage.
Embedded SD10 neutral arithmetic is unchanged. The missing-neutral negative
regression now invalidates calibration links as well: absence of the optional
neutral alone legitimately uses calibrated derivation, while absent/ambiguous
calibration still fails. Public SD14 admission remains explicitly gated until
its complete processing/color is independently qualified.

SD10 complete output, rotation and stage gate passed 30 checks with zero
failures/ignored tests: `research/x3f-shared-neutral-stages-2026-10-06.json`.
The just-built test binary also passed the SD14 full sensor/eight WB-neutral
and public unsupported-calibration cleanup regression:
`research/x3f-sd14-shared-neutral-regression-2026-10-06.log`. This establishes
shared resolver wiring and preserves SD10 full-output correctness; it does
not yet prove full SD14 photographic color or viewer support.

### Complete SD14 explicit-Auto linear processing exact (2026-10-06)

The SD14 external regression now optionally compares every final linear RGB16
channel after native processing/crop/output transformation with an independent
complete converter PPM, with exact header and payload-length checks and final
managed processing usage zero. Original SD14 WB_DESC is Sunlight; comparing
that converter mode to the existing explicit-Auto API initially produced
13,473,224 differences. This mismatch was retained and not accepted/tolerated.
An independent fixture copy changes only the UTF-16 WB_DESC value to Auto with
null padding, preserving sensor/CAMF bytes and all container offsets. The
standalone reference was rebuilt with contraction disabled and run -4 -W -o1.
All 13,910,169 final RGB16 channels (2639x1757) now match exactly. Input/source,
reference and PPM hashes plus exact changed-byte offsets are recorded in
`research/x3f-sd14-auto-full-linear-2026-10-06.json`, with passing log alongside.
Original mode mismatch: `research/x3f-sd14-sunlight-vs-auto-mismatch-2026-10-06.log`.

This qualifies explicit Auto arithmetic on this SD14 source, not its original
Sunlight appearance or public viewer admission. Next required work is explicit
selected-WB propagation through all stages, strict WB_DESC resolution and a
complete Sunlight oracle comparison before enabling normal SD14 viewing.

### Complete selected-WB SD14 Sunlight pipeline (2026-10-06)

Added `X3fLegacyChannels::process_with_white_balance` with explicit mode
propagation through camera matrix, sensor correction, highlight neutral,
noise curves and subsequent color/chroma processing. Existing Auto API/stage
entrypoints delegate to the same implementation and preserve compatibility.
On unmodified pinned SD14 source, explicit Sunlight processing now matches
all 13,910,169 final linear RGB16 channels (2639x1757) of the independently
compiled standalone converter with -4 -W -o1; no tolerance and no source WB
edit. Managed processing leases release to zero. Report and log:
`research/x3f-sd14-sunlight-full-linear-2026-10-06.json`.

Rechecked explicit SD14 Auto against the previously qualified Auto fixture
oracle: exact full image (`research/x3f-sd14-selected-auto-regression-2026-10-06.log`).
SD10 complete output/all-rotation gate passed 30 tests:
`research/x3f-sd10-selected-wb-regression-2026-10-06.json`. Public file opening
still requires strict WB_DESC resolution and selected-mode integration; it
remains gated for SD14, so this is library processing evidence rather than
viewer admission or physical display qualification.

### Public SD14 selected-WB opening and Metal frame (2026-10-06)

Added `decode_x3f_legacy` and connected the public raster route to a unique,
bounded ASCII WB_DESC from validated UTF-16 properties, with no Auto default
for absent/ambiguous modes. Explicit Auto API remains available and now admits
calibrated neutral derivation. SD14 exposed valid two-byte zero alignment
padding after declared strings: parser now bounds strings to the declared
region and admits at most three zero padding bytes. Padding is not string data.

Public SD14 Sunlight output matches all 13,910,169 RGB16 channels and opaque
alpha against the independent original-source PPM. Missing/duplicate/non-ASCII
WB fails with zero managed peak before processing; unknown WB and cancellation
fail without fallback or retained memory. Evidence:
`research/x3f-sd14-public-sunlight-2026-10-06.json` and corresponding log.
SD10 full output/all rotations passed 30 checks after public routing changed:
`research/x3f-sd10-public-wb-regression-2026-10-06.json`.

Existing application `raw_view_timing --raster-view` completed actual Metal
Apple M4 Max decode/preparation/upload/offscreen frames for SD14: p50
1251.740 ms / p95 1869.842 ms, 15 measured iterations after 3 warmups, optimized
development profile. Tracked CPU peak 156,726,212 bytes, GPU peak 148,375,136
bytes, both released to zero. Configured CPU/GPU caps 512/256 MiB. Log:
`research/x3f-sd14-public-metal-timing-2026-10-06.log`. This is completed
offscreen-frame latency, not interactive window navigation or physical HDR
qualification. Other cameras/encodings and full outputs for additional WB modes
remain unqualified; SD14 cache/swap/preload-specific regressions remain next.

### SD14 actual application RAM/swap and neighbour windows (2026-10-06)

Generalized the existing external X3F application RAM/swap regression to
explicit expected output dimensions; default stays SD10. Queue and live
restore caps are explicitly 128 MiB so the larger 74,187,568-byte SD14
prepared float frame is admissible, with 128 MiB disk and independent count 4
and TTL 60s. This does not widen production defaults. SD14 2639x1757 public
decode/preparation gives strict color, RAM reopening cannot call the decoder,
count-pressure spill writes exactly once and restore reads exactly once
without decoding. Serialized full frame/metadata payload is identical,
all managed leases released. SD10 2267x1513 same-path regression also passed.

Existing actual application preload regression on five copied real SD14
files passed previous=1 / next=2 in forward and backward directions. Both
planner ordering and supported-route admission are checked, preload shares
the same managed pixel allocation with later RAM access, count remains 2
and the current visible frame is preserved. Managed root returns to zero.
This exercises synchronous application helpers, not live UI navigation.
Logs: `research/x3f-sd14-app-ram-swap-2026-10-06.log`,
`research/x3f-sd14-app-prefetch-2026-10-06.log`,
`research/x3f-sd10-app-swap-regression-2026-10-06.log`. Positive TTL elapsed
expiry, SD14 queued/in-flight cancellation and sustained latency/pressure
measurements are not established by these specific runs.

### X3F one entropy pass with exact offset scan (2026-10-06)

Removed the second complete Huffman traversal used when the raw signed
minimum is negative. The first pass retains each signed predictor's bit
pattern; a bounded cancellable final scan performs wrapping offset addition
and clamps after addition, exactly as the prior second pass. No new output
allocation. Authored boundary rows check signed wrapping, clamp order and
i16::MIN offset refusal with released leases. Prior two-pass code is retained
only as a test reference, not as a selectable production fallback.

Paired SD14 sensor benchmark alternated old/new order in one test binary,
2 warmup pairs and 9 measured pairs; every output was checked against all
14,450,688 independent LibRaw samples. Two-pass p50/p95 476.188/504.508 ms;
one-pass 247.275/261.501 ms, median ratio 1.926 for sensor unpack only.
Full public Sunlight output remains exact, malformed-WB cleanup passes and
managed root releases. Report/log: `research/x3f-sd14-one-pass-paired-2026-10-06.json`.
SD10 full output/all-rotation and boundary gate passed 30 tests:
`research/x3f-sd10-one-pass-2026-10-06.json`.

Separate completed Metal application frame after the change measured p50
1062.148 / p95 1212.993 ms (15 samples after 3 warmups), unchanged tracked
CPU/GPU peaks 156,726,212 / 148,375,136 bytes and zero final usage. The prior
whole-frame baseline was noisy (3361.345 / 12490.324 ms); no causal whole-view
speedup ratio is inferred. Logs: `research/x3f-sd14-before-one-pass-2026-10-06.log`
and `research/x3f-sd14-after-one-pass-2026-10-06.log`. Physical display, other
camera variants and universal performance qualification remain open.

### Reproducible combined SD10 + SD14 gate (2026-10-06)

Extended the existing `scripts/qualify-x3f-sd10.py` with `--sd14-source`
(requires `--dcraw-source`). It verifies both pinned source hashes before
execution, independently unpacks SD14 sensor/CAMF with enabled isolated
LibRaw 0.22.2, builds the test-only neutral arithmetic helper, generates all
eight WB responses and independently renders original Sunlight and explicit
Auto reference inputs. The Auto property edit is recorded and source/oracle
artifacts are hashed. Inherited X3F environment variables are cleared.

The gate requires exact sensor, all eight neutral, full-image, public Sunlight
opening and malformed-WB/cancellation cleanup markers; it runs the native
SD14 regression separately for Sunlight and Auto. A missing marker or any
failed native/reference command makes the overall report failed. Current
combined run passed 30 SD10 tests plus two SD14 camera test invocations, no
excluded camera checks. Helper hashes were verified against current source.
Report: `research/x3f-two-camera-reproducible-2026-10-06.json`.

Reproduce with the pinned external sources and enabled isolated LibRaw build:

```sh
python3 scripts/qualify-x3f-sd10.py /tmp/rrrah-sigma-sd10.x3f \
  --libraw-root /tmp/LibRaw-0.22.2 \
  --dcraw-source /tmp/rrrah-dcraw-foveon-reference.c \
  --sd14-source /tmp/rrrah-sigma-sd14.x3f \
  --target-dir /tmp/rrrah-required-corpus-target \
  --report /tmp/rrrah-two-camera-report.json
```

Temporary generated oracles are owned by the script and cleaned after run;
the report preserves their hashes and commands/output. This qualifies the
two pinned camera sources and stated modes, not other cameras, encodings,
SD14 rotation, full outputs for other WB modes or physical HDR. Cargo
manifest/lock inspection still finds no production LibRaw dependency.

### Reproducible complete-frame Metal SD10/SD14 readback (2026-10-06)

Generalized the existing qualified X3F raster readback test to explicit
expected dimensions (SD10 default preserved), with exact complete PPM
payload and readback lengths. SD14 2639x1757 on Metal Apple M4 Max checked
all 4,636,723 rendered pixels against CPU sRGB conversion of the independently
qualified linear source: maximum RGBA8 deviation 1, opaque alpha and zero
final managed CPU leases. Log: `research/x3f-sd14-full-gpu-readback-2026-10-06.log`.

The existing combined gate now accepts `--metal-readback` with
`--dcraw-source`, executes full-resolution physical Metal readback against
its freshly generated PPMs, requires the Metal adapter/exact successful
test markers, and records dimensions, all-pixel counts and maximum deviation.
Missing GPU or any readback failure fails the report; inherited GPU/X3F
selection variables cannot turn it into a skip or select another backend.
The combined run passed 30 SD10 checks, two SD14 CPU camera invocations and
both GPU invocations. SD10 3,429,971 and SD14 4,636,723 pixels each have maximum
code-value deviation 1. Current script and readback source hashes verified.
Report: `research/x3f-two-camera-metal-reproducible-2026-10-06.json`.

Reproduce using the preceding two-camera command with `--metal-readback`.
This proves qualified linear fixture transport to offscreen SDR sRGB GPU
pixels, composed with the separate exact native decoder equality. It is
not a live UI/event-loop check or physical HDR brightness/color measurement.

### BAY sensor foundation (2026-10-06)

Added native Rust `unpack_bay_sensor` with explicit Casio QV-2000UX,
QV-3000EX and QV-5700 layouts. Exact input lengths are required before a
managed allocation; cancellation before, during and after unpacking releases
the output lease. QV-5700 uses MSB-first 10-bit packing with 12 trailer bytes
per row; the two other layouts promote 8-bit samples without normalization.
Layout source: https://www.inweb.ch/foto/rawformat.html.

Authored fixtures exercise all sample values, row boundaries, nonzero row
trailers, length rejection, allocation refusal, cancellation and shared
buffer ownership. Camera identification, CFA, black/white levels, WB, crop,
color and viewer routing remain pending. Synthetic tests cannot qualify a
real camera file; BAY therefore remains Pending in the 100-format catalog.
Focused `cargo test --locked -p rrrah-decode --lib bay::` passed both tests
with zero ignored cases. Log: `research/bay-sensor-tests-2026-10-06.log`.

The external LibRaw 0.22.2 unpack comparison distinguishes QV-5700's 2585
storage columns from its 2576-column active region (crop `[0,0,2576,1924]`).
The explicit native layout returns those 2576 columns; it does not return
the nine additional samples LibRaw extracts from trailer bytes. Other two
layouts compare every storage sample. `scripts/qualify-bay-sensor.py` checks
exact oracle dimensions, complete payload lengths and all values in the
declared region, recording excluded columns explicitly; no real-camera,
trailer semantics or color qualification is claimed.
All three synthetic cases passed: 10,150,336 sample values exactly equal,
with managed output leases returning to zero. Report:
`research/bay-synthetic-libraw-2026-10-06.json`. Build the existing
`raw-fixture-oracle.cpp` with external LibRaw and the `bay_sensor_dump`
Cargo example, then run the gate with `--oracle`, `--native` and `--report`.

### BAY full storage follow-up (2026-10-06)

Supersedes the preceding QV-5700 active-region-only implementation:
`dimensions()` now returns 2585x1924, and native unpack retains all nine
additional storage columns. Its final 10-bit sample comes from the high ten
bits of the last two row bytes; the six padding bits cannot alter it.
The display active area remains a separate, unimplemented metadata contract.
The differential gate now compares complete storage buffers for all three
layouts, without excluded columns; synthetic input exercises the extra
columns and nonzero final padding. Real-camera/color qualification is still
pending and BAY is not yet routed to the viewer.
Both focused native tests passed, including storage-column and padding
boundaries (`research/bay-full-storage-tests-2026-10-06.log`). Independent
LibRaw 0.22.2 unpack equality passed all 10,167,652 storage samples across
the three layouts, with zero excluded columns and zero final output leases:
`research/bay-full-storage-libraw-2026-10-06.json`.

### Swap TTL maintenance under a busy command stream (2026-10-06)

The swap worker previously pruned expiry only after a one-second receive
timeout. Repeated barrier commands reset that timeout without touching the
LRU, so expired entries could remain indefinitely. Maintenance now uses a
monotonic deadline independent of command arrival and checks expiry before
processing the next command once that deadline passes. The one-second
maintenance cadence and live-handle retention contract are preserved.

A regression continuously submits barriers every 10 ms and observes physical
store usage without any lookup or explicit pruning. The expired file, object
quota and source leases release despite the busy queue, with zero swap errors.
The entire cache suite passed: 115 unit tests and five invariant integration
tests, zero ignored. Log: `research/swap-ttl-busy-full-2026-10-06.log`.
This verifies cache behavior, not the still-incomplete format/GPU/HDR goal.

### Nested restore budget accounting (2026-10-06)

Rebinding a swap restore budget to its own existing budget created a nested
generation. Summing both generations double-counted new restored owners:
three live bytes incorrectly became five and prevented a fourth byte under
a four-byte cap. The new regression failed with
`Capacity { requested: 1, used: 5, limit: 4 }` before the fix.

`MemoryBudget::is_descendant_of` exposes immutable accounting ancestry.
Swap restore occupancy now sums only outermost budgets, deduplicating equal
handles, while retaining independent old roots and serialized admission.
The regression reaches exactly four live bytes, refuses another byte,
permits retry after releasing one owner and ends with zero managed usage.
Existing independent-generation and concurrent restore checks also pass.

Full checks passed: 116 cache unit tests, five cache invariants, 38 memory
unit tests and the memory integration/doc checks, with zero ignored tests.
Additional ancestry assertions cover same-handle clones, nested ancestors,
siblings, unrelated roots and reversed relationships in the existing memory
pressure test. Logs: `research/swap-nested-budget-before-2026-10-06.log`,
`research/swap-nested-budget-after-2026-10-06.log` and
`research/budget-ancestry-2026-10-06.log`.

### Owned CUDA exposure foundation (2026-10-06)

Added workspace crate `rrrah-cuda`, separate from wgpu backend/vendor
selection. It contains an owned PTX scene-linear exposure kernel and a
dynamic 64-bit Linux/Windows CUDA Driver API boundary. Signed/HDR RGB is
scaled by a finite checked gain; alpha is copied unchanged. CPU output and
device input/output use independent managed budgets, with pre-allocation
admission, synchronization before result publication and context-owned
device cleanup. Unproven cleanup retains credit conservatively; unavailable
CUDA reports an explicit error, never a CPU/Metal/wgpu fallback.

Local native tests: two passed, one NVIDIA hardware qualification ignored.
CUDA crate Clippy with `--all-targets --no-deps` passed without warnings;
Windows x86_64 GNU cross-check with `--all-targets` passed. Logs:
`research/cuda-foundation-current-2026-10-06.log`,
`research/cuda-clippy-current-2026-10-06.log`, and
`research/cuda-windows-current-2026-10-06.log`.

The required hardware test compares every output channel for seven sizes
around CUDA block boundaries and three exposures, tests alpha preservation,
last-owner release, independent GPU admission and cancellation. It has not
run on this macOS host; Rust tests/cross-checks do not assemble/JIT PTX or
prove NVIDIA execution. Viewer integration, graphics interop, persistent
device resources, hardware cleanup qualification and throughput/latency
remain open. Per-call context/JIT setup is an explicit optimization limit.
Reproduction and primary references are in `crates/rrrah-cuda/README.md`.

### NVIDIA compiler acceptance for the owned CUDA kernel (2026-10-06)

Used official NVIDIA ptxas 12.8.93 from the CUDA 12.8.1 redistributable
manifest. The linux-sbsa compiler archive SHA256 was independently verified
as `dc0b713ce69fd921aa53ac68610717d126fc273a3c554b0465cf44d7e379f467`
before extraction. Source manifest:
https://developer.download.nvidia.com/compute/cuda/redist/redistrib_12.8.1.json.

`scripts/qualify-cuda-ptx.py` assembled the current `rrrah_exposure` entry for
all ten targets: sm_52, sm_61, sm_70, sm_75, sm_80, sm_86, sm_89, sm_90,
sm_100 and sm_120. Every invocation produced its required entry/architecture
marker and a nonempty ELF cubin. The compiler reports zero stack and spill
bytes for this kernel. Kernel, compiler, cubin hashes and complete compiler
outputs are in `research/cuda-ptx-assembly-2026-10-06.json`.

The macOS host ran the Linux compiler in an owned network-disabled ARM64
container using pinned image ID
`sha256:224a1869083a311ef3f13648a154ba79832fbef6364d31493642ca03082da254`.
The gate copies its inputs without assuming daemon bind-mount paths and
removes that container on success/failure. No GPU, driver execution or pixel
comparison is involved; the ignored NVIDIA hardware gate remains required.

### CUDA connected to the existing completed-frame timing path (2026-10-06)

The existing `raw_view_timing --raster-view` now accepts the explicit prefix
`--cuda-exposure STOPS`. Its timer includes native raster decode, color
preparation, CUDA context/JIT, host/device transfer, exposure/readback,
host-mediated renderer upload and completed offscreen frame. CUDA ordinal
zero and the graphics adapter are separately selected; the graphics vendor
must be NVIDIA. No matching-device or zero-copy interop claim is made.

`CudaExposure::execute_interleaved` returns a managed flat RGBA32F buffer
without a flattening copy, preserving source ownership and avoiding unsafe
reinterpretation. CUDA output passes through the existing raster renderer,
with sample scale, image selection/count and hotspot retained. CUDA device
input/output and renderer resources have independent limits; CPU output
shares the existing view budget. `RRRAH_CUDA_GPU_MB` configures CUDA admission.

The example builds under the locked workspace. Local CUDA tests pass two
cases, with the required NVIDIA hardware case ignored; that gate now covers
both grouped and interleaved outputs and rejects incomplete pixels before
admission. CUDA crate Clippy passes without warnings. An authored six-pixel
PNG with an explicit sRGB chunk completes the existing non-CUDA path on
Metal Apple M4 Max, fifteen measured frames, CPU/GPU final usage zero.
On macOS the CUDA flag fails explicitly with `UnsupportedPlatform` before
graphics adapter selection or timing output; NaN exposure fails validation.

Logs: `research/cuda-view-timing-build-2026-10-06.log`,
`research/cuda-interleaved-tests-2026-10-06.log`,
`research/cuda-interleaved-clippy-2026-10-06.log`,
`research/cuda-view-metal-regression-2026-10-06.log`,
`research/cuda-view-cuda-unavailable-2026-10-06.log`, and
`research/cuda-view-cuda-invalid-2026-10-06.log`.
NVIDIA frame execution, independent pixels, camera-size timing, interactive
viewer integration and physical HDR remain unqualified.

### Standard PNM color preparation and frame opening (2026-10-06)

An ordinary PPM previously decoded but failed display preparation with
`ColorTransformRequired` because its color space was unspecified. Standard
P1-P6 inputs now declare full-range BT.709 explicitly. `RasterColorSpace::Bt709`
prepares RGB with the nominal inverse BT.709 transfer into linear sRGB
(same primaries/D65), leaving alpha untransformed. Explicit
`assume_untagged_srgb` still selects the sRGB variant. PAM and nonstandard
linear variants do not inherit this standard interpretation.

Primary specifications: https://netpbm.sourceforge.net/doc/ppm.html,
https://netpbm.sourceforge.net/doc/pgm.html and
https://www.itu.int/rec/R-REC-BT.709-6-201506-I.
The raster payload adds color tag 6, retaining existing tags 0-5; the color
declaration roundtrip checks include BT.709. Prepared samples/managed buffers
retain existing selection, scale and hotspot metadata.

All 65,536 u16 levels match an independent f64 nominal formula with maximum
absolute deviation below 3e-7; alpha normalization and final managed release
are checked. Six authored ASCII/binary PBM/PGM/PPM cases exercise black/white,
gray, color and full-range 8/16-bit normalization through public preparation.
The existing Metal completed-frame example now opens the formerly refused
six-pixel PPM on Apple M4 Max; CPU/GPU final managed usage is zero.

`scripts/qualify-pnm-netpbm.py` checks 196,608 linear RGB channels against
both the nominal f64 reference and an independent Netpbm executable/model.
Netpbm 11.2.30 uses a continuous toe, whereas nominal BT.709 uses slope 4.500.
The initial strict one-code comparison failed and is preserved in
`research/pnm-netpbm-nominal-before-2026-10-06.json`. Inspection of official
`editor/pnmgamma.c` established its alternate compression/cutoff formula;
the gate now verifies that model explicitly rather than widening an equality
tolerance. Nominal native max float error is 2.4848746216221684e-7; nonlinear
native/oracle deviation is at most one u16 code. The documented toe difference
is at most four codes, affecting 13,719 RGB values. This is not exact Netpbm
equality. Source/tool/output hashes and policy are recorded in
`research/pnm-netpbm-full-range-2026-10-06.json`.

Full library regression: 116 cache, 80 core and 776 decoder tests pass;
47 external qualification cases remain ignored and are not counted as passes.
Logs: `research/pnm-full-regression-2026-10-06.log`,
`research/pnm-bt709-reference-2026-10-06.log`,
`research/pnm-variants-2026-10-06.log`, and
`research/pnm-metal-frame-2026-10-06.log`. PNM broader variants, physical
display color and the full 100-format/GPU/HDR objective remain unqualified.

PNM physical swap transport qualification (2026-10-06): authored 16-bit P6
4x2 RGB samples cover endpoints, nominal BT.709 knee and Netpbm toe boundary.
The public decoder/preparer runs through RasterRamCache lease protection,
zero-byte eviction, physical swap write, shared-memory pressure refusal,
restore-budget refusal while an alias is alive, and successful retry after
the last alias drops. Both encoded BT.709/u16 and prepared linear/f32 storage
retain identical prepared float bits; all managed allocations return to zero.
The required hardware gate ran on Metal Apple M4 Max and compares every byte
of the 64x64 rendered frame before/after restoration, with nonuniform image
content asserted. Two tests passed, zero skipped. This proves transport
preservation, not independent physical display color or all PNM variants.
Source: `crates/rrrah-app/tests/pnm_cache_swap.rs`; evidence:
`research/pnm-swap-metal-2026-10-06.log`.

Busy neighbour RAM TTL maintenance (2026-10-06): the foreground loader now
checks a monotonic maintenance deadline before each background neighbour,
after checking the foreground request queue. Previously expiry sweeps ran
only on idle receive timeouts; a long uninterrupted neighbour queue could
postpone expiry of unrelated cached buffers. The same sweep covers RAW,
raster and model RAM caches and enqueues eligible expired entries for swap.
Idle sweeps rearm the deadline to avoid duplicate work. A decode already
running is not interrupted by this timer, and a continuous foreground queue
keeps priority; this is not a hard real-time TTL deadline.
The deterministic timer regression simulates 3.1 seconds of uninterrupted
neighbours, verifies three sweeps, releases the expired unpinned allocation,
retains the visible pin, and returns the memory budget to zero on teardown.
Disabled timers and idle rearming have separate coverage.
Validation: two new maintenance tests, three existing idle-TTL tests and
27 gallery/scheduler tests pass. Six external RAW corpus tests remain ignored,
not counted as passes. Logs: `research/busy-prefetch-ttl-2026-10-06.log`,
`research/idle-ttl-regression-2026-10-06.log`, and
`research/prefetch-window-regression-2026-10-06.log`.

Flat raster compute integration (2026-10-06): LinearExposureCompute now
accepts/returns managed interleaved RGBA32F directly. Both flat and array APIs
share validation/dispatch/accounting; no additional CPU layout conversion is
allocated. Actual Metal Apple M4 Max readback checks all 516 components from
129 pixels with three forced dispatches, exact RGB/alpha bits, incomplete
pixel refusal before admission, one-byte-short CPU/GPU budgets, identical
allocation pointers after DecodedRaster construction, and last-alias release.
Two ordinary compute tests pass; two large opt-in tests were not rerun.
`raw_view_timing --raster-view --compute-exposure 1` completed native P6 decode,
linear preparation, WGSL exposure/transfers and 1920x1080 offscreen render.
On the tiny 3x2 fixture, 15 post-warmup measurements gave p50 0.794 ms and
p95 1.864 ms; CPU peak 192 bytes, GPU peak 400 bytes, both released to zero.
These tiny-fixture timings are not full-image performance evidence. The test
readback proves compute arithmetic separately from the benchmark's completed
frame; the latter does not independently qualify rendered color.
Evidence: `research/interleaved-compute-metal-2026-10-06.log`,
`research/compute-api-regression-2026-10-06.log`,
`research/compute-view-metal-2026-10-06.log`, and
`research/compute-view-build-2026-10-06.log`. Interactive viewer compute,
NVIDIA/CUDA execution and physical HDR qualification remain pending.

Managed compute readback cancellation (2026-10-06): CPU readback copies now
poll cancellation every 4096 pixels for both array and flat RGBA32F APIs.
On cancellation the mapped view is dropped and the buffer unmapped before
return; GPU work completes and CPU/GPU reservations release. A real Metal
regression cancels at the second copy chunk after the first chunk was copied,
checks both budgets return to zero, and verifies all 8200 pixels on retry for
each API. Three ordinary GPU tests pass, two opt-in tests were initially
skipped. The large managed test was then run explicitly and extended to check
all 25,494,560 pixels through both flat and array APIs with exact arithmetic,
bounded budgets and final zero accounting. Timing is one allocation/upload/
dispatch/readback sample per API, not throughput or screen latency evidence.
Logs: `research/compute-copy-cancel-2026-10-06.log` and
`research/compute-copy-full-2026-10-06.log`. GPU dispatch cancellation remains
cooperative at CPU boundaries; submitted GPU work is allowed to complete.

Mamiya ZD explicit native sensor API (2026-10-07): exported
`rrrah_decode::decode_mef_zd_sensor` accepts the qualified 4016x5344 packed
layout and returns a managed u16 sensor buffer. This exposes the existing
native foundation without introducing a LibRaw production dependency or
claiming color/viewer support. Final-row cancellation now discards a result
before publication. The pinned camera source SHA-256 was reverified as
`bcd63507c3c4cc3ea1bad2945e3f88cb4677a1a3c762e1acc38df4445714eb7c`.
A fresh independent LibRaw oracle reproduced sensor SHA-256
`3daa5dcb996733310dc06971f75066c74ded92d2f645cc25fc61878dd574eb1b`;
all 21,461,504 native sensor samples match. Both sensor tests passed with zero
skips, covering boundary values, malformed packing, cancellation including
after the final row, one-byte-short output admission, truncation and retention
through the last alias. Evidence: `research/mef-public-sensor-2026-10-07.log`.
The camera's SubIFD next fields point at byte sequences that do not parse as
TIFF directories; exact offsets and observed tag/type words are recorded in
`research/mef-directory-observation-2026-10-07.json`. This observation does not
authorize relaxing general TIFF validation. Native WB and normal MEF viewing
remain unresolved; the format catalog still marks MEF Pending.

CUDA bounded readback (2026-10-07): device-to-host copies now use at most
4096 pixels per synchronous transfer, polling cancellation between chunks.
The context and device reservation remain owned until context cleanup, including
cancelled copies. Pure host tests cover exact device offsets, final partial
chunk, cancellation after one copied chunk and address overflow before copying.
Three tests passed; the actual NVIDIA execution gate remains skipped on this
Mac. These tests prove transfer scheduling, not CUDA driver execution.
Evidence: `research/cuda-copy-chunks-2026-10-07.log`.

Combined library regression (2026-10-07): one locked Cargo invocation tested
rrrah-core/cache/memory/cuda/decode library targets together. Passed: 80 core,
116 cache, 38 memory, 3 CUDA host and 776 decode tests (1013 total). Forty-eight
external/hardware tests remain ignored: 47 decode and one CUDA execution gate.
No failures; two existing unused-import warnings in cache container tests.
Evidence: `research/libraries-regression-2026-10-07.log`. This is library-only
regression evidence, not all-format corpus, interactive viewer, HDR display or
NVIDIA qualification; earlier explicit Metal and MEF runs remain separate.

HDR RAM-to-swap-to-Metal transport (2026-10-07): strengthened the existing
linear HDR test with actual RasterRamCache admission, visible lease refusal
of zero-byte shrink, owner transition and physical spill. Shared CPU pressure
refuses restore without corrupting the entry. Restored [4,2,-0.5,1] samples
render at +1 stop into RGBA16Float; all 4096 output pixels (previously only
the central pixel) match exact IEEE binary16 [8,4,-1,1]. CPU and renderer
texture reservations return to zero. Required Apple M4 Max / Metal execution
passed, no skip. Evidence: `research/hdr-ram-swap-metal-2026-10-07.log`.
This is a uniform synthetic offscreen transport test, not physical HDR display,
nonuniform interpolation, all HDR formats or NVIDIA qualification.

Combined raster swap GPU regression (2026-10-07): six integration tests pass
on required Metal Apple M4 Max with no optional adapter skips. Covers profiled
PNG/TIFF/ORA/KRA/XCF color transport, eleven legacy XCF transparency/mask cases,
three payload representations under corruption/truncation/replacement, HDR
float transport and committed PICT fixtures. Two PICT tests were removed from
the ignored set because fixtures are committed and this integration target
already requires a real GPU. Only the external PICT/TwelveMonkeys corpus test
remains ignored. Evidence: `research/raster-swap-all-metal-2026-10-07.log`.
This is selected fixture transport coverage, not qualification of all variants
of these formats or physical display color/HDR.

Thumbnail prefetch count admission (2026-10-07): submit now limits iterator
consumption explicitly to the bounded channel capacity. Previously a worker
could dequeue one job while submit was producing the same window, permitting
one additional job or consuming an item beyond the declared window. A new
regression supplies a lazy iterator that refuses access beyond capacity; five
thumbnail scheduler tests pass. Evidence: `research/prefetch-count-2026-10-07.log`.
The release benchmark exposed a stale Vec-to-PixelBuffer construction; it was
updated to shared PixelBuffer ownership. Performance results require rerunning
the corrected benchmark; test success alone is not latency evidence.
The corrected release benchmark subsequently completed: 50,000 window
replacements, capacity eight, p50 0.333 us, p95 0.375 us, p99 0.625 us,
maximum 144.959 us. Latest-generation-only publication and bounds of eight
queued jobs/eight ready buffers were asserted. Eight ready pixel buffers use
2,097,152 bytes; the analytical bound including one active buffer is
2,359,296 bytes. This synthetic blocked-worker benchmark measures scheduling
only, not RAW decode, GPU display, full application RSS or physical allocation
peak. Evidence: `research/prefetch-scheduler-bench-2026-10-07.log`.

Raster GPU replacement accounting (2026-10-07): actual Metal test uses a
32-byte texture budget. A one-byte pressure reservation refuses replacement
of a resident 16-byte frame; after pressure release, old/new textures jointly
reach 32 bytes. An external old-frame resource lease continues to block the
next replacement after queue completion. Dropping it releases 16 bytes and
retry succeeds. Final renderer teardown returns credit to zero. All three
resource-lease GPU tests pass without skips, including existing upload and
submitted-work lifetime cases. Evidence:
`research/raster-replacement-metal-2026-10-07.log`. These assertions concern
explicit texture reservations, not driver allocation/RSS or display color.

Public PNM swap restore cancellation (2026-10-07): the native P6 transport
integration now cancels only after detecting a live output reservation during
restore. Both encoded BT.709/u16 and prepared linear/f32 paths return no result,
release output credit to zero, leave read/error counters unchanged, and then
successfully retry from the same physical swap entry. Exact prepared float bits
and the entire Metal frame remain identical after retry. Both tests passed on
required Metal Apple M4 Max, no skips. Evidence:
`research/pnm-swap-cancel-metal-2026-10-07.log`.

Combined GPU qualification regression (2026-10-07): required Metal Apple
M4 Max backend, optional GPU skipping disabled. All 47 library tests and 44
ordinary integration tests pass (91 total), covering camera color, RAW and
orientation, raster, filmstrip, model rendering, compute, four-channel RGBe
and resource lifetimes. Four opt-in tests were skipped in that invocation.
The two full-resolution compute tests were then explicitly executed: both
passed, including flat/array managed all-pixel comparisons and the separate
chunked GPU exposure path. Two corpus-dependent X3F/DSC-F828 tests still lack
their external inputs in this run and are not counted as passes. Evidence:
`research/gpu-suite-metal-2026-10-07.log` and
`research/gpu-full-compute-metal-2026-10-07.log`. This is Metal offscreen
regression coverage; physical HDR, NVIDIA/CUDA execution and full 100-format
qualification remain separate incomplete requirements.

RLA managed output implementation (2026-10-07): the explicit alpha/color
interpretation API previously returned unmanaged pixel vectors even with a
request memory budget. Native RLA now admits the u16 working raster before
allocation, admits temporary decoded RLE byte planes, and constructs retained
RGBA8/float results in managed buffers. RGBA16 adopts the working buffer
directly. Premultiplied conversion checks cancellation during pixel processing.
The public API regression uses committed 8/16-bit alpha fixtures, checks
one-byte-short working-output refusal, retained final credit through the last
alias and zero accounting after teardown. Directory metadata and allocator/
driver overhead remain outside this pixel-buffer accounting.
Final full decoder library regression after RLE plane admission: 777 passed,
zero failed, 47 external tests ignored. The new public RLA budget test passed,
as did the independent raster corpus comparison. Evidence:
`research/rla-managed-full-regression-2026-10-07.log`.

Explicit RLA RAM/swap/Metal integration (2026-10-07): committed RGB+alpha
8-bit and mixed 16-bit-color/8-bit-alpha fixtures decode under an explicit
straight-alpha/sRGB interpretation. The test checks visible lease protection,
zero-byte RAM shrink after owner transition, physical swap write, restore
refusal under shared-memory pressure, exact serialized payload preservation,
exact prepared float bits and every byte of the 64x64 Metal frame before/after
restore. Retained aliases hold output credit until the last owner drops; final
credit is zero. Required Apple M4 Max execution passes with no skip. Evidence:
`research/rla-cache-swap-metal-2026-10-07.log`. The color/alpha interpretation is
caller-supplied; this is transport preservation, not independent RLA producer
color qualification or an implicit default for ambiguous alpha association.

Common RLA request interpretation (2026-10-07): DecodeRequest now exposes
optional `rla_alpha_mode`. The common native raster route preserves the default
refusal of alpha-bearing files and accepts an explicitly selected straight or
premultiplied association. Color remains unspecified unless the separate
`assume_untagged_srgb` policy is explicitly enabled, in which case it is marked
AssumedSrgb. The common/direct API comparison uses the committed RLA fixture,
checks strict default refusal, identical decoded pixels and managed teardown.
The request API is available to library callers; a viewer CLI/UI alpha option
and corresponding cache interpretation keys remain pending before claiming
configurable RLA viewing in the application.
Four RLA tests pass on the final source, including explicit sRGB preparation.
Evidence: `research/rla-common-request-2026-10-07.log`. The earlier application
unit sweep in this turn passed 146 tests with 18 external tests ignored before
the request API addition; it is recorded separately as
`research/app-unit-regression-2026-10-07.log`, not proof of viewer integration.

RLA application cache policy isolation (2026-10-07): RasterDisplayKey now
includes the optional RlaAlphaMode; straight, premultiplied and absent policies
have distinct RAM/swap identities. Before this change the same source and color
policy could reuse a frame prepared under an explicitly selected alpha mode
for a subsequent strict request without one. The regression caches an actual
straight-alpha fixture, verifies absent-policy refusal despite that resident
entry, then restores the explicit policy and requires a cache hit without
decoding. Managed credit returns to zero. Full application unit suite: 147
passed, zero failed, 18 external cases ignored. Evidence:
`research/rla-cache-key-app-2026-10-07.log`. Viewer CLI/UI policy selection is
still pending; this fixes cache identity before exposing that selection.

RLA application launch configuration (2026-10-07): added optional
`--rla-alpha straight|premultiplied`. RasterInterpretation carries the alpha
and independent untagged color policies through the viewer's foreground
loader, neighbour requests and thumbnail loader. Default alpha remains absent
and color remains strict. The CLI regression checks both enum values, rejects
unknown values, applies configuration to an actual RLA decode/display-preparer
and creates a managed thumbnail; restoring default configuration clears both
assumptions and all managed credit releases. Full application unit suite:
148 passed, no failures, 18 external cases ignored. Evidence:
`research/rla-cli-app-2026-10-07.log`. This proves configured decode/thumbnail
paths and propagation, not a physical window presentation/color measurement
or support of all RLA storage/window/channel variants.

RLA dual alpha-policy cache qualification (2026-10-07): authored two-pixel
RLA with nonzero half/opaque alpha goes through the application's actual native
load/preparation/cache path under straight and premultiplied settings. The
half-alpha RGB differs as expected, alpha bits remain identical, and each red
sample is checked against separate f64 sRGB arithmetic (64/255 vs 64/128).
Both entries remain resident and each repeat request must return its original
shared allocation without decoding. All managed credit releases on teardown.
Three focused RLA application tests passed with zero skips. Evidence:
`research/rla-alpha-cache-2026-10-07.log`. This qualifies prepared pixel/cache
semantics for the authored case, not producer-wide or physical display color.

RLA contained active/full windows (2026-10-07): native decoding now counts
only active rows in the scanline directory, allocates the full normalized
canvas, places active samples with full-window-relative offsets, and leaves
outside pixels transparent black. Invalid/reversed/outside bounds are rejected.
The test checks every pixel of a 145x6 canvas with a 140x3 active region,
translation near both signed i16 coordinate limits, one-byte-short working
budget refusal, and zero retained managed memory after owners are dropped.
Five focused RLA tests pass; decoder library regression: 779 passed, zero
failed, 47 external tests ignored. Evidence: research/rla-window-focused-2026-10-07.log
and research/rla-window-full-2026-10-07.log.
OpenImageIO independently confirms the active/full dimensions but reports a
different vertical-origin convention; offset-canvas pixel parity is NOT claimed.
See research/rla-window-observation-2026-10-07.md and
research/rla-window-oiio-2026-10-07.log. Real-producer coordinate qualification,
world-origin retention, broader channel/color variants and live window display
remain pending. This does not qualify all 100 formats or complete the goal.

RLA offset full-canvas RAM/swap/Metal transport (2026-10-07): expanded the
actual app integration test to four cases: 8/16-bit RGB with explicit straight
alpha, each with original windows and a contained 140x3 active area in a 145x6
full canvas. Transparent border samples are checked explicitly before spill.
Each case exercises visible lease preventing eviction, spill after release,
restore refusal under full memory pressure followed by successful retry,
exact wire payload and prepared float bits, and exact whole 64x64 GPU frame
before/after physical swap. Aliases retain the reservation until their final
drop, then managed memory returns to zero; each case reports one successful
write/read and zero swap errors. Actual adapter: Metal Apple M4 Max.
One integration test covering all four cases passed, zero ignored; evidence:
research/rla-window-swap-metal-2026-10-07.log. This establishes transport
preservation, not independent producer coordinate interpretation, physical
HDR screen output, or NVIDIA execution.

RLA offset-canvas application thumbnail (2026-10-07): new test exercises the
actual thumbnail loader with CLI straight-alpha/sRGB policy and a file whose
140x3 active area is contained in a 145x6 canvas. Output retains the full extent
at edge 145 and job index; every outside pixel is checked against an independent
f64 encoding of the thumbnail's linear 0.018 background, with opaque alpha.
Dropping the returned thumbnail releases all managed memory. A stale generation
returns no result without retained memory, and a budget below the full working
raster refuses the load with no retained memory. Four focused app RLA tests
passed, zero ignored; research/rla-window-thumbnail-2026-10-07.log.
This exercises the loader shared by thumbnail prefetch, not worker scheduling
or physical presentation. Producer coordinate qualification remains pending.

Combined raster cache policy transition (2026-10-07): new managed-buffer
regression holds a consumer lease on one HDR raster and the visible pin on a
second, then simultaneously tightens bytes 128->64, count 2->1 and TTL None->0.
The refused transition preserves both entries and their lookup behavior.
After lease release the transition succeeds, evicts the old entry and retains
64 bytes for the visible owner. Existing deadlines stay fixed by the documented
policy; replacing the visible frame applies TTL zero, preventing fresh lookup
while preserving its pin. Unpin plus expiry drains the final entry and releases
all memory. The initial test expectation that TTL changes existing deadlines
was corrected to the existing explicit contract; production semantics unchanged.
Full cache library: 117 passed, zero failed/ignored. Evidence:
research/cache-combined-policy-2026-10-07.log. This is a cache library state
transition test; it does not prove live UI reconfiguration or all formats.

RLA float producer byte-order observation (2026-10-07): independently authored
2x1 RGBA float32 fixture with OpenImageIO oiiotool, source values [4,2,-0.5,1],
saved as tests/fixtures/raster/rla-float32-le-hdr-oiio.rla. Header fields are
big-endian; float records are uncompressed little-endian on this arm64 producer.
The alternate big-endian interpretation is finite and nearly zero, so finite
validation cannot disambiguate it. OpenImageIO reader directly copies float
records and omits float endian swapping:
https://github.com/AcademySoftwareFoundation/OpenImageIO/blob/main/src/rla.imageio/rlainput.cpp
Observed channel type is 4 and precision 32. Evidence, exact bytes, producer
version and SHA256: research/rla-float-byte-order-2026-10-07.json.
Native float support remains pending an explicit byte-order interpretation
contract and independent tests; do not add automatic native-platform guessing
or claim HDR RLA support from this fixture alone.

Native explicit-endian float RLA (2026-10-07): added public
RlaFloatByteOrder and decode_rla_float_with_interpretation. Supports gray/RGB
float32, optional float32 matte, contained full/active windows, managed final
float pixels and cancellation checkpoints. Mixed integer/float groups remain
refused. Finite negative/HDR RGB values retained; finite alpha in [0,1]
required; premultiplied unassociation rejects zero-alpha emission and overflow.
Native output matches the committed OpenImageIO-produced/read-back [4,2,-0.5,1]
fixture exactly. An authored byte-swapped version verifies explicit Big;
alpha 0.5 verifies [8,4,-1,0.5] unassociation. NaN, invalid alpha, zero-alpha
emission and one-byte-short output budget fail without retained pixel credit.
Public file API retains only the final output reservation, then releases it.
Six focused tests passed. Full decoder regression: 780 passed, 47 external
tests ignored, zero failures. Evidence: research/rla-float-native-2026-10-07.log,
research/rla-float-full-2026-10-07.log and
research/rla-float-oiio-readback-2026-10-07.log.
The default route refuses float input without a byte-order contract. Common
request/CLI/cache-key wiring and float-RLA-specific RAM/swap/Metal/HDR display
qualification remain unfinished. This does not complete the broader goal.

Float RLA common request/cache identity (2026-10-07): DecodeRequest now exposes
optional rla_float_byte_order, default None. The common raster decoder forwards
it alongside explicit alpha and color policy. RasterDisplayKey includes the
order, preventing prepared display entries from being shared across Little,
Big or an unspecified request. An actual file/display-cache test creates two
separate entries for the external float fixture with explicit sRGB assumption
and straight alpha, checks distinct output bits, requires allocation-identical
cache hits on repeat, and verifies that None refuses despite both entries.
Dropping results and cache returns managed memory to zero. Five focused app
RLA tests passed, zero ignored. Evidence:
research/rla-float-request-app-2026-10-07.log. This test declares encoded sRGB;
it does not infer linear color from float storage. CLI byte-order/color controls,
float-specific swap/GPU qualification and physical HDR presentation remain
pending.

Float RLA native RAM/swap/linear Metal readback (2026-10-07): shared existing
HDR transport harness now runs both the synthetic one-pixel source and the
independent two-pixel float-RLA producer fixture. The float fixture is decoded
with explicit Little/Straight/LinearSrgb policy, retained under a visible lease,
then spilled after unpin. Full CPU budget pressure refuses restore without a
swap corruption error; releasing pressure enables restore. All eight restored
f32 bits match [4,2,-0.5,1] repeated twice, and RAM memory returns to zero after
upload/drop. The source GPU texture retains 32 managed bytes until renderer
drop; this accounting excludes the test target/readback and driver overhead.
At +1 exposure, every one of 4096 Rgba16Float target pixels equals [8,4,-1,1]
exactly in binary16. The initial new test used insufficient zoom for the 2:1
fixture and encountered the expected viewport background; adjusting test
framing to zoom four made the full target contain image pixels. No renderer
behavior was changed. Two tests passed, zero ignored, actual Apple M4 Max Metal.
Evidence: research/float-rla-hdr-swap-metal-2026-10-07.log. Physical HDR display,
CLI color/byte-order selection, mixed float/integer groups and NVIDIA remain
pending; offscreen linear readback does not establish physical presentation.

Float RLA application byte-order configuration (2026-10-07): added optional
--rla-float-byte-order little|big. RasterInterpretation forwards it to requests
on the foreground, neighbour and thumbnail paths; applying the default policy
clears a previously selected order rather than retaining stale interpretation.
A new actual CLI/decode/thumbnail test verifies Little with explicit straight
alpha and encoded sRGB assumption, default refusal, Big selection, invalid
native choice refusal, dimensions and released managed memory. Six focused
app RLA tests passed, zero ignored; research/rla-float-cli-2026-10-07.log.
Encoded sRGB in this routing test is an explicit test interpretation, not an
inferred property of float storage. Producer-declared linear color still uses
the library API; a suitable CLI color option and live presentation remain
pending. This does not establish physical HDR output or all formats.

Explicit RLA color request/CLI/cache identity (2026-10-07): added optional
RlaColorSpace::{Srgb,LinearSrgb} to DecodeRequest and --rla-color srgb|linear-srgb.
The RLA-specific choice takes precedence over the generic untagged policy,
without inferring linearity from storage; default None retains strict behavior.
Shared RasterInterpretation carries the choice to foreground, neighbours and
thumbnails and clears it when default settings are applied. RasterDisplayKey
includes color alongside alpha and float byte order. Actual common file loading
with the linear CLI policy preserves all eight HDR f32 values exactly. Choosing
encoded sRGB creates a distinct prepared cache entry; switching back must reuse
the same linear pixel allocation. Actual thumbnail loading succeeds under the
linear policy; final owner drops release managed memory. Seven focused app
RLA tests passed, zero ignored; research/rla-linear-cli-2026-10-07.log.
README now documents the producer-declared Little/Straight/LinearSrgb command.
Physical HDR presentation, producer-wide color/window qualification, mixed
channel storage, NVIDIA and completion of all 100 formats remain unproven.

Final RLA request/CLI regression sweep (2026-10-07): the full application
unit suite passed on the final alpha/float-order/color fields and cache identity:
153 passed, zero failed, 18 external/hardware-dependent tests ignored.
The decode/cache/memory library sweep passed 780/117/38 respectively, total
935 passed, zero failed, 47 external decoder tests ignored. Existing two unused
import warnings in cache container tests remain; this is not a clean Clippy
claim. Evidence: research/rla-app-full-regression-2026-10-07.log and
research/rla-library-full-regression-2026-10-07.log. These library/unit suites
do not replace independent producer qualification, live display, CUDA hardware
execution or all-format acceptance. Full goal remains incomplete.

AVIF primary clean-aperture implementation (2026-10-07): primary-associated
clap records now retain their exact 32-byte rationals; duplicate/malformed
records refuse. Crop conversion uses signed offset numerators and unsigned
nonzero denominators with i128 arithmetic. Integral positive extents and
pixel-aligned origins are required without rounding; negative/outside bounds
refuse. Native selected-image decode applies the admitted crop before image
orientation/rotation/mirror and admits final RGBA bytes using cropped dimensions.
Codec scratch remains separate from final-pixel accounting, as before.
Tests verify integral/reduced fractions, odd-sized images with half offsets,
negative offsets, bounds/denominator/fractional refusals, primary association
and duplicate properties. Full decoder suite: 782 passed, zero failed, 47
external tests ignored; research/avif-clean-aperture-regression-2026-10-07.log.
Rational convention checked against the primary libavif implementation:
https://github.com/AOMediaCodec/libavif/blob/main/src/avif.c
No avifenc executable is available locally. Independently produced cropped
AVIF pixel oracles, combined transform qualification, RAM/swap/GPU and live
presentation of this crop path remain pending. Fractional resampling and HDR
AVIF are still unsupported/unqualified; this is not full AVIF qualification.

AVIF essential clean-aperture coded-pixel qualification (2026-10-07): added
CC0 avif-clean-aperture-integer.avif on the pinned libavif RGB producer, with
authored essential primary clap/ipma metadata for crop (3,5)-(11,11). Generator
scripts/generate-avif-crop-fixture.py rewrites only the known fixture layout,
updates iloc offsets and verifies unchanged independently decoded source pixels.
Pillow 12.3.0/libavif decodes stored pixels without applying clap; Pillow's crop
provides the separate 8x6 RGBA oracle. This is not a crop exported by an
independent producer, nor an independent container crop interpreter.
The real file exposed mp4parse rejecting essential clap before coded decode.
After our primary-property parsing, only handled clap association essential
bits are cleared in the private decode buffer. Source bytes, offsets, unknown
required properties and our explicit crop interpretation remain intact. The
native crop now passes dimensions, alpha/pixel tolerance <=3, file SHA256,
managed output ownership and final reservation release in the shared corpus.
Corpus now has 151 admitted cases; its fixed coverage count was updated from
150 after all pixel checks passed. Full decoder regression: 782 passed, zero
failed, 47 external tests ignored. Evidence: research/avif-crop-pixel-regression-2026-10-07.log
and research/avif-crop-generation-2026-10-07.log.
Independent producer clap/transform interpretation, broader rational/resampling
cases, cropped-image RAM/swap/GPU and physical presentation remain pending.

Cropped AVIF RAM/swap/Metal transport (2026-10-07): added actual cropped
container test in hdr_raster_swap. Native 8x6 sRGB pixels are compared with the
independent stored-pixel/Pillow-crop oracle, tolerance <=3. Shared managed U8
transport harness verifies leased eviction refusal, physical streamed swap,
root memory-pressure restore refusal followed by successful retry, local restore
cap binding while the parent has space, exact retained pixels/dimensions/color,
nonuniform Metal frame and whole-frame equality before/after restore. Aliases
retain reservation until final drop, then root memory is zero; second restore
also succeeds after release. Actual adapter: Metal Apple M4 Max.
The shared harness preserves existing PICT expected color and adds explicit
restored dimensions. Focused cropped AVIF passes; full HDR/raster-swap suite
passes eight tests with one external PICT ignored. Evidence:
research/avif-crop-swap-metal-2026-10-07.log and
research/avif-crop-swap-suite-2026-10-07.log. Independent producer transform
interpretation, combined crop/orientation, fractional resampling and physical
presentation remain pending. This does not establish full AVIF qualification.

Combined AVIF crop/orientation transport (2026-10-07): expanded the fixture
writer with seven crops on the committed libavif orientation-2..8 sources.
Authored essential clap is associated after ispe and before rotation/mirror;
initial insertion before ispe was correctly refused by the container codec and
was fixed in the fixture writer, without relaxing parser strictness. Source
coded pixels stay unchanged. Independent Pillow stored pixels are cropped
before ImageOps.exif_transpose; expected final dimensions are 4x2 or 2x4.
The full shared pixel corpus now has 158 cases and passes dimensions, SHA256,
alpha/pixel tolerance <=3, managed ownership and final memory release.
The actual RAM/swap/Metal integration covers all eight cropped sources, including
visible lease protection, parent pressure retry, local restore cap, exact native
pixels/dimensions/color, source-dependent nonuniform whole frame preservation,
alias credit retention and repeat restoration. One integration test with eight
cases passes, zero ignored, actual Apple M4 Max Metal. Evidence:
research/avif-crop-orientation-pixels-2026-10-07.log and
research/avif-crop-orientation-metal-2026-10-07.log; fixture generation observation:
research/avif-crop-orientation-generation-2026-10-07.log.
Crop metadata is authored; independent exported crop producer, fractional
resampling, HDR/sequence and physical display qualification remain pending.

Actual AVIF clean-aperture refusal/memory test (2026-10-07): mutated the
committed cropped container through its public file decoder. Zero denominator,
fractional width, out-of-bounds offset/extent and half-pixel origin return typed
InvalidAvif and release all managed input/output credit. Replacing the essential
clap property with unknown zzzz refuses, proving handled-clap normalization does
not silently admit another mandatory property. One-byte-short input-plus-final
output budget returns typed memory refusal with zero retained credit. Retrying
the valid source succeeds at 8x6; only final raster capacity remains charged,
and an alias retains credit until the final owner drops. This accounting covers
managed input/final pixels, not codec scratch. One focused test passed, zero
ignored; research/avif-crop-refusals-2026-10-07.log. Broader invalid-property
combinations, independent producer crops and all-format qualification remain
pending.

### AVIF primary transform association order (2026-10-07)

The native metadata parser now rejects `clap` after `irot`/`imir` and `irot` after `imir`, before codec decoding or output admission. MIAF 7.3.6.7 requires clean aperture, rotation, mirror in that order; see [libavif reference implementation](https://github.com/AOMediaCodec/libavif/blob/main/apps/shared/avifutil.c) and [AVIF specification](https://aomediacodec.github.io/av1-avif/latest-approved.html). Other property types may interleave. A committed coded AVIF fixture is mutated only in its primary association bytes: all five incorrect permutations return typed `InvalidAvif` both in metadata parsing and the public file decoder, with managed memory returning to zero. The full decoder suite passed 784 tests, 47 ignored, including the 158-case pixel corpus; evidence: `research/avif-transform-order-2026-10-07.log`.

The authored offset-crop fixtures prove exact HEIF clean-aperture geometry and subsequent AVIF orientation handling, not complete AVIF v1.2 producer conformance. The latest approved AVIF specification additionally restricts crop origin to (0,0), unless an uncropped secondary non-hidden item is present. These fixtures do not contain that secondary item. Independent conforming crop producers, sequences and HDR AVIF qualification remain pending.

### AVIF top-left crop corpus and Metal transport (2026-10-07)

Eight additional authored essential-clap fixtures use crop origin (0,0), satisfying the crop-origin restriction of AVIF section 2.2.3 without needing a secondary uncropped item. Stored 16x16 pixels crop to 8x6; seven orientation producers crop stored 8x4 pixels to 4x2 before rotation/mirroring (2x4 for orientations 5–8). The committed libavif coded payloads are unchanged. Golden pixels remain independent Pillow/libavif stored-pixel decoding followed by explicit Pillow cropping/orientation; this is not independent producer-exported clap metadata or independent container-transform validation.

The native pixel corpus now contains 166 cases and passed. The actual Apple M4 Max Metal RAM/swap integration passed all 16 AVIF crop cases (eight offset and eight top-left), including managed reservation/alias lifetime, lease pressure, physical swap restore admission/refusal/retry, exact restored native pixels and equal GPU frames before/after restore. Logs: `research/avif-top-left-generation-2026-10-07.log`, `research/avif-top-left-pixels-2026-10-07.log`, `research/avif-top-left-metal-2026-10-07.log`. This does not establish full AVIF v1.2 conformance, HDR AVIF or sequence support.

### Independent AVIF container crop metadata oracle (2026-10-07)

`scripts/avif-crop-oracle.c` (external qualification tool only) decodes all 16 committed crop files using libavif 1.4.2 defaults and calls `avifCropRectFromCleanApertureBox` on the container-parsed image. The committed result `tests/fixtures/raster/avif-crop-libavif-1.4.2.tsv` records stored dimensions, crop x/y/width/height, transform flags, rotation angle and mirror axis. A native regression compares every record against rrrah's primary-property parsing and exact rational crop calculation; all eight AVIF metadata tests passed with zero ignored. This closes the missing independent container-interpretation evidence for these fixtures, while producer-exported crop metadata, HDR and sequences remain unqualified. The libavif acceptance does not by itself prove full AVIF specification conformance.

Oracle provenance: Pillow 12.3.0 macOS arm64 bundled libavif 1.4.2, library SHA256 `ee0124cb07fb6419ed743cf9658b25211f7cc87d9081ad3e9416100db02c3c09`; official v1.4.2 header SHA256 `43ec2563fcc739a4d5ca18e22f74bc15e76a11f8c9a82bd7e1ee572bff7afc04`. Compile the C tool against that matching header and library; for the Pillow macOS wheel, set its executable dependency to `@rpath/libavif.16.4.2.dylib` and add the wheel `.dylibs` directory as rpath, since the library's original install name points to its build directory. Run `avif-crop-oracle tests/fixtures/raster/avif-clean-aperture-*.avif`, strip the directory prefix for the committed TSV. Comparison log: `research/avif-libavif-metadata-comparison-2026-10-07.log`. No production dependency changes.

### Independently exported AVIF clean-aperture fixtures (2026-10-07)

The external C oracle now exports top-left crops using libavif 1.4.2 `avifCleanApertureBoxFromCropRect` and `avifEncoderWrite` at quality 100, speed 10, preserving the independently decoded source orientation. Eight exported fixtures (`avif-libavif-crop-*`) and golden RGBA files are committed; the generator is `scripts/generate-avif-producer-crop-fixture.py`. Libavif writes both the coded payload and the container metadata; our metadata rewriter is not involved. Pillow/libavif independently decodes the exported stored pixels, then Pillow crops and applies exposed orientation for golden pixels. The separate C container oracle reads all 24 crop files and independently calculates clap rectangles and transform declarations. Native metadata matches all 24 records.

The complete native decoder suite passed 785 tests, 47 ignored, including the 174-image pixel/hash corpus. Actual Apple M4 Max Metal transport passed all 24 crop fixtures (16 authored, eight independently exported), including original/restored pixel equality, rendered frame equality, budget refusal/retry and last-owner credit release. This qualifies first-image RGB8 clean-aperture/orientation behavior for these independent producer files, not HDR, fractional sampling, sequence, all producer variants, or complete AVIF conformance. Evidence: `research/avif-producer-crop-generation-2026-10-07.log`, `research/avif-producer-crop-pixels-2026-10-07.log`, `research/avif-producer-crop-metal-2026-10-07.log`.

### AVIF HDR CICP declaration retention through swap (2026-10-07)

AVIF NCLX declarations outside the already qualified sRGB/linear-sRGB cases now retain their exact primaries, transfer, matrix and full-range fields in `RasterColorSpace::Cicp` instead of becoming `Unspecified`. PQ (transfer 16) and HLG (18) BT.2020 declarations are exercised with both range flags; code points follow [ITU-T H.273](https://www.itu.int/rec/T-REC-H.273). This is metadata preservation, not HDR display qualification or a change to codec sample precision. ICC precedence is unchanged.

Raster swap payloads serialize CICP as color tag 7, using header bytes 52–58 for three little-endian u16 values and a strict boolean, with bytes 59–63 reserved. Existing color tags require the whole extension area to remain zero; older readers reject the new tag instead of guessing an interpretation. Roundtrips preserve PQ/HLG metadata and pixel precision; malformed range/reserved bytes or changing a CICP header to an older color tag fail with managed memory returning to zero. Both original and restored CICP frames still require a qualified color transform before conversion to linear sRGB.

Core/cache/decode full regression: 80/118/785 tests passed, 47 decoder cases ignored. App check succeeded; focused AVIF metadata tests passed 8, zero ignored. Logs: `research/cicp-preservation-regression-2026-10-07.log`, `research/cicp-app-check-2026-10-07.log`, `research/cicp-avif-metadata-2026-10-07.log`. PQ/HLG conversion, luminance policy, independent high-bit-depth AVIF pixel fixtures and physical HDR display qualification remain pending.

### Independent 10-bit PQ AVIF decode evidence (2026-10-07)

Committed CC0 synthetic `avif-pq10-svt.avif` is 128x64, with four grayscale stripes from 10-bit Y values 0/128/512/1023 and neutral 4:2:0 chroma 512, full range. External FFmpeg 9.0.2/SVT-AV1 encodes it using preset 8, CRF 0 and explicit SVT parameters `color-primaries=9:transfer-characteristics=16:matrix-coefficients=9`. Merely specifying FFmpeg output color options initially produced primaries/transfer 2/2; the public decoder test caught that mismatch, and the fixture was regenerated with encoder parameters rather than weakening the test. FFmpeg independently decodes it to `rgba64le` for the committed oracle. This shares dav1d AV1 decoding with image but uses an independent color conversion/scaling path; it is not an independent AV1 decoder implementation.

The native file test passed: dimensions, Rgba16 storage, exact CICP 9/16/9/full-range, every pixel against the independent output within 128 u16 code values, managed allocation lifetime and explicit refusal of unqualified linear-sRGB conversion. This is high-bit-depth decode evidence, not accurate PQ display. image 0.25.10 currently expands 10-bit RGB to u16 by shifting six bits (including opaque alpha); full-range normalization differs from the independent conversion and remains to be corrected/qualified before exact HDR display claims. No physical HDR screen proof.

Source SHA256 `724497164e88d67921ab3226df435b155afd556ed6cd3913b0b7a6a7393b5a0e`; golden SHA256 `0ec255dcbf829909ea9c9449aa25cfa400460c924ed354e01b3ed7d98ad014b9`. Logs: `research/avif-pq10-generation-2026-10-07.log`, `research/avif-pq10-native-2026-10-07.log`. External encoders remain fixture-only dependencies.

### AVIF integer code-range normalization (2026-10-07)

Native metadata parsing retains primary av1C depth from its high_bitdepth/twelve_bit fields, validates marker/version and impossible twelve-bit-without-high-bitdepth combinations, and rejects duplicate configurations. See [AV1 codec configuration syntax](https://aomediacodec.github.io/av1-isobmff/#av1codecconfigurationbox). Rgba16 output from image 0.25.10 is now corrected in place: recover the original 10/12-bit code from the codec's rotated expansion, then integer-round `code * 65535 / ((1 << depth) - 1)`. All 1024 ten-bit codes and 4096 twelve-bit codes match an independent f64 rational oracle; endpoints are exactly 0/65535. Nonconforming low bits or a 16-bit result without a supported coded depth refuse rather than guess. Already-full opaque alpha remains full; otherwise alpha receives the same exact code-range normalization. Cancellation is checked every 4096 pixels and no second output allocation is introduced.

The independent SVT PQ10 file passes, including every opaque alpha equal to 65535, unchanged CICP, managed lifetime and every sample comparison. Maximum deviation against the independently generated FFmpeg RGBA16 conversion is 63/65535 (different conversion scaling/rounding); this is not exact independent decoder equality. Full decoder regression passed 787 tests, 47 ignored. Logs: `research/avif-u16-normalization-regression-2026-10-07.log`, `research/avif-u16-normalization-pq10-2026-10-07.log`. The earlier shift-normalization limitation is corrected for primary coded depths 10/12; independent 12-bit producer and alpha producer qualification, PQ luminance/color conversion and physical HDR display remain pending.

### Explicit PQ BT.2020 to linear sRGB conversion (2026-10-07)

`DecodedRaster::pq_to_linear_srgb_with_budget_and_cancel` accepts CICP primaries 9/transfer 16 and an explicit finite reference-white luminance from 1–10000 cd/m². RGB samples have already been decoded from container YUV into normalized RGB; matrix/range metadata is retained provenance and is not applied a second time. The native f64 ST 2084 inverse transfer yields absolute 0–10000 cd/m², normalized by reference white, then applies the [Khronos Data Format 1.4 BT.2020-to-BT.709 matrix](https://registry.khronos.org/DataFormat/specs/1.4/dataformat.1.4.inline.html). Output is managed f32 linear sRGB with straight alpha unchanged. Values above one and negative out-of-gamut components are retained; there is no tone mapping/clipping. Sample scale, hotspot and container selection remain intact.

Tests cover black, published PQ codes corresponding to 100/1000 cd/m², 10000 cd/m² peak, saturated red yielding negative green/blue, alpha preservation, exact managed credit, one-byte-short admission refusal, cancellation with credit release, and invalid luminance policy. Core full suite: 81 passed, zero ignored. The independent SVT PQ10 public-file test now completes decode → normalized Rgba16 → managed linear PQ output, proves opaque alpha, peak >95 at a 100-nit white, dark samples, joint source/output accounting and release to zero. It passed. Logs: `research/pq-linear-core-2026-10-07.log`, `research/pq-linear-avif-file-2026-10-07.log`.

This is an explicit library API, not yet wired to viewer/thumbnail interpretation or a physical HDR display. Independent full-color conversion oracle, GPU/Metal transport of the converted AVIF, display luminance policy, HLG scene/display interpretation and hardware output qualification remain pending.

### Viewer, preloading and thumbnail PQ policy (2026-10-07)

CLI `--pq-reference-white-nits` validates a finite 1–10000 cd/m² value, stores it in RasterInterpretation and propagates through the existing foreground/neighbour/thumbnail request policy. DecodeRequest exposes `pq_reference_white_nits` (default None); raster decode converts only declared PQ BT.2020 frames using the explicit native API and the same parent budget/cancellation. Other color declarations remain unchanged. Both coded and converted outputs are accounted during conversion. Display-cache identity includes the white's exact f32 bits alongside the existing RLA color declaration, so it stays hashable without exceeding Rust tuple-trait arity and preserves all existing identity fields.

The actual CLI/cache/thumbnail test uses independent SVT PQ10 AVIF: 100/200 cd/m² produce separate entries and every RGB sample differs by exactly factor two within 1e-5; alpha is identical. Returning to 100 reuses the exact managed pixel owner without decoding; dropping cached owners releases root credit; 32x16 thumbnail generation with the same policy succeeds; default policy clears a prior white setting; 0/NaN/infinity/>10000 CLI values refuse. Full app tests passed 154, 18 ignored; full decoder passed 787, 47 ignored. Logs: `research/pq-viewer-cli-regression-2026-10-07.log`, `research/pq-viewer-cache-thumbnail-2026-10-07.log`, `research/pq-request-decode-regression-2026-10-07.log`. This connects the library transform to actual viewer preparation but does not prove physical HDR output, GPU readback of these converted AVIF frames or HLG support.

### Converted PQ AVIF RAM/swap/Metal qualification (2026-10-07)

The actual SVT PQ10 source, opened through public DecodeRequest with 100 cd/m² reference white, produces managed 128x64 linear-sRGB f32 samples including >95 RGB values. On actual `Metal Apple M4 Max`, the new test renders a nonuniform image before swap, inserts it into RasterRamCache, verifies an active lease blocks eviction, releases lease and changes visible ownership, then physically spills through ImageSwapCache. Parent-root saturation makes restore refuse; dropping pressure permits retry. Restored dimensions, color and every f32 bit match the original; GPU-rendered RGBA8 frame bytes are identical before/after. Aliases retain the 131072-byte root credit until last-owner release; one write/one read and zero swap errors are asserted.

Focused test passed without skip. Full HDR raster swap suite passed 9 tests, one externally dependent PICT test ignored. Logs: `research/pq-avif-metal-swap-2026-10-07.log`, `research/pq-avif-metal-swap-suite-2026-10-07.log`. The PQ test uses RGBA8 render readback for transport equality; it does not establish unclamped HDR float readback, physical HDR screen output or NVIDIA execution. Existing separate float RLA HDR readback remains independent evidence and is not substituted for PQ AVIF qualification.

### PQ AVIF float-target Metal readback before/after swap (2026-10-07)

The existing independent PQ AVIF transport test now additionally uploads its managed converted linear source through `RasterRenderer::new_linear_hdr_with_budget`, renders 128x64 into Rgba16Float at +1 exposure stop, and reads every binary16 channel. An explicit 128x64 viewport and zoom 2 cover the fixture. GPU readback contains RGB >128, every alpha is exact binary16 1.0, and decoded peak luminance matches twice the CPU linear peak within 0.25 (binary16 rounding tolerance). A second independent upload/render after physical swap restoration produces bit-identical 8192 RGBA pixels, in addition to the existing exact f32 source comparison and RGBA8 transport check.

Both float render passes ran on actual Metal Apple M4 Max. Each renderer uses a separate GPU source-memory budget of 131072 bytes and its counted credit returns to zero on destruction. Render targets, readback buffers and driver overhead are outside that source-texture accounting; this is not a total GPU peak bound. Focused test passed with no skips. Evidence: `research/pq-avif-float-metal-swap-2026-10-07.log`. This closes unclamped float render/readback evidence for this PQ fixture; physical HDR monitor output, NVIDIA hardware execution, HDR color producer breadth and HLG remain unverified.

### Independent full-color PQ numerical oracle (2026-10-07)

`generate-pq-color-oracle.py` produces 445 committed u16 RGBA/reference-white vectors and linear-sRGB expectations. It uses 60-digit Decimal ST 2084 inverse-transfer arithmetic, exact Fraction Gauss-Jordan matrix inversion and primary/white chromaticities from [ICC BT.2020 registry](https://registry.color.org/rgb-registry/bt2020) and [ICC sRGB registry](https://registry.color.org/rgb-registry/srgb), with D65 shared white. It derives RGB→XYZ and XYZ→RGB matrices independently rather than importing production coefficients. Eight gamut corners, 17 grayscale codes and 64 seeded nonneutral random colors are evaluated at 1/100/203/1000/10000 cd/m² reference whites, with four alpha levels. The generator/oracle is independent numeric qualification, not an external coded HDR producer or monitor measurement.

Native conversion now uses full-precision primary-derived matrix coefficients rather than rounded six-decimal entries. Every output channel matches its independent reference within `1e-7 + abs(reference)*1e-6`, retaining negative gamut components and HDR magnitudes; all 445 managed outputs release credit to zero. Core suite passed 82, zero ignored. Actual Metal Apple M4 Max PQ AVIF float/RGBA8 readback and physical-swap regression passed again after coefficient changes. Logs: `research/pq-color-oracle-generation-2026-10-07.log`, `research/pq-color-oracle-core-2026-10-07.log`, `research/pq-color-oracle-metal-regression-2026-10-07.log`. Full-color coded producer breadth, HLG and physical HDR/NVIDIA qualification remain pending.

### Independent colored PQ AVIF producer and signed float transport (2026-10-07)

Added CC0 synthetic 128x64 RGB16 planar stripes (red, green, blue and mixed 0.5/0.75/0.25 PQ codes), externally converted to full-range BT.2020 YUV420P10 and encoded by FFmpeg 9.0.2/SVT-AV1 4.2.0, CRF 0/preset 8 with explicit CICP 9/16/9 encoder parameters. `generate-pq-avif-fixtures.py` recreates both neutral/colored AVIFs and independently decodes RGBA64LE golden files. Replaying the generator reproduced the colored source and oracle hashes exactly. This does not claim CRF 0 is lossless against authored RGB; native samples are compared to the decoded exported file.

Public-file tests now exercise both sources and every sample; maximum native-vs-independent-FFmpeg error is 63 for grayscale and 116 for color in u16 units, below the unchanged 128 bound. Native output retains Rgba16, exact CICP, opaque alpha, joint managed conversion accounting and release to zero. Colored PQ conversion has components below -1 in linear sRGB.

Both actual Metal Apple M4 Max cases pass RAM eviction/lease/refusal/retry, physical swap, exact restored f32 bits, before/after RGBA8 equality, and complete 128x64 binary16 float readback equality. For the colored file, negative RGB sign bits survive GPU rendering, opaque alpha remains exact and the positive red peak at +1 stop matches CPU peak times two within 0.25. The peak search explicitly filters negative bit encodings rather than treating unsigned half bits as positive magnitudes. Logs: `research/pq-colored-avif-generation-2026-10-07.log`, `research/pq-colored-avif-native-2026-10-07.log`, `research/pq-colored-avif-metal-swap-2026-10-07.log`. HLG, physical HDR screen output and NVIDIA still require qualification.

Current fixture SHA256:
- `avif-pq10-svt.avif`: `724497164e88d67921ab3226df435b155afd556ed6cd3913b0b7a6a7393b5a0e`
- `avif-pq10-svt.avif.rgba16le`: `0ec255dcbf829909ea9c9449aa25cfa400460c924ed354e01b3ed7d98ad014b9`
- `avif-pq10-color-svt.avif`: `1217972d706cd16fadb2318f3dfd04478c5f7fa0b48b59a93d9db3fe9d4305d6`
- `avif-pq10-color-svt.avif.rgba16le`: `5c1f65fae0c2d2a0d23cddc1d2b72e7716107f738e8a0f8796dc9731f6b4fb4d`

### Native explicit HLG display interpretation (2026-10-07)

`hlg_to_linear_srgb_with_budget_and_cancel` accepts CICP BT.2020/HLG (9/18), explicit reference white (1–10000 cd/m²), display peak (1–10000 cd/m²) and system gamma (supported interval 0.1–4). It implements the zero-black reference-display [BT.2100 HLG inverse OETF and luminance-dependent OOTF](https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.2100-2-201807-S%21%21PDF-E.pdf): relative scene RGB is multiplied by peak/white and scene luminance to gamma−1 before the shared full-precision BT.2020→linear-sRGB matrix. Zero scene luminance is handled explicitly, including gamma<1, so black stays zero rather than producing an undefined infinity-times-zero. Output retains HDR and signed gamut components, straight alpha and existing metadata. PQ/HLG declarations are distinguished; an HLG source cannot enter the PQ API.

The shared managed conversion path preserves source/output memory accounting, cancellation checks every 4096 pixels, bounded encoded-domain validation and last-owner release. Tests cover invalid peak/gamma, one-byte-short output admission, cancellation release and black with gamma 0.8. `generate-pq-color-oracle.py --hlg` independently derives 267 full-color Decimal/chromaticity vectors at (white,peak,gamma) = (100,1000,1.2), (203,4000,1.4), (1,100,0.8). All four channels match within 1e-7 + abs(reference)*1e-6 and all root credits return to zero. Existing 445 PQ vectors still pass after conversion-path refactoring. Core suite passed 84 tests, zero ignored; independent neutral/colored PQ AVIF Metal/swap regressions also passed.

Logs: `research/hlg-color-oracle-generation-2026-10-07.log`, `research/hlg-native-core-2026-10-07.log`, `research/hlg-pq-metal-regression-2026-10-07.log`. This is a native library API with a zero-black display contract; HLG producer files, viewer/thumbnail policy/cache identity, HLG GPU/swap qualification, nonzero-black compensation, physical HDR display and NVIDIA remain pending.

### Independently encoded neutral/colored HLG AVIF and Metal transport (2026-10-07)

The external generator now emits both PQ and HLG transfer variants via FFmpeg 9.0.2/SVT-AV1 4.2.0, with explicit encoder CICP 9/18/9 for HLG. Two new committed CC0 128x64 10-bit YUV420 AVIFs carry neutral four-level and nonneutral RGB stripe samples plus independently decoded RGBA64LE references. Public native decoder checks retain exact HLG declaration, Rgba16 precision, opaque alpha and every sample within the unchanged 128-u16 tolerance; observed maxima are 63 for neutral and 116 for color. HLG cannot enter the PQ conversion API. Native HLG conversion at white=100, peak=1000 cd/m², gamma=1.2 preserves color components below −0.1, opaque alpha and joint source/output root accounting with zero after release.

Both HLG fixtures and both prior PQ fixtures pass real Metal Apple M4 Max physical-swap transport. HLG conversion is explicit in this integration test and the coded owner is dropped before RAM insertion, leaving only counted linear output. Lease refusal, eviction, disk write, parent pressure restore refusal/retry, exact restored f32 bits, RGBA8 equality and all 8192 Rgba16Float pixels before/after swap are covered. Signed gamut components survive float rendering; +1-stop positive peaks agree with CPU times two within 0.25. Full HDR swap suite passed 10 tests with one external PICT case ignored. HLG native file test passed without skips. Logs: `research/hlg-avif-producer-generation-2026-10-07.log`, `research/hlg-avif-producer-native-2026-10-07.log`, `research/hlg-avif-metal-swap-suite-2026-10-07.log`. Viewer/preloading/thumbnail HLG policy/cache identity and physical HDR/NVIDIA/nonzero-black qualification remain pending.

HLG fixture SHA256:
- `avif-hlg10-svt.avif`: `ecf1d548702645b8b2d583a7198bf94e7208c16fa8aad37844d78f1da6a47fdf`
- `avif-hlg10-svt.avif.rgba16le`: `0ec255dcbf829909ea9c9449aa25cfa400460c924ed354e01b3ed7d98ad014b9`
- `avif-hlg10-color-svt.avif`: `fa4e8e1cb13ce9c6151ee10ab2500a79e12c492a77f236a6da77ee4dc9a404cf`
- `avif-hlg10-color-svt.avif.rgba16le`: `5c1f65fae0c2d2a0d23cddc1d2b72e7716107f738e8a0f8796dc9731f6b4fb4d`

### HLG viewer/thumbnail/preload interpretation and cache identity (2026-10-07)

CLI now exposes `--hlg-reference-white-nits`, `--hlg-peak-nits`, `--hlg-system-gamma`. Clap requires the complete trio and validates finite white/peak 1–10000 and gamma 0.1–4. RasterInterpretation carries the triple to foreground, neighbours and thumbnails; DecodeRequest's `hlg_display` defaults to None and converts only declared BT.2020/HLG samples through the managed zero-black HLG API. PQ and HLG policies can coexist in a mixed folder and apply only to their declared transfers. All three f32 bit representations are included in display-cache identity alongside existing RLA/PQ choices, while maintaining the supported outer tuple arity.

Actual colored HLG file CLI/cache tests prove peak doubling scales every RGB value by two while alpha remains equal, changing gamma changes samples, and independently changing white/peak/gamma produces four cache entries. Restoring the original complete policy reuses the exact pixel owner. With policy omitted, strict display refusal cannot borrow a differently interpreted cached frame. Dropping all owners releases root credit; 32x16 thumbnail generation with the same HLG policy succeeds; default policy clears a previously set triple; incomplete or invalid CLI combinations refuse.

Full app tests passed 155, 18 ignored; full decoder 788, 47 ignored. Both PQ and HLG independent neutral/colored producer cases also pass real Metal Apple M4 Max RAM/swap/float/RGBA8 transport using public DecodeRequest interpretation, without test-side HLG conversion. Logs: `research/hlg-viewer-cache-thumbnail-2026-10-07.log`, `research/hlg-viewer-cli-regression-2026-10-07.log`, `research/hlg-viewer-request-regression-2026-10-07.log`, `research/hlg-viewer-request-metal-swap-2026-10-07.log`. Physical HDR output, NVIDIA, nonzero-black HLG compensation and remaining producer/format breadth are still pending.

### Direct float CMYK oracle and confirmed pre-ICC precision loss (2026-10-07)

Current Hayro 0.7.0 source `ColorSpace::convert_f32` converts DeviceCMYK operands using `f32_to_u8` before calling the shared fixed-profile ICC transform; `f32_to_u8` is `(val*255+0.5) as u8`. Thus the earlier documentation's implication that native DeviceCMYK enters ICC at float precision was incorrect and has been amended. Parsed PDF operands are float, but the ICC input on this path is quantized. A native dump of the pinned 12x8 opaque CMYK strips was refreshed using the public decoder and managed-memory example.

New external `qualify-pdf-cmyk-float.py` binds LittleCMS 2.19 public APIs and format identifiers, extracts exact authored operand text from the pinned PDF, converts operands to f32, supplies floating CMYK in LittleCMS percentage units and compares RGB8 results. It separately reproduces Hayro's f32→CMYK8 rounding before independent ICC conversion, using identical pinned CGATS001Compat-v2-micro profile bytes. For intents 0–3, direct float input differs from native at 64/96 pixels with max error 3; reproduced CMYK8 input differs at 32/96 with max 1. Source/profile/native/library hashes, normalized input values and reference first rows are recorded in `research/pdf-cmyk-float-littlecms-2026-10-07.json`; native dump log: `research/pdf-cmyk-native-dump-2026-10-07.log`. Float CMYK format follows the [public LittleCMS header](https://github.com/mm2/Little-CMS/blob/lcms2.19/include/lcms2.h).

This confirms measurable pre-ICC precision loss for this fixture and identifies the next renderer change: use the existing float ICC transform instead of the DeviceCMYK u8 shortcut. It does not establish that this accounts for all AI differences or remove the Poppler gate; final RGB output remains quantized, and masks, blends, rendering intent and path coverage remain separately unqualified. No renderer behavior has yet been changed by this diagnostic.

### Rejected float-ICC renderer experiment (2026-10-07)

A temporary repository-local hayro-interpret patch bypassed both DeviceCMYK convert_f32 quantization and the separate ToRgb::convert_sample u8 preference. The first change alone produced identical pixels because vector fills used the second shortcut. With both bypassed, the independent same-profile LittleCMS float oracle still differed by up to 3 RGB8 levels, now at 72/96 pixels (baseline 64/96). Against the reproduced CMYK8 oracle, the patched result differed at 8/96 pixels by at most 1. Thus invoking the existing moxcms float executor does not establish the expected direct-float ICC behavior and does not improve this qualification fixture. The patch was rejected and production dependencies/behavior restored.

The reproducible rejected diff is `research/pdf-cmyk-float-rejected-renderer-patch-2026-10-07.diff`; independent report is `research/pdf-cmyk-float-patched-littlecms-2026-10-07.json`. The next investigation must compare ICC executor/profile interpolation and input normalization directly, rather than assume the float method name proves precision. This qualifies neither AI nor EPS and leaves the strict Poppler gate unchanged.

### ICC discrepancy isolated from PDF rasterization (2026-10-07)

New diagnostic example `cmyk_transform_probe` calls public moxcms 0.8.1 float and byte executors directly with the same pinned profile and independently recorded PDF operands. It does not invoke Hayro, parse a PDF, rasterize geometry or composite pixels. Direct float output equals the rejected float-patched PDF byte-for-byte for all 96 opaque pixels; direct byte output equals current production PDF byte-for-byte. Independent LittleCMS comparison reproduces the float discrepancy (max 3, 72/96 pixels), ruling out PDF geometry/compositing as its cause for this exact fixture. This is evidence to investigate ICC profile interpretation/interpolation/normalization, not a blanket diagnosis of other AI differences.

Probe inputs and unrounded float RGB outputs: `research/pdf-cmyk-direct-moxcms-2026-10-07.log`. Independent report: `research/pdf-cmyk-direct-float-littlecms-2026-10-07.json`. Production runtime remains unchanged.

### ICC interpolation/precision option matrix (2026-10-07)

The direct diagnostic now exports all 16 combinations of moxcms Linear/Tetrahedral/Pyramid/Prism interpolation, preferred fixed-point on/off, and Low/High barycentric weights. The `options` feature is enabled only as a development dependency for this example, not the production decoder dependency. Comparing the 12 authored samples to the pinned LittleCMS float reference: default Linear/fixed/Low max error is 3 at 9/12 samples; Tetrahedral/Low yields max 2 at 7/12 samples for either fixed preference. High weights do not consistently improve agreement: Tetrahedral/no-fixed/High differs at 5/12 but max 3; other High configurations reach max 4. None matches the independent reference exactly.

This establishes that interpolation choice affects this discrepancy, but neither a precision toggle nor the available tetrahedral option fully resolves it. No production policy is changed or gate relaxed. Logs with every unrounded RGB and option: `research/pdf-cmyk-moxcms-options-2026-10-07.log`; comparison: `research/pdf-cmyk-moxcms-options-comparison-2026-10-07.json`. Further qualification needs ICC table/curve execution comparison and broader CMYK vectors, not adoption based on this one strip.

### Independent ICC oracle optimization audit (2026-10-07)

The external LittleCMS diagnostic now tests default and public cmsFLAGS_NOOPTIMIZE (0x0100), each with RGB8 and float RGB output (the latter rounded explicitly afterward). Across intents 0–3, all four direct-float CMYK reference variants are identical at RGB8 precision, preserving the direct moxcms discrepancy of max 3 and 72/96 differing pixels. Thus the float comparison is stable under these oracle implementation choices. Byte-CMYK references are not stable: disabling optimization changes the reference and its difference from direct moxcms float rises to max 4 at 32/96 pixels, versus max 1 at 8/96 under the optimized byte oracle. Earlier closeness to the optimized byte oracle does not prove equivalent float input quantization inside moxcms.

Report: `research/pdf-cmyk-littlecms-nooptimize-2026-10-07.json`. Public flags/formats: [LittleCMS 2.19 header](https://raw.githubusercontent.com/mm2/Little-CMS/lcms2.19/include/lcms2.h). Production behavior remains unchanged; investigate profile stages/interpolation against the stable float oracle.

### Pinned CMYK ICC stage inventory (2026-10-07)

Binary inspection of the SHA-pinned 8464-byte profile identifies ICC 2.1 scanner-class CMYK→Lab PCS with one A2B0 `mft2` stage, four input and three output channels, identity 3×3 matrix, 48-entry input curves per channel, a 6×6×6×6 CLUT, and two-entry identity output curves. A2B1/A2B2 are absent; separate rendering-intent tables do not exist in this profile. Input curves are not identity: their maximum normalized identity deviations are approximately 0.05615, 0.01324, 0.01941 and 0.04296. Full input curve samples, tag offsets/types and header metadata are recorded in `research/pdf-cmyk-icc-profile-structure-2026-10-07.json`.

The next stage-level comparison should isolate CMYK→Lab before destination sRGB, including nonlinear input curve sampling, four-dimensional CLUT interpolation and ICC v2 Lab normalization. The identity matrix/output curves and nonexistent per-intent tables need not be treated as hypothetical causes for this exact profile. This structural inspection does not prove which remaining stage causes the numeric discrepancy.

### Raw A2B0 stage independent evaluation (2026-10-07)

New external `probe-cmyk-icc-stage.py` independently evaluates the pinned 48-entry input curves with linear interpolation and all 16 corners of the 4D six-node CLUT using Python floating arithmetic. It compares all 12 authored CMYK samples against LittleCMS public cmsReadTag(A2B0)/cmsPipelineEvalFloat, without destination sRGB, physical Lab unit conversion or RGB quantization. Maximum normalized Lab component difference is 1.3209851023265884e-05. Full shaped inputs and both stage outputs are in `research/pdf-cmyk-a2b0-stage-2026-10-07.json`, with profile/library hashes.

This verifies the independent quadlinear interpretation of the source stage against LittleCMS for these samples and provides intermediate oracle values. It does not yet compare moxcms intermediate Lab output or prove that the small Lab difference cannot amplify near gamut boundaries. Next isolate moxcms source-stage execution and ICC v2 Lab normalization/destination conversion against these recorded intermediates. Production rendering remains unchanged.

### Public moxcms CMYK→Lab probe limitation (2026-10-07)

The direct diagnostic now attempts the pinned source profile→ColorProfile::new_lab() with Layout::Rgba→Layout::Rgb for each of the 16 options. All return UnsupportedProfileConnection from moxcms 0.8.1. The example records the per-variant error while retaining successful RGB diagnostics, rather than fabricating intermediate Lab values or failing before the report. Successful diagnostic execution and all 16 errors are recorded in `research/pdf-cmyk-moxcms-lab-2026-10-07.log`.

This public transform cannot expose the intermediate oracle needed for source-stage comparison. Next use diagnostic-only instrumentation of internal ICC pipeline stages, keeping production dependencies unchanged until an independently validated correction exists. This is a limitation of the attempted public diagnostic connection, not a conclusion that CMYK PDF decoding is unsupported or an impasse for the broader goal.

### Composite CMYK→RGB grid density experiment (2026-10-07)

Source tracing found that moxcms 0.8.1 default CMYK→sRGB path precomputes the whole source curves/CLUT/Lab/destination conversion into a 17^4 RGB grid and interpolates the resulting RGB values. An isolated /tmp crate copy changed only this GRID_SIZE from 17 to 33; repository dependencies were untouched. Direct float qualification improves from max 3 at 72/96 pixels to max 1 at 40/96, under the unchanged stable float LittleCMS oracle. This demonstrates grid density contributes to the discrepancy for this fixture, beyond the earlier Hayro single-sample CMYK8 rounding.

Raw RGB grid storage increases from 17^4×3×4 = 1,002,252 bytes to 33^4×3×4 = 14,231,052 bytes, before other construction/runtime allocations. A production change needs construction/throughput/memory measurements and broader nonneutral/edge CMYK qualification, or a stage-executed alternative avoiding the composite RGB approximation. Reports: `research/pdf-cmyk-grid33-probe-2026-10-07.log`, `research/pdf-cmyk-grid33-littlecms-2026-10-07.json`; exact isolated patch: `research/pdf-cmyk-grid33-experiment-2026-10-07.diff`. No production policy or strict AI gate is changed.

### Composite grid performance tradeoff (2026-10-07)

Diagnostic warm-process benchmark on macOS arm64 uses 262144 deterministic full-domain CMYK samples, 16 transform repetitions and five fresh transform constructions, opt-level 3. Median default-grid 17 build is 3.292 ms and throughput 68.386 Mpix/s; isolated grid 33 build is 31.400 ms and throughput 56.434 Mpix/s. Grid 33 uses a separate build directory after a shared-target trial incorrectly reused a same-version local dependency artifact; that trial is excluded. Final output checksums differ, as expected from the changed grid, and the baseline source file is byte-identical to the registry source. These are transform-only timings, excluding PDF load/rasterization and physical GPU display. Raw grid estimates remain 1.00 MB versus 14.23 MB, excluding temporary construction arrays and allocator overhead.

Logs: `research/pdf-cmyk-grid17-benchmark-2026-10-07.log`, `research/pdf-cmyk-grid33-benchmark-2026-10-07.log`; comparison JSON: `research/pdf-cmyk-grid-benchmark-comparison-2026-10-07.json`; standalone benchmark source: `scripts/cmyk-grid-benchmark.rs` (dependencies moxcms 0.8.1 with options/extended_range and serde_json; isolated grid patch already recorded). Grid 33 improves the measured strip accuracy but costs about 9.5× construction time, 17.5% throughput and 14.2× raw grid memory. This motivates a stage-executed source ICC path rather than blindly increasing the production composite table. Broader color accuracy, actual load latency and managed intermediate allocation still require qualification.

### Stage-executed CMYK matches independent float oracle (2026-10-07)

An isolated moxcms 0.8.1 copy enabled any_to_any and forced the existing Katana source-stage route for the CMYK→RGB branch, avoiding the precomposed 17^4 RGB LUT. Direct float output matches the independent LittleCMS float reference exactly at RGB8 precision for all 96 strip pixels; all 16 precision/interpolation options yield zero maximum error on the 12 source colors. This supplies a concrete correction candidate to the earlier composite-grid discrepancy. Production remains unmodified and Hayro float/single-sample quantization still needs removal when integrating the candidate.

Same deterministic benchmark: median build 1.154 ms, throughput 43.766 Mpix/s, versus composite17 3.292 ms/68.386 Mpix/s and composite33 31.400 ms/56.434 Mpix/s. The staged path improves preparation and measured accuracy but costs about 36% transform throughput versus composite17. It does not allocate the composite RGB grid; actual retained/temporary allocation accounting has not yet been measured. Broad seeded CMYK/gamut-edge qualification, vector-fill latency and PDF/AI regressions must precede production adoption.

Exact patch: `research/pdf-cmyk-staged-experiment-2026-10-07.diff`; option/sample log: `research/pdf-cmyk-staged-probe-2026-10-07.log`; independent report: `research/pdf-cmyk-staged-littlecms-2026-10-07.json`; benchmark: `research/pdf-cmyk-staged-benchmark-2026-10-07.log`. Build uses isolated manifest with moxcms path dependency and options/extended_range/any_to_any, serde_json, opt-level 3, separate target directory.

### Broad CMYK corpus rejects immediate staged-path adoption (2026-10-07)

External `generate-cmyk-icc-corpus.py` creates 625 regular full-domain CMYK grid points plus 4096 seeded random points (seed 20261007), converts normalized f32 input to LittleCMS percentage units and exports unoptimized perceptual float RGB using the same pinned profile. Standalone `cmyk-corpus-probe.rs` executes the isolated forced-Katana moxcms path on exactly these 4721 inputs. Though the original strip matched, this broader corpus reaches max 7 RGB8 error at 2969 differing pixels, clamping both outputs to [0,1] before rounding. Raw float maximum 2.6934 includes gamut/clipping policy differences and must not be mistaken for normalized display error.

Report with hashes and 16 worst-case source/reference/native vectors: `research/pdf-cmyk-staged-corpus-2026-10-07.json`. Immediate production adoption is rejected; diagnose these nonneutral/high-ink cases and destination/gamut handling first. This demonstrates why the prior 12-color success was insufficient qualification. Production dependencies remain unchanged and the strict AI gate remains active.

### Broad ICC candidate versus baseline comparison (2026-10-07)

The same 4721-point independently generated corpus now compares direct moxcms composite17, composite33 and forced stage execution. Composite17 max RGB8 error is 16, mean absolute per-channel error 0.57170, with 921 pixels above one level and 232 above three. Composite33 max is 8, mean 0.48874, 721 above one and 149 above three. Staged max is 7, mean 0.40556, 620 above one and 116 above three; 1248 samples improve but 322 worsen against composite17 using each pixel's maximum channel error. Thus the stage candidate reduces aggregate error by about 29%, but is not a uniform correction. Actual prefer_fixed_point=false plus allow_extended_range_rgb_xyz=true produces byte-identical float output to the staged default for this corpus.

Report: `research/pdf-cmyk-broad-path-comparison-2026-10-07.json`. Each isolated library uses a separate target directory, with output hashes recorded to detect accidental dependency artifact reuse. Comparisons cover direct float execution, not Hayro's quantized production single-fill path. Diagnose the 322 regressions before adoption; no acceptance gate is weakened.

### Broad source-stage discrepancy localized (2026-10-07)

Diagnostic-only instrumentation in the isolated moxcms copy exports KatanaLut4x3::to_pcs_impl raw normalized Lab before v2/v4 scaling and destination conversion. For all 4721 samples, moxcms agrees with independent Python input-curve/quadlinear CLUT evaluation within 1.4166e-7, but differs from LittleCMS raw A2B0 pipeline by up to 0.01295072. The earlier 12-strip stage agreement does not generalize to the full CMYK domain. This locates a discrepancy in source-stage interpolation, before destination sRGB/gamut treatment. moxcms forcibly chooses quadlinear for Lab/XYZ PCS even when a tetrahedral option is selected; alternative source interpolation is the next targeted experiment.

Comparison with 16 worst cases: `research/pdf-cmyk-broad-pcs-stage-comparison-2026-10-07.json`; exact isolated PCS dump instrumentation: `research/pdf-cmyk-pcs-instrumentation-2026-10-07.diff`. Generate the same corpus, use its normalized input rows with `probe-cmyk-icc-stage.py`, and compare the diagnostic f32 little-endian PCS dump. Production code remains unchanged.

### Four-input ICC interpolation method identified (2026-10-07)

Forcing moxcms Hypercube::tetra_vec3 at the source stage still leaves max RGB8 7 and max normalized source Lab discrepancy 0.0128276 (2525 differing RGB8 pixels), so generic four-dimensional tetrahedral interpolation is not the required method. LittleCMS 2.19 [Eval4InputsFloat source](https://raw.githubusercontent.com/mm2/Little-CMS/lcms2.19/src/cmsintrp.c) linearly interpolates along the first input C between two three-dimensional tetrahedral evaluations over M/Y/K. The external stage probe now independently implements this hybrid using sorted fractional coordinates and four barycentric vertices within each section.

Across all 4721 samples, independent hybrid evaluation agrees with LittleCMS raw A2B0 within max 2.42257e-5 normalized Lab, versus quadlinear max 0.0129507. Mean per-sample maximum error is 8.31156e-6. Report: `research/pdf-cmyk-hybrid-stage-comparison-2026-10-07.json`; rejected generic tetra comparison: `research/pdf-cmyk-source-tetra-comparison-2026-10-07.json`. Next implement the hybrid in the isolated Rust source stage and qualify final RGB, speed and allocations before integrating. This does not yet change production behavior.

### Rust hybrid source-stage candidate broad qualification (2026-10-07)

The isolated Rust stage now implements first-dimension linear interpolation between two 3D tetrahedral sections directly over the original CLUT. Full 4721-point corpus reaches max 1 RGB8 difference, 161 differing pixels, mean per-channel RGB8 error 0.01136765; raw Lab stage matches LittleCMS within 2.42293e-5. This reduces mean error by about 98% from composite17's 0.57170 and eliminates all errors above one on this profile/corpus. The max-one agreement is evidence, not a universal-format or universal-profile gate.

Five-repeat benchmark median preparation is 1.183 ms and throughput 20.106 Mpix/s. The straightforward implementation uses sorted fractional coordinates and eight source-table vertices, with no composite RGB grid; it is slower than the prior quadlinear stage (43.766 Mpix/s) and composite17 (68.386 Mpix/s). Optimization can specialize the three-coordinate ordering and reduce scalar indexing, but must retain the independent oracle. Next integrate a narrowly scoped float DeviceCMYK candidate into the PDF renderer, run actual PDF/AI qualification and measure single-fill/load behavior before any broad policy change.

Clean candidate patch without PCS dump: `research/pdf-cmyk-hybrid-rust-candidate-2026-10-07.diff`, applied together with the prior forced-stage diagnostic patch and any_to_any feature; report: `research/pdf-cmyk-hybrid-rust-comparison-2026-10-07.json`; benchmark: `research/pdf-cmyk-hybrid-rust-benchmark-2026-10-07.log`. Production dependencies remain unchanged.

### Production DeviceCMYK float-fill integration (2026-10-07)

Repository-owned, license-preserving moxcms 0.8.1 and hayro-interpret 0.7.0 patches now integrate the qualified hybrid stage. New TransformOptions::rrrah_cmyk_hybrid defaults false and selects source-stage float execution plus hybrid interpolation only when explicitly enabled. Hayro's fixed DeviceCMYK float-fill transform enables it, bypassing both pre-ICC operand rounding and the single-sample u8 preference; existing byte image conversion remains its separate path. Other ordinary ICC transforms retain the default option. any_to_any dependency capability is enabled, so full decoder regression was run.

Public PDF decode of the pinned 12×8 CMYK strips matches LittleCMS float reference exactly for every RGBA8 channel. Committed independent golden `tests/fixtures/pdf/cmyk-float-littlecms.rgba` and native regression test prove dimensions, pixels and zero managed root usage after last owner release. Focused PDF suite: 15 passed; full decoder library: 789 passed, 47 external cases ignored. Logs: `research/pdf-cmyk-integrated-regression-2026-10-07.log`, `research/pdf-cmyk-full-decode-regression-2026-10-07.log`; independent report: `research/pdf-cmyk-integrated-littlecms-2026-10-07.json`. Vendor provenance/local patch notes and upstream licenses are retained. ICC internal transform allocations remain outside managed pixel accounting and still need budget qualification.

Strict actual Adobe AI Poppler comparison still fails 2/3 pages: VectorApple 33066 differing pixels (max 213), one.ai page0 2790 (max 115), page1 exact. Report: `research/ai-cmyk-integrated-poppler-2026-10-07.json`. Thus this corrects a measured DeviceCMYK conversion loss, not full AI qualification; private/PostScript AI, EPS, other rendering differences and overall 100-format/GPU objective remain open.

### CMYK PDF viewer RAM/swap/Metal qualification (2026-10-07)

The existing PDF readback integration suite now includes the corrected 12×8 DeviceCMYK strip PDF against the committed independent LittleCMS RGBA golden. Golden pixels are correctly interpreted as sRGB and converted to linear before rendering; the older endpoint-only cases had hidden a test-side assumption of already-linear byte values. Four independent-reference PDF cases now compare actual GPU frames before/after RAM lease use and native/prepared physical swap, with exact source bytes/restored float bits, selected-page/color identity, parent-budget saturation restore refusal/retry and zero root credit after release.

Actual Metal qualification passed the targeted four-case test and the full PDF readback suite (2 passed, 1 external-source case ignored), with explicit GPU requirement rather than silent skip. Logs: `research/pdf-cmyk-ram-swap-metal-2026-10-07.log`, `research/pdf-cmyk-metal-full-regression-2026-10-07.log`. This covers CMYK float-fill transport on this fixed-profile sample, not PostScript AI/EPS, unknown CMYK profiles, physical HDR output, NVIDIA or full AI pixel conformance.

### Native hybrid coordinate-order optimization (2026-10-07)

Replaced general-purpose sorting of three M/Y/K fractional coordinates with a stable three-comparison sorting network in the repository-owned hybrid ICC stage. The production opt-in path emits byte-identical f32 output to the previous hybrid implementation for all 4721 independently generated corpus points, retaining max-one RGB8 oracle agreement. Targeted public PDF independent-golden regression passes after recompilation.

Same warm-process five-repeat benchmark: median 23.074 Mpix/s versus prior 20.106 (about 14.8% faster), preparation 1.165 ms. Both checksums are identical. This is CPU transform throughput, not whole-viewer load latency or GPU throughput. Separate standalone production-library target enables rrrah_cmyk_hybrid explicitly; scripts/cmyk-corpus-probe.rs and cmyk-grid-benchmark.rs supply the workload. Reports: `research/pdf-cmyk-hybrid-optimized-benchmark-2026-10-07.log`, `research/pdf-cmyk-hybrid-optimized-comparison-2026-10-07.json`. Remaining AI/PDF rendering gaps and overall objective are unchanged.

### Committed full-domain CMYK regression oracle (2026-10-07)

The independently generated 4721-point corpus is now committed as normalized f32 little-endian CMYK inputs plus clamped/rounded LittleCMS RGB8 references. Metadata records 625 regular-grid samples, 4096 seeded samples, profile/library/input/reference hashes, perceptual intent, unoptimized float oracle, exact quantization and CC0 synthetic numeric provenance. A native decoder-library test executes the repository-owned opt-in hybrid ICC transform and checks every channel against this independent reference with maximum one RGB8 level, finite output and exact fixture extents. It cannot pass merely by matching the original strip.

Both targeted DeviceCMYK tests pass without ignores: full-domain transform and exact public PDF decode/memory-release reference. Log: `research/pdf-cmyk-full-domain-native-regression-2026-10-07.log`; fixtures: `tests/fixtures/pdf/cmyk-full-domain-input.f32le`, `cmyk-full-domain-littlecms.rgb8`, `cmyk-full-domain-reference.json`. The one-level allowance is specific to this independently measured profile corpus and does not relax the strict Poppler AI gate. Other CMYK profiles and final AI renderer differences remain unqualified.

### Shared ICC dependency cross-library regression (2026-10-07)

After production hybrid integration and coordinate-order optimization, locked core/cache/decode/GPU/app library, binary and integration suites all completed successfully: 1286 passed, 0 failed, 107 ignored across 56 suites. This includes existing real Metal compute/readback, HDR producer transport, RAM/swap/TTL and decoder regressions. External-source/hardware tests retain their explicit skip constraints; this does not prove physical HDR or NVIDIA behavior, EPS/PostScript AI support or exact AI conformance. The 100-format catalog PDF row now reflects the independently qualified DeviceCMYK subset and remaining breadth.

Full log: `research/cmyk-integration-cross-library-regression-2026-10-07.log`; parsed counts/scope: `research/cmyk-integration-cross-library-summary-2026-10-07.json`. No overall completion claim follows from these regression passes.

### Single DeviceCMYK fill RGB buffer allocation removed (2026-10-07)

Hayro's corrected single-color DeviceCMYK path now uses a three-float stack output instead of allocating a temporary RGB Vec for each vector fill. Batched output retains the existing vector path; the ICC executor's internal stage scratch remains separately allocated and unaccounted. Exact independent PDF and 4721-point native ICC tests both pass. Log: `research/pdf-cmyk-single-fill-buffer-regression-2026-10-07.log`. This removes a source-visible allocation; no whole-PDF latency improvement is claimed without a dedicated measurement. Remaining RAW routes such as MEF still require real format/color evidence and are not bypassed.

### Independent CMYK fixture regeneration check (2026-10-07)

Fresh generation from the recorded seed/grid and pinned ICC profile through external LittleCMS reproduces all 4721 committed normalized input vectors and RGB8 references byte-for-byte, including provenance hashes. The generator now exposes --verify-fixtures to fail on an input/reference/hash mismatch without consulting native renderer output. This verifies the regression golden is reproducible independent evidence, rather than a frozen native result. Reports: `research/pdf-cmyk-corpus-reproduction-2026-10-07.json`, `research/pdf-cmyk-corpus-reproduction-2026-10-07.log`. Broader profiles, byte-image ICC paths and unresolved formats remain separately pending.

### Embedded CMYK8 PDF image discrepancy reproduced (2026-10-07)

External corpus generator now supports --quantized-input, producing normalized f32 coordinates at exact CMYK8 code values before independent LittleCMS float conversion. An authored PDF image XObject contains all 4721 grid/seeded code tuples as one opaque 4721×1 DeviceCMYK8 image, with Interpolate false and exact one-source-pixel/page-point mapping. Current public PDF decode differs from the independent RGB8 golden at 3015 pixels, with maximum channel error 7. A preliminary 16-pixel grid slice had only max-one error and was insufficient; the committed fixture/reference contains the full corpus.

Fixtures: `tests/fixtures/pdf/cmyk-image8.pdf`, `cmyk-image8-littlecms.rgba`; report with hashes/dimensions/scope: `research/pdf-cmyk-byte-image-baseline-2026-10-07.json`. This confirms the still-separate DeviceCMYK byte-image path needs stage-based precision qualification too. Source codes are already bytes, so preserving float PDF operands is not the relevant issue here. Next compare the native hybrid executor at these exact normalized byte codes and integrate the byte-image path if verified, with image memory/latency and full PDF regression. No byte-image production behavior changed in this investigation.

### Embedded DeviceCMYK8 PDF image correction (2026-10-07)

DeviceCMYK convert_u8 now normalizes exact byte code values and invokes the qualified original-stage hybrid float transform. It validates four-input/three-output lane correspondence and processes at most 256 pixels per call with fixed stack scratch (1024 CMYK floats + 768 RGB floats = 7168 bytes), avoiding a whole-image normalized-float allocation. Executor-internal stage allocation remains a separate bounded-per-block allocation and still needs explicit budget integration.

The public decoder regression processes the entire 4721×1 authored CMYK8 image, including the last partial block, against the independent byte-code LittleCMS reference: every RGB channel within one level, alpha exact, dimensions exact and root memory zero after final owner release. This replaces baseline max-seven/3015-differing-pixel evidence. All 17 PDF tests pass with no ignores. Log: `research/pdf-cmyk-byte-image-regression-2026-10-07.log`. Batch throughput/load latency, GPU readback for this image case and broader ICC/mask/JPEG/color profiles remain pending; no full AI or overall completion claim.

Full decoder library regression after the CMYK8 correction: 791 passed, 47 external cases ignored, no failures. Log: `research/pdf-cmyk-byte-full-decode-regression-2026-10-07.log`.

### Embedded CMYK8 image RAM/swap/Metal qualification (2026-10-07)

The independent-reference PDF viewer test now includes the 4721×1 CMYK8 image. It checks native source channels within one of LittleCMS, compares the displayed Metal frame within one, then requires byte-exact GPU frame identity through RAM leases and native/prepared swap restore, with exact source bytes or float bits, pressure-induced restore refusal/retry and zero root credit after final ownership release. The target spans the source width and the image is aligned vertically to a screen pixel center. A mandatory >128 distinct rendered RGB colors check prevents blank-frame false positives.

An initial width-fit run placed the subpixel-height strip between pixel centers and falsely yielded equal blank frames; the content-presence assertion caught it. That result is excluded. The corrected run on Metal Apple M4 Max shows actual independent frame max error 1 and passes content presence plus transport checks. Full PDF readback suite: 2 passed, 1 external case ignored. Log: `research/pdf-cmyk-byte-ram-swap-metal-2026-10-07.log`. No general GPU/HDR/NVIDIA or full AI conformance claim follows.

### Public CMYK8 PDF load/block-size measurement (2026-10-07)

New `pdf_cmyk_load_bench` runs public decode_raster (including cached filesystem read) plus public display preparation, verifies zero managed bytes after every iteration and records root peak. Five repeats of 32 iterations on the 4721×1 source compare 256 versus 1024-pixel CMYK normalization blocks. Median warm decode is 0.248724 versus 0.242159 ms; display preparation 0.017226 versus 0.017220 ms; first process decode 3.242 versus 3.209 ms includes ICC first use, not cold disk I/O. Managed peak is 94420 bytes and final zero for both. Stack scratch rises from 7168 to 28672 bytes; ICC/external parser internals and stack bytes remain outside managed root accounting.

The larger block offers only about 2.6% warm decode difference in this small-file trial for fourfold stack scratch, so production was restored to 256. No statistically established speedup or full-viewer/camera-file latency is claimed. Logs: `research/pdf-cmyk-byte-load-block256-2026-10-07.log`, `research/pdf-cmyk-byte-load-block1024-2026-10-07.log`; comparison: `research/pdf-cmyk-byte-block-size-comparison-2026-10-07.json`. Benchmark example preserves repeat counts, scope and parent-credit checks for larger producer inputs.

### Embedded CMYK JPEG APP14/Decode inversion qualification (2026-10-07)

A synthetic CC0 64×8 eight-color CMYK JPEG is independently encoded/decoded with Pillow 12.3/libjpeg at quality 100 without chroma subsampling. Its decoded CMYK codes feed external LittleCMS float ICC conversion with the pinned profile. The JPEG is embedded unchanged in a PDF DCTDecode image with explicit Decode [1 0] for all four channels, matching Adobe APP14 inverted sample convention. Native zune-based PDF decode matches all 512 independent RGBA pixels exactly. The golden is based on independently decoded compressed codes, not the pre-JPEG authored colors.

New public decoder test checks dimensions, exact bytes and zero managed root after release. Source JPEG/PDF, LittleCMS RGBA golden and provenance metadata are committed under `tests/fixtures/pdf/cmyk-jpeg-app14*`; report: `research/pdf-cmyk-jpeg-app14-comparison-2026-10-07.json`. External corpus generator --input-cmyk8 supports numeric oracle creation from independently decoded JPEG bytes. YCCK, ColorTransform overrides, malformed APP14, arbitrary profiles and producer breadth remain pending; this does not establish whole JPEG/PDF or AI conformance.

### Independently encoded YCCK JPEG PDF qualification (2026-10-07)

External fixture-only `generate-ycck-jpeg.c` uses libjpeg-turbo public API with JCS_CMYK input, JCS_YCCK output, quality 100 and 1×1 sampling for all channels. The emitted JPEG has Adobe APP14 transform 2, verified from actual marker bytes. Pillow independently decodes CMYK values; external LittleCMS float ICC supplies final RGBA reference. JPEG bytes are embedded unchanged in a PDF with four inverted Decode pairs. Native PDF output agrees within max one RGB8 level (64/512 pixels differ), with exact alpha.

New public native regression checks all channels, extent and last-owner managed release. Synthetic CC0 JPEG/PDF/golden/provenance files: `tests/fixtures/pdf/ycck-jpeg-app14*`; comparison: `research/pdf-ycck-app14-comparison-2026-10-07.json`. This qualifies one explicit APP14 YCCK/Decode case; ColorTransform overrides, subsampling, malformed markers, arbitrary ICC profiles and independent producer breadth remain pending. No production JPEG decoder change was required by this fixture.

### Standalone profiled CMYK JPEG channel-loss reproduction (2026-10-07)

The independently decoded APP14 CMYK JPEG now also has a standalone variant with the exact pinned 4-channel ICC profile inserted as one standard APP2 ICC_PROFILE segment without JPEG recompression. Independent Pillow CMYK decoding confirms all compressed channel codes remain byte-identical. Public standalone native output differs from the existing LittleCMS RGB oracle at 448/512 pixels, max RGB8 115; public display preparation fails UnsupportedProfileSpace. The ordinary image JPEG decoder projects source CMYK into RGB before its ICC profile is attached, so the retained four-channel profile no longer matches available samples. These are not equivalent to the corrected PDF CMYK/JPEG paths.

Reproduction source: `tests/fixtures/raster/cmyk-app14-profiled.jpg`; report with source/profile hashes: `research/standalone-cmyk-jpeg-baseline-2026-10-07.json`. Next introduce original-channel JPEG CMYK decoding and ICC application before RGB projection, with managed source/output overlap and explicit unprofiled interpretation. No fabricated ICC fallback or goal-completion claim is made.

### 2026-10-07: standalone profiled CMYK JPEG channel preservation

The standalone JPEG path now preserves four CMYK channels through the embedded
ICC transform rather than applying a CMYK profile to an already converted RGB
image. Qualified source: `tests/fixtures/raster/cmyk-app14-profiled.jpg`, Adobe
APP14 transform 0, 64 x 8. Public native decode matches every RGBA8 sample in the
independent Pillow/libjpeg + LittleCMS float oracle
`tests/fixtures/pdf/cmyk-jpeg-app14-littlecms.rgba` (512 pixels, maximum error 0).
The previous standalone baseline had maximum RGB8 error 115 and failed display
preparation with UnsupportedProfileSpace. Converted output is tagged sRGB; the
source CMYK profile is consumed during conversion. Source and output buffers
coexist and source capacity is separately admitted to the supplied memory budget.
Codec/ICC scratch is not fully covered by that budget. This does not qualify all
CMYK profiles, standalone YCCK, arbitrary JPEG metadata, or all 100 formats.

### 2026-10-07: standalone profiled YCCK JPEG

The same profile-preserving route now admits Adobe APP14 transform 2. Native
JPEG decoding retains Y/Cb/Cr/K samples; a bounded full-range fixed-point color
conversion reconstructs CMY, with Adobe K inversion, before ICC processing.
`tests/fixtures/raster/ycck-app14-profiled.jpg` matches all 512 RGBA8 pixels in
`tests/fixtures/pdf/ycck-jpeg-app14-littlecms.rgba` exactly. This oracle uses
independent libjpeg/Pillow decoded CMYK samples and LittleCMS float conversion.
The source is produced by `scripts/generate-ycck-jpeg.c`; add the pinned CGATS
profile with `scripts/attach-jpeg-icc.py` without recompressing samples. Both
profiled JPEG variants have public decode/display-preparation/managed-release
regression checks. Wider subsampling, profiles, orientations and actual viewer
RAM/swap/Metal qualification remain open for standalone profiled JPEG.

### 2026-10-07: profiled CMYK/YCCK JPEG RAM, swap and actual Metal

`rrrah` integration test
`independently_referenced_pdfs_and_profiled_jpegs_match_through_ram_swap_and_metal`
now includes both independently referenced standalone profiled JPEG fixtures.
Actual adapter: Metal Apple M4 Max. Both decoded samples and the complete 96 x 64
rendered frame match the independent JPEG/LittleCMS RGBA oracle exactly. A color
count assertion requires the stripes to appear, preventing a blank-frame pass.
RAM leases preserve the frame when cache capacity is reduced to zero. Native
RGBA8 and prepared linear RGBA32F streamed swap restorations preserve bytes or
float bits, color metadata and the full frame exactly. Exhausting the shared
root budget refuses restoration without recording a corrupt-entry error;
releasing pressure permits retry. Each case ends with zero managed bytes.
Log: `docs/research/profiled-jpeg-ram-swap-metal-2026-10-07.log`.
This proves these two fixtures through the tested offscreen viewer rendering
pipeline, not physical display/HDR or arbitrary JPEG/profile qualification.

### 2026-10-07: asymmetric profiled JPEG EXIF orientation qualification

Sixteen independently referenced cases cover all eight EXIF orientations for
both CMYK APP14 transform 0 and YCCK APP14 transform 2 with the pinned ICC
profile. The 17 x 13 source varies along both axes; orientations 5–8 have 13 x 17
output. External libjpeg/Pillow decoded CMYK samples feed LittleCMS float ICC;
Pillow EXIF transpose supplies independent pixel placement. Public decoder
comparison checks all 221 pixels in every case; maximum channel error is one
RGB8 level, exact alpha. Managed ownership returns to zero after every decode.
`jpeg_cmyk::tests::asymmetric_profiled_jpegs_preserve_all_eight_exif_orientations`
is a permanent regression with tolerance two. Provenance hashes are in
`tests/fixtures/raster/profiled-jpeg-orientation-reference.json`; measured
per-case results are `docs/research/profiled-jpeg-orientations-2026-10-07.json`.
Fixture tools: `generate-ycck-jpeg.c --asymmetric`,
`generate-cmyk-icc-corpus.py --input-cmyk8`, and
`generate-profiled-jpeg-orientations.py`. Other profiles, subsampling and physical
presentation remain unqualified; this is not completion of the JPEG family.

### 2026-10-07: all profiled JPEG orientations through RAM/swap/Metal

The actual Metal integration now exercises all sixteen asymmetric CMYK/YCCK
EXIF orientation fixtures in addition to the existing seven PDF/JPEG cases.
Every full 96 x 64 frame is compared against the independently oriented ICC
reference, with maximum observed RGB8 channel difference one for all sixteen
cases. Color diversity prevents a blank-image pass. Each case also checks RAM
lease survival under zero capacity, native RGBA8 and prepared RGBA32F streamed
swap byte/bit identity, exact post-restore rendered frame, retained metadata,
restoration refusal under root-memory pressure and successful retry afterward.
All cases release managed root memory to zero. Actual adapter: Metal Apple M4
Max. Evidence: `docs/research/profiled-jpeg-orientations-ram-swap-metal-2026-10-07.log`.
This remains offscreen rendering qualification of the pinned profile and tested
JPEG sample layouts; it does not prove physical presentation, broader JPEG
profiles/subsampling, NVIDIA/CUDA or completion of all 100 formats.

### 2026-10-07: profiled JPEG foreground prefetch limits and cancellation

New foreground-loader regressions use real asymmetric CMYK/YCCK JPEG files
through the production display preloader. Forward/backward navigation plans
behind=1/ahead=2 with the correct priority neighbour; both independent byte and
entry-count ceilings retain only two 3536-byte prepared frames. Every neighbour
load preserves the visible frame and reuses its exact managed storage on a RAM
hit. Newly admitted zero-TTL neighbours expire while the visible pin survives.
The policy change preserves existing entry deadlines, as designed.
A queued-navigation test holds the decode gate, starts an old-generation JPEG
preload, then submits newer selections. The old preload cancels without cache
admission; only the latest request remains queued. Releasing the gate permits
real YCCK orientation-6 decode and linear display preparation (13 x 17), with
zero managed bytes after cleanup. The tests cover cancellation at the gate,
not interruption inside synchronous entropy decoding or ICC executor creation.
Evidence: `docs/research/profiled-jpeg-prefetch-limits-cancellation-2026-10-07.log`.

### 2026-10-07: reject contradictory JPEG APP14 channel declarations

The profiled CMYK/YCCK header path now rejects conflicting Adobe APP14 transform
values before decoded pixel allocation. Previously its accumulated declaration
was overwritten by the last Adobe marker, matching the underlying JPEG codec's
last-marker behavior; contradictory declarations could therefore pass admission.
Repeated consistent transform-0 declarations remain accepted and reproduce all
512 independent reference pixels exactly. Regressions prove the conflicting
transform-2/transform-0 case returns InvalidJpegColor and retains zero managed
bytes; the existing CMYK/YCCK color and sixteen orientation cases still pass.
Evidence: `docs/research/profiled-jpeg-app14-conflicts-2026-10-07.log`.
This guards declarations in the header before the first scan; arbitrary
multi-scan marker placement and all malformed JPEG variants are not qualified.

### 2026-10-07: pinned GPR HERO9 source and native compression gap

Pinned official GoPro GPR repository commit
`446c736a38fb14f51343605c0780d347dc602f89`, sample
`data/samples/HERO9/GOPR0002.GPR`, 6,763,778 bytes, SHA256
`9249208a3482d6d71d39575210fe19ab3dd737e5e446a4d0718d4bd9d3c4358b`.
The TIFF/DNG declares a 5568 x 4176 16-bit CFA sensor, Compression=9,
2 x 2 RGGB CFA, WhiteLevel=16383, explicit AsShotNeutral and two color matrices.
Its one VC-5 tile is at offset 14622 with length 6749156. The current public
native RAW decoder refuses `DNG Compression 9 is unsupported` with a 256 MiB
managed budget. GPR remains Pending. Required next work is native VC-5 entropy
and inverse-wavelet decoding plus Bayer reconstruction, followed by an
independent full-sensor oracle and color/cache/swap/GPU qualification. Existing
metadata is not proof of decoded correctness; preview substitution is excluded.
Source/hash/structure/tile baseline:
`docs/research/gpr-hero9-native-baseline-2026-10-07.json`.

### 2026-10-07: native borrowed VC-5 tag/chunk reader

New `rrrah_decode::vc5::Vc5Records` reads the VC-5 signature and big-endian
signed tag/value records, including optional tags, 16-bit small chunks and
24-bit large/section chunks. Payloads borrow the source buffer without copying;
checked size/extent arithmetic rejects truncated records/chunks, terminates the
iterator after one error and never scans ahead to resynchronize.
The pinned official HERO9 essence parses completely: 5568 x 4176 header and
four channel sections of 2126056, 1454472, 1375580 and 1792872 bytes. An explicitly
run external regression verifies these source-dependent bounds; local regressions
verify borrowed pointer identity and truncated header/large chunk refusal.
This is stream framing, not coefficient decoding. Native VC-5 entropy decoding,
subband reconstruction, inverse wavelet, Bayer assembly and sensor/color/viewer
qualification remain required; GPR remains Pending.

### 2026-10-07: native VC-5 nested sections and 16-bit lowpass unpack

The borrowed stream reader can now enter recognized structural sections while
keeping entropy blocks and vendor payloads opaque. The external pinned HERO9
regression traverses all four channels, twelve wavelet sections, forty subband
sections and forty entropy codeblocks with exact borrowed extents. The four
lowpass blocks each contain 181656 bytes for 348 x 261 coefficients. New
`lowpass_16` returns a nonallocating, exact-size big-endian u16 iterator with
checked geometry and exact payload length; synthetic endian and overflow/zero/
mismatched-size cases pass. Real block geometry passes; independent decoded
coefficient/sensor equality is still required. Highpass entropy unpack, inverse
wavelet and final Bayer reconstruction are not implemented, and GPR remains
Pending. Evidence: `docs/research/gpr-vc5-native-nested-lowpass-2026-10-07.log`.

### 2026-10-07: native VC-5 codebook-17 entropy runs

New `Vc5Runs` reads MSB-first codebook-17 magnitudes, sign suffixes and zero runs
without allocating output storage. It bounds every run by the admitted band
coefficient count, rejects truncated/unknown codes and requires the exact band
end marker after the final coefficient. The numerical codebook is attributed to
GoPro commit `446c736a38fb14f51343605c0780d347dc602f89`; its MIT license is retained
at `crates/rrrah-decode/data/vc5-LICENSE-MIT`. Algorithm implementation is Rust.
The first real HERO9 highpass block produces exactly 90828 quantized coefficients
and reaches its valid end marker. This is framing/count validation, not yet an
independent coefficient-value oracle or inverse-companding qualification.
Sensor reconstruction and viewer routing remain unimplemented; GPR stays Pending.

### 2026-10-07: independent GoPro VC-5 coefficient oracle

A test-only adapter compiles the pinned, unmodified official GoPro `GetRlv` and
`GetRun` with `table17.inc`, using a separate MSB-first host bit reader. The
public Rust `vc5_entropy_dump` example and C oracle independently decode the
first HERO9 channel's first highpass block (47692 bytes). All 90828 signed
quantized coefficients are bit-exact, with zero differences. Source/block/
reference-code/output hashes are retained in
`docs/research/gpr-vc5-first-highpass-coefficient-oracle-2026-10-07.json`.
Oracle build/run instructions and host adapter are in `scripts/vc5-oracle`.
This qualifies entropy values for one real band; all other bands, inverse
companding/quantization, wavelet reconstruction and final sensor equality remain
required before GPR viewer support can be enabled.

### 2026-10-07: all HERO9 VC-5 highpass coefficients match GoPro oracle

`scripts/qualify-gpr-entropy.py` pins the official HERO9 source SHA256, extracts
all 36 highpass blocks through channel/wavelet/subband framing and compares the
public Rust coefficient dumper with unmodified official GoPro GetRun/GetRlv.
All 22888656 signed quantized coefficients match bit-for-bit. Every block ends
at its admitted coefficient count and valid end marker. A separate run of the
committed qualification script reproduces every input/output hash and count.
Per-band hashes/counts and source provenance are retained in
`docs/research/gpr-vc5-all-highpass-coefficient-oracle-2026-10-07.json`.
The measured 3.88-second diagnostic run includes C/Rust dump processes, disk
writes and comparisons; it is not a viewer decode benchmark. This qualifies
entropy output on this pinned file, not inverse companding, dequantization,
wavelet reconstruction or final Bayer codes. GPR remains Pending.

### 2026-10-07: native VC-5 inverse companding and dequantization

New `vc5::dequantize` implements cubic inverse companding with exact integer
arithmetic, multiplication by the quantization factor and signed i16 saturation.
It refuses magnitudes outside the codebook, including i32::MIN, without overflow.
All 4088 cases (every magnitude -255..255 and factors 0,1,12,24,48,96,144,65535)
match the independent pinned GoPro companding/dequantize/pixel routines exactly.
The original C files are unmodified; release NDEBUG selects reference saturation
instead of debug range assertions. Golden and source hashes are retained under
`tests/fixtures/vc5/dequantize-gopro*`. Rebuilding/re-running that oracle reproduces
the golden exactly. Production uses Rust and no temporary buffers in this stage.
Inverse wavelet, managed band allocation, final Bayer assembly and whole-sensor
oracle qualification remain required; GPR is still Pending.

### 2026-10-07: native horizontal VC-5 inverse 2–6 filter

New `vc5::inverse_horizontal` writes into a caller-admitted row without temporary
allocation. It applies left/right boundary filters, interior correction, signed
arithmetic shifts and 14-bit output clipping; the final odd point is omitted for
odd output widths. Invalid width or mismatched row extents are refused before
any output write. An independent oracle executes the unchanged official GoPro
InvertHorizontal16s function and original shift/clamp helpers. The permanent
256-row corpus has 3200 outputs, widths 3–10, both output parities, signed inputs
and explicit lower/upper saturation cases. Golden/source hashes are retained in
`tests/fixtures/vc5/horizontal-gopro-reference.json`; oracle host/build instructions
are in `scripts/vc5-oracle`. This is the no-prescale horizontal stage only:
vertical reconstruction, prescale=2 behavior, managed bands and full sensor
reconstruction remain required before GPR support is complete.

### 2026-10-07: native VC-5 prescaled horizontal inverse

`inverse_horizontal_prescaled` now admits the reference prescale values 0 and 2.
The existing no-prescale API delegates to mode 0 without changing its 14-bit
clipping. Mode 2 reverses the encoder scale with signed i16 saturation, retaining
negative intermediate results instead of clipping them to zero. It writes into
caller storage without allocation. An independently produced 256-row corpus
(3200 values, odd/even output widths 3–10 source widths) matches the unchanged
GoPro InvertHorizontalDescale16s function exactly, including 1502 negatively and
275 positively saturated results. The original mode-0 corpus still matches.
Golden and pinned-function hashes are in
`tests/fixtures/vc5/horizontal-descaled-gopro-reference.json`.
Vertical reconstruction, managed intermediate-band storage, Bayer reconstruction
and a full sensor oracle remain required; GPR is not yet viewer-supported.

### 2026-10-07: native VC-5 two-dimensional mode-0 reconstruction

New `vc5::inverse_spatial` combines vertical border/interior filtering with the
qualified horizontal filter. It admits exact four-band geometry, odd/even output
extents and four-row scratch before writing; highpass magnitudes are checked
before reconstruction. It dequantizes LH/HL/HH inputs, preserves signed i16
vertical intermediates and uses caller-provided buffers without allocating.
All 2280 samples in 32 independently generated images match the unchanged GoPro
InvertSpatialQuant16s function and its horizontal/dequantization helpers exactly.
Cases cover source widths/heights 3–6 and both output parities. Golden/reference
hashes are in `tests/fixtures/vc5/spatial-gopro-reference.json`; test-only host and
reproduction instructions are in `scripts/vc5-oracle`. Mode-2 spatial integration,
managed band admission, cancellation between rows, real multilevel reconstruction
and full Bayer sensor equality are still required. GPR remains Pending.

### 2026-10-07: native VC-5 two-dimensional prescale=2 reconstruction

`inverse_spatial_prescaled` now admits modes 0 and 2, sharing the signed vertical
intermediate stage and selecting the independently qualified horizontal scale
restoration. The existing mode-0 API retains its behavior. Mode 2 matches all
2280 values in 32 independently generated images from unchanged official GoPro
InvertSpatialQuantDescale16s, including all edge filters and odd/even extents.
The mode-0 spatial and both horizontal regression corpora still pass.
`tests/fixtures/vc5/spatial-descaled-gopro-reference.json` retains golden and
original-function hashes. Production uses Rust with caller-provided scratch and
output. Native multilevel reconstruction of real channels, managed band/output
admission and cancellation, Bayer assembly, full sensor oracle, color and viewer
qualification are still required. GPR remains Pending.

### 2026-10-07: VC-5 managed entropy/output buffers and row cancellation

`decode_highpass_managed` admits quantized band storage to the request budget
before decoding, checks cancellation between entropy runs and retains the charge
in a shared buffer. Invalid entropy input drops allocated storage. Reconstruction
now accepts a cancellation callback checked before every source row; callers
must discard partial output after cancellation. `reconstruct_managed` admits
both output and four-row scratch to the same request/root budget, checks request
cancellation before allocation and after reconstruction and returns shared output
with retained ownership. A 3 x 3 -> 6 x 6 case requires 72 output plus 24 scratch
bytes: 95-byte admission fails and releases all charges; 96 bytes succeeds, peaks
at 96, retains only 72 after scratch drops and returns to zero when output drops.
Tests also verify exact cancellation placement and unchanged later output rows.
The managed entropy regression covers pre-allocation refusal, exact valid sample,
retained ownership and full release on invalid source. Caller-owned compressed
source and input bands must be accounted separately. Real multilevel integration
and whole-sensor GPR qualification remain incomplete; GPR stays Pending.

### 2026-10-07: native baseline VC-5 channel/band reconstruction index

`vc5::index_raw` now maps all forty compressed blocks into a fixed four-channel,
ten-band borrowed table with per-block quantization and lowpass precision. It
records sensor dimensions and packed prescale, checks RAW format/pattern/channel
admission, required tags, complete/unique band assignment and nonzero highpass
quantization, without allocating or copying payloads. The pinned HERO9 regression
verifies 5568 x 4176, packed prescale 10240, exact lowpass/highpass block sizes and
observed quantization 24/12, and refuses a truncated final section. All thirteen
VC-5 tests, including the explicitly enabled external source test, pass.
Evidence: `docs/research/gpr-vc5-native-band-index-2026-10-07.log`.
This index is the next input to managed multilevel reconstruction; full channel
and Bayer decoding are not yet implemented and GPR remains Pending.

### 2026-10-07: managed native three-level HERO9 channel reconstruction

`vc5::decode_channel` now indexes a baseline component, unpacks its managed
lowpass band, admits/decompresses three highpass bands per level and restores
all three wavelet levels with request cancellation. Prescale is unpacked using
GoPro's two-bit fields (deep-to-shallow 2,2,0 for HERO9). Earlier intermediates
release as each new lowpass replaces them. Channel 0 yields 2784 x 2088, all
5812992 signed i16 values matching independent GoPro entropy output and unchanged
reference spatial functions exactly. The test-only host assembles the admitted
bands explicitly; this is not yet a complete independent GPR sensor decoder.
Measured managed output/scratch/band peak is 23263104 bytes, retained output
11625984 bytes and zero after drop. Compressed source remains caller-owned.
Evidence: `docs/research/gpr-vc5-channel-zero-reconstruction-2026-10-07.json`.
Reproduction: native `vc5_channel_dump`, GoPro `spatial-file-main.c` host and
`scripts/qualify-gpr-channel-oracle.py`. Other components, component-to-Bayer
reconstruction, source-wide prescale/format variants and full sensor/color/viewer
qualification remain required; GPR remains Pending.

### 2026-10-07: all four HERO9 components reconstructed independently

The native managed channel decoder and independent GoPro entropy/spatial host
now match all four HERO9 components bit-for-bit: 23251968 signed i16 values in
four 2784 x 2088 channels. Each channel run separately records admitted peak,
retained output and zero managed bytes after release. Channel-oracle qualification
now accepts `--channel 0..3`; per-channel hashes and accounting are in
`docs/research/gpr-vc5-all-channel-reconstruction-2026-10-07.json`.
These components are GS/RG/BG/GD rather than ready Bayer photosites. Inspection
of unchanged official vc5_decoder/raw.c confirms required inverse component
mixing around midpoint 2048, 12-bit clipping, inverse Protune log lookup and
requested output-bit-depth scaling before RGGB/GBRG arrangement. Consequently
plain interleaving would be incorrect. This final transform and its independent
whole-sensor oracle still need implementation; GPR remains Pending.

### 2026-10-07: native VC-5 inverse Protune/component-to-Bayer values

`inverse_log12` and `components_to_bayer_cell` now implement the required final
baseline RAW value transform: midpoint-2048 component differences, R/B doubling,
G1/G2 combination, 12-bit clipping, inverse Protune curve and output-bit shift.
All 4096 log-table codes match unchanged official SetupDecoderLogCurve exactly;
Rust explicitly retains the reference f32 rounding before integer truncation.
The immutable 8192-byte lazy static table uses no per-request heap buffer and is
outside request budget accounting. Unsupported input log codes/precision refuse.
All 768 independent C cell records match unchanged PackComponentsToRAW exactly,
covering 12/14/16-bit value outputs and saturation/negative component cases.
Golden and original source hashes:
`tests/fixtures/vc5/bayer-transform-gopro-reference.json`.
These are cell-value and reference-placement tests, not admission of every
16-bit GPR format/layout. Managed full Bayer output assembly, full real-sensor
comparison, DNG routing/color and cache/viewer qualification remain required.

### 2026-10-07: full native managed HERO9 Bayer sensor equality

`vc5::decode_bayer` preadmits final sensor output, reconstructs all four managed
components, applies the qualified inverse log/component transform and assembles
linear RGGB codes with cancellation between component rows. The explicit
assembly helper also admits GBRG arrangement, but this real-file qualification
is RGGB only. The full 5568 x 4176 HERO9 sensor, all 23251968 14-bit samples,
matches the independent unchanged GoPro entropy/spatial/logcurve/packing routines
bit-for-bit under the explicit test-host band assembly. The shared budget peaks
at 104644992 bytes (output plus retained components and live reconstruction),
retains 46503936 bytes of sensor output and returns to zero after release.
Compressed source is caller-owned and outside this diagnostic budget accounting.
Source/output hashes and scope are retained in
`docs/research/gpr-vc5-full-sensor-oracle-2026-10-07.json`. Native diagnostic entry:
`vc5_sensor_dump`; independent packing host: `scripts/vc5-oracle/bayer-file-main.c`.
Tests refuse bad assembly geometry and cancellation before writes. Ordinary DNG
Compression=9 routing, source admission, camera metadata/color, cache/swap/viewer
qualification and broader GPR sources still remain; GPR stays Pending.

### 2026-10-07: ordinary managed GPR sensor admission

Native DNG Compression 9 now routes a single complete even-sized RGGB/GBRG tile through owned VC-5 reconstruction into managed mosaic ownership without pixel copies or duplicate output reservations. Extents must match the VC-5 header; linearization and other layouts are refused. WhiteLevel selects the 12/14/16-bit sensor range independently of the TIFF container width. All 23,251,968 HERO9 sensor values match the staged independent GoPro oracle. Source-inclusive managed peak is 111,408,770 bytes, retained sensor 46,503,936, after-drop zero. Other cameras, independent color, cache/swap/prefetch and rendered GPR qualification remain open; GPR is not yet generally qualified. See `research/gpr-native-dng-admission-2026-10-07.json`.

### 2026-10-07: HERO9 GPR RAM, persistent cache, swap and Metal

The ignored external-source `gpr_readback` integration passed on Metal Apple M4 Max. All independently referenced sensor samples and metadata survive persistent cache and swap exactly; full 128x96 native/cache/restored GPU frames match. RAM checks cover byte/count refusal, zero-TTL unpinned expiry, visible-pin preservation on impossible limit shrink and shared-pixel hits. Swap pressure refusal retains the entry for a successful retry; final managed usage and queue bytes are zero. Render comparison shares native metadata and GPU processing, so it does not qualify independent camera color. Foreground preload/cancellation and other GPR cameras remain open. See `research/gpr-cache-swap-metal-2026-10-07.json`.

### 2026-10-07: GPR gallery admission, preload and active cancellation

Added GPR to image/RAW extension candidates so gallery and RAW prefetch include it. The external HERO9 app preload test covers allocation pressure refusal, native retry, shared RAM hits at zero free memory, exact disk restore, stale-generation refusal and preserved visible pin; final root usage is zero. A separate thread test advances generation only after managed peak exceeds source plus final sensor, proving cancellation during intermediate VC-5 reconstruction and complete lease release. Both readback tests and preload test pass. Multi-path previous/next GPR windows, independent rendered color and other cameras remain open. See `research/gpr-prefetch-cancellation-2026-10-07.json`.

### 2026-10-07: bounded GPR previous/next windows

Application preload testing now exercises five HERO9 copies with distinct source timestamps/cache keys, previous=1 and next=2 in both directions. All six planned visits pass production RAW preload, nearest-direction priority and pointer-identical warm hits with zero free root memory. A two-entry/93,007,872-byte RAM limit retains the visible frame and exactly one neighbour; managed usage equals that bound after each visit and zero after release. These are repeated same-camera sources, not cross-camera qualification. Independent color, other camera variants and hardware qualification gaps remain. See `research/gpr-neighbour-windows-2026-10-07.json`.

### 2026-10-07: official HERO5/6/7 and both Fusion sensors

Downloaded five additional samples from pinned GoPro GPR commit `446c736a38fb14f51343605c0780d347dc602f89`; source hashes are recorded. Ordinary NativeRawDecoder admits both Fusion sensors (3104x3000) and HERO5/6/7 (4000x3000) under a 128MiB root. All 54,624,000 sensor values match the staged independent unchanged GoPro entropy, inverse spatial and Bayer packing functions exactly, including rounded wavelet extents for Fusion. `scripts/qualify-gpr-camera-sensors.py` reproduces comparison with pinned source hashes and existing external C oracle hosts. This remains staged-function qualification, not an independent full SDK parser or rendered color oracle. See `research/gpr-official-camera-admission-2026-10-07.json` and `research/gpr-official-camera-sensor-oracle-2026-10-07.json`.

### 2026-10-07: malformed GPR admission

External HERO9 mutation testing confirms dimension disagreement, unsupported WhiteLevel, invalid VC-5 signature and one-byte truncation produce errors before managed sensor allocation, with managed used/peak zero. Test-host source Vec is outside this budget; this is structural admission coverage, not all entropy-corruption classes. Full default decode regression remains green. See `research/gpr-damaged-admission-2026-10-07.json`.

### 2026-10-07: complete independent GoPro SDK oracle

Built the unmodified pinned GoPro SDK externally in Release. Complete gpr_tools GPR-to-RAW conversion matches all six native sensor outputs (77,875,968 values) exactly, superseding the staged-function parser limitation for this corpus. HERO9 SDK GPR-to-DNG conversion also permits independent LibRaw 0.22.2 unpack: sensor remains exact and WB matches, but camera-to-sRGB matrices differ (maximum coefficient difference 0.12308195233241578). Rendered color remains unqualified; conversion metadata and illuminant-selection policies need investigation before accepting or changing color. SDK/LibRaw remain external oracles only. See `research/gpr-full-sdk-sensor-oracle-2026-10-07.json` and `research/gpr-sdk-libraw-color-diagnostic-2026-10-07.json`.

### 2026-10-07: explain HERO9 independent color-matrix discrepancy

Independent TIFF extraction proves ColorMatrix1/2, AsShotNeutral and CalibrationIlluminant1/2 are unchanged by SDK GPR-to-DNG conversion. A standalone Python f64/Gauss-Jordan calculation explains both outputs: native CM2 Bradford adaptation D65-to-D50 matches camera-to-sRGB within 3.85e-8, while LibRaw matches unadapted CM2 within 7.63e-8. Thus the 0.123 coefficient discrepancy is a white-point policy difference rather than sensor mismatch or changed SDK metadata. This proves implementation of the current native policy, not full DNG scene-illuminant interpolation or rendered-camera color accuracy. Production color is unchanged pending a full independent rendering contract. See `research/gpr-color-matrix-policy-2026-10-07.json`; reproducer `scripts/qualify-gpr-color-matrices.py`.

### 2026-10-07: GPR default development includes gain maps

Fixed a product omission: raw_development_opcodes now includes GPR, so HERO gain maps force default quality development. HERO9 gain-map area extends to row 4192 beyond the 4176-row sensor. Removed an incompatible pre-validation bound check; the existing area iterator intersects with the destination exactly as pinned SDK dng_gain_map.cpp ProcessArea does. Full HERO9 default development produces 5568x4176 output under a 1GiB budget and releases all managed bytes. All four HERO samples parse four list2 gain maps; Fusion optional unsupported WarpRectilinear remains omitted. 84 core tests plus external opcode and full-development tests pass. Prior direct-mosaic Metal comparisons did not exercise gain maps; independent developed/rendered color remains open. See `research/gpr-gain-map-development-2026-10-07.json`.

### 2026-10-07: DNG gain-map normalized saturation

Inspection of pinned official dng_opcode_GainMap::ProcessArea showed an additional numerical difference: SDK bounds value*gain at 1.0, while native multiplication retained over-range samples. The native opcode now bounds at the stage scale (1.0 for supported list2 GainMap), preserving the SDK upper-bound semantics. A compact 2x2 fixture with padded area tests overlap and highlights (0.75*2 becomes 1.0; 0.25*2 remains 0.5). All 85 core tests and full default HERO9 development pass; final managed usage is zero. Spatial interpolation and final developed/rendered-color oracle remain open. See `research/gpr-gain-map-saturation-2026-10-07.json`.

### 2026-10-07: independent DNG SDK gain interpolation fixture

An external C++ host links the unmodified pinned SDK dng_gain_map::Interpolate. Its 460-value fixture exercises a nonconstant 3x4 map on a 20x23 image, fractional positions, nonzero/negative origins and all edge clamps. Native gain_at differs by at most 3e-7. The fixture is now included in default core tests without an SDK runtime dependency. Provenance and hash: `tests/fixtures/dng/gain-map-sdk.json`; host: `scripts/gain-map-sdk-oracle.cpp`. This is spatial single-plane interpolation qualification, not full developed color.

### 2026-10-07: developed GPR float swap and Metal

HERO9 now has a distinct post-development integration: native full sensor, four real list2 gain maps and default develop_raw produce a managed 5568x4176 RGBA32Float raster. All 93,007,872 float component bits and color-space metadata survive RasterSwapCache exactly. Full 128x96 native/restored frames match on Metal Apple M4 Max and contain visible nonzero image data. Budget pressure refuses restore without invalidating entry; retry succeeds; final root usage and queue are zero. This proves transport of corrected pixels, not independent rendered-color fidelity. See `research/gpr-developed-float-swap-metal-2026-10-07.json`.

### 2026-10-07: full GPR quality cancellation and pressure

Application prepare_quality_raw cancellation is now exercised on HERO9 after live retained managed bytes exceed 256MiB, proving float-development allocation rather than pre-cancelled sensor decode. Generation change returns cancellation and releases all managed buffers. A separate 128MiB root admits the full native sensor but refuses quality development while preserving all 23,251,968 source samples and exactly 46,503,936 retained sensor bytes; final drop returns root usage to zero. See `research/gpr-quality-cancellation-pressure-2026-10-07.json`.

### 2026-10-07: measured native GPR import baseline

Existing raw_import_timing measured HERO9 warm-source import over five verified repetitions after one warmup: wall p50 2308.45ms/p95 2340.60ms; source p50 0.459ms; native decode p50 2307.87ms. All full sensor hashes and metadata match, and managed usage returns to zero each repetition. Own code uses workspace dev opt-level=1 (dependencies=3), so this is a development baseline, not release or full-viewer speed. Inspection identifies magnitude prefix matching linear codebook scans as an optimization candidate; its causal impact needs a paired change measurement. See `research/gpr-native-import-timing-baseline-2026-10-07.json`.

### 2026-10-07: native VC-5 prefix tree improves warm sensor import

Replaced per-prefix linear scans over 264 codes with an immutable compile-time 527-node prefix tree (3162 bytes, no heap allocation). Const construction asserts prefix-free uniqueness and complete node count; a unit test resolves every original codeword with exact consumed bit count. Sequential same-dev-build HERO9 five-run measurements improve wall p50 from 2308.45ms to 564.26ms (4.09x), p95 from 2340.60ms to 568.99ms, without changing full sensor BLAKE3 or 111,408,770-byte managed peak. Re-decoding all six sources after optimization matches all 77,875,968 full-SDK reference values exactly. These are dev warm sensor-import measurements, not release or end-to-end viewer speed; timings were sequential rather than interleaved. See `research/gpr-vc5-prefix-tree-performance-2026-10-07.json`.

### 2026-10-07: VC-5 optimization lifecycle revalidation

After the production prefix-tree optimization all three external GPR integration tests pass: active native reconstruction cancellation, sensor RAM/persistent/swap/Metal preservation, and corrected developed RGBA32Float swap/Metal preservation. Adapter is Metal Apple M4 Max. Updated the GPR catalog entry to reflect full SDK sensor corpus and actual development/lifecycle tests while retaining Pending for independent final color, Fusion warp and unqualified variants. See `research/gpr-prefix-lifecycle-regression-2026-10-07.json`.

### 2026-10-07: stable VC-5 sensor headers

Native indexing now refuses conflicting repeated sensor dimensions, channel count, image format, pattern dimensions and component count, matching its single-representation reconstruction contract. Explicit zero first values are tracked rather than treated as missing. This prevents last-header reinterpretation of earlier indexed bands; per-band quantization and channel/subband navigation remain mutable. Fourteen compact adversaries and all 19 VC-5 tests including official HERO9 pass. See `research/gpr-vc5-stable-header-2026-10-07.json`.

### 2026-10-07: independent rectilinear coordinate oracle

Native square-pixel rectilinear source-coordinate evaluation now matches 300 external SDK points with mixed radial/tangential coefficients and an asymmetric optical center within 1e-11. The host calls unchanged pinned SDK EvaluateRatio/EvaluateTangential and explicitly assembles square-pixel geometry. Fixture and provenance are in `tests/fixtures/dng/rectilinear-sdk.f64le` and `.json`; default core tests include them. Image resampling, non-square pixel aspect and full SDK image-warp qualification remain open; Fusion correction is not yet enabled.

### 2026-10-07: exact SDK bicubic warp weights

Implemented native DNG warp 4x4 bicubic weights (A=-0.75), with SDK 1/32-phase quantization and f32 normalization. An external host links unchanged SDK dng_resample_weights_2d. All 16,384 float weight bits across 1024 phases match exactly in default tests. All 89 core tests pass. Image sampling, border extension, cancellation and pipeline admission remain open; this does not yet enable Fusion correction. Fixture provenance: `tests/fixtures/dng/bicubic-sdk.json`; host `scripts/bicubic-sdk-oracle.cpp`.

### 2026-10-07: native Fusion rectilinear warp in development

Enabled opcode1 parsing and validated one/three-plane square-pixel rectilinear list3 corrections. Native development combines independently checked coordinate functions and exact SDK quantized bicubic weights, repeated-edge sampling and normalized output clamping; cancellation is checked per row. RGB source copy is covered by the existing conservative development scratch reservation. Official Fusion-back default quality now triggers its real warp, produces 3104x3000 output different from no-op development, and releases all managed buffers. All 90 core tests pass. Full independent SDK warped image/edge comparison, non-square aspect and corrected Fusion GPU/swap qualification remain open; do not claim full rendered equivalence. See `research/gpr-fusion-native-warp-2026-10-07.json`.


### 2026-10-11: explicit linear PFM viewer policy and Metal route

The current CLI adds `--untagged-color linear-srgb` to the earlier strict/srgb
policy. It qualifies only linear RGB with unspecified primaries; default strict
behavior and producer declarations remain intact. Authored PFM tests cover
preload, exact float oracle, TTL spill, decoder-free swap, incompatible-color
cache isolation and thumbnail ownership. Actual Metal SDR/HDR pre/post-swap
readbacks and a normal native HDR window were checked. The ordinary viewer
gate passed 196 tests, with the new ignored Metal case explicitly run separately.
Full producer corpus and alternate PFM dialect qualification remain open.
Evidence: `research/metal-pfm-linear-cli-2026-10-11.json`.


2026-10-11: ASTC HDR explicit linear-sRGB policy now reaches the decoder.
The authored 23×17 reference fixture passes exact preload/TTL/swap float-bit
preservation and actual M4 Max Metal SDR/HDR readbacks; strict and encoded-sRGB
requests cannot reuse its cache entry. Decoder gate: 986 passed / 67 ignored;
viewer gate: 197 passed / 34 ignored; new Metal test run separately.
[Evidence and boundaries](research/metal-astc-hdr-linear-policy-2026-10-11.json).
NVIDIA remains deferred; broad format and physical HDR qualification remain open.


2026-10-11: KTX2 explicit linear-sRGB imports now qualify only absent DFD
primaries. Declared non-sRGB primaries cannot be overridden by the assumption.
Authored float samples remain bit-exact through viewer preload/TTL/swap, with
exact actual M4 Max Metal SDR/HDR pre/post-restore readbacks. Gates: 987 decoder
and 198 viewer tests pass; the new actual Metal case passes separately.
[Evidence and limits](research/metal-ktx2-linear-policy-2026-10-11.json).


2026-10-11: DPX normalized linear import now accepts explicit linear-sRGB
for undefined colorimetry (255) only. The authored 5×3 RGB8 subset preserves
prepared float samples through viewer preload/TTL/swap and exact actual
M4 Max Metal SDR/HDR frames before/after restore. Gates: 988 decoder and
199 viewer tests pass; the new actual Metal case passes separately.
[Evidence and limits](research/metal-dpx-linear-policy-2026-10-11.json).


2026-10-11: Explicit-layout BAY source reads now enforce the camera-size cap
before managed allocation. Four focused tests pass, including oversized-file
refusal with zero peak allocation for all three layouts. The full ordinary
decoder gate passes 989 tests (67 ignored). Sensor-only qualification remains
separate from camera/color/viewer support.
[Evidence](research/bay-source-admission-cap-2026-10-11.json).


2026-10-11: The actual Metal raster collection now includes compressed ASTC
HDR and plain/DEFLATE RGBA32Float KTX2: 65 fixture sources, 94 advertised
selections and 188 native/prepared storage cases. Full float samples match
their reference/authored oracles; RAM/swap, pressure refusal, cancellation,
SDR/HDR readback and final root/queue release pass. The runner verifier accepts
the expanded log. File counts are not fully qualified format counts, and this
is not physical HDR measurement.
[Expanded collection evidence](research/metal-raster-collection-hdr-expanded-2026-10-11.json).


2026-10-11: Viewer loader/preload ping-pong qualification now includes PFM
and compressed ASTC HDR alongside profiled PNG, GIF, EXR and Radiance HDR,
using the shared explicit linear-sRGB CLI interpretation. All 120 selections
preserve exact Metal SDR/HDR frames, bounded neighbours and RAM membership,
visible-owner reuse under managed pressure, stale-generation refusal and final
root release. The CPU case and actual M4 Max Metal case pass separately;
199 ordinary viewer tests pass. This synchronous helper test does not qualify
asynchronous worker timing or physical HDR.
[Evidence and limits](research/metal-linear-six-format-navigation-2026-10-11.json).


2026-10-11: The actual bounded thumbnail worker now has a linear-import
generation-replacement regression: held ASTC HDR returns no stale thumbnail,
queued KTX2 is replaced without starting, and only the latest PFM publishes
the expected pixels. All managed thumbnail memory releases. The real loader
and queue are exercised with a barrier before decoding; this is not native
ASTC mid-computation cancellation or foreground event-loop qualification.
The focused test and 200 ordinary viewer tests pass.
[Worker evidence](research/linear-thumbnail-worker-reversal-2026-10-11.json).


2026-10-11: Model swap now has actual Metal SDR and RGBA16Float preservation
checks for six STL/OBJ/OFF/PLY sources over 120 repeated restores, pressure
refusal and early cancellation/retry. Seven model tests pass; the shared
readback helper passes the expanded raster collection again. Neutral materials,
small authored sources and offscreen attachments retain explicit limits.
[Evidence](research/metal-model-swap-hdr-expanded-2026-10-11.json).


2026-10-11: CUA opened the compressed 23×17 ASTC HDR fixture in the normal
viewer with explicit linear-sRGB. The native Metal window reported image-ready
in 6.78 ms, RGBA16Float / ExtendedSrgbLinear and upload pending zero; AppKit
reported current EDR 16×. Exposure keys reveal colored gradients immediately,
and reset restores the original appearance. The test app was closed and prior
temporary launcher restored. This single-source observation is not a benchmark
or physical luminance/color qualification.
[Native ASTC evidence](research/metal-native-astc-hdr-window-2026-10-11.json).


2026-10-11: An accepted empty folder scan now replaces the old image list and
clears selection, preventing arrow/index navigation from loading stale files
from the previous context. A real FolderScanner/App regression covers both
open-first modes and confirms no old load is enqueued; pending-drop and stale
generation guards remain covered by the full 201-test viewer gate (36 ignored).
[Evidence](research/empty-folder-navigation-context-2026-10-11.json).


2026-10-11: Selecting a folder now invalidates prior foreground file generations,
clears duplicate/flight state, drains queued file requests and pauses/cancels
RAW speculative work before scanning. Empty folders no longer leave old loads
current simply because no replacement file is available. A real App submission
queue / FolderScanner regression passes; the full ordinary viewer gate passes
202 tests (36 ignored). Cancellation remains cooperative; admitted store writes
retain their existing contract.
[Evidence](research/folder-intent-cancellation-2026-10-11.json).


2026-10-11: Added explicit PFM top-down request/CLI policy, shared across
foreground/preload/thumbnails and included in RAM/swap identity. Authored
RGB/grayscale both-endian samples, scale and decoder-free swap rows pass; actual
Metal SDR/HDR frames match before/after restore. Gates: 990 decoder and 203
viewer tests pass, with actual GPU run separately. Independent Adobe output
and physical HDR qualification remain open.
[Evidence](research/metal-pfm-top-down-import-2026-10-11.json).

2026-10-11: The qualification runner now exposes `--metal-linear-import`,
requiring five individually selected, actually executed Metal SDR/HDR swap tests
for PFM, ASTC, KTX2, DPX and explicit top-down PFM. Each exact test must pass
without skips and emit its workload evidence; missing tests cannot silently pass.
The current runner passed eight suites / 115 test executions on Apple M4 Max,
including 107 ordinary GPU checks, all five import checks, 12 MP upload
cancellation and both 6188×4120 compute tests. Managed full-sensor exposure
checks all 25,494,560 pixels and releases CPU/GPU credits to zero. Ordinary
ignored cases remain explicitly recorded; this is bounded Metal qualification,
not all-format or physical HDR/slow-disk completion. Reproducer and log hashes:
`research/metal-linear-import-integrated-2026-10-11.json`.

2026-10-11: Fixed a normal-viewer HDR format gap: HEVC HEIF/HEIC now
retains known BT.2020 PQ/HLG declarations instead of dropping them to
unspecified color, allowing the existing explicit reference-white / HLG
display policy to perform linear conversion. ICC precedence is unchanged.
Two authored lossless 12-bit controls validate every native code, including
adjacent values, plus strict refusal and managed conversion. Viewer tests
exercise CLI, preload/cache hit and thumbnail. Actual Apple M4 Max Metal
validates float bits and SDR/HDR frames through RAM pinning, pressure refusal
and disk swap retry. Gates: 991 decode / 204 viewer tests pass; full HDR/swap
Metal suite 14 pass / 2 ignored. Chromatic HEVC HDR, alpha, independent camera
producers and physical HDR remain open. Evidence and fixture hashes:
`research/metal-heif-pq-hlg-2026-10-11.json`.

2026-10-11: Extended HEVC HDR beyond grayscale: lossless PQ/HLG RGB
controls now check every native code, including mixed-color bands. Independent
FFmpeg HEVC readback reproduces all recovered 12-bit codes for four sources
(12,288 pixels). Pure-primary linear conversion matches independent 60-digit
Decimal and chromaticity-derived reference vectors; all four sources pass
actual Metal SDR/HDR exact transport through RAM pinning, pressure refusal
and swap restore. Gates: six focused HEIF tests and 992 decoder tests pass;
the expanded four-source Metal case passes. HDR alpha, photographic producers,
independent mixed-color conversion and physical HDR remain open. Evidence:
`research/metal-heif-hdr-color-independent-2026-10-11.json`.

2026-10-11: Straight-alpha HEVC HDR controls extend the current Metal
qualification to six PQ/HLG sources. FFmpeg separately decodes the auxiliary
HEVC alpha stream: all 6144 12-bit alpha codes are exact. Native decoding
retains every normalized RGB/alpha sample and transparent hidden RGB; HDR
conversion leaves alpha unchanged. Actual Metal checks 16 interior composite
controls per alpha source against linear source-over, then exact bits and
SDR/HDR frames through swap/pressure retry. CLI preload and thumbnails pass,
including background-only transparent regions. Gates: 992 decoder / 204
viewer tests pass; the six-source Metal case passes. A separate native HDR
window loads the 64×48 control in 4.34 ms (single warm observation, not a
benchmark), visibly responds to exposure/reset, and quits cleanly. Physical
HDR and independent photographic producers remain open. Evidence:
`research/metal-heif-hdr-alpha-2026-10-11.json`.

2026-10-11: Fixed a real photographic HEIF admission failure: a Canon
DPP 6000×4000 10-bit PQ HIF was rejected because the 512 MiB retained-output
cap was applied to three working-plane equivalents. The cap now applies to
retained RGBA, while managed requests separately admit estimated native
working credit before codec invocation and keep output credit with pixels.
A cropped 64×48/encoded64×64 regression qualifies exact 90112-byte admission
and one-byte-under refusal/release. The external photographic source passes
all 72M RGB-component comparisons against FFmpeg within max71/65535
(mean22.478265, bounded not exact); full float digest and Metal SDR128×96 /
HDR128×64 frames survive swap and pressure retry with root/queue zero.
Gates: 993 decoder / 204 viewer / 14 ordinary HDR-swap tests pass; external
photographic case separately passes. Normal native Metal HDR window displays
the full 6000×4000 photograph, exposure and reset redraw without re-decoding.
Single ready time8.83s is not a speed qualification; optimization remains.
Direct camera producers and physical HDR remain open. External copyrighted
source stays under /tmp; provenance, reproducible commands and hashes:
`research/metal-photographic-heif-working-budget-2026-10-11.json`.


### 2026-10-11 — exact optional integer PQ lookup on Metal path

Large u16 PQ rasters now use a managed 512 KiB exact f64 lookup (u8: 2 KiB),
with scalar fallback when scratch cannot be admitted/allocated. HLG and float
input retain their formulas. All integer codes match scalar float bits; output
credit, scratch peaks and cancellation release are covered. Current gates:
105 core, 993 decoder, 14 ordinary HDR tests pass; external 24 MP photographic
HEIF separately passes actual Apple M4 Max Metal SDR/HDR and swap pressure
retry, full float payload exact, root/queue zero. Five paired release repeats
on that file, alternating order: median PQ 4131.707 -> 422.678 ms (9.775x),
full float digest identical. Native decode median 2577.390 ms measured separately.
Shared host, warm filesystem; not app ready-time or physical HDR qualification.
NVIDIA remains deferred; full 100-format acceptance remains open. Evidence:
`research/metal-pq-integer-lookup-2026-10-11.json` and accompanying gate log.


### 2026-10-11 — independent mixed-color HEIF HDR conversion

Closed the mixed-color conversion gap recorded above: all four authored
RGB bands now compare against independent Decimal/Fraction references for
PQ/HLG, including straight-alpha controls. The mixed 12-bit RGB vector is
expanded independently to u16; added five PQ and three HLG policy references.
105 core tests pass (450 PQ / 270 HLG oracle vectors), focused HEIF
independent color test passes, six-source actual Apple M4 Max Metal
SDR/HDR pressure/swap transport test passes. No production color formula
changed. This is synthetic HEVC qualification; direct camera producers and
physical HDR remain open, NVIDIA deferred. Evidence:
`research/metal-heif-mixed-color-independent-2026-10-11.json` and gate log.


### 2026-10-11 — 24 MP viewer preload at full managed pressure

External Canon DPP PQ HEIF passes the actual normal-viewer preload/cache
route on Metal M4 Max: initial float frame retains 384000000 bytes; with all
remaining 1 GiB managed root credit reserved, a RAM hit shares its allocation
and preserves SDR/HDR128x64 frames. A distinct source is refused before
decoding without evicting the ready source; releasing pressure permits retry,
with identical rendered frames. Dropping all owners and cache returns root
to zero. Ignored external/GPU regression passes (1 test, 7.26s).
This is managed CPU pressure, not OS-wide pressure or physical HDR validation.
Evidence: `research/metal-photographic-heif-viewer-pressure-2026-10-11.json`
and accompanying retained log.


### 2026-10-11 — real photographic full-resolution Metal HDR readback

The external 6000x4000 Canon DPP PQ HEIF now passes full-resolution
Rgba16Float HDR attachment readback before/after RAM/disk pressure swap,
using RRRAH_HEIF_FULL_HDR_READBACK=1. All 96000000 binary16 components
(192000000 payload bytes, padded readback192512000) compare bit-for-bit.
Every component finite, alpha exactly1, image nonconstant; full float source
digest also exact, restore pressure/retry succeeds, root/queue zero. Actual
Metal M4 Max, 1 external test passed in9.47s; SDR remains128x96.
Readback/test attachments are outside managed CPU root, so this is not an
overall GPU/host peak-memory proof. Physical HDR and camera producer breadth
remain open; NVIDIA deferred. Evidence:
`research/metal-photographic-heif-full-hdr-readback-2026-10-11.json` and log.


### 2026-10-11 — viewer requests actual adapter texture extent

Fixed production viewer device negotiation: portable default8192 restricted
valid wide/tall rasters even on M4 Max supporting16384. The main window now
requests the selected adapter max_texture_dimension_2d; other limits and
existing renderer byte/queue caps remain. Actual Metal regression proves
the same8193x128 source refuses with the old default device, uploads with
production-negotiated limits, renders SDR/HDR red controls correctly and
releases managed texture credit. 204 ordinary viewer tests and1 actual
Metal regression pass. This is headless device/renderer evidence, not native
window interaction; above-hardware tiling, physical HDR and NVIDIA remain
separate. Evidence:
`research/metal-viewer-supported-texture-extent-2026-10-11.json` and log.


### 2026-10-11 — native wide-raster Metal HDR window

Verified current normal binary in a separate native qualification bundle:
authored9000x1024 linear PFM opens on Metal M4 Max, Rgba16Float /
ExtendedSrgbLinear surface. Four HDR RGB bands visibly display; exposure
and reset redraw without re-decoding (one Raster ready event). Fresh
second launch shows content after raising the window before any exposure
key. Initial background-only blank capture is recorded, not treated as a
new production bug without further evidence. Single warm ready times
193.82ms/181.11ms are not benchmarks. Native CmdQ closes own process.
This closes the prior headless-only evidence boundary for this wide-raster
fix; physical HDR, above-device tiling and full format scope remain open.
Evidence: `research/metal-wide-native-window-2026-10-11.json` and log.


### 2026-10-11 — bounded texture-array raster tiling

Production CPU-prepared raster upload now splits oversized extents into
balanced texture-array tiles without constructing a packed CPU tile buffer.
The shader resolves each bilinear sample globally before selecting a layer,
retaining premultiplied-alpha interpolation across horizontal/vertical seams.
Padded texture bytes and conservative aligned queue payload are admitted;
partial cancellation retains old image and queued credit through completion.
50 GPU units,11 actual Metal raster readbacks,204 viewer units,14 HDR tests,
viewer extent regression and65-source/94-selection/188-case collection pass.
Negotiated64-pixel test device qualifies tile seams/corners/alpha exact
against untiled SDR/HDR, padding, partial cancellation and retry. Actual
native Metal HDR window displays20000x512 authored PFM (two10000x512
layers on16384-extent M4 Max), then quits cleanly.
Resident atlas remains capped512MiB and device layer count; no out-of-core
GPU tile cache or above-device resident-compute transfer tiling yet. Physical
HDR, NVIDIA and full format qualification remain separate. Evidence:
`research/metal-raster-texture-array-tiling-2026-10-11.json` and log.


### 2026-10-11 — GPU-resident compute array tiling

Closed the resident-compute extent restriction recorded in the CPU tiling
milestone: upload_resident copies packed GPU-buffer rectangles/individual
unaligned rows directly into balanced texture-array layers, with original
stride and no intermediate staging/readback. Padded texture admission,
queue occupancy, source/destination completion leases and aspect remain.
12 actual Metal raster tests and50 GPU units pass; new controls force
64-pixel extents across aligned/unaligned rows, seams and last cropped
layer/corner, plus20000x64 above hardware extent. SDR/HDR frames match
CPU exposure controls, one-byte-under refusal/cancellation preserve old
image, retry succeeds and all tracked roots release. Separate compute-cache
small/full4000x3000 regressions pass (2 tests, full bits/frame exact).
The latter uses CPU cache payloads; direct GPU-buffer swap is not claimed.
Resident512MiB/device limits and out-of-core streaming remain separate,
as do physical HDR/NVIDIA/full format acceptance. Evidence:
`research/metal-resident-compute-array-tiling-2026-10-11.json` and log.

### 2026-10-11 — Metal single-layer raster specialization

Static WGSL override selects a single-layer fetch pipeline while tiled arrays retain global-coordinate sampling. Hardware timestamp ABBA comparison on M4 Max, 4096-square RGBA32F input to 2048-square HDR target, reduces median render-pass GPU time from 0.476688 to 0.355938 ms (25.33%). Each run has five warmups and thirty samples; decode/upload/CPU/startup costs are excluded. Dynamic branching was measured slower and discarded. Actual Metal raster checks pass 12, HDR/cache/swap checks pass 14, and collection checks cover 65 sources, 94 selections and 188 storage cases with tracked root/queue zero. Full format acceptance, physical HDR, NVIDIA and GPU streaming remain open. Evidence: `research/metal-raster-single-layer-specialization-2026-10-11.json` and log.

### 2026-10-11 — Full-resolution HDR collection on specialized Metal shader

Added optional RRRAH_METAL_COLLECTION_FULL_HDR=1 to the long-lived eight-frame 4000x3000 collection test, replacing its small HDR probe with whole-frame Rgba16Float readback. Actual M4 Max Metal completed 108 restore/pressure/TTL cycles in 60.050 seconds: every restored source float bit and all full SDR/HDR frame components match pre-spill references. All 108 cycles exercise successful parallel restore and mid-batch cancellation; reads=108, writes=8, managed root/queue zero per cycle and final. Test total69.83s including setup. Reference/readback allocations are outside cache accounting; authored sources/local disk/full HDR attachment readback do not establish photographic coverage, slow-disk resilience or physical display accuracy. Evidence: `research/metal-full-hdr-collection-specialized-2026-10-11.json` and log.

### 2026-10-11 — Bounded GPU source-region transfers on Metal

RasterRenderer::upload_region_with_cancel now transfers a source rectangle directly from packed CPU rows into bounded texture-array residency while preserving full-image placement, pixel aspect, exposure and premultiplied SDR/HDR sampling. Callers provide the bilinear halo; missing sampling footprints show ordinary shader background. Static region variants compile lazily on first admitted region; ordinary single-layer/tiled variants remain. Actual Metal tests cover single/multiple layers with64-pixel negotiated extents, cropped corners, exact full-image reference frames, invalid rectangles, memory refusal, queued cancellation, retry and returning to whole-image upload. A6000x6000 RGBA32F source (576000000 bytes, rejected by eager upload) renders an exact full HDR viewport with only a128x128 resident region (262144 bytes). Final gates:50 GPU units,14 Metal raster cases,14 HDR/swap cases and65-source/94-selection/188-storage collection, tracked roots/queues zero. Ordinary-path timestamp smoke median0.35525ms. This is an implemented region-transfer primitive, NOT automatic viewer streaming: viewport planning, replacement scheduling, prefetch/LOD and out-of-core source decoding remain. Physical HDR/NVIDIA/full format acceptance also remain. Evidence: `research/metal-resident-source-regions-2026-10-11.json` and log.

### 2026-10-11 — Visible source-region planner with Metal view matrix

RasterRenderer::visible_source_region computes a conservative source rectangle with bilinear/coordinate-rounding halo, matching shared view normalization, shader f32 fit/scale,32-pixel fit margin and source pixel aspect. Fully offscreen images return None without transfer. Planning itself allocates no pixels and performs no device access; region upload remains separately admitted. Actual M4 Max Metal comparison covers60 complete SDR/HDR viewport frames over fit, zoom0.02..128, fractional pan, cropped edges, return to fit and pixel aspects0.5/1/2:46 planned uploads and14 correct offscreen skips, every frame exact against whole-image rendering. A64-pixel negotiated region device exercises array seams; source has signed/bright/fractional-alpha data and exposure1.25. Final gate51 GPU units and15 raster tests pass (7 manual tests ignored), tracked roots/queues zero. Automatic normal-viewer source retention/scheduling/reuse and full-footprint LOD/streaming remain to connect; no complete viewer-streaming claim. Physical HDR/NVIDIA/full-format qualification remain. Evidence: `research/metal-visible-region-planner-2026-10-11.json` and log.

### 2026-10-11 — Viewer ownership and interaction for regional fallback

Connected source-region planning/upload to RasterReady and GpuState view changes through raster_viewport::ResidentSource. Full texture upload remains preferred; memory/extent refusal falls back when the current visible region is smaller and fits. Only regional mode retains a shared managed CPU source (no pixel copy). Exposure-only updates and contained footprints reuse residency; offscreen views skip upload; pan refreshes source region, preserves/reapplies RAW curve and tracks queued resources. Admission failure preserves old region/source, surfaces status and permits retry; successful whole-image/native-RAW/model switches release regional source. All204 ordinary app tests pass (40 hardware/manual ignored). One explicit actual-Metal production-helper test covers normal/RAW-developed HDR, whole-texture refusal under160000-byte texture/queue caps, source lifetime, exposure reuse, fractional pans, pressure refusal/retry and offscreen skip, with exact whole-image HDR references and final source/root/queue zero. Native-window interaction was not exercised here. Initial fit-to-window above budget still refuses: a bounded HDR LOD, automatic background scheduling/overscan and multiregion eviction remain. Physical HDR/NVIDIA/full-format qualification remain. Evidence: `research/metal-viewer-region-integration-2026-10-11.json` and log.

### 2026-10-11 — Managed native HDR area-filtered LOD

Added DecodedRaster::downsample_linear_with_budget_and_cancel: native Rust f64 area filtering of premultiplied linear RGBA32F, returning straight alpha without clamping/tone mapping/quantization. Fractional footprints cover odd source edges; adjusted pixel aspect preserves physical geometry, with hotspot/image selection/sample scale retained. Output credit is admitted before allocation, no full-source staging copy, and cancellation/error returns credit. Four new controls include48 shapes against an independent integer-overlap oracle, transparent RGB/bright/signed analytic reference, metadata and exact/one-byte-under/cancel admission. All109 core units and3 color-precision integrations pass (2 pre-existing manual units ignored). Small and explicit36MP actual-Metal controls match independent SDR/HDR frames. Large6000x6000/576000000-byte source reduces to1000x1000/16000000-byte output, all1000000 output pixels exact, credit zero after drop. One optimized-test CPU filter observation141.271042ms is not a broad benchmark. Automatic viewer LOD sizing/budget wiring/cache/initial-fit fallback remains unconnected; full CPU source and physical HDR/NVIDIA/full-format scope remain separate. Evidence: `research/metal-managed-hdr-area-lod-2026-10-11.json` and log.

### 2026-10-11 — Automatic initial-fit HDR LOD and source-region switching

Closed the initial-fit refusal from prior viewer-region milestone: RasterReady fallback now halves source dimensions until managed HDR output/GPU texture/queue headroom admit a level. CPU LOD budget16MiB is a child of configured managed root (otherwise independent managed budget). Viewer retains source and one preview, reuses exposure/fit, promotes to source regions when possible and returns to cached preview; if cached level no longer fits replacement headroom, a smaller preview is admitted before old frame/cache replacement. Atomic upload still enforces budgets after advisory headroom check. App204 units pass (41 manual ignored);2 actual-Metal production-helper controls cover ordinary/RAW modes and SDR/HDR, exact source/preview references,160000-byte texture/queue caps, cached40960-byte level and10240-byte pressure replacement, external old-level ownership and final credits zero. Native window also opens1000x1000/16000000-byte authored HDR PFM in fit with raster cap1MiB (preview250x250/1000000 bytes), visibly zooms/pans using16x14 original regions, then returns to fit viaF; logs show only one decode event. Own app quits cleanly. CPU filtering/upload remain synchronous, full CPU source is retained, and background scheduling/cancellation, multiregion LRU/prefetch, physical HDR/NVIDIA/full-format acceptance remain. Evidence: `research/metal-automatic-viewer-hdr-lod-2026-10-11.json` and log.

### 2026-10-11 — Cancellation across managed HDR LOD and upload

Added cancellation-aware ResidentSource install/refresh, carrying callbacks through source upload, regional transfer, LOD selection, CPU area filter and final GPU upload. RasterReady initial installation uses actual foreground GenerationToken; cancelled results skip later status/neighbour publication. CPU RasterError::Cancelled is terminal and normalized to RasterUploadError::Cancelled. Three explicit actual-Metal viewer controls pass, including atomic generation7->8 during admitted filtering, cancellation after queued LOD write, fresh-generation retry and cancellation of a smaller pressure-induced LOD replacement. Old HDR frames/source region/cached level survive; partial output releases immediately, queued texture/queue credits release after completion, final credits zero. Normal204 app tests pass (42 hardware/manual ignored). This is cancellation plumbing, not background execution: preparation/upload initiation remain synchronous and normal interactive refresh wrapper currently passesfalse until viewport worker/scheduling is connected. Physical HDR/NVIDIA/slow disks/out-of-core/full100-format acceptance remain. Evidence: `research/metal-viewer-hdr-lod-cancellation-2026-10-11.json` and log.

### 2026-10-11 — Background initial-fit HDR LOD worker

Initial-fit HDR filtering now runs on a lazy rrrah-hdr-lod thread with one active job, one replaceable pending request and one completed output slot. Own revision plus foreground GenerationToken cancel stale work; obsolete output is released, and input source ownership ends before wake-up. App retains original ready event/consumer lease, consumes matching job/generation only and uses EventLoopProxy to wake UI. Prepared publication skips the full original-source pixel scan when its texture cannot fit; bounded-level validation/upload remains on UI. Deterministic paused-worker test proves submit/coalescing without waiting, only latest publication and output-credit release. All205 app units pass (43 manual/hardware ignored);4 actual-Metal production-helper tests pass including real worker-thread preparation, exact HDR frame and final source/LOD/texture/queue zero, alongside prior cancellation/SDR/HDR/RAW/cache controls. Native window opens1000x1000/16MB HDR source in fit at1MiB raster cap; visible RGB/gray bands and logs confirm250x250 worker level, event-driven ready and one decode. Own app quits cleanly. Initial filtering is now asynchronous; viewport refresh/replacement filtering, GPU upload initiation and ordinary full-source validation remain synchronous, with automatic budget-change rescheduling/prefetch/streaming still open. Physical HDR/NVIDIA/full-format scope remains. Evidence: `research/metal-background-initial-hdr-lod-2026-10-11.json` and log.

### 2026-10-11 — Reject a ready HDR level after generation changes

Completed worker output retains its foreground GenerationToken and checks cancellation again when consumed. A generation change after publication drops the stale managed level before handing it to the GPU upload path. Deterministic regression waits for actual publication, advances generation7 to8, verifies all4096 output bytes return, then prepares a current result using the same entire budget. Both worker controls pass; all206 ordinary app units pass (43 manual/hardware ignored). All4 explicit actual-Metal viewport controls pass, covering exact HDR frames, RAW curves, regional/LOD switching, filtering/upload cancellation and final managed credits zero. Cancellation during subsequent upload remains guarded separately. Interactive background replacement and physical HDR qualification remain open.

### 2026-10-11 — Background viewport HDR LOD replacement

Viewport refresh now separates pixel-free LOD need/planning, background native area filtering and transactional Metal publication. GpuState owns a separate lazy coalescing worker with the same EventLoopProxy wake; source identity cancels obsolete requests and prevents stale publication. Active/results are consumed before replanning against their already-owned memory. Production refresh refuses synchronous replacement filtering; existing source-region transfers and cached-level reuse remain immediate. Successful publication reapplies RAW curve and tracks uploads; old frame/cache survive failures. Extended actual-Metal worker control drops cached preview after source-region promotion, proves deferred refresh does not allocate/filter or change the old HDR frame, computes on worker, then publishes an exact HDR reference under a40960-byte child budget with zero available bytes. Final managed credits zero. All206 ordinary app units and4 explicit Metal controls pass;2 worker controls pass. Native gesture scheduling was not exercised in this run. GPU validation/transfers remain on UI and CPU full source remains retained; physical HDR, NVIDIA, out-of-core and full100-format qualification remain open. Evidence: `research/metal-background-viewport-hdr-lod-2026-10-11.json` and log.

### 2026-10-11 — Keep prepared viewport level across Metal admission failures

GpuState now retains one ready managed viewport LOD when texture/queue upload admission fails, retries the same pixels on subsequent view/wake events and drops them when source identity changes or the level is no longer needed. Upload failure no longer immediately submits another filter, avoiding a worker/filter/wake loop under persistent GPU pressure. Filter errors also return without immediate resubmission in the same wake. Extended actual-Metal control exhausts texture and then upload-queue credit separately: old regional state and full HDR frame remain exact, failed queue admission returns attempted texture credit, and the same40960-byte prepared result succeeds after pressure release. Final tracked credits zero. All206 ordinary app units (43 ignored),4 explicit Metal tests and2 worker tests pass on the rebuilt current app. Native window scheduling and physical HDR remain unqualified. Evidence: `research/metal-viewport-lod-publication-retry-2026-10-11.json` and log.

### 2026-10-11 — Native-window background viewport replacement

Current native Metal build opens authored1000x1000/16MB HDR PFM under1MiB raster budget, scrolls to original16x14 region then70x54 region, returns viaF and prepares125x125 replacement on rrrah-hdr-lod. Screenshots confirm complete RGB/gray fit and visible panning; logs show one decode, one viewport worker request, cached return without new filtering. Native run exposed unconditional App.update_view in drain_load_events, requesting redraw each about_to_wait: fixed by checking completed worker slot first and avoiding cancellation when no pending request exists. Worker availability assertions cover empty/published/consumed states. All206 regular app units,4 explicit Metal controls and2 worker tests pass after rebuild. Own app exits cleanly. Residual idle presentation activity (~7.5% CPU, sampled main thread predominantly in CAMetalLayer nextDrawable) remains to isolate; do not claim idle efficiency complete. Screenshot is visual interaction evidence, not physical HDR measurement. Evidence: `research/metal-native-viewport-background-2026-10-11.json`, log and idle sample.

### 2026-10-11 — Remove telemetry idle render loop

Residual native-window activity was the minimized telemetry window requesting its own next redraw after every successful present. Removed that unconditional request; pipeline/cache changes, resize and input already invalidate telemetry. Rebuilt native Metal run repeats1000x1000 HDR fit/source-region/background125x125 return, with one foreground decode. In an untouched22.0003-second interval, ps CPU time stays0.54 seconds and displayed CPU is0.0%; idle sampling shows main thread blocked in macOS event wait instead of continuous CAMetalLayer nextDrawable. Exposure minus after idle visibly darkens the gray band, confirming presentation resumes on input. Own process exits cleanly. All206 ordinary app units (43 ignored) and4 explicit Metal controls pass. This resolves the prior observed idle presentation loop for this run, not a universal power benchmark; physical HDR, NVIDIA, out-of-core and full100-format qualification remain open. Evidence: `research/metal-telemetry-event-driven-idle-2026-10-11.json`, log and sample.

### 2026-10-11 — Bounded panning margin for GPU source regions

ResidentSource now expands a visible source region by up to min(32,visible smaller dimension/4) pixels, trying decreasing margins only when padded texture and aligned queue budgets independently have room for two copies. This advisory spare planning leaves room for a future replacement under stable independent budgets; concurrent/shared-ancestor admission remains authoritative. Expanded transfer memory/extent refusal retries the exact visible rectangle; cancellation remains terminal. Contained pan footprints reuse the uploaded region without new pixels. New actual-Metal control compares48 SDR and48 HDR frames against whole-source reference: each sequence requires only2 replacement uploads after initial install, all frames exact, final texture/queue credit zero under1MiB caps. All5 explicit Metal controls,206 regular app units (44 manual/hardware ignored), and51 GPU units pass. Existing160000-byte pressure/cancellation/cache controls remain valid without forcing overscan. Full CPU source retention, out-of-core/physicalHDR/NVIDIA/full100-format scope remain open; no wall-clock benchmark claim. Evidence: `research/metal-source-region-pan-margin-2026-10-11.json` and log.

### 2026-10-11 — Shared-ancestor texture/queue planning

MemoryBudget::can_reserve_with checks simultaneous requests against every local/ancestor cap, adding both requests at shared nodes without reserving or updating peaks. Renderer extent planning now uses this combined headroom for texture and aligned queue, including two-copy panning spare. Unit controls cover deep siblings, same budget, ancestor/descendant, independent roots, pressure and overflow. Actual-Metal100x100 planning boundary refuses300000-byte shared root although each separate request fits, admits400000, and refuses two-copy spare under either. Extended pan control covers192 exact SDR/HDR frames:2 replacement uploads per48 views with independent1MiB budgets,4 with600000-byte shared parent, final texture/queue/parent credits zero. All39 memory units,1 real TTL integration,1 doctest,206 app units (44 ignored),51 GPU units and5 explicit Metal controls pass. Shared ancestry is now included; concurrency still requires atomic allocation admission. Native/physicalHDR/NVIDIA/out-of-core/full100-format qualification remain open. Evidence: `research/metal-shared-budget-region-headroom-2026-10-11.json` and log.

### 2026-10-11 — Single-file verified swap ranges

Added SwapStore::write_segmented and restore_range with immutable SegmentedSwapHandle. One complete blob reserves one disk object/quota before I/O; managed32-byte BLAKE3 digests index64KiB blocks. Source streaming retains one managed scratch block, with no complete serialized buffer. Range restore seeks only intersecting blocks, verifies each complete block before copying and publishes only after successful integrity/cancellation; result and scratch admission roll back on failure. Handles share immutable digest index and disk quota until last owner drops. Three new controls cover unaligned ranges, float-bit bytes, requested/unread corruption, incomplete/extra input, quota, transient cancellation, memory refusal/retry, overflow/foreign/empty ranges and shared lifetime. Generated8388609-byte source has no full input allocation; measured managed peaks are4128-byte index,65536-byte scratch and8-byte result, one disk object, final credits/quota zero. Full swap suite passes14 units,6 integrations and1 doctest. This is an out-of-core byte-storage primitive, not connected image region decode or viewer streaming; existing fixed stream stack/Entry metadata/OS page cache remain outside these managed buffer figures. PhysicalHDR/NVIDIA/full100-format acceptance remain open. Evidence: `research/segmented-swap-verified-ranges-2026-10-11.json` and log.

### 2026-10-11 — Strided verified swap rows

Added SwapRows/restore_rows for packed extraction of non-overlapping rows from one segmented source. Opens one file, seeks only requested blocks, reuses the last verified64KiB block across adjacent rows, skips row gaps and publishes only after all selected bytes succeed. Bounds/overflow/overlap are validated before allocation. Existing restore_range delegates to the same row path. New independent slice oracle covers192 layouts over odd offsets, contiguous/gapped strides, empty rows and block boundaries; corrupt skipped block remains unread, invalid layouts release all credit. All15 swap units,6 integrations and2 doctests pass, retaining prior cancellation/admission/ownership/HDR-bit-byte controls. This supplies image-region byte extraction but does not yet parse raster metadata, produce native cropped rasters or connect GPU/viewer streaming. No disk performance/physicalHDR/NVIDIA/full100-format completion claim. Evidence: `research/segmented-swap-strided-rows-2026-10-11.json` and log.

### 2026-10-11 — Native swapped raster regions

Added direct segmented encoder output and SwappedRaster/RasterRegion. Persisted native rasters retain only geometry/header and the managed digest index, allowing the complete decoded source to drop. Region restoration reads verified profile/pixel rows and reconstructs native u8/u16/f32 crops while preserving color declaration, sample scale, pixel aspect and image selection; hotspot translates when contained. Controls prove full2621440-byte source release before HDR restoration, exact sample bits including signed zero/negative/above-one/NaN storage, integer coordinate oracle, ICC, pressure refusal/retry, cancellation/bounds and final credit/quota zero. Full cache/swap test command passes. Existing512MiB native payload cap and initial full decode remain; transient packed plus decoded crop/profile buffers are admitted. This component is not yet connected to Metal/viewer; no new hardware, physicalHDR, NVIDIA or full100-format completion claim. Evidence: `research/native-swapped-raster-regions-2026-10-11.json` and log.

### 2026-10-11 — Packed swapped regions on Metal

RasterRenderer now accepts packed cropped linear-sRGB pixels plus original source size/origin through upload_packed_region_with_cancel. Shared transfer logic preserves global geometry, pixel aspect, texture tiling, sample validation, cancellation and transactional admission. Actual Metal Apple M4 Max control persists source to segmented swap, computes full-source reference, releases full managed CPU pixels, restores only the visible crop and compares exact SDR/HDR readback for aspects0.5/1/2. Invalid geometry, exhausted texture budget and cancellation preserve prior output; final managed/disk credits zero. All51 GPU units and previous single/multilayer region Metal regression pass. This verifies storage-to-GPU components; production viewer still retains full source and needs background swap-region orchestration. PhysicalHDR/NVIDIA/full100-format scope remains open. Evidence: `research/metal-packed-swapped-regions-2026-10-11.json` and log.

### 2026-10-11 — Viewport planning from swapped metadata

Visible-region planning now accepts source dimensions/pixel aspect independently of DecodedRaster, and the existing whole-source API delegates to this path. SwappedRaster exposes retained pixel aspect without disk reads. Actual Metal control drops complete source first, then computes the region exclusively from swapped metadata; rectangle and SDR/HDR readback match the pre-release whole-source reference for aspects0.5/1/2. All52 GPU units pass including invalid zero dimensions/aspects and offscreen geometry. This removes the planner dependency on full pixels; production viewer source ownership/async region integration remains pending. Evidence: `research/metal-swapped-metadata-viewport-2026-10-11.json` and log.

### 2026-10-11 — Background swapped-region worker

Existing application LodWorker now accepts either native filtering or a verified SwappedRaster region restore, preserving its one-active/one-replaceable-pending/one-completed bounded scheduling and wake path. Complete source ownership is unnecessary for region jobs. Paused-worker control proves requests1/2/3 coalesce to3, verifies crop sample coordinates, memory pressure refusal/retry, stale generation after publication and final buffer/index/disk release. Actual Metal integration now restores through this application worker before exact SDR/HDR comparisons for aspects0.5/1/2, after full-source release. All207 ordinary app tests pass (44 manual controls ignored), and4 integration/worker controls pass. Main viewer still needs to submit region jobs and migrate source ownership; no complete viewer streaming or physicalHDR/NVIDIA/full100-format claim. Evidence: `research/metal-background-swapped-region-worker-2026-10-11.json` and log.

### 2026-10-11 — Swapped viewer residency and region refresh

ResidentSource now supports full-pixel ownership migration to a verified SwappedRaster, guarded by source identity, geometry/aspect and retained fit preview. Metadata planning, coalesced worker region jobs and transactional packed publication preserve global geometry and RAW curve; fit reuses preview. GpuState update_view routes swapped states through this asynchronous refresh. Prepared useful crops survive GPU admission failures without reread; stale/unneeded results drop. Actual Metal viewer-state control releases2621440 full-source bytes, retains only40960 preview bytes between crops, compares three panned HDR frames exactly, forces GPU refusal/retry, restores exact fit preview and returns all managed/index/disk credit. All207 ordinary app tests pass (45 manual ignored); all6 explicit Metal viewport controls pass, including prior panning/shared-budget/LOD/RAW/cancellation regressions. Initial ordinary loading does not yet automatically schedule segmented write/adoption; this migration trigger and native gesture qualification remain pending. PhysicalHDR/NVIDIA/full100-format scope remains open. Evidence: `research/metal-swapped-viewer-residency-2026-10-11.json` and log.

### 2026-10-11 — Automatic visible source swap

Ordinary raster load now passes a clone of the existing ImageSwapCache store (same disk quota/object cap), schedules segmented persistence on a separate coalescing worker and adopts only successful matching-source completion with retained fit preview. Disabled swap does not schedule it; errors preserve RAM source. Stale persisted completion control drops index/disk/input ownership, and worker output distinguishes frames from swap handles. All208 ordinary app units and6 explicit Metal viewport controls pass. Native M4Max HDR1000x1000 PFM under1MiB texture limit/64MiB enabled swap logs automatic adoption, then20x18 background reads on zoom/pan; fit visibly shows all bands after input, app exits. Native follow-ups remain: first screenshot was blank until F, and one temporary swap file remains after exit. These need startup invalidation/shutdown investigation; this run does not establish finished native lifecycle. Initial complete decode/512MiB payload cap, physicalHDR/NVIDIA/full100-format acceptance remain open. Evidence: `research/metal-automatic-visible-source-swap-2026-10-11.json` and log.

### 2026-10-11 — Native exposure and temporary swap shutdown

Native blank first screenshot was diagnosed from logs as Metal Occluded during background launch; raising the exposed window presents HDR bands without keyboard input. Viewer explicitly requests redraw on unocclusion/focus. Added terminal ImageSwapCache shutdown (cancel/wake/join; ordinary Drop remains nonblocking), foreground loader disconnect/join and shutdown of its raster/model/RAW swap workers. ApplicationHandler::exiting explicitly releases GPU source/workers, loader caches and queued event owners because macOS Cmd+Q can bypass Rust stack teardown. New TTL/noTTL idle-worker shutdown regression verifies temporary directory removal and idempotence. All208 ordinary app tests,142 cache tests (3 manual ignored) and6 explicit Metal viewport controls pass. Native Cmd+Q after automatic segmented adoption logs completed transient shutdown; process exited and fresh test cache has zero files. This supersedes previous native cleanup concern for the tested clean exit; crashes/forced kills and broader pressure/format/physicalHDR/NVIDIA scope remain unqualified. Evidence: `research/metal-visible-source-lifecycle-2026-10-11.json` and log.

### 2026-10-11 — Swapped viewport pressure soak and large native sources

Extended actual-Metal swapped viewer control with repeated CPU restore-budget pressure, alternating GPU texture/queue pressure, retained ready-crop retry, exact detail/fit references and baseline/final credit assertions.60.030sec completes8468 cycles,25404 exact HDR detail frames,8468 CPU refusals and25404 GPU refusals; final managed/index/disk credit zero. This is warm local I/O/small-source pressure, not physical slowdisk or OSwide pressure. Removed the ordinary full-payload512MiB restriction specifically from segmented source encoding; native descriptor/profile bounds, checked lengths, disk quota and ordinary restored-crop limit remain. Explicit36MP6000x6000 float control streams576000000 pixel bytes, releases full source before17x11 crop, preserves bits/aspect with281280-byte managed index/65536 scratch peak/2992 crop bytes and final credits zero. Whole-payload cap remains unchanged. Full142 ordinary cache tests pass (4 manual ignored, including separately-run36MP control), and6 explicit Metal viewport regressions pass. Removed duplicate Occluded arm; existing unocclusion redraw and added focus redraw remain. Initial full decode, broader corpus, physicalHDR/NVIDIA/full100-format acceptance remain open. Evidence: `research/metal-swapped-viewport-pressure-soak-2026-10-11.json` and log.

### 2026-10-11 — Shared CPU crop admission and coalesced pan

SwappedRaster::can_restore_region now checks read/parse memory phases (retained prefix/packed bytes, shared64KiB scratch, native crop/profile), ordinary per-crop size cap and ancestor budgets without reservation/peak changes. Viewport overscan uses this advisory; if only exact visible crop fits CPU, margin is omitted. Cache boundary control refuses2111/admit2112 independent output bytes and66623/66624 shared parent bytes, with actual restore at admitted boundaries. Metal control leaves95000 CPU bytes and proves exact visible fallback succeeds despite GPU spare supporting larger margin. Extended deterministic paused-worker viewport control supersedes first request through three pans, then only latest HDR reference publishes; final CPU/index/texture/queue/disk credits zero. Three cache region controls and all6 explicit Metal viewport controls pass. Actual atomic admission remains authoritative under concurrency. Physical slowdisk/OSwide pressure/physicalHDR/NVIDIA/full100-format qualification remain open. Evidence: `research/metal-shared-cpu-crop-admission-2026-10-11.json` and log.

### 2026-10-11 — Delayed swapped-source navigation on Metal

Added explicit actual-Metal cross-source worker control: two distinct HDR sources share capped store, full CPU sources drop, first old-source region job is paused before I/O, then generation7 changes to8 and new source is submitted through same worker. Old result/wake never publishes; its sole-reader file and managed index release before latest wake. Only newest generation crop reaches packed Metal upload and exactly matches new-source HDR reference. Final CPU/output/scratch/index/texture/queue/disk credits zero. New explicit Metal control passes and5 existing integration/worker controls pass. Delay is a deterministic before-work barrier, not physical slowdisk or native window file-navigation stress. PhysicalHDR/OSwide pressure/NVIDIA/full100-format qualification remain open. Evidence: `research/metal-delayed-swapped-source-navigation-2026-10-11.json` and log.

### 2026-10-11 — Corrupt swapped region preserves Metal frame

Extended actual-Metal viewer residency control to flip a byte in a selected source pixel block beyond the independently verified metadata block. Background restore rejects digest mismatch before native crop publication; exact prior HDR fit frame and40960 preview bytes remain, temporary allocations release, pending/ready slots clear and no automatic retry wake occurs. After repairing original byte, existing retry/detail/fit reference controls succeed; rapid paused-worker pans still publish only latest crop and final CPU/index/texture/queue/disk credits zero. Explicit Metal control passes. This qualifies deliberate local corruption/repair, not a physical failing disk or native corrupt-file UI navigation. PhysicalHDR/OSwide pressure/NVIDIA/full100-format acceptance remain open. Evidence: `research/metal-corrupt-swapped-region-publication-2026-10-11.json` and log.

### 2026-10-11 — Swapped fit fallback under GPU pressure

Fixed a production gap after complete source migration: retained fit preview could not be replaced by a smaller level when old detail plus cached preview exceeded GPU headroom. Swapped viewport work now distinguishes region restore from background filtering of the retained native linear-HDR preview, preserves active output admission and retains ready level across failed upload. Explicit Metal control leaves20k texture headroom, refuses64x40 preview, filters32x20 on worker, exhausts queue to preserve exact old frame/ready level, retries to exact independently downsampled-preview HDR reference and retains only10240 preview CPU bytes instead40960. Complete source remains released and final CPU/index/texture/queue/disk credits zero. All208 ordinary app tests (46 manual ignored) and6 explicit Metal viewport controls pass. Reference is the filtered retained preview, not claimed direct-original LOD equivalence. Physical slowdisk/OSwide pressure/physicalHDR/NVIDIA/full100-format acceptance remain open. Evidence: `research/metal-swapped-fit-preview-pressure-2026-10-11.json` and log.

### 2026-10-11 — Original-source HDR LOD from swap

SwappedRaster now area-filters original verified source rows into managed native linear-HDR LOD without restoring a complete source. Working allocations are output pixels, one f64 accumulator row, one source byte row and swap scratch; per-output accumulation order matches native core filtering, including premultiplied alpha and fractional edges. Source metadata/aspect/hotspot/scale/selection retained. Worker SwapLod operation replaces repeated filtering of retained preview in production swapped fit fallback. Five129x73 target shapes (including odd edges and identity) match original-source native reference bit-for-bit after full source release; pressure/cancellation/bounds roll back.36MP6000x6000 original576MB stays released during17x11 exact direct-reference LOD with165072-byte managed workspace peak plus281280-byte index; scratch65536 and retained output2992 bytes. Actual Metal fit-pressure control now compares against pre-release original-source filtered reference, preserving ready level/old frame on queue refusal. All208 app tests,144 ordinary cache tests (4 manual ignored;36MP separately passes) and6 explicit Metal viewport controls pass. This supersedes retained-preview quality limitation for this fallback. Initial full decode, per-row reopen/reverification cost, broader corpus/physicalHDR/NVIDIA/physical slowdisk/OSwide pressure/full100-format qualification remain open. Evidence: `research/metal-original-swapped-hdr-lod-2026-10-11.json` and log.

### 2026-10-11: scoped verified swap reader for original-source HDR LOD

The LOD worker now retains one open swap file, one managed verified block and one reusable source row through filtering. Adjacent range reads reuse verified bytes; failed block replacement invalidates the cache before I/O. Original-source native area-filter references remain bit exact, including the 36MP source after releasing its 576MB full buffer (165072-byte managed LOD peak plus 281280-byte retained index). Six real Metal viewport checks pass. A warm-filesystem 8MiB/4096-row synthetic comparison measured 373–449ms for separate restores versus 9.4–9.5ms for the scoped reader; this does not measure end-to-end viewer latency or physical slow disks. See `research/metal-scoped-verified-swap-reader-2026-10-11.json` and `.log`. Full goal remains open.

### 2026-10-11: restore swapped fit quality after memory pressure

Fit selection now starts from original source dimensions and considers the complete streamed CPU workspace, including shared verification scratch. After GPU pressure lowers the preview to 32x20, releasing pressure restores 64x40 directly from the original swap object; actual Metal HDR frames match the original fit reference exactly, the full source stays released, and the next refresh schedules no redundant job. Six real Metal viewport tests, 133 cache unit tests and 208 ordinary app tests pass. Shared CPU admission is verified at the exact 71136-byte test boundary and refuses one byte below without allocation. Evidence: `research/metal-swapped-fit-quality-recovery-2026-10-11.json` and `.log`. Physical HDR and the full goal remain open.

### 2026-10-11: repeated pressure while swapped fit is already resident

Expanded the actual Metal residency regression with eight 64x40 -> 32x20 -> 64x40 cycles. Texture pressure selects the smaller original-source LOD; blocked upload queue preserves the previous exact HDR frame and retries prepared pixels. Exhausted CPU admission starts no speculative job and leaves the smaller frame intact. Releasing pressure restores the original larger fit exactly without restoring the full CPU source. Pending/ready work is empty at each recovered baseline; all CPU/index/texture/queue/disk credit is zero at final teardown. This bounded synthetic regression does not qualify OS-wide pressure or physical slow disks. Evidence: `research/metal-repeated-swapped-fit-pressure-2026-10-11.json` and `.log`.

### 2026-10-11: common verified reader and mid-block cancellation

Range/strided-row restore now shares the scoped block-verification reader used by original-source HDR LOD. A regression cancels after replacement block I/O before digest verification, verifies that the cached identity is invalid, introduces disk corruption and confirms retry rejects it, then repairs and reads exact original bytes. Full cache/swap tests and doctests pass, including delayed-I/O cancellation/retry and bounded resources. Six real Metal viewport checks pass, including repeated pressure/recovery and old-frame retention on corruption. Evidence: `research/metal-unified-verified-swap-cancellation-2026-10-11.json` and `.log`. Physical-device and full-format acceptance remain open.

### 2026-10-11: skip invisible swapped fit refinement

Swapped fit planning and publication now require a visible source intersection. The real Metal regression releases pressure while a 32x20 preview is resident and moves the image entirely offscreen: no rebuild is scheduled, CPU ownership and the prior frame remain unchanged. Returning to fit restores the exact 64x40 original-source frame. Six Metal viewport checks and 208 ordinary application tests pass. Evidence: `research/metal-offscreen-swapped-fit-admission-2026-10-11.json` and `.log`.

### 2026-10-11: measured original-swap to Metal HDR frame latency

Eight exact-frame recovery samples for a 512x320 RGBA32F source, 64x40 original-source LOD and 64x64 Metal HDR framebuffer measured worker preparation 3.0–3.21ms, publication/render/readback 0.5–0.8ms, total 3.57–4.01ms. Every timed frame equals the independent original-source reference and full source CPU pixels stay released. Warm filesystem and small image only; includes readback, excludes physical presentation and UI event-loop latency. Evidence: `research/metal-swapped-fit-stage-timing-2026-10-11.json` and `.log`.

### 2026-10-11: 36MP original-swap worker to exact Metal HDR frame

A variable 6000x6000 RGBA32F source with alpha and non-square pixel aspect is independently downsampled before releasing all 576MB source CPU pixels. The worker subsequently streams its swap object to 128x128 using exactly 427776 bytes of managed workspace plus 281280 retained index bytes. Actual Metal 128x128 HDR readback exactly equals the original-source reference. One warm-filesystem run measured 608.93ms preparation, 4.81ms publication/render/readback, 615.67ms total including renderer setup; excludes initial decode/write and physical display presentation. Final all-resource accounting is zero. This exposes original-source CPU filtering as the dominant latency for large-image refinement; it does not qualify general interactive latency. Evidence: `research/metal-36mp-original-swap-frame-latency-2026-10-11.json` and `.log`.

### 2026-10-11: sequential swap cursor reuse and partial-read recovery

The verified block reader avoids redundant seeks for adjacent blocks and invalidates known cursor state before physical reads. A truncation/repair regression confirms failed partial reads invalidate block and cursor state and a later retry seeks correctly and restores exact bytes. Full cache/swap suites and the 36MP real Metal exact-frame path pass. The single current worker sample was 719.71ms versus the previous 608.93ms under different concurrent load: no end-to-end speedup is demonstrated. Managed workspace/index remain 427776/281280 bytes, full source CPU zero, final credit zero. Evidence: `research/metal-sequential-swap-cursor-reuse-2026-10-11.json` and `.log`.

### 2026-10-11: transparent-sample filtering experiment rejected

A candidate skipped accumulation arithmetic after validating alpha-zero samples. Three original 36MP worker samples were 762.70/671.43/684.08ms; candidate samples were 1206.42/1177.82/736.34ms. Exact HDR references and bounded memory passed, but no speedup was demonstrated, so the candidate was reverted. The restored path passes the exact original-source Metal frame at 611.43ms. Concurrent load/small sample count prevent causal attribution; no general regression claim. Profile the original-source filter before further arithmetic changes. Evidence: `research/metal-36mp-transparent-filter-experiment-2026-10-11.json` and `.log`.

### 2026-10-11: sampled original-source36MP worker, integrity cost visible

A targeted native sampler captured77 observations of `rrrah-hdr-lod`:50 in verified block read/hash/copy (37 BLAKE3,11 file read,2 copy),27 in filter/sample conversion. This corrects the earlier loose interpretation of worker preparation as purely filtering: integrity work is significant in the captured slice. Short and profiler-perturbed sampling does not establish whole-pass percentages; the profiled2821ms is not a normal latency measurement. The exact original-reference Metal HDR frame and zero-resource teardown still pass. Next optimization should reduce repeated original-source passes using bounded reusable exact levels, while preserving integrity. Evidence: `research/metal-36mp-swapped-worker-profile-2026-10-11.json`, `.sample.txt`, `.log`.

### 2026-10-11: bounded exact LOD swap cache integrated into viewer recovery

Swapped sources now optionally retain at most one exact native LOD disk object, sharing source disk byte/object quotas and managed digest index credit. A larger cached level survives smaller pressure downgrades, enabling viewer recovery without a full original-source pass. Verified cached pixels stream directly into admitted native storage; no extra CPU pixels remain resident. Full quota skips this optimization without failing the generated frame; a dedicated single-object-quota retry regression passes. Memory/cancellation retain retryability; source teardown releases cached disk/index ownership. Seven real Metal checks and208 ordinary app tests pass, plus133 full cache units,11 integrations and the additional quota case. In the36MP test, first preparation609.70ms and cached worker0.305ms yield exact identical Metal HDR frames; cached managed peak327680 bytes versus427776 for original filtering. Original+cached index281440 bytes; final all resources zero. Eight512x320 viewer recovery cycles now measure0.46–0.70ms including GPU readback. Warm filesystem and repeat target only; no first-open speed claim. No independent level TTL: lifetime follows live source ownership. Evidence: `research/metal-bounded-exact-lod-swap-cache-2026-10-11.json` and `.log`. Full goal remains open.

### 2026-10-11: corrupted/truncated exact LOD cache regeneration

Fixed an opening-error path that retained a truncated optional LOD indefinitely. Reader-open length/I/O damage now invalidates the optional cached level; memory refusal and cancellation retain it. A real-file regression corrupts a cached pixel byte, then separately truncates the regenerated file: each request refuses damaged output, drops only the optional level, and the next request reconstructs the exact reference from the intact original source. Budget refusal/cancellation keep the valid entry; final memory/index/disk credit is zero.135 cache units and7 actual Metal viewport tests pass. Evidence: `research/metal-exact-lod-cache-damage-recovery-2026-10-11.json` and `.log`.

### 2026-10-11: current cached-LOD Metal soak and ordinary GPU gate

A60-second swapped viewer soak with exact LOD cache completed10364 cycles and31092 exact detail frames, with10364 CPU and31092 GPU pressure refusals. Final all-resource accounting is zero; peaks CPU2662400/texture160000/upload queue160000 bytes. The current ordinary GPU suite explicitly selected Metal: 116 tests passed,12 manual/corpus-heavy cases ignored and listed in the evidence. This expands current backend evidence across compute/RAW/color/resource behavior without treating skipped cases as passed. Small synthetic source, warm filesystem and managed budget pressure only. Evidence: `research/metal-cached-lod-pressure-soak-and-gpu-gate-2026-10-11.json` and `.log`. Physical HDR/NVIDIA/slow-disk and full-format scope remain open.

### 2026-10-11: current manual full-sensor compute and large raster Metal checks

Four separately invoked manual tests pass on Metal Apple M4 Max. Managed exposure covers all25494560 pixels exactly, end-to-end241.162ms (interleaved326.546ms), tracked GPU/CPU peaks1223738992/815825920 bytes and final zero. Already-resident compute encode/submit/wait median3.006ms excludes setup/upload/full readback and checks three positions; the separate managed test checks every pixel.12MP upload cancellation preserves prior texture across six validation/admission checkpoints.36MP native HDR area filtering produces1M exact pixels in161.60ms and exact SDR/HDR Metal framebuffers, final output credit zero. Synthetic exposure does not qualify complete RAW color/demosaic. Evidence: `research/metal-full-sensor-compute-large-raster-current-2026-10-11.json` and `.log`. Physical HDR/NVIDIA/full goal remain open.

### 2026-10-11: hardware raster timing and empirical Metal HDR extremes

Three additional manual checks pass on Metal Apple M4 Max.30 hardware render-pass samples for uploaded4096x4096 source to2048x2048 HDR target have median0.355333ms; excludes decode/upload/readback/UI/physical presentation. Empirical binary16 exposure saturation and opposite-f32-extreme interpolation produce the specified finite outputs without NaN/Inf. These tests remain outside the portable caller contract and do not extend cross-backend guarantees. Five corpus-dependent manual GPU cases remain (HERO9, Fusion front/back, SD10 and F828). Evidence: `research/metal-raster-timestamp-and-hdr-extremes-current-2026-10-11.json` and `.log`. Full goal remains open.

### 2026-10-11: current manual Metal corpus preflight contradicts old-path availability

Current filesystem inspection and streaming hash preflight find all five pinned external files absent: Fusion front/back, HERO9, SD10 and F828. Repository tests, including ignored files, contain no matching RAW sources. Historical successful reports remain historical evidence; five corpus-dependent checks cannot be freshly executed from these paths. Added `scripts/audit-metal-required-inputs.py` to report all missing/mismatched/unreadable inputs and fail closed rather than stop after the first missing file. Its real current audit exits1 and records five missing inputs in `research/metal-required-inputs-current-audit-2026-10-11.json`. This is a qualification-data gap, not a failure of the separately refreshed synthetic/managed Metal tests. Full goal remains active; native/cache/format work can continue independently.

### 2026-10-11: recover pinned official HERO9 and refresh full-frame Metal display

The official pinned GoPro HERO9 GPR was downloaded and SHA256-verified. Native Rust production development regenerated372031488-byte5568x4176 linear pixels whose SHA256 exactly matches the historical pinned fixture. Stored in ignored `tests/fixtures/data/metal-recovery/`, outside temporary directories. Current full-frame Metal display transport checks23251968 pixels, maximum RGB8 difference1, pass. This is display transport evidence, not independent complete demosaic/color qualification. The input audit now supports relocated corpora via `--input-root`, retains pinned checksum requirements, and uses portable streaming SHA256; it verifies HERO9 and reports the other four missing inputs. Evidence: `research/metal-official-hero9-recovery-and-display-2026-10-11.json`, `.log` and `metal-required-inputs-recovered-hero9-audit-2026-10-11.json`.

### 2026-10-11: recover both pinned official Fusion camera fixtures

Downloaded Fusion GPBK7066/GPFR7066 from the pinned GoPro revision and verified both source checksums. Native Rust production development reconstructs3104x3000 linear pixels (148992000 bytes each) matching both historical pinned SHA256 values exactly. Both current Metal full-frame display transport checks pass:9312000 pixels per camera, maximum RGB8 difference1. Recovered corpus now verifies HERO9 plus both Fusion fixtures; SD10/F828 remain missing. Display transport is not independent complete demosaic/color qualification. Evidence: `research/metal-official-fusion-recovery-and-display-2026-10-11.json`, `.log` and `metal-required-inputs-recovered-gopro-audit-2026-10-11.json`.

### 2026-10-11: all five required Metal corpus inputs recovered and hash-verified

Recovered pinned SD10 and DSC-F828 RAW sources. The independent pinned converter reproduces SD10 linear PPM exactly; the test-only LibRaw0.22.2 unpack utility reproduces F828 sensor oracle exactly. Both generated SHA256 values equal historical pins. Current SD10 Metal full-frame display2267x1513 has max RGB8 deviation1; F828 RGBE compute checks33062400 channel values with final CPU/GPU credit zero. The portable streaming audit now verifies all five original input hashes under the persistent ignored test-data directory and exits0. These checks do not prove universal X3F color correctness or F828 as-shot WB; physical HDR/NVIDIA/full-format scope remains open. Evidence: `research/metal-sd10-f828-recovery-and-full-corpus-audit-2026-10-11.json`, `.log` and `metal-required-inputs-all-recovered-audit-2026-10-11.json`.

### 2026-10-11: current integrated storage/viewer/Metal run with recovered corpus

The qualification driver completed24 requested suites with512 passing test executions and zero failures; every requested suite has verified execution evidence. Includes storage/cache, full ordinary viewer, QOI controls, model readback, TTL/HDR restoration, camera color, Metal GPU, pinned full camera transport/F828,8 live policy cycles plus12MP policy control,65-source/94-selection raster collection, prefetch reversals, all five linear import paths, viewer TTL-prefetch, compute cache,8 model-swap cycles and full-sensor compute.72 ignored executions appear in individual invocations, some later run explicitly; none counted as passes. This is not512 unique tests or proof of full100 formats. Physical HDR/NVIDIA/physical slow disk remain open; swapped-source residency/soak evidence is separate. All driver suites verified. Evidence: `research/metal-integrated-recovered-corpus-current-2026-10-11.json` (commands, counts, limitations and full-log SHA256) and `.log` (concise result markers).

### 2026-10-11: mandatory swapped-source checks in default Metal qualification

Fixed an evidence gap: ordinary viewer tests skip hardware residency cases, so a default Metal qualification could previously omit new swapped-source behavior. The non-GPU-only Metal driver now always executes7 viewport checks and6 integration/navigation checks, requiring zero ignored cases and semantic evidence markers for36MP exact LOD cache, repeated pressure, corrupt-block frame retention and delayed source cancellation. Current default11-suite run completes with501 successful test executions, zero failed suites and both new workload-evidence checks true. This gate now preserves future coverage instead of relying on manually remembered followups. Evidence: `research/metal-default-gate-requires-swapped-residency-2026-10-11.json` and `.log`. Full goal/physical hardware limits remain open.

### 2026-10-11: pending RAW source availability audit

Current public raw.pixls.us2016-entry snapshot was inspected by original download extension: MEF1; CAP/PTX/BAY/RWZ0. Kept all pending acceptance requirements intact; no renamedIIQ/PEF alias is promoted to original camera-file support. This bounded search guides fixture acquisition, not universal absence or decoder completion. CAP extension context must also distinguish Phase One camera RAW from unrelated network-capture files. Evidence: `research/pending-raw-original-fixture-availability-2026-10-11.json`. Full goal remains active.

### 2026-10-11: refreshed sustained swap qualification

All three explicitly ignored storage scenarios pass on the current checkout: 100000 distinct keys with 100000 writes/reads and 1000 refused-then-retried restores (93.35s); independent byte cap, 443 cycles in 60.09s, peak 786624 bytes/3 objects despite count=100 and TTL=3600s; delayed-I/O/count/TTL configuration, 438 cycles in 60.04s, peak 4195328 bytes/16 objects. Managed source/queue credits end at zero in every scenario. Tiny key-churn payloads and simulated codec delays do not qualify full-resolution 100000-image collections, physical slow disks, or OS-wide pressure. [Evidence](research/cache-sustained-three-scenarios-current-2026-10-11.json).

### 2026-10-11: independent sustained count cap

Added an ignored sustained count-only scenario, included automatically by the storage-soak driver. With byte cap 8MiB and TTL 3600s, count=3 alone evicts old keys: 443 exact restore/pressure-retry cycles in 60.12s, peak 3 objects/786624 bytes, final managed credits zero. A final 10s rerun passes after making the evicted-key assertion safe across the 100000-key wraparound. This supplements independent byte-cap and existing idle-TTL tests; physical slow disks and OS-wide pressure remain unqualified. [Evidence](research/cache-independent-count-cap-2026-10-11.json).

### 2026-10-11: idle TTL and required storage workloads

Four ordinary sustained-swap controls pass: idle TTL reclaims physical files without foreground prune; cancellation releases partial allocations and permits exact retry; stale lock waiters release credit for latest work; queue bytes/count bound delayed overload. The optional storage-soak driver now requires all eight named scenarios, zero ignored executions, and explicit 100000 read/write evidence. AST-executed validator checks accept assembled successful current logs and reject each missing scenario, insufficient passes, and ignored execution. This validates the verifier without claiming a new full integrated run. [Evidence](research/cache-idle-ttl-and-required-storage-gate-2026-10-11.json).

### 2026-10-11: integrated Metal plus required storage workloads

A single updated qualification-driver invocation passes all 12 suites (509 passing test executions, not unique tests), including seven swapped viewport tests, six source-navigation/worker tests, and all eight mandatory storage scenarios with zero ignored tests in the storage workload. Each log SHA256 was verified. Three delayed-I/O modes each ran 10 seconds; the complete 100000-key churn also ran. This refreshes the integrated gate after count-only coverage and strict workload admission were added; separate 60-second evidence remains available. Physical HDR, NVIDIA, OS-wide pressure, physical slow disks and full 100-format qualification remain open. [Evidence](research/metal-integrated-required-storage-current-2026-10-11.json).

### 2026-10-11: thumbnail worker ownership at shutdown

Thumbnail Prefetcher previously cancelled but detached its loader thread, allowing resources to outlive owner destruction. It now owns the join handle, cancels/drains work, disconnects the job sender and joins after releasing the publication lock. A paused-loader regression proves destruction waits for worker completion and cannot deadlock on publication. Thumbnail, gallery and full ordinary viewer tests pass; external/hardware ignored tests are not newly qualified. [Evidence](research/thumbnail-prefetch-owned-worker-shutdown-2026-10-11.json).

### 2026-10-11: Metal navigation and decoder-owner release

Refreshed actual Apple M4 Max mixed navigation: 120 direction reversals/transitions over six formats, exact SDR/HDR frames, stale generation refusal and final managed root zero. Strengthened the separate thumbnail shutdown regression with a managed credit owned by the decoder closure: it stays charged while the paused loader runs and returns to zero before destruction completes. Both tests execute and pass; this is separate transport/lifecycle evidence, not native UI or physical HDR qualification. [Evidence](research/metal-navigation-and-thumbnail-decoder-credit-2026-10-11.json).

### 2026-10-11: recover original MEF and independent sensor oracle

Recovered the pinned Mamiya ZD source from raw.pixls.us into persistent ignored local fixture storage; SHA256 matches the original pin. Test-only LibRaw 0.22.2 independently regenerates full sensor reference. All nine native MEF tests execute and pass, including full-sensor equality, budget/cancellation ownership and public viewer refusal. Oracle WB remains [0,1,0,0]; no photographic color, native WB or full MEF viewer support is claimed. First attempt lacked the oracle environment and failed that one test; corrected run passes all nine. ExifTool is unavailable, so no renewed MakerNote extraction is claimed. [Evidence](research/mef-original-and-sensor-oracle-recovery-2026-10-11.json).

### 2026-10-11: streamed BAY sensor input

Explicit-layout native BAY file import now holds one managed input row instead of the full input beside the u16 output. Declared size is checked before admission; reads are cancellation-aware and reject truncation/growth. Every authored sample for all three layouts is checked independently, including QV5700 trailing sample/padding. Exact output+row admission succeeds and one-byte-short admission releases all credits. Five focused tests and the full ordinary decoder unit suite pass. Original camera interoperability, CFA/WB/color/viewer routing remain pending; this does not close BAY format acceptance. [Evidence](research/bay-row-streamed-managed-import-2026-10-11.json).

### 2026-10-11: BAY late cancellation and changing stream

Added deterministic fault injection around the same row reader used by file import. Cancellation after first/middle/final payload reads and final EOF read returns no sensor and releases all credits. Fixed missing cancellation recheck after EOF. Truncated or enlarged streams after size admission reject partial results and release output+scratch. Six focused tests and full ordinary decoder unit suite pass; original BAY color/viewer qualification remains open. [Evidence](research/bay-stream-late-cancellation-and-faults-2026-10-11.json).

### 2026-10-11: MEF streaming feasibility from original layout

Audited the recovered pinned original directly: sensor strip starts at 4383052 and spans 32192256 bytes; 4016x5344 u16 output is 42923008 bytes. Current file importer peak is 79498316 bytes from source+output. Valid root/SubIFD tables are before payload; two following pointers lead to non-TIFF data (entry counts 4084/4088). A safe streamed importer requires bounded seekable metadata/range validation, not simply skipping generic TIFF errors or hardcoding this fixture prefix. Production MEF streaming is not implemented by this audit. [Evidence](research/mef-streaming-layout-audit-2026-10-11.json).

### 2026-10-11: MEF staged source release

Native file import now validates TIFF and hashes packed sensor bytes, releases full source memory, then rereads rows on the same open file into managed output. Digest and final file length must match before publication. Pinned original peak drops from 79498316 to 42929032 bytes; relocated source remains accepted with max(source,output+row) admission. All nine real MEF checks and full ordinary decoder unit tests pass. Two-pass I/O/hash cost is not claimed faster; full input is still temporarily allocated for metadata validation. Deterministic mutation fault injection remains to be added. WB/color/viewer support stays pending. [Evidence](research/mef-staged-source-release-import-2026-10-11.json).

### 2026-10-11: MEF second-pass fault qualification

File import now shares its verified second-pass reader with deterministic fault tests. Mutated first/middle/last sensor byte, one-byte truncation and cancellation after first/middle/final row all refuse publication and release output+scratch. Unchanged authored samples are exact; all original real-MEF/oracle controls remain passing. Ten MEF tests and full ordinary decoder suite pass. This closes the sensor digest/cancellation fault gap from staged import; final file-length change injection, native WB/color/viewer and physical hardware gaps remain open. [Evidence](research/mef-second-pass-integrity-cancellation-2026-10-11.json).

### 2026-10-11: real MEF full-sensor Metal transport

Added and ran a pinned-original integration test on actual Apple M4 Max Metal. Native decoded MEF codes at all 21461504 sensor positions map to explicitly authored signed/HDR/alpha channels; +1 stop managed compute matches every output exactly. Tracked CPU peak 729691136 and GPU peak 1030152288 bytes, both final zero. Compute/transfer/readback 243.418ms excludes source decode, input construction, oracle comparison and display. This qualifies full-sensor transport only; no Mamiya WB/color/demosaic or viewer support is inferred. [Evidence](research/mef-real-full-sensor-metal-compute-2026-10-11.json).

### 2026-10-11: integrated pinned MEF Metal gate

Qualification driver adds --mef-sensor-source with pinned original SHA256, mandatory Metal backend, environment wiring and strict full-sensor/exact-channel/final-zero workload markers. One actual invocation of all ordinary GPU suites plus original MEF passes 117 test executions across 2 driver suites; each log hash verified. Three invalid input/backend invocations fail before GPU execution; validator rejects missing markers, zero passing tests and ignored execution. MEF camera color remains explicitly unqualified. [Evidence](research/metal-integrated-pinned-mef-gate-2026-10-11.json).

### 2026-10-11: independent BAY authored sensor oracles

Added immutable source/reference SHA256 pins and an ignored external-corpus test comparing every native streamed sensor sample against independent test-only LibRaw 0.22.2 output. All 10167652 samples across QV2000UX, QV3000EX and QV5700 match exactly, including storage columns. Seven BAY tests pass with explicit corpus; full ordinary decoder tests pass separately. Managed credits end at zero. These are authored packing fixtures, not original camera-file interoperability or CFA/WB/color/public viewing; BAY remains pending. [Evidence](research/bay-independent-authored-sensor-oracles-2026-10-11.json).

### 2026-10-11: refreshed strict AI failure and localization

Recovered both original pinned AI files into persistent ignored corpus. Current test-only native replay completes all three pages; fresh strict Poppler comparison still fails with VectorApple 33072 pixels/max218, one.ai page0 2494/max58, page1 exact. Localizing mismatches against four equal oracle neighbours gives 787 flat-neighbourhood candidates in VectorApple (max42), zero in one.ai. This is a diagnostic classifier, not proof of a geometry/color cause; it directs separate investigation of one.ai boundaries and VectorApple source/compositing/color. RGB tolerance is unchanged and AI stays pending. [Strict evidence](research/ai-native-strict-current-2026-10-11.json), [localization](research/ai-native-current-difference-localization-2026-10-11.json).

### 2026-10-11: one.ai source-curve boundary oracle

Decoded original page content identifies two black cubic-filled shapes, not strokes. Four largest boundary mismatches are independently integrated directly from source cubic control points: converged coverage 0.772340531678 and 0.806089008856 implies RGB8 gray 58/49, matching native output while Poppler is black. A native area regression passes at all four pixels in both f64 and canonical-f32 contour modes; area error bound for the regression is 0.0002 and RGB8 exact. This is selected-pixel geometric evidence, not a formal interval proof, full-page fidelity or permission to relax strict comparison. Avoid changing geometrically supported coverage merely to match this Poppler rasterization. [Evidence](research/ai-source-cubic-boundary-oracle-2026-10-11.json).

### 2026-10-11: full-page one.ai independent source-polynomial oracle

Added a standalone pinned-source geometric verifier for the two original cubic fills on one.ai page0. It splits at pixel-boundary roots and integrates x(t)*dy/dt polynomial antiderivatives, without native renderer, flattened paths or Poppler. Full RGB8 output is stable between 32/52 bisections; current native differs at 17 pixels by exactly one channel level, compared with 2494 Poppler differences. The verifier correctly exits failure for those 17 pixels; no tolerance or AI acceptance is relaxed. Initial Simpson 64/256 experiment had eight unstable pixels and was replaced. Numerical convergence is not a formal interval certificate; source fidelity and AI production viewing remain pending. [Evidence](research/one-ai-full-page-source-polynomial-oracle-2026-10-11.json).

### 2026-10-11: AI f64 contour counterfactual

Fresh same-source replay without extra f32 contour canonicalization completes but leaves 20 independent source-oracle pixels off by one, all lighter than expected (baseline canonical-f32:17). Strict one.ai Poppler comparison remains 2494 differences. Removing canonicalization is rejected as a fix. Next candidate is lost area from inscribed curve flattening near RGB8 rounding boundaries, requiring direct accuracy verification; no renderer or acceptance threshold changed. [Evidence](research/ai-f64-contour-counterfactual-2026-10-11.json).

### 2026-10-11: finer native contour matches full one.ai source oracle

Direct tolerance experiment reproduces all 20 f64 residual pixels at 1e-4 and removes them at 1e-5 (8196 to 32772 total contour vertices); finer 1e-6/1e-7 adds 88924/262148 vertices with no gain for these cases. Test-only native prepared contour now uses 1e-5 without increasing work/vertex/memory caps. Fresh full one.ai page0 RGB matches the independent source-polynomial oracle exactly, zero differences. Both real scenes complete within original 256MiB admission, peak VectorApple232664374 / one169698840 bytes, final zero; 135 compositor regressions pass,10ignored. Diagnostic replay ~30.3s/~29.6s versus prior f64~11.9s/~10.1s is a material performance cost, not an optimized production result. Strict Poppler failures, VectorApple appearance and PostScript AI remain pending. [Evidence](research/ai-finer-native-contour-source-exact-2026-10-11.json).

### 2026-10-11: reject disjoint edge crossings before slope division

Test-only polygon crossing kernel now skips vertically disjoint pairs before division, retaining the same work charges and cancellation. Both original AI scenes complete; all six component/RGBA outputs are byte-identical to finer-contour baseline, preserving the full one.ai source-oracle result. 135 regressions pass,10ignored. Single-run timings are recorded without claiming a stable speedup; production AI and VectorApple fidelity remain open. [Evidence](research/ai-disjoint-crossing-rejection-2026-10-11.json).

### 2026-10-11: repeated crossing experiment rejected

Four ABBA replays preserve byte-exact outputs and release all admitted memory. Earlier single-run speedup does not reproduce: VectorApple baseline25.56–25.64s versus changed33.46–46.32s under concurrent heavy host load. Early-rejection change reverted; finer contour accuracy remains. This supersedes the preceding timing observation, not the pixel-accuracy evidence. [Repeated evidence](research/ai-disjoint-crossing-abba-2026-10-11.json).

### 2026-10-11: isolate VectorApple green DeviceCMYK mismatch

Pixel216,25 traces to shading event1023 with coverage255,opacity1,mask1 and CMYK[0.8794023,0.23389284,0.99231184,0.08509913]. A standalone constant DeviceCMYK fill with those values produces PopplerRGB[38,135,62], exactly its original pixel; nativeRGB[0,135,68]. This isolates a selected color-policy discrepancy rather than requiring a geometry change; not a classification of all33072 mismatches. Added source sample tracing and --nocapture vendor runner option. Six outputs remain byte-identical,135 regressions pass,10ignored,all replay credit released. AI stays pending. [Evidence](research/ai-vectorapple-green-device-cmyk-isolation-2026-10-11.json).

### 2026-10-11: independent explicit-profile check for selected VectorApple green

Same selected CMYK sample transformed independently by LittleCMS with pinned native CGATS profile yields clipped RGB8[0,135,68], matching native. Authored explicit DefaultCMYK PDF changes Poppler to[0,134,67] from bare[38,135,62]; one-code residual remains. Added native regression against independent profile result. No source-profile policy or acceptance threshold changed; original AI remains pending. [Evidence](research/ai-vectorapple-green-explicit-profile-2026-10-11.json).

### 2026-10-11: selected VectorApple white-pixel coverage discrepancy

Original pixel78,144 is nativeRGB[253,237,213], Poppler white with four white neighbours. Trace identifies root shading2676,coverage108/255,opacity1,mask1,validCMYKsample. Thus selected missing coverage cannot be addressed merely by changing display profile; original path/clip independent area analysis is still required. Raster traces now expose source coordinates, interpolation and alpha metadata including nested masks. Six outputs remain byte-identical,all pages complete,finalcredit0;135 regressions pass,10ignored. No fidelity fix claimed. [Evidence](research/ai-vectorapple-white-pixel-coverage-isolation-2026-10-11.json).

### 2026-10-11: selected VectorApple source-clip cubic area oracle

Captured pre-flatten clip for shading2676; independent cubic-y inversion/nonzero-winding Simpson256/1024/4096 gives area0.425110034717,all quantize108/255 matching native. Authored isolated clip Poppler coverage at1x is4/255;100x mean0.42592039 and400x0.42532152 approach geometric area. Crop does not exactly reproduce full-page white; no independent PDF parser or formal interval proof claimed. This supports keeping native area rather than forcing white. Added reusable source-clip integration probe;135 regressions pass,10ignored,six outputs byte-exact,finalcredit0. Full AI fidelity remains open. [Evidence](research/ai-vectorapple-selected-clip-cubic-oracle-2026-10-11.json).

### 2026-10-11: pinned native AI replay planes through actual Metal

New explicitly ignored external-corpus GPU test pins both originals and three native RGBA planes, converts the strict diagnostic white composite to managed linear samples, uploads with independent texture/queue budgets and draws at1:1 on actual Metal AppleM4Max. All1,154,109 pixels across three pages match exactly; portable roundtrip bound remains one code. TrackedCPU/texture/upload credit final0; harness readback/driver/OS allocations excluded. First attempt used fit margins and was corrected in test zoom,shader unchanged. Test passes1,ignored0. This qualifies transport of test-only replay outputs,not production AI decoder/full fidelity/physicalHDR. [Evidence](research/ai-native-managed-metal-transport-2026-10-11.json).

### 2026-10-11: integrated pinned AI Metal transport gate

Combined driver now accepts --ai-native-root with mandatory Metal backend and five immutable source/plane SHA256 inputs verified before output creation. Requires exact page identities/counts,total1,154,109 pixels,per-plane error<=1,finalcredit0 and actual Metal marker. Full current gpu-only driver completed2suites,117 passing executions,allverified; log hashes checked. Six malformed/skip workload cases and three wrong/missing-input CLI cases rejected. Ordinary ignored GPU tests remain unqualified; explicit AI transport test passes with0ignored. Original AI fidelity and physicalHDR remain open. [Evidence](research/metal-integrated-pinned-ai-transport-2026-10-11.json).

### 2026-10-11: native AI planes through RAM/swap pressure and Metal

Pinned three replay pages in both sRGB8 and linearRGBA32F pass six disk cases on actualMetalM4Max: full sample/metadata equality, exact restored GPU frame, managed pressure refusal,cancellation Ok(None),successful retry,writes1/reads1/errors0,queue0/root0. Live RAM lease blocks eviction until release,then zero-size cap evicts. GPU texture/upload budgets also released; harness/driver/OS excluded. Test passes1,ignored0. Production AI fidelity,physicalslowdisk,OSpressure andphysicalHDR remain unqualified. [Evidence](research/ai-native-ram-swap-pressure-metal-2026-10-11.json).

### 2026-10-11: combined current storage/viewer/Metal with pinned AI

Default combined driver with --ai-native-root now mandates AI RAM/swap pressure/cancel/retry in addition to pinned transport. Completed13suites,503passing executions,allverified;all logSHA256 checked. Includes cache,viewer,hostCUDA,QOI,model,HDR/TTL,color,GPU,swappedviewport/navigation andbothAIgroups. 73ordinary ignored executions remain explicitly unqualified. Eight malformed/skip AI-swap evidence variants rejected. No extended soak requested in this invocation;previous soak evidence remains separate. ProductionAI/full100formats,NVIDIAhardware,physicalHDR/slowdisk/OSpressure remain open. [Evidence](research/metal-integrated-ai-storage-current-2026-10-11.json).

### 2026-10-11: thumbnail queue completion under pressure without redraw

Strengthened previously ignored actualMetal callback test:2cycles,queue256B,texturelimit8B;firsttileholds4Btexture/256Bqueue,secondtiledeniedandprovisionaltexturecreditrolledback,rendererDropkeepsinflightcredit,explicitpollretiresallcreditwhilemetadataworkerblocked,nextcycleworks. ActualM4MaxMetal passes1,ignored0. Productionunchanged;notappearance/HDRqualification. [Evidence](research/metal-thumbnail-completion-pressure-retry-2026-10-11.json).

### 2026-10-11: actual Metal full supported texture width

Strengthened texture-limit test from one8193x128 center to full8193x128 and16384x128 attachments on actualMetal(limit16384). Every3,145,856 HDR pixel exactbinary16red;everySDRpixelvisibleopaque red. Portable8192device uploads tiled;viewer-supporteddeviceexactsource-sizeGPUallocation;managedCPU/native/portabletexturecreditfinal0. Test passes1,ignored0. Authoredconstantcolor doesnotqualifyorientation/allinterpolation/physicalHDR. Productionunchanged. [Evidence](research/metal-full-supported-texture-extent-2026-10-11.json).

### 2026-10-11: coordinate pattern across native and tiled Metal extents

Replaced constant source in strengthened8193/16384x128 test with per-coordinate linearRGBA[x&1,y&1,(x>>10)&1,1]. Full3,145,856 SDRpixels andsameHDRpixels matchexactly;8portabledevice128x128crops add131,072exactHDRpixelchecks atedges/interiortileboundaries. ManagedCPU/native/portabletexturecreditfinal0. Test passes1,ignored0. Initialtestcropoutside8193sourcecorrectlyreturnedbackgroundandwasbounded;shaderunchanged. Periodicpattern doesnotproveeverypermutation/scale/physicalHDR. [Evidence](research/metal-texture-coordinate-pattern-2026-10-11.json).

### 2026-10-11: original AI pages through production cache and Metal

Pinned originals pass current production raster load and page1preload;threeindependentkeys retainsharedRGBA32F. Fullmanaged64MiBrootpressure doesnottriggerredecode/alloconcachehit;dimensions/pageindices/countpreserved,128x96SDR/HDRMetalframesexactbefore/afterhit,root0aftercachedrop. StrengthenedCPUcaseandnewMetalcasepass2,ignored0. Noindependentproductionappearance,asynchronousneighbourscheduler,OSpressure orphysicalHDRqualificationclaimed. [Evidence](research/real-ai-production-cache-metal-pressure-2026-10-11.json).

### 2026-10-11: original AI queued page reversal and Metal

ProductionForegroundLoader submit/coalescing with test-owned backgroundpreload thread:activeVectorAppleblockedatdecodegate,one.ai page1->0->1 leavesonlylatestrequest,stalegeneration refusedwithoutcacheentry/rootcredit. Latestpage1loadsafterjoin,neighbourpage0independentkey,count2;fullmanagedrootpressurehitsharesallocationandpreserves128x96SDR/HDRMetalframesexactly,root0. ActualM4MaxMetal passes1,ignored0. Thisdoesnotinstantiatewindow/eventloopworkerorprovecancellationmid-AIrender;appearance/physicalHDRremainopen. [Evidence](research/ai-queued-page-reversal-metal-2026-10-11.json).

### 2026-10-11: original AI cancellation after renderer scratch admission

ProductionVectorApple decode invalidated only after observedlivecredit exceeds2718090B(two sourcecopies+finalRGBA8+pixmap),provingrenderer-stage scratch admission. TypedCancelled returnswithroot0;latest-generationcacheloadfullRGBA32Fsamplesand128x96SDR/HDRMetalframesexactagainstuncancelledreference,bothroots0. Three separate successful runs,ignored0;no productionchange. Timingobserver notuniversalcompletion-percentage guarantee;AIappearance/physicalHDRremainopen. [Evidence](research/real-ai-render-scratch-cancel-metal-recovery-2026-10-11.json).

### 2026-10-11: current-code live Metal HDR/EDR surface refresh

Connectedbuilt-inLiquidRetinaXDR,currentM4MaxMetalprobe presents178RGBA16Float/ExtendedSrgbLinearframesover3.003s,validationerrors0. OSreportedEDRheadroom1beforeconfigure,rises16duringpresentation,returns1afterownedwindowexit(read-onlyAppKitquery). No displaysettingchanged. RefreshesexistingOct8EDRactivationevidence;notmeasurednits/chromaticity/calibration/PQ-HLG/fullHDRformatqualification. [Evidence](research/metal-live-hdr-surface-refresh-2026-10-11.json).


### 2026-10-11: integrated mandatory Metal gate

Added previously manual Metal thumbnail completion and large texture checks to the default full runner; pinned AI mode additionally requires production cache, queued reversal and render-stage cancellation/recovery. Full current run: 18 suites / 508 passed executions / 76 ignored, no failures, all executed suites verified; preserved per-suite logs and hashes. Current generalized texture evidence validator rechecked against the log and four negative variants. See `research/metal-integrated-final-2026-10-11.json`. Physical HDR calibration, complete format qualification and AI appearance remain open; NVIDIA deferred by user.


### 2026-10-11: native declared Radiance BT.2020/D65

Radiance RGBE now recognizes explicit BT.2020/D65 primaries and prepares scene-linear RGB through the shared native BT.2020-to-sRGB matrix. The source float samples are retained; output preserves values above one and negative gamut components. Conversion uses managed output admission and cancellation; unfamiliar primaries/white points stay unqualified. CICP recognition is restricted to primaries 9, linear transfer 8, identity matrix 0 and full range.

A separate Gaussian inversion of published BT.2020/BT.709 xy/D65 coordinates confirms the matrix. Authored 1×1 RGBE red=2 exercises decode, bounded output refusal/retry, native color preservation through disk swap, pressure/cancellation recovery and actual Metal SDR/HDR frame equality against the analytical reference, with root credit returning to zero. Core: 109 passed / 2 ignored; HDR decoder: 4 passed / 0 ignored; focused Metal: 1 passed / 0 ignored; full HDR regression: 15 passed / 3 ignored. The focused GPU case is included in the existing ordinary `hdr_raster_swap` suite and therefore in the default integrated gate.

Evidence: `research/radiance-bt2020-matrix-oracle-2026-10-11.json` and its preserved hashed logs. This authored fixture does not qualify arbitrary Radiance primaries, exposure/color-correction semantics, broad producer/orientation coverage or physical HDR calibration.


### 2026-10-11: declared Radiance Display P3/D65

Extended the native linear-primary matrix path to CICP 12/8/0/full and explicit Radiance Display P3/D65. RGBE source samples remain unchanged; prepared floats preserve above-one and negative gamut components. A four-pixel RGBW HDR fixture covers every matrix column and white neutrality using a separate Gaussian inversion of H.273 primary coordinates. Actual Metal SDR/HDR frames match the analytical reference before and after source swap, memory refusal/cancel/retry; managed root credit returns to zero. The shared integration helper also strengthens the earlier BT.2020 case with preparation-pressure/cancellation and direct per-sample analytical checks.

Core tests: 109 passed / 2 ignored. Entire HDR Metal regression: 16 passed / 3 ignored, no failures. Both declared-primary tests are ordinary tests included by the default integrated runner. Evidence and preserved hashed logs: `research/radiance-p3-matrix-oracle-2026-10-11.json`. Arbitrary primaries/non-D65 whites, exposure semantics, broad original corpus and physical HDR calibration remain unqualified.


### 2026-10-11: explicit OpenEXR BT.2020/P3 color declarations

Native color declaration parsing now accepts linear BT.2020/D65 and P3/D65 via explicit chromaticities or standard `lin_rec2020_scene` / `lin_p3d65_scene` IDs. Conflicting known IDs/chromaticities and incompatible ACES flags are refused. Unknown IDs override otherwise familiar chromaticities; non-D65 adopted neutral remains unqualified.

14 authored metadata variants retain independently encoded EXR channel chunks byte-for-byte, with relocated scanline offsets. Generation is deterministic; all 16 fixture/oracle pins verified. Six accepted metadata cases preserve analytical samples and SDR/HDR frames on actual Metal before/after swap and preparation/storage memory refusal, cancellation and retry; managed root credit returns to zero. Four conflicting and four unknown/neutral cases exercise refusal policies. Full HDR regression: 17 passed / 3 ignored. Final focused current-source test: 1 passed / 0 ignored. Existing integrated gate includes the new ordinary test. Evidence: `research/exr-wide-color-metal-2026-10-11.json` and preserved hashed logs. Other EXR primaries/white points, layers/deep/tiled, zero-alpha emission rendering and physical HDR calibration remain open.


### 2026-10-11: general native linear-primary matrix constructor

Added public `linear_rgb_to_srgb_from_xy([f64; 8])`, using stack-only XYZ basis inversion and existing Bradford adaptation for a different source white, without transfer/tone mapping or clipping. Unnormalized primary XYZ supports imaginary AP0/AP1 and XYZ-axis y=0 primaries; physical white and nonsingular finite bases are required. Existing linear BT.2020/P3 display preparation now uses this constructor. Current core tests: 112 passed / 2 ignored; entire HDR actual-Metal regression: 17 passed / 3 ignored. Evidence: `research/linear-primary-matrix-2026-10-11.json`.

This establishes native matrix calculation, not arbitrary-source viewer support. Arbitrary per-frame chromaticity metadata, versioned swap serialization and decoder routing remain to implement. Non-D65 independent matrix and arbitrary-source pixel/frame qualification also remain open; neutral-only AP0/AP1 checks are not full color qualification.


### 2026-10-11: arbitrary linear chromaticity frame metadata and swap V3

Added validated `RgbChromaticities` and `RasterColorSpace::LinearRgb`: eight exact f64 xy coordinates held inline as fixed little-endian bytes, with exact Eq/Hash and native linear-sRGB preparation through the general matrix constructor. Raster payload V3 stores a 64-byte coordinate block; metadata reading uses stack storage, with no separate heap/scratch admission. Existing V1/V2 payloads remain readable. New tag/version, metadata length and invalid basis/nonfinite combinations are refused.

Exact full-frame metadata/sample/aspect round trip succeeds with pixel-only output budget. Segmented swapped-region coverage now exercises both CICP and explicit chromaticities, with aspect, scale, selection, hotspot and every sample bit retained after full-source release. Direct explicit-xy P3 frames also pass analytical preparation and actual Metal SDR/HDR equality before/after preparation/storage pressure, cancellation and retry, with root credit zero. Current core: 112 passed / 2 ignored; full cache crate: 147 passed / 5 ignored; full HDR Metal: 18 passed / 3 ignored. Evidence: `research/linear-chromaticity-metadata-2026-10-11.json`.

File decoder routing for arbitrary source primaries and non-D65 independent full-color/frame qualification remain open. This milestone qualifies frame metadata and its storage/render route, not arbitrary EXR/Radiance originals or physical HDR calibration.


### 2026-10-11: Radiance arbitrary explicit primary/white decoder routing

RGBE `PRIMARIES` now routes valid invertible declared bases and physical source white through `LinearRgb(RgbChromaticities)`; known Rec.709/BT.2020/P3 compact tags remain supported. Imaginary AP1/AP0 primaries are accepted by the native mathematical basis validator. Singular/nonfinite/invalid-white/duplicate declarations are refused; absent primaries stay unqualified and XYZE is never relabeled RGB.

Three authored non-D65 RGBW files (custom white, AP1, AP0) preserve every native source sample and exact color coordinates through preparation/storage full pressure, cancellation, retry and disk swap. Independent partial-pivot Gaussian XYZ/Bradford oracles cover every matrix column and neutral; double-precision and f32-cone precision variants bound approximation loss. Prepared samples and complete actual Metal SDR/HDR frames match analytical references, with root credit zero. Full HDR Metal suite: 20 passed / 3 ignored, no failures. Evidence: `research/radiance-arbitrary-color-metal-2026-10-11.json`, per-case matrix reports and preserved hashed log. Reusable oracle: `scripts/probe-linear-color-oracle.py`.

This qualifies the authored explicit-primary route, not all producers, missing declarations, exposure/COLORCORR calibration, scan orders, XYZE or physical HDR display calibration. Arbitrary EXR declaration routing is still pending.


### 2026-10-11: arbitrary OpenEXR chromaticities and ACES linear IDs

Single-part RGB EXR now routes valid arbitrary chromaticities/physical source white to native exact chromaticity metadata, including imaginary AP1/AP0 primary bases. Added standard `lin_ap1_scene` and `lin_ap0_scene` ID routing. Known IDs are authoritative when accompanying chromaticities agree; conflicts remain explicit errors. Matching adopted neutral is supported for any qualified source white; a different adopted neutral remains unqualified. Compatible AP0 ACES flags are accepted for color interpretation; non-AP0 conflicts are refused. Unknown IDs suppress chroma fallback. A standalone ACES flag without explicit chromaticities/ID is not qualified, and accepting the color flag does not certify SMPTE container layout.

26 metadata derivatives retain independently encoded EXR channel chunks unchanged; deterministic generation and all 38 file pins verified. Twelve accepted cases match independent Gaussian XYZ/Bradford analytical samples and full Metal SDR/HDR frames before/after managed preparation/swap pressure, cancellation and retry. Every accepted source retains original sample bits and coordinate metadata; tracked credit is zero after each accepted/refused/unknown case. Eight conflicting cases are refused; six unknown/neutral cases remain unqualified. Full HDR regression: 21 passed / 3 ignored. Final focused test: 1 passed / 0 ignored. Evidence: `research/exr-arbitrary-color-metal-2026-10-11.json`, preserved hashed logs and `tests/fixtures/exr-arbitrary-color.json`.

Missing or unknown declarations, different adopted neutral, broader producer/layer/deep/tile/associated-emission qualification and physical HDR calibration remain open.


### 2026-10-11: native independent adopted-neutral matrix calculation

Added `linear_rgb_to_srgb_with_adopted_neutral_from_xy`, preserving the declared RGB/XYZ basis while Bradford-adapting the independently declared adopted neutral. The existing helper delegates with the original white to preserve prior behavior. Two independent Gaussian oracles (Rec.709 and AP1 with different adopted neutral) verify all matrix coefficients and neutral mapping; tests distinguish incorrect source-basis re-normalization and reject invalid adopted neutral. The oracle generator now supports `--neutral`, including the correct transformed source-white RGBW reference.

Core tests: 114 passed / 2 ignored. Existing entire HDR Metal regression: 21 passed / 3 ignored. Evidence: `research/adopted-neutral-native-matrix-2026-10-11.json` and preserved hashed logs. This establishes matrix computation only. Frame/swap adopted-neutral metadata and distinct-neutral EXR source routing remain to implement; no actual distinct-neutral source is yet qualified on Metal.


### 2026-10-11: adopted-neutral frame metadata and swap V4

`RgbChromaticities` now retains optional adopted-neutral xy separately from the original RGB basis. The legacy coordinate API remains 64 bytes; ordinary metadata stays V3. Explicit adopted-neutral metadata uses an inline 80-byte block in V4, read on stack without separate metadata heap/scratch admission. V1/V2/V3 remain readable. Full and segmented restores retain adopted neutral, aspect, scale, selection, hotspot and sample bits. Corrupt version/length/tag or invalid adopted neutral is refused without leaked admission.

V4 round trips succeed with pixel-only output budget. Native Rec.709 and AP1 RGBW frames with different adopted neutral match independent Gaussian/Bradford analytical samples and exact Metal SDR/HDR frames before/after preparation/storage memory refusal, cancellation and retry, with root zero. Full cache crate: 148 passed / 5 ignored. Core library: 114 passed / 2 ignored, plus 3 core integration passes. Entire HDR Metal: 22 passed / 3 ignored. Evidence: `research/adopted-neutral-metadata-2026-10-11.json` and preserved hashed logs.

Distinct-neutral EXR decoder routing remains to connect. This qualifies typed frame/storage/render metadata; no distinct-neutral EXR original is qualified yet, and physical HDR calibration remains open.


### 2026-10-11: distinct adopted neutral in EXR on Metal

EXR decoding now retains a valid distinct `adoptedNeutral` independently of the original RGB basis, through typed frame preparation and V4 swap. This supersedes earlier notes that distinct adopted neutral remains unconnected. Invalid physical neutral coordinates are refused; unknown color IDs still suppress fallback. Known IDs use canonical basis coordinates.

On Apple M4 Max Metal, the full HDR suite passed 22 tests (3 ignored). Arbitrary EXR variants: 17 accepted, 11 refused, 6 unknown; wide-gamut variants: 10 qualified, 4 refused, 2 unknown. Accepted cases preserve native sample bits and metadata, match independent analytical colors and exact SDR/HDR frames before and after swap, and pass memory refusal/cancellation/retry with tracked root credit zero. All 73 fixture pins and deterministic regeneration verified. Evidence: `research/exr-adopted-neutral-metal-2026-10-11.json`.

These are authored metadata variants of independently encoded samples. Broader EXR producer/layer/deep/tile qualification and physical HDR calibration remain open. NVIDIA hardware is deferred at the user's request.


### 2026-10-11: integrated Metal after EXR adopted-neutral routing

Current integrated driver completed 29 suites: 530 passed, zero failed, 76 ignored. All suite execution checks and preserved log SHA256 values verified. In addition to default cache/viewer/GPU/AI transport gates, this run requires mixed-format prefetch navigation, all five linear import paths, 12MP float HDR, viewer TTL/prefetch, large upload cancellation, compute RAM/swap/Metal and two model swap cycles. Mandatory thumbnail completion and supported texture extent checks passed.

Evidence: `research/metal-integrated-exr-neutral-2026-10-11.json`. Ignored hardware/corpus/manual tests remain explicitly skipped; this does not prove the complete 100-format goal or physical HDR calibration. CUDA host checks do not qualify NVIDIA hardware, which remains deferred.


### 2026-10-11: sustained full-frame Metal HDR and storage revalidation

Eight 4000x3000 float sources (192 MB each) completed a 60-second actual Metal collection loop. All 83 full SDR frames and 83 full RGBA16Float HDR frames matched references, including 83 mid-batch cancellations. Final tracked root and queue credit are zero. The cache sustained suite passed all eight tests with no ignores, including 100000 distinct raster keys, three independent 30-second delayed-I/O modes, byte/count eviction, physical TTL deletion, overload and cancellation/retry.

Evidence: `research/metal-sustained-full-hdr-current-2026-10-11.json` and preserved hashed logs. Managed pressure and delayed I/O do not qualify OS-wide pressure or a physical slow disk. Physical HDR calibration and NVIDIA hardware remain open.


### 2026-10-11: mandatory Metal linear color evidence

The integrated HDR suite now captures output and requires four named successful color tests plus their workload markers and actual Metal adapter evidence. Missing any of four markers, marking any required test ignored, or replacing Metal with Vulkan is rejected. Current actual HDR/TTL run passes (24 passed, 8 ignored). Evidence: `research/metal-linear-color-required-gate-2026-10-11.json`. This hardens execution evidence; broader format and physical HDR qualification remain open.


### 2026-10-11: current RAW reversal on Metal

Actual EOS R8 worker reversal passed with 25494560 native samples exact and complete 6188x4120 Metal readback, plus 1920x1080 output. Cancelled forward work stays absent; backward work is warmed; forward/backward subsequent restores preserve all samples/frames with disk count one and final tracked root zero. The optional integrated worker scenario now requires these workload markers; deleting any required marker or marking execution ignored is rejected. Evidence: `research/raw-reversal-metal-current-2026-10-11.json`. Two pinned EOS R8 sources do not qualify every camera or the full format goal.


### 2026-10-11: full-frame multi-neighbor RAW Metal

Expanded the behind=1/ahead=2 actual worker scenario from 1920x1080-only readback to additional full sensor-sized Metal readback. Six neighbor restores across forward/backward order preserve all original samples, metadata and full frames for ORF/PEF/NEF/CR2 sources. Selected source is not speculatively decoded, disk count remains three, failures and final tracked credits are zero. The integrated optional scenario now requires six full-frame results; removing any result is rejected. Focused test passed, no ignores. Evidence: `research/raw-neighbors-full-metal-2026-10-11.json`. This qualifies worker/storage/GPU transport for these four files, not independent camera-color fidelity.


### 2026-10-11: nine original RAW worker sources on Metal

The actual worker test passed for all nine pinned manifest sources: CR2/NEF/ARW/ORF/PEF/RW2/RAF and two EOS R8 CR3 files. Every source preserves all sensor samples, metadata and full-sized Metal output through prefetch/disk restore. Alternating directions and one-entry disk eviction pass, with previous source absent and tracked root zero. All nine distinct source paths and SHA256 pins recorded in `research/nine-raw-worker-metal-2026-10-11.json`. This establishes native storage/render transport against direct native decoding, not independent camera-color fidelity.


### 2026-10-11: sustained nine-source RAW RAM/swap/Metal collection

Current real-camera collection completed 85 alternating traversal rounds, 765 disk restores over 60.458 seconds, with nine initial writes. Every restore compares all native samples/metadata and 1920x1080 Metal output, including memory refusal, cancellation/retry, visible-frame pinning and one-entry RAM/spill policy. Final tracked root/queue credit and swap errors are zero. Focused test passed without ignores. Evidence: `research/nine-raw-collection-current-2026-10-11.json`. Full-sized per-source transport was qualified separately; this sustained loop uses 1920x1080 output. Physical slow disk, independent camera color and physical HDR remain open.


### 2026-10-11: bounded RAW viewing latency measurement

The sustained nine-source collection now measures swap restore and GPU upload/render/readback separately, excluding reference comparison and refusal/cancellation controls. Bookkeeping is bounded to the last 4096 timings per stage. On Apple M4 Max, 450 restores across 50 rounds passed with zero errors: restore p50=41.346 ms, p95=70.207 ms, max=142.440 ms; Metal 1920x1080 upload/render/readback p50=24.916 ms, p95=42.225 ms, max=91.302 ms. Source values/output remain exact, tracked root/queue zero. Evidence: `research/raw-collection-latency-2026-10-11.json`. These are warm local filesystem restores and offscreen GPU readback, not first RAW decode, physical display latency or universal performance guarantees.


### 2026-10-11: initial native RAW latency baseline

Nine-source test now records initial native mosaic decode separately from GPU 1920x1080 upload/render/readback. Single-run decode ranges 105-1840 ms: ORF 1839.866 ms, CR2 1581.452 ms, PEF 1276.728 ms, NEF 662.066 ms, RW2 255.952 ms, ARW 153.359 ms, RAF 139.392 ms, CR3 105.495/111.345 ms. Regression passed including 90 swap restores and exact samples/output. Evidence: `research/raw-initial-latency-2026-10-11.json`. These are per-file baselines with no forced cold filesystem state, not format-wide guarantees; expensive native decode paths are candidates for profiling.


### 2026-10-11: native Olympus entropy extraction optimization

Replaced per-bit residual reads with bounded covering-byte extraction, retaining checked truncation and exact bit order. Differential tests compare all alignments and widths 0..16 against the original bitwise definition, including truncated tails and unchanged cursor on refusal. Three entropy tests and twelve ORF tests pass. Two actual nine-source Metal regressions (90 restores each) pass. The pinned 1084.orf decode measured 659.654/630.439 ms after, versus prior single-run baseline 1839.866 ms. Evidence: `research/orf-native-bit-window-2026-10-11.json`. This is a per-file performance improvement with non-interleaved baseline, not a universal format speed guarantee or GPU entropy decoder.


### 2026-10-11: ORF performance claim correction

Supersedes the earlier 2.8x ORF speedup claim: unrelated CR2 decode also improved substantially without CR2 code changes, so non-interleaved complete-decoder timings are confounded. Production ORF bit reader is restored to its original checked algorithm; byte-window candidate remains test-only. A corrected same-process checked-reader microbenchmark alternates order for six rounds with identical checksums (reference median 22.030 ms, candidate median 20.270 ms), a modest synthetic result that does not establish complete original-file decode improvement. Four entropy tests pass; restored production nine-source Metal/90-restore regression passes. Evidence: `research/orf-performance-correction-2026-10-11.json`. Initial unchecked-reference microbenchmark is superseded. Full decoder paired A/B profiling remains necessary before promoting the candidate.


### 2026-10-11: paired complete original ORF entropy reconstruction

Six alternating same-process rounds reconstruct the entire pinned Olympus entropy strip with both readers and compare every one of 16110080 sensor values after each round. All values match. Median original reader 530.482 ms; byte-window candidate 567.190 ms. Candidate is rejected for production; original reader retained. Evidence: `research/orf-paired-real-2026-10-11.json`. Unlike the synthetic microbenchmark, this tests complete real entropy reconstruction; TIFF/container parsing, allocation and GPU are outside timing.


### 2026-10-11: validated inline short-window ORF production path

A revised byte-window candidate adds same-byte extraction and inline call integration. Six alternating same-process complete original entropy reconstructions improve in every pair; median original 517.072 ms, candidate 420.772 ms (18.6% lower time). All 16110080 values match after each round. This revised candidate is enabled in production; checked original reader remains available to test specialization. Three entropy tests, twelve ORF format tests and actual nine-source Metal/90-restore regression pass. Evidence: `research/orf-short-inline-production-2026-10-11.json`. This is one-file entropy reconstruction, not an end-to-end or format-wide guarantee; the earlier 2.8x claim remains withdrawn.


### 2026-10-11: full native decoder regression after ORF optimization

Full `rrrah-decode` ordinary test command passes: 1001 passed, zero failed, 77 ignored. Preserved report enumerates all ignored tests instead of treating unavailable external corpora/manual benchmarks as coverage. The two ignored Olympus tests are manual benchmarks, exercised separately in preceding qualification; no additional independent original ORF oracle test is present in this suite. Evidence: `research/decode-full-after-orf-2026-10-11.json`. This verifies regression coverage, not complete 100-format or camera-color qualification.


### 2026-10-11: independent original ORF sensor validation

The optimized public NativeRawDecoder with 128 MiB managed budget matches independent LibRaw 0.22.2 unprocessed_raw for all 16110080 samples of pinned original 1084.orf (4640x3472), zero mismatches. Oracle output is unprocessed 16-bit PGM without gamma/autoscale. Production remains native Rust; LibRaw is test-only. Source/native/oracle SHA256 pins are preserved in `research/orf-independent-sensor-2026-10-11.json`. This qualifies that original sensor plane, not WB/color/HDR or all Olympus cameras.


### 2026-10-11: independent sensor planes for nine RAW originals

Public native Rust decoding under a 128 MiB managed output budget matches independent LibRaw 0.22.2 unprocessed_raw exactly for all 187104320 samples across nine pinned originals: CR2/NEF/ARW/ORF/PEF/RW2/RAF and two CR3. Zero mismatches; sample counts and source/oracle/native SHA256 pins recorded in `research/nine-raw-independent-sensors-2026-10-11.json`. This independently validates the sensor planes subsequently exercised by worker/RAM/swap/Metal tests. LibRaw remains test-only. Nine-file sensor qualification does not establish every camera, independent WB/color/orientation or physical HDR.


### 2026-10-11: independent RAW WB and level audit

Normalized RGB camera WB matches independent LibRaw for all nine originals within 2e-6. Sensor equality alone does not prove full development: saturation differs for ORF (native4095 vs oracle4065) and RW2 (3967 vs3989); black level differences in CR2/RAF/CR3 require coordinate/channel and calibration audit. PEF cblack channel order must not be compared directly to native coordinate black grid. Evidence: `research/nine-raw-metadata-audit-2026-10-11.json`. These discrepancies remain explicit open color qualification items; no oracle values blindly copied into production.


### 2026-10-11: RAW level provenance audit

Direct original CR2 TIFF/EXIF/Canon MakerNote inspection confirms ColorData v13 (1592 words), black values at word778 =512,512,511,511, exactly matching native stored-level extraction. Independent LibRaw level state changes from all-zero black before unpack to black511+cblack[0,0,1,1] after unpack. These are different stages of processing; equality of stored data must not be conflated with post-unpack calibration. Six original level-stage observations preserved in `research/raw-level-provenance-2026-10-11.json`. Canon coordinate mapping/calibration, ORF/RW2 saturation policy and RAF black policy remain unresolved. No production values changed solely to match the oracle.


### 2026-10-11: Canon stored black channel mapping resolved for original CR2

Independent Canon parsing evidence reads four sequential black words in RGGB order (LibRaw converts via RGGB_2_RGBG to its channel array). Pinned 1294.cr2 has native raw RGGB CFA, so stored words512,512,511,511 map directly to native 2x2 coordinate grid. No channel permutation bug is demonstrated for this original. ExifTool independently identifies ColorData v13 black offset0x30a. Evidence: `research/canon-black-order-2026-10-11.json`. Post-unpack calibrated black policy and other camera/CFA phases remain open.


### 2026-10-11: conflicting independent saturation calibration

LibRaw model table uses E-M5 white4065 / GH2 white3989, while RawSpeed camera database uses4095 /3800 (E-M5 black255). Native GH2 reads explicit file RGB linearity limits3967; native E-M5 falls back to12-bit full scale4095. Thus third-party equality alone is not a valid saturation oracle. Preserved source hashes and active-crop parity minima/maxima/above-white counts: `research/raw-saturation-provenance-2026-10-11.json`. Existing file values retained; physical saturation requires controlled exposure or independent manufacturer calibration and remains unqualified.


### 2026-10-11: RAW values above file white retained on Metal

Extended scene-linear RAW attachment test with authored encoded3989/black15/white3967 field (GH2 original file levels), identity color transform and +1 stop. Independent normalized expected RGB exceeds2 and matches both Rgba16Float/Rgba32Float within separately defined attachment precision. SDR output remains bounded; switching back to HDR reproduces exact prior output; managed GPU budget releases. Actual Metal two-test suite passes. Evidence: `research/raw-above-white-metal-2026-10-11.json`. This qualifies above-white headroom arithmetic/transport, not photographic GH2 color or physical display calibration.


### 2026-10-11: invalid explicit ORF white levels refused

Fixed ORF metadata reader accepting zero/out-of-depth explicit white and silently taking first element of an invalid multi-value declaration. Explicit WhiteLevel now requires exactly one finite positive integer within the sensor bit depth. Missing declaration retains full-scale fallback. Added empty/zero/4096-for12-bit/multiple-value refusal regressions. Thirteen ORF tests and actual nine-source Metal collection with90 restores pass. Evidence: `research/orf-white-validation-2026-10-11.json`. Metadata validity is separate from physical camera saturation calibration, which remains open.


### 2026-10-11: RW2 explicit white bit-depth validation

Explicit RGB linearity limits now reject values above the sensor bit depth (new4096-for12-bit regression), in addition to existing zero/partial limits refusal. Focused RW2 suite passes. Public native original GH2 decode under128MiB remains bit-identical for all16526720 samples to prior independently qualified sensor dump, with file RGB white3967 retained. Evidence: `research/rw2-white-validation-2026-10-11.json`. Physical saturation calibration remains separate.


### 2026-10-11: current cross-format raster collection on Metal

Actual current source passes all65 raster sources,94 selections,188 native/prepared storage cases with exact pixel bits/color metadata and SDR/HDR frames, bounds refusal, memory refusal/cancellation/retry and zero tracked root/queue at completion. Independent bright/negative constant HDR control passes. Strict workload validator passes and missing completion marker is rejected; all65 source hashes pinned. Evidence: `research/raster-collection-current-2026-10-11.json`. This is transport against direct decode, not independent fidelity of every format or100-format completion.


### 2026-10-11: nine independent camera matrix declarations and Metal transforms

All nine original RAW native XYZ-to-camera matrices match independent LibRaw coefficients within6e-8 absolute error. Actual camera-color Metal suite passes12 tests without ignores, covering independent transforms, authored fields and cropped/minified/oriented/tiled paths. Evidence: `research/nine-raw-camera-matrices-metal-2026-10-11.json`. GPU cases are authored fields, not photographic per-original appearance or physical calibration. The six Pending format implementations remain Pending.


### 2026-10-11: ORF black metadata and positive normalization range

ORF scalar black parsing now rejects empty/multiple declarations and nonfinite/negative offsets instead of silently taking first value. Metadata construction requires black below white, refusing collapsed/inverted normalization ranges. Regression covers empty/multiple/equal/greater cases and valid nonzero offset. Fourteen ORF tests pass; public original sensor decode under128MiB retains all16110080 samples exactly against the previously independent-qualified dump. Evidence: `research/orf-black-validation-2026-10-11.json`. Per-coordinate black metadata and physical calibration remain separate work.


### 2026-10-11: declared ORF black repeat grid

ORF metadata now supports explicit BlackLevelRepeatDim 1x1/2x2 and retains row-major sensor-coordinate offsets. Distinct authored10/20/30/40 values roundtrip metadata exactly; empty/malformed/unsupported dimension vectors and mismatched value counts are refused. Missing black uses zero grid of declared shape; all values remain checked below white. Fifteen ORF tests pass. Evidence: `research/orf-black-grid-2026-10-11.json`. No original ORF with nonuniform explicit repeat grid or new original-file Metal qualification is claimed.


### 2026-10-11: combined current Metal after native RAW changes

Integrated driver completes 34 suites, 535 passed, zero failed, 76 ignored. Every suite execution validator and preserved log hash verified. Includes raster65/94/188 collection, nine-source RAW collection and full-frame worker, multi-neighbor full-frame restore, direction reversal, linear imports, TTL/preload,12MP HDR, upload cancellation, compute cache, model swap and AI/thumbnail/texture-extent checks. Evidence: `research/metal-integrated-raw-levels-2026-10-11.json`. CUDA checks are host-only; NVIDIA remains deferred. Ignored manual/external cases, six pending formats, complete fidelity and physical HDR calibration remain open.


### 2026-10-11: invalid RAW range preserves existing Metal frame

Extended actual Metal scene-linear test with16 malformed uploads: equal/inverted black-white range independently at each CFA phase for Rgba16Float/Rgba32Float. Every upload refuses with InvalidLevels, previous valid HDR readback remains exact, managed credit stays unchanged, final renderer drop releases admission. Two-test suite passes. Evidence: `research/invalid-raw-levels-metal-2026-10-11.json`. This validates failure atomicity of existing upload guards, not physical camera/display calibration.

### Exact black-grid admission (2026-10-11)

Removed silent top-left 2x2 approximation for unusual black-level repeat grids. Exact larger periodic grids remain accepted; nonrepresentable grids are refused, including odd-size wrap cases. Core: 115 passed, 2 ignored. Real Metal RAW linear readback: 2 passed. Arbitrary repeat-grid shader support remains open. Evidence: `research/black-grid-metal-2026-10-11.json`.

### Full Bayer viewport black repeat grid (2026-10-11)

The Bayer Metal viewport now consumes the full single-component black-level repeat grid in a read-only storage buffer. This supersedes the viewport refusal recorded above; `bayer_quad` remains strict for consumers requiring a quad. Exactly quad-representable grids reuse the uniform allocation. Grid bytes join the atlas reservation and resource lease; failed admission preserves the prior frame. Independent authored 3x5 neutral-field check passes on Rgba16Float and Rgba32Float, including memory pressure, invalid final cell, and return to fast-path credit release. GPU suites: 18 passed; library: 52 passed. RGBE calibration and compute arbitrary-grid qualification remain open. Evidence: `research/repeat-grid-metal-2026-10-11.json`.

### Fractional RAW level normalization (2026-10-11)

Metal Bayer viewport no longer rounds a positive white-minus-black range up to one. Independent encoded=1, black=.75, white=1.25, exposure=1 fixture yields RGB=1 on both float attachments. Non-finite subtraction is refused before replacing the visible frame or its reservation. RAW linear and camera-color suites passed. Evidence: `research/fractional-levels-metal-2026-10-11.json`; physical calibration remains unqualified.

### RGBE full-grid calibration (2026-10-11)

RGBE calibration now resolves per-site black levels over the full repeat grid and validates the joint grid/CFA period. Quad-equivalent grids retain allocation-free calibration; unusual grids own bounded metadata copies. A 3x5 authored grid passes 144-site four-gain normalization and Metal signed HDR viewport checks on both float targets. Core RGBE: 13 passed; GPU RAW/RGBE suites: 7 passed, 1 ignored. Dedicated arbitrary-grid normalization-to-compute integration and genuine camera appearance remain unqualified. Evidence: `research/rgbe-repeat-grid-2026-10-11.json`.

### Full-grid RGBE normalization-to-compute integration (2026-10-11)

Managed calibration now reserves the unusual-grid payload and shares its storage/reservation across calibration clones. Metal test normalizes a 3x5 grid over a 17x33 sensor, checks independent four-plane values at all 561 sites and across multiple compute dispatches, and verifies final CPU/GPU credit zero. Core: 13 passed; GPU compute/linear: 6 passed, 1 ignored. Very small binding limits that cannot fit an aligned row chunk remain explicitly refused (17-wide/1024-byte case); physical HDR and original unusual-grid camera appearance are not qualified. Evidence: `research/rgbe-grid-compute-2026-10-11.json`.

### Integrated refresh after RAW grid changes (2026-10-11)

34 integrated suites: 535 passed, 0 failed, 76 ignored; all execution validators and 34 log hashes checked. Separate expanded GPU refresh includes new RAW/RGBE grid and fractional-level tests with actual Metal markers. These tests are now in the default qualification driver GPU suite. Evidence: `research/metal-integrated-repeat-grid-2026-10-11.json`. This regression does not close original format fidelity, physical HDR/NVIDIA, genuine slow-disk or OS-pressure requirements.

### 100k distinct-key storage refresh (2026-10-11)

100000 distinct tiny raster writes and exact restores passed in 26.045 seconds, including 1000 full-root pressure refusals/retries. Logical disk quota and physical files stayed within 17 objects/4096 bytes; evicted keys absent, final root/queue credit zero. Small payloads isolate index/key churn; this is not large-image throughput or OS-pressure qualification. Evidence: `research/100k-swap-current-2026-10-11.json`.

### CPU/Metal level-range refusal consistency (2026-10-11)

CPU Bayer development now rejects an overflowing white-minus-black subtraction before normalization, matching Metal viewport range validation. Authored regression verifies typed refusal and complete managed scratch/output rollback. Core: 118 passed, 2 ignored. Evidence: `research/cpu-level-overflow-2026-10-11.json`.

### Ten-minute sustained storage pressure (2026-10-11)

Three delayed-local-I/O scenarios ran concurrently for 600 seconds each: independent byte quota, independent count quota, combined limits. All passed; 13388 writes and exact restores, each with full managed-root refusal/retry. Physical and logical quotas checked every cycle; final root/queue credit zero and swap errors zero. Sampled process RSS max 5824 KiB over 16 samples (not an exact peak). Evidence: `research/ten-minute-swap-2026-10-11.json`. This qualifies simulated slow I/O and managed pressure, not a physical slow disk, OS-wide pressure, or simultaneous ten-minute GPU viewing.

### Ten-minute real RAW Metal collection (2026-10-11)

Nine original files completed 957 alternating traversals in 600.074 seconds: 8613 full-sensor restores and exact 1920x1080 offscreen Metal frames. Per-cycle managed pressure refusal, cancellation, retry, visible pin, spill, root/queue-zero, and zero swap-error assertions passed. Source hashes rechecked after run. Last4096 latency: restore p50=41.462/p95=71.050 ms; GPU upload/render/readback p50=25.043/p95=50.197 ms. Warm local filesystem, fresh renderer per frame, SDR offscreen target; not physical HDR, persistent-renderer soak, OS-wide pressure, or independent appearance qualification. Evidence: `research/ten-minute-raw-metal-2026-10-11.json`.

### Persistent RAW renderer accounting regression (2026-10-11)

Collection test now reuses one RAW renderer and applies separate resident/upload/queued GPU payload budgets. Thirty-second nine-original regression: 495 exact restores/frames; resident peak 146800640 bytes, packing peak 2097152, queued peak 73400320. Active credit equals current atlas after each readback; upload/queue zero each frame, all GPU roots zero after drop. Driver requires final residency evidence and rejects missing/nonzero-final markers. A new 600-second persistent run is in progress; no long-run result claimed here. Evidence: `research/persistent-raw-metal-2026-10-11.json`.

### Ten-minute persistent RAW renderer completion (2026-10-11)

Nine-original collection completed 926 alternating traversals and 8334 exact sensor restores/Metal frames in 600.006 seconds using a single persistent renderer. Resident/packing/queued GPU payload peaks: 140/2/70 MiB. Active credit equals current atlas after readback, temporary credit zero each frame, all GPU/storage roots zero at final drop. Driver workload validator and original input hashes reverified. Last4096 restore p50/p95: 41.830/68.318 ms; GPU upload/render/readback: 22.928/41.585 ms. Warm local SDR offscreen transport; not physical HDR, OS pressure, driver-allocation accounting, or independent appearance qualification. Evidence: `research/ten-minute-persistent-raw-metal-2026-10-11.json`.

### Real RAW GPU replacement pressure atomicity (2026-10-11)

Nine-original persistent-renderer regression completed 441 exact replacements over 30.188 seconds, injecting GPU resident-budget exhaustion before all 440 replacements with an existing frame. Every refusal retained previous frame pixels and resident credit; retries exact, final GPU/storage roots zero. Driver now requires refusal count=restores-1 and preservation/retry marker. Reported 512 MiB budget peak includes artificial pressure reservation, not atlas footprint. Evidence: `research/real-raw-gpu-pressure-2026-10-11.json`. Physical VRAM exhaustion and HDR remain separate unqualified requirements.

### Partial real RAW GPU upload cancellation (2026-10-11)

Nine-original reuse regression completed 306 exact restores/new frames over 30.482 seconds, with 305 GPU pressure refusals and 305 partial upload cancellations. Forced 512-sample tiles, halo2, check64 crosses tile writes before cancellation in later packing under the current checked loop. Previous frame and resident atlas preserved; scratch credit zero on cancellation, cancelled atlas/queue released after GPU completion, retries exact, all final roots zero. Driver validates exact cancellation counts and release/retry evidence. Progress stage inferred from source loop rather than hardware tracing; submitted work completes normally. Evidence: `research/real-raw-partial-cancel-2026-10-11.json`.

### Native RAW live HDR probe extension (2026-10-11)

Live HDR example now accepts `RRRAH_HDR_RAW_SOURCE`, decodes via native managed RAW path, uploads to scene-linear RawRenderer, and verifies source-credit release. Redraw retries now honor their deadline instead of spinning on about_to_wait. Build passed. Current original 6188x4120 CR3 setup/configuration succeeded without validation errors, but live surface returned Occluded after retries; zero presented frames. The raster control also returned Occluded. No current live RAW HDR presentation success or physical calibration claimed. Evidence: `research/live-raw-hdr-occluded-2026-10-11.json`.

### Live HDR occlusion cause verified (2026-10-11)

Read-only macOS Root registry reports `CGSSessionScreenIsLocked=true`, explaining the current live-probe Occluded result. Local winit documentation confirms activation already enabled by default; no activation workaround added. Live presentation requires an unlocked interactive desktop. Offscreen qualification remains independently actionable; successful presentation would still not prove physical calibration. Evidence: `research/live-hdr-screen-lock-2026-10-11.json`.

### Real RAW HDR float swap/reuse (2026-10-11)

Nine-original collection now verifies both SDR and scene-linear RGBA16Float 1920x1080 frames with persistent renderers sharing GPU budgets. Thirty-second run: 216 sensor restores, 216 exact SDR and 216 bit-exact HDR frames; original HDR references contain above-one and negative components and are all finite. Shared resident credit equals both active atlases, all final GPU/storage roots zero. Existing 215 pressure/cancel failures target SDR renderer. Driver requires HDR source/frame evidence. GPU timing now includes both SDR and HDR, so it is not comparable to prior SDR-only measurements. Offscreen transport is not physical HDR/appearance qualification. Evidence: `research/real-raw-hdr-swap-2026-10-11.json`.

### HDR-specific RAW replacement failure atomicity (2026-10-11)

Nine-original combined collection completed 135 exact SDR/HDR restore frames over 31.782 seconds. HDR renderer now independently undergoes 134 full logical GPU-budget refusals and 134 partial upload cancellations, with previous binary16 frame read back after every failure. Previous atlas credit retained, cancelled temporary credit released on completion, retries exact, all final roots zero. Driver validates HDR failure counts and preservation evidence. This closes the HDR-specific failure readback gap recorded above; physical VRAM pressure, live display calibration and independent appearance remain unqualified. Evidence: `research/real-raw-hdr-failures-2026-10-11.json`.

### Full ordinary GPU regression after HDR failure tests (2026-10-11)

All ordinary rrrah-gpu suites passed: 117 passed, 0 failed, 13 ignored. Actual Metal adapter markers and log hash verified. This refresh covers shared helper changes used by real HDR replacement/failure tests; manual ignored external-camera tests are not implicitly qualified. Live HDR still awaits an unlocked desktop, confirmed through read-only registry; no redundant probe restart. Evidence: `research/gpu-hdr-failures-refresh-2026-10-11.json`.

### Normalized sensor overflow refusal (2026-10-11)

CPU Bayer development now rejects a non-finite normalization result before clipping recovery or stage2 correction. Smallest-positive-f32 range and finite-gain multiplication overflow regressions verify typed refusals, complete reservation rollback and successful same-budget valid retry. Core: 119 passed, 2 ignored. This does not qualify extreme-range Bayer GPU shader input arithmetic; that remains separate. Evidence: `research/normalized-overflow-2026-10-11.json`.

### GPU extreme sensor correction admission (2026-10-11)

RAW upload certifies normal u16 correction bounds from metadata, scanning actual samples only for potentially non-finite gain/normalization or subnormal division. Unsafe nonzero sites refused before replacement; exact zero sites remain accepted via shader zero-numerator branch. Actual Metal 16/32Float regressions preserve prior frame/credit on both normalization and gain overflow, with exact valid retry. Focused: 14 passed; full ordinary GPU: 117 passed, 13 ignored. Later matrix/interpolation/exposure and half-float range remain separate numerical qualification concerns. Evidence: `research/gpu-extreme-correction-2026-10-11.json`.

### RAW GPU decoded-buffer length admission (2026-10-11)

Upload now checks width*height sample count before calibration or allocation. Empty, truncated and oversized buffers produce typed InvalidPixelCount errors while preserving previous HDR frame/credit on both float targets. Additional extreme-range scan cancellation regression confirms first-row cancellation leaves old zero frame and credit exact. Focused Metal suite: 2 passed. Evidence: `research/gpu-invalid-pixels-2026-10-11.json`.

### RGBE aligned output-window compute (2026-10-11)

Compute now binds aligned windows including any prefix before the first output row, with an explicit global output index in a budgeted 48-byte uniform. The previously refused 17x13/1024-byte case executes five dispatches; 17x33/8192-byte managed integration remains covered. Nonconstant CPU reference and tail sentinel pass, as do independent four-plane odd-grid values and cancellation/credit ownership checks. Library: 52 passed; actual Metal compute: 4 passed, 1 ignored. Input must still fit one binding and each output row must fit a window; no column segmentation claimed. Evidence: `research/rgbe-aligned-windows-2026-10-11.json`.

### Full original F828 compute refresh after window alignment change (2026-10-11)

Pinned independent 3360x2460 F828 sensor dump passed actual Metal managed compute: all 33062400 channel values agree with CPU lattice reference within 1e-5, CPU/GPU final credit zero; updated 48-byte uniform budget peak asserted. Full ordinary GPU regression: 117 passed, 13 ignored; the full F828 case was then explicitly run and passed separately. Both execution validators and log hashes verified. Authored normalization/gains test transport and arithmetic, not missing as-shot Emerald WB or photographic appearance; LibRaw oracle remains test-only. Evidence: `research/rgbe-window-full-sensor-refresh-2026-10-11.json`.

### Sustained current RAW SDR/HDR replacement failures on Metal (2026-10-11)

Current combined scenario passed 120.288 seconds, 49 forward/backward collection rounds, 441 exact full-sensor restores, and 441 exact SDR plus finite binary16 HDR frames across nine original RAW files. Each renderer passed 440 GPU-budget refusals and 440 partial-upload cancellations while preserving its prior frame and reservation; retries were exact. Final CPU, GPU resident, packing, and queue reservations were zero. All 117 ordinary GPU tests also passed (13 ignored). The qualification driver verified all required workload markers and log hashes.

Restore latency p50/p95/max: 43.459/110.205/405.155 ms. Combined SDR+HDR upload/render/readback: 84.529/190.688/411.452 ms; these combined measurements cannot be compared directly with earlier SDR-only runs. Logical GPU resident peak reached the injected 512 MiB budget; packing peak 2 MiB, queue peak 77,276,160 bytes. This is managed budget pressure and local swap, not physical slow-disk or OS-memory-pressure qualification. Live HDR presentation/calibration remains separate. Evidence: `research/raw-hdr-pressure-120s-2026-10-11.json` and preserved log.

### Explicit Metal HDR extreme and 36 MP LOD checks (2026-10-11)

All three normally ignored `raster_readback hdr_` tests were explicitly executed with Metal required and passed. Authored exposure extremes produced expected finite binary16 outputs; opposite finite f32 extremes interpolated to the expected center without Inf/NaN anywhere in the HDR target. The 36 MP area-LOD scenario checked all 1,000,000 output pixels exactly, SDR/HDR Metal frames, and output root release. These are offscreen empirical Metal results; extreme conversion remains outside the portable binary16 caller contract and the tests remain explicitly opt-in. No NVIDIA, physical HDR, or original-camera appearance claim follows. Evidence: `research/metal-hdr-extremes-2026-10-11.json` and hashed log.

### Large raster independent GPU admission failures (2026-10-11)

Strengthened the shared raster cancellation helper with separate texture-budget and upload-queue-budget refusals. Both must return a typed memory error, retain the prior full frame and its reservations, and release all temporary reservations. Existing cancellation checkpoints and successful replacement/retry remain checked, including final zero credits. The explicit 4000x3000 (12 MP, 192 MB float payload) case passed on Metal Apple M4 Max, with six validation/admission cancellation checkpoints. The ordinary raster suite passed 16 tests (8 ignored). This helper does not test cancellation after partial GPU writes; that is a separate tiled/RAW scenario. Evidence: `research/metal-large-admission-2026-10-11.json` and hashed logs.

### Compute-to-RAM-to-swap HDR frame preservation (2026-10-11)

Extended compute-cache integration tests to compare finite signed binary16 HDR frames after compute, while RAM-pinned despite a zero cache limit, and after disk restore. Each oracle includes negative and above-one RGB values. Both explicit manual tests passed on Metal Apple M4 Max: 164x50 and full 4000x3000 (12 MP) render/readback. Existing all-component float-bit and SDR checks also passed; one swap write/read per case, final CPU/GPU/queue credits zero. This validates authored exposure transport and retention, not original-camera appearance or physical HDR calibration. Evidence: `research/metal-compute-cache-hdr-2026-10-11.json` and hashed log.

### Compute/cache HDR qualification gate (2026-10-11)

The integrated driver now requires both explicit compute/cache cases, exact SDR extents, finite signed HDR computed/pinned/restored markers, zero final CPU/GPU credit, one swap write/read per case, and actual Metal adapter markers. A valid current log was accepted; seven variants with missing HDR, duplicated extents, wrong full-frame extent, retained credit, wrong backend, missing execution, or ignored execution were rejected. The updated driver completed 117 ordinary GPU tests (13 ignored) and both explicit compute/cache tests with workload evidence verified. Logs and hashes preserved in `research/compute-cache-hdr-driver-2026-10-11.json`. Earlier SDR-only reports do not satisfy this new HDR gate.

### Viewer TTL/prefetch binary16 HDR frame checks (2026-10-11)

Extended the existing production viewer EXR/Radiance TTL test with finite, above-one binary16 HDR frame oracles, in addition to exact float payload and SDR checks. All 128 TTL restores and two actual preloads from swap while the decoder gate is blocked must reproduce the HDR frame exactly. The real Metal test passed and all final tracked CPU credits were zero. The integrated driver now requires the new 130-frame HDR marker, the no-decoder-fallback/blocked-preload marker, actual Metal, and executed tests. Updated integrated run passed 117 ordinary GPU tests (13 ignored) and the explicit TTL/prefetch scenario. Zero TTL tests expiry paths, not elapsed nonzero-TTL scheduling; small authored files do not qualify large originals or physical HDR. Evidence: `research/viewer-ttl-hdr-binary16-2026-10-11.json` and hashed logs.

### RGBE input storage binding windows (2026-10-11)

Removed the requirement that a complete normalized RGBE sensor fit one storage binding. Each dispatch now binds an aligned input window containing two neighboring sensor rows on either side for lattice interpolation; the shader translates global sample indices to the window. Output windows remain independently aligned. Checked planning limits rows to both windows, and full buffers must still fit max_buffer_size and managed budgets. This is binding segmentation, not streaming allocations or column segmentation.

A 17x33 sensor now passes with a 1024-byte binding cap despite its 2244-byte input: nonconstant four-plane CPU/GPU comparisons cover all pixels, odd boundaries, three CFA permutations and output-tail preservation; managed repeating-black-grid compute also passes this cap and releases credits. Focused validation: 53 library and four GPU tests passed, one full-sensor test ignored there. The integrated run separately executed the original full F828 compute successfully and all 118 ordinary GPU tests passed (13 ignored). Full-sensor authored normalization/gains do not qualify as-shot camera appearance. Evidence: `research/rgbe-input-windows-2026-10-11.json` and hashed logs.

### Full F828 with forced segmented input binding (2026-10-11)

Strengthened the explicit full-sensor test to run both default 128 MiB bindings (one dispatch) and required 1 MiB bindings (130 dispatches). The latter input is over 31 MiB and cannot fit one binding. Actual Metal Apple M4 Max passed every one of 33,062,400 channel comparisons per case against the CPU reference within 1e-5, including all input/output window boundaries. GPU peak equals each plan's explicit buffers and 48-byte uniforms; output drop releases CPU credit and final CPU/GPU credits are zero. Independent F828 sensor values use authored normalization/gains, not as-shot appearance. Buffers remain fully allocated. Evidence: `research/full-f828-input-windows-2026-10-11.json` and hashed log.

### Every segmented RGBE cancellation checkpoint (2026-10-11)

Added actual-GPU regression for a nonconstant signed 17x33 input (2244 bytes) under 1024-byte bindings and exact CPU/GPU budget limits. The successful baseline matches independent CPU lattice interpolation for every channel. All 18 cancellation checks, including validation, admission, each dispatch encoding, pre-submit and post-readback, return Cancelled and release reservations; every immediately following retry exactly matches the baseline. Five RGBE GPU tests passed, one explicit full-sensor test ignored in this focused run. The GPU submission is still one command buffer: dispatch-encoding cancellation does not claim interruption of already submitted partial work. Evidence: `research/rgbe-segment-cancel-2026-10-11.json` and hashed log.

### Current full-resolution linear exposure compute refresh (2026-10-11)

Both explicit 6188x4120 compute tests passed on Metal Apple M4 Max. Managed and interleaved paths checked every one of 25,494,560 pixels exactly, preserving negative/HDR channels and alpha. End-to-end times were 169.132 ms managed and 215.877 ms interleaved. Tracked peaks: GPU buffers 1,223,738,992 bytes and CPU buffers 815,825,920 bytes, final usage zero. The separate low-level warmed encode/submit/wait sample p50 was 2.609 ms (max 2.712 ms); it excludes upload/readback and checks only three samples, so it is not full viewer latency. Authored pattern, explicit buffers only, no original-camera or physical HDR claim. Evidence: `research/full-compute-current-2026-10-11.json` and hashed log.

### RGBE window geometry sweep (2026-10-11)

Added independent lattice-neighbor containment checks across 16,384 dimension/binding combinations (width/height 2..65; caps 256/512/1024/4096). All 7,557 admitted plans contain every needed input lattice row, have aligned in-bounds input/output bindings, cover every output row contiguously, and account exactly for full buffers plus per-dispatch uniforms. Four planner tests passed. Refused layouts remain unsupported by row-only segmentation; the sweep does not classify them as successful or prove GPU execution. Read-only macOS registry recheck still reports screen locked, so live RAW HDR remains pending. Evidence: `research/rgbe-window-geometry-2026-10-11.json` and hashed log.

### 3D model cancellation after geometry admission (2026-10-11)

Strengthened actual-Metal model swap cycles to cancel after a nonzero geometry reservation is observed, in addition to immediate cancellation. Every such cancellation must release geometry credit, leave cache errors zero, and retain the disk entry for exact retry. Seven model tests passed, including six STL/OBJ/OFF/PLY fixtures across 20 repeated cycles (120 restores) and the ordinary one-cycle case. Geometry/metadata payloads and SDR plus binary16 HDR frames remain exact; final CPU/queue credits zero. These are small subset fixtures, not full 3D-format or large-collection qualification. Evidence: `research/model-admitted-cancel-2026-10-11.json`, test-source hash, and preserved log.

### Cancellable model upload API (2026-10-11)

Added `ModelRenderer::upload_with_cancel` with typed Cancelled and checkpoints before work, every 1024 triangles during both validation and mapped-buffer filling, after admission, and before committing replacement. The existing upload API delegates with cancellation disabled. Temporary buffer/reservation ownership rolls back on cancellation, preserving existing resident geometry. Seven actual Metal model tests passed; new regression exercises all five checkpoints for a triangle, exact retained resident credit/bytes, successful retries and final zero credit. Application check passed. This new regression does not read back the previous frame, and viewer event-loop scheduling is unchanged. Evidence: `research/model-upload-cancel-2026-10-11.json` and hashed logs.

### Model upload failure frame preservation on SDR/HDR (2026-10-11)

Closed the frame-readback gap in the new cancellable model upload regression. On actual Metal, both Rgba8UnormSrgb and Rgba16Float attachments retain every 63x65 frame sample after each of five cancellation checkpoints and an independent GPU-budget refusal. Resident credit stays unchanged; a successful replacement with different geometry changes the image, proving the retry is not a no-op. Renderer drop releases all geometry/depth credit. Eight model GPU tests passed. Small authored single-triangle geometry and logical budget pressure, not large-model or physical HDR qualification. Evidence: `research/model-cancel-frame-2026-10-11.json` and hashed log.

### Multi-chunk model upload cancellation (2026-10-11)

Expanded SDR/HDR replacement regression to 2049 triangles, crossing three 1024-triangle polling blocks in both validation and mapped-buffer filling. All nine cancellation checkpoints per attachment retain the exact previous frame and reservation, including cancellation after partial vertex-buffer filling. Independent budget refusal, changed-image retry, and final zero credit also pass. Eight actual Metal model tests passed. Viewer source audit confirms generation changes are handled by the same synchronous event loop; merely wiring a generation callback would not provide responsive UI cancellation, so no such behavior is claimed. Evidence: `research/model-multichunk-cancel-2026-10-11.json` and hashed log.

### Worker-safe prepared model geometry (2026-10-11)

Split model GPU preparation from visible installation. `PreparedModelUpload` owns the GPU vertex buffer and reservation; static cancellable preparation can run on a worker, while installation moves the accepted result into a renderer. Existing upload wrappers preserve behavior. Actual Metal regression prepares 2049 triangles in a std::thread, checks exact previous SDR/HDR frames while prepared geometry is retained, drops the obsolete result and verifies released temporary credit, then installs accepted geometry and verifies changed pixels. All eight model tests pass, including cancellation and pressure. Viewer worker/event integration remains pending; callers must use the same device and intended shared budget. Evidence: `research/model-prepared-2026-10-11.json` and hashed log.

### Prepared geometry budget admission guard (2026-10-11)

Closed a limit-bypass possibility in the prepared model API: a budgeted renderer now accepts nonempty geometry only when its preparation budget is the renderer budget or a descendant. Unbudgeted and independent-root preparations return typed BudgetMismatch before visible replacement; consumed rejected geometry drops and releases its reservation. Actual Metal tests verify both refusal modes preserve exact previous SDR/HDR frames and release unrelated-root credit, while matching-budget worker preparation/install still succeeds. Eight model tests and application check passed. Same-device use remains a caller contract; viewer worker integration remains pending. Evidence: `research/model-prepared-budget-2026-10-11.json`, source hash and preserved log.

### Renderer-bound model worker preparation (2026-10-11)

Added cloneable `ModelUploadPreparer`, carrying the originating device, shared budget and private Arc owner identity into a worker. Prepared uploads require matching owner identity and compatible budget before installation. Unbound low-level results cannot be installed. A direct wgpu Device-equality experiment failed because independent Instances yielded equal DeviceId(0,1); that equality guard was removed in favor of owner identity. Actual Metal regression now prepares geometry on a second independent Instance/device with the same budget and proves typed RendererMismatch, temporary credit release and exact retained SDR/HDR pixels. Matching worker context still installs successfully. Eight model tests and application check passed. Viewer worker integration remains pending. Evidence: `research/model-preparer-owner-2026-10-11.json`, source hash and preserved log.

### Bounded application model upload worker (2026-10-11)

Added model upload worker module with one active job, one replaceable pending source, and one completed prepared geometry. Revision plus decode-generation cancellation rejects stale work; replacement drops previous pending/completed owners, publication releases source geometry before waking UI, and shutdown cancels/joins the worker. Actual Metal test submitted 1000 replacements, accepted only the latest job ID/generation, installed it, then cancelled a subsequent generation and proved only installed credit remained; final renderer drop credit zero. Application check and explicit worker test passed. Module is not yet wired to ModelReady dispatch, so interactive behavior is not claimed. Evidence: `research/model-upload-worker-2026-10-11.json` and hashed log.

### Model worker integrated into viewer events (2026-10-11)

Production ModelReady handling now queues geometry validation/normalization and mapped-buffer filling on the bounded model worker, retaining the original source event/consumer lease until completion. The worker wakes the application event loop. UI accepts only matching job ID and current loader generation, then resizes depth and installs the prepared geometry; generation changes clear pending work/results and cancel active preparation. The accepted UI branch no longer traverses triangles. Completion handling avoids consuming simultaneous raster-LOD output over an accepted model result.

Application check passed; 22 ordinary model cache/swap/prefetch tests passed, one manual worker test ignored there and then explicitly executed successfully on Metal with the 1000-replacement burst. Locked screen prevents live interactive navigation/frame-time qualification. Initial pipeline creation and depth resize remain on UI thread. Evidence: `research/model-worker-integrated-2026-10-11.json` and hashed logs.

### Completed model GPU result cancellation races (2026-10-11)

Extended actual Metal worker regression to wait deterministically for a completed result retained in its slot while the original geometry is installed. GPU credit is exactly 72 bytes (36 installed + 36 prepared). Changing the loader generation before consumption rejects the stale result and releases credit to 36; explicit cancel of a second already-completed result does likewise. The 1000-replacement burst and active-generation cancellation still pass; final worker/renderer drop credit zero. This uses observed completed state rather than sleeps to assume a race. Live-window responsiveness remains pending. Evidence: `research/model-completed-races-2026-10-11.json` and hashed log.

### Integrated model GPU worker gate (2026-10-11)

Added `--metal-model-upload-worker` to the qualification driver. It explicitly runs the manual bounded worker test and requires both the 1000-replacement/latest-only/final-zero marker and the completed-result generation-change/explicit-cancel marker, with exactly one executed test and none ignored. The integrated run passed 122 ordinary GPU tests plus the explicit worker test; every suite execution and log hash verified. This includes current RGBE window and model upload API changes in the ordinary GPU run. Live-window responsiveness and physical HDR remain separate pending checks. Evidence: `research/model-worker-gate-2026-10-11.json` and hashed logs.

### Model worker managed source and cache-lease lifecycle (2026-10-11)

Worker qualification now decodes its STL source under a CPU MemoryBudget and holds an actual LeaseCache consumer lease while preparing GPU geometry. Source usage equals model capacity; zero-cap eviction is refused while the lease is held, including after worker completion. Dropping the consumer lease permits eviction. The last source owner moves into a cancelled job; worker shutdown releases all CPU source credit, while installed GPU credit remains until renderer drop. Actual Metal test passed with prior 1000-replacement and completed-result race checks retained. Driver now additionally requires this ownership marker; older gate reports do not qualify it. Evidence: `research/model-worker-source-2026-10-11.json` and hashed log.

### Deterministic bounded model queue under overload (2026-10-11)

Added test-only pre-preparation rendezvous to hold the first active request while 1000 newer jobs replace the pending slot. The test observes exactly one latest pending job, no completed output and zero admitted GPU credit while blocked; releasing the first request rejects its obsolete revision and processes only the latest. The rendezvous uses a bounded timeout to avoid hanging teardown on an assertion failure. Actual Metal test passed with managed source/lease lifecycle, completed-result race checks and final zero credits retained. Driver now also requires the deterministic queue marker; older reports do not prove this strengthened test. Evidence: `research/model-worker-bounded-2026-10-11.json` and hashed log.

### Prepared model replacement versus GPU submission lifetime (2026-10-11)

Added actual-Metal regression encoding the old model before preparing/installing replacement. Its geometry/depth lease must retain old credit across replacement and renderer drop. The encoded old commands are then submitted and the lease retained until completion; no scoped GPU validation error and final credit zero. This verifies explicit budget ownership does not release old in-use resources early during async replacement. Nine model GPU tests passed. Small authored geometry, not a large-model benchmark. Evidence: `research/model-submission-lifetime-2026-10-11.json` and hashed log.

### Native 3D swap-to-worker SDR/HDR pipeline (2026-10-11)

Added actual Metal end-to-end worker test for six STL/OBJ/OFF/PLY originals, including ASCII and 64-bit/big-endian PLY variants. Each is decoded under CPU budget, spilled once, restored separately for SDR/HDR, moved into bounded worker preparation, installed and rendered. All frame samples exactly match pre-swap direct-render references; CPU source credit is released before completion consumption and GPU credit reaches zero after each renderer drop. Six writes, twelve reads, zero swap errors. This is transport/retention equivalence for subset fixtures, not independent appearance or full 3D syntax qualification.

The existing driver worker option now explicitly runs both manual worker tests and requires all six source markers plus deterministic overload, managed-source lease and completed-result cancellation markers. Integrated run passed 123 ordinary GPU tests and both worker tests; all suite evidence/log hashes verified. Evidence: `research/native-model-worker-2026-10-11.json` and hashed logs.

### Application-wide regression after model worker integration (2026-10-11)

All ordinary rrrah binary tests executed after the model event-loop changes: 208 passed, zero failed, 52 ignored. This broadens validation beyond the 22 model-focused tests to ordinary navigation, RAW/raster caching, prefetch, scalar/window and foreground-loader behavior. Manual GPU and external-fixture tests remain separate and are not counted as passing; the two current model worker Metal tests were independently executed in the native-model worker report. Live event-loop/window behavior still awaits an unlocked desktop. Evidence: `research/app-after-model-worker-2026-10-11.json` and hashed log.

### Model worker GPU pressure and retry, loading-state fix (2026-10-11)

All six native model swap-to-worker cases now force full logical GPU-budget pressure before both SDR and HDR preparation. Twelve typed memory refusals release restored CPU geometry without invalidating swap entries; after pressure release, all twelve rereads/worker preparations render exact reference frames. Six writes, 24 reads, zero swap errors, final CPU/queue/GPU zero. Both manual Metal worker tests passed. Driver now requires the pressure marker.

Fixed production error handling for failed preparation or worker startup: clear pending-first-present and reset pipeline/telemetry to idle while retaining previous visible frame. All 208 ordinary application tests passed (52 ignored); live error-state UI is not yet verified due locked desktop. Evidence: `research/model-worker-pressure-2026-10-11.json` and hashed logs.

### Million-triangle native model worker (2026-10-11)

Added manual native binary-STL workload with 1,000,000 authored repeated triangles. Source decode uses a 256 MiB managed CPU root; worker preparation uses an exact 36,000,000-byte GPU geometry cap. Accepted output owns exactly 36 MB; source CPU credit reaches zero before completion consumption, and worker/renderer drop leaves both roots zero. First focused run: decode 38.312 ms, UI-side submit including worker startup 0.096 ms, submitted-to-ready 113.610 ms. Single-run stage timings, not a live UI or million-triangle rendering benchmark.

Driver now explicitly runs all three model worker tests and requires the million-triangle completion marker alongside all queue/lease/race/swap-pressure evidence. Current integrated gate passed 123 ordinary GPU tests and three manual worker tests; hashes/workloads verified. Evidence: `research/million-model-worker-2026-10-11.json` and hashed logs (integrated run has its own timings).

### Million-triangle Metal draw/readback smoke test (2026-10-11)

Extended the million-triangle prepared-worker test through actual drawing and readback. All 16x16 SDR pixels exactly match a single-triangle reference of the repeated coplanar authored surface. Draw plus target/readback setup took 21.160 ms in this single run; geometry credit is 36 MB, depth 1024 bytes, final CPU/GPU credit zero. This is a small-target million-vertex-upload/draw smoke test, not complex-scene or full-viewer-resolution performance qualification and not million-triangle HDR qualification. Driver now requires the draw marker; previous gate reports do not cover it. Evidence: `research/million-model-render-2026-10-11.json` and hashed log.

### Million-triangle grid at full HD SDR/HDR (2026-10-11)

Added native binary STL with a 500x1000-cell grid (1,000,000 distinct coplanar triangles), avoiding the earlier repeated-overdraw smoke geometry. Each attachment independently decodes under a managed CPU root, prepares via bounded worker, installs and renders 1920x1080. All SDR and finite binary16 HDR samples exactly match an independently constructed two-triangle plane with the same bounds, including internal grid boundaries. Source CPU releases before completion consumption; explicit geometry/depth usage is exactly 36,000,000 + 8,294,400 bytes, final zero. Test target/readback and driver allocations are outside that explicit budget.

First focused run: SDR prepare 125.315 ms, render/readback 30.201 ms; HDR prepare 164.368 ms, render/readback 43.283 ms. Single-run host stage timings, not live UI FPS or complex-scene performance. Driver now requires full-HD completion and executes four worker tests. Integrated gate passed 123 ordinary GPU tests plus all four worker tests, workloads/hashes verified. Evidence: `research/million-grid-full-hd-2026-10-11.json` and hashed logs (integrated timings recorded separately).

### Million-triangle swap/restore failures through full-HD Metal (2026-10-11)

Extended the native million-triangle grid scenario to spill its managed decoded geometry once, release the source, and independently restore for SDR and HDR. Both complete serialized payloads equal the pre-spill geometry/metadata. Before each successful restore, full CPU-budget pressure must refuse admission and cancellation after a nonzero restore reservation must release partial ownership without invalidating the swap entry. Two refusals and two admitted cancellations passed, followed by exact full 1920x1080 SDR/HDR frames via bounded GPU worker. One write, two successful reads, no swap errors, final CPU/queue/GPU credit zero.

Focused Metal test passed. Driver additionally requires swap/failure markers; older integrated reports do not prove these additions. Local disk and managed pressure, authored planar geometry; oracle allocations/readback targets are outside managed source/geometry budgets. Evidence: `research/million-grid-swap-2026-10-11.json` and hashed log.

Current memory/cache/swap/core regression (2026-10-11): 335 passed, zero failed, 8 ignored across 16 result groups including doctests. Evidence: `research/storage-core-current-2026-10-11.json` and hashed log. Complements the current ten-minute original RAW SDR/HDR workload; does not qualify physical slow disk or OS pressure.

Current decoder regression (2026-10-11): 1004 passed, zero failed, 77 ignored across 7 result groups. Evidence: `research/decode-current-2026-10-11.json` and hashed log. This does not change the format acceptance ledger: 94 implemented subsets, six pending viewer rows, none fully qualified.

Current mixed raster storage/Metal regression (2026-10-11): 65 fixture sources, 94 selected images, 188 native/prepared storage cases; exact SDR/HDR frame markers, bounded selection indices, final CPU/queue zero all verified by driver. Also 125 ordinary GPU tests passed, 13 ignored. Evidence: `research/raster-collection-current-2026-10-11.json`. These are fixture sources and transport contracts, not 65 fully qualified formats or independent decoding/color proof.

Current original RAW prefetch/Metal gate (2026-10-11): 129 passes across five suites, including 125 ordinary GPU tests (13 ignored), nine-original swap/frame collection, mixed-camera worker, three-neighbor worker in both directions, and real EOS R8 navigation reversals. Required native samples/frame/storage markers verified. Evidence: `research/raw-prefetch-current-2026-10-11.json` and preserved logs. Library worker/frame coverage does not establish live UI responsiveness or independent camera appearance.

### ORF fused entropy prefix (2026-10-11)

Combined sign and two-bit remainder into one bounded three-bit read. Six alternating-order pairs on original1084.orf (4640x3472) match all16,110,080 samples exactly per pair. Existing-window median 312.897ms versus fused 282.858ms (9.60% reduction). Full decoder regression:1004 passed, zero failed,78 ignored. Evidence: `research/orf-prefix-paired-2026-10-11.json` and hashed logs/source/original. Entropy-loop timing on one original, excluding file IO/metadata/upload/render; not whole-viewer speedup or fresh independent decoding oracle.

### Current original ORF Metal opening and swap timing (2026-10-11)

After fused entropy prefix, original1084.orf4640x3472 to completed1920x1080 offscreen sRGB frame:15 measured loads after3 warmups. Native decode p50/p95 358.741/417.112ms; upload9.347/13.157; completion7.243/8.820; total374.563/434.679ms. Separate warm local-disk swap invocation:restore20.709/20.910; upload9.318/9.723; completion7.300/7.517; total37.303/37.788ms. Sixty resident zoom/pan frames p50/p95 native1.780/2.063ms and swap1.788/2.234ms. Source sample/metadata assertions passed; final managed CPU/GPU zero both runs. Evidence: `research/orf-metal-view-timing-2026-10-11.json` and hashed logs/source/original. Separate descriptive invocations; historical nine-original timing is not a paired baseline. Excludes pipeline creation, physical presentation and physical HDR; not a general format or live UI qualification.

### Nonzero background swap TTL (2026-10-11)

New ordinary regression uses2-second TTL, checks fresh explicit prune retains physical object/bytes, then observes only physical accounting until background worker removes expired file/index and releases quota. Source budget zero, no swap errors. Full cache regression:{'passed': 149, 'failed': 0, 'ignored': 5}. Evidence: `research/nonzero-swap-ttl-2026-10-11.json` and hashed focused/full logs. Small authored raster on real local filesystem; not full RAW viewer or physical slow disk.

### Independent sustained swap caps, current source (2026-10-11)

Strengthened byte-cap workload to verify capacity-evicted keys are unreadable, as already required for count cap. Two concurrent60-second tests passed:byte448 cycles/refusals;count449 cycles/refusals. Each reaches786,624bytes/3objects; byte cap786,624 with count100, count cap3 with byte8MiB, TTL3600s. Every pressure refusal followed by exact restore, physical/logical limits checked each cycle; final managed root/queue zero. Evidence: `research/independent-swap-caps-current-2026-10-11.json` and hashed logs/source. Authored256x256 rasters with injected2ms/64KiB I/O delays, not physical slow disk or GPU viewer qualification.

### Current integrated Metal gate (2026-10-11)

Full storage/viewer-selected Metal driver passed544 tests across19 suites (81 explicit ignores), including nine-original RAW collection, mixed-camera and multi-neighbor workers, actual CR3 reversal, viewer TTL/preload and bounded model upload worker. Separate full ordinary GPU gate passed130 tests (13 ignored), with all five required numeric regressions execution-verified. Both reports mark every executed suite verified; hashed logs preserved in `research/metal-integrated-current-2026-10-11/` and `research/metal-numeric-current-2026-10-11/`. These totals overlap and must not be added as unique tests. Explicit ignores, subset formats, physical HDR/live presentation and NVIDIA remain outside this evidence.

### Mandatory cache TTL execution evidence (2026-10-11)

Cache qualification now requires four exact passing test names: leased physical-file lifetime, idle background expiry, busy-barrier starvation resistance, and nonzero-TTL fresh retention/background expiry. A successful aggregate harness alone no longer verifies cache row. Validator accepts preserved current149-pass cache log and rejects13 altered cases (each of four required names missing/ignored/failed plus aggregate-only fake success). Evidence: `research/cache-ttl-required-gate-2026-10-11.json`; current preserved source log in `research/metal-integrated-current-2026-10-11/cache.log`. Validation used the existing execution log; no claim of a fresh full driver execution after this evidence-policy edit. PTX independent producer fixture remains missing despite existing PEF-compatible routing.

### Nonzero RAM TTL with live RAW leases (2026-10-11)

Added ordinary1-second TTL regression:expired RAW rejects new resident lookup while two consumer leases preserve exact sensor pixels and4-byte managed reservation. First release cannot spill; last release allows exactly one swap write, source credit zero after drain. Restored samples/metadata exact, no swap errors. Focused test and RAM adapter tests passed (28 passes). Evidence: `research/raw-lease-nonzero-ttl-2026-10-11.json` and hashed logs/source. Small authored mosaic/local filesystem; no live GPU presentation claim.

### ORF source-lease submission ordering timing (2026-10-11)

20 alternating-order measured pairs after4 warmup pairs:cached ORF upload-to-completed1920x1080 frame combined p50/p95 17.080/17.383ms; additional empty submit retaining source lease17.063/17.592ms; paired delta -0.016/0.835ms. Final managed budgets zero (example asserts before printing peaks). Both paths retain existing production queued-upload guard; this does not measure disabling that guard. No useful median advantage demonstrated; preserve lifetime ownership. Evidence: `research/orf-upload-submit-paired-2026-10-11.json` and hashed log/source. Native decode/physical presentation excluded; not a fresh per-pair independent pixel oracle. Session remains locked, physical/live HDR unverified.

### Failed-nearest RAW prefetch fallback (2026-10-11)

New explicitly executed manual real-CR3 regression verifies both directions:count1 and two-neighbor window, corrupted nearest source records one failure without consuming retained-entry slot; next valid original is stored, exactly one disk entry. All native sensor samples and metadata match direct decode after restoration; managed memory zero. Initial test incorrectly waited only for first completed attempt; corrected synchronization waits for both attempts, production worker unchanged. Evidence: `research/raw-prefetch-failed-nearest-2026-10-11.json` and preserved initial/corrected logs. Worker/local-disk coverage, not GPU presentation or independent camera appearance proof.

### Distinct-entry RAW prefetch count (2026-10-11)

RAW worker now tracks protected unique keys in a HashSet:duplicate paths/content keys record skip instead of consuming another count slot. Explicit real-CR3 test with repeated IMG_9043 and distinct IMG_9074 passes:count2/ahead3 stores two unique readable entries, expected full dimensions, no failures and final managed budget zero. Ordinary app package regression:270 passes/125 explicit ignores. Initial fixture used identical-byte copy (same content key); initial failing logs are not independent distinct-neighbor reproduction. This test checks restored dimensions, not full pixel oracle; separate tests cover pixels. Evidence: `research/raw-prefetch-distinct-count-2026-10-11.json` and preserved logs/source.

### Compute HDR restore pressure/cancellation recovery (2026-10-11)

Extended164x50 and4000x3000 Metal compute/RAM/swap pipeline:full root-budget refusal and cancellation only after nonzero restore reservation, then exact all float bits and full SDR/HDR frames on retry; credits zero after cancellation/final shutdown, one write/read, no swap errors. Both explicitly selected tests passed. Driver now requires recovery marker for both sizes; real log accepted/four altered cases rejected. Initial assertion expected Err on cancellation, corrected to documented Ok(None); production unchanged. Evidence: `research/compute-cache-recovery-2026-10-11.json` and hashed logs/source. Authored signed HDR, local disk/offscreen Metal; no physical HDR or OS pressure proof.

### Exposure compute signed f32 boundaries (2026-10-11)

New ordinary Metal test executes0/1/+32/-32 stops with admitted RGB magnitudes up to f32MAX (scaled before positive stops). Positive/negative RGB and signed zero bit-exact to CPU multiplication; negative f32MAX alpha unchanged, outputs finite, CPU/GPU zero each case. Full ordinary compute readback suite5 passed/2 explicitly ignored. Existing overflow/nonfinite rejection remains preallocation. Evidence: `research/exposure-signed-boundary-2026-10-11.json` and hashed logs/source. One-pixel buffer boundary qualification, not filtering/physical display/full-sensor qualification.

### Full sensor exposure distinct window oracle (2026-10-11)

Strengthened authored6188x4120 managed/interleaved compute field:nonperiodic green encodes position across blocks, independent blue/alpha periods; explicitly requires7 dispatches of up to4,194,240pixels with distinct expected window starts. Both variants compare all25,494,560pixels exactly, final CPU/GPU zero; peaks815,825,920/1,223,738,992bytes. Latest single-run end-to-end182.935/141.338ms, no comparative performance claim. Existing periodic1024 field could hide shifts by multiples1024; actual chunk is not a1024 multiple, so this is broader offset sensitivity, not evidence that prior exact-window boundaries were blind. Evidence: `research/full-exposure-distinct-windows-2026-10-11.json` and logs/source. No production bug, physical HDR or camera appearance claim.

### Explicit unqualified MEF color route (2026-10-11)

MEF extension now resolves to explicit unqualified-color route instead of generic DNG fallback; recipe/decode return MEF white-balance/color-support diagnostic. Generic TIFF keeps MEF choice, strong supported container magic still wins. Real pinned ZD public refusal has zero managed allocation peak and no preview; full independent native sensor comparisons remain exact. Eleven MEF tests passed with complete source/oracle environment (initial run omitted oracle variable); full decoder regression1005 passed/78 ignored. Evidence: `research/mef-explicit-color-refusal-2026-10-11.json` and hashed logs/source. Color/viewer acceptance remains pending; sensor-only import unchanged.

### RAW prefetch duplicate path avoids repeated fingerprint I/O (2026-10-11)

Added per-command path set before source fingerprinting so repeated path skips large-file hash/recipe work; separate copied paths still resolve content identity, then existing unique-key policy skips duplicates. Real four-neighbor test(count2):repeated original path, identical-content copy and second original produce two unique disk entries, four completed attempts, no failures. Both full sensor arrays and metadata exact against direct native decode; managed memory zero. Gallery ordinary regression:38 passed/14 explicit ignores. Evidence: `research/raw-prefetch-path-content-dedup-2026-10-11.json` and hashed logs/source. No comparative speedup, independent color or live GPU presentation claim.

### Post-prefetch Metal gate and robust harness output (2026-10-11)

GPU-all now captures successful output with --show-output rather than parallel --nocapture:uncaptured diagnostic stderr split the signed HDR passing harness line, causing a correct conservative evidence rejection despite zero harness failures. Strict mandatory-name regex unchanged. Fresh captured driver:139 passes/13 explicit ignores across7 suites, all execution verified:ordinaryGPU, nine-original RAW collection, mixed/multi-neighbor workers, actual RAW reversal, compute-cache SDR/HDR failure/retry, and full-sensor compute. Current numeric validator rejects16 altered/missing/ignored/failed/aggregate-only cases. Both initial rejected and final accepted reports/logs preserved in `research/metal-post-prefetch-current-2026-10-11.json` and linked directories. Explicit format/physical display/NVIDIA gaps remain.

### Model float64 extremes in HDR (2026-10-11)

Extended existing model normalization test to full64x64 binary16 HDR equality:translation(+1e12,-1e12,+1e12), scale1e300, smallest positive f64 subnormal and f64MAX signed coordinates all match baseline HDR and SDR frames exactly; baseline HDR every channel finite. All11 model readback tests passed. Evidence: `research/model-hdr-extreme-scale-2026-10-11.json` and hashed log/source. Authored triangle/offscreen targets, not full3D materials/textures or physical HDR. Production normalization unchanged.

### PNG physical pixel aspect to Metal and swap (2026-10-11)

Implemented bounded pHYs parsing before IDAT with CRC/length/duplicate/unit validation; positive densities produce pixel width/height=Y/X, zero density keeps unknown aspect. Ordinary PNG and APNG result metadata retain aspect; PNG EXIF axis-swapping rotations invert it. Decoder regression1006 passed/78 ignored including actual encoded sample preservation, ratios0.5/2, zero density, duplicate/CRC and EXIF90 cases. Explicit authored4x4 PNG Metal test:linear preparation, exact64x64 SDR/HDR against aspect0.5 reference, square-pixel control differs and analytic center/side checks, swap preserves aspect/frames, final source/queue zero, one write/read/no errors. Evidence: `research/png-physical-aspect-2026-10-11.json` and hashed logs/source/fixture. Initial GPU test incorrectly supplied encoded samples to prepared-only helper; corrected test preparation, shader unchanged. APNG propagation lacks a dedicated pHYs selected-frame case; physical display/full PNG acceptance remains open. Source: https://www.w3.org/TR/png-3/#11pHYs .

### Selected APNG physical aspect Metal gate (2026-10-11)

Authored two-frame4x4 APNG (opaque white/green SOURCE, pHYs2/1, repeat3) through common selected-frame decode:indices/count and aspect0.5 retained, both full64x64 SDR/HDR frames exactly match independent authored linear reference rasters. Two swap writes and reverse-order reads preserve metadata/frames, source/queue finalzero/errorszero. New --metal-png-physical-aspect driver option requires both PNG/APNG workload markers and actualMetal adapters; fresh gate133 passed/13 ignored across ordinaryGPU plus selectedPNG/APNG suite, every executed row verified. Evidence: `research/png-apng-physical-aspect-current-2026-10-11.json` and linked hashed logs/report/source/fixture. APNG transient canvas/scratch and direct decode_apng ownership are not yet fully request-budgeted; wrapper retained-pixel admission is insufficient proof of full peak bounds. PhysicalHDR/playback/variants remain open.

2026-10-11 unlocked desktop refresh: actual Metal live RGBA16Float / ExtendedSrgbLinear presentation passed for authored raster (173 frames / 3.010 s) and native CR3 6188x4120, 25,494,560 samples (129 frames / 3.024 s). No validation errors; source CPU credit released after RAW upload as asserted by probe. macOS reported EDR active with current headroom 8.791209, potential 16. These are presentation checks with deliberate redraw pacing, not throughput benchmarks, physical luminance/color calibration, or full viewer resize/recovery qualification. Evidence: research/metal-live-hdr-unlocked-2026-10-11.json and hashed logs.

2026-10-11 APNG memory ownership: float composition canvas and PREVIOUS saved canvas reserve before allocation; selected output freezes the same managed canvas, keeping credit until its last reference drops. Color preparation now receives the same budget and cancellation predicate. Three APNG tests passed, including exact composition oracles and insufficient canvas/color-buffer admission with final zero credits. Full decoder library results are retained in research/apng-managed-canvas-2026-10-11.json with hashed logs. Encoded frame reconstruction and PNG codec RGBA8 scratch remain untracked; this is not complete transient-memory qualification.

2026-10-11 APNG frame admission: reconstructed PNG reserves exact encoded capacity before allocation and retains adopted credit through decoding; decoder pixel output and RGBA conversion are both admitted before conversion, with managed RGBA ownership through color preparation. Pressure sweep 240..2048 bytes in steps of16 covers success/refusal, PREVIOUS restoration, final zero credits and peak <= limit. Full decoder library passed 1004 tests, 71 ignored. Initial test compilation missing a test-only Arc import was corrected; failed and passing logs preserved in research/apng-frame-admission-2026-10-11.json. PNG codec private inflater/row allocations remain controlled by codec limits rather than the shared budget; do not infer complete transient-memory accounting.

2026-10-11 APNG constructor limit and Metal refresh: image0.25.10 PngDecoder::new uses no limits; subsequent set_limits does not update png::Reader internal allocations (dependency source TODO). Use PngDecoder::with_limits before header/row initialization. Three APNG tests passed after correction (composition oracles, pressure sweep, malformed/cancel paths); actual Metal PNG/APNG pair passed2 tests with exact SDR/HDR frames before and after reverse swap restore, final budgets zero. Evidence: research/apng-constructor-limits-metal-2026-10-11.json and hashed logs. Codec internal scratch still uses its own cap rather than shared reservation; no claim of full transient accounting or physical display calibration.

2026-10-11 selected16-bit APNG: replaced unconditional RGBA8 intermediate with depth-aware RGBA16/RGBA8 conversion and doubled conversion reservation for16-bit. Earlier16-bit sources were explicitly refused (not silently truncated); now the supported selected-frame path retains low bits and alpha. Two1x1 authored sRGB frames with adjacent16-bit RGB/alpha values match independent f64 transfer-function expectations; output lifetime charged16bytes then zero. Initial regression correctly exposed the old8-bit-only guard; removed that guard for16-bit and also installed codec limits at initial metadata decoder construction. Full decoder1005 passed/71 ignored. Actual Metal suite3 passed, including two16-bit frames against independent linear SDR/HDR references and existing PNG/APNG swap cases; integrated report all executed suites verified. Evidence: research/apng-16bit-precision-2026-10-11.json and retained full logs/report/source/fixture hashes. Full16-bit variants, color profiles, playback and shared accounting of private codec scratch remain unqualified.

2026-10-11 packed APNG: removed8/16-only admission guard; PNG codec validates depth/color legality and expands packed samples. Authored1-bit grayscale,2-bit palette+tRNS(0/64/128/255 alpha),4-bit grayscale sources each have two SOURCE frames in reverse pixel order. Six selected frames match independently calculated f64 sRGB/alpha oracles, indices/count and retained64byte managed canvas; final zero credit. Full decoder1006 passed/71 ignored. Actual Metal selected PNG/APNG suite4 passed, including packed six-frame SDR/HDR comparisons before/after six swap writes/reads, errors0/source+queue0. Integrated ordinaryGPU and manual suite both verified. Evidence: research/apng-packed-depths-2026-10-11.json and retained report/log/hash directory. Full packed variants, playback and private codec shared-budget accounting remain unqualified.

2026-10-11 live resize probe preparation: opt-in RRRAH_HDR_RESIZE_PROBE=1 requests800x400 after30 presents, consumes actual window resize event, reconfigures HDR surface/viewports, checks acquired texture dimensions and requires at least30 presentations after confirmed resize. Raster zoom retained. Final probe cargo check passed. Live attempt was Occluded and Root registry reports screen locked again; no resize/presentation success claimed. Eleven current actual-Metal model readback tests passed including pressure refusal, retained SDR/HDR old frame and retry. This is separate offscreen evidence, not production live event-loop recovery. Evidence: research/metal-live-resize-preparation-2026-10-11.json and hashed logs.

2026-10-11 APNG chunk ownership: track default-image IDAT run, current animation data ownership and completion in CRC-validated scan. Refuse IDAT attached to a later animation frame, fdAT attached to default image, empty controlled frames and partial default-frame geometry before pixel admission. Four valid-CRC structural mutations refuse with zero used/peak shared credit. Positive separate static preview plus split consecutive IDAT reproduces all selected last-frame float samples and animation count4. Full decoder1007 passed/71 ignored; current actual Metal PNG/APNG four manual tests passed, SDR/HDR/swap and packed/16-bit precision markers retained. Specification: https://www.w3.org/TR/png-3/#11fcTL . Evidence: research/apng-chunk-ownership-2026-10-11.json and hashed logs/source. Lazy skipped compressed payloads are not fully inflated/validated; playback/all variants remain unqualified.

2026-10-11 application APNG TTL/cache/prefetch: ordinary and explicit actual-Metal tests use production load_cached_raster_mode/preload_raster helpers with16-bit and2-bit palette APNG sources. Each source stores two distinct selected frames under RAM128bytes/count2/TTL1second, keeps fresh entries, spills both after elapsed TTL, then restores1→0 via real prefetch while foreground decoder gate is held (5-second cancellation watchdog). RAM disabled by count0 for restore isolation; disk count2/65536bytes, queue128/count2, restore128. All serialized payloads/index/count and SDR/HDR Metal frames exact;2writes/2reads per source/errors0 and final shared credit0. Updated --metal-viewer-ttl-prefetch gate now requires new APNG suite as well as existing EXR/HDR scenario:131 ordinaryGPU passed/13 ignored plus1 HDR and1 APNG manual tests passed; all rows verified. Evidence: research/apng-viewer-ttl-prefetch-2026-10-11.json and retained report/log/hash directory. This is bounded real-time TTL and selected-frame integration, not animation playback/long collections/physical slow disk qualification.

2026-10-11 large APNG admission: replace arbitrary four-times-output512MiB dimension rejection with actual RGBA32Float output bound; managed canvas/frame/linear/PREVIOUS reservations govern simultaneous working admission. Authored4000x3000 two-frame16-bit APNG now decodes selected1 under512MiB shared limit. Full and128MiB partial pressure cause clean refusal; retry preserves all48million components within1e-7 of independent f64 sRGB oracle and bit-exact original/restored constant field. Managed decode peak480554140bytes; retained192000000; final CPU/queue0. Actual Metal128x96 SDR/HDR exact against1x1 equivalent-aspect constant reference before/after192MB swap; one write/read/errors0. Full decoder1007 passed/71 ignored; new --metal-large-apng integrated gate131 ordinaryGPU+1 manual passed/13 ignored, all rows verified. Evidence: research/large-apng-memory-metal-2026-10-11.json and retained logs/report/source/fixture hashes. Constant authored content is capacity/numeric evidence, not varied-content offset qualification; PNG private scratch excluded from tracked peak; no physical display claim.

2026-10-11 APNG PREVIOUS patch ownership: save only frame rectangle into managed buffer and restore rows in place, retaining original canvas allocation. Cancellation checked per save/restore row. Independent authored4x3 canvas/2x2 offset patch regression admits192+64bytes (full canvas copy would exceed256-byte limit), proves snapshot offsets, restores every original component including surroundings, finalzero. Full decoder1008 passed/71 ignored with existing composition and pressure oracles. New actual-Metal test selected3→1→2 disposal frames against independent5x3 float composition files; full64x64 SDR/HDR exact, finalzero. Updated PNG/APNG integrated gate131 ordinaryGPU plus5 manual passed/13 ignored, all rows verified. Evidence: research/apng-previous-rectangle-2026-10-11.json and retained report/log/hash directory. Memory savings equal patch-vs-canvas difference; runtime speedup not benchmarked. Full playback/variant/codec scratch accounting still unqualified.

2026-10-11 PREVIOUS paired timing and full-frame fast restore: on12MP canvas/64x64 region,9 alternating measured pairs after3 warmups give prior managed full-copy median38.176250ms versus production rectangle helper0.019000ms. Managed peaks384000000 versus192065536bytes; all48million components match authored position-dependent field bit-exact after both paths; final0. This isolates save/overwrite/restore/release, not full decode/viewer speedup; other host work affects timings. Preserve full-canvas PREVIOUS ownership transfer rather than copying back all samples; new3-frame regression full-white→full-green PREVIOUS→partial-blue OVER proves white base restored and untouched pixels intact, final64byte output then0. Full decoder1009 passed/72 ignored (benchmark separately executed). Current Metal gate131 ordinaryGPU+5 PNG/APNG+1 largeAPNG passed/13 ignored, all rows verified. Evidence: research/apng-previous-paired-current-2026-10-11.json and retained paired/raw logs/report/source hash.

2026-10-11 raster foreground prefetch priority: protect each successfully cached nearest-first raster neighbor with CacheLease until the current neighbor queue finishes or a new foreground request replaces it. Clear protections before idle wait/new request, leaving TTL/normal eviction active afterward. Reuse extracted raster_display_key for lookups and protection, including assumed-color/scalar interpretation; scalar fallback protects actual loaded window. Regression with visible16-bit APNG plus nearest palette APNG/count2 proves farther raster cannot evict nearest while protected, nearest RAM hit bypasses decoder, release allows farther replacement, final shared credit0. All210 ordinary app tests passed/57 ignored. Current integrated actualMetal131 ordinaryGPU + navigation + HDR TTL + APNG TTL manual suites passed, all rows verified. Evidence: research/raster-prefetch-priority-2026-10-11.json and retained report/log/hash directory. Priority regression is bounded; no claim all queue/failure/byte-cap cases or RAW/model parity. Lease lookup currently repeats sampled source fingerprint IO; no speedup timing claim.

2026-10-11 raster priority count/byte/failure coverage: expanded loader/cache retention regression to count2/1MiB and independent80-byte/no-count policies. Real truncated PNG prefetch fails without cache membership or retained credit, leaving only selected16-byte APNG; next64-byte palette frame admitted and leased, farther64-byte candidate cannot displace it. Nearest hit bypasses decoding; releasing protection permits farther replacement; final root0. All210 ordinary app tests passed/57 ignored. Production unchanged from prior actualMetal navigation/TTL report; no redundant GPU run claimed. Evidence: research/raster-prefetch-count-byte-failure-2026-10-11.json and hashed log/source. Equivalent model/foregroundRAW scoped queue protection remains to audit; this helper test does not prove complete scheduler/collection behavior.

2026-10-11 model prefetch priority: foreground neighbor worker retains successfully cached models using scoped CacheLease protection, cleared alongside raster protection at new request/idle waiting. Shared model_display_key helper preserves path/fingerprint/subimage identity for cache and prefetch lease. Regression STL/OFF/OBJ/PLY under count2 and independent two-model byte cap: real corrupt STL fails without membership, nearest retained against farther candidate, nearest hit bypasses decoder, release permits farther replacement, final shared credit0. All211 ordinary app tests passed/57 ignored. Current actual-Metal gate ordinaryGPU+mixed navigation+model upload worker suites verified; logs retain queue replacement/cancellation/source-lifetime evidence. Evidence: research/model-prefetch-priority-2026-10-11.json and retained report/log/source hashes. Small authored helper scenarios are not full scheduler/collection qualification; foregroundRAW equivalent protection and redundant fingerprint IO remain to audit.

2026-10-11 foregroundRAW scoped prefetch priority: preload_raw_with_lease returns admission plus optional resident CacheLease using already resolved key; worker retains admitted RAW neighbors until queue completion/new request, clearing beside raster/model protections. Boolean test wrapper preserves prior API/admission semantics including TTL0. No extra source fingerprint IO for RAW lease. Manual native original IMG9043.CR3 copied to five distinct timestamp identities: forward/backward nearest-first plans, count2 and independent two-mosaic byte cap, corrupt neighbor rejection, farther admission refused while nearest leased, full shared-budget occupancy still allows pointer-identical nearestRAM hit, release permits farther replacement, visible retained, finalzero. All211 ordinary app tests passed/58 ignored; manual original priority test separately passed. Current actual-Metal ordinaryGPU+mixed navigation+RAW worker navigation suites verified. Evidence: research/raw-scoped-prefetch-priority-2026-10-11.json and retained report/log/source hashes. One camera source and helper scenarios do not prove all cameras/full scheduler/long collections; disk writer persistence priority remains separate.

2026-10-11 single-resolution raster/model prefetch protection: lazy FnOnce cache-key resolver after cancellation check supplies same key to load and returned CacheLease; worker consumes returned lease instead of rereading source fingerprint. Scalar-window fallback and assumed-color flag use actual resolved load identity. Ordinary regression proves cancelled requests never invoke key resolver/decode and returned STL/APNG lease shares exact loaded allocation/subimage; finalzero after last lease. Initial refactor made legacy helper acquire lease, eagerly removingTTL0 entries and failing4 linear-import tests; explicit retain mode fixes legacy semantics, failed/passing logs preserved. All213 ordinary app tests passed/58 ignored. Current actual-Metal ordinaryGPU+navigation+model worker+HDR/APNG TTL suites all verified. Evidence: research/prefetch-resolved-key-2026-10-11.json and retained report/log/source hash directory. Removes redundant sampled fingerprint IO, but no whole-viewer timing speedup claimed.

2026-10-11 current prefetch large-collection Metal refresh: 100000 hardlink paths to four small authored raster sources, 512 navigation visits, exact128x96 SDR/HDR frames, scoped nearest-first leases, full managed-budget pressure and stale cancellation; scan/navigation8.1918s, total78.45s including path setup/cleanup. Separately eight spatially varying12MP float frames completed110 swap restores in120.230s, full12MP SDR exact and63x65 HDR exact each cycle; 110 mid-batch cancellations; writes remain8; final managed/queue0. OrdinaryGPU131 passed/13 ignored, sustained manual1 passed. Evidence: research/metal-collection-priority-refresh-2026-10-11.json and hashed report/log directory. Not100k independent source decodes, full-resolution HDR, physical slow disk, OS-wide pressure or physical display calibration.

2026-10-11 live HDR resize after user unlock reply: two actual Metal launches configured RGBA16Float but surface remained Occluded; neither presented a frame. Resize remains unverified. Logs/hashes: research/metal-live-resize-unlock-attempt-2026-10-11.json.

2026-10-11 full-resolution HDR collection: eight spatially varying authored12MP float frames on AppleM4Max/Metal; 150 successful restores and150 admitted mid-batch cancellations across120.803s. Every4000x3000 SDR byte and HDR binary16 component equals pre-spill render; CPU components bit-exact against authored field. Writes remain8; final managed/queue0. New explicit --metal-collection-full-hdr requires duration; driver sets environment deterministically, reports output extent and rejects missing workload markers. Separate actual driver gate131 ordinaryGPU passed/13 ignored plus1 manual collection passed, output4000x3000 verified; invalid flag combination rejected. Evidence: research/metal-full-hdr-collection-2026-10-11.json and retained hashes/logs. Render equality is not independent color/renderer fidelity, physical HDR, diverse decoder qualification, slow-disk or OS-wide pressure. Partial concurrent GPU qualification means elapsed soak duration is not a throughput benchmark.

2026-10-11 collection evidence audit: qualification now requires exactly one matching Metal seed, complete duration, at least8 restores, read count equal to cycles, exactly8 writes, matching SDR/HDR/parallel cancellation counts, requested HDR extent and final root/queue0. Four verifier regression tests reject truncated, duplicate, wrong-resolution/duration/count/accounting/backend and skipped evidence; serial no-parallel-scratch host remains valid. Revalidated retained actual-Metal120s fullHDR,10s fullHDR and120s63x65HDR logs successfully without rerunning unchanged GPU workload. Evidence: research/metal-collection-evidence-audit-2026-10-11.json. Full100-format, independent color fidelity, physical HDR, live resize, slow disks/OS pressure and NVIDIA remain open.

2026-10-11 live HDR probe correction: total redraw attempts incorrectly counted successful presentation toward120-attempt failure threshold. Probe now bounds consecutive surface unavailability to10seconds, reset on each successful frame; diagnostic reports actual presented count. ActualMetal remainedOccluded and terminated after10.007s without validation errors or presentation; live resize still unverified. Evidence: research/metal-live-probe-unavailability-2026-10-11.json and logs. This changes only qualification example, not production viewer.

2026-10-11 fitting cache policy updates: WeightedLru::set_limits returns without cloning/sorting candidate keys when current byte/count membership already fits, after pin feasibility validation. Existing deadlines and recency remain unchanged; no expiration semantics changed. New100000-entry regression verifies zero key clones, existing no-TTL memberships remain live afterTTL0 policy change, and touched-key LRU ordering survives. Memory/cache suites192 passed/5 manual ignored; actualMetal eight12MP collection10s/full4000x3000 SDR-HDR verified by strict evidence helper. Evidence: research/cache-fitting-limit-update-2026-10-11.json and hashes/logs. Removes unnecessary planning allocation/work; no measured whole-viewer speedup claimed.

2026-10-11 broad post-policy regression: current ordinary rrrah-decode, rrrah-gpu and rrrah app library/binary/integration suites completed70 harnesses,1418 passed,0 failed,226 explicitly ignored; actualMetal required with optionalGPU disabled. App213 passed/59 ignored. Evidence: research/cross-format-cache-metal-2026-10-11.json and full hashed log. This broad regression covers implemented fixture paths; skipped manual/corpus cases remain unverified and it does not prove full100-format, independent color or physical HDR qualification.

2026-10-11 selected TIFF pages: native bounded top-levelIFD walker supports classicTIFF/BigTIFF both byte orders, checks complete tables/next pointers, caps4096 pages/entries, rejects cycles/missing pages and cancellation before codec. Temporarily redirects firstIFD pointer in already admitted mutable source then restores after success/error; original pixel/tag offsets remain absolute, no whole-file copy. Existing codec applies selected orientation/ICC/alpha path. New3 regressions validate authored gray8 pages/orientation, source preservation, malformed chains, magic file route and failed admission. Full decoder1012 passed/72 ignored. Evidence: research/tiff-selected-pages-2026-10-11.json. Newpage-specific compressed16/float, ICC/alpha and viewerRAM/swap/Metal qualification remain open; TIFF row still not fully qualified.

2026-10-12 selected TIFF viewer metadata/cache/Metal: bounded complete top-levelIFD count now attached with selected index to first and later TIFF/BigTIFF pages; source pointer restored after decode before metadata/color conversion. Full DICOM signature retains precedence over arbitrary TIFF-like preamble, caught by existing DICOM oracle regression; failed/passing logs preserved. Decoder1012 passed/72 ignored. ActualMetal four authored classic/BigTIFF LE/BE gray8 files with second-pageorientation6: explicit assumed-sRGB then linear preparation, independently authored pixel references,63x65 SDR/HDR exact, count1/32byteRAM, full-budget liveRAM hit, TTL0 visible protection then spill, reverse1->0->1 restore with selection metadata, writes2/reads3/errors0 each; final CPU/queue0. Evidence: research/tiff-pages-cache-metal-2026-10-12.json. Initial test GPU refusal without color preparation and expired-lease refusal preserved in logs; these are required API semantics, not bypassed. Page-specific ICC/associatedalpha/compressed16/float and full TIFF variants remain unqualified.

2026-10-12 independent compressed TIFF precision: eight existing tifffile/imagecodecs-produced Deflate TIFF uint16/float32 files, both byte orders, original and last-page one-low-bit changed variants; all24 selected pages match authored RGB component bits and opaque alpha exactly, retain3-page metadata and ICC. Native output then ICC preparation,32 reverse2->0->1->2 swap restores,63x65 SDR/HDR transport exact, writes3/reads4/errors0 perfile and finalroot/queue0. Manifest source hashes verified. Explicit --metal-tiff-pages driver requires both tests, four classic/BigTIFF cases and all eight precision cases; actual gate131 ordinaryGPU+2manual passed/13 ignored, all executed suites verified. Evidence: research/tiff-compressed-precision-metal-2026-10-12.json and retained logs/report. Floatproducer0..1 does not qualify above-one HDR; distinct page ICC, associatedalpha and independent color correctness remain open.

2026-10-12 signed HDR TIFF and explicit linear policy: decode previously ignored assume_untagged_linear_srgb on unprofiled floatTIFF, retainingUnspecified and refusing display. Explicit opt-in now declares onlyfloatTIFF linear-sRGB; default unknown color still refuses preparation, integerpolicy unchanged, embeddedICC precedence tested with independent precisionfiles. Two authored DeflateRGBA32 TIFFs LE/BE, threepages each: RGBabove1/negative/signedzero, straight/associatedalpha, orientation6. Allnativebits match authored straight/oriented references; Metal63x65 HDR target containsfiniteabove1andnegativecomponents; SDR/HDR everycomponent equals independent authored raster. Pressure/cancel restores publishnothing then retryexact; reverse2->1->0->2 payloadbits/selection retained, writes3/reads4/errors0, finalCPU/queue0. Actual driver131ordinaryGPU+3manual passed/13ignored allverified; finaldecoder1012passed/72ignored. Evidence: research/tiff-signed-hdr-linear-policy-2026-10-12.json, authoredgenerator/manifest and retained failed/passinglogs. No independent HDRproducer/color, physicalHDR or full TIFFvariant qualification claimed.

2026-10-12 distinct page ICC qualification: authored3-page Deflate RGB8 TIFF contains identical codes with LittleCMS-generated sRGB, explicitly gamma1-edited/independentlyparsed RGB profile, sRGB. Native ICCbytes differ onlywhereexpected, first/thirdprofiles/preparedframes identical, middle differs. Both fallbackflags do notoverrideICC. Independent LittleCMS relative-colorimetric RGB8 oracle agrees after preparedfloat->sRGB encoding within1code; initial idealcurve0.00015 testfailed and preserved, replaced by actual-profile quantizedoracle ratherthan claiming exactICCfloatprecision. Four reverse1->2->0->1 swaprestores retainselection/floatbits and63x65 SDR/HDRexact; writes3/reads4/errors0; finalCPU/queue0. Fixture/oracle/manifest regenerate bitidentically with normalizedICCtimestamp/ID. Updated explicitTIFFgate requires4manualtests; actual131ordinaryGPU+4manualpassed/13ignored allverified. Evidence: research/tiff-distinct-page-icc-2026-10-12.json. Full floatingICCaccuracy, arbitraryprofiles and physicalHDR remain unqualified. Production unchanged from final1012decoderpass revision.

2026-10-12 viewer TIFF selected-page integration: actual production raster loader/cache/preload helpers retain3 page-specific keys for one TIFF with distinctICC.96byte/count3RAM, full-root-pressure RAMhits bypassdecoder,1secondTTL spillsall3 backgrounds; count0RAM then reverse1->2->0 swap preloads bypass heldforegrounddecoder ticket (watchdog). Payload/metadata/128x96MetalSDR/HDR exact, writes3/reads3/errors0, finalroot0. CPU-only marker now reports payloadonly; Metalmarker separately requiresactualadapter andSDR/HDR. All214ordinaryapp passed/60ignored; explicitTIFFgate131ordinaryGPU+4fixturemanual+1viewerhelper passed/13ignored allverified. Evidence: research/tiff-viewer-selected-cache-2026-10-12.json and preserved logs (initial misplacedtest compile failure retained). This is productionhelper integration, not liveUIeventloop/workercollection or largeTIFF qualification.

2026-10-12 rapid page navigation fix: App previously based every bracket-key step on lastdisplayedindex, so whilepage1loaded asecondforward againrequestedpage1 and reversefromvisible0 didnothing. Generation-bound pendingtarget now accumulates0->1->2 and reversesbeforecompletion without changingvisibleindex; currentterminalevent/newfile clearsit, cancellation/obsoletegeneration ignored. Scientificwindow adjustment retainspendingpage. HeadlessactualApp/queue regressions cover0->1->2->1->0, boundary, cancelledtokens, staleRasterReady/Failed, currentfailure reset. All216ordinaryapp passed/60ignored; separatelyactualMetal TIFFviewer cache/TTL/blockdecoder helper passed1 with128x96 SDR/HDRexact/root0. Evidence: research/pending-page-navigation-2026-10-12.json. Livephysicalkeyboard/window interaction remains unverified.

2026-10-12 selected ICO images: native bounded directory selector provides depth/area-ranked imageindices, preservingcodec defaultbest/tiebreak; validates4096-entry cap and payloadbounds, managedshared-budget rank scratch released beforepixels. Redirects selectedrecord/count in existing admitted source and restoresheader afterresult, no fullfilecopy; selectedindex/count retained. Three PillowPNG authored entries prove exactRGBA/alpha/dimensions and rankorder, malformed/memoryrefusal cleanup; existingBMP-backed baseline remainspassing. ActualMetal64byte/count1RAM full-pressurehit, visibleTTL thenreverse2->0->1->2 swap keepsmetadata and63x65 SDR/HDRexact, writes3/reads4/errors0/finalCPUqueue0. Decoder1014passed/72ignored; app216passed/60ignored; combinedICO/TIFFgate131ordinaryGPU+6manual passed/13ignored allverified. Evidence: research/ico-selected-images-2026-10-12.json and source/fixture/log hashes. Multi-DIB/masks, embeddedcolor and multiCUR remain unqualified; ICOrow not fully qualified.

2026-10-12 selected CUR and hotspot tie fix: boundedmanagedICOselector now supportsCUR, validateshotspots and ranksbyarea without treatinghotspotY asbitdepth. Selectedentry pixels/point/imageordinal/count staytogether; header restored afterdecode. Legacycursor normalizer usedlastmaxarea hotspot whileICOcodec couldchoosefirstmaxarea pixels whenlastdirectoryentrywassmaller; tie rule corrected toactualcodecdefault. Authored3PNG-backedentries (twolargesttieddifferentpoints+smalllast) reproduceandprotectthiscase; invalidhotspot refusal/root0/inputunchanged. Decoder1016passed/72ignored, app216passed/60ignored. Actual combinedgate131ordinaryGPU+7manual passed/13ignored allverified: CUR/ICO64byte/count1RAM fullpressurehits, visibleTTL, reverse2->0->1->2 restorekeepspoint/metadata and63x65SDR/HDRexact/finalrootqueue0. Evidence: research/cur-selected-hotspots-2026-10-12.json and hashes/logs. Multi-DIBmaskvariants andliveOS cursorpositioning remainunqualified; CURrow notfullyqualified.

2026-10-12 unlocked desktop: actual Apple M4 Max / Metal raster HDR window probe passed:176 presentations/3.015s, actual800x400 resize confirmed and146 subsequent frames; RGBA16Float/ExtendedSrgbLinear, no validation errors. macOS reported active EDR headroom11.983057/potential16. Evidence: research/metal-live-resize-unlocked-2026-10-12.json and hashed log. This qualifies the live raster probe resize/presentation, not physical luminance/color calibration or production viewer recovery/native RAW resize.

2026-10-12 native CR3 live HDR resize: Apple M4 Max/Metal, NativeRawDecoder6188x4120 (25,494,560 samples),131 presentations/3.020s; actual800x400 resize confirmed,101 subsequent frames. RGBA16Float/ExtendedSrgbLinear, no validation errors; source managed CPU credit zero after upload (probe assertion). Evidence: research/metal-live-raw-resize-unlocked-2026-10-12.json and hashed log/source. This validates native RAW live probe presentation/resize, not production viewer memory-pressure recovery or physical color/luminance calibration.

2026-10-12 current-source recovery refresh: all11 model readback tests passed with Metal required and optional GPU disabled. ExactSDR/HDR retained on depth resize admission refusal and replacement cancellation; pressure release/retry, same-size reuse and finalcredit0 verified. Inspected production resize retains surface/config until admission and schedules250ms retry. Evidence: research/metal-model-recovery-current-2026-10-12.json and hashed sources/log. Offscreen checks still do not prove production window event-loop recovery under pressure.

2026-10-12 CUR DIB selected-image qualification: new authored24bitDIB fixture includes padded rows and AND-mask transparency, independently read by Pillow as ICO. Three selected CUR images retain native pixels, matched hotspots, selection metadata through RAM64bytes/count1, full-pressure hits, visibleTTL and reverse swap; exact63x65SDR/HDR on Metal, finalroot/queue0. All3ICO/CUR integration tests passed; driver now requires third DIB workload marker/adapter. Combined actualMetal gate passed. Evidence: research/cur-dib-mask-metal-2026-10-12.json and hashed logs/fixtures. Other DIB bitdepth/mask variants and liveOS cursor placement remain open.

2026-10-12 CUR DIB depth extension: authored1/4/8bit indexed,16bitRGB555 and32bitBGRA three-selected-entry fixtures independently decoded per-entry by Pillow. Exactnativepixels, matching hotspots, partial32bitalpha orANDmask transparency survive64byte/count1RAM/fullpressurehits/visibleTTL/reverse swap and exact63x65MetalSDR/HDR; finalroot/queue0. Combined gate135passed/13ignored, all executed suites verified; driver requires allfive depth markers and adapters. Evidence: research/cur-dib-depths-metal-2026-10-12.json. Uniform-color authored fixtures do not qualify arbitrary palette packing, compression/bitfields, mixed per-pixel masks or liveOS cursor placement.

2026-10-12 mixed CUR DIB coverage: six9x3 authoredfixtures (1/4/8/16/24/32bits) vary pixels across packedbyte/row boundaries with bottom-up rows, mixedANDmask and32bitpartialalpha; per-entryPillow decode independently validates expectedRGBA. Native pixels/hotspot and exactMetalSDR/HDR preserved through full-pressureRAMhit, TTLexpiry andswap; finalroot/queue0. Combinedgate136passed/13ignored, all executed suites verified. Twoinitialtest setupfailures retained: ZEROttl beforehit andfixedexistingTTLdeadlines; corrected sequencing, no productionchange. Evidence: research/cur-dib-patterns-metal-2026-10-12.json. UncompressedBITMAPINFOHEADER only; compression/bitfields/extendedheaders/liveOScursor placement remainopen.

2026-10-12 ICO/CUR viewer selected cache: generalized existingTIFF application qualifier to production load_cached_raster_mode/load_raster_for_display_typed/preload_raster for ICO,PNG-CUR,DIB-CUR three selected entries. Separate keys survive full-rootpressure RAMhits withoutdecode,1sTTL,3swapwrites and reverseprefetch fromswap whileforegroundDecodeGate held; exactserializedpayload/hotspot and128x96MetalSDR/HDR,3reads/errors0/finalCPU0. App217passed/61ignored; combinedgate142passed/13ignored allverified (131GPU+5ICO/CUR+1ICO/CURviewer+4TIFF+1TIFFviewer). Evidence: research/ico-cur-viewer-selected-cache-2026-10-12.json. Headlesshelper evidence does not prove livekeyboard navigation.

2026-10-12 liveCUR keyboard navigation: current normalviewer onAppleM4Max/Metal SDRsurface(Bgra8UnormSrgb), cachecount1/TTL1s, actual bracketkey events producedindices0->1->2->1->0 with matching dimensions/windowtitles; targetedwindow screenshots captured (image2 visuallyinspected green). Viewerclosednormally/exit0. UnprofiledPNG-in-CUR strictmode refused as designed; explicit--untagged-color srgb enabled display. Evidence: research/ico-cur-live-navigation-2026-10-12.json and screenshots/logs. LiveSDR only; no inferredspecificswap-hit orHDRsurface/calibratedcolor claim.

2026-10-12 actual mainviewerHDR: AppleM4Max/Metal surfaceRgba16Float/ExtendedSrgbLinear selected three authoredfloatTIFFpages0->1->2->1->0 using keyboard; explicitlinear-sRGBpolicy/cachecount1/TTL1s. ActualAXwindowsizes640x480then900x650 confirmed; targetedpage3screenshots at bothsizes visuallyinspected correctrotated2x3content. Normalclose/exit0. Evidence: research/metal-live-hdr-viewer-resize-2026-10-12.json and screenshots/logs. This closes selected mainviewerHDR presentation/resize path only, not injectedpressure recovery, physicalHDR calibration or inferredswap-hit.

2026-10-12 live mainviewer model budget recovery: actualMetalHDRRgba16Float/ExtendedSrgbLinear,16MiBmodelbudget. Enlarging1100x800logical refuses13,516,800byte depth replacement with7,680,036bytesresident;39deferredretryrefusals retain model visibly. Shrink600x432then400x332 releasesdepth; reissued identical1100x800 renders model withno furtherrefusals. Before/refused/recovered/retry targeted screenshots, two large states visuallyinspected; normalexit0. Evidence: research/metal-live-model-budget-recovery-2026-10-12.json. This is managedbudget pressure/supersedingresize recovery, not OS-widepressure or autonomoussamependingrequest recovery afterexternalrelease; physicalHDR remainsuncalibrated.

2026-10-12 largefloat delayed storage: new ignored64MiB2048x2048RGBA32Float soak passed3cycles/96.628s. Eachcycle realfilewrite, full128MiBroot pressure refusal, mid-read cancellation and retry compares everyfloatbit includingnegativezero/signedHDRextremes. Disk2objects/134217856bytes bound, thirdobject evictsfirst;3writes/3reads/errors0/finalrootqueue0. Ordinarydelayedstorage4passed/5ignored. Storagegate verifier now requires marker,requested duration and>=3cycles; syntax checked, fullcombinedstoragegate notrerun. Evidence: research/large-float-delayed-swap-2026-10-12.json and hashedlogs. ArtificialdelayedlocalI/O/managedpressure only; physicalslowdrive,OS-widepressure andcolor/display remainseparate.

2026-10-12 combined storage/GPUgate completed:140passed/13ignored, all executed suites verified (131ordinaryGPU+9storage). Storage331.09s includes100000distinctkeys/100000writes/100000reads/1000pressure refusals, independentbyte/count10s modes, idleTTL/physicalfilereclamation, delayedwriter burst, admittedreadcancel/stalewaiter andlargefloat3cycles/101.586s with64MiBpayloads. Finaltrackedcredits0. Evidence: research/storage-large-float-combined-2026-10-12.json and fullhashedreports/logs. Largefloatcancellationtimer25ms doesnot independently assertallocationphase; separateordinarycanceltest doesassertpixeladmission. Localdelay/managedpressure notphysicalslowdisk/OSpressure. Currentcatalogue audit: research/goal-open-format-audit-2026-10-12.json;94subsets/6pending,0fullyqualified.

2026-10-12 largefloat cancellation evidence strengthened: replaced25mstimer with observed64MiBmanaged pixel admission and>=65536+64bytesactualSlowIo read beforecancel, assertedbothconditions. Three64MiBcycles/87.84s passed exactfloatretry/pressure/eviction/finalrootqueue0; ordinarydelayedstorage4passed/5ignored. Newpureverifier rejects oldtimermarker, duplicate/truncated/shortcycle/shortduration/wrongextent logs;7tests passed andactualnewlog accepted/oldlog rejected. Evidence: research/large-float-admitted-cancellation-2026-10-12.json and hashedlogs. No productioncodechange; currentfocusedtests do notconstitute rerun offull9-testcombinedgate. Physicalslowdrive/OSpressure remainopen.

2026-10-12 CAP boundary audit: primaryLoC listsCAP/CAPTURE separately fromIIQ; inspectednativePhaseOne pipeline onlyqualifiesP20+geometry/format3/firmware/calibration. Currentraw.pixls.us2016rows/43PhaseOne rows/0CAP-CAPTUREdownloadsuffixes; snapshot hash andcandidate rows retained. NoIIQalias inferred and94/6cataloguecounts unchanged. Requiredoriginal/provenance,sensorlayout/CFA/WB/profile/calibration/nativeboundeddecode/fullsensororacle/viewer-cache-swap-Metal listed in research/cap-source-boundary-2026-10-12.json. This is scoped research, not CAPdecoder qualification or worldwideabsence claim.

2026-10-12 selected-image prefetch: production foreground worker now queues neighbouring indices within the successfully loaded raster container before neighbouring files. Shared gallery planner preserves configured behind/ahead limits and travel-direction priority; selected index reaches DecodeRequest, generation cancellation and existing raster cache/lease/TTL/swap paths. File neighbours retain index0. Exhaustive small-gallery ordering/bounds test covers shared index planner. Live automatic container-prefetch timing and large-container sustained pressure remain unqualified. Evidence: research/selected-image-prefetch-2026-10-12.json.

2026-10-12 planned container prefetch Metal: selected-container qualifier now obtains restore order from shared production neighbour planner (backward2->[1,0], forward1->[2]) instead of fixed indices. Actual AppleM4Max/Metal TIFF,ICO,PNG-CUR,DIB-CUR selected payload and SDR/HDR outputs exact after nonzeroTTL/swap with foreground decoder blocked; each source3writes/3reads/errors0/finalCPU0. Two manual tests passed. Evidence: research/planned-container-prefetch-metal-2026-10-12.json and hashed logs. Headless helper integration does not establish live automatic worker telemetry or physicalHDR calibration.

2026-10-12 selected-prefetch observability: background raster worker emits debug ready(actual selected index/count/cache lease), refusal(error), and stale-generation skip records with source/generation. Current cargo check and whitespace check passed. Screen currently locked, so live automatic worker verification still pending. Ready marker does not identify RAM/swap/decode source. Evidence: research/selected-prefetch-telemetry-2026-10-12.json.

2026-10-12 combined selected-prefetch Metal gate: after production automatic selected-image queue and debug telemetry, current combined run passed142/failed0/ignored13 (131ordinaryGPU+5ICO/CUR+1ICO/CURviewer+4TIFF+1TIFFviewer), all_executed_suites_verified=true. Driver now additionally requires exactly one shared-planner reverse/forward marker per TIFF/ICO/CUR fixture, preventing earlier fixed-order helper output from satisfying current qualification. Evidence: research/planned-container-metal-gate-2026-10-12.json and retained report/log hashes. Live automatic window queue, physicalHDR, NVIDIA and complete100format scope remain open.

2026-10-12 selected-container stale generation: TIFF/ICO/PNG-CUR/DIB-CUR qualifier now rejects all three planned indices under alreadyobsolete generation before swapread, RAMentry or sharedcredit (all0), while foregrounddecodegate held. Freshgeneration then restores3pages with exactpayload/MetalSDR/HDR and3writes/3reads/errors0/final0. Two current actualMetal manualtests passed; driverrequires additionalstale markers/syntaxvalid, fullcombinedgate notrerun after testonlyaddition. Evidence: research/stale-container-prefetch-metal-2026-10-12.json. Midreadcancel/livewindow stillseparate.

2026-10-12 admitted-cancellation combined storage gate:140passed/failed0/ignored13; all_executed_suites_verified=true. Nine sustainedstorage tests273.72s:100k writes+reads/1000pressurerefusals105.437s; separatebyte/count/combinedlimits; idleTTLphysicalreclaim; writerburst; stalewaiters;64MiBfloat3cycles117.519s with cancellation after actualallocation and>=64KiBreadprogress eachcycle, everyrestoredfloatbitexact, diskcount2/bytesbounded, finalrootqueue0. This supersedes prior timer-based combined cancellation evidence. Evidence: research/storage-admitted-cancel-combined-2026-10-12.json and retainedreport/log/sourcehashes. ArtificiallocalI/O/managedpressure only; physicalslowdrive/OSpressure/RSS/physicalHDR/NVIDIA/all100 remainopen.

2026-10-12 actual automatic container worker: current built normalviewer opened authored3pagefloatTIFF in isolatedonefilefolder. Productiondebug confirms automaticselectedindices ahead0->[],ahead1->[1],ahead2->[1,2], retained=true undercachecount3. ActualMetalAppleM4Max/HDRRgba16Float/ExtendedSrgbLinear; desktoplocked/occluded, no presentationclaim. Threeownqualificationprocesses stoppedwithSIGTERM; no normalcloseclaim. Evidence: research/live-automatic-container-prefetch-2026-10-12.json plusbuild/runlogs/settings/hashes. This closes initialforwardautomaticworker/configuredcount path, not livebackwardnavigation, generationreversal, physicalHDR orallsources.

2026-10-12 actualmiddle/end containerprefetch: currentnormalviewer sixevent-deadline runs(twoeach) exact index1/ahead1/behind1->[2,0],index2/ahead0/behind2->[1,0],index2/ahead0/behind1->[1]. ActualMetalAppleM4Max, retainedleases. Addedproductiondebugplan event withselected/count/direction/limits/indices. Initialfixed2sobservation lackedend-pageprefetch records, causeunresolved/originallogretained; no schedulerfixclaim. Currentbuildpassed. Evidence: research/live-middle-end-prefetch-2026-10-12.json. Desktoplocked/occluded; SIGTERMshutdown; no dynamicnavigationreversal/physicalpresentation claim.

2026-10-12 BAY rowimage boundary: publicBaySensorLayout::image_samples_per_row separates documentedQV5700 2576image samples from legacy2585fullrowunpack includingtrailerpositions. Mutationofall12trailerbytes/everyrow preservesallimage samples andchangestrailerpositions; outputcreditreturns0. SevenfocusedBAYtests passed/oneindependentoraclemanualignored; fulldecoder notrerun. Primarylayoutsource https://www.inweb.ch/foto/rawformat.html . Evidence: research/bay-row-image-boundary-2026-10-12.json. Originalcamera/CFA/WB/color/viewer remainpending; no implicitcrop/colorinference.

2026-10-12 whole native-library regression afterBAYAPI:1331passed/failed0/ignored76 (cache139/1,core119/2,decode1017/72,memory40/0,swap16/1). Explicit --libscope only; skippedfixture/hardware/integrationcases remainunverified. CatalogueMEFrow corrected tocurrenttypedUnqualifiedMefcolorrefusal. Catalogue100rows retains6Pending. Evidence: research/native-library-regression-2026-10-12.json and hashedfullrunlog. This doesnotqualify all100formats orphysicalhardware.

2026-10-12 AI strictfailure localization: savedpinnedVectorApple artifacts(nativeRGBAhashverified againststrictreport) show29mismatchingpixels insidefullyuniform5x5oracle neighbourhoods,maxchannelerror38; probe(210,21) native[0,135,68]/oracle[38,135,62]. Radius3uniformneighbourhood hasnomismatches. Nextsourcecolor/groupcompositingtrace candidate; causalityunproven, no newrender/fix/productionqualification. Evidence: research/ai-flat-difference-probes-2026-10-12.json and visualization/probecoordinates. AIremainspending.

2026-10-12 AI currentICC isolation: authored16x16constantPDF withpreviouslytracedgreenCMYK sample decodedbycurrentnativepublicrasterpath; all256RGBApixels[0,135,68,255]. IndependentLittleCMSfloatCMYKpercent->sRGB all4intents round/clamp[0,135,68], agreeingnative; Popplerdefault[38,135,62]. NativefixedICCconversion not disproven bydefaultPoppler mismatch for thissample. Popplerexplicitprofileinput refused; profileheaderalreadyinput/scnr, initialoutputclassassumptioncorrected; rejectioncauseunresolved. No acceptancechange/fix/allAIqualification. Evidence: research/ai-cmyk-current-profile-oracle-2026-10-12.json and pinnedinputs/logs/oracle.

2026-10-12 diagnosticPoppler ICCcheck: official26.10.0source wronglychecksPerceptual OUTPUTdirection whilevalidating INPUT. ExactnativeICC supportsPerceptualinput only. Separatetempbuild changesonlythischeck toUsedDirection; originalICCacceptedunmodified. ConstantoriginalCMYKfloatnative/LittleCMS[0,135,68]; patchedPoppler[0,134,67] equalsLittleCMSafter8bitsourcequantization. AuthoredquantizedsourcePDF nativeandpatchedPoppler all256pixels EXACT[0,134,67]. Evidence: research/ai-poppler-profile-check-fix-2026-10-12.json, patch/buildlogs/inputs/oracles/hashes. Singleconstantcolor only; fullAIgeometry/fidelity anddefaultpolicies remainopen, no productionrendererchange/tolerancerelaxation.

2026-10-12 fullVectorApple sameCMYKprofile reference: pinnedoriginalsource/pinnednativeRGBAhashverified; freshpatchedPoppler26.10.0withunchangednativeCGATSprofile. Strictdifferentpixels33072->28120,maxerror216,MAE3.905608->1.949806. Greenprobe210,21nowexact; whitegeometryprobe78,144stillnative[253,237,213]/referencewhite. Savednative heldfixed, no newnative/productionqualification. Nextoriginalpath/clip/meshcoverageoracle. Evidence: research/ai-full-page-same-profile-2026-10-12.json and freshreference/log/hashes. AIremainspending.

2026-10-12 fulloriginalAI whitepixelresolution: freshpatchedPoppler/originalCGATSICC atsourcepixel78,144 scales1/16/64/128/256 (croppedslicesoforiginalVectorApple,notisolatedreconstruction). 72dpiwhite;256xnonwhiteproxy0.423599,meanRGB[253.0083,239.7579,217.7915], approaching priorindependentcubicarea0.425110/nativecoverage108. Supports partialpaint, notgeometriccorrectionbasedonwhite72dpi. NativeRGB[253,237,213] stilldiffers; withinpixelcolorintegration/groupmodel unresolved; RGBaveragingnotformalequivalence. Evidence: research/ai-original-white-pixel-resolution-2026-10-12.json and allsourcecroppedoracles/commands/hashes. Nofix/tolerancerelaxation/AIcompletionclaim.

2026-10-12 AI coveredsourcegrid: testonlydiagnostic1..64grid withpath/allclips/cancel, selectedpixel78,144 edge32 ->436/1024hits, meanCMYK[.0165058,.1378029,.3855294,0] vscenter[.0166027,.1386821,.3871869,0]. IndependentLittleCMS showsbothcenter/mean CMYKcoveragebeforeICC roundnative[253,237,213], so centersamplingdoesnotexplainmosterror. ICCbeforeRGBcoverage differs/closertohighresoracle; grouping/rasterizationsemantics stillunadjudicated. Current3pagescomplete/final0, all6outputsbyteidentical;136ordinarytests/10ignored and1manual passed. Evidence: research/ai-covered-grid-conversion-order-2026-10-12.json and logs/lock/source/outputhashes. No productionAI/fix/tolerancechange.

2026-10-12 direct AI page isolation: PDF1.7 section7.3.6 requires transparent page-group backdrop, conversion to output device space, then media backdrop. Test-only diagnostic now uses replay_native_page_scene with zero-alpha initial pixels; arbitrary backdrop/group replay unchanged. New regression checks unpainted transparency and unassociated half-alpha red.137passed/10ignored; real3page replay completed/finaladmission0. VectorApple probe78,144 whiteRGB[253,237,213]->[253,239,217]; sameICC strict MAE1.949806->1.707407/differentpixels28120->28008/max216. onepage0 remains2494different/max58; onepage1exact. Strict failures retained, productionAI remainsPending; no physicalHDRclaim. Evidence: research/ai-page-isolation-2026-10-12.json and pinnedoutput/logs.

2026-10-12 raster replacement HDR recovery: shared GPU qualifier now checks both Rgba8UnormSrgb and Rgba16Float using existing respective renderer constructors. ActualMetalAppleM4Max small+12MP manualtests2passed; ordinaryraster17passed/8ignored. Texture/queuepressure and cancellation atvalidation/admission preservepreviousframebitexact; validblueretryexact; finalGPUqueue0. InitialSDRconstructor/HDRassertionfailed retained/correctedtestonly. Sourcecolors0..1, offscreenonly; physicalHDR/livewindow andsignedHDRqualification separate. Evidence: research/metal-raster-hdr-upload-recovery-2026-10-12.json and hashedlogs.

2026-10-12 signed/above-one HDR replacement recovery: strengthened12MP authoreduniformsource now retains[4,-.5,0,1] andvalidretry[-.25,0,4,1]. ActualMetalAppleM4Max small+large2passed. Exactbinary16centers independentlyspecified; everyframecomponent exactafter texture/queuepressure andsixcancellationcheckpoints; SDRclipsasexpected; finalGPUqueue0. Existingordinary17pass is priorrevision; currentfocused2rerun only. Testonlysource/helperstrengthening, offscreen/authoreduniformcolors only; physicalHDR, independentdecoding andlivewindow remainseparate. Evidence: research/metal-signed-hdr-upload-recovery-2026-10-12.json and hashedlog/source.

2026-10-12 admitted selected-page prefetch cancellation: actualproductionhelper/manualMetalAppleM4Max testpassed1/3.44s. Twoauthored64MiB2048squarefloatpayloads understablecontainerfingerprint; observedrestoreallocation>=64MiB then generation1->2 cancelsoldpage1(nooutput/lease/RAM/root0). Newpage2restorebypassesheldforegrounddecodegate; allfloatsexact/signedHDR,128x96SDR/HDRwholeplaneequalsauthoredreference; writes2/successfulreads1/errors0/finalroot0. Watchdogboundswaiting. Sourcefile notdecoded; no minimumphysicalreadprogress/liveeventloop/physicalHDRclaim. Evidence: research/selected-page-admitted-cancel-metal-2026-10-12.json and current/initiallogs/sourcehashes.

2026-10-12 combined recovery Metal gate: currentdriver gpu-only+metal-large-upload-cancel+newmetal-selected-page-cancel completed133passed/0failed/13ignored:131ordinaryGPU+1signed12MPupload+1admitted64MiBselectedpagecancel. All3suites verified_execution=true; bothnewrecoveryworkloads requireexactmarkers/phases/backend/signedvalues/finalaccounting. Pureevidenceverifier3tests rejectmissing/duplicate/skipped/wrongbackend/range/accounting; existingSDRonlylogs cannotqualifyHDRrecovery. Evidence: research/metal-recovery-combined-gate-2026-10-12.json and fullreport/logs. Offscreenonly,13manualskips retained; no100format/physicalHDR/NVIDIAcompletionclaim.

2026-10-12 manual36MPHDRareaLOD: actualMetalAppleM4Max1passed/currentrepeat.6000square/576MBauthoredperiodicfloatinput->1000square/16MB; all1millionoutputRGBAfloats exactly[-2,8,.5,.375], excludingtransparentRGB.64x64SDR/HDRGPUplanesexactagainstauthoredsameaspectreference; outputroot16MBthen0. TwoCPUdownsampletimes0.621642/1.338911s uncontrolledload, notbenchmark. SourceCPUuntracked; GPUreadback64x64 not1millionpixels; nodecoder/physicalHDR/all100claim. Evidence: research/metal-large-hdr-lod-current-2026-10-12.json and bothlogs.

2026-10-12 productionLODqueue stale submission fix: obsoleteGenerationToken now refusesInterrupted beforethreadcreation and rechecksunderqueue lock before revision/completed/pending mutation. Regressionfailedbefore; afterchecks currentready8x8level/1024bytecredit/revision survive stale request, originalresultconsumable/final0. Worker5passed/1ignored; fullviewer218passed/62ignored; actualMetal delayedoldswapnavigation1passed/onlynewHDRsource/finalcredits0. Evidence: research/lod-stale-submit-preserves-current-2026-10-12.json plusbefore/after/fullviewer/Metal logs andsourcehash. HeadlessAPI/actualGPU only; nolivewindow/physicalHDR oroverallperformanceclaim.

2026-10-12 productionmodelqueue stale submit fix: obsolete token refusesInterrupted beforethreadcreation and rechecksunderqueue lock before revision/slot mutation. ActualMetal regressionfailedbefore atstaleacceptance withcurrentpreparedgeometry; aftertestcurrentrevision/72byteGPUcredit retained, staleinitialstartsnone. All4manualmodelworkertests passed(including1milliontriangle1920x1080SDR/HDR/swap/1000coalescedrequests/sourceleases); fullviewer218passed/62ignored. Driverrequiresnewstalesubmitmarker, currentcombinedMetal135passed/13ignored/allsuitesverified. Evidence: research/model-stale-submit-preserves-current-2026-10-12.json and model-stale-submit-metal-gate-2026-10-12 report/logs. HeadlessMetal only; nofull100/physicalHDR/NVIDIAclaim.

2026-10-12 AI largesterror originaloracle/trace: currentpageisolatednativewhite at141,36/149,56; freshoriginalPoppler sameICCclean16x/64x crops partiallypainted.256x emittedBogusmemoryallocationsize despiteexit0, explicitlyexcluded. Currentnative3pagereplaypassed/finalcredits0/all6outputsbyteidentical. Trace141,36 event2670 coverage96/255 butsource.sample(center)=None, renderercontinues/skipswholepixel; covered32xgrid384/1024validCMYKpoints, mean[.344236,.808281,.938362,.211643]. Concretepartialshadingboundarysampleloss localized; coverage-awareintegration fixpending, noacceptancerelaxation/productionAIclaim. Evidence: research/ai-largest-errors-original-crops-2026-10-12.json and allcrops/trace/hash/report. Foregroundqueue auditfoundfinish_foreground_flight clearsdecodeflight afterexecute_load; noanalogousstalesubmitfix needed there.

2026-10-12 latest unlock reply: actual Apple M4 Max Metal HDR resize probe configured RGBA16Float but returned Occluded after10.01545525seconds, exit1, zero presented frames. No live resize or physical HDR qualification. Evidence: research/metal-unlock-retry-current-2026-10-12.json.

2026-10-12 native AI covered-mesh integration remains opt-in. Precisely diagnosed VectorApple triangle50569 at pixel129,227: source denominator0 with repeated vertices; excluded exactly zero-area source geometry before device transform, retaining nonzero thin triangles.109 interpreter and138 renderer tests passed (10 ignored). Fresh three-page replay: both one.ai pages complete, VectorApple now progresses to candidate limit256 at pixel138,34 and refuses WorkLimit; no capture failure/deadline and final managed credit0 on all pages. Next work: reduce conservative bbox candidates using actual triangle/pixel intersection while preserving bounded admission and source ownership, then verify image parity. Evidence: research/ai-covered-fragment-sampling-2026-10-12/summary.json.

2026-10-12 native mesh rectangle query now filters disjoint bbox candidates using five device separating axes and fixed scratch; exact-touch boundaries retained, no geometric epsilon.110 interpreter and138 renderer tests pass (10 ignored). Regression checks bbox false positive, boundary touch, just-disjoint and thin triangle plus query callback exclusion. Real three-page replay retains finalcredit0/no capture failure/deadline; both one.ai pages complete; VectorApple still refuses at256 candidates on pixel138,34. This reduces avoidable query work but does not qualify AI or prove numerical exact predicates. Next: measure actual dense-pixel candidate count and design bounded processing. Evidence: research/ai-covered-fragment-sampling-2026-10-12/sat-summary.json.

2026-10-12 bounded allocation-free density diagnostic measures327 actual triangle/rectangle intersections at VectorApple pixel138,34 (query100000work, Ok327). Increased pre-admitted per-pixel candidate bound256->512;138 renderer tests pass (10 ignored). Fresh replay passes former limit pixel and now refuses fragment phase at138,39 with263 candidates. Allthreepages final managedcredit0/no capture failure/deadline; both one.ai pages complete, VectorApple incomplete. Integration remains opt-in. Next: enable polygon refusal trace to distinguish fragment work exhaustion from geometry failure; do not increase limits without evidence. Evidence: research/ai-covered-fragment-sampling-2026-10-12/density-and-512-summary.json.

2026-10-12 native selected-fragment refusal at VectorApple138,39 diagnosed as work exhaustion (178left/request266). Avoided region-wide owner scans for exactly zero-area strips while preserving winding updates and cancellation.139 renderer tests pass (10 ignored); new300 coincident-contour regression yields area0.5,owner299,exactly3 positive-width owner selections. Real AI still reaches20M work limit at same pixel; all3pages finalcredit0/no capture failure/deadline; VectorApple incomplete. Next work must reduce nonzero-strip ownership cost or subdivide dense pixel with aggregate bounded work; current opt-in scope unchanged. Evidence: research/ai-covered-fragment-sampling-2026-10-12/zero-strip-summary.json.

2026-10-12 polygon pixel work now excludes wholly-right row edges by binary search over existing min-X sorted storage, no new allocation. Left edges retained for initial winding.140 renderer tests passed (10 ignored);300 disjoint-right contours regression preserves area1 with5000 work and all-right empty pixel0. Real VectorApple remains incomplete at138,39 with263triangles:20M work exhausted (215left/request265). Both one.ai pages complete; allpages final managedcredit0/no capture failure/deadline. Micro-optimizations insufficient: next requires dense ownership sweep/index or spatial subdivision with bounded aggregate work. Evidence: research/ai-covered-fragment-sampling-2026-10-12/right-edge-summary.json.

2026-10-12 structural dense-owner improvement: incremental selector rebuilds at each slab then receives precisely one changed winding region after every crossed edge, including zero-area coincident boundaries. AI uses fixed512bit active owners,64clip flags and source-sorted candidates; highest active rank preserves source ownership. Charged initialization regions+16, update16, same20M pixel work limit.141 tests pass (10 ignored), new regression exactly matches full-scan fragment area/centroid/owner and asserts every incremental winding state plus bounded refusal cases. Real VectorApple passes prior138,39 work exhaustion and now refuses candidate cap512 at147,61; one.ai pages complete; allpages finalcredit0/no capture failure/deadline. Remains opt-in, incomplete AI. Next: bounded spatial processing of >512 source intersections rather than further full scans. Evidence: research/ai-covered-fragment-sampling-2026-10-12/incremental-summary.json.

2026-10-12 dense native mesh boundary pixel subdivision implemented: after512-query overflow, four nonoverlapping half-pixel quarters processed sequentially with512 pre-admitted candidates each,5M preparation+5M fragment work perquarter (same40M aggregate polygon allowance as original20M+20M), no recursive fanout. Kernel accepts validated subrectangles within global unit pixel; areas remain global units, native weighted color/opacity combines without display conversion.142 tests pass (10 ignored), authored600-triangle PDF verifies coverage0.18676470033824444 and originalCMYK within1e-12; analytic geometry uses actual f32 PDF coordinate decode. Real VectorApple147,61 no longer limited solely by root512 list, but quarter with410 candidates exhausts5M fragment work (3970left/request7040). Both one.ai pages complete; allpages finalcredit0/no capture failure/deadline. Still incomplete opt-in AI. Next: use shared remaining polygon work across sequential quarters instead of rigid equal allocations, preserving aggregate budget and cancellation. Evidence: research/ai-covered-fragment-sampling-2026-10-12/subpixel-summary.json.

2026-10-12 native AI quarter processing uses a single40M polygon-work remainder shared by preparation/integration and all quarters, replacing rigid5M per-phase allotments while retaining same aggregate upper bound. Actual work propagated on success/refusal; exhaustion sets0.143 tests pass (10 ignored), new exact-budget regression proves sequential preparation/two-subrect consume existing allowance, one-unit shortage refuses without resetting, cancellation preserves unspent work. Real VectorApple passes previous147,61 and dense147,63-65; now exhausts full40M at147,66 (423candidates,3left/request16). Both one.ai pages complete; allpages finalcredit0/no capture failure/deadline. AI remains opt-in and incomplete. Next: bound row preparation to actual quarter vertical domain to avoid irrelevant row crossings, then reassess dense integration. Evidence: research/ai-covered-fragment-sampling-2026-10-12/shared-budget-summary.json.

2026-10-12 native polygon preparation restricted to actual quarter vertical domain: original endpoints unchanged, inactive edges/crossings excluded; prepared domain stored and out-of-domain integration refused.144 tests pass (10 ignored). Regression with300 contours in other vertical half proves fewer retained edges/lower preparation work and exactly equal fragment areas/centroids/owners to full-row preparation. Real VectorApple still exhausts shared40M at147,66 (423candidates,8left/request16); both one.ai pages complete, allpages finalcredit0/no capture failure/deadline. Still incomplete opt-in AI. Next: deeper bounded proactive spatial partition of dense interior arrangements with same shared polygon work; fixed depth and explicit query-work bound. Evidence: research/ai-covered-fragment-sampling-2026-10-12/domain-summary.json.

2026-10-12 bounded depth2 proactive mesh partition (>256 candidates,512 storage perleaf) completes allthree real native AI diagnostic pages, including VectorApple, with finalcredit0/no capture failure/deadline. Atmost21 query nodes/16leaves/2.1M querywork; same40M shared polygon work.144 tests pass (10 ignored); authored600-triangle control traverses both levels and preserves decoded-geometry area/nativeCMYK. Outputs saved in tests/fixtures/data/metal-recovery/ai-native-mesh-boundary-depth2-20261012. Both one.ai RGBA outputs identical to page-isolation baseline. Strict same-profile VectorApple comparison: differingpixels28008->28168, maxcomponent216->198, MAE1.7074071073->1.7109679569; global parity NOT improved/qualified. Previously white edge141,36 now[214,185,174], near64x independent mean[213.1255,184.0791,173.9282];149,56 now[231,213,209], near64x mean[229.8359,211.0508,207.0173]. Ordinary1x Poppler edge colors differ from area averages. Boundary integration remains opt-in; further color/coverage/nonlinear qualification and production integration required, full100-format/HDR/NVIDIA scope staysopen. Evidence: research/ai-covered-fragment-sampling-2026-10-12/partition-depth2-summary.json.

2026-10-12 independent original-profile area oracle investigation: four largest1x discrepancies tested at16x/64x; native RGB nearly matches area means, so1x strict error alone mischaracterizes those edges. Rendered whole interior300x245 ROI at8x and16x with patched-input-direction-only Poppler/originalICC, box-averaged unrounded RGB.16x MAE before0.80061343/after0.72905437 (~8.94% improvement), >10errorpixels2101->1896;8x MAE0.81396804->0.74630577. Oracle8x/16x convergence mean0.19355/max29.34375, so no uniform convergence/parity claim; fractional final row/column excluded. True large green-edge mismatch remains:163,39 native[37,158,101] vs64x mean[192.3203,226.0713,210.2498];161,47 native[38,158,102] vs64x[193.5601,226.6589,211.1538]. Next trace green edge source/coverage, especially center-inside partially covered mesh pixels. Native AI still diagnostic opt-in; full goal remains open. Evidence: research/ai-covered-fragment-sampling-2026-10-12/area-oracle-investigation-summary.json and independentPPMs/commands/hashes.

2026-10-12 green edge163,39 native trace: shadingevent1023 coverage62/255 with center sample, followed by repeated native-solid events eachcoverage62. Final RGBA[0, 141, 75, 218]; replacing alpha with single62coverage predictswhiteRGB[193, 227, 211], near independent64x[192.3203,226.0713,210.2498]. Strong hypothesis: prematurely averaged per-paint coverage loses repeated-edge spatial correlation and accumulates alpha; requires isolated repeated opaque-contour PDF regression before attributing all error. All3trace replay pages complete/finalcredit0/no capture failure/deadline. Secondary center-inside mesh area reproducer saved as unexecuted research text, not passing suite or implemented fix. Next: authored repeated-contour control and native spatial composition of correlated coverage. Evidence: research/ai-covered-fragment-sampling-2026-10-12/green-edge-diagnosis.json.

2026-10-12 correlated-edge mechanism reproduced with two authored opaque nativeRGB PDFs. Seventeen paints of same quarterpixel:1x scalar alpha0.99264836095 vs exact0.25. Two disjoint quarterpixel green/blue paints:1x alpha0.4389696271 withcomponents[0,0.4282511211,0.5717488789] vs exactalpha0.5/components[0,0.5,0.5]. Native spatial16x/64x controls exact within1e-12; independent Poppler16x/64x whiteRGB means exactly[191.25,255,191.25] and[127.5,191.25,191.25].144 ordinary tests pass,11ignored; new explicit diagnostic1pass documents currentincorrect1x behavior and must be converted to a correctness regression after fix, not counted as rendererqualification. Mechanism requires composition on spatial fragments before averaging; alpha clamps or wholepage supersampling do not satisfy full intended engine/performance scope. Next implement bounded native spatial composition preserving group/clip/blend/color semantics. Evidence: research/ai-covered-fragment-sampling-2026-10-12/correlated-edge-control-summary.json.

2026-10-12 native spatial composition primitive implemented in vendor/hayro/src/native_spatial.rs: partition whole pixel by path/clip regions, compose ordered constant paints percell, then average original native premultiplied components. No supersampling or displaycolor roundtrip. Explicit polygon/paint work budgets, pre-admitted region/kernel storage, limits4095regions/4096paints/4096requirements perpaint; checked cancellation/refusal returns no partial result.146 ordinary tests pass (11 ignored); explicit authoredPDF control1passes: repeated quarter alpha0.25/nativegreen; disjointquartersalpha0.5/components[0,0.5,0.5], matching independently rendered controls. Additional regressions cover translucent repetition, overlapping order, CMYKclip, every observed cancellation checkpoint, every admission failure, both work budgets and finalcredit0. Scope constant paints already in shared blending coordinates; full groups/masks/rasters/shadings and production viewer not yet routed, no realVectorApple fix claimed. Next extend spatial state across transparency group lifecycle and native shading/raster sampling while retaining original color-space conversion and memory/work ownership. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-primitive-summary.json.

2026-10-12 spatial native group lifecycle implemented via compose_grouped_polygon_pixel: shared geometry cells evaluate existing admitted native PaintFrame child/exit operations before averaging, retaining opacity/isolation/knockout/sourceorder and supported blend algebra in one caller-verified blending space. Bounds32groupdepth/4096commands/4095regions/4096requirements perpaint; explicit polygon/command budgets and admitted stack/frame storage.150 ordinary tests pass (11 ignored), retained manualedgecontrol1passes. New authored PDF with nested forms: normal group alpha0.25/components[0,0.5,0.5]; knockoutgroupalpha0.125/components[0,0,1], no increased resolution. Additional regressions isolate/nonisolate Multiply on opaque red backdrop, translucent repeats, nestedgroup exits, every observed cancellation/admission refusal, command budget and malformed exits; final managedcredit0. Full VectorApple routing, masks, shading/raster spatial sampling, cross-space groups and production viewer remain unintegrated; no realAI green-edge correction claim. Next connect percell native sampled paints and masks through interpreter event router without losing group coordinate-space admission. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-groups-summary.json.

2026-10-12 spatial native sampled-paint foundation added: convex fragment centroid passed to bounded/cancellable native source callback; original native sample enters percell PaintFrame/group lifecycle then premultiplied averaging. Winding snapshot pre-admitted; explicit sampler cost1..1M charged before invocation; source refusal/absence differentiated; cancellation rechecked afterward.153 tests pass (11 ignored), retained manualedgecontrol1passes. New affineRGB triangular gradient with17repeats/groupopacity matches analytic area/color; actual native Type4CMYK PDF source givesalpha0.5/C=M=f64(f32(1/3)),Y=K0 without RGB roundtrip. Refusal/absence/budget-before-call/post-call cancellation/wrongmodel controls pass. Exact centroid claim limited to affine final premultiplied fields; nonlinear functions/blend/varying-alpha products need further integration and all source boundaries must enter geometry partition. Full VectorApple/masks/raster/crossspace/production routing remain open. Next connect native raster texel boundaries and softmask source state, then scene router/realAI qualification. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-sampled-summary.json.

2026-10-12 spatial mask state added: SetMask/ClearMask samples native mask percell before area averaging with precharged work and post-source cancellation; Begin captures parent mask for single group-exit application and clears child mask; End restores parent mask. Constant/sampled paints use native opacity-mask application.157 ordinary tests pass (11 ignored); retained explicit edgecontrol1passes. Regressions prove17opaque paints under parent mask do not accumulate maskalpha, child clear cannot erase capturedparent, following paint seesrestoredmask, affine triangular mask integratesanalytically, invalid masks/refusal/cancellation/every observed admissionfailure returnnocompletepixel/finalcredit0. NativePDF alpha-mask control retains actualsourcealpha0.5 and originalTRsquare0.25 (no lookup quantization), maskedquartergroupalpha0.0625. Full maskscene/luminosity/crossspace/nonlinear/VectorApple routing and production remain unqualified. Next native rastertexel geometry and source sampling, then full interpreter scene integration and realAI validation. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-mask-summary.json.

2026-10-12 user unlock confirmation retry: existing actual Metal HDR surface probe on Apple M4 Max, requested5s and resize. HDR configuration accepted, no validation error, but surface remained Occluded for10.009418s;0frames, exit1. Resize/presentation and physicalHDR remain unqualified. Evidence: research/metal-unlocked-confirmation-2026-10-12.json.

2026-10-12 native spatial nearest-neighbor raster geometry added: transformed image footprint and inverse-bounds-selected texel boundaries, original component storage retained externally. Pre-admitted path storage, explicit work/cancellation,4095region limit; dense/singular queries refuse before allocation.159 ordinary tests pass,11ignored. Single green(alpha0.5)/blue(alpha1) halves integratealpha0.75;17repeated paints match pertexel analytic alpha/colors rather than repeated averaged paint. Cancellation/work/admission controls release finalcredit0. Actual PDF raster source bridge, bilinear/nonlinear sampling, complete AI routing and physicalHDR remain unqualified. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-raster-summary.json.

2026-10-12 actual PDF16bit RGB raster source connected to spatial diagnostic control: captured2x1Image XObject samples[1,32768,65534,65535,12345,42]/65535 retained as sourcef32; actual captured affine transform and every clip enter texel geometry. Native sampler original components compose percell, single and17opaque repeats match exact sourcefloat mean within1e-12,alpha1.160 ordinary tests pass,11ignored. Full scene router, bilinear/nonlinear sampling and distinct raster-mask grid geometry remain unqualified. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-raster-pdf-summary.json.

2026-10-12 actual decoded16bit PDF raster spatial route failure audit: all244observed cancellation checkpoints and all15observed managed admissions injected individually, each returns no partial pixel and final managedcredit0. Focused real-source control passes. This audits geometry/source/compositor scratch; source capture occurs beforehand and remains outside these counters. Full AI scene/production integration still open. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-raster-pdf-refusal-summary.json.

2026-10-12 spatial raster/mask geometry supports distinct source resolutions mapped to same footprint under shared aggregate geometry-work allowance. Retains independent admitted color/mask paths; combined4095region bound. Synthetic2x1green/blue with4x1alpha[0,1,0.5,1] givesalpha0.625,RGB[0,0.4,0.6] before area averaging; exact aggregatework39refuses/40accepts.161 ordinary tests pass,11ignored. Actual PDF distinct-alpha-grid bridge, interpolation and full AI scene integration remain open. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-raster-mask-grid-summary.json.

2026-10-12 actual PDF RGB16image2x1/Gray16SMask4x1 native spatial control passes single/17repeats analytic original-color/alpha within1e-12. Found and fixed diagnostic Recorder capture refusal of distinct mask pixel count: validates nonzero dimensions/data length and admits retained mask bytes beyond preallocated same-size alpha allowance.162ordinarytests pass,11ignored. Compatibility sampler still refuses distinct alpha dimensions; native diagnostic source accepts. Full production AI routing/interpolation remain unqualified. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-pdf-alpha-grid-summary.json.

2026-10-12 diagnostic compatibility raster sampler no longer refuses distinct alpha dimensions. Maps original source coordinates to independent mask dimensions; validates mask size and uses its own interpolation flag; retains premultiplied color filtering. Actual PDF2x1/4x1mask control verifies allfour native sourcepositions including transparent/half/opaque samples.162ordinarytests pass,11ignored. Distinct-grid bilinear integration and full AI production routing remain unqualified. Evidence: research/ai-covered-fragment-sampling-2026-10-12/compatibility-alpha-grid-summary.json.

2026-10-12 retained Recorder scene-to-spatial router started: native constant global fill paths/clips, source order, blend/alpha-source state and nested group lifecycle now translate automatically rather than handwritten test command lists. Same-coordinate explicit groups verified; unsupported mask/raster/shading/crossspace/paintAIS/transform refuse wholepixel. Region/source/requirements/command scratch pre-admitted,4095regions/4096events,40Mpolygon/4Mcommandwork. Actual authored normal/knockout PDFs match explicit spatial controls exactly.162ordinarytests pass,11ignored. Raster/shading/mask scene routing and production remain open. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-scene-router-summary.json.

2026-10-12 automatic native spatial scene router supports nearest-neighbor raster events alongside constant fills/groups. Retains captured16bit original samples; partitions color and optional distinct alpha grid plus captured clips, invokes native sampler percell, verifies shared blending coordinates. Bounded128rasterevents/4095regions/4096sceneevents/100kgeometrywork perraster; route metadata pre-admitted and scratch reserve fallible. Actual PDF2x1RGB16/4x1Gray16mask scene route matches analytic alpha/originalRGB; constant normal/knockout controls retained.162ordinarytests pass,11ignored. Interpolation/stencils/shadings/maskevents/crossspace and full production AI remain open. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-raster-scene-router-summary.json.

2026-10-12 diagnostic scene router includes native shading mesh boundaries, original native centroid callbacks, source paint path/clips and group/source order. Actual Type4CMYK affine triangle PDF scene route equals prior explicit spatial control exactly,alpha0.5. Bounded32mesh-events/4095triangles permesh/4095totalregions/100kquerywork each, conservatively pre-admitted geometry storage.162ordinarytests pass,11ignored. Exactcentroid scope affine finalpremultipliedfields only; nonlinear sampling/blends unqualified, conservative meshstorage needsoptimization; full VectorApple/maskevent/production integration remain open. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-mesh-scene-router-summary.json.

2026-10-12 native spatial mesh scene scratch now admits exact candidate geometry: count pass with100kquerywork, pre-admit candidate paths/4PathEls each, materialize second100kquerypass, verify identical count. Removes4095path reserve for sparse meshes; retained admission owners drop after geometry. Actual singletriangleCMYK PDF route maxsingleadmission672bytes, exactcolor/alpha retained.162ordinarytests pass,11ignored; focused control passes after drop-order adjustment. Totalquerybound200kpermesh, production/full AI still unqualified. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-mesh-exact-admission-summary.json.

2026-10-12 real VectorApple spatial scene pixel diagnostic added and run at163,39. Actual source capture completes:2755events/13shadings/6masks, no deadline, peakmanaged202254128bytes/final0. New scene route returnsNone; native spatial completion FALSE. Ignored diagnostic1pass measures observation/cleanup only, not green-edge correction or AI qualification. Masks and unsupported source boundaries still require routing before real scene can complete. Evidence: research/ai-covered-fragment-sampling-2026-10-12/real-spatial-problem-pixel-summary.json.

2026-10-12 allocation-free native spatial scene structural preflight added: reports first unsupported event, cancellation, invalid group lifecycle/opacity, non-global/stroke/compatibility paint, missing/interpolated native raster, stencil or maskscene before temporary geometry admission. Authored unsupported mask regression forbids all scratch admission.163ordinarytests pass,12ignored. Real VectorApple capture completes, preflight specifically refusesevent13 mask_scene_not_routed, routeadmissions0/finalmanaged0, spatialpixelcompletionFALSE. This avoids wasted preparation; actual mask implementation and AI qualification still required. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-preflight-summary.json.

2026-10-12 native group/mask command evaluation shared between integrated pixel and new point-source mode. Point mode evaluates source state before area averaging, with bounded geometry/commandwork, cancellation and admitted winding scratch; mask callers must include all mask boundaries in parent partition. Quarter-domain mask regression yields insidealpha0.25/outside0 while pixelaverage0.0625, preventing use of already averaged alpha as spatial source.164ordinarytests pass,12ignored. Actual mask scene preparation/event routing and real AI/production qualification remain open. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-point-mask-summary.json.

2026-10-12 automatic alpha-mask scene routing implemented for constant native fill/group sources: parent partition includes mask paths/clips, mask sampled by shared point evaluator before area averaging in declared mask blending space, original transfer function applied without lookup quantization, SetMask participates in existing group capture/restore semantics. Actual authored PDF alpha0.5/TRsquare produces routedalpha0.25 at24,0 unoccluded blue region.164ordinarytests pass,12ignored. Luminosity/complex/nested mask sources still refuse early; per-sample mask preparation needs performance refinement; full AI/production remain unqualified. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-alpha-mask-router-summary.json.

2026-10-12 native spatial constant-source device luminosity mask routing added: original native BC backdrop retained, DeviceGray/RGB/CMYK luminance approximation reused, source/group point evaluation precedes transfer and parent area averaging. Three authoredPDFmodels/twopositions each match existing original-component controls within1e-14.164ordinarytests pass,12ignored; calibratedPCS rejects early. RealVectorApple now specifically reports event13 complex_mask_source_not_routed,0routeadmissions/finalcredit0/spatialcompletionFALSE. Complex mask geometry/source routing and productionAI still required. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-device-luminosity-router-summary.json.

2026-10-12 native spatial mask source routing now accepts nearest-neighbor raster events. Parentpartition retains raster color/independentalpha grids and sourceclips, pre-admitted metadata/geometry,128maskrastercap. Actual Gray16luminositymask PDF straight/curvedclip interior points on both sides match original-component controls within1e-14; curvedboundaryintegral notqualified by interiorchecks.164ordinarytests pass,12ignored. RealVectorApple still event13complex_mask_source_not_routed,0routeadmissions/finalcredit0/completedFALSE; shading/nestedmask/calibrated/interpolation sources and productionAI remainopen. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-raster-mask-scene-summary.json.

2026-10-12 actual VectorApple mask inventory identifies event13luminositymask with1shading,0nested/rasters; sixmask inventory retained. Native mesh-mask source routing implemented with exact pre-admitted candidate triangle paths in parent geometry and original native point-source evaluation. Authored GrayType4 affine luminositymask matchesalpha0.5*f64(f32(1/3)),nativegreen.165ordinarytests pass,12ignored. RealVectorApple preflight advances fromevent13 toevent1218raster_alpha_is_shape_not_routed,0routeadmissions/final0/completionFALSE; no realpixelcorrectness claim. Axial/radial/nonlinear integration, AIS and production still required. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-mesh-mask-scene-summary.json.

2026-10-12 native sampledpaint carries AlphaIsShape into existing admitted frame algebra; raster scene route preserves AIS state. New knockout regression: halfalpha blue overopaquegreen AISfalse alpha0.5/blue vsAIStrue alpha1/green0.5blue0.5.166ordinarytests pass,12ignored. RealVectorApple structuralpreflight nowOK across2755events;10routeadmissions thenrefusal, no deadline/finalmanaged0/spatialpixelcompletionFALSE. Next locate preparation source/geometry refusal; productionAI still unqualified. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-raster-ais-summary.json.

2026-10-12 spatial scene failure-stage tracing identifies firstactual preparationrefusal rasterevent3coordinate mismatch. Raster samplers now reuse existing native frame component conversion before destination-space composition, source model retained explicitly, postconversion cancellation checked; no display8bitroundtrip. Actual RGB16PDF→capturedDeviceGraytarget matches pertexel nativefloat conversion thenmean.166ordinarytests pass,12ignored. RealVectorApple advances to mesh_source_geometry event1017,18routeadmissions/final0/no deadline/completionFALSE. Existing conversionpairs only; RGB→CMYK needs originalBG/UCRpolicy, mesh/groupcrossspace and productionAI remainopen. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-raster-color-routing-summary.json.

2026-10-12 native mesh spatial callback now retains source model and applies existing verified native component conversion before destination blending, postconversion cancellation; actual CMYKType4→capturedDeviceGray PDF control passes.166ordinarytests pass,12ignored. Source/geometry tracing resolves realVectorApple1017: sourceblendingmodelNone,destinationCMYK,visit_mesh_triangles Unsupported; stage mesh_query,preflightOK,18routeadmissions/final0/completedFALSE. Requires original nonmesh/sourcekind and tint handling; nonlinear conversion integration/production remainunqualified. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-mesh-color-routing-summary.json.

2026-10-12 native axial boundary API exposes originalparameter t0/t1 infinite lines transformed from source normals, preserving shear/reflection geometry without colorconversion/allocation. Focused nativeidentity/shear/reflection/radialcontrol1passes;Hayro166ordinarypass/12ignored. RealVectorApple1017 now positively identified axial=true/sourceDirectBlendingModelNone; meshqueryUnsupported expected,final0/completionFALSE. Next original axial boundaries into pixelpartition and alternate tint/function coordinates; scene routing and realpixel correctness remain incomplete. Evidence: research/ai-covered-fragment-sampling-2026-10-12/axial-boundary-summary.json.

2026-10-12 native axial scene boundaries routed as admitted rect-clipped infinite halfplane paths atsource t0/t1; sourceopacity/function and original alternate tint coordinates resolve before destination blending. Exact authored Grayaxial PDF0.25..0.75/noextend givesalpha0.5/gray0.5.167ordinarytests pass,12ignored. RealVectorApple all13shadings now prepare including1017/2742alternate-space axial; refusal advances tospatial_composition,32routeadmissions/final0/completionFALSE. Nonlinear finalfield integration, axialmaskboundaries and full productionAI remainunqualified. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-axial-scene-summary.json.

2026-10-12 spatial composition refusal traced to4097row-activeedges despite34.4Mpolygonwork remaining. Pixel-aware row preparation now excludes wholly-right original edges before4096cap, retains allleftwinding edges, recordsrightdomain and refuses out-of-domainreuse.3000irrelevantrightcontour regression:ordinaryrowrefuses,pixelrowexactarea1,limitsunchanged.168ordinarytests pass,12ignored. RealVectorApple rowgeometry nowprepares and reachesfirstcell163.03813986627904,39.00199035512648; maskchild rejects explicitgroupcoordinates atgroup_spaces,50routeadmissions/final0/completionFALSE. Next real mask group color-space lifecycle; no finalAIqualification. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-pixel-row-culling-summary.json.

### Metal HDR retry after unlock reply (2026-10-12)

Actual Apple M4 Max Metal probe accepted HDR configuration with no validation error, but the surface remained Occluded for 10.001019041 seconds and presented zero frames (exit 1). Physical HDR presentation and resize qualification remain open. Evidence: `research/metal-unlock-reply-retry-2026-10-12.json`.

### Native spatial group source spaces (2026-10-12)

Retained constant, raster and shading sources now convert in the active group space; isolated groups enter their declared space and convert on exit. Four authored Gray-child/RGB-parent pixels match native replay exactly for both isolation modes. Ordinary suite: 168 passed, 12 ignored. Real VectorApple problem pixel advances beyond group-space refusal but still refuses during spatial composition: completed=false, deadline=false, 4996 admissions, final credit=0. No production AI or complete-file qualification is claimed. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-group-source-space-summary.json`.

### Spatial command budget diagnosis (2026-10-12)

Opt-in tracing identifies the real VectorApple refusal: command 940 requires 100001 units with 65132 left. SetMask currently charges a fixed 100000 per callback, independently of child work. Limits remain unchanged. Ordinary tests: 168 passed, 12 ignored; real pixel completed=false, final credit=0. Next work is shared actual mask work accounting and prepared child reuse, not arbitrary limit escalation. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-command-budget-summary.json`.

### Shared mask work interface (2026-10-12)

Added budgeted mask callbacks that debit the parent remaining work. Regression checks two callbacks share one budget, exact 18/19-unit refusal/success threshold, and callback budget growth refusal. Suite: 169 passed, 12 ignored. This interface is not yet wired into real scene mask replay; fixed 100000 charging and child preparation remain to replace. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-shared-mask-budget-summary.json`.

### Shared command budget wired through real masks (2026-10-12)

Real scene mask callbacks now debit the same 4M command budget as the parent; no fixed 100000 callback charge and no child command-budget reset. All 169 ordinary tests pass, 12 ignored. Real problem pixel reaches y=39.02981068999465 before actual command exhaustion (command 2267, remaining 0); completed=false, deadline=false, 562741 admissions, final credit=0. Geometry budgets are still independent and mask scenes are rebuilt per point. Prepared child reuse and further spatial work reduction remain necessary. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-shared-mask-route-summary.json`.

### Pixel-disjoint spatial paint command culling (2026-10-12)

Cull native color/raster/shading paint commands if any required fill control box is strictly disjoint from the pixel. Preserve boundary contact, all region geometry and group/mask state. Suite: 169 passed, 12 ignored. Real VectorApple reaches y=39.06530243988036 before refusal, compared with 39.02981068999465 previously; no limit increase. Diagnostic 12.17s, completed=false, deadline=false, final credit=0. Prepared mask reuse, common geometry accounting and full pixel/file qualification remain open. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-command-culling-summary.json`.

### Empty spatial group pruning (2026-10-12)

In-place bounded-stack pruning removes groups without retained paint commands after pixel culling. Painted nesting, state outside removed groups, and all boundary contacts are retained. Regression compares isolated/nonisolated and knockout output before/after pruning, plus invalid nesting and cancellation refusal. Suite: 170 passed, 12 ignored. Real pixel advances to y=39.13641970825194, with 657771 admissions versus previous 1106457; still completed=false, deadline=false, final credit=0. Diagnostic runtime 12.53s is not a speedup claim. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-empty-group-summary.json`.

### Lazy per-fragment mask evaluation (2026-10-12)

Group state retains mask handles. Resolve only on actual paint/group application and cache once within the spatial fragment, in admitted command-sized storage. Cache clears before another fragment; original native samples/transfer and shared command budget remain. Regression proves replaced mask is never called, two paints reuse one sample, and another point resamples. Suite: 171 passed, 12 ignored. Real problem pixel reaches y=39.22393587393761, still completed=false/deadline=false; 541078 admissions, final credit=0. Child scenes are still prepared per sampled point. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-lazy-mask-summary.json`.

### Canonical spatial regions (2026-10-12)

Bounded admitted canonicalization hashes each repeated path object once, checks full structural equality plus fill rule before merging, remaps paint intersections and retains source order. If optional 4M canonicalization work runs out before mutation, retain original regions. Regression protects cloned equality, distinct fills, adjacent geometry and cancellation. Suite: 172 passed, 12 ignored. Real region count 2528 -> 697; problem pixel still completed=false/deadline=false, final credit=0. 9.10s single diagnostic is not a general speed claim. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-region-dedup-summary.json`.

### Source sample budget refusal identified (2026-10-12)

Opt-in source sampling diagnostics identify the current real pixel stop: remaining=126, fixed sample_cost=128. 172 ordinary tests pass, 12 ignored; real completed=false, deadline=false and final credit=0. Limits unchanged. The router uses fixed 128 for both native raster and shading samples, independently of source operations; bounded source work accounting remains to examine before changing that estimate. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-sample-budget-summary.json`.

### External native shading function work budget (2026-10-12)

ShadingFunction now exposes eval_native_with_budget using the existing actual function-operation counter. Legacy eval_native_bounded retains its 4096-unit wrapper. Regression verifies shared work across successive samples and unchanged output on exhausted/zero/cancelled evaluation. Native interpreter suite: 112 passed. Next: propagate through native source geometry/BVH queries, tint resolution and scene callbacks; fixed router 128 is still present. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/shading-external-budget-summary.json`.

### Native shading source shared work API (2026-10-12)

Added sample_with_budget: source transform, BVH nodes, candidate triangle checks, mesh component interpolation and original function operations debit caller work. Ordinary sample preserves legacy function limit and values. Regression checks exact axial components/opacity, successive-sample exhaustion, zero budget and cancellation. Native suite: 113 passed; replay suite: 172 passed, 12 ignored. Scene callback integration, native tint budget propagation and raster accounting remain open; fixed router source cost 128 is not removed yet. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/native-source-external-budget-summary.json`.

### Actual shading and tint work wired into scene replay (2026-10-12)

Native spatial source callbacks now receive caller remaining work. Shading callbacks debit sample_with_budget and native_sample_coordinates_with_budget, including original Separation/DeviceN functions. Raster callbacks retain fixed 128 pending explicit raster accounting. Shared limit stays 4M. Native suite: 113 passed; replay suite: 172 passed, 12 ignored. Real pixel remains completed=false/deadline=false, last point y=39.223985146514075; final credit=0. No full AI qualification. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-shading-actual-budget-summary.json`.

### Shared native raster sample work (2026-10-12)

Replaced fixed raster callback charge 128 with setup/component/alpha-fetch debits and shared original color/tint-function work. Legacy sampler retains prior color function bounds. Regression checks exact legacy color and alpha including interpolation, two successive samples sharing budget, and zero refusal. Suite: 172 passed, 12 ignored. Real problem pixel advances to y=39.37979221038817 with unchanged common limit, still completed=false/deadline=false; final credit=0. Mask preparation reuse and geometry-work sharing remain open. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-raster-actual-budget-summary.json`.

### Shared native source refusal hardening (2026-10-12)

Validate shared source budget monotonicity and cancellation even when a source returns no color at the point. Regression rejects empty samples that mint work or hide cancellation, and tests every observed budgeted raster cancellation checkpoint. Suite: 173 passed, 12 ignored. This verifies refusal semantics, not full AI rendering; real diagnostic was not repeated for this narrow safety fix. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-native-budget-refusal-summary.json`.

### Prepared constant native paint reuse (2026-10-12)

Admitted per-command cache retains PreparedPaint across spatial fragments with fixed enclosing group-space lifecycle. Each use still validates target space; sampled raster/shading values are excluded. Suite: 173 passed, 12 ignored, including native group/color and refusal controls. Real problem pixel stops at the same location, completed=false/deadline=false, final credit=0. Single diagnostic took 8.10s; no generalized speedup claim. Mask scene preparation reuse remains open. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-retained-paint-cache-summary.json`.

### Parent raster mask grid reuse (2026-10-12)

Admitted parent raster-mask color/alpha grids are now shared with child point replay, keyed by exact source Recorder identity and event index within one pixel scope. Arc ownership preserves geometry credit until last owner drops; metadata includes shared-owner storage. 173 tests pass, 12 ignored, including actual raster/clip luminosity controls. Real problem pixel remains incomplete with unchanged 849465 admissions and final credit=0; no improvement claim for this pixel. Mesh mask preparation still repeats and is next. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-mask-raster-geometry-reuse-summary.json`.

### Parent mesh mask geometry reuse (2026-10-12)

Parent-prepared mesh mask paths now pass to child point replay by source Recorder identity and event index, scoped to the same pixel. Shared geometry owns its admission credit; metadata includes Arc storage. 173 tests pass, 12 ignored, including authored mesh luminosity mask output. Real pixel still refuses at the same position, with unchanged 849465 admissions and final credit=0. Single 7.62s run is not a general performance claim. Region/command preparation and native frame creation still repeat. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-mask-mesh-geometry-reuse-summary.json`.

### Opaque normal paint coalescing (2026-10-12)

Coalesce only consecutive identical original opaque Normal paints with equal remapped region intersections, active blending-coordinate identity and no active mask. Regression preserves fractional coverage and masked alpha accumulation. Suite: 174 passed, 12 ignored. Real problem pixel has 344 commands before and after this optimization; remains incomplete, final credit=0. No real-pixel improvement claim. Next inspect unused shading partition geometry after command culling. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-opaque-coalescing-summary.json`.

### Unused shading partition geometry culling (2026-10-12)

Omit shading interior partition paths when the original fill or any clip has a conservative control box strictly disjoint from the pixel; keep source/clip requirements and boundary contacts. Suite: 174 passed, 12 ignored. Real diagnostic excludes 16 regions (2512 before dedup) but unique count remains 697; completion and refusal point unchanged, final credit=0. No speedup or full AI completion claim. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-unused-mesh-regions-summary.json`.

### Unused mask boundary exclusion (2026-10-12)

Conservatively track pixel-touching paints and mask capture/reset/restore across groups. Exclude original mask paths, raster grids and mesh boundaries only for masks with no possible use. 174 tests pass, 12 ignored, including alpha/luminosity/group controls. Real pixel has zero live masks of six; unique regions fall 697 -> 689. Completion/refusal point unchanged, final credit=0. This narrows current problem to main scene partition/commands rather than mask sampling. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-live-mask-boundaries-summary.json`.

### Dead paint source and clip region exclusion (2026-10-12)

Pixel-disjoint constant/shading/raster paints keep empty source requirement slots and add no source/clip/partition geometry. Empty slots never become paint commands, preserving event/source indexing. Suite: 174 passed, 12 ignored. Real unique region count 689 -> 63 (120 entries before dedup); commands remain 344. Real problem pixel still incomplete, no deadline, final credit=0. Next focus per-fragment command/group work rather than retained region count. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-dead-paint-regions-summary.json`.

### Per-fragment single-paint group guards (2026-10-12)

Prepare bounded group-to-leaf guards. A subtree with exactly one paint skips all frame allocation and command dispatch when that paint geometry excludes the fragment. Preparation and geometry tests debit the existing shared budget. Updated exact exhaustion controls to include preparation. Suite: 174 passed, 12 ignored. Real admissions 849465 -> 512641; pixel remains incomplete/no deadline, final credit=0. Group shape, mask capture/restore and source order controls remain green; no full AI qualification. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-single-paint-group-guards-summary.json`.

### Shared-coverage subtree guards (2026-10-12)

Extend group guards to multiple paints only when all required region indices match. Equality preparation debits common work. Regression preserves repeated alpha accumulation, distinct regions and zero outside sampling. Suite: 175 passed, 12 ignored. Real refusal point/admissions unchanged (512641), final credit=0; no real speedup or completion claim. Broader common-clip guards and per-fragment dispatch remain next candidates. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-shared-coverage-group-guards-summary.json`.

### Common required-region subtree guards (2026-10-12)

Intersect required region sets over all descendant paints in admitted bounded bitsets. A missing common region skips the complete group even with different source paths. Preparation/lookup consume shared command work; source order, group masks and blending remain unchanged inside coverage. Suite: 175 passed, 12 ignored. Real pixel advances slightly to y=39.3813818647011, still incomplete/no deadline, final credit=0. 6.95s single run is not generalized speed evidence. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-common-clip-group-guards-summary.json`.

### Bounded per-coverage command route cache (2026-10-12)

Cache up to 32 command routes keyed by full fill membership. Build routes from visible paints and their nonempty ancestor groups; source values still sample each point. Admitted storage, key/lookup/construction work and cancellation remain explicit. This replaces previous common-region subtree guards. Regression verifies affine color integration across fragments; suite: 176 passed, 12 ignored. Real admissions 505386, incomplete/no deadline, final credit=0; no generalized performance/completion claim. Next inspect redundant source triangle search versus existing partition ownership. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-fragment-route-cache-summary.json`.

### Selected mesh triangle shared-budget API (2026-10-12)

Added sample_mesh_triangle_with_budget for already clipped source owners, preserving legacy selected-triangle API. Setup/component interpolation/original functions debit shared work without another BVH search. Regression verifies legacy components/opacity, two-sample exhaustion, zero and cancellation. Native: 113 passed; replay: 176 passed, 12 ignored. Next connect partition winding to original triangle indices with highest source-owner precedence; current scene callbacks still search the BVH. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/native-selected-triangle-budget-summary.json`.

### Partition-selected native mesh owners wired (2026-10-12)

Retain stable source triangle indices with admitted shared geometry. Map them to canonical NonZero regions, select highest covered original index, and call budgeted selected-triangle evaluation; retain ordinary sampling for background/outside owners. Source fields still sample each point. Regression verifies deduplicated overlapping owner priority; suite: 177 passed, 12 ignored. Real pixel remains incomplete at same location, final credit=0; no real speedup claim. Next narrow route cache keys to command geometry, excluding triangle boundaries that do not affect command routing. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-selected-mesh-owner-summary.json`.

### Command-only route keys and live Metal recheck (2026-10-12)

Route keys now include only command visibility regions (43 of 63 on the real pixel). Mesh ownership still uses full winding and colors sample each fragment. Suite: 177 passed, 12 ignored. Real replay advances to (163.75457640677956, 39.41392965393066), still refuses at shared command budget; final credit=0. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-command-only-route-key-summary.json`.

Live HDR retry after unlock reply: Apple M4 Max / Metal accepts HDR configuration without validation error, but returns Occluded for 10.015s, zero frames, exit 1. Live presentation remains unqualified. Evidence: `research/metal-unlocked-live-recheck-2026-10-12.json`.

### Reuse certified route coverage during execution (2026-10-12)

Cached routes already contain only visible paints, so execution no longer repeats required-region winding checks or charges for removed checks. Owner lookup, source sampling, masks, groups, cancellation and dispatch still consume shared work. Exact budget exhaustion control adjusted for two removed region checks. Suite: 177 passed, 12 ignored. Real refusal advances to y=39.45667084896487, still shared command-budget exhaustion; final credit=0, no completion or generalized speed claim. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-route-coverage-reuse-summary.json`.

### Ordered mesh owner lookup (2026-10-12)

Prepare descending original source triangle priority with bounded sort work. Validate ordering/region indices against shared work before sampling, then stop owner lookup at the first covered region. Duplicate-region highest-owner control retained; new unsorted-owner control refuses before source callback. Suite: 178 passed, 12 ignored. Real problem pixel refusal/admissions unchanged, final credit=0; no measured improvement or AI completion claim. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-owner-priority-search-summary.json`.

### Actual fragment work counters (2026-10-12)

Opt-in allocation-free counters survive early refusal through RAII: fragments, route hits/misses, dispatch, group creation, native/fixed/mask source sampling. Suite: 178 passed, 12 ignored. Real pixel: 10651 fragments, 10521 route hits, 130 misses, 2844731 dispatches, 133781 groups, 23422 native samples, zero fixed/mask samples. Refusal unchanged, final credit=0. Next target repeated command/group execution rather than mask work or cache misses. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-work-counters-summary.json`.

### Constant route result cache (2026-10-12)

Store a native Pixel in admitted route entries only when every retained source is constant and no mask sampler is present. On reuse, geometry key lookup, cancellation and dispatch work still apply. Groups and blending execute normally before the first cached result. New analytical control verifies 17 repeated opaque quarter paints in a half-opacity group across unrelated partitions yield alpha 0.125; affine sampled-source regression remains green. Suite: 179 passed, 12 ignored. Real pixel: zero constant-result hits and unchanged refusal, final credit=0. Whole-route caching cannot help this pixel because every visible route contains sampled sources; next investigate constant isolated subgroups. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-constant-route-result-summary.json`.

### Immutable admitted group reuse API (2026-10-12)

PaintFrame can apply a borrowed retained child while recomputing parent mask/opacity/AIS/blending. Existing consuming API delegates to the same validated path. Regression compares repeated reuse to fresh children for knockout/non-knockout, AIS, three masks and Normal/Multiply/Screen/HardLight, verifies cancellation poisons the parent and retained child remains unchanged. Suite: 180 passed, 12 ignored. Real diagnostic refusal/counters unchanged, final credit=0. Route cache is not connected yet; next retain only independently constant isolated subgroups keyed by route coverage. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-retained-group-frame-summary.json`.

### Constant isolated subgroup cache connected (2026-10-12)

Route entries retain admitted immutable frames only for isolated subgroups without sampled sources or internal mask callbacks. Nested nonconstant state propagates to ancestors. Cache hits skip internal commands and reapply external mask/opacity/AIS/blend. Cache metadata is admitted before allocation and child credits survive until route eviction/drop. Analytical mixed constant-group/affine-source regression: 16 fragments, 15 group hits, one created group, 50 dispatches; alpha 0.75 and RGB [1/3,1/3,0]. Suite: 181 passed, 12 ignored. Real pixel has zero eligible group hits and unchanged shared-budget refusal, final credit=0; no real improvement claimed. Next inspect nonisolated group dependence on actual parent backdrop before widening reuse. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-constant-group-cache-summary.json`.

### Exact-backdrop nonisolated group cache (2026-10-12)

Extend constant subgroup reuse to nonisolated groups only after exact premultiplied storage/alpha/model and blending-coordinate checks against the actual parent child backdrop (initial for knockout, current otherwise). Comparison debits shared work and cancellation. Mismatched frames release before replacement admission. Regressions: same backdrop yields 15 group hits/one creation across 16 fragments; affine changing backdrop yields zero hits/16 creations and analytic alpha/RGB. Suite: 183 passed, 12 ignored. Real: 7177 group hits, fragments 10651 -> 12467, creations 133781 -> 94591, admissions 588421 -> 440736; refusal advances to y=39.55068671875, still incomplete, final credit=0. No generalized wall-time/full AI qualification claim. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-backdrop-group-cache-summary.json`.

### Exact-state constant paint run cache (2026-10-12)

One-pixel transition stores exact initial/current premultiplied pixels, alpha, coverage, knockout and native space. Restore requires exact input match, shared budget and cancellation; misses leave state unchanged, exhaustion poisons frame. Route cache admits transition storage before allocation, retains only consecutive constant paints, excludes active masks, and preserves source order. Regression: 17 constant paints followed by affine source across 16 fragments gives 15 run hits/48 dispatches versus 288 uncached, analytic alpha/RGB intact. Suite: 185 passed, 12 ignored. Real pixel has zero run hits and unchanged refusal/counters, final credit=0; do not claim real speedup. Next inspect command-type distribution/state barriers before widening spans. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-paint-run-cache-summary.json`.

### Redundant mask clears removed; real spatial pixel completes (2026-10-12)

Command-kind tracing found 2244251 of 2560074 previous dispatches were ClearMask (275 of 344 prepared commands). Route preparation now tracks mask presence across group capture/reset/restore and skips ClearMask only when already absent. Parent restored masks still require explicit clearing; regression reduces 101 clears to one and preserves analytic alpha/RGB. Suite: 186 passed, 12 ignored. Real VectorApple page 0 pixel (163,39) completes within unchanged work limits: 22140 fragments, 698928 dispatches, alpha 0.24414157717689028, CMYK [0.9499999880790699,0.26999829237874273,1,0], no deadline, final credit=0. This is one completed pixel, not full-page correctness/AI support. Next compare pixel output against native-profile oracle and broaden spatial checks. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-redundant-mask-clear-summary.json`.

### Native-profile oracle comparison and neighbor qualification (2026-10-12)

Diagnostic accepts up to 16 explicit pixel coordinates and reports native-profile RGB composited on white; production replay is unchanged. Reused one captured scene for (162,39), (163,39), (164,39), same per-pixel work limits/global deadline. Suite: 186 passed, 12 ignored. White pixel completes/exact [255,255,255]. Edge completes with [192.743897819893,226.42065353350807,210.78666545414288]; independent Poppler with identical CMYK profile gives max channel error 0.537 at 64x and 0.243 at 128x area oracle, whose own max change is 0.294. Interior neighbor still refuses shared work, no deadline; final credit=0. These are bounded pixel results, not full-page support or an error certificate. Next target interior budget consumption and fragment geometry. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-oracle-neighbors-summary.json`.

### Inactive mask state removed from pixel commands (2026-10-12)

Reuse conservative mask liveness to emit ClearMask for unused replacements, preserving override semantics and source indices. Route preparation removes redundant clears. Suite: 186 passed, 12 ignored. Three-pixel repeat retains exact completed native/RGB outputs for white and edge; edge dispatch 698928 -> 543948. Interior advances 26809 -> 27756 fragments, still refuses shared work, final credit=0. All mask dispatch is gone at these pixels. Remaining command volume is predominantly group begin/end; inspect group semantics before further reuse or flattening. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-inactive-mask-state-summary.json`.

### Bounded collision-safe route index (2026-10-12)

Replace per-fragment linear route scan with admitted fixed 64-slot index for 32 routes. Hash work, probes, full-key comparisons and eviction rebuild debit shared budget/cancellation; full key equality prevents hash collisions changing routing. Exact first-route controls include three added setup operations; real limits unchanged. New color/coverage control runs 260 distinct routes with repeated eviction. Suite: 187 passed, 12 ignored. Three-pixel repeat retains exact white/edge outputs; interior advances 27756 -> 33039 fragments, still incomplete, final credit=0. Group property trace identifies nonisolated/non-knockout groups with both neutral and non-neutral opacity/blend/AIS; preserve HardLight/half-opacity boundaries when investigating neutral groups. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-route-index-summary.json`.

### Protected neutral group flattening (2026-10-12)

Mask-free inherited nonisolated/non-knockout groups with Normal, opacity 1, AIS false flatten only under a non-knockout parent. Bounded in-place preparation uses shared work. Regression compares alpha/native colors and preserves isolation, knockout, fractional opacity, AIS, HardLight, masks and knockout-parent boundaries. Suite: 188 passed, 12 ignored. Real removed boundaries=0: the opacity-1 neutral candidate belongs to a knockout parent. White/edge complete with unchanged outputs; interior remains incomplete, final credit=0. Additional preparation consumes budget (33039 -> 33033 fragments), no real speedup claimed. Next investigate fully overwriting later sibling paints within knockout groups without removing required object boundaries. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-neutral-groups-summary.json`.

### Certified opaque overwrite prunes hidden objects (2026-10-12)

Mask-free exact coverage routes remove earlier same-frame objects only after opaque Normal paint establishes a complete state independent of prior backdrop. Group certificate propagates only through opacity-1 Normal exit; knockout/nonopaque objects invalidate independence. Required group boundaries remain. Preparation/removed-range traversal debit shared work and cancellation, fixed stack only. New analytical regression preserves knockout shape and protects partial alpha/opacity and HardLight sibling boundaries. Suite: 189 passed, 12 ignored. Real white/edge outputs exactly unchanged; edge dispatch 543948 -> 240838, group creations 192324 -> 89116. Interior advances 33033 -> 33960 fragments but still refuses; native samples 69288 and allocations 744374, final credit=0. Next measure actual source/key/lookup/group budget shares rather than infer from dispatch alone. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-hidden-object-summary.json`.

### Measured budget categories and one-time mask-clear pruning (2026-10-12)

Opt-in category measurement wraps bounded operations and records debits even on refusal. Interior baseline key=1528200, preparation=1294761, source=491866. Move provably redundant mask clears to one bounded in-place scene-command pass, retaining per-route checks for geometry-specific dead groups. Expanded parent-mask regression removes 100 redundant clears while preserving one necessary explicit clear and analytic output. Suite: 189 passed, 12 ignored. Real removes 279 clears before route construction; edge preparation 314988 -> 98741 with exact unchanged native/RGB output. Interior advances 33960 -> 40335 fragments, still incomplete. New dominant budget: key 1815054, preparation 760035, source 595108, lookup 224059, backdrop 83258; final credit=0. Next certify constant clip membership within a pixel to avoid repeated key-bit work. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-global-mask-clear-summary.json`.

### Constant rectangle clip membership and route credit correction (2026-10-12)

Certify only exact single axis-aligned rectangles strictly containing the entire pixel. Seed fixed membership and remove those bits from repeated winding work; point override remains dynamic. Regression rejects boundary contacts, self-intersecting corners and outside pixels. Also corrected an accounting bug: GroupSlot storage belongs in every one of 32 reserved Route entries, not once in the ends-vector credit. This fixes previously undercounted cache metadata; final-credit evidence describes managed ownership, not RSS. Suite: 190 passed, 12 ignored. Real: 22 of 43 routing regions constant, edge key work 974160 -> 487080, exact unchanged edge output. Interior advances 40335 -> 48504 fragments, still incomplete, final credit=0. New key work 1115592 / preparation 1140992 / source 727402; 2711 route misses indicate cache churn remains a candidate. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-constant-rectangle-summary.json`.

### Measured route capacity 32 versus 64 (2026-10-12)

Increase bounded route cache to 64 entries / 128 index slots with complete upfront metadata admission, unchanged 4M shared-work and 256 MiB diagnostic memory limits. Existing 260-route control still forces repeated eviction (65 distinct coverage states). Suite: 190 passed, 12 ignored. Real edge output exactly unchanged. Interior route misses 2711 -> 206, preparation work 1140992 -> 96548, fragments 48504 -> 65433; still incomplete, final credit=0. Retain 64 because measured preparation savings outweigh reserve growth on this case; no universal timing claim. Remaining work dominated by key generation (1504959) and native sources (1005340). Next investigate a certified route-local membership signature for geometry belonging only to overwritten objects. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-route-capacity64-summary.json`.

### Certified local route membership signature; interior pixel completes (2026-10-12)

Keep all visible paint requirements and one false-region witness per non-hidden invisible paint. Ignore only geometry covered by independently certified opaque overwrite. Last-route shortcut tests this signature directly against current winding; mismatch uses complete key/hash lookup. Sampled fields/mesh ownership still evaluate each point. Shared work and cancellation apply to both paths. Signature storage is compact to actual key-word count and fully admitted; unchanged sparse-mesh peak-memory regression caught/rejected a 4096-region fixed reserve. New witness regression rejects uncertified invisibility. Suite: 191 passed, 12 ignored. Real edge native/RGB output exactly unchanged; interior (164,39) completes 68286 fragments, 38436 local signature hits, final credit=0. Independent 128x native-profile oracle max channel error 0.1290345622824347 / 255. This qualifies these bounded pixel controls, not full-page AI or speed of the production viewer. Next broaden spatial pixels/masks and shared-page preparation. Evidence: `research/ai-covered-fragment-sampling-2026-10-12/spatial-local-signature-summary.json`.

2026-10-12 current unlock confirmation: existing Apple M4 Max Metal HDR surface probe accepted RGBA16Float/ExtendedSrgbLinear configuration, validation None, display headroom current1.2/potential16.0. Surface returned Occluded for10.004427667s, zero presented frames, exit1. This does not establish that the screen is locked; live presentation/resize and physical HDR remain unqualified. Evidence: research/metal-unlock-current-confirmation-2026-10-12.json and hashed log.

### Broader spatial pixels and active-mask oracle comparison (2026-10-12)

Three additional earlier pixels (161,47),(63,180),(82,162) completed with maximum native-profile 64x oracle RGB errors 0.517,0.563,0.887 /255 and no live masks. New mask-locator diagnostic adds conservative source/next-path bounds; ordinary suite191passed/12ignored. Three candidate pixels completed/no deadline/finalcredit0. (232,95) actually uses1862 mask samples, 64x max RGB error1.243; (225,15) error0.217. (127,38) has no live mask and substantial color mismatch: native [166.09565377235413, 184.0734162926674, 111.30137041211128], 128x oracle [185.04766845703125, 203.4061279296875, 123.74212646484375], max error 19.33271163702011 /255; oracle64-to128 change 1.02398681640625. This is a concrete unresolved color/compositing failure, not full-page qualification; next isolate original point colors/group/blend ordering against oracle before production integration. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-live-mask-pixels-summary.json and spatial-broader-pixels-summary.json, retained hashed logs/PPMs.

### Individual point conversion diagnostic (2026-10-12)

Optional diagnostic-only RRRAH_AI_SPATIAL_POINT_GRID (1..4) replays fully composited native point samples and converts each independently to RGB. No production path or sampling policy changed. Ordinary suite191passed/12ignored. Real (127,38) four quarter-cell points complete/finalcredit0, RGBmean [165.90838082134724, 183.99480611085892, 111.10072787851095], maximum difference from native-area-then-RGB 0.20064253360033035 /255; error versus128x profile oracle 19.411321818828583. Coarse diagnostic does not certify continuous color integration, but conversion averaging alone does not account for the observed large discrepancy. Next isolate shading owner/source and group blend order. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-point-grid-summary.json and hashed log.

### Direct source versus routed source diagnostic (2026-10-12)

Optional point-only RRRAH_AI_TRACE_POINT_SOURCES compares routed native source with independently bounded100k-work direct query without modifying replay work. Suite191passed/12ignored. Actual VectorApple center127.5,38.5 invokes shading event2103, ownerNone; selected and direct source tuples exactly agree: CMYK[0.5443226,0.21231109,0.798306,0.009295191],opacity1. Full pixel/point still complete and finalcredit0; observed color mismatch remains. This rules out a routed-versus-direct owner discrepancy at this point, not an underlying shading/function or compositor error. Next inspect original axial source evaluation and intervening group/paint states against independent reference. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-point-source-summary.json and hashed log.

### Point group-state isolation (2026-10-12)

Optional point-only RRRAH_AI_TRACE_POINT_STATES records current native pixel after dispatched commands and group isolated/knockout/opacity/AIS/mode. Suite191passed/12ignored; actual center127.5,38.5 completed/finalcredit0. Sourceevent2103 is followed by nonisolated knockout group opacity1 Normal; visible retained siblings increase opacity through0.4375. Last visible child CMYK[0.1889999955892563, 0.07000000029802322, 0.6019999980926514, 0.0] gives final[0.388868966139853, 0.15004998771473765, 0.7124221175909042, 0.005228544760029763], matching baseline*(1-0.4375)+child*0.4375 exactly within 0.0. Source PDF page explicitly declares DeviceCMYK, I=false,K=false (direct pypdf inspection); root CMYK choice agrees. Color discrepancy remains, and this control does not prove complete PDF knockout semantics. Next independently compare retained-fade/knockout group interpretation to reference. Evidence: research/ai-covered-fragment-sampling-2026-10-12/spatial-point-state-summary.json and hashed log.

### Independent retained knockout fade control (2026-10-12)

Authored8x8 uniform DeviceCMYK PDFs retain actual15child colors/opacities from point trace, explicit nonisolated child forms and parentK=true/false. Same-profile Poppler centerRGB: ko-all[185,203,122],ko-last[165,184,109],nonko-all[204,210,122]. Native realpoint[164.830,183.558,110.188] matches last-only behavior, not reference fullsequence. This reproduces large mismatch independently of original mesh/curves. No semantics change yet: reference is not itself proof of PDF specification; next qualify knockout group shape/opacity and add regression before fix. Reproduciblegenerator,PDFs,PPMs/hashes: research/ai-covered-fragment-sampling-2026-10-12/knockout-control/report.json.

### Live HDR after desktop-ready confirmation (2026-10-12)

Current actual Apple M4 Max/Metal HDR surface probes both succeed, exit0, RGBA16Float/ExtendedSrgbLinear/no validation errors. Raster291frames/5.000s, resize800x400confirmed/261subsequentframes; native tests/IMG_9043.CR3 RAW255frames/5.004s, resizeconfirmed/225subsequentframes. Display currentHDRheadroom2.6666667/potential16.0. Supersedes prior Occluded attempts for current live presentation/resize only. No calibrated luminance/color claim or full100format/NVIDIA qualification. Evidence: research/metal-desktop-ready-2026-10-12.json and metal-desktop-ready-raw-2026-10-12.json, hashedlogs/source.

### Knockout reference conflict resolved against ISO control (2026-10-12)

CORRECTION to earlier color-error wording: (127,38) establishes cross-renderer discrepancy, not proven native formula error. Read authoritative Adobe PDF32000_2008, sections11.4.6/11.4.8, physicalpages346/347/349/350; full binary shape requires topmost enclosing element only, with nonisolated subgroup using outer initialbackdrop. Native finalCMYK matches this formula. Added two authored PDF regression fixtures (15siblings versuslastonly), real interpreter+spatial replay gives identical expectedCMYK within1e-6 andalpha1. Suite192passed/12ignored. Independent PDFium gives ko-all==nonko-all[204,210,122]; Quartz likewise[202,210,122], so neither qualifies this knockout rule. Poppler ko-all differs frombothnative andlast-only. Retain native semantics; do not patch shape to force Poppler parity. Full original-page/color qualification still open. Primarysource URL/hash, three-renderer controls andnative testhash in research/ai-covered-fragment-sampling-2026-10-12/knockout-control/report.json.

### Current combined Metal/storage gate after live HDR (2026-10-12)

Full invoked driver terminalexit0,25suites/580passed/0failed/90explicitignored,complete_suite_execution=true/all_executed_suites_verified=true. Cache150,viewer218,GPU111,model6,HDRTTL24,camera12; separatelyrequired selectedICO/TIFFplannedprefetch/cancellation,3Dworker,compute-cache,fullsensorcompute,textureextent andthumbnailcompletion all verified. Storage9tests241.51s:100k writes+reads/1000pressurerefusals92.329s; byte/countTTL/writerburst/stalewaitercontrols;64MiBfloat3cycles101.166s,eachcancelafter>=64KiBread/exactallfloatbits/finalcredit0. Counts include5CUDAhost tests, notNVIDIAhardware. Explicitignores andartificialdiskdelay/managedpressure limits remain. LiveHDRraster/RAWsuccess is separate report, notphysicalcolorcalibration; complete100format scope notachieved. Evidence: research/metal-storage-current-2026-10-12-final-live-summary.json and report/loghashes in research/metal-storage-current-2026-10-12-final-live/.

### Current full ordinary native decoder regression and PTX boundary (2026-10-12)

Current rrrah-decode --lib terminalexit0:1017passed/0failed/72explicitignored. Includes independent authored raster corpus, malformed/admission/cancellation regressions; does not qualify ignored original-camera corpus or100viewerformats. Retained loghash/ignoredtest names in research/native-decode-current-2026-10-12.json. PrimaryPentaxBrowser/Laboratoryv3.10manual andK10DRAWrecordingdocument listPEF/DNG, not distinctPTXstorage. KeepPTXrowPending and94/6countsunchanged; audit limitedsourceabsence is not worldwideabsenceproof. Evidence: research/ptx-primary-source-boundary-2026-10-12.json.

### TTL expiry during an admitted active swap read (2026-10-12)

New ordinary concurrent regression pauses actual delayed file restore only after262144byteoutput admission and>=65536bytesread. TTL200ms expires during250mspause; explicit indexprune retains1diskobject/physicalfile owned by active handle. Resume returns exact256x256RGBA8; expiredkeylookup misses,no republish; diskquota/files0,buffercredit262144untilresultdropthen0,queue0. Focused1passed; fullcache151passed/0failed/6ignored. Existing production lifecycle already correct; added regression closes combined expiry/read ownership gap. No dynamicpolicy-during-read orOSpressure claim. Evidence: research/cache-active-reader-ttl-regression-2026-10-12.json and hashedlogs.

### Live model HDR surface and resize (2026-10-12)

Extended existing HDR probe with RRRAH_HDR_MODEL_SOURCE: native decode under512MiBmanagedCPUbudget, direct triangle iteration into ModelRenderer, depthresize andviewportupdate onwindowresize, sourcecredit0afterupload. Offlinebuildpassed. Current actualAppleM4Max/Metal RGBA16Float/ExtendedSrgbLinear four sequential fixtures STL/OBJ/PLY/OFF all terminalexit0/no validation errors,3seach,173frames(STL/OBJ/OFF),175frames(PLY);800x400resizeconfirmed andpositiveframesafterresize ineach. This qualifies current livepresentation/resize for these authoredsubsetfixtures; no physicalcolorcalibration/liveinput/broadmaterials orspeedclaim. Evidence: research/metal-live-models-2026-10-12.json and hashedlogs/sources.

### Ordinary viewer live 3D input (2026-10-12)

Current normalviewer offlinebuildpassed. TemporarymacOSbundle wraps samebinary solelyforCUAbinding; concaveOBJ6triangles actuallyvisible. CUAdrag changesorbit, wheel enlargesgeometry, F restoresinitialgeometry; baseline/orbit/zoom/resetPNG retainedandvisuallyinspected. Bundlewindowclosed throughnativeclosebutton; initialunbundledownprocess cleanedSIGTERM separately. This qualifies these ordinarywindowinputactions for authoredOBJ, not allmodels/materials/color/performance orNVIDIA. Evidence: research/metal-live-model-input-2026-10-12/report.json and hashedscreenshots.

### 2026-10-12 — live mixed raster/model navigation

Ordinary Metal viewer with HDR surface enabled navigated QOI (257x3) → OBJ (6 triangles) → PFM (2x2) → OFF (1 triangle), then back through PFM/OBJ/QOI using native arrow-key input. Each renderer switch displayed the expected fixture. Viewport return comparisons and source/screenshot hashes are recorded in `research/metal-live-mixed-navigation-2026-10-12/report.json`. Four authored fixtures only; this does not qualify all formats, throughput, or physical HDR color/luminance. Window closed through its native close button.

### 2026-10-12 — foreground asymmetric prefetch windows

Added ordinary foreground planner regression covering ahead=3/behind=1, both collection boundaries, reverse direction and disabling both sides. Focused test passed; current ordinary viewer suite: 219 passed, 0 failed, 62 explicitly ignored. Evidence and log hashes: `research/prefetch-asymmetric-window-2026-10-12.json`. Initial window status now says loading instead of claiming every source is RAW. These planner tests do not qualify physical disk throughput or all decoders.

### 2026-10-12 — current 100k-path actual Metal navigation

Re-ran `hundred_thousand_valid_sources_preserve_metal_frames_and_priority`: terminal exit 0, one test passed on Metal Apple M4 Max. 100000 valid hard-linked paths from four PNG/GIF/EXR/HDR originals; 512 navigation steps took 5.874807 seconds excluding path creation/cleanup. Exact source float bits and SDR/RGBA16Float frames versus fresh same-renderer references, nearest prefetch protection, 2-entry/64KiB cache bound, full managed pressure hit and stale cancellation all passed. Managed CPU peak 1048576 bytes, final 0. This is collection-scale transport evidence, not 100000 unique files, OS RSS, physical slow-disk or calibrated HDR color evidence. Report: `research/metal-100k-current-2026-10-12.json`.

### 2026-10-12 — four prefetch policies across mixed Metal sources

Extended the shared ordinary/actual-Metal mixed-gallery regression to cycle ahead/behind windows (2/1), (3/0), (0/3), (0/0), each across three complete forward/backward periods. Six PNG/GIF/EXR/HDR/PFM/ASTC fixtures, 120 steps per test. CPU and Metal variants both passed; exact SDR/RGBA16Float reference frames, visible float bits, count=2/64KiB cache bounds, full managed pressure hits, stale cancellation and final budget release hold. Ordinary viewer suite remained 219 passed, 0 failed, 62 explicitly ignored. Evidence: `research/metal-mixed-prefetch-policies-2026-10-12.json`. These remain transport checks against the same renderer, not universal decoding or independent color qualification.

### 2026-10-12 — actual half-float output across live cache policy changes

Closed a verification gap in `ttl_swap_metal`: the live policy helper previously checked HDR source bits but rendered only SDR output. It now also compares RGBA16Float frames while a lease pins the original, after its swap restore and after replacement restore. Actual Metal Apple M4 Max passed 200 small cycles (7.19s) and one 4000x3000 full-component/full-output cycle (3.80s), both terminal exit 0. CPU and HDR-renderer credits return to zero. Report: `research/metal-policy-half-readback-2026-10-12.json`. Authored payload transport evidence only; physical HDR calibration, OS RSS and slow disks remain outside these tests.

### 2026-10-12 — JPEG XL pre-decode output admission

Fixed JXL retained output allocation occurring before managed admission. RGBA output credit now reserves before `DynamicImage::from_decoder` and is adopted by 8/16/float output pixels. New 8/16-bit regression rejects a one-byte-short budget with managed peak zero, accepts exact capacity and tracks cloned owners to final release. Focused 3 tests and full decoder suite 1018 passed, 0 failed, 72 ignored. Report: `research/jxl-output-admission-2026-10-12.json`. Header/internal codec scratch remains separate; no broader JXL animation/HDR/full-qualification claim.

### 2026-10-12 — JPEG XL codec working admission

Extended the earlier output-only fix: separate working credit now covers a tracker cap of min(65536 + 64 bytes/pixel, MAX_RASTER_BYTES), plus one output-image equivalent for conversion overlap. The cap is enforced through JXL decoder limits and the shared memory budget reserves working credit before pixel decoding. An output-only budget now correctly refuses; enough working/output credit succeeds and retains only output credit after decode. Three focused tests and 1018 ordinary decoder tests passed (72 ignored). Actual Metal collection passed 65 sources, 94 selections, 188 native/prepared storage cases, final root/queue zero. Report: `research/jxl-working-admission-2026-10-12.json`. Header parsing and untracked codec allocations are outside this reservation; it is not a strict RSS guarantee or universal JXL variant qualification. This supersedes the earlier output-only admission boundary.

### 2026-10-12 — grayscale JXL viewer recovery and decode-error cleanup

Expanded Metal collection with alpha-container-1.jxl and precision16.jxl plus independent libjxl native RGBA oracles and exact authored 16-bit samples. The first run failed at display preparation: Gray ICC was categorically rejected. Fixed color preparation to transform replicated grayscale samples through Gray ICC into linear sRGB, with Gray tone-curve normalization, unchanged alpha and refusal of nonreplicated inputs. Mathematical gamma-2 control passes. A truncation regression tests all 249 nonempty prefixes, requires at least one error after working admission and verifies final managed bytes zero after every attempt. Final decoder suite: 1020 passed/0 failed/72 ignored. Actual Metal collection now passes 67 sources, 96 selections, 192 native/prepared cases; final root/queue zero. Failed reproduction and successful logs hashed in `research/jxl-gray-icc-recovery-2026-10-12.json`. Broader gray ICC variants and calibrated physical display qualification remain open.

### 2026-10-12 — grayscale ICC through ordinary PNG/TIFF decode

Added an ordinary file-route regression for PNG La16 and TIFF Gray16 carrying a Gray ICC gamma-2 profile. Native samples 0/32768/32769/65535 remain exact; linear RGB matches the independent mathematical squared-normalized-sample control within 0.0001, adjacent values remain distinct and PNG 16-bit alpha is unchanged. All managed credit is released. Focused test and full decoder suite passed: 1021 passed, 0 failed, 72 ignored. Evidence: `research/gray-icc-png-tiff-2026-10-12.json`. This validates one authored profile curve, not all ICC variants or physical HDR display behavior.

### 2026-10-12 — authored grayscale PNG/TIFF across RAM/swap/Metal

Added deterministic independently byte-authored Gray ICC gamma-2 PNG La16 and TIFF Gray16 fixtures (generator `scripts/generate-gray-icc-fixtures.py`; regeneration hashes match). Metal collection verifies native samples, mathematical gamma-2 linear values, distinct adjacent 16-bit values, alpha, native/prepared RAM/swap and exact SDR/RGBA16Float transport. Actual Apple M4 Max passed 69 sources, 98 selections, 196 storage cases; final root/queue zero. Evidence and source hashes: `research/metal-gray-png-tiff-2026-10-12.json`. This covers one authored profile curve; broader producers/profiles and physical calibrated HDR output remain open.

### 2026-10-12 — independent LittleCMS grayscale color oracle

Built a separate LittleCMS C oracle using the embedded authored Gray gamma-2 ICC profile, relative-colorimetric intent and double-precision linear-sRGB output. Current normal decoder/display dumps for both PNG La16 and TIFF Gray16 agree at all four authored gray values; maximum absolute linear RGB error 1.1920928955078125e-7. Adjacent 32768/32769 outputs remain distinct and PNG alpha remains normalized source coverage. Evidence, oracle source and hashes: `research/gray-icc-lcms-2026-10-12/report.json`. This is independent CPU color evidence for one profile/four values, not all ICC variants or physical HDR calibration.

### 2026-10-12 — complete 16-bit grayscale range vs LittleCMS

Independent CPU oracle comparison now covers all 65536 source codes in a 65536x1 PNG alpha16 row, across every 4096-pixel transform chunk boundary. Authored gamma-2 ICC: maximum linear RGB error 1.7881393432617188e-7; every adjacent red pair strictly increasing. Authored 17-entry sampled Gray curve: maximum error 1.537799835205078e-5 (1.007797 linear 16-bit code units), no decreasing pairs; both native and LittleCMS produce 16383 equal adjacent red pairs. The initial strict-distinctness requirement for sampled curves was invalid and rejected; no native-only precision defect was established. Alpha error <=2.9801867640344426e-8. Evidence, generator and independent C oracle: `research/gray-icc-full-ramp-2026-10-12/report.json`. Two authored curves only; broader ICC variants and physical HDR remain unqualified.

### 2026-10-12 — DPX/Cineon explicit untagged viewer policy

Extending the shared Metal collection with all 24 DPX and 12 Cineon native-oracle fixtures exposed refusal despite explicit untagged sRGB selection. Fixed DPX and the shared Cineon envelope path to accept requested sRGB/linear-sRGB only if both transfer and colorimetry are undefined (255); declared incompatible color and strict default remain refused. Ordinary regression verifies both policies and declaration boundaries. Full decoder suite: 1022 passed, 0 failed, 72 ignored. Actual Metal collection: 105 source files (not formats), 134 selections, 268 native/prepared RAM/swap cases, final root/queue zero; DPX/Cineon native samples match FFmpeg/OpenImageIO references. Evidence includes failed reproduction: `research/dpx-cineon-untagged-metal-2026-10-12.json`. Explicit interpretation is not producer-color or film-log qualification; broader variants and calibrated HDR remain open.

### 2026-10-12 — DPX/Cineon prefetch color-policy cache separation

Added ordinary viewer regression for 16-bit DPX/Cineon: actual CLI policies reach preload, sRGB and linear-sRGB retain distinct prepared buffers/cache entries with unchanged alpha, strict requests refuse reuse, and both variants hit identical retained buffers under full managed pressure without decoding. Final root zero. First test used an unjustified >0.01 difference threshold on dark DPX samples; corrected to policy ordering/separation. Current viewer suite: 220 passed, 0 failed, 62 ignored. Report: `research/viewer-dpx-cineon-policy-prefetch-2026-10-12.json`.

### 2026-10-12 — DPX/Cineon buffer admission before allocation

Fixed DPX output allocation before managed admission: final RGBA credit reserves before pixel storage and is adopted into output; 8-bit conversion also reserves the overlapping u16 working buffer. Exact u8 capacity avoids retaining a wider reused allocation. Cineon now admits its exact envelope header/payload copy in the same root before allocation. New 8/16-bit tests verify one-byte-short refusal releases all credit, exact-budget peak and only retained output credit after decode. Full decoder: 1024 passed, 0 failed, 72 ignored. Actual Metal collection remains green at 105 sources/134 selections/268 cases, final root/queue zero. Report: `research/dpx-cineon-buffer-admission-2026-10-12.json`. Managed admission is not a strict OS RSS guarantee.

### 2026-10-12 — direct 8-bit DPX output

Removed the full-image u16 intermediate and subsequent conversion for 8-bit DPX (also used by Cineon). Pixel components now write directly into admitted final RGBA8; higher precision writes admitted RGBA16. Pixel-buffer peak falls from 12 to 4 bytes/pixel, excluding source and Cineon envelope. Exact-budget regressions prove only final pixel credit is needed; short budgets release cleanly. All 24 DPX/12 Cineon independent sample controls pass. Full decoder 1024 passed/0 failed/72 ignored; actual Metal collection 105 sources/134 selections/268 cases remains exact with final root/queue zero. Evidence: `research/dpx-direct-output-2026-10-12.json`. Throughput was not measured, so no speedup claim.

### 2026-10-12 — current 12MP DPX CPU loading measurements

Extended raster_load_bench with explicit managed-sRGB mode using a 512MiB shared root and final-release assertion. Deterministic 4000x3000 RGB8 DPX, three warmups and eleven measured iterations on local M4 Max. Final p50 load 299.734ms, prepare 79.915ms, CPU-ready 379.650ms; CPU-ready p95 454.486ms. Managed peak 240000000 bytes, final zero. Stable display hash across all iterations; percentile helper test passed. Build is dev opt-level=1 for workspace, dependencies opt-level=3. Final run followed confirmed benchmark-test exit; shared host load remains uncontrolled. No matched baseline or GPU claim. Evidence and generator: `research/dpx-12mp-current-bench-2026-10-12/report.json`.

### 2026-10-12 — measured DPX 8-bit row fast path

Added direct 8-bit nonrotated row handling for gray/RGB/RGBA/ABGR, preserving shared header/output admission and checking cancellation in <=4096-pixel blocks. General orientation/packing path remains. All 1024 decoder tests pass (72 ignored). Added authored 4097x2 RGB8/padded-row fixture and exact native oracle; actual Metal collection passed 106 source files, 135 selections, 270 cases, final root/queue zero. On the same authored 12MP RGB8 source, saved-before/new binaries ran ABBA sequentially, each three warmups plus eleven samples: load p50 before 283.361/305.919ms, after 84.398/82.908ms (~3.52x ratio of mean run medians); CPU-ready p50 before 375.655/383.941ms, after 173.600/164.812ms. Prepared hashes match in all runs; managed peak remains 240000000 bytes. Dev workspace opt-level=1/dependencies=3, warm filesystem/shared host; not a universal/release/GPU speed claim. Evidence and binary/source hashes: `research/dpx-row-fast-path-2026-10-12/report.json`.

### 2026-10-12 — 12MP DPX cancellation and same-budget retry

Extended the timing diagnostic to accept an explicit benchmark memory cap and retry every cancelled decode using the same root. On authored 4000x3000 RGB8 DPX, 11 samples each at requested 1/25/60ms signals: all 33 cancelled and all 33 retried successfully at 4000x3000; every final managed root zero. The 25/60ms groups each reached 84002048 managed bytes (source plus output) before cancellation; signal-to-exit p95 0.030667/0.210875ms respectively. Early 1ms p95 2.094333ms. Local scheduling/cleanup included; no universal bound, physical disk or RSS claim. Evidence: `research/dpx-cancel-retry-2026-10-12/report.json`.

### 2026-10-12: full 12MP DPX Metal/swap output

Actual Apple M4 Max Metal passed native and prepared storage for an authored 4000x3000 RGB8 DPX: every native sample matched the authored oracle, and every SDR / RGBA16Float output pixel matched before and after swap. Lease protection, cancellation after restore allocation admission, budget refusal/retry, and final managed root/queue zero passed. Full-output test: 1 passed, 0 failed (13.81 s). Shared collection regression: 106 sources, 135 selections, 270 storage cases; 1 passed, 0 failed (23.17 s). Evidence: `research/metal-dpx-12mp-full-frame-2026-10-12/report.json`. RGB8 content qualifies HDR output transport only; physical HDR calibration, NVIDIA hardware, OS RSS and physical slow-disk pressure remain outside this check. Format qualification counts unchanged.

### 2026-10-12: 12MP float HDR restore pressure and cancellation

Extended the full 4000x3000 float HDR Metal/swap test with full managed-budget refusal, cancellation after restore allocation admission, zero-credit cleanup and successful retry. Actual Apple M4 Max Metal: 1 passed, 0 failed, 2.83 s. All source float bits and all 48 million binary16 output components remain exact after swap. Independent bright/negative/exposure region controls pass (boundary pixels included in transport comparison). Evidence: `research/metal-12mp-hdr-pressure-cancel-2026-10-12.json`. Managed budgets do not establish OS RSS, physical slow-disk pressure or physical HDR calibration; NVIDIA remains deferred.

### 2026-10-12: qualification runner collection inventory refresh

Updated `scripts/qualify-viewer-storage-gpu.py` from the stale 65-source/94-selection/188-case inventory to the current 106-source/135-selection/270-case collection. The current actual Metal log is accepted; seven mutated logs (missing native/HDR case, duplicate native/HDR case, missing bounds, old summary, missing independent HDR control) are refused. Evidence: `research/metal-collection-verifier-current-2026-10-12.json`. This repairs aggregate evidence validation; it does not expand format support or qualify physical HDR.

Current combined actual Metal GPU gate completed successfully: `research/metal-current-gpu-gate-2026-10-12/report.json`. Includes ordinary GPU suites plus the complete raster collection and 12MP float HDR swap test. Explicit ignored tests remain unqualified; every executed suite passes the runner evidence checks.

### 2026-10-12: full LOD output and upload recovery

Expanded the 36MP authored HDR/alpha area-LOD test to compare every SDR and binary16 component of the full 1000x1000 reduced GPU output against a separate constant reference, retaining the 64x64 control and all-million-pixel CPU oracle. Actual Metal passed (1.81 s; downsample 183.614167 ms local observation; output credit zero). Re-ran 12MP upload replacement pressure/cancellation recovery for SDR and signed HDR at six checkpoints, preserving previous frame then exact retry with final GPU/queue credit zero (2.23 s). Evidence: `research/metal-large-lod-upload-current-2026-10-12.json`. These two ignored-by-default tests are now explicitly executed; other skips and physical HDR/NVIDIA limitations remain.

### 2026-10-12: full-sensor compute admission recovery

Extended the 6188x4120 managed exposure test with independent CPU/GPU budget pressure refusal and cancellation after both admissions; the source survives and all temporary credits return to zero before successful retries. Actual Apple M4 Max Metal passed both interleaved and managed interfaces with every one of 25,494,560 pixels compared against index-dependent RGB/alpha across seven dispatch windows. Final shared-owner release is zero; GPU peak 1,223,738,992 bytes and CPU peak 815,825,920 bytes. Test 1 passed, 0 failed, 2.53 s. Evidence: `research/metal-full-sensor-recovery-current-2026-10-12.json`. Authored input does not qualify camera demosaic/color, OS RSS or NVIDIA hardware.

### 2026-10-12: minute-long full-output HDR collection

Actual Metal M4 Max completed 61.703 s of eight 4000x3000 float frames (192 MB each): 37 restores, 37 admitted mid-batch cancellations, 37 full SDR and full RGBA16Float comparisons; all source float bits and outputs exact. TTL pin/unpin, managed pressure refusal and parallel restore retry pass; only the initial eight writes occur, zero redundant writes, final root/queue zero. Test 1 passed, 0 failed, 77.89 s including setup; runner `collection_evidence` validates complete workload. Evidence: `research/metal-full-hdr-collection-60s-current-2026-10-12.json`. Local disk has no injected delay; one minute does not qualify hours-long operation or OS RSS. Added permanent collection evidence acceptance/refusal tests in `scripts/test-viewer-storage-gpu-evidence.py` (3 passed, including six missing/duplicate subcases); refreshed CLI help inventory.

### 2026-10-12: current Metal HDR render-pass timing

Extended manual timestamp reporting with nearest-rank p95, maximum, warmup count and output format. Actual Apple M4 Max Metal, 4096x4096 float source to 2048x2048 RGBA16Float: 5 warmups, 30 samples, GPU pass median 0.108271 ms, p95 0.554042 ms, max 0.651125 ms. 1 passed, 0 failed; final managed GPU credit zero. Evidence: `research/metal-hdr-gpu-timestamps-current-2026-10-12.json`. Pass timestamps exclude decode/upload/CPU submission/readback/presentation; authored constant input and sequential completion do not establish whole-viewer latency or saturated throughput. No speedup claim.

### 2026-10-12: sustained delayed float swap and independent limits

Current storage tests passed 4/4: 64MiB float HDR delayed I/O completed three whole cycles in114.883s (requested at least60s plus minimum3 cycles); every float bit including negative zero/HDR extrema exact after managed-pressure refusal, cancellation after allocation and actual64KiB payload progress, then retry. Physical local-file count2/byte bounds and final root/queue zero. Independent byte/count/TTL delayed modes passed58/59/60 cycles respectively over10s each; all177 pressure refusals retry and finalzero. Evidence: `research/swap-delayed-current-2026-10-12.json`; runner large-float evidence validator passes. Delays are injected per local I/O call, not physical slow disk or OS RSS evidence.

### 2026-10-12: queued duplicate selection refreshes prefetch plan

Fixed a production foreground-queue omission: duplicate same-file selection before worker dequeue previously retained the old explicit neighbour plan. It now updates the queued plan (including explicit empty/disabled) while preserving generation, foreground admission and single decode request. Regression exercises forward-to-backward order and enabled-to-disabled changes without extra queued requests. Full ordinary viewer suite:220 passed,0 failed,62 ignored; mixed six-format actual Metal120-step navigation regression:1 passed,0 failed, SDR/HDR exact and final managedzero. Evidence: `research/prefetch-pending-plan-current-2026-10-12.json`. This addresses requests still queued; already active/dequeued work is outside the change. Initial incorrectly qualified exact filter ran0 tests; full suite subsequently executed the regression.

### 2026-10-12: observed process memory for12MP DPX

Measured native DPX decode plus sRGB display preparation with macOS `/usr/bin/time -l`, two fresh processes, each3 warmups+11 measured loads. Both display digests match; managed peak240,000,000bytes/finalzero. OS maxRSS and peak footprint are recorded separately in `research/dpx-process-memory-observed-2026-10-12.json` with source/binary hashes and original logs. These are observed per-process peaks, not hard RSS caps, system-wide pressure qualification or GPU/display memory measurements; allocator retention is outside managed finalzero. Current benchmark build verified before second run.

### 2026-10-12: active decode observes latest prefetch plan

Replaced the preceding queued-only plan update with shared per-request plan ownership between UI/foreground identity and LoadRequest. Same-decode identity deliberately excludes neighbour settings; explicit plan revisions including disable preserve generation/admission and update the running worker. Worker snapshots the latest plan after decode, before neighbour resolution; no channel dequeue/requeue is needed. Regression covers queued, already dequeued and source-identified active requests, no duplicate decode/cancellation, forward/backward changes and disable. Final ordinary viewer suite220 passed/0 failed/62 ignored; actual Metal mixed six-format120-step navigation1 passed/0 failed, SDR/HDR exact, managedfinalzero. Evidence: `research/active-prefetch-plan-2026-10-12.json`. Updates after neighbour planning starts are outside this change; physical HDR/NVIDIA remain open.

### 2026-10-12: empirical Metal HDR range controls

Both ignored empirical Metal extreme-range tests already passed on current M4 Max; no shader change warranted. Expanded signed binary16 control to exposures -10/0/+10 with every4096 output pixels checked against exact half encodings; opposite f32 extrema retain exact symmetric center and all63x65 components finite. 2 passed/0 failed,1.27s. Added explicit Metal-only runner option `--metal-hdr-range` with required adapter/control evidence; nine acceptance/refusal checks pass. Evidence: `research/metal-hdr-range-expanded-2026-10-12.json`. Empirical range behavior remains outside portable caller contract, does not qualify other adapters or physical HDR.

### 2026-10-12: native BAY image-only row streaming

Added `read_bay_image_sensor_with_budget` and explicit `image_dimensions`: QV5700 produces2576x1924 image-bearing sensor samples without decoding trailer positions or allocating a full-storage intermediate/crop copy; legacy2585-wide full-storage API is preserved. QV2000UX/QV3000EX retain1632x1211/2080x1547. Every new output sample matches pinned authored independent LibRaw test-only dumps; self-contained ordinary tests also verify all authored values. Exact output+one-row budget passes; one-byte-short refusal releasesall; QV5700 peak9,915,680bytes/finalzero. Focused9 passed; full decoder1024 passed/0 failed/73 ignored. Evidence: `research/bay-image-only-stream-2026-10-12.json`. Initial fixture path error corrected and failed log preserved. Original camera interoperability, CFA/WB/color and viewer routing remain Pending; catalogue counts unchanged. Layout source: https://www.inweb.ch/foto/rawformat.html.

### 2026-10-13: BAY image-only late cancellation and retry

Expanded stream recovery regression across QV2000 full storage, QV3000 image-only, QV5700 full storage and image-only. All16 cancellations after allocation (first/middle/last row and EOF) release partial buffers and permit successful same-root retry with correct output length/retained weight; all8 truncated/growing-source cases refuse and release credits. Finalrootzero throughout. Focused including independent authored oracles9 passed/0 failed; complete decoder1024 passed/0 failed/73 ignored. Evidence: `research/bay-image-recovery-2026-10-13.json`. Retry success/shape/ownership is distinct from the separate exact pixel-oracle test. Original CFA/WB/color/viewer remains Pending.

### 2026-10-13: actual RAW worker scene-linear HDR navigation

Extended real EOS R8 gallery RawPrefetcher reversal qualification beyond SDR:1920x1080 plus three full6188x4120 frame comparisons now check every RGBA16Float scene-linear component for exact transport and finiteness. Actual Metal M4 Max1 passed/0 failed,6.80s. Original sensor samples/metadata exact; obsolete blocked-forward work unpublished, backward and subsequent two reversals preserve sole disk slot, final managedrootszero. The runner now requires four HDR markers; permanent evidence tests reject missing/duplicated/SDR-only logs (5 tests passed including collection cases). Evidence: `research/metal-raw-worker-linear-hdr-2026-10-13.json`. This gallery worker differs from foreground shared-plan loader; same-decoder/render comparisons do not independently qualify camera color, all cameras, physical HDR or NVIDIA.

### 2026-10-13: mixed-camera and three-neighbor RAW HDR transport

Extended actual gallery RAW-worker tests with full-resolution scene-linear RGBA16Float comparisons and finite-component checks for all9 corpus files (CR2/NEF/ARW/ORF/PEF/RW2/RAF/CR3) and6 restores across forward/backward three-neighbor plans (ahead2/behind1). Existing all-source sample/metadata and SDR checks pass; disk count1/3 bounds, selected-source exclusion and finalrootszero preserved. Metal M4 Max2 passed/0 failed (16.56s mixed;12.18s neighbors). Inputs rehashed against pinned originals. Runner now requires each source HDR marker in exact expected order; eight actual-branch acceptance/refusal checks pass for complete/missing/duplicate/SDR-only logs. Evidence: `research/metal-mixed-raw-workers-hdr-2026-10-13.json`. Same native decoder/renderer verifies transport, not independent camera color or all RAW variants. Physical HDR/NVIDIA remain open.

### 2026-10-13: combined current-source Metal gate

Current-source combined actual Metal M4 Max gate completed11 suites,144 passed/0 failed, all runner workload evidence valid. Includes131 ordinary GPU tests, nine-source RAW collection90 restores/90 exact finite HDR frames over10 rounds, mixed and three-neighbor RAW workers, raster collection106sources/270storage cases, RAW reversal, empirical HDR range,12MPfloat swap/upload cancellation, compute-cache full12MP and full-sensor compute. RAW collection restorep50/p95=45.982/74.447ms and upload/render/readbackp50/p95=80.773/131.860ms are local descriptive observations, not isolated optimization benchmarks. Evidence: `research/metal-current-combined-2026-10-13/report.json`; `execution-audit.json` separates default skips explicitly executed elsewhere in the same run from tests not executed here. Original100format scope, physical HDR and NVIDIA remain incomplete.

### 2026-10-13: all current default-ignored GPU tests explicitly executed

Supplemental actual Metal run `research/metal-remaining-gpu-cases-2026-10-13/report.json` passed6 suites/143 tests/0 failures, with all workload evidence verified: ordinary GPU, pinned X3F/GoPro transport, fullF828 compute, full36MP HDRLOD, raster timestamp timing and pinned native AI replay-plane transport. Added Metal-only `--metal-large-lod` and `--metal-raster-timing` runner modes with complete-output/distribution evidence checks; permanent evidence tests7 passed including malformed scope/distributions. Union with preceding combined run explicitly executes all13 default-ignored rrrah-gpu tests; none remain unexecuted in that inventory. Audit: `research/metal-default-ignored-inventory-2026-10-13.json`. This is GPU test inventory coverage, not qualification of production AI appearance, all camera demosaic/color,100formats, physicalHDR or NVIDIA.

### 2026-10-13: streaming binary STL viewer geometry

Production shared model/viewer binary STL now selects binary by exact84+50*count length (including `solid` headers), caps input/facet storage before allocation, pre-admits final geometry, and streams1024-facet/51,200-byte stack blocks. No whole-file source allocation; fixed scratch/header are outside managed heap credits. Public low-level/ASCII behavior remains bounded whole-file. All fields/bounds of2050 authored facets match, four admitted cancellations plus EOF and same-root retries recover, truncated/growing streams refuse, clone lifetimes retain final credits. Control managedpeak106,656bytes vs old calculated source+geometry209,240bytes, not a paired performance measurement. Focused7 passed; decoder1025 passed/0 failed/73 ignored; viewer220 passed/0 failed/62 ignored; Metal model6 passed;5 cycles of6 model payloads preserve SDR/HDR under pressure/cancel with finalroot/queuezero. Evidence: `research/stl-binary-stream-2026-10-13.json`. No RSS/speedup, materials/textures or all-format completion claim.

### Live Metal HDR after desktop unlock — 2026-10-13

Current-source HDR surface probe passed raster (176 frames / 3.004 s) and native IMG_9043.CR3 RAW (146 frames / 3.007 s), including resize to 800×400 (146 / 116 subsequent frames). RGBA16Float / ExtendedSrgbLinear, Apple M4 Max / Metal, no validation errors; reported current/potential headroom reached 16. Evidence and source/binary/input hashes: `research/metal-live-ready-2026-10-13.json`. This closes the Occluded retry for this live probe; it does not measure physical luminance/color or qualify NVIDIA.

### Paired STL CPU load measurements (2026-10-13)

Current whole-file low-level API plus output management versus shared streaming model API, warm filesystem, ABBA order, 20 measured samples per path: 250k facets median 6.235 / 6.138 ms, p95 7.416 / 6.805 ms; 1M facets median 22.326 / 21.565 ms, p95 24.765 / 31.280 ms. No robust speedup established; streaming has a worse tail in the million-facet run. Geometry bits/bounds hashes match on all loads and managed credit returns to zero. The separate streaming admission test proves removal of the whole-file heap buffer. This is not a historical before/after or GPU/display/cold-disk/RSS benchmark. [Evidence](research/stl-load-paired-summary-2026-10-13.json).

### Independent delayed swap limits — 2026-10-13

Two current-source 60-second runs passed independently: bytes cap 786,624 with count 100 / TTL 3600 s (520 exact restores + 520 pressure refusals/retries), count cap 3 with bytes 8 MiB / TTL 3600 s (528 restores + 528 refusals/retries). Physical file usage stays within caps; evicted keys become unreadable; root/queue final zero. Injected delay is 2 ms per IO call, max 64 KiB per call, local disk. Runs overlap the ongoing Metal soak, so throughput is descriptive. No physical slow-drive/OS-pressure/full-format claim. Evidence: `research/swap-independent-60s-summary-2026-10-13.json`.

### Ten-minute full-frame Metal cache/swap soak — 2026-10-13

Actual Apple M4 Max / Metal run passed after 600.006 seconds (604.83 s test total): 1,335 restores, 1,335 admitted mid-batch cancellations and retries, 1,335 exact full 4000×3000 SDR plus HDR comparisons. Eight 192 MB float sources; only eight original writes, no redundant writes; managed root and queue return to zero. Process RSS sampled every five seconds after initial ~40 seconds, observations retained separately; this is not a leak proof or managed-budget RSS cap. Local disk, no injected delay, same-renderer transport; no physical HDR calibration, hours-long stability or NVIDIA/full-100-format claim. Evidence: `research/metal-full-hdr-collection-600s-current-2026-10-13.json`.

### Preserve prefetch intent on changed-source retry — 2026-10-13

Reproduced and fixed lost explicit neighbor/disabled plan when a plain duplicate triggers source validation and changed-file reload. Duplicate validation no longer carries a separate stale optional plan; retry snapshots the current shared identity plan. Regression checks selected list and explicit empty disable, replacement generation, active/retry plan preservation. Before: one failed regression; after: foreground loader 77 passed / 13 ignored, whole app 221 passed / 62 ignored. Initial exact-filter zero-test run retained and excluded. No new GPU/format qualification claim. Evidence: `research/prefetch-source-retry-2026-10-13.json`.

### PNG cICP wide-gamut/HDR metadata and explicit policies — 2026-10-13

PNG now preserves cICP primaries/transfer/range for non-sRGB declarations instead of dropping to Unspecified. Existing linear BT.2020/P3 display and explicit DecodeRequest BT.2020 PQ/HLG conversion become reachable. Authored 16-bit black/mid/white/alpha controls verify endpoints, alpha, retained metadata, exact direct-policy/public-request agreement and managed release. Narrow-range PNG is valid metadata (W3C PNG Third Edition section 11.3.2.6) but display expansion remains unsupported; explicit PNG HDR policies refuse narrow codes rather than treating them as normalized full-range RGB. Invalid matrix/range flags still refuse. Full decoder 1027 passed / 73 ignored, app 221 passed / 62 ignored. Initial build errors retained; final runs terminal zero. No new original-producer or Metal/physical-HDR qualification. Evidence: `research/png-cicp-expanded-2026-10-13.json`.

### PNG cICP through actual Metal cache/swap — 2026-10-13

Added deterministic self-authored RGB16 PNG controls for linear BT.2020/P3 and explicit-policy PQ/HLG BT.2020. Actual M4 Max / Metal raster collection passed 110 sources / 139 selections / 278 native/prepared storage cases, including all four new paths. Original linear sample codes, wide-red out-of-sRGB channels, alpha and linear/PQ/HLG white endpoints checked; full 64×64 SDR/HDR frames exact before/after swap. Pressure refusal, admitted cancellation/retry, metadata preservation and final root/queue zero hold. Generator regeneration byte-identical. Evidence verifier requires all four paths and passes eight tests including substitution rejection. Authored and same-renderer transport only; no independent producer, narrow-range display or physical HDR/NVIDIA claim. Evidence: `research/metal-png-cicp-collection-2026-10-13.json`.

### Narrow-range PNG BT.709 SDR display — 2026-10-13

Added integer RGB8/RGB16 CICP(1,1,0,0) display conversion: 16..235 / 4096..60160 codes expand without clipping undershoot/overshoot; signed inverse BT.709, alpha remains full-range. Every 256/65,536 code value agrees with independent f64 formula within 1e-6. Exact output budget, one-byte-short refusal, admitted cancellation/retry and final zero checked. Core120 passed/2 ignored; decoder1027 passed/73 ignored. Actual Metal collection now111 sources/140 selections/280 native/prepared cases; authored five-pixel PNG preserves original codes/color through swap and all64×64 SDR/HDR components. Global PQ/HLG folder policies leave this SDR transfer alone. Narrow float source-depth and narrow PQ/HLG still unsupported. Evidence: `research/narrow-png-bt709-2026-10-13.json`.

### Encoded Display P3 CICP display and Metal transport — 2026-10-13

Shared raster conversion supports full-range CICP(12,13,0,1): signed inverse sRGB transfer followed by D65 P3-to-sRGB primary transform, unchanged alpha, retained negative/>1 linear channels. Fast integer shortcuts now apply only without a primary matrix, preventing skipped P3 conversion. Fixed independent matrix/f64 reference checks RGBA8, large65,536-pixel RGBA16 and signed/extended RGBA32Float within2e-6, with admitted cancellation/retry and exact output budget/final0. Core121 passed/2 ignored; decoder1027 passed/73 ignored. Authored RGB16 P3 PNG joins actualMetal collection112 sources/141 selections/282 native/prepared storage cases, all SDR/HDR frames and metadata exact through pressure/cancel/swap/final0. Verifier8 tests pass and requires encodedP3 source. No independent original-producer, physicalHDR/NVIDIA or speed claim. Evidence: `research/encoded-p3-display-metal-2026-10-13.json`; primary DisplayP3 reference https://www.w3.org/TR/css-color-4/#predefined-display-p3.

### Optional P3 integer transfer tables and measured CPU speed — 2026-10-13

P3 integer paths now use existing sRGB transfer tables while explicitly applying the same primary matrix afterward. RGB16 table scratch256KiB remains optional; one-byte-short scratch budget falls back to scalar without refusing output. All65,536codes/channel match independent fixed-matrix/f64 reference within2e-6; scalar/LUT/fallback bits equal, admitted table cancellation/retry releasesall. Core121passed/2ignored; currentactualMetal collection112files/141selections/282storagecases exact/rootqueue0. Current-path ABBA CPU benchmark4000×3000 RGB16,20samples/path after2warmupblocks: scalar median306.765ms/p95329.902ms; table median64.701ms/p9575.392ms, median ratio4.741. All48conversion output hashes exact, final0. Source/hash/drop outside timer; source96MB outside managed output budget. Dev optimized+debug, no historical-before/after/end-to-end/GPU/RSS speed claim. Evidence: `research/p3-table-summary-2026-10-13.json`.

### PNG gAMA+cHRM and consistent ICC D65 display — 2026-10-13

Added explicit legacy PNG color from paired gAMA/cHRM: gamma1 uses native linear primaries; nonlinear gamma builds an internal ICC matrix/power profile. Higher cICP/ICC/sRGB precedence preserved; CRC/length/duplicates/zero gamma/singular chroma validated. Valid gamma1/2/3 integer declarations with paired cHRM exceed ICC exponent representation and refuse instead of silently clamping. Unpaired RGB metadata remains unqualified. Independent 5×65,536-code ramps cover gamma2/~2.2, P3, linear and arbitrary-D50 primaries; alpha/native codes exact, display cancellation/retry final0. Found and corrected ICC destination white: CMS predefined6504K approximation differed from native standardD65xy(0.3127,0.3290). Maximum authored-ramp display error now0.000032273 versus f64 references; nonlinear P3 beforefix0.000535037, linear native9.51e-8. ICC16.16 quantization remains. Final decoder1029passed/73ignored, app221passed/62ignored; actualMetal collection114sources/143selections/286storagecases all fullSDR/HDR frames/metadata exact through cache/swap/pressure/cancellation, finalrootqueue0. Verifier9tests passed, two new fixtures regenerate byte-identically. No full-PNG/producer/physicalHDR/NVIDIA claim. Evidence: `research/png-gamma-chroma-display-2026-10-13.json`; failed diagnostic/build logs retained.

### 2026-10-13: grayscale PNG gAMA without cHRM

Gray and gray-alpha IHDR declarations now admit a gAMA-only gray ICC transfer;
RGB gAMA-only remains unspecified because primary coordinates are missing.
sRGB/ICC/cICP precedence is retained. ICC-unrepresentable gamma is refused.
Full 65,536-code gray-alpha16 ramps at gamma 1, 2 and approximately 2.2 preserve
native codes and alpha; independent f64 power reference maximum error is
6.68e-7. Admitted cancellation, same-root retry and final zero memory credit pass.
Decoder library: 1031 passed, 0 failed, 73 ignored. Evidence:
`research/png-gray-gamma-2026-10-13.json`. This change adds no Metal fixture and
makes no claim of all PNG variants or original-producer interoperability.

### 2026-10-13: grayscale gAMA Metal storage qualification

Added deterministic gray-alpha16 `gamma-gray.png` to the actual Metal collection.
Its exact source codes, adjacent 16-bit levels, unchanged alpha and independent
power-2 linear reference pass before native/prepared RAM, pressure, admitted
cancellation/retry and swap checks. Every SDR/HDR comparison frame matches after
restore. Collection now has 115 sources, 144 selections, 288 storage cases;
these are fixture counts, not supported-format counts. Strict evidence verifier
passes and its 10 tests reject substituted gray-gamma sources. Evidence is in
`research/png-gray-gamma-2026-10-13.json`. Physical HDR calibration remains open.

### 2026-10-13: current cache configuration audit

Inspected application wiring for independent RAW/raster/model RAM and swap
size/count/TTL plus directional behind/ahead prefetch controls. Corrected
`--no-cache` help to describe disabling caches, swap and neighbour preloading.
Current default rrrah-cache/rrrah-memory/rrrah-swap tests: 217 passed,
0 failed, 7 ignored. Evidence: `research/cache-limits-current-2026-10-13.json`.
Manual ignored soaks and Metal require their separate evidence; this run is not
OS memory-pressure or NVIDIA qualification.

### 2026-10-13: gray PNG integer precision variants

Expanded gAMA-only public decode/display regression to gray and gray-alpha at
8 and 16 bits, each with gamma 1, 2 and approximately 2.2. All 256 or 65,536
native levels preserve source precision and alpha, match an independent f64
power reference with maximum error below 6.68e-7, and release credits after
admitted cancellation and successful same-root retry. All 12 combinations
pass. Evidence: `research/png-gray-variants-2026-10-13.json`. Packed low-bit PNG
and separate Metal fixtures for the other three variants are not covered here.

### 2026-10-13: MEF independent white-balance source audit

Inspected current RawSpeed MefDecoder source and saved a hashed snapshot in
`research/mef-rawspeed-metadata-2026-10-13.cpp.txt`. Its metadata override only
calls generic setMetaData; this file supplies no explicit as-shot WB extraction.
This does not prove no other software supports camera WB. It rules out this
specific decoder as the missing independent WB reference. Native MEF sensor
unpack evidence does not qualify camera color. Keep explicit public refusal;
seek documented MakerNote WB or original Mamiya software parameters and compare
against original camera data before enabling the viewer. Audit:
`research/mef-white-balance-source-audit-2026-10-13.json`.

### 2026-10-13: packed gray PNG gAMA/tRNS

Authored independent packed scanlines for depths 1, 2 and 4, all representable
levels, with/without tRNS, three gAMA values (18 combinations). Public decode
preserves exact expanded u8 levels and alpha; display matches f64 power
reference within 1e-6, including retained RGB at alpha zero. Final display
credit is zero. Decoder suite 1032 passed, 0 failed, 73 ignored.
Evidence: `research/png-packed-gray-2026-10-13.json`. No packed low-bit Metal
fixture, interlace or original-producer claim is added.

### 2026-10-13: coordinate-sensitive large swap and delayed HDR

Strengthened the ignored 36MP region regression: previously uniform pixels
could hide wrong source offsets; now RGB varies by x/y/parity, including signed
zero, HDR and negative values. All requested bottom-right crop samples match
source coordinates exactly after the 576MB source is released; crop2992 bytes,
scratch65536, index281280. LOD matches direct-source downsample (same algorithm,
not independent filtering correctness). Current actual run passes.
Large float delayed swap passes three 64MiB cycles in93.805 seconds with exact
bits, full-root refusal, cancellation after real admitted allocation/read,
same-key retry, physical disk caps and final zero root/queue. Delay is2ms per
max64KiB call, not physical slow-disk or OS-pressure qualification. Evidence:
`research/large-region-float-current-2026-10-13.json`.

### 2026-10-13: actual Metal swap regions at image corners

Expanded existing coordinate-varying float packed-region Metal regression from
3 centered views to 15 views: center/four corners at pixel aspects0.5/1/2.
Assert each corner rect actually touches the expected source edges. Release full
source before worker restoration, compare every64x64 SDR/HDR frame, verify
pressure/cancelled replacement preserves installed frame, and zero all owners.
Verified actual M4Max run passes. Initial pan test setup omitted viewer fit
margins and was corrected; production code unchanged. Evidence:
`research/metal-region-corners-2026-10-13.json`. Same-renderer transport only;
not independent color, physical HDR or large GPU capacity qualification.

### 2026-10-13: original PTX sample source search

Inspected current rawsamples.ch Pentax producer-sample listing, pinned response
hash and downloadable raw link inventory in
`research/ptx-original-source-search-2026-10-13.json`. No direct PTX download is
listed there; PEF/DNG candidates do not prove PTX. This is one repository only,
not evidence that PTX samples do not exist. Original sample/software location
requested while other qualification work remains possible. PTX remains pending.

### 2026-10-13: idle nonzero TTL application integration

Strengthened real RAW and profiled PNG native/prepared idle timer regressions:
previously TTL zero, now100ms. Immediate maintenance must preserve the entry
and its managed pixels; repeated idle timer callbacks spill only on expiry,
without navigation. Native/prepared serialized payload and full RAW samples/
metadata restore exactly; RAW pressure refusal/cancellation/retry and final0
remain checked. Two focused tests pass including explicitly enabled RAW case;
whole application221 passed/0failed/62ignored. Evidence:
`research/app-idle-nonzero-ttl-2026-10-13.json`. Tests exercise the actual idle
receive helper/cache integration, not a running window end-to-end.

### 2026-10-13: packed grayscale PNG actual Metal qualification

Added deterministic packed gray1/2/4-bit gAMA+tRNS fixtures to full collection.
All native/prepared pressure, admitted cancel/retry, swap and SDR/HDR frame
comparisons pass on actual M4Max Metal. Expanded u8 codes/alpha and independent
power2 reference are checked before storage. Collection118 sources/147 selections/
294 storage cases, not format counts. Strict evidence verifier accepts actual
log;11 verifier tests include refusal of substituted packed fixtures. Generator
regeneration is byte-identical. Evidence: `research/png-packed-gray-metal-2026-10-13.json`.
No interlace, original-producer or physical HDR calibration claim.

### 2026-10-13: paired segmented row reading benchmark

Replaced fixed-order3round swap row timing with7ABBA blocks, first2warmup,
10measured samples/path. Correctness compares every byte outside timing, checks
output/scratch credits each run and final index/store zero. Current APIs on
warm local filesystem8MiB/4096rows: per-range p50=204.7047495ms versus scoped
reader p50=4.696771ms (43.58x), p95nearest-rank222.679083/5.410375ms.
All28loads exact. This supports retaining the scoped reader already used by
streamed regions/LOD. It is not historical speedup, cold storage, decoder,
GPU presentation or whole-viewer benchmark. Source/comparison Vec is outside
managed accounting. Evidence: `research/swap-row-abba-2026-10-13.json`.

### 2026-10-13: 100k distinct swap keys and asynchronous teardown

Extended100k real-file churn to observe physical files and managed root/queue
after Drop. Initial immediate-zero assertion raced detached worker shutdown;
source confirms Drop signals cancellation and sender destruction wakes receiver.
Now require eventual physical files/bytes/root/queue zero within2seconds without
forcing blocking I/O cleanup onto render thread. Verified run passes100000exact
writes/restores,1000pressure refusals/retry, physical caps and evicted-key checks.
Evidence: `research/100k-swap-teardown-2026-10-13.json`. Four-byte payloads isolate
index retention, not large-image performance or OS-pressure qualification.

### 2026-10-13: full objective acceptance audit

Current100-row catalog remains94 implemented subsets,6 pending normal viewer
(PTX,MEF,RWZ,BAY,CAP,AI),0 fully qualified rows. Selected green Metal/cache
fixtures do not override catalog acceptance gaps. CUDA library exists with
native driver/PTX implementation; NVIDIA hardware remains deferred, no claim
of its execution. HDR live presentation is verified, physical color/luminance
calibration is not. Native3D is STL/OBJ/PLY/OFF subsets; wider formats/materials
are unqualified. Audit and hashed authoritative sources:
`research/goal-acceptance-audit-2026-10-13.json`. Objective stays active/incomplete.

### 2026-10-13: original AI white-probe causal recheck

Fresh current spatial diagnostic at VectorApple(78,144) captures2755events,
completes with alpha0.425108, whiteRGB252.693/239.358/217.027, peak202254128bytes,
final0. Identical source snapshot and root-lock dependency versions verified.
Matched-profile Poppler original crops at72/1152/2304dpi show72dpi white is
resolution-dependent: higher-resolution reference contains partially colored
coverage. Thus forcing this probe to white is not a justified geometry fix.
Next separate area/coverage from color integration. This diagnostic is not
production AI or full-page acceptance; strict AI pending unchanged. Evidence:
`research/ai-current-white-probe-2026-10-13.json` includes oracle means/hashes,
corrected Poppler limitation, failed initial workspace invocation and lock scope.

### 2026-10-13: AI original-probe reference convergence

Extended white-probe reference crops to64/128/256x: means converge toward current
spatial diagnostic (max difference0.765RGB8 at256x). Fresh current diagnostic
also completes two previous largest-error white probes141,36and149,56 with
partial colored coverage and final admitted0. Matched-profile Poppler256x means
are within0.566 and1.102RGB8 respectively.72dpi results are strongly resolution-
dependent. This narrows further investigation to source color integration versus
remaining mask/edge coverage at149,56; do not force white or loosen strict
whole-page acceptance. Three pixels/experimental diagnostic do not qualify
production AI. Evidence: `research/ai-localized-convergence-2026-10-13.json`.

### 2026-10-13: AI source color versus sampled coverage diagnostic

Extended test-only point grid limit4->16 and report mean alpha. On original
VectorApple149,56 all256point compositions complete, alpha53/256 versus area
0.21184744. After local coverage normalization, point-converted RGB differs
from area-converted RGB by at most0.017251RGB8. This supports edge/mask coverage
as dominant remaining local discrepancy, not a large conversion-order error;
normalization is not a general proof and grid16is not exact area. Next adjudicate
original source boundary independently. Compositor192passed/0failed/12ignored,
explicit original diagnostic completed/final0. Production unchanged; AI pending.
Evidence: `research/ai-color-integration-grid-2026-10-13.json`.

### 2026-10-13: independent original AI cubic contour area

Added pinned-source pypdf diagnostic `scripts/diagnose-ai-original-paths.py`:
walk original PDF path/CTM/form instructions, inventory conservative control-box
candidates at149,56 and independently flatten/clip individual cubic contours.
Ten main-stream candidate clips found. Path ending atoperator1181 beforeSh8
(operator1186) has converged individual area0.1291612849;512->2048 subdivision
delta2.22e-6. Full-pixel rectangles area1 and excluded contours area0; analytic
half-rectangle/no-overlap controls pass. This is individual signed contour area,
not final visibility/winding-overlap/clip-intersection/mask/shading domain oracle.
Next adjudicate Sh8 support and other contributing layers before comparing
nativealpha0.211847. Evidence: `research/ai-original-path-inventory-2026-10-13.json`.

### 2026-10-13: corrected independent AI tensor-domain area

Found diagnostic coordinate error: original CropBox top245.5, not ceil raster
height246. Both independent scripts now use pinned original page top; previous
0.129161area at149,56 is superseded, not a renderer defect. Independent original
Type7record decoding (flags0..3), cubic boundaries and GEOS union/clipping yield
Sh8area0.214461532 at2048subdivisions;512->2048delta2.18e-6. No invalid candidate
patch boundaries; all main-page axial clips exclude this pixel. Original Sh8
patch union fully contains its clip locally. Native diagnostic alpha0.211847443
is0.002614089lower. Next isolate native clip flattening versus tensor triangle
support. Diagnostic-only Shapely installation outside production; forms/masks,
folded patches/fullpage remain unqualified. Evidence:
`research/ai-original-tensor-domains-2026-10-13.json`.

### 2026-10-13: live HDR repeated after user unlock confirmation

Repeated the existing Metal HDR surface probe for raster and original RAW with
three-second presentation and live resize. Both processes exited successfully,
used RGBA16Float / ExtendedSrgbLinear and reported no validation errors. RAW
presented 146 frames in 3.009 seconds, including116 frames after resize. Logs:
`research/metal-live-unlocked-raster-2026-10-13.log` and
`research/metal-live-unlocked-raw-2026-10-13.log`. This validates presentation;
physical luminance and color calibration remain unverified. NVIDIA is deferred
at the user's request.

### 2026-10-13: AI coverage deficit isolated to captured mesh support

At original VectorApple.ai pixel(149,56), independent GEOS union of43 captured
triangles intersected with the original cubic clip gives0.2118474427430499;
native replay alpha is0.21184744274305764 (difference7.75e-15). The original
clip area is0.21446153215456878, while native clip alone is0.2144597600545532.
The missing0.0026140894115188615 area therefore comes from triangle support,
with clip flattening contributing only about1.8e-6. Current native tensor
capture requests a1/64pixel geometric bound. The next correction must improve
boundary accuracy while retaining cancellation, triangle caps and memory
admission; increasing all subdivisions indiscriminately is not validated.

Reproducible script `scripts/diagnose-ai-native-mesh-support.py` now refuses
source or original-fixture hashes that differ from its captured evidence.
Report: `research/ai-native-mesh-support-deficit-2026-10-13.json`.
Current-source compositor suite:192 passed,12 ignored,0 failed
(`research/ai-native-mesh-area-suite-2026-10-13.log`). This is a localized,
test-only diagnosis, not a production AI fix or full-page fidelity acceptance.

### 2026-10-13: boundary-cell refinement experiment refused by resource limits

Isolated source copies under /tmp/rrrah-ai-boundary-probe-20261013 tightened
only source-boundary-touching cells from1/64pixel to1/4096pixel, retaining the
interior tolerance, cancellation, maximum depth12,1,048,576triangle cap and
256MiB diagnostic admission. Original VectorApple.ai capture could not complete:
1455-patch shading refused TriangleLimit;323-patch shading refused Admission.
The ignored diagnostic failed its captured-space assertion; no output accuracy
claim is made. Logs `research/ai-boundary-refinement-experiment-2026-10-13.log`
and `research/ai-boundary-refinement-refusal-2026-10-13.log` retain the failures.
Production sources were unchanged. This rules out indiscriminate boundary-cell
quadtree refinement at that accuracy under existing limits. Next approach:
refine shared cubic edge samples independently and use existing boundary fans,
without subdividing all boundary-adjacent cells.

### 2026-10-13: independent shared-edge refinement reduces AI area error

In isolated source copies, added dyadic samples along canonical shared cubic
edges using control-point-to-chord deviation, leaving interior leaves unchanged.
Existing boundary fans consume the extra samples. Memory admission/cancellation,
maximum depth12 and triangle cap remain enforced.1/4096pixel boundary tolerance
refused Admission.1/256pixel completed with alpha0.2139417204790806 and peak
216466640managed bytes.1/1024pixel completed with alpha0.2144258053457193 and
peak239197736bytes, final credit0, no deadline,7.40seconds. Against independent
original area0.21446153215456878, error fell from0.0026140894115 to0.00003572680885
(about73times smaller). Default compositor suite192passed,12ignored,0failed.

Comparison: `research/ai-edge-refinement-comparison-2026-10-13.json`; complete
experimental source snapshot `research/ai-edge-refinement-sixteenth-2026-10-13.rs.txt`.
Production remains unchanged: adoption still requires focused adaptive-mesh
budget/cancellation/shared-edge regressions and wider original-page evaluation.

### 2026-10-13: shared-edge accuracy correction adopted and verified

Working vendored adaptive mesh code now refines canonical cubic edge samples
independently with chord tolerance pixel_error/16, preserving existing interior
leaves and exact shared point ownership under reversed parameter direction.
New samples use admitted arrays, bounded depth-first stack, cancellation and
existing depth/triangle limits. Only interior edge endpoints are appended once;
source endpoints already exist in planned leaves.

Added an independent analytic area regression: a cubic bowed boundary has exact
area64.4. Removing refinement makes it fail at64.39375000000001. Added curved
reversed-shared-edge bit equality. Existing exhaustive allocation-refusal and
checkpoint-cancellation tests now include curved source edges. Full interpreter
suite115passed; compositor192passed/12ignored. Original three-pixel capture
completed without deadline, final credit0, peak239197736managed bytes. At149,56,
alpha0.2144258053457193 reduces original-area error about73times; other two
probes retain their previous coverage. Evidence:
`research/ai-edge-current-qualification-2026-10-13.json` and
`research/ai-edge-area-regression-before-2026-10-13.log`.

This corrects working native experimental capture, not normal-viewer AI
acceptance. Full-page fidelity remains open. Baseline mesh-support diagnostic
now pins the original adaptive-source hash too, refusing reuse against this
changed geometry rather than attaching old measurements to new code.

### 2026-10-13: strict full-page spatial diagnostic exposes pre-existing work exhaustion

Added explicit RRRAH_AI_SPATIAL_PAGE_OUTPUT mode to the ignored original AI
spatial diagnostic. Output capacity is admitted; PPM is created exclusively only
after all pixels complete. First refused pixel aborts traversal, final retained
credit is checked, and the test fails instead of presenting partial output as a
qualified page. No production rendering route or acceptance threshold changed.

Original301x246page processed7724/74046pixels before refusal at(199,25), after
56.53seconds, well before180second deadline; final credit0 and no PPM published.
A localized instrumented rerun reports remaining_work=0,cancelled=false at
spatial composition.27,314fragments and repeated group/sample evaluation consume
the existing4,000,000work budget. Reconstructing the pre-edge-refinement source
in an isolated copy reproduces the same pixel/centroid refusal, confirming this
is pre-existing, not introduced by the accuracy correction. Next work: reuse
composition evaluation across equivalent fragments rather than raise the budget.
Logs: `research/ai-spatial-full-page-2026-10-13.log`,
`research/ai-spatial-first-refusal-budget-2026-10-13.log`,
`research/ai-spatial-first-refusal-baseline-2026-10-13.log`.
Full-page fidelity and normal-viewer AI remain pending.
Current compiled diagnostic binary default suite:192passed/12ignored/0failed,
`research/ai-spatial-page-diagnostic-suite-direct-2026-10-13.log`. A redundant
Cargo invocation blocked on another project's package-cache lock was stopped;
the already compiled current-source binary ran directly, avoiding interference
with other active builds.

### 2026-10-13: exact single-opaque-paint group fast path

Working test-only spatial compositor now recognizes an inherited-space Normal
group containing exactly one retained opaque Normal paint after route coverage
has been proved. The opaque paint removes all child-backdrop dependence: apply
its prepared source directly with group opacity/mask and AIS shape semantics,
without child allocation, child-backdrop comparison or an End dispatch. Other
source alpha, blend modes, explicit group spaces and multiple paints retain the
ordinary path. Budget remains4,000,000; cancellation remains checked.

Added144case matrix comparing the optimized group with two identical opaque
paints, which force ordinary child-frame execution. Covers parent/child knockout,
child isolation, AIS, group opacities0/.4/1 and masks0/.3/1. Components and alpha
agree within1e-12. Suite193passed/12ignored/0failed
(`research/ai-opaque-group-suite-2026-10-13.log`). Original(199,25) still refuses
at work0, but reaches40,493fragments rather than27,314; group creation falls
from412,720 to119,617 despite processing more fragments. No pixel completion or
full-page qualification claimed. Log `research/ai-opaque-group-fastpath-2026-10-13.log`.

Exact winding-vector epoch reuse was also tested, produced identical work counts
and was removed; its logs are retained as rejected optimization evidence in
`research/ai-route-epoch-original-2026-10-13.log`. Next bottleneck is route-key
construction and repeated native sampling over dense fragment arrangements.

### 2026-10-13: incremental coverage bits remove first full-page work refusal

Spatial cell composition now uses the polygon row's existing incremental winding
selector. At slab initialization it constructs exact NonZero/EvenOdd membership;
a crossed edge updates only that region's saved winding and bit. Route membership
and key construction use masked64-bit words rather than repeated per-region
classification. Native samples and masks continue evaluating at each centroid;
no color approximation or geometric tolerance was introduced. Incremental
updates debit polygon work; point-mode bit construction debits shared paint work.

Added129region regression crossing bit indices0/64/128, coincident edges,
EvenOdd overlap and reversed NonZero contours; exact expectedRGB(.5,.5,0),alpha1.
Full suite194passed/12ignored/0failed, including existing cancellation/admission
and exact mask-budget boundary tests. At original(199,25), routing-key work falls
1838752->75770, and pixel completes within unchanged4Mpaint budget. Three prior
pixel results remain identical. All retained credit releases.

Strict full-page traversal advances7724->10038of74046pixels, then refuses at
(105,33), before deadline; no partial PPM published. Independent original-source
matched-profile Poppler crops at64x/256x reveal another open issue: completed
(199,25) RGB differs by up to12.184566at256x. Thus successful execution does not
qualify color fidelity. Next: diagnose new refusal and original pixel color.
Evidence `research/ai-incremental-routing-qualification-2026-10-13.json` and
`research/ai-first-refusal-color-comparison-2026-10-13.json`.

### 2026-10-13: second refusal and color-oracle correction

Localized original(105,33) refusal occurs while preparing53polygon regions:
active_edges=4097 exceeds the existing4096edge cap. Paint work remains3999386,
polygon work39933454, cancellation=false. This is geometry occupancy, not
paint-budget exhaustion. Evidence `research/ai-second-refusal-geometry-2026-10-13.log`.
Next investigate exact duplicate/shared contours before changing capacity caps.

Correction: the previously quoted12.184566RGB8difference at(199,25) used an
invalid256x oracle image. Poppler exited zero but logged `Bogus memory allocation
size` twice. The original comparison report now explicitly invalidates that
measurement. Diagnostic copy of the original page with only CropBox changed to
[199,219.5,200,220.5] preserves original resources/content and leaves source AI
unchanged. It reproduces valid64x and128x crops, while256x still errors: crop
canvas alone does not eliminate the source allocation limit.64x/128x means
differ by1.787109375RGB8;128x/native differs by11.6170306802RGB8, still an
unconverged diagnostic discrepancy, not full-page qualification.

Reproducible `scripts/diagnose-ai-cropped-pixel-color.py` checks process exit,
stderr and exact image dimensions; invalid samples have no mean and cause exit1.
An explicit64,128,256run verifies rejection of the failing oracle while retaining
valid lower scales. Report `research/ai-cropped-pixel-color-verified-2026-10-13.json`.

### 2026-10-13: contour sharing rejected; left-of-pixel winding dominates occupancy

Tested both retained-object identity and exact PathEl sequence equality with
separate alias winding/fill states. Authored shared-contour case and existing
suite passed, but original(105,33) has52distinct contours and no exact aliases;
(199,25)64distinct, (149,56)95distinct. No original failure reduction. Removed
the canonicalization and its quadratic comparisons from working code. Complete
experimental snapshot retained at
`research/ai-exact-contour-experiment-2026-10-13.rs.txt`; logs
`research/ai-canonical-path-original-2026-10-13.log` and
`research/ai-exact-contour-original-2026-10-13.log`. No contour approximation was
adopted, and the prior incremental coverage optimization remains intact.

Extended gated geometry-refusal diagnostics: edge4097 belongs to region47,
path2113elements;2867of the first4097active edges are wholly left of pixel105.
Those edges affect initial winding, not interior geometry, so cannot simply be
dropped. Next investigate exact per-region vertical winding-event compression
for wholly-left segments, retaining interior segments and alias fill semantics
under current admission/cancellation/work limits. This is a causal diagnosis,
not an implemented fix. Evidence
`research/ai-second-refusal-left-edges-2026-10-13.log`.

### 2026-10-13: left-chain winding compression resolves second pixel refusal

Pixel-row preparation now replaces each continuous chain strictly left of the
pixel with one vertical edge at the left border connecting the chain's endpoint
Y coordinates. Oriented half-open crossings telescope even with nonmonotoneY;
closed left-only contours contribute zero. All interior/crossing source segments
remain unchanged. Compression uses only bounded stack state and the existing
admitted output arrays, cancellation, work and4096edge cap. A prepared pixel row
now explicitly refuses requests outside that original pixel/subrectangles;
general reusable row preparation is unchanged.

Added eight authored cases with6000oscillating left edges, original/reversed
orientation, one/two contours and NonZero/EvenOdd. Ordinary uncompressed row
hits its unchanged edge cap; pixel-specific preparation retains<=4edges/contour
and exact expected area(.5 or0). Exhaustive preparation allocation-refusal and
cancellation checkpoints release all credit and allow retry. Full compositor
suite196passed/12ignored/0failed. Original(105,33) now completes under unchanged
limits; all five requested pixels complete, final credit0, peak239197736bytes.
Across four prior probes, maximumRGB8difference1.99e-13 and component difference
3.07e-10. Evidence `research/ai-left-chain-qualification-2026-10-13.json`.
Full-page diagnostic is run separately; local completion is not full-page AI
fidelity or normal-viewer acceptance.
Strict full-page rerun completes10372/74046pixels in57.58seconds, next refusal
(138,34), before deadline; final credit0, no PPM published. Localized follow-up
reports shared paint work0 after21527fragments,86108group creations and62571
native samples. Only3route misses, so the next bottleneck is per-fragment native
group/sample evaluation rather than route discovery. Logs
`research/ai-spatial-left-chain-page-2026-10-13.log` and
`research/ai-third-refusal-localized-2026-10-13.log`.

### 2026-10-13: indexed source-owner lookup resolves third pixel refusal

Native spatial paint now prepares admitted sparse owner-word records once per
command. Current exact coverage bits intersect candidate masks; only covered
bits are visited, selecting maximum original source index. Descending-priority
validation remains; duplicate region entries preserve the greatest source index.
Fill rules and centroid sampling are unchanged. Plans and outer metadata are
admitted before allocation, construction/query debit shared work, and all owners
release on refusal/cancellation.

Added independent source-path winding priority regression at region indices
1/64/128, overlap, duplicate owners and EvenOdd exclusion. Exhaustive plan
allocation-refusal and cancellation checkpoints release all credit and allow
retry. Suite198passed/12ignored/0failed. Current-source original probes including
(138,34) complete within4Mwork and256MiB limits, prior probeRGBunchanged, final
credit0. Evidence `research/ai-owner-bitset-qualification-2026-10-13.json`.

Strict full-page run reaches10373/74046pixels, next refusal(139,34),52.57seconds,
no deadline and no partial image. Localized case has696regions,72612fragments,
215459native samples,3route misses and shared remaining work2. Native-source
work2292134, key construction806670. Next optimize sparse routing-word checks;
current routing still scans zero words across a dense region signature.
Logs `research/ai-spatial-owner-bitset-page-2026-10-13.log` and
`research/ai-fourth-refusal-localized-2026-10-13.log`.

### Sparse routing words and neutral AIS groups (2026-10-13)

Routing traverses only occupied coverage words. Mask-free opacity-one Normal
AIS groups flatten only under the existing safe inherited-space, nonisolated,
non-knockout conditions. Added highest-word coverage and sampled HardLight
parent regressions, and strengthened shared mask-budget refusal checks.
Suite: 200 passed, 12 ignored. Five original probes complete; four previously
completed probes retain identical native components and RGB. Pixel (139,34)
now completes within unchanged limits. Strict full-page run completes
10974/74046 pixels, refuses at (138,36), without deadline or partial output.
Evidence: research/ai-sparse-neutral-group-qualification-2026-10-13.json.

At (199,25), converting individual composited point samples before averaging
gives RGB (78.68,151.60,70.45) at 4x4 and (79.66,152.00,70.42) at 16x16,
compared with native area RGB (81.61,151.67,70.54). This does not resolve
the valid 128x Poppler crop discrepancy (90.78,163.28,63.80). Convergence
and cause remain unproven. The erroneous 256x oracle remains rejected.
Evidence: research/ai-point-grid-color-comparison-2026-10-13.json.

### Localized next AI refusal (2026-10-13)

Current compiled source reproduces (138,36) refusal with no deadline and
final managed credit zero. Of 1013 regions only three influence routing;
84760 fragments use one route. Native-source evaluation spends 3150074
work units versus 169520 key work, 17 lookup and 77 route preparation.
254278 native samples precede refusal. This rules out route lookup as the
main next target at this pixel. Separate owner selection, triangle sampling
and native color-coordinate cost before changing those paths.
Evidence: research/ai-sixth-refusal-localized-2026-10-13.json and matching log.

### Source-cost split and rejected priority-word experiment (2026-10-13)

Optional spatial trace now separately measures owner lookup (1441000 work)
and sampling/native coordinates (1709074 work) at (138,36), preserving all
budget debits. Owner-word maximum-priority ordering was tested on six original
probes: five prior results remain identical, target still refuses, fragments
84760 -> 84896 (0.16 percent). Removed this optimization as insufficient benefit.
Retained independent winding regression for nonmonotonic source priorities
across words. Final suite 200 passed, 12 ignored, zero failed; diff check clean.
Evidence: research/ai-source-cost-qualification-2026-10-13.json.

### No-owner sampling cost and rejected mesh shortcut (2026-10-13)

At (138,36), no-owner sampling consumes 1202014 work units of 1709074
sampling/native-coordinate work. A certified outside-mesh background shortcut
was tested, preserving five prior original probes exactly, but target remains
incomplete at 85082 fragments versus 84760 (0.38 percent improvement).
Removed the shortcut API and caller changes; retained only optional cost tracing.
Final compositor suite: 200 passed, 12 ignored, zero failed. Next attribution
must distinguish native source events, especially axial/function evaluation;
no-owner work is not equivalent to outside-mesh search work.
Evidence: research/ai-no-owner-cost-qualification-2026-10-13.json.

### Native nearest-neighbor raster last-sample cache (2026-10-13)

Per-source measurement identifies two nearest-neighbor raster events 1218/2298,
each spending about 593313 work units in the former (138,36) refusal. Added
one-entry sample caches scoped to each source replay, keyed separately by
color and alpha texel coordinates. Color or alpha interpolation bypasses cache.
Cache lookup charges work and polls cancellation; misses preserve original
sampling and validation. Raster sampler admission allowance is now 256 bytes
per source (was 128), with actual closure-size check against that allowance.

Pixel (138,36) now completes within unchanged 4M paint-work/256MiB limits,
98335 fragments; each raster spends 123010 work over 98335 samples. Five prior
original probes retain identical RGB and native components. Peak managed
239197736 bytes, final credit zero. Regression covers changing color texels,
16-bit alpha, unequal color/mask dimensions, clamping, cancellation, budget
exhaustion, nonfinite coordinates and interpolation bypass. Suite 202 passed,
12 ignored, zero failed. Strict full-page run completes 11274/74046 pixels,
next refusal (137,37), no deadline or partial image; 63.94 seconds.
Normal-viewer AI integration and independent color discrepancy remain open.
Evidence: research/ai-raster-last-sample-qualification-2026-10-13.json.

### Pixel-specific triangular-owner edge capacity (2026-10-13)

Localized (137,37): 1473 source regions hit the old 4096-edge preparation
cap before any fragment evaluation. Pixel-only preparation now admits
max(4096,3*region_count) edges, at most12288; general reusable row remains
capped4096. Shared polygon40M/paint4M work, 256MiB memory and1M crossing
limits unchanged; actual edge storage still admitted before allocation.
This expands bounded geometry capacity, not a measured speed optimization.
Regression:1500 triangular regions/4500activeedges, analytic area0.225,
ordinary row refusal, low-work/cancel/admission refusals. Suite203passed,
12ignored,0failed. Original (137,37) passes preparation but exhausts paint
work after91936fragments. Prior probes(138,36),(199,25),(149,56) match all
native components, alpha andRGB exactly; final credit zero. Full page
therefore remains incomplete at(137,37); no repeated full-page run needed.
Evidence: research/ai-pixel-edge-capacity-qualification-2026-10-13.json.

### Occupied coverage-word routing for source owners (2026-10-13)

Incrementally track which of the64coverage words are nonzero, intersect with
each source-owner plan word mask, and inspect only those words. Plan position
metadata included in existing sizeof-based admission; polygon update cost
raised4to6 to account for mask maintenance. Point mode initializes occupancy
in its existing charged coverage loop. Exact source-index priority unchanged.
Independent winding regression expanded to region4094/word63 entering/leaving
coverage, nonmonotonic priorities and duplicate owners. Suite203passed,
12ignored,0failed, including admission/cancellation/refusal coverage.
At(137,37), fragments before paint-work exhaustion91936->137183. Owner work
per fragment decreases about57percent; four prior original probes match
alpha/native components/RGB exactly. Peak239197736managed bytes, finalcredit0.
Target and full page remain incomplete; no repeated full-page run needed.
Evidence: research/ai-occupied-owner-words-qualification-2026-10-13.json.

### Rejected per-word selected-owner cache (2026-10-13)

Tested memoizing each owner word maximum against exact covered-bit pattern.
At(137,37), fragments before refusal137183->137664, only0.35percent benefit;
removed mutable cache fields and lookup changes. Retained stronger independent
winding regression with multiple changing source-owner bits in one word.
Final suite203passed/12ignored/0failed; diff check clean. Next design should
propagate changed words and update a bounded source maximum, avoiding complete
occupied-word traversal per fragment. Full-page completion remains open.
Evidence: research/ai-owner-word-cache-rejected-2026-10-13.json.

### Rejected incremental selected-owner tree (2026-10-13)

Tested dirty-word propagation plus a64-leaf max tree per source, admitted
metadata and charged every propagation/update. At(137,37), completed
fragments drop137183->111728 under same paint limit (18.55percent worse).
Removed tree, dirty-word state and propagation changes. Retained independent
winding regression for a source skipped across a middle clip interval then
visible again with different owner. Final suite203passed/12ignored/0failed.
Next candidate should track added/removed owner bits against selected owner
without maintaining a full tree. Full-page completion remains unresolved.
Evidence: research/ai-owner-tree-rejected-2026-10-13.json.

### Rejected incremental added/removed owner selection (2026-10-13)

Tested retaining a still-covered selected owner, checking added owner bits,
and refreshing when that owner disappears. Dirty changes persist across
clipped-out source intervals; independent priority/cancellation tests pass.
At(137,37), fragments before refusal137183->90540 under same budget, so
removed dirty-word propagation, selection cache and metadata. Experimental
source archived only as research text. Final suite203passed/12ignored/0failed.
Next inspect source-order triangle storage and certified monotonic owner
indices to enable direct highest-covered-bit queries. Full goal remains open.
Evidence: research/ai-owner-add-remove-rejected-2026-10-13.json.

### Stable source-order triangle regions and certified maximum lookup (2026-10-13)

Triangle regions are inserted in source-index order using admitted temporary
index storage and bounded sorting. Region deduplication previously reordered
representatives by hash, defeating this order; it now retains first input
representative order and remaps requirements using its existing admitted
buffers. Owner plans verify monotonic source-index order explicitly; only
proven plans select the highest covered bit directly. Nonmonotonic mappings
retain the general maximum lookup. Every certificate query/construction
continues charging shared work and checking cancellation.

Strengthened canonical-region test verifies preserved order and requirement
remapping; existing independent owner-winding tests cover monotonic and
nonmonotonic mappings, duplicate owners, word63 and skipped source intervals.
Suite203passed/12ignored/0failed. Original(137,37) now completes170483
fragments within unchanged4Mpaint/40Mpolygon work and256MiB memory. Four
prior probes differ at most1.4211e-13RGB and3.3307e-16native components,
consistent with changed floating summation order. Finalcredit0, peak239197736.
Strict page reaches11332/74046pixels, next refusal(195,37),60.76seconds,
no deadline or partial image. Normal-viewer and independent color remain open.
Evidence: research/ai-stable-ordered-owner-qualification-2026-10-13.json.

### AI refusal(195,37) and rejected larger route cache (2026-10-13)

Current pixel has72regions/58routing regions, no triangle-owner work.
At98587fragments it exhausts paint work;192route misses, sourceevent1023
uses1084446sampling and98586coordinate work, raster1218uses137471.
A256-route/512-slot cache for small-command streams with>64regions reaches
99878fragments/169misses, still incomplete (1.31percent improvement).
Removed cache-capacity/index changes. Sparse-scene allocation regression
remains protected; final suite203passed/12ignored/0failed. Next inspect
axial function evaluation and repeated group-reuse checks rather than cache size.
Evidence: research/ai-eighth-refusal-route-capacity-2026-10-13.json.

### Immutable Type3 interval certificate (2026-10-13)

Type3 now certifies native internal bounds ordering and finite domain once
at construction. Legacy parsing behavior retained; invalid certificates
refuse native evaluation. Removed repeated full-bound scans in scalar and
component evaluators; interpolation, boundary interval choice, output clamp
and cancellation semantics remain. Type metadata accounted by existing
sizeof-based native retention admission. Cancellation tests enumerate actual
checkpoints, and minimal shared-work test updated for removed scanning work.
Interpreter115passed; compositor203passed/12ignored/0failed. Four prior
original probes match alpha/native components/RGB exactly, finalcredit0.
At(195,37), axialevent1023 source work per sample11->7, fragments before
paint refusal98587->109437. Target remains incomplete under unchanged4Mwork;
no repeated full-page run needed. Independent color remains unqualified.
Evidence: research/ai-stitch-bound-certificate-qualification-2026-10-13.json.

### Prepared Normal paint batch primitive (2026-10-13)

Added test-only native frame primitive representing a Normal paint sequence
by its composed premultiplied pixel and independent accumulated shape.
Preparation requires transparent initial backdrop and non-knockout frame;
application requires non-knockout matching destination coordinates. Proof
covers36RGB combinations (AISfalse/true, opacity0/.4/1, backdropalpha0/.3/1,
two backdrop colors), comparing native color, pixel alpha, shape and alpha
coverage within1e-12. Knockout and cancellation refuse. Suite204passed,
12ignored,0failed. This is groundwork only: AI routing does not use it yet.
Next proveCMYK/Gray, add admitted cache and integrate route grouping, then
measure original(195,37). Full-page/color qualification remains open.
Evidence: research/ai-normal-batch-proof-2026-10-13.json.

### Normal batch Gray/CMYK proof and rejected adjacent route integration (2026-10-13)

Extended prepared Normal batch equivalence to native Gray andCMYK spaces
captured from authored PDF raster sources:108combinations across3models.
Color, alpha, shape andalpha coverage agree within1e-12; knockout andcancel
refuse. Tested route cache for strictly adjacent single-opaque-paint Normal
groups with no mask andnon-knockout parent. Original(195,37) remains at
109437fragments with identical dispatch counts: this restricted batching
does not benefit that fixture. Removed route integration/cache metadata
and accumulator helpers; proof primitive/tests retained. Sparse allocation
regression stays unchanged. Final suite205passed/12ignored/0failed.
Next inspect actual route nesting before broadening certified batching.
No AI speedup or full-page completion claimed from this proof alone.
Evidence: research/ai-normal-batch-gray-cmyk-qualification-2026-10-13.json.

### Knockout full-shape sibling overwrite (2026-10-13)

Route properties reveal the constant fade groups are inside a knockout
parent; this explains why the prior non-knockout batch integration could
not apply (previous commentary about separation was incomplete). Extended
existing overwrite pruning: an independent opaque Normal child withAISfalse
has full shape even at reduced group opacity, and erases earlier siblings
inside a knockout parent. Global soft-mask exclusion remains. AIStrue and
ordinary parents retain previous rules. Reduced-alpha replacement explicitly
does not mark the parent as an opaque independent source, preserving inherited
backdrops; existing authoredCMYK fade regression catches that distinction.

Expanded sibling test includes opacity0, and a new guard test covers parent
knockout/AIS combinations. Suite206passed/12ignored/0failed. Original(195,37)
now completes130473fragments, with7041hidden commands across prepared routes.
Four prior probes match RGB/native components exactly; finalcredit0,
peak239197736bytes. Strict page reaches11877/74046pixels, next refusal
(138,39),61.71seconds, no deadline or partial output. Normal-viewer and
independent color qualification still open.
Evidence: research/ai-knockout-full-shape-qualification-2026-10-13.json.

### Single-native-source group execution fusion (2026-10-13)

Localized (138,39): 257898 fragments exhaust the unchanged 4M work budget,
with two raster-source groups and 1645479 route dispatches. An attempted
opaque-Normal direct group exit gives no progress; source snapshot and
207-pass experiment evidence are archived, runtime shortcut reverted.

Adopted route fusion for Begin/NativeSampledPaint/End. It retains the exact
child frame creation, native source preparation, source blend/shape, parent
mask and group exit, while avoiding stack push/pop and two route dispatches.
Shared bounded source resolution preserves owner selection, cancellation,
work debit and refusal of budget minting. No larger work/memory/cache limits.
A 2304-case native-vs-general-sampled matrix covers parent/child knockout,
isolation, AIS, source alpha0/.4/1, opacity0/.4/1, masks and group/source blend
modes. Exhaustive admission failure/cancel polling, source failure/absence,
and exact retry release all credit. Suite208passed/12ignored/0failed.

Original (138,39) completes279623fragments; five prior native/RGB/alpha
probes match exactly. Strict page advances11877->14022 of74046pixels,
next refusal(176,46), deadlinefalse,95.94seconds,peak239197736,finalcredit0.
No partial page published. This is experimental AI replay progress, not
normal-viewer integration or independent color qualification. Route dispatch
reduction is verified; no paired wall-clock speedup claim.
Evidence: research/ai-fused-native-group-qualification-2026-10-13.json.
Localized next(176,46):65regions,111206fragments,179route misses;
native source work1899571 (axial events1023/2097 plus raster1218),
route lookup840216,preparation156314. Deadlinefalse/finalcredit0.
Next inspect axial sampling and route lookup rather than blindly increasing
cache or per-pixel work limits. Log:research/ai-tenth-refusal-localized-2026-10-13.log.

### Coverage route hash distribution (2026-10-13)

At (176,46), route lookup costs840216 work units before refusing111206
fragments. Add a charged3-unit 64-bit avalanche after coverage-word hashing,
before selecting the power-of-two table bucket. Full key equality remains
mandatory for collisions. Cache64/index128/work4M/admission256MiB unchanged.
Lookup now417109units, route preparation156314->104686, and the original
pixel completes113816fragments. Six prior probes match alpha/components/RGB
exactly. Strengthened eviction test64->96routes, crossing words and the64route
capacity with independent analytic color/coverage expectations. Lazy-mask
fixture budget36->39 accounts exactly for charged hash finalization; unused
mask still never sampled, live mask once per point. Suite208passed/12ignored.

Strict full-page replay advances14022->15195 of74046pixels, next refusal
(145,50),deadlinefalse,97.34seconds,peak239197736,finalcredit0. No partial
page published. This demonstrates route lookup reduction and wider native
AI replay completion, not paired wall-clock speedup, normal-viewer integration
or independent color qualification.
Evidence: research/ai-route-hash-qualification-2026-10-13.json.
Next(145,50) localized:760regions,219616fragments,only2route misses;
native source work2630503, owner lookup638095, mesh event2670 dominates
sampling. Next inspect mesh evaluation/owner routing, rather than route cache.
Deadlinefalse/finalcredit0. Log:research/ai-eleventh-refusal-localized-2026-10-13.log.

### Rejected mesh channel certificates; measured index loading improvement (2026-10-13)

Two channel-certificate experiments (zero-only and arbitrary constant input
channels) both leave(145,50) at219616fragments, event2670source1300565work.
Metadata raises peak239197736->246653464bytes. Seven prior probes exact;
experimental source suites116/117passed and compositor208passed. Both
certificates and added tests reverted byte-for-byte; source115/compositor208
restoration gates pass. Archived snapshots/logs/hashes:
research/ai-mesh-channel-rejected-2026-10-13.json.

A3second/5ms CPU sample covers initial capture/index construction, not later
pixel replay. Native mesh quicksort partition has225collapsed top-of-stack
samples of448active-thread samples; test harness main thread is parked.
Adopt median selection instead of fully sorting every index subtree. Unique
center/source-index comparator preserves median partitions; leaf traversal
order is permitted to change, original source-order winner remains explicit.
No new metadata/allocation/capacity. Source115/compositor208passed; seven
original native/alpha/RGB probes exact, managedpeak239197736/finalcredit0.

Serial ABBA2blocks,2excludedwarmups,8measuredprocesses on originalAI with two
fully completed pixels per process: block ratios1.81290/1.73512, median
baseline/candidate1.77401. Individual-run medians5.96307s->3.31633s. All20
pixel checks exact and finalcredit0. This measures capture/preflight plus two
selected pixels, not full-page/general viewer throughput. Reusable guarded
benchmark:scripts/bench-ai-mesh-index.py.

The work-budget refusal(145,50) remains219616fragments; no new full-page
completion claim. Last verified page prefix15195/74046. Independent color,
normal-viewer AI integration and remaining format/hardware scope stay open.
Evidence:research/ai-mesh-median-qualification-2026-10-13.json.

### Composition profiling, diagnostic deadline, and child buffer reuse (2026-10-13)

Post-preflight CPU sample reveals diagnostic clock overhead:238collapsed
mach_absolute_time samples of452active-worker samples. Replace per-check
Instant::elapsed with a180s watchdog AtomicBool, still checked at every native
checkpoint. Drop stops/unparks/joins watchdog, including panic. Regression
covers zero deadline expiry and dropping a180s timer without waiting. This is
measurement correction, not a production speedup. Reprofile after preflight:
447active samples, main+watchdog894parked samples excluded; allocations/free
are prominent (63free,37tiny_malloc,26allocator-deduplicated,15zone_malloc).

Reuse one admitted one-pixel child across fused single-native groups. Reset
backdrop using actual parent knockout state, initial/current pixels, coverage,
child knockout and space key; failed/cancelled state refuses, space/buffer
mismatch reallocates. Release scratch before ordinary nested group allocation.
Existing2304-case equivalence matrix and exhaustive allocation/cancel tests
now span two fragments, exercising reused state. New painted-then-absent-source
regression proves no prior paint leaks. Suite210passed/12ignored/0failed.
Seven prior original alpha/native/RGB probes exact. At(145,50), admission
requests2855050->1098130; work refusal remains219616fragments. Peak239197736,
finalcredit0, all limits unchanged.

Guarded benchmark now records parent-observed preflight marker, with60s
subprocess watchdog and independent capture/replay phases. Both executables
use identical atomic deadlines. Serial ABBA2blocks/2excludedwarmups/8measured
runs: replay ratios1.37757/1.37421, median1.37589; replay medians375.013ms->
272.144ms for two completed native pixels. Overall block ratios1.05810/1.04940.
20exact pixel checks/finalcredit0. Replay phase includes conversion, teardown
and process exit; no normal-viewer or universal throughput claim.

Strict page still15195/74046,next(145,50),deadlinefalse,peak239197736,final0,
45.51seconds, no partial output. Page timing also includes diagnostic clock
correction, so is not an isolated buffer speed comparison. Full AI completion,
independent color and remaining broader format/hardware requirements stay open.
Next inspect root/stack per-fragment allocations and mesh ownership/integration.
Evidence:research/ai-fused-child-reuse-qualification-2026-10-13.json.

### Root pixel frame and execution stack reuse (2026-10-13)

Retain root storage between fragments, validate single-pixel completion without
consuming its buffers, and reset initial/current/backdrop/coverage/knockout/key
to fresh root state. Failed/cancelled state refuses; incompatible space/shape
releases old storage before readmission. New18case regression compares fresh
state and subsequent blending, cancellation-preserved state, failed and
multi-pixel rejection. Suite211passed/12ignored/0failed; existing multi-fragment
2304case equivalence, admission/cancel and painted-then-absent-source gates pass.

Execution stack Vec and its credit are admitted lazily once; enforce empty
state between fragments and allocated capacity<=maximum+1. Debit allocation
work once, reflecting the eliminated actual repeated allocation. All limits
unchanged (4M pixel work/256MiB admission/64routes/128index slots). At(145,50),
admission requests1098130->219670(root only)->55(root+stack), fragments before
refusal219616->233053. Seven prior original native/RGB/alpha probes exact;
peak239197736/finalcredit0.

Serial ABBA2blocks/2excludedwarmups/8measuredruns/20exactpixel checks:
replay ratios1.31972/1.20902,median1.26437; replay medians306.748ms->232.474ms.
Both versions use the same atomic deadline. Capture/preflight separated by
parent-observed stdout marker; replay includes conversion/teardown/process exit.
Overall ratios1.04141/0.95875 do not establish overall process speedup.

Strict page remains15195/74046,next(145,50),deadlinefalse,48.73seconds,
peak239197736/finalcredit0,no partial output. Root/stack allocation improvement
is verified, not full AI rendering or independent color qualification. Next
refusal spends2779853units in native source, including677560owner lookup and
2102293sampling/coordinates; focus shifts to mesh ownership/integration.
Evidence:research/ai-root-stack-reuse-qualification-2026-10-13.json.

### Native CMYK interpolation lane batching (2026-10-13)

Owner diagnostic at(145,50):727entries/12words/monotonictrue; existing ordered
maximum lookup already applies. Add trace-only plan metadata, then focus on
native field sampling. Share scalar helper for mesh component interpolation;
count4 uses fixed independent f64 lanes accumulated in original vertex order
from-0.0 (Rust Sum identity). Gray/RGB/function-input counts keep scalar path.
No precision reduction, reassociation, geometry, work debit, heap allocation
or retained metadata change. Safe Rust, no added dependencies or intrinsics.

3072scalar bit-reference cases include negative zero, minimum subnormal,
minimum normal and1. Source116passed/1explicitbenchmarkignored; compositor
211passed/12ignored. Seven prior original native/RGB/alpha probes exact.
Synthetic bounded selected-triangle API benchmark:5Mvarying points/run,
unchanged25Mwork debit and exact aggregate checksums. Initial ABBA4block
median1.09549, one block0.99264; independent confirmation4blocks median1.11459.
Combined8blocks/32measuredruns/4excludedwarmups:median1.11459,7/8positive,
range0.99264..1.16936. This is a modest CMYK sampling kernel gain, not general
viewer/page throughput. Reusable benchmark:scripts/bench-native-mesh-interpolation.py.

Strict originalAI page remains15195/74046,next(145,50),deadlinefalse,
45.88seconds,peak239197736/finalcredit0,no partial page published. Target
work refusal still233053fragments. Independent color and normal-viewer AI
qualification remain open; the broader100format/GPU/cache goal is incomplete.
Evidence:research/ai-mesh-interpolation-qualification-2026-10-13.json.

### 2026-10-13: live Metal HDR repeat after ready reply

Repeated live surface probes after explicit user readiness: raster 179 frames in3.014s,149 after resize; native RAW153 frames in3.010s,123 after resize. Both exited0 with confirmed resize, RGBA16Float/ExtendedSrgbLinear, display headroom16 and no validation errors. Logs: `research/metal-live-ready-raster-2026-10-13.log`, `research/metal-live-ready-raw-2026-10-13.log`. Presentation verified; physical luminance/color calibration remains unqualified. NVIDIA deferred by user.

### 2026-10-13: native AI coordinates validate once

Experimental spatial mesh replay skips redundant native coordinate copy/validation when blending_model is native; component counts still checked, Pixel::from_native validates finite/range/alpha, cancellation retained. Special spaces retain bounded conversion. Suite211passed/12ignored. Original controls(199,25),(138,39),(149,56) exact. Target(145,50) enlarged diagnostic completes329469fragments with identical alpha/CMYK/RGB; work5688254→5363971 (324283 fewer). Standard4M still refuses, reaching248125fragments versus233053; unchanged256MiB admission/239197736peak/final0. No throughput claim or normal-viewer/full-page qualification. Report:research/ai-direct-native-coordinate-qualification-2026-10-13.json.

### 2026-10-13: native coordinate validation and strict page regression

Extracted spatial_native_sample for one-pass native validation; source/model component counts checked before slicing, finite/range/opacity validated by Pixel, cancellation before/after conversion. New independent bounded-reference bit comparison includes signed zero/minnormal, invalid counts/nonfinite/range/opacity and cancellation/retry. Suite212passed/12ignored; four original controls unchanged under explicit8M diagnostic budget, finalcredit0. Strict4M page still15195/74046,next(145,50),deadlinefalse,exit101 in48.19s; no partial image published. Thus coordinate optimization does not yet advance full-page coverage. Evidence:research/ai-native-coordinate-validation-2026-10-13.json.

### 2026-10-13: AI page complexity reveals independent polygon limit

Ignored diagnostic now accepts explicit page work4M..16M(default4M), watchdog1..600s(default180), reports maximum pixel work. Production replay remains4M; polygon40M and managed256MiB unchanged. Suite212passed/12ignored. Explicit8M page reaches17906/74046 then(147,59),deadlinefalse,exit101,103.47s; no partial PPM. Isolated16M probe still refuses after708497fragments,710regions: command work10825219 spent/5174781remaining, polygon work_left4332 requested8736 proves independent40M geometry exhaustion. Peak239197736/finalcredit0. Thus raising color budget alone cannot finish page; next optimization must address polygon subdivision complexity. Report:research/ai-page-complexity-qualification-2026-10-13.json.

### 2026-10-13: reject cross-slab ordering reuse

Tried admitted previous-hit permutation reuse with exact membership/strict-order checks, then bounded adjacent repair; ties and disorder retained original unstable sort. Both suites213passed/12ignored and four original complete controls unchanged. But original(147,59),same40Mpolygon/16Mdiagnostic-command limits, stopped after577277fragments versus708497baseline (18.52% less progress); validation/fallback overhead dominates. Both experiments archived and fully removed; restored suite212passed/12ignored,diffcheck clean. No speed/completion improvement claimed. Next investigate coincident edge grouping while preserving per-region winding, rather than reuse validation per slab. Evidence:research/ai-slab-order-rejected-2026-10-13.json.

### 2026-10-13: canonical shared-edge geometry and grouped slab hits

Bounded diagnostic found1866edges/903undirectedduplicates at(147,59),all903first matches reversed; (145,50)2027/944duplicates,5first matches directed. Canonical lower-Y slope/X anchor gives bit-identical reverse interpolation, oriented winding retained. Admitted3usize/edge metadata groups exact canonical endpoint-bit identities; each region winding/selector update retained; tied distinct lines fall back to original full hit sorting. Exact intersection(x,y) duplicates removed before cut collection, original allocation credit retained. Independent reverse-anchor and repeated/reversed contour NonZero/EvenOdd tests added; suite214passed/12ignored. Original(147,59)reaches922996fragments under same40Mpolygon/16Mdiagnostic-command limits versus708497baseline (+30.28%); still refuses geometry, not complete. Four previously complete controls max native delta4.44e-16/RGB1.14e-13 from anchor rounding; no bit-exact historical claim. Strict4M page remains15195/74046,next(145,50),deadlinefalse,exit101,43.54s,no partial image. Peak239197736/finalcredit0. No measured throughput claim or independent color/viewer qualification. Evidence:research/ai-edge-grouping-qualification-2026-10-13.json; patch and pre-change snapshot archived.

### 2026-10-13: grouped edge ordering clears localized geometry refusal

Cut observation: (147,59)5628emitted/3298unique,(145,50)4018/1312; sorting those values is minor relative40Mgeometrywork. Reintroduced bounded previous-hit permutation repair only after exact shared-edge grouping, which removes the ties that defeated earlier experiments. Membership proven via retained ranks, at most2Nadjacent swaps, original input/fullsort fallback on ties/disorder. Additional24bytes/edge scratch admitted and released. 1024deterministic full-sort reference comparisons plus changing membership/crossings/ties/cancel/work cases; suite215passed/12ignored. Original(147,59)now completes924358fragments within unchanged40Mpolygon budget under explicit16Mdiagnostic color budget, spends14107965colorwork; alpha0.6737398252860465. Four prior complete original controls bit-identical to grouped baseline. Peak239197736/finalcredit0. Strict default4Mpage remains15195/74046,next(145,50),deadlinefalse,exit101,no partial PPM. Clears one localized geometry refusal, not page/normal-viewer/color qualification; reducing14.1Mcolorwork to4M remains required. No throughput claim. Evidence:research/ai-grouped-order-qualification-2026-10-13.json.

### 2026-10-13: affine native source capability and route eligibility observation

At(147,59)mesh2670 alone spends4569787sourcework, already above4M; total14107965. Added mesh_has_affine_native_field(destination): retainedmesh/no function/native channel count/finite bounded opacity/identity destination coordinates, documented as mathematical source capability before rounding, not an integration certificate. Regression rejects axial functions, transformed destination, invalid count/opacity and absent mesh. Source117passed/1ignored; compositor215passed/12ignored. Five original pixels unchanged. Ignored diagnostic candidate(147,59)admits two root Normal native affine meshes/two nearest raster sources; (145,50)global candidate conservatively rejects axial source2103, whose sampler call counter remains0. This identifies per-active-route qualification as next step; no fragment merging, speed gain or viewer qualification enabled. Clipped owner, stable raster cell key, admitted moments and rounding bounds still required. Evidence:research/ai-affine-capability-qualification-2026-10-13.json.

### 2026-10-13: qualify active affine/constant routes

Native sampled commands now carry conservative source capability Unknown/ConstantCells/AffineOwner. Mesh capability comes from identity-coordinate native mesh/no function; raster capability only from admitted nearest color/alpha cell geometry. Active-route qualifier examines coverage-filtered/pruned command sequence, permits root Normal/AISfalse affine owner sources or constant raster fields in fixed-space groups, refuses unknown sampling/masks/explicit spaces/unsafe variable blend lifecycles. Regressions cover inactive unknown source exclusion, active rejection, nested variable sources, Multiply/Screen/HardLight variable modes,AIS,masks,invalid indices/unbalanced lifecycle and constant-only group support. Suite216passed/12ignored. Both created routes at(147,59)and(145,50)qualify, whereas global source-list gate wrongly blocks latter on inactive axial2103. Five native original results bit-identical,peak239197736/final0. No coalescing or speed change enabled yet; stable source-field keys, admitted moments, clipped ownership and rounding bound remain required before integration. Evidence:research/ai-active-affine-route-qualification-2026-10-13.json.

### 2026-10-13: admitted fragment area and first moments

Added bounded open-addressed MomentTable with exact key equality, local-coordinate compensated area/X/Y sums, capacity<=16384/keywidth<=64, pre-admitted entry/index buffers, bounded work and cancellation before commit. Capacity/work/cancel/invalid-data refusals preserve existing entries; admitted credit released atDrop. Independent4096strip affine integral/translated coordinates and triangle first-moment tests; suite218passed/12ignored. Explicit observer skips no sampling and uses separate256Mdiagnostic allowance. 4096entry table fills at860700fragments/90.2455%area for(147,59), normal result still completes unchanged. 8192entries(4,718,592admittedbytes) observe all924358fragments in4587exactcoveragekeys; (145,50)329277fragments in2641keys. Areas1+2e-15/1+1.6e-15,nativepixelsbit-identical,peak239197736/final0. This is potential sample-count reduction, not enabled acceleration: full-key observer costs143399984/49111036diagnostic units, too expensive for working4M. Compact incremental field keys, centroid composition and rounding bounds remain before guarded integration. Evidence:research/ai-fragment-moments-qualification-2026-10-13.json.

### 2026-10-13: centroid composition comparison catches nonconvex exterior

Explicit moment observer now replays centroid samples with preserved coverage/winding identity, compares with original integrals and returns original result. Initial(147,59)comparison error3.17e-10native premultiplied; (145,50)failed alpha8.5901e-5/color7.2145e-5. Optional admitted5reference compensated integrals localized three bad buckets: outside-owner union centroid enters mesh, changing source alpha0→1. NativeField::AffineOwner now additionally requires selected triangle; unowned fragments keep old composition. Added geometric regression with two outside points whose average enters triangle, and transactional optional-reference payload tests. Suite220passed/12ignored. Guarded comparisons: (147,59)4582centroid samples+6fallback fragments, alpha1.362e-13/nativepremul3.170e-10; (145,50)2627centroid samples+1240fallback, alpha2.468e-13/nativepremul4.468e-10. Original outputs bit-identical,peak239197736/final0; table5,373,952admittedbytes. No sampling skipped in ordinary path, no speed/physicalcolor/full-page claim; cheap keys and formal rounding/raster-cell proof still required. Evidence:research/ai-moment-owner-fallback-qualification-2026-10-13.json.

### 2026-10-13: bounded active-prefix moment identity

MomentTable admits fixed identity prefix width1..N. Slice API requires exactly that width, materializes zero tail once per new entry, hashes/compares every identity word; wrong width/full-array calls refuse rather than truncate. New regression checks differing active words, zero stored tail, malformed width preservation and repeat accumulation. Observer now passes12meaningful coverage words instead of copying/hashing/comparing64. Suite221passed/12ignored. Same original geometry/centroid/fallback/native results, lookup work(147,59)152582586→47480544(68.88%reduction),(145,50)52203946→16744430(67.93%); centroid error unchanged3.17e-10/4.47e-10. Table5,373,952admittedbytes,peak239197736/final0. Work reduction is not measured wall-clock throughput; diagnostic lookup still above4M and production coalescing remains disabled. Incremental compact field keys and formal integration bounds remain. Evidence:research/ai-moment-prefix-keys-qualification-2026-10-13.json.

### 2026-10-13: shared polygon budget accounting

Added shared remaining-work API retaining debit on success/refusal/cancellation; legacy wrapper delegates with a fresh local allowance. Regression verifies exact allowance, repeated calls exhausting one shared allowance, and cancellation/retry. Suite222passed/12ignored. Original diagnostic pixels under explicit16Mcolor allowance consume32230736/40000000geometry units at(147,59),18830136at(145,50), leaving7769264/21169864. Baseline and explicit moment observer geometry counters/native outputs identical,peak239197736. Observer lookup47480544/16744430still exceeds geometry spare allowance at first pixel; no allowance transfer or production coalescing enabled. Normal4Mpage/color/integration qualification remain open. Evidence:research/ai-shared-polygon-budget-qualification-2026-10-13.json.

### 2026-10-13: reject previous-entry moment lookup shortcut

Exact-key previous-entry shortcut tested on original AI: observer work47480544→58572728at(147,59),16744430→20680709at(145,50),about23.4%worse. Original pixel/bucket/moment values unchanged. Candidate suite221passed/1failed/12ignored: existing refusal test assumes old repeated-key minimum, newly cheaper repeat path violates that threshold; regression was not weakened. Candidate archived and source restored. Reject locality hypothesis before any production use; next inspect incremental coverage/source-field identity. Evidence:research/ai-moment-last-entry-rejected-2026-10-13.json.

### 2026-10-13: admitted incremental coverage fingerprint

Moment observer updates XOR fingerprint when coverage bits change; deterministic bit tokens prepared once in admitted709/759word buffers, explicit preparation/toggle work charged separately from unchanged polygon/color budgets. Prehashed table index still compares full exact active key; caller must use one stable hash scheme. Regression checks8192toggles/all4096bits against full recomputation, deliberate hash collisions preserve distinct keys and work refusal preserves entries. Suite223passed/12ignored. On-demand token computation was1.1%worse; admitted tokens reduce observer work47480544→44317504(6.66%),16744430→15626072(6.68%). Original pixels/buckets/centroid/fallback results identical, three extra controls unchanged,peak239197736. Additional slot-hash storage experiment improved first pixel but slightly worsened second; rejected and source restored to token-only variant. No sampling skipped or throughput claim; working4M/fullpage/color/viewer integration remain unresolved. Evidence:research/ai-moment-incremental-qualification-2026-10-13.json.

### 2026-10-13: compact nearest raster field identity

Extracted two-u64 color/alpha texel identity (16bytes versus prior four-usize32bytes) and used it in existing last-sample cache. Independent alpha dimensions/scaling/clamping preserved; absent alpha sentinel cannot overlap validu32texel coordinates. Image/transform/space/dimensions belong to enclosing command, keys cannot identify sources globally. AuthoredRGB2x2/native-soft-image/RGB1x1+alpha2x2 regressions each enumerate1089points, compare uncached native fields for same keys and three convex combinations per point, including exterior clamp; interpolation/nonfinite key refused. Constant1x1fixture correctly expects one cell. Suite223passed/12ignored; five original AI pixels exactly match previous diagnostic output,peak239197736. Necessary constant-field identity groundwork; no fragment sampling skipped, throughput/fullpage/working4M/color/viewer qualification still open. Next stable route/source/selected-owner identity and rounding-bound integration. Evidence:research/ai-raster-cell-identity-qualification-2026-10-13.json.

### 2026-10-13: carry source field and stable route identity

NativeSpatialSample carries optional nearest raster color/alpha cell key copied from successful existing cache, no inverse/index recomputation. SourceFieldIdentity requires present sample plus raster cells or selected affine triangle; missing/unknown evidence rejects moment qualification. Routes carry checked monotonic generation unique within one replay and retained through64entry cache eviction, rather than treating shifted vector indices as identity. Explicit1work/newgeneration; tight lazy-mask regression39→40accounts for that single debit and still detects replaced/duplicate sampling. New identity/1024generation/exhaustion regressions; existing actual route-eviction rendering regression passes. Suite225passed/12ignored; five original AI pixel outputs exactly match prior log, original moment counts/work44317504/15626072unchanged,peak239197736. Metadata/guard groundwork only; compact combined moment identity and rounding proof still needed, no sampling skipped or fullpage/working4M/color/viewer qualification claimed. Evidence:research/ai-source-field-identity-qualification-2026-10-13.json.

### 2026-10-13: source-field identity moment comparison with representative coverage

Added bounded ordered16word route-generation/length/command/source-field key (<=4source evaluations; overflow/unknown/unowned retain original fragments), optional admitted64word first-coverage witness in MomentTable, transactional witness insertion and explicit creation/copy work. Separate source-field diagnostic replays centroid with first witness and verifies complete identity before comparing. Original(147,59)coverage4582keys→1659fieldkeys, all1659centroids accepted; (145,50)2627→2192,2191accepted/1original-reference fallback. Initial strict comparison caught generation2→3with otherwise equal source fields at one centroid; guard retained, no assumed route equivalence. Alpha/nativepremul errors1.344e-13/3.685e-10 and2.471e-13/2.048e-10 including6/1240unowned fragment fallbacks. Five original output lines exactly match previous; suite227passed/12ignored,peak239197736. Witness table9,568,256admittedbytes; legacy observer remains complete with unchanged results and increased witness-creation charge. New fragment observation work82,478,711/29,553,328includes key construction; worse than coverage observer, not production acceleration. No source sampling skipped; formal rounding bound, cheap incremental source identity/accumulation, working4M fullpage, independentcolor/viewer qualification remain open. Evidence:research/ai-source-field-moments-qualification-2026-10-13.json and archivedpatch.

### 2026-10-13: exact variable field-key length and active-prefix reuse

MomentTable variable-key API stores exact length<=admitted maximum, hashes/compares every active word, zeroes stored tail; fixed API still requires full declared width. Forced identical-hash[7]/[7,0]/[7,0,0] regression proves differing lengths cannot alias, malformed/oversize/work refusal preserves entries/witness. FieldObservation reuses buffer by resetting generation/length only; stale tail excluded from all identity operations, regression verifies fresh active prefix despite different tail. Centroid guard uses stored exact length, unchanged strict route-generation fallback. Suite229passed/12ignored. Original five pixels/buckets/centroid deltas/fallbacks unchanged; source-field observation work82,478,711→59,250,992 and29,553,328→21,177,975(28.16%/28.34%reduction),peak239197736; table9,633,792admittedbytes includes per-entry length. Previous-entry shortcut retested on source fields and still worse65,656,612/24,432,792(+10.81%/+15.37%); rejected/source restored. Work reduction is not measured throughput or production coalescing; rebuilding identities after full native evaluation remains too expensive. Next geometry-transition source identity; working4M fullpage/color/viewer qualification remains unresolved. Evidence:research/ai-variable-field-key-qualification-2026-10-13.json and patch.

### 2026-10-13: geometry-to-native raster texel identity

Nearest RasterCells retain query-grid bounds with aligned24byte metadata admission. Constant-work path-index→packedtexel mapping refuses footprint/out-of-range/non-grid line partitions; query indices preserve source coordinates and row-major geometry order. Native ConstantCells source capability now explicitly requires grid metadata for color and optional alpha. Partial8x6query regression checks9cells; three authored decoded raster fixtures each checked under identity/skew/reflection transforms, every emitted color/independentalpha cell at three interior points against native sampler identity and polygon winding. Existing cancellation/admission/work/storage-release cases retained. Suite230passed/12ignored; five original AI output lines and moment counts/work59,250,992/21,177,975exactly unchanged,peak239197736. Provides geometry identity necessary for incremental source keys; deduplicated-region mapping/selector-driven updates still unwired, no sampling skipped or throughput/working4M/fullpage/color/viewer qualification claimed. Evidence:research/ai-raster-geometry-identity-qualification-2026-10-13.json.

### 2026-10-13: preserve raster source texels through canonical region deduplication

Added admitted per-source color/alpha region-to-texel bindings after geometry deduplication, retained on native sampled commands; independent bounded4Mpreparation allowance, allocation/cancel/work refusal releases storage. Shifted images sharing canonical geometry regression proves source texels remain distinct; conflicting same-source mapping and multiple active texels refuse rather than choose arbitrary source. Explicit geometry-key observer scans bindings and compares with existing native sampling, charging separate observation work. Five original pixels:1,852,034/662,938/41,506/558,924/10,146checks,total3,125,548,mismatches0. Original output lines/buckets/centroid errors/fallbacks exactly unchanged,peak239197736;suite231passed/12ignored. Observation work92,527,880/35,007,609includes extra scans; no speedup or source sampling skipped. Necessary canonical source mapping is now wired; next region-to-source transition index and selector-driven active-cell state. Working4Mfullpage/rounding/independentcolor/viewer qualification remains open. Evidence:research/ai-raster-region-bindings-qualification-2026-10-13.json and patch.

### 2026-10-13: selector-driven active raster identities

Added admitted compressed region→source links, per-command64bit active color/alpha cells and constant-work identity query; missing/multiple active cells refuse. Complete bounded update/rebuild work debited and cancellation checked before state mutation; centroid replay rebuilds state from representative coverage. >64cells/grid or >16384links refuses tracker and keeps scan fallback, never truncates.4096deterministic transitions/shared sources/channels/region70match independent scan; cancellation/work refusal preserves state, all preparation cancel/admission paths and link/bitmap ceilings release credit. Suite233passed/12ignored. Explicit tracker active on five original pixels;3,125,548native identity comparisons,mismatches0,original output/buckets/centroid errors/fallbacks unchanged,peak239197736. Observation work92,527,880→66,719,457(27.89%),35,007,609→23,841,608(31.90%). ABBAABBA two-pixel diagnostic timings median scan3.377s/transition2.666s but individual2.57–4.73s; wide variation precludes stable throughput claim. No source sampling skipped; mesh-owner transitions/cheap accumulation/formal rounding/working4Mfullpage/color/viewer qualification remain open. Evidence:research/ai-raster-transitions-qualification-2026-10-13.json,pairedreport,andpatch.

### 2026-10-13: selector-driven maximum triangle owner identity

Added admitted region→source links and active owner-word summaries; duplicate canonical region keeps maximum source owner. Constant query for proven monotonic region-index layouts, bounded exact maximum scan for arbitrary layouts; owner0distinguished fromNone. Updates and centroid rebuild read authoritative coverage rather than duplicating512byte buffer; inconsistent active word refuses. Complete bounded mutation debit/cancel before state changes;16,384link ceiling/invalid region refuses before allocation.4096deterministic transitions/repeated notifications match independent oracle for aliases/nonmonotonic indices/region70; cancel/work refusal preserves state, all preparation cancellation/admission cases release retained/scratchcredit. Suite235passed/12ignored. Tracker active on five original pixels:915,368/327,493/0/107,246/4,012ownerchecks,total1,354,119,mismatches0; raster3,125,548checks stillmismatches0. Original output/buckets/centroid errors/fallbacks identical,peak239197736. Combined observer work75,811,570/27,066,717versus duplicate-coverage prototype80,381,274/28,691,196; adds work versus raster-only66,719,457/23,841,608because old owner lookup retained as oracle. No source sampling skipped or throughput/working4Mfullpage/color/viewer qualification claimed. Next compact geometry-derived field identity before native composition. Evidence:research/ai-owner-transitions-qualification-2026-10-13.json and patch.

### 2026-10-13: geometry field identity before color evaluation

Explicit RRRAH_AI_PRECOLOR_KEYS constructs ordered route/source identity from admitted raster and owner transition state before native color sampling; missing/ambiguous fields refuse. Actual color evaluation remains the reference and compares exact active key including route generation. Panic-on-call sampler regression proves no color lookup during construction; missing trackers/owner, cancellation and work exhaustion refuse. Suite236passed/12ignored. Five original AI pixels bit-identical;1540674ready/checks,0mismatches,1347unavailable; centroid errors and existing raster/owner checks unchanged. Combined observer work137605135/49053197 versus75811570/27066717,peak239197736. This adds diagnostic work and skips no color sampling; no throughput or ordinary4M/fullpage/viewer/color qualification. Next compact pre-color identity and bounded integration certificate. Evidence:research/ai-precolor-field-keys-qualification-2026-10-13.json.

### 2026-10-13: cache immutable route capability

Affine capability now retained on exact immutable route and rebuilt on new generation/eviction; current raster cells and triangle ownership still queried every fragment. Qualification traversal debited once on route creation. Existing size_of Route admission covers metadata. Regression compares qualified/unqualified exact keys and debit, and confirms changed coverage still refuses absent owner; route eviction regression passes. Suite236passed/12ignored. Five native pixels,1540674precolor checks/zero mismatches,centroid comparisons unchanged. Diagnostic work137605135→131145281(4.69%),49053197→46752234(4.69%). No original color sampling skipped or wall-clock acceleration claimed; working4M/fullpage/color/viewer qualification remain unresolved. Evidence:research/ai-route-qualification-cache-2026-10-13.json.

### 2026-10-13: reuse pre-color field key buffer

One initialized scratch key per replay replaces fresh16word zeroing on every geometry prediction; only3metadatawords reset, active fields overwritten. Scratch published only after complete success. Regression verifies inactive stale tail preservation/identity parity/16work saving, exhausted partial call then successful retry, and immediate cancellation preserving buffer. Suite236passed/12ignored; five native pixels,1540674precolor comparisons/zero mismatches and centroid results unchanged. Observer work[131145281, 46752234]→[116355569, 41483818]. This is diagnostic counted-work reduction, not measured throughput. All original color samples remain; working4M/fullpage/independent native color qualification open. Evidence:research/ai-precolor-buffer-2026-10-13.json.

### 2026-10-13: wall-clock check rejects observer-count speedup inference

ABBAABBA paired five-original-pixel diagnostics compared compiled source snapshots before/after pre-color scratch reuse. All8runs preserve exact native pixel lines and zero pre-color mismatches. Median before2.960836s,after2.963044s(+0.0746%),range2.9121–3.4565s. No demonstrated wall-clock speedup; prior11.27% counted observer-work reduction is not throughput evidence. Harness restored to current source and binary hashes archived. Stop optimizing observer bookkeeping in isolation; next reduce actual native color evaluations subject to exact identity, integration bound and ordinary budget constraints. Evidence:research/ai-precolor-buffer-paired-2026-10-13.json.

### 2026-10-13: reject retained mesh denominator candidate

Cached exact cross(b-a,c-a) per triangle, existing size_of admission accounts8extra bytes/triangle. Candidate suites236passed/12ignored and117passed/1ignored; five original AI native values exact, but peak239197736→246653464(+7455728bytes). ABBAABBA5Mselected-mesh-sample kernel median118771021ns→123820125ns(+4.25%),checksum/work exact. Candidate rejected; original source and harness source restored. No throughput benefit claimed. Patch/logs retained to avoid repeating the same cache experiment. Next reduce actual native color evaluation count subject to bounded integration certification. Evidence:research/ai-mesh-denominator-rejected-2026-10-13.json.

### 2026-10-13: exact final raster pixel cache

Per-source nearest color/alpha cell key now also caches validated native Pixel, avoiding repeated model selection/from_native component validation on identical cells. Native raster component cache remains unchanged. Interpolated samples bypass keyed reuse; cancellation before hit/publication, missing key validates fresh, validation failure preserves cache. Authored RGB/soft image/unequal alpha fixtures verify fresh/cached output, cancellation and invalid unkeyed sample handling. Suite236passed/12ignored; five original pixels,1540674key checks/zero mismatches and centroid comparisons exact. Existing admitted256byte/source closure limit passes; peak239197736unchanged. ABBAABBA diagnostic medians before2.788602646s,after2.7761443750000008s; overlapping variation prevents viewer acceleration claim. Working4M/fullpage/integration/color qualification remain open. Evidence:research/ai-raster-pixel-cache-2026-10-13.json.

### 2026-10-13: measure actual raster final-pixel reuse and mesh budget floor

Existing spatial trace now reports successful final-Pixel cache hits/builds per raster source, disabled without trace. Five original controls: hits3072925,builds52623,hit rate98.3163592432431%,including centroid replays. Native pixel/source-work/centroid lines unchanged;236passed/12ignored,existing256byte/source capture admission guard passes,peak239197736. At147,59mesh event2670alone takes4578032source work/915368calls, already exceeding ordinary4000000totalcolor allowance. Raster cache improvements alone cannot satisfy this budget; prioritize mesh field integration certificate and actual sample-count reduction. Evidence:research/ai-raster-pixel-cache-counters-2026-10-13.json.

### 2026-10-13: conservative native mesh component range certificate

Added budgeted/cancellable source range API for successful selected-triangle samples in identical native coordinates. Vertex extrema widened onef32step outward covers normalized positivef64weights and finalf32rounding; functions/color transforms/invalid vertices/index refuse. No retained triangle metadata or color evaluation. Regression enumerates1681selected points including projected exterior points and tiny/constant/extreme components; budget13/exhaustion12/cancel/invalidsource/index/NaN tested. Source118passed/1ignored,compositor236passed/12ignored. Explicit4Mdiagnostic range preparation preserves five native pixels and centroid comparisons. All retained original event2670triangle spans exceed1e-6; source-wide near-constant replacement cannot safely solve sampling budget. Next local affine-integration error bound accounting inverse coordinates/projection/rounding; no samples skipped. Evidence:research/ai-mesh-component-bounds-2026-10-13.json.

### 2026-10-13: selected-owner projection counterexample protects integration boundary

Added executable source regression: one native affine-capable owner at exterior/contained endpoints yields sample mean0.25 versus selected midpoint0 due negative-weight projection/renormalization. Same owner/key/source capability is insufficient to certify a domain; fully contained control preserves its mean, generic hit-test refuses exterior point. Source119passed/1ignored. This is an adversarial API fixture, not evidence that the compositor emits that exterior segment. Documented required domain/projection, coordinate/conditioning, rounding/composition, moments, identity and budget proof before sample skipping. No production behavior changed. Evidence:research/ai-mesh-integration-certificate-requirements-2026-10-13.md and research/ai-projected-field-regression-source-suite-2026-10-13.log.

### 2026-10-13: measure selected mesh projection on original AI

Budgeted/cancellable diagnostic source API returns sum of negative computed barycentric weights, no color sample. Adversarial regression checks0.5/zero deficits, shared allowance, exhaustion/cancel/index/nonfinite refusal. Source119passed/1ignored,compositor236passed/12ignored. ExplicitRRRAH_AI_MESH_PROJECTION uses admitted40byte counters and separate shared16Mdiagnostic allowance; original five native pixel values/sourcework/rastercache/key/centroid comparisons unchanged. 1352772selectedchecks,364projectioncases,maxdeficit1.3635338747552817e-10,0refusals. Actual projection is small but nonzero; empirical maximum is not a domain certificate and says nothing about nonnegative normalization rounding. Next bound projection/coordinate/rounding error over clipped domain before any sample skipping. Evidence:research/ai-mesh-projection-2026-10-13.json.

### 2026-10-13: outward intervals for computed mesh weights

Added cancellable/shared-work API enclosing raw barycentric weights over device rectangle with outwardf64rounding of each operation in the actual inverse transform/cross/division sequence. Degeneracy/nonfinite/reversed domain/exhaustion refuse. Source120passed/1ignored,compositor236passed/12ignored;10404point regressions span positive/negative orientation, decimal vertices, nearly degenerate triangles, skew/reflection transforms and degenerate point rectangles; every smaller work allowance refuses. Explicit diagnostic query preserves five original native pixels/sourcework/cache/key/centroid lines. Whole-pixel rectangles produce very loose deficit upper bounds(up to197629) and certify no nonnegative domains for retained skinny triangles; observed tiny sample deficits do not justify narrowing those bounds. Next preserve clipped triangle/polygon-domain correlation; this API is not an integration certificate and skips no samples. Evidence:research/ai-mesh-weight-intervals-2026-10-13.json.

### 2026-10-13: retain affine coordinate correlation in mesh weight bounds

Added vertex-hull raw-weight enclosure with interval coefficients for exact linear expression and independent absolute roundoff envelope. Each addition/subtraction propagates both input errors; scalar multiplication/division scales input error; one outward output-range ulp bounds ordinary operation rounding. Ideal affine extrema enclosed at hull vertices; final intervals expanded by evaluation error. No per-triangle retained metadata. Source121passed/1ignored,compositor236passed/12ignored;10404existingpoint tests now check both enclosures, skinny domain test reduces rectanglewidth>1e9toabout0..1; all smaller budgets/all cancel checkpoints/domain limits tested. Original triangle hull diagnostic upper projection deficit147,59now1.63136e-7rather than wholepixel197629; other pixels1.30732e-8/4.65560e-8/1.14825e-9,0refusals,120work/triangle. Original five pixels/source/cache/key/centroid comparisons unchanged. Hull membership of rounded clipped fragments/centroids, projection/normalization/color/route/moment error propagation still required before sample skipping. Evidence:research/ai-affine-weight-bounds-2026-10-13.json.

### 2026-10-13: propagate raw-weight bound to native source component error

Source API now bounds pointwise component discrepancy from exact corresponding affine field over supplied hull: raw evaluation E, clipping deficit D, sum/division error eta yieldL1<=2(E+D)+eta; channel span scales variable part, constant term uses eta, nativef64arithmetic and fullf32ulp at1 included. Unsafe source/normalization/domain/nonfinite/work/cancel refuse. Source122passed/1ignored,compositor236passed/12ignored; inside/exterior component tests, constant channel, all smaller budgets/all cancellation checkpoints. Refactor initially failed late-cancellation regression; lazy error construction and final check fixed it. Original source hull component upper error1.20833e-7/1.19305e-7/1.19439e-7/1.19214e-7,0refusals; five native pixels/source/cache/key/centroid lines exact. This is a conditional source-component certificate, not finalpixelerror: clipped fragment/centroid membership, composition/moment propagation and ordinary4Mfullpage/color qualification remain before sampling can be skipped. Evidence:research/ai-affine-component-error-2026-10-13.json and derivation note.

### 2026-10-13: one-step fixed-alpha native Normal error propagation

Added bounded/cancellable premultiplied error helper: gains alpha,shape-alpha,1-shape propagate source/native and backdrop/previous storage error, outward arithmetic plus conservative256f64EPS conversion/Normal allowance. Same fixed alpha/shape and checked native0..1input coordinates required; invalid/inactive bounds and positivealpha<1e-12 refuse.4608actualPixelpairs/12288component comparisons acrossGray/RGB/CMYK/opacities/shapes; all insufficient budgets/all cancellation checkpoints tested. Suite238passed/12ignored. Original AI one-step source contribution max1.208335e-7withzero preceding error,0refusals; five native pixels/source/cache/key/centroid/source-component results exact. This is not fullroute/finalpixel error. Group boundaries, fragment/centroid hull membership and moment accumulation remain before any sample skipping or ordinary4Mfullpage qualification. Evidence:research/ai-normal-error-bound-2026-10-13.json and derivation.

### 2026-10-13: group error envelopes and unsafe variable-backdrop lifecycle guard

Added fixed-parameter same-space storage-error propagation for initial backdrop removal/nonexpansive clamp, group opacity, and native component extraction. Clamp error bound is not an affine certificate.3456group pairs/27648stage component comparisons acrossGray/RGB/CMYK; all smaller budgets/all cancellation checkpoints and invalid channels/parameters tested. Actual HardLight counterexample proves old affine route ordering unsafe: variable opaque parentq + nonisolated constant0.25HardLight child +0.5opacity/HardLight end yields0.5q²+0.5q(midpoint0.375 versus endpointmean0.5). Predicate now refuses groupBegin after variable field until lifecycle certified; constant group prefix still qualifies. Updated previous unsafe acceptance regression to refusal with reordered-prefix positive control. Suite241passed/12ignored. Original five AI native pixels/momentcounts/fieldchecks/sourceerror/centroid diagnostics unchanged. Baseline composition preserved, no sample skipping. Fragment/centroid domain and moment error/full4Mpage/color/viewer qualification remain open. Evidence:research/ai-group-error-bound-2026-10-13.json and derivation.

### 2026-10-13: enclose compensated area/centroid accumulation error

Optional separately admitted48byte/bucket geometry side vector tracks outward intervals for exact sums of reported fragment areas and local first moments, including subtraction/product/sum rounding. Positive interval-area division/origin addition enclose exact weighted centroid; actual compensated-output distances yield area/coordinate error bounds. Ordinary entry storage/numeric outputs unchanged. Work14/fragment+32/export,side393216bytes/table; all mutation occurs after complete debit/cancel checks.4096fragment exact-value regression; exhausted/canceled update and first insertion preserve both states/slots and permit retry; denied side admission/drop release credit. Suite245passed/12ignored. Original AI3851buckets certified,0unavailable,maxareaerror7.62584e-15,maxcentroiderror2.41585e-12pixels; five native pixels/source/cache/key/centroid comparisons exact. Observerwork129349585/46146480. These bounds exclude preexisting polygon clipping/fragment-centroid error; full domain/source/route/moment propagation still required before sample skipping/ordinary4Mfullpage qualification. Evidence:research/ai-moment-geometry-bounds-2026-10-13.json.

### 2026-10-13: affine gradients and geometric integral replacement term

Added budgeted/cancellable interval bounds for exact native affine source device gradients and moment geometry kernelA_upper*(Gx*deltaX+Gy*deltaY)+deltaArea*valueBound. Known identity/skew/reflection/constant-channel derivatives, all smaller work allowances/all cancellation polls, area/centroid corner perturbations tested. Source123passed/1ignored,compositor246passed/12ignored. Original five pixels/source/cache/key/centroid/moment-rounding controls exact. Original source gradients max200.0529/16.8751at147,59and8.57546/3.38046at145,50; unit-gradient/value2 aggregate geometry terms3.10853e-13/1.48974e-13. Conditional scaled source-affine geometry terms6.21871e-11/1.27751e-12, excluding pointwise source/route error, clipping/domain membership and final integral summation. No sample skipping or ordinary4Mfullpage/viewer qualification. Evidence:research/ai-affine-moment-error-2026-10-13.json and derivation.

### 2026-10-13: exact reported-sample integral/quotient arithmetic envelope

Added optional outward envelope for exact sums of reportedf64area*alpha andarea*alpha*nativesample values, with maximum absolute reported components bounding the exact weighted quotient. New differing-alpha quotient error kernel(eI+B*eA)/positiveAlphaLower plus division rounding supports tiny positive alpha; zero/nonfinite/overflow refuses. Envelope112admittedbytes,21diagnosticwork/fragment; failure disables certificate while baseline values/order remain.4096dyadic known exact integral/ratio test, differing-alpha cases down to1e-300, all smaller budgets/all cancellation checkpoints for accumulation/export/quotient and invalid inputs tested. Suite249passed/12ignored. Five original pixels/source/cache/key/moment/centroid/geometry controls exact;5/5complete quotient bounds,maxbaselinequotient arithmeticerror1.23905e-10,maxalphaerror5.41421e-11. Excludes clipping/source/domain, combined replacement and later profile/display error; no sampling skipped or ordinary4Mfullpage qualification. Evidence:research/ai-integral-rounding-envelope-2026-10-13.json and derivation.

### 2026-10-13: observed-point domain/projection evidence closes numeric source-hull gap

Prepared source/owner-bound object encloses raw-weight evaluation errorEover checked device rectangle. Every actual observed point validates finite membership and bounds negative weight massDwith outward rounding; positive mass<=1+E+Dpermits source projection/normalization/f32error bound without assuming original triangle-hull membership. Failure preserves evidence; callers reject incomplete point sets. Source125passed/1ignored covers289interior/exterior points, every short prep/observe/finish budget and every cancellation checkpoint. Bounded admitted metadata vector binds original owners, charges index sorting/binary search and observations; sampler128byte capture guard verifies old admission. Explicit original AI diagnostic certifies1352772selected-owner points incl centroids,0observation/finalizationrefusals,maxcomponenterror1.193365e-7;1347unowned samples retain original fallback. All five native pixels/sourcework/identity/centroid/moment/integral controls and239197736peak unchanged;finalcredit0. Temporary harness directories disappeared; added repeatable offline source-linked harness helper under ignoredtarget, copied root-lock versions preserved/idempotent/rootlock unchanged. No sampling skipped, normal4M/fullpage/combined-error/truegeometry/independentcolor qualification remain open. Evidence:research/ai-point-evidence-2026-10-13.json, derivation and harness-recovery report.

### 2026-10-13: candidate centroid/fallback accumulation arithmetic envelope

Added optional separately admitted candidate-centroid and unchanged-fragment fallback envelopes; outward merge includes final stream addition and preserves maximum component bound. Separate original/candidate quotient arithmetic errors add outward. Individual area cap1preserved; dedicated aggregate input accepts existing moment tolerance1+1e-12, with boundary regression. Merge short-budget/all-cancel tests; suite251passed/12ignored.147,59candidatealphaerror9.64784e-14,integralerror7.31637e-14,original+candidate native arithmetic term1.24134e-10.145,50explicitlyincomplete because one identity-guard fallback uses an original compensated per-bucket reference whose interval is not retained. No invented bound or identity weakening; original fallback values preserved. All five original pixels/source/identity/centroid controls exact,finalcredit0. Next retain per-bucket reference intervals to close this missing input; combined route/source/moment budget, ordinary4Mfullpage and independentcolor/viewer qualification remain open. No source sampling skipped. Evidence:research/ai-replacement-rounding-2026-10-13.json and derivation.

### 2026-10-13: original reference interval closes candidate fallback arithmetic gap

Optional separately admittedKinterval side vector encloses reported reference inputs(or one upstream rounded product), positive area weighting and sums outward, independently of compensated values. Geometry/reference/ordinary values commit together only after full debit/cancel checks. Reference storage655360bytes for8192x5channels;21extra work/fragment, ordinary tables disabled. Candidate fallback retains exact old reference value and inserts its matching interval plus independently proved original native-value bound. Compositor256passed/12ignored;4096reference regression, FMAresidual upstream product test, all insert/update short budgets/cancel checkpoints, three admission failures/release and interval-addition invalid cases. Both heavy original pixel candidate envelopes now complete:147,59integralerror7.31637e-14/native arithmetic1.24134e-10;145,50integralerror1.36003e-13/native arithmetic4.78288e-11. Bucket1868identity fallback remains original, now bounded. Five original pixels/source/identity/centroid/peak exact,finalcredit0. Combined source/route/moment error, ordinary4Mfullpage and independentcolor/viewer qualification remain; no sampling skipped. Evidence:research/ai-reference-interval-2026-10-13.json and derivation.

### 2026-10-14: checkout-contained APNG/ICO/CUR compile-time fixtures

Moved six small regression inputs into the tracked raster fixture layout by byte-identical copy, retaining ignored originals for historical/external tools. Updated compile-time APNG/ICO/CUR includes, selected-icon GPU fixture routing, explicit viewer references and ICO/CUR generator destinations. SHA-256/length manifest and producer metadata retained. APNG10passed/1manual-benchmarkignored; ICO/CUR4passed. Literal compile-time input audit found no other nontracked paths outside the newly added raster fixtures. Focused source whitespace check passed; global rustfmt currently fails on unrelated concurrent work. Full clean-checkout CI, MSRV, other warnings and physical Metal checks remain unverified; no universal format completion claim. Evidence:research/compile-time-format-fixtures-2026-10-14/report.json and logs.

### 2026-10-14: actual Rust 1.92 workspace compiler check

Corrected workspace/CI MSRV from1.89to1.92, including cargo-deny toolchain reporting. Locked Hayro0.7.1, HayroInterpret0.7.0 and HayroSyntax0.7.2 require1.92; all-feature metadata530packages has no declared requirement above1.92. Installed actual1.92.0 and checked all workspace libraries/binaries on aarch64-apple-darwin offline/locked in an isolated target:exit0. Historical audits retain original1.89 statements with a dated correction. No all-feature compilation, test/example/benchmark check, Linux/Windows MSRV or fullCI qualification claimed. Evidence:research/msrv-1.92-2026-10-14/report.json, rustc.txt, declared-dependency-msrv.json and workspace-check.log. Full100format/cache/GPU objective remains open.

### 2026-10-14: application/test dead-code boundaries and worker supervision checks

Five test-only foreground/resident-source wrappers now compile only under cfg(test), preserving production paths and test API. Partial swapped-region integration module permits unused shared worker members only at its module boundary. The worker scope marker permits dead-code only on platforms without Linux/macOS group supervision; global warnings policy unchanged. App binary+swapped-region target and app test configuration check passed without warnings. Actual -Dwarnings worker build:1quiescenceunit+7singleprocess+8group tests passed. Initial sandbox group run hit ps PermissionDenied in5post-stop assertions; same built binary outside sandbox passed8/8, original failed log preserved. No native Windows/Linux, GPU runtime, full strict workspace/clippy or CI completion claimed. Evidence:research/target-warning-cleanup-2026-10-14/report.json and logs.

### 2026-10-14: current Metal icon and swapped-region execution

Built current ico_pages_metal and swapped_region_metal targets; explicitMetal/anyvendor/GPUoptional0 selectedAppleM4Max. FiveICO/CURtests passed: selected PNG payload quality/alpha/hotspots, DIB1/4/8/16/24/32 masks and patterned packed rows, RAMcount1/size64B/visibleTTL/pressure/reverse swap, SDR/HDR exact and finalcredits0. PNG selected fixtures now tracked; DIB variants/oracles remain ignored local corpus. Swapped-region target6passed(5workerunit+1GPU),15aspect/corner regions preserve SDR/HDR exactly after full source release, including refusal/cancel and zero credits. Explicitignored delayed-navigation GPUtest1passed: oldfile/index released before latest wake; only newHDRsource renders. Total12tests,7GPUtests; no physicalHDR/NVIDIA/universalformat claim. Invalid vendor=apple first attempt refused beforeGPU; correctedanyvendor remainsMetal-only. Evidence:research/metal-icon-swap-current-2026-10-14/report.json and logs.

### 2026-10-14: portable complete authored CUR DIB fixture suite

Copied30CC0authoredCUR/JSON/RGBAfiles(12046bytes) byte-identically into raster fixture layout. All ico_pages_metal fixture paths now refer to raster, including DIB1/4/8/16/24/32 selected and mixed9x3packedmask tests. Three generator destinations updated; bundledPillow12.3.0 reproduced18binaryfixtures/oracles exactly in an independent temporary root. Target cargo check passed; focused whitespace check passed. GPU execution was not repeated for this path-only change; previous same-byte Metal evidence stays in metal-icon-swap-current-2026-10-14. Historical ignored corpus preserved. Evidence:research/portable-cur-dib-fixtures-2026-10-14/report.json and reproduction.json.

### 2026-10-14: checkout-contained selected-icon application loader prefetch

Updated selected-container app qualification helper: ICO/CUR fixtures resolve raster, TIFF still resolves localdata. CPUloader test1passed plus explicitMetalAppleM4Maxloader test1passed across PNGICO/PNGCUR/DIBCUR. Each refuses3obsolete planned indices before swap/decode(reads0,RAM0,credit0), then backward2[1,0]/forward1[2] restoration bypasses blocked decoder. Selected keys, pressureRAMhits, nonzeroTTL, payload/hotspot and SDR/HDR exact; final0. Changes are test fixture routing only; no fullCI/allformat/physicalHDR/NVIDIA claim. Evidence:research/portable-icon-viewer-prefetch-2026-10-14/report.json and CPU/Metal logs.

### 2026-10-14: portable TIFF page-ICC app/cache/Metal evidence

Authored2x1three-pageDeflateTIFF(sRGB/gamma1/sRGB), LittleCMSRGB8oracle and metadata copied byte-identically into raster; generator output and selected-container helper now resolve raster. BundledPillow12.3.0/LittleCMS2.19 reproduced all3files exactly. ActualMetalM4MaxpageICCtest passed:18RGB8components within1 of independent oracle, untagged assumptions cannot overrideICC, reversed swap retains preparedfloatbits and SDR/HDRframes exactly, finalcredits0. CPUapp-loader1passed and explicitMetalapp-loader1passed: independent selectedkeys, managedpressureRAMhits, nonzeroTTL,3stale prefetch requests refused before swap/decode, backward2/forward1 swap restoration bypasses blocked decoder, final0. Total3tests(2GPU). No fullTIFF/allformat/physicalHDR/CI/NVIDIAclaim. Evidence:research/portable-tiff-icc-viewer-2026-10-14/report.json, reproduction.json and logs.
