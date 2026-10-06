# Rrrah

Rrrah is a native Rust viewer whose first displayed image is developed from the
sensor mosaic itself. It never substitutes the embedded JPEG for the main view.

The current fast paths are deliberately bounded and measurable:

1. parse Canon EOS R8 CR3/CRX, TIFF/DNG, or a TIFF-family camera RAW (CR2,
   NEF, ARW, ORF, PEF, RW2, RAF) and decode the sensor samples in native Rust;
2. cache that decoded mosaic;
3. upload it once as an integer GPU texture;
4. normalize, demosaic, white-balance, color-convert, and tone-map only the
   visible viewport in WGSL.

The CR3 backend accepts the confirmed full-resolution, one-tile, 14-bit Canon
EOS R8 profile. The DNG backend accepts bounded classic-TIFF and BigTIFF CFA
DNGs in either byte order, with 8–16-bit uncompressed or lossless-JPEG
strip/tile storage. It applies `LinearizationTable`, preserves levels, crop,
orientation, white balance and `ColorMatrix1`. Display supports 2×2 RGB Bayer
and 6×6 RGB X-Trans CFA layouts.

Unsupported DNG features are rejected explicitly: LinearRaw/non-CFA images,
lossy JPEG, Deflate and JPEG XL compression, unsupported required opcodes,
`BlackLevelDeltaH/V`, fractional display crops, and other CFA layouts. The default build has no
external RAW-decoder dependency.

Seven camera formats share a bounded clean-room TIFF reader with the DNG
backend and decode real sensor data through the same mosaic pipeline. Each
one accepts only the storage variants it can verify and rejects the rest with
typed errors — the embedded JPEG is never substituted:

- **Canon CR2** — single-strip lossless JPEG with CR2 vertical-slice scatter.
  Other compressions, subsampled sRAW/mRAW, tile/multi-strip storage, and the
  EOS-1D two-column width quirk are rejected.
