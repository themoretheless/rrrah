# Current coverage of the requested viewer scope

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
| 100 image formats | IMAGE_FORMATS_100.md has 100 entries, 86 implemented subsets and 14 pending; pending AI has PDF-compatible routing and real RAM/swap/Metal transport evidence | Implement missing families and qualify variants, color and display; AI color comparison still fails, and the 86 implemented subsets are not blanket correctness proof |
| 3D | Native STL/OBJ/PLY/OFF decoding, streamed model swap codecs, viewer spill/restore and real swap Metal readback | Additional formats, materials, thumbnails, persistent model disk cache and physical navigation qualification |
| RAM size/count/TTL | RAW/prepared-raster LRU adapters and model viewer RAM cache; independent RAW/raster CLI limits and pin/transition regressions | Broader admission coverage and physical RSS qualification; managed budgets account for tracked allocations and external buffer owners, not total process RSS |
| Disk size/count/TTL | Persistent RAW policy and real CLI regression with two CR3s | Raster/model persistence and broader concurrent policy qualification |
| Buffers | RAW and raster PixelBuffer carry lifetime reservations; generic raster RGBA output and sRGB/ICC display output reserve before allocation; ICC row buffers are budgeted; disk/swap restore reserves before pixel allocation; MemoryBudget::child supports local caps under shared ancestor budgets | Optional --managed-memory-mb shares allocation admission across managed buffers and RAW restore; spill/write queue occupancy is independent to avoid charging shared pixels twice; native CR3/DNG/Camera TIFF source/output reserve before read/assembly (EOS R8, synthetic DNG and seven real camera fixture exact-pixel/peak-budget tests); extend to remaining viewer pools and decoder scratch/output allocation admission; specialized raster and model pre-allocation admission; ModelBuffer now retains geometry reservations; GPU staging accounting |
| Swap | Shared ImageSwapCache engine for RAW/raster streaming codecs; independent optional RAW/raster viewer spill/restore, independent queue occupancy, disk bytes/count/TTL, checksum/cancellation and managed restore; raster HDR/metadata/pressure regressions | STL/OBJ/PLY/OFF model swap is also integrated with independent disk bytes/count/TTL, queue and restore budgets; end-to-end physical navigation, cold-storage and sustained pressure measurements remain |
| Previous/next preloading | Configurable navigation-relative RAW RAM/disk warming with shared managed-budget admission and foreground priority; prepared raster/model neighbour RAM warming and bounded generation work | Broader raster subimage preload; physical navigation qualification |
| GPU/shaders/Metal | RAW/raster/model render paths, explicit auto/Metal/Vulkan/DX12/GL API selection and shared test mask; 73 tests passed with forced actual Metal; explicit NVIDIA numeric-vendor/surface filter and no-fallback absence regression | Physical presentation and broader devices; resource-budget logical ownership is tested, driver/in-flight physical retention remains outside those caps |
| Compute/CUDA/NVIDIA | LinearExposureCompute WGSL storage-buffer pass; actual Metal Apple M4 Max readback verifies HDR RGB, unchanged alpha and dispatch tail | Viewer integration, sustained resource/latency qualification, CUDA path and NVIDIA hardware execution evidence |
| HDR | Native float samples and RGBA16F raster intermediate retain values above one; explicit scene-linear RAW mode has Rgba32Float Metal readback; RRRAH_HDR_SURFACE=1 selects advertised Rgba16Float/ExtendedSrgbLinear or explicitly refuses; live Metal configuration validated | Current live probe presented RGBA16Float / ExtendedSrgbLinear without validation errors; current headroom was 1.0, potential 16.0. Output transfer/primaries/luminance controls and physical HDR display qualification remain |

The current GPU run is recorded at /tmp/rrrah-gpu-current-tests.log. Its passing
readbacks validate their fixtures and assertions, not universal GPU/device support
or HDR display. Window presentation and CUDA/NVIDIA remain unproven.

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
CMYK8 whereas the native PDF uses float CMYK; this is not a float-color oracle.
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