- **Nikon NEF** — uncompressed 12/14-bit MSB-packed strips and Nikon lossless
  (compression 34713: makernote Huffman tree + linearization curve, decoded
  by a native bitstream decoder modelled on rawspeed's `NikonDecompressor`).
  The Z-series high-efficiency/lossy variants, multi-strip 34713, and exotic
  curve variants are rejected.
- **Sony ARW** — uncompressed 16-bit and LSB-packed 12/14-bit rows, ARW 2.x
  cRAW block-delta, and lossless JPEG (Alpha 1+). The ARW 1.0 Huffman/curve
  encoding is rejected.
- **Olympus ORF** — `IIRO`/`IIRS`/`MMOR`/`MMSR` containers with Olympus
  12-bit packing, uncompressed 16-bit, adaptive Olympus entropy (verified
  on E-M5), or single-strip lossless JPEG. Other bitstreams remain unsupported.
- **Pentax PEF** — uncompressed MSB-packed 12/14/16-bit, TIFF/EP lossless
  JPEG (K-x and newer), and Pentax lossless (compression 65535: makernote
  Huffman table 0x0220, custom predictor). Multi-strip 65535, odd widths, and
  files missing the makernote table are rejected.
- **Panasonic RW2** — `IIU\0` container with 16-bit uncompressed rows, the
  Panasonic packed 12/14-bit bitstream (dcraw `panasonic_load_raw`
  semantics), or single-strip lossless JPEG. Legacy Panasonic compression
  codes are rejected.
- **Fujifilm RAF** — `FUJIFILMCCD-RAW` container with Bayer 12/14-bit
  LSB-packed, 16-bit, or lossless-JPEG storage. A valid RAF-directory X-Trans
  pattern enables the 6×6 development path for supported storage. Rotated
  Super-CCD and Fuji's proprietary compressed format remain rejected.

Other camera backends require a 2×2 RGB Bayer CFA for display.

`--raw-quality` enables CPU AHD development and spatial highlight recovery;
`--tone-curve` supplies a monotone display curve. Supported DNG opcodes and
X-Trans automatically select CPU development. See [RAW_QUALITY.md](docs/RAW_QUALITY.md)
for usage, supported opcode stages, memory costs and qualification limits.
Color calibration uses embedded DNG matrices where present, otherwise an exact
make/model match in the bundled 839-entry camera matrix table. The data's
source, revision and CC-BY-SA 3.0 license are recorded in
[`CAMERA_PROFILES.md`](crates/rrrah-decode/data/CAMERA_PROFILES.md).
As-shot WB is read from DNG tags or bounded vendor metadata: Canon ColorData,
Sony's encrypted private IFD, Nikon's unencrypted WB_RBLevels, Olympus
multipliers/ImageProcessing, Pentax WB_RGGBLevels, Panasonic channel levels,
and Fuji channel levels. Unsupported WB layouts (including encrypted Nikon
variants) and unknown camera calibrations are explicit errors. Neither unity WB
nor identity color matrices are substituted on the production camera path.
DNG display likewise requires as-shot WB and a usable embedded or camera-table
matrix; GPU upload rejects invalid transforms. RAF's camera model comes from
its fixed header. These are baseline linear color calibrations, not camera JPEG
looks. Nine public CC0 RAW cases and two local EOS R8 CR3 captures match
independent LibRaw sensor, WB and final matrix references. CPU/GPU color,
minification and orientation have separate rendering checks; exact commands,
normalization policies and qualification limits are recorded in
[`RAW_QUALIFICATION.md`](docs/RAW_QUALIFICATION.md).
Additional opt-in camera regressions use `RRRAH_<FORMAT>_FIXTURE` (see
[`docs/DECODE_FORMAT_AUDIT.md`](docs/DECODE_FORMAT_AUDIT.md) §4).

Detailed design, equations, budgets, and benchmark protocol live in
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
The full-resolution tiled phase is implemented as a first atlas-backed step and
specified in
[`docs/TILED_PIPELINE.md`](docs/TILED_PIPELINE.md) and
[`docs/TILED_MATH.md`](docs/TILED_MATH.md); it replaces the temporary
large-RAW downsample fallback with GPU tile residency.

The production editor roadmap is [EDITOR_100.md](docs/EDITOR_100.md), with the
mathematical contract in [EDITOR_MATH.md](docs/EDITOR_MATH.md) and the canonical
benchmark matrix in [BENCHMARK_MATRIX.md](docs/BENCHMARK_MATRIX.md).
The live HUD/telemetry design is [LIVE_BENCHMARKS.md](docs/LIVE_BENCHMARKS.md),
and the twenty-role review is [BENCH_AGENT_REVIEW.md](docs/BENCH_AGENT_REVIEW.md).
The native DNG correctness and paired wall-time comparison is
[DNG_BENCHMARK_2026-07-23.md](docs/DNG_BENCHMARK_2026-07-23.md).
Current CR3 decode, completed GPU frame and resident zoom/pan measurements are
in [RAW_LOAD_PERFORMANCE.md](docs/RAW_LOAD_PERFORMANCE.md).
The parameter-sweep matrix and runnable synthetic GPU smoke are documented in
[PARAMETER_SWEEP_ARCHITECTURE.md](docs/PARAMETER_SWEEP_ARCHITECTURE.md) and
[GPU_SYNTHETIC_SWEEP_GATES.md](docs/GPU_SYNTHETIC_SWEEP_GATES.md).
The extension backlog is [EDITOR_101_200.md](docs/EDITOR_101_200.md), with its
math contract in [EDITOR_200_MATH.md](docs/EDITOR_200_MATH.md) and critical stop
conditions in [EDITOR_100_CRITIQUE.md](docs/EDITOR_100_CRITIQUE.md).

The 50-role competitor/paper/practice audit, current implementation scorecard,
parallelism model, innovation review, and production gates are consolidated in
[`docs/RESEARCH_DEEP_DIVE.md`](docs/RESEARCH_DEEP_DIVE.md). Supporting evidence is
split into [competitors](docs/RESEARCH_COMPETITORS.md),
[papers](docs/RESEARCH_PAPERS.md), and [practice/forums](docs/RESEARCH_PRACTICE.md).

The execution breakdown for ingest, scheduler/GPU residency, quality, and their
adversarial critic gates is [IMPLEMENTATION_AGENT_PLAN.md](docs/IMPLEMENTATION_AGENT_PLAN.md).
The three detailed work packages are [ingest/tiles](docs/PLAN_INGEST_TILES.md),
[scheduler/residency](docs/PLAN_SCHEDULER_RESIDENCY.md), and
[quality/critic](docs/PLAN_QUALITY_CRITIC.md).

Dependency, security, test, benchmark and lint audits are tracked in
[DEPENDENCY_UPDATE_AUDIT.md](docs/DEPENDENCY_UPDATE_AUDIT.md),
[SECURITY_DEPENDENCY_AUDIT.md](docs/SECURITY_DEPENDENCY_AUDIT.md), and
[TEST_BENCH_LINT_AUDIT.md](docs/TEST_BENCH_LINT_AUDIT.md).
The latest GPU, decoder, fuzz, cache and final adversarial reviews are
[GPU_VALIDATION_AUDIT.md](docs/GPU_VALIDATION_AUDIT.md),
[DECODE_FORMAT_AUDIT.md](docs/DECODE_FORMAT_AUDIT.md),
[FUZZ_HARDENING_AUDIT.md](docs/FUZZ_HARDENING_AUDIT.md),
[CACHE_STRESS_AUDIT.md](docs/CACHE_STRESS_AUDIT.md),
[CI_LINT_AUDIT.md](docs/CI_LINT_AUDIT.md), and
[FINAL_AUDIT_CRITIC.md](docs/FINAL_AUDIT_CRITIC.md).
The latest UI benchmark run and its explicit fixture gate are recorded in
[UI_BENCHMARK_RUN_2026-07-21.md](docs/UI_BENCHMARK_RUN_2026-07-21.md).
The folder gallery architecture, preload policy, security gates, and benchmark
contract are recorded in [GALLERY_ARCHITECTURE.md](docs/GALLERY_ARCHITECTURE.md).

## Run

```bash
cargo run --release -p rrrah -- --no-cache path/to/image.CR3
cargo run --release -p rrrah -- --no-cache path/to/image.DNG
cargo run --release -p rrrah -- --no-cache --inspect path/to/image.DNG
```

Controls: drop a supported RAW file or folder onto the window; a dropped folder
opens its first CR3/DNG/TIFF and `←`/`→` navigate the folder. Mouse wheel zooms, left-drag
pans, `+`/`-` changes exposure, `F` returns to fit, and `R` resets the view.
The in-image HUD reports decode/cache/adapt/upload/open timings and a live frame
encode sample.

Persistent RAW write-back uses a one-job pending queue and an independent `--cache-write-queue-mb` budget (default 128 MiB; zero skips writes). The budget includes active and pending pixel-buffer capacity and is released when the job is dropped. Admission never copies sensor samples and does not block the foreground loader. Active foreground write-back checks generation cancellation during hashing, block writes and disk-lock retries; canceled jobs release their retained-byte reservation and count as superseded rather than write failures. This retained-buffer limit excludes metadata and untracked decoder allocations.

`RasterRamCache<K>` provides independent size/count/TTL admission for decoded or prepared rasters, charging allocated sample capacity. Foreground transition works at count one and failed admission restores the previous pin. Hits clone original u8/u16/f32 storage without changing HDR bits or color metadata. The viewer now caches prepared rasters using path, sampled source fingerprint, image index and exact scalar-window bits. It retains the untagged-sRGB indicator across hits. RAW uses `--ram-cache-mb/count/ttl-secs`; prepared rasters independently use `--raster-cache-mb`, `--raster-cache-count` and `--raster-cache-ttl-secs` (default 512 MiB, unlimited count/TTL). These are separate capacities, not one combined application budget. `--no-cache` disables raster admission. Prepared raster neighbours now warm in the idle foreground worker alongside models, using the previous/next window, generation cancellation and pin-aware background admission. Subimage zero is warmed. If the selected request has a scalar window, neighbours are attempted with the same exact bounds; ordinary rasters can fall back to their no-window key. Scientific data requiring a window is warmed only when bounds were explicitly supplied. The foreground still chooses its own window/key, so warming never substitutes another interpretation. Optional raster swap is configured independently with `--raster-swap-mb/count/ttl-secs/queue-mb/restore-mb`. Membership limits do not bound buffers held by external owners.

### RAW neighbour prefetch

`--prefetch-behind N --prefetch-ahead M` controls how many full RAW files are warmed after the foreground frame is ready. Aliases are `--prefetch-previous` and `--prefetch-next`. Defaults are 2 behind and 5 ahead in the direction of navigation; moving backward swaps their sides in the sorted gallery. Zero disables that side; zero for both disables neighbour work. Counts clamp to gallery bounds, exclude the selected file and retain foreground priority/cancellation. Speculative disk writes check cancellation while hashing/writing 16 KiB blocks, before/after data sync and before publication. Cancellation preserves an existing destination and removes the temporary file; write-lock contention uses nonblocking attempts with cancellation checks and a 2 ms retry interval. An already running filesystem sync remains uninterruptible. The window also controls prepared raster/model neighbours warmed into their separate RAM caches during idle foreground queue periods. RAW neighbours now warm decoded RAM entries on the idle loading worker and submit newly decoded frames to persistent write-back. New foreground generations cancel speculative decode and replace the bounded neighbour path plan.

### Memory cache and swap libraries

`rrrah-memory` is format-independent: a shared reservation budget, flat mutable/immutable buffers and the generic cache policy used by the existing RAW adapter. Buffer clones retain their reservation until the last owner drops; adopting a vector preserves its allocation and samples. Buffer capacity is accounted separately from cache membership and excludes allocator bookkeeping, metadata and untracked allocations. RAW mosaics now use `PixelBuffer<u16>`: managed storage retains its reservation through cache/UI/GPU handoff; legacy `Arc<Vec<u16>>` storage remains unaccounted until exclusive zero-copy adoption via `try_manage_pixels`. Shared legacy allocations are rejected to avoid losing accounting when an external owner survives. Decoder allocation admission is not yet wired to the shared budget.

The viewer's RAW RAM cache accepts independent `--ram-cache-mb`, `--ram-cache-count` and `--ram-cache-ttl-secs` limits. Count and TTL are unlimited by default; size defaults to 2 GiB. TTL is age since insertion/replacement, not idle age. Expired pinned entries cannot produce hits but stay stored until protection is released. Zero TTL expires immediately; zero count refuses admission. Foreground replacement can replace the previously visible entry even at count one; rejected replacement restores the previous pin. RAM admission charges allocated pixel capacity, including spare vector capacity. These flags limit cache membership, not total application RAM.

`rrrah-swap` stores immutable temporary blobs in a private session directory, with independent byte/object quotas, shared handles, BLAKE3 integrity checking and cancellation at 64 KiB I/O boundaries. In-progress writes and retained handles consume disk quota. Managed restore reserves RAM first and publishes only verified immutable buffers. The store performs blocking I/O for a caller's worker; it does not provide crash durability or expire retained dirty data. The viewer supports optional RAW spill/restore with `--swap-mb`, `--swap-count` and `--swap-ttl-secs`; swap is disabled by default and by `--no-cache`. Evicted RAM entries are streamed through a dedicated writer with a four-job channel and a retained pixel-capacity budget (`--swap-queue-mb`, default 128 MiB). Saturated queues skip disposable spill work. Verified restores run on the foreground loading worker and are readmitted to RAM; failures fall through to persistent cache/source decoding. This tier uses the existing mosaic payload schema. Restored pixel buffers reserve capacity before allocation and retain their reservation through all frame owners. The restore pool has its own `--swap-restore-mb` limit (default 512 MiB; zero refuses restoration), independent of RAM-cache size and write-queue capacity. When swap is enabled, foreground persistent-disk-cache reads share this same restore budget. The limit covers all still-owned restored pixel allocations from both tiers, including frames evicted from RAM cache. Allow room for the previous visible frame and its successor during transitions; metadata, I/O scratch and legacy decoded frames remain outside this pool. Memory-pressure misses preserve the swap entry for retry. Foreground persistent-cache reads support generation cancellation before allocation, between 16 KiB pixel blocks and before frame publication; canceled reads release their pixel reservation and preserve the cache file. Migration of decoder/RAW/GPU staging allocations to the shared budget remains pending; queue accounting does not bound total process RAM. Existing persistent mosaic formats remain separate.

The persistent disk-cache library also supports an independent `with_max_entries` limit. Count admission and byte pruning share the interprocess write lock; replacing the same key consumes one entry, zero rejects writes, and count eviction removes the oldest written entries. The library also offers `with_ttl`: file-write age determines expiry, reads do not renew it, zero TTL always misses. Expired entries are removed under the write lock during the next store; read probes leave files in place to avoid deleting a concurrent replacement. Unavailable/future modification times miss when TTL is enabled. The viewer and `--inspect` expose the same policy through `--disk-cache-mb`, `--disk-cache-count` and `--disk-cache-ttl-secs`. Foreground reads/write-back and RAW neighbour prefetch share the configured policy. Defaults retain the existing disk byte limit with unlimited count/TTL.

### CR3 streaming buffer tuning

The “four buffers” often visible in CR3 diagnostics are the four independent
parity planes (R, G₁, G₂, B), not a user-selectable queue count. The streaming
assembler defaults to 32 rows per batch and queue depth 1, with two reusable
batch vectors per plane.

For repeatable experiments only, the bounded alternatives can be selected with:

```bash
RRRAH_CR3_STREAM_BATCH_ROWS=8|16|32|64|128
RRRAH_CR3_STREAM_QUEUE_DEPTH=1|2|4
```

Invalid values fall back to 32 rows / depth 1. The defaults remain the measured
low-memory choice; deeper queues reserve more scratch memory and did not show a
stable wall-time improvement on the current EOS R8 fixture.

## Status

Work toward broad image support is tracked in
[IMAGE_FORMATS_100.md](docs/IMAGE_FORMATS_100.md). The library now exposes
`rrrah_decode::decode_image_file` for content-routed sensor/raster decoding and
`rrrah_decode::decode_raster_file` for first-image raster decoding, separately
from sensor mosaics, retaining 8/16-bit or float RGBA, orientation and ICC
profile bytes. Color-prepared raster stills now use a dedicated GPU/window path, including
folder navigation and thumbnails. RGB ICC conversion is available; unsupported
source color spaces are explicit errors. Raster cache, animation/page handling
and qualification of all 100 target formats remain in progress.

This is an architecture-first prototype. It provides real native EOS R8 CR3,
bounded CFA DNG, and CR2/NEF/ARW/ORF/PEF/RW2/RAF full-RAW decode, full-resolution tiled GPU upload for adapters
with texture-array capacity, per-stage timing instrumentation, total wall time,
and warm-open cache. It is not yet a replacement for a color-managed production
raw developer. Additional camera profiles and DNG feature families require
separate framing, metadata and pixel-oracle validation.

Aseprite frames (`ase`/`aseprite`) use an owned parser with selected cel inflation, linked storage, palettes, normal layer/group composition and ICC/linear color preparation. Frame timing and tag metadata are available through `decode_aseprite`; live playback and other blend/tilemap modes remain in progress.

JPEG-XR stills (`jxr`/`wdp`/`hdp`) use a static JXRLib decoder, retain 8/16-bit or HDR float samples, separate alpha and ICC, and apply all container orientations. Broader layout and interoperability qualification remains tracked in the format matrix.

AVIF decoding requires libdav1d >=1.3 and pkg-config (`brew install dav1d pkg-config` on macOS; `apt install libdav1d-dev pkg-config` on Ubuntu). Runtime decoding uses dav1d directly through image, without FFmpeg.

HEIF/HEIC HEVC still-image decoding requires libheif >=1.17 and its HEVC decoder (`brew install libheif` on macOS; `apt install libheif-dev libde265-dev` on Ubuntu). The primary image retains straight alpha, item orientation, ICC and 8/10/12-bit sample precision; broader HEIF variants remain qualification targets in `docs/IMAGE_FORMATS_100.md`.

ASTC decoding bundles Arm astcenc through astcenc-sys. Building it requires a C++14 compiler and libclang for generated bindings (`apt install g++ libclang-dev` on Ubuntu; Xcode Command Line Tools on macOS). No astcenc CLI is required at runtime. LDR textures use an explicit assumed-sRGB import; HDR is retained as float data with unspecified RGB primaries.

DCX pages, WAD3 textures and scientific-array slices can be selected with `--image-index` (zero based), for example `cargo run -p rrrah -- --image-index 1 textures.wad`. The same option works with `--inspect`. In the viewer, `[` selects the previous image and `]` the next image within the current container; Left/Right still navigate files. The title shows the current image and total count; bracket navigation stops at the first/last image. Explicit out-of-range CLI selection reports an error. Opening another file resets its image index to zero. Sensor RAW accepts only index zero.

Attached scalar NRRD arrays are available through `decode_nrrd`: uint8/uint16/int16/float32, raw/gzip/text and selected volume slices. Raw numeric values and header descriptors are retained. Use `NrrdImage::windowed(minimum, maximum)` to produce explicit linear grayscale for display. For the viewer, provide `--window MIN MAX`; automatic window estimation is pending.

MRC2014 scalar sections are available through `decode_mrc`: signed 8/16-bit, unsigned 16-bit and float16/32, both byte orders. Header and extended metadata are retained; slice indices use storage order. `MrcImage::windowed` applies an explicit grayscale range. Physical-coordinate visualization and automatic window estimation are pending.

To display a scientific array, supply an explicit finite range in raw sample units:

```sh
cargo run -p rrrah -- --window -32768 32767 --image-index 1 volume.mrc
cargo run -p rrrah -- --inspect --window -100 1000 --image-index 1 volume.nrrd
```

With a scientific window active, Up/Down shift its center by 10% of its width; PageUp widens it by 25%, and PageDown narrows it by 20%. Brackets change the slice while keeping the range. The window title shows the active bounds. Opening another file clears the range. Invalid ranges, missing-value ambiguity and attempts to window an ordinary color image report errors. Native keyboard/window qualification is still pending.

FITS primary/IMAGE HDUs use `decode_fits`, retaining native integer/float sample types, scaling and original header cards. `--image-index` selects a plane across image HDUs and `--window MIN MAX` uses physical values after BSCALE/BZERO. Empty HDUs and tables are skipped. BLANK/nonfinite values require an explicit missing-value policy; float32 conversion rejects precision loss. Higher-precision raw samples remain accessible via `FitsSamples`. Compressed images and WCS sky projection remain pending.

PVR v3 textures support ordinary normalized 8/16-bit and float32 RGB channels, plus PVRTC1 2/4 bpp. `--image-index` selects a mip level; bracket navigation preserves mip count/selection metadata. Declared transfer and X/Y orientation are applied. Linear premultiplied alpha becomes straight float samples; other associated-alpha interpretations require further qualification. Legacy PVR, packed channels, other codecs and cube/array/volume selection remain pending.

APNG supports selected 8-bit animation source frames via `image_index`, presented as linear sRGB float32 after color-aware SOURCE/OVER composition and disposal. `decode_apng` retains rational delay, repeat count and source color declaration. A separate default preview is excluded from animation indices. Live timed playback, wider sample precision and unqualified color/orientation variants remain pending; unknown source colors require interpretation rather than a silent guess.

### 3D model library

`rrrah_decode::decode_model` loads binary/ASCII STL, indexed OBJ ASCII/binary LE/BE PLY meshes and ASCII/binary OFF meshes, including bounded concave-polygon triangulation, source metadata, bounds and cancellation. These formats open in the existing window with depth-tested GPU rendering, drag-to-orbit, wheel zoom and F reset; CLI inspection and mixed-folder discovery are connected. PLY/OFF source attributes and float64 coordinates are retained; world coordinates are centered before GPU float32 conversion. Model previews, point clouds, material/color interpretation and broader 3D formats remain pending; see [MODEL_FORMATS.md](docs/MODEL_FORMATS.md) for validation boundaries and speed measurements.

### Shared managed RAW admission budget

`--managed-memory-mb N` optionally caps the combined reservations of RAW
swap restoration and managed decoder/display buffers. Restore admission
retains its own local limit. Omitting the option preserves independent
budgets; zero rejects positive reservations. Restored buffers keep their
reservation while retained by RAM cache or any external owner. Swap and persistent-write queues have independent occupancy budgets, counting
active and pending pixel capacity. These budgets do not charge the shared RAM
root again: managed pixel owners retain the original allocation reservation
until the last job/frame drops. Legacy untracked buffers remain outside the
shared RAM root. Native CR3, DNG and Camera TIFF (CR2/NEF/ARW/ORF/PEF/RW2/RAF) decoding
reserve compressed input before reading and their full sensor output before
assembly, then transfers the output reservation to the returned
pixel buffer. Managed reads check cancellation between 64 KiB blocks and reject
source-length changes. This is not a total
process-RAM limit: entropy/plane/segment scratch, other native
decoder outputs, transient raster/model parser allocations, metadata, allocator overhead and GPU
memory remain outside it. Persistent disk reads share restore admission when swap is enabled and use the
common budget directly when swap is disabled.

On foreground image-kind detection or RAW disk-read/decode admission failure, the loader releases one
unpinned LRU entry and retries until admission succeeds or no victim remains. If the requested reservation
exceeds the reported limit, the cache is preserved because eviction cannot help.
Pressure victims are dropped without queued swap writes so the queue cannot
retain their pixels. The visible frame remains protected; external owners and
active background jobs may still hold reservations. This does not guarantee
that a budget smaller than the simultaneous visible/input/new-output working set
can display the next image.

RAW neighbour prefetch uses the same optional managed budget as foreground
decoding. It skips failed admission and can retry on a later navigation command;
it never evicts foreground RAM entries to make room. A local EOS R8 worker test
checks that an exhausted shared budget publishes no persistent entry, and that
releasing the reservation allows a later prefetch to store the frame and release
its managed input/output allocations.

Image-kind detection reads its 256-byte header into fixed stack storage. Generic
TIFF sensor classification uses the managed whole-file reader when a request
budget is present, releases that source after classification, and propagates
admission failure. Header-only recognition does not consume a heap reservation.

The common native bounded source reader now returns ownership-bound managed
storage whenever DecodeRequest carries a memory budget. This covers raster and
model readers using that helper as well as RAW. Raster parsers can normalize CUR
headers in place without copying the input. Prepared raster/model neighbour
requests receive the same shared budget. Returned raster pixel buffers now adopt exclusive storage into the shared budget
without copying and retain the reservation through cache/viewer clones. Admission
failure returns a typed memory error and releases the rejected output. This is
post-allocation admission for specialized native raster parsers. The common
`image` decoder path reserves final RGBA capacity from header dimensions before
pixel decode and transfers that reservation into the result without copying.
Codec scratch and overlapping orientation/channel-conversion allocations are
not counted; specialized parser output can still transiently exceed the budget
before adoption. Model output now retains post-allocation admission as described below.

RAW swap restore distinguishes allocation admission failure from absence or
corruption through `MosaicSwapCache::try_get`. The RAM adapter drops unpinned
entries and retries temporary pressure, preserving the visible frame and swap
entry. If requested allocation exceeds the reported budget limit, eviction is
skipped because it cannot make admission possible. Allocation failure does not
increment swap corruption/read-error counters.

`--inspect --managed-memory-mb N` applies the same source/output admission to
headless native decoding and persistent RAW restore. It prints managed used,
peak and limit bytes on success. Real-process CLI tests verify 96 MiB full CR3
decode, 64 MiB persistent restore, refusal of a 64 MiB fresh decode and zero-budget
restore, and preservation of the existing persistent cache entry on refusal.
These are managed counters, not total process memory.

KRA/ORA ZIP members (MIME, XML and merged PNG) reserve declared expanded length
before decompression into managed storage. CRC/read completion, expansion limits
and cancellation checks remain active. Member buffers retain admission through
parsing or embedded-image decode and release it on drop. Tests cover all four
plain/profiled KRA/ORA fixtures with exact pixels/profile preservation and
source-only-budget rejection. Merged PNG output retains managed admission after decoding. Transient PNG codec
allocations and XML/ZIP parser internals remain outside the managed budget.

Raster display preparation accepts the same optional shared budget through
`prepare_raster_for_display_with_budget`. Foreground and prepared neighbour
loads reserve linear RGBA32F output before allocation for sRGB and RGB ICC
conversion. The returned buffer keeps that reservation through cache/viewer
ownership; an already linear float frame reuses its buffer without another
reservation. The two f64 ICC row buffers also reserve memory before allocation and release
it after conversion. ICC transform tables remain outside the budget.
The thumbnail worker still uses the legacy unbudgeted preparation API.

Raster cache misses retain typed decode/color allocation errors until the retry
decision. Recoverable capacity pressure releases one unpinned prepared raster
and retries; the visible frame remains pinned. Requests larger than the reported
budget limit and non-memory errors do not trigger this eviction. External frame
owners can retain memory after eviction, so retries stop when no victim remains.
Source-kind detection can release unpinned raster, model and RAW entries under shared
pressure, retrying one victim at a time. A real TIFF classification regression
verifies raster neighbour release, visible-frame preservation and no eviction
for zero-budget admission. Cross-pool decode admission still needs integration.

`--raster-gpu-mb N` optionally limits renderer-owned RGBA32F raster texture
footprint. Zero refuses uploads; omitting it preserves the existing texture
limit. Admission occurs before texture creation. Replacement requires room for
old and new texture footprints together, and refusal preserves the existing
image. Clear/drop releases its reservation. This is a logical texture budget,
not physical VRAM: driver allocation, padding, uniforms, queue staging,
in-flight command retention, RAW/model/filmstrip resources and render targets
are excluded. It is independent of the managed CPU memory budget.

The shared `decode_model` entrypoint admits STL/OBJ/PLY/OFF retained geometry
capacity after parsing and transfers exclusive mesh ownership into ModelBuffer.
Its private reservation follows all ModelBuffer/DecodedModel clones, including
cache and viewer owners; the underlying mesh Arc cannot be extracted separately.
Admission includes the existing conservative nested vector/attribute/string
capacity weight. Shared legacy mesh adoption is rejected instead of allowing
owners to escape the reservation; already managed meshes retain their original
budget. Source read admission remains in force. Parser construction, temporary
triangulation buffers and allocator/ownership metadata are not included: this
bounds retained geometry and is not pre-allocation model admission.

Binary STL through `decode_model` now reserves the mesh object and declared facet
capacity before facet-array allocation/decoding, while managed source input is
still retained. The reservation transfers into the returned ModelBuffer; actual
vector capacity beyond the estimate requires additional admission. Exact-length
binary detection and the 512 MiB facet ceiling remain unchanged. Tests verify
input+geometry peak, rejection before non-finite facet parsing, cleanup after
parse errors and last-owner retention. OBJ/PLY/OFF still use post-allocation output admission; direct legacy `decode_stl` returns an unwrapped
mesh and does not retain output admission.

ASCII STL now counts facet records with periodic cancellation checks before
allocating geometry, and reserves exact facet-array capacity in the shared model
entrypoint. Parsing fills that capacity without geometric-growth allocations.
The direct legacy STL API also uses exact capacity but retains its legacy output
ownership. Synthetic one-facet tests verify capacity 1 instead of the former
1024-facet first growth, input+geometry reservation peak, memory refusal before
non-finite coordinate parsing, cancellation during count and release on error.

The source-kind pressure path now also considers ModelDisplayCache geometry
owners, after raster victims and before RAW victims. It drops one unpinned entry
per retry and never bypasses visible-model protection. A real OFF fixture with
retained spare geometry capacity verifies refusal while pinned, managed memory
release after unpin/eviction and subsequent TIFF classification on the same
budget. Model decode failures now retain typed admission errors in the model cache
loader, as described below.

Model foreground and neighbour cache misses now preserve ModelDecodeError until
admission retry is decided. Source-buffer failures from all four parsers and
retained-geometry admission failures may release one unpinned model per retry.
Visible models stay pinned while decoding; cancellation/format errors and
reservations larger than the reported limit do not evict. A real OFF-to-STL
regression fills the shared budget, verifies zero-budget refusal preserves both
entries, releases one OFF neighbour on recoverable pressure, completes STL on
the second attempt and verifies reservations follow the retained result after
cache drop. Cross-pool decode retry beyond model-cache victims remains pending.

`--model-gpu-mb N` optionally caps owned model vertex buffers and Depth32Float
viewport attachment footprint, independently of CPU and raster texture budgets.
Both upload and resize reserve before resource creation and include replacement
overlap. Refusal retains previous resources; unchanged depth dimensions reuse
existing admission. `clear_model` releases vertices but retains cached depth and
its reservation; renderer drop releases both. Zero refuses positive resources.
Uniforms, driver overhead, staging and in-flight command retention are excluded:
this is logical owned-resource admission, not a physical VRAM cap.

`--gpu-memory-mb N` creates a shared parent budget for managed RAW/raster/model
GPU resource footprints. --raw-gpu-mb, --raster-gpu-mb and --model-gpu-mb remain independent
child caps; omitted child caps inherit the parent's limit. With no parent option,
the existing independent optional caps are preserved. Parent and local admission
both apply, including old/new replacement overlap. Zero refuses positive
admissions. Filmstrip/other render targets, uniforms, staging, driver
and in-flight retention remain excluded; the option is not a total VRAM limit.
A Metal mixed-resource test verifies parent exhaustion, local-cap refusal despite
parent room, release/retry and last-renderer reservation cleanup.

`--raw-gpu-mb N` applies local admission to owned R16Uint RAW atlas footprint,
including every padded tile halo and array layer. It participates in the same
--gpu-memory-mb parent as raster/model resources. Admission precedes texture
creation and parameter mutation; refusal preserves the previous image. Successful
replacement retains old+new credit until resource replacement, and clear/drop
releases the atlas reservation. Metal tests verify padded footprint exceeds source
sensor bytes, larger replacement refusal, exact overlap peak and release/retry.
CPU tile packing can now use the managed CPU budget as described below; upload
staging and in-flight/driver retention remain excluded.

RAW upload connects `--managed-memory-mb` to transient halo-tile and aligned-row
CPU packing admission. `RawRenderer::with_upload_memory_budget` reserves the
maximum simultaneous footprint of one u16 tile and its optional padded byte copy
before atlas creation and packing. Sequential tiles reuse this credit; actual
Vec capacity growth requires additional admission. Reservation drops when upload
returns. Parameters are prepared locally and committed only with successful
resource replacement, so packing refusal preserves the old image. Metal tests
verify the exact managed packing peak, one-byte-short refusal, cleanup and retry.
RAW queued atlas payload is now separately admitted with
`RawRenderer::with_upload_queue_budget`; the viewer connects it to the same
managed budget. Admission covers aligned rows across every atlas layer before
texture creation. A completion guard submits pending writes and holds the
reservation until submitted GPU work completes, including early-return paths.
This bounds logical queued bytes, not driver overhead or physical staging RSS.
Device polling is required to deliver completion callbacks. Metal tests verify
queue refusal preserves the current image, multilayer admission and release
following completion after renderer destruction.

Managed cache-policy regressions cover independent raster byte/count eviction and
expired pinned raster/model entries. Weight uses allocated capacity, including
unused vector slots. TTL-expired protected entries miss without being evicted;
after unpin they can be released. Consumer-owned pixel/mesh clones keep their
reservation even after cache eviction/drop, until the final owner is dropped.
Deterministic zero-TTL tests exercise this boundary without wall-clock sleeps;
fixed-age/hit-recency policy tests remain in rrrah-memory.

A versioned streaming raster payload is now available in rrrah-cache through
raster_payload_len/write_raster_payload/read_raster_payload. It preserves RGBA8/u16/f32 sample bits, all raster color
declarations, ICC bytes, sample scale, hotspot and subimage selection. Writes
use direct byte views of immutable samples on little-endian targets; big-endian
writes retain at most 16 KiB scratch. Reads admit initialized pixels before
allocation and fill managed buffers directly, converting endian order in place
on big-endian targets. The 64-byte header validates dimensions, flags, limits
and metadata before pixels. Payload pixels are capped at 512 MiB and ICC at 1 MiB;
ICC allocation is bounded but not retained in the pixel memory budget. The swap
store supplies checksum/publication/cancellation through `ImageSwapCache<K,V>`.
`MosaicSwapCache` and `RasterSwapCache<K>` share that engine with streaming codecs.
`RasterRamCache::enable_swap` spills admission victims and restores misses on the
loading worker, retrying temporary pressure by releasing unpinned RAM entries.
Pressure victims are dropped without spill to release their managed allocation.
Disk bytes/count/fixed-age TTL, queue occupancy and restored allocation budgets
remain separate. A failed memory admission retains the disk entry; cancellation
returns a miss. Tests cover exact HDR/NaN/-0 bits and ICC/selection metadata,
full-parent-budget spill, external restored owners, independent limits, and RAM
spill/restore transitions. The viewer connects this adapter with `--raster-swap-mb` (default 0,
disabled), `--raster-swap-count`, `--raster-swap-ttl-secs`,
`--raster-swap-queue-mb` (default 128), `--raster-swap-queue-count` (default 4)
and `--raster-swap-restore-mb`
(default 512). `--no-cache` disables it. Swap needs the configured/default cache
root but does not depend on RAW swap being enabled. Restore admission is a child
of `--managed-memory-mb` when supplied; queue occupancy remains independent.
Prepared display pixels are restored under the same source-fingerprint,
subimage, scalar-window and assumed-sRGB key, bypassing decode/color preparation.
Generation cancellation is checked during store reads and before returning a
hit. Background restore does not transfer the visible pin. Real GIF and synthetic prepared-HDR
regressions cover subimage metadata, exact prepared HDR samples, decode bypass,
background protection and cancellation. Physical navigation latency and cold
storage qualification remain pending.


Single-part EXR with explicit linear Rec.709/D65 chromaticities or
`colorInteropID=lin_rec709_scene` now enters the linear display path with HDR
sample values retained. Missing/unknown declarations and unsupported primaries
stay unqualified; conflicting/malformed metadata fails explicitly. Other gamut
and white-point transforms and physical HDR output remain pending. See
[format contracts](docs/IMAGE_FORMATS_100.md).


Raster swap adversarial tests modify real stored blobs: truncated sample payloads
and checksum mismatches return misses, release partial restore allocations and
remove the invalid entry. Cancellation triggered after pixel admission releases
RAM while retaining the disk entry for retry. A same-key replacement injected
after the old handle is selected survives the old object's checksum failure;
handle identity prevents deleting the replacement. These are deterministic
store/adapter regressions, not crash-durability or sustained navigation tests.


RAW neighbour preload now shares the loader-owned RAM cache and the existing
navigation-relative previous/next plan. It checks RAM/swap, takes a low-priority
decode permit, rechecks the verified persistent cache, then decodes only on a
miss. Managed restores use the RAW restore child budget when swap is enabled.
Speculative memory-admission failure skips warming rather than evicting entries
for decode/restore pressure. Background swap hits preserve the visible pin.
Decoded neighbours are submitted to bounded persistent write-back; persistence
is opportunistic and generation cancellation may supersede jobs. When RAW RAM
admission is enabled, the separate disk-only warmer receives an empty window,
avoiding duplicate neighbour decodes. With RAM bytes/count zero, the disk-only
warmer retains the requested window. Real EOS R8 qualification explicitly runs
the normally ignored local-fixture test, checking native RAM warming, shared
RAM hits, exact verified disk restoration, cancellation, visible ownership and
final managed-memory release. Physical navigation latency remains unmeasured.


Speculative raster/model decode and raster swap restoration now skip temporary
memory pressure without discarding existing unpinned neighbours. Foreground
loads still release unpinned entries and retry, preserving the visible pin.
The shared-root regression fills RAM with a visible raster and a warmed
neighbour: a third background request fails after one admission attempt while
both entries remain; the same foreground request frees the neighbour and
succeeds on its second attempt. The swap regression exercises the corresponding
background refusal and later foreground restore against a full allocation root.


Raster payload u8/u16/f32 reads now use a safe Pod byte view of the exclusive
managed allocation, eliminating the former per-block staging copy/sample loop
on little-endian targets. u16/f32 writes likewise use immutable byte views;
big-endian wire conversion remains explicit. Independent expected LE bytes and
roundtrip tests cross multiple 16 KiB blocks with a partial final block, including
HDR, negative zero and NaN payload bits. Store checksum and cancellation paths
remain covered by adversarial tests. No viewer latency gain is claimed without
a before/after timing run; RAW restore already used direct managed-buffer reads.

`--gpu-backend auto|metal|vulkan|dx12|gl` selects the viewer GPU API for both windows;
explicit requests never fall back. The default is automatic selection. Adapter
logs include the actual backend/name/vendor/device. Readback qualification and
the view-timing example support `RRRAH_GPU_BACKEND` using the same selection
implementation. Metal passed on this host; Vulkan/NVIDIA and CUDA remain
unqualified/unimplemented respectively. See [GPU qualification](docs/GPU_COMPUTE.md).


`--gpu-vendor any|nvidia` independently filters the viewer adapter; `nvidia`
requires matching hardware compatible with each window and never substitutes a
different vendor. GPU tests/view timing use `RRRAH_GPU_VENDOR` with the same
selector. Explicit missing NVIDIA hardware fails qualification rather than being
turned into an optional hardware skip. Native NVIDIA execution remains pending.

Swap write queues have independent waiting-job count limits: `--swap-queue-count`
for RAW and `--raster-swap-queue-count` for rasters. Both accept 0 through 1024 and default to four waiting
writes, excluding the one active write. Zero disables new spills. The byte limit
includes both active and waiting payloads. Duplicate keys in the same generation
share one job; pressure cancellation releases retained owners asynchronously and
keeps completed disk objects.

Raster uploads also share the managed budget through
`RasterRenderer::with_upload_queue_budget`. The aligned RGBA32F row footprint
is admitted before texture replacement and held until GPU completion, including
renderer destruction. This caps logical queue payload; driver overhead and
physical staging RSS are excluded. Filmstrip thumbnail queue admission also shares this budget through
`FilmstripRenderer::try_upload_tile`; refused thumbnails keep their placeholder. Temporary pressure retains at most
one managed thumbnail for retry every 25 ms for up to two seconds; folder
changes cancel it, and the source stamp is rechecked before retry. Requests
exceeding the total quota are skipped immediately.

Filmstrip RGBA8 texture residency now participates in `--gpu-memory-mb` through
its own child budget. Individual tile removal releases its logical texture
credit. On texture-pressure refusal, the viewer removes one least-recently-used
thumbnail and retries the retained ready buffer. Queue-pressure refusal uses
retry without evicting textures. Count-based thumbnail LRU remains independent.
The cap excludes driver overhead, filmstrip vertex/uniform allocations and
resources retained by submitted GPU commands.

The viewer captures `GpuResourceLease` for each submitted frame: RAW atlas,
raster texture, model vertices/depth and filmstrip tiles retain their logical
resource reservations through queue completion even after replacement/removal.
Library callers should snapshot `resource_lease()` before replacing resources,
submit the encoded work, then call `retain_until_complete(&queue)` and continue
device polling. Filmstrip snapshots conservatively retain all resident tiles.
This covers listed frame resources, excluding uniforms, physical driver overhead and surface render targets.

RAW/raster/thumbnail upload guards now also retain destination texture credits
through submitted upload completion, independently of frame resource snapshots
and queued-byte accounting. Dropping or clearing a renderer before its first
frame does not release the upload's texture credit early. Poll the device to
advance these callbacks; the viewer tracks successful RAW/raster/strip uploads
even without a cache lease or redraw. Budget-refusal cleanup checks must await
GPU completion before assuming all retired texture credit is available.

## RLA interpretation

For RLA files whose producer declares straight alpha and sRGB encoding, use
`rrrah --rla-alpha straight --untagged-color srgb image.rla`.
Choose `--rla-alpha premultiplied` only for associated-alpha source pixels.
These independent settings apply to foreground loading, neighbour prefetch
and thumbnails. Omitting the alpha setting rejects ambiguous alpha-bearing
RLA, and omitting the color setting keeps untagged color unspecified.
