# 3D model loading

Priority: file selection to first presentation, then interaction latency and peak memory. The existing raster qualification matrix remains separate; model decoding alone does not qualify interactive viewing.

## STL

`rrrah_decode::decode_stl(&DecodeRequest)` loads binary and ASCII STL into `StlMesh`. Facets retain float32 vertices, source normals and raw binary attribute words. Bounds are computed while decoding; no welding, triangulation, normal recomputation or unit conversion is performed. Empty meshes have no bounds. Source winding and degenerate facets remain available to consumers.

Binary layout follows the [Library of Congress description](https://www.loc.gov/preservation/digital/formats/fdd/fdd000505.shtml); ASCII grammar follows its [ASCII STL description](https://www.loc.gov/preservation/digital/formats/fdd/fdd000506.shtml). Exact binary length takes priority over a `solid` header prefix. Color extensions are ambiguous and their raw attribute words are preserved, without color interpretation.

A binary load reserves the final facet array once and computes bounds in the parsing pass. ASCII parsing walks borrowed lines and tokens without a token array. Source input is capped by the existing 2 GiB bounded reader; retained facets are capped at 512 MiB. Both paths check cancellation during parsing and reject non-finite geometry. Whole-file input is still retained during parsing; streaming and measured latency/peak memory remain future work.

The STL API remains separate from the sensor/raster `decode_image` API. The application foreground loader, CLI `--inspect`, file drops and mixed-folder navigation now recognize STL (including uppercase extensions). Selected models use `ModelRenderer`, a separate depth-tested GPU pass with a neutral material. Drag rotates, the wheel changes scale and F resets the initial view. The pipeline is created lazily on the first model, so raster opening does not require model shader creation. Model thumbnails and color extensions are pending.

GPU upload validates geometry and fits its bounding sphere with float64 arithmetic. It writes normalized positions directly into the mapped wgpu vertex buffer, without an intermediate application vertex vector; backend staging is not claimed to be copy-free. Source coordinates and normals remain in the decoder. Geometry and depth attachments have device-limit and 512 MiB allocation caps. Depth is reused when viewport dimensions remain unchanged. View rotation updates uniforms without re-uploading geometry.

Validation: four STL decoder tests cover exact binary/ASCII geometry, binary headers beginning with `solid`, every truncated binary prefix, non-finite values, raw attributes, empty input geometry and cancellation. All 60 application tests pass, including STL discovery/inspection and invalid frame/window arguments. Naga validates the shader. Two readback tests pass on Metal Apple M4 Max: coverage, edge-on orbit, extreme finite coordinates, empty rendering and overlapping facets independent of submission order. These synthetic tests do not prove real-window input, navigation transitions or physical presentation; the attempted native-window inspection reported that the Mac was locked.

On 2026-10-05, the authored grid from `python3 scripts/generate-model-load-fixtures.py` contains 250,000 facets and 12,500,084 bytes. Five CLI inspection runs after one warmup report 4.80, 4.81, 4.77, 4.97 and 5.03 ms (median 4.81 ms) on Apple M4 Max, using the optimized development build. Timing begins inside CLI inspection and includes reading, decoding and bounds calculation; process startup, cold disk I/O, GPU upload and first presentation are excluded. The file cache is warm. This planar synthetic input is not a general CAD benchmark or a before/after speedup. Local results are in `target/bench/models/load-results.txt`.

GLB/glTF, CAD formats, scene materials, full variant qualification and qualification of 100 formats remain outstanding.

## Common model API and OBJ

`decode_model(&DecodeRequest)` returns `DecodedModel::Stl`, `DecodedModel::Obj`, `DecodedModel::Ply` or `DecodedModel::Off`. Discovery accepts STL/OBJ/PLY/OFF extensions case-insensitively; extension candidacy is not full variant qualification. The existing application model event and GPU renderer consume all four formats. `triangles()` yields float64 world coordinates as a cloneable, exact-size iterator over source/indexed storage and does not allocate another triangle array; bounds are float64. Native STL/OBJ fields retain their existing float32 types. Native PLY float64 coordinates remain float64 through CPU fitting and are converted to GPU float32 only in centered local coordinates. Sensor and raster APIs remain separate.

The native OBJ parser reads float32 `v`, `vt`, `vn` and face data; absolute indices address previously declared entries, and negative indices resolve against each array's size at that face. All four corner layouts are supported, with a consistent layout required within a face. Raw xyz coordinates, optional rational-curve weights, texture coordinates, normals, face corner associations, original faces, object/group names, smoothing groups and material/library references are retained. Polygon positions use xyz; the optional weight belongs to rational curves and does not divide polygon coordinates. Semantics follow the original [Wavefront Appendix B1, mirrored specification](https://raw.githubusercontent.com/mokiat/java-data-front/master/documents/OBJ%20Specification.txt).

Triangular faces use a direct indexed fast path. Larger contours project to their dominant plane, check self-intersection and triangulate by ear clipping, retaining winding and concave cutouts. Only exactly collinear removable corners are dropped from tessellation; numerically ambiguous corners reject instead of silently deleting small features. Original corners remain retained. A relative 1e-6 planarity tolerance is used after extent normalization. Strongly non-planar polygons reject. Input is capped at 2 GiB; geometry vector reservations share a 512 MiB budget, metadata string content has a 16 MiB budget, logical lines are capped at 64 KiB, faces at 1024 corners, and nontriangle faces consume a separate conservative cubic work budget of 64 million units. These are implementation bounds, not universal geometry guarantees or measured total process-memory limits.

Forward absolute references, points/lines/free-form surfaces, display statements and vertex-color extensions currently reject explicitly. External calls and shell statements are unsupported. MTL/texture loading, source-normal/smoothing shading, model thumbnails and broader OBJ qualification remain pending. The window currently displays geometry with the same neutral material as STL. Float64 textual source precision and comprehensive near-degenerate polygon handling remain unqualified.

`scripts/generate-obj-fixtures.py` uses pinned trimesh 4.11.2 outside the runtime workspace for five independent geometry oracles. Two concave contours have explicit authored area/cutout references, including a U shape that cannot safely use a simple fan. Seven models are checked through the public common model API for signed projected area, bounds and 37-by-41 interior coverage samples. Source UV/normal/index/state, continuations, weight semantics, malformed indices/coordinates, unsupported elements, work limits and cancellation are covered separately. All 15 source/oracle/manifest files regenerate byte-identically; SHA-256 fields and typed oracle sizes match. Full decoder tests pass 501 tests (3 ignored), application unit tests pass 61 tests, and the decoded U-shape GPU readback matches the authored rectangles exactly on Metal Apple M4 Max, with the notch remaining empty. No runtime dependency or lockfile change was introduced. This is selected-contract evidence, not complete OBJ or 100-format qualification.

The speed-fixture generator now also writes `grid.obj`: 250,000 triangles, 251,001 declared positions, one source normal and 10,570,830 source bytes. Five warm-cache CLI runs after one warmup report 67.42, 67.15, 66.23, 66.17 and 65.67 ms (median 66.23 ms) on the same Apple M4 Max optimized development build. This includes file reading, parsing, source metadata, indexed geometry and bounds, excluding startup/GPU/first presentation. It is an absolute synthetic-input measurement, not a format-independent comparison or a before/after optimization. Local results: `target/bench/models/obj-load-results.txt`.

## PLY 1.0 polygon meshes

`decode_ply` supports ASCII and binary little/big endian, the eight standard signed/unsigned 8/16/32-bit integer and IEEE float32/float64 types, plus their conventional width aliases. Headers accept LF, CRLF and CR line endings without consuming binary payload bytes. Element and property order comes from the header. Scalar columns retain native types; list columns retain the count type, native values and row offsets. Comments, object-info text and unknown numeric elements/properties are retained. The source schema follows the [Stanford PLY tools and usage](https://graphics.stanford.edu/software/vrip/plyusage.html) and [PLY description hosted by the University of Maryland](https://gamma-web.iacs.umd.edu/POWERPLANT/papers/ply.pdf).

Mesh geometry uses scalar vertex x/y/z and integer face `vertex_indices` or `vertex_index` lists. Vertex storage is not duplicated into an owned coordinate array. Face references are validated after all declared elements are read, including when face data precedes vertex data. Triangle faces use direct indexing; larger contours use the shared polygon triangulation with winding/cutout preservation. Extraction reuses bounded scratch storage. Positions must be finite; raw non-finite ancillary attributes remain in native columns. Source input is capped at 2 GiB, typed-vector reservations at 512 MiB, the header at 256 KiB, elements at 128 and properties at 4096. Individual lists are capped at 1,048,576 values; rendered faces at 1024 corners and the existing polygon work budget. These are resource bounds, not measured total process-memory limits.

The model GPU uploader accepts float32 or float64 source coordinates. It rebases against the lower bounds in scaled local coordinates before conversion, handling overflowing full extents by half-differences. This avoids loss from absolute float32 conversion, overflow near the float64 maximum and half-extent underflow near zero. Indexed/source storage is retained during loading, and the mapped GPU buffer is filled without an intermediate application position vector. Geometry view fitting does not change source units or samples.

Point clouds/range-grid rendering, vertex/face color and alpha interpretation, normals/smoothing, textures, real-window interaction and comprehensive near-degenerate polygon qualification remain pending. Point clouds currently return an explicit unsupported result. Numeric attributes are preserved but the window uses neutral geometry shading. No complete PLY-family or all-100 qualification is claimed.

Validation: 12 CC0 files cover all three encodings, float32/float64 vertices, coordinates translated by 10^12, concave U contours, reordered properties, all eight native scalar types, unknown variable lists/edge data and ancillary NaN payloads. `scripts/generate-ply-fixtures.py` pins plyfile 1.1.3 outside the runtime workspace for independent typed-column and triangle readback; concave surface references are authored. With the installed numpy version, plyfile's scalar binary writer lost endian in records containing lists; the generator therefore writes endian-aware binary fields explicitly and asserts readback against authored coordinates. All 49 source/native/schema/triangle/manifest files regenerate byte-identically; source hashes and triangle byte sizes match.

Four PLY tests cover native byte/schema equivalence, malformed/truncated inputs, element-order independence, index/position limits, explicit point-cloud boundaries and cancellation before I/O/during polygon work. The full decoder suite passes 505 tests (3 ignored), application unit tests pass 62, and five model GPU/readback tests pass on Metal Apple M4 Max. Readback confirms float64 translations, smallest-subnormal/maximum/10^300 scales, all PLY encodings, unchanged STL/OBJ rendering and exact U-contour pixels against authored rectangles. Native-window inspection still reports a locked Mac, so physical presentation and live input remain unverified. Cargo.lock and the native semantic digest remain unchanged.

CPU load measurement on 2026-10-05: `scripts/generate-model-load-fixtures.py` authors 250,000-triangle grids with 251,001 vertices. Binary LE float32 source size is 6,262,217 bytes; float64 source size is 9,274,232 bytes and includes a 10^12 translation. Five warm-cache CLI runs after one warmup report float32 times 23.79, 24.28, 23.67, 23.89, 23.96 ms (median 23.89 ms), and float64 times 24.97, 24.75, 25.11, 24.55, 24.68 ms (median 24.75 ms), on Apple M4 Max with the optimized development build. These include read/typed decode/mesh extraction/bounds, excluding process startup, cold disk access, GPU upload and first presentation. No before/after speedup or general scan/CAD throughput is implied. Local results: `target/bench/models/ply-load-results.txt`.

## OFF polygon meshes

The shared `decode_model` OFF path now reserves the managed geometry budget
before growing positions, normals, vertex colors, texture coordinates, faces,
indices and triangles. Actual reported vector capacity is charged. The same
reservation transfers into `ModelBuffer`; source and output overlap during
decoding, and output credit follows the last shared owner without re-admission.
ASCII/binary triangle-attribute fixtures pass at source plus exact retained-output
capacity; one byte less rejects and releases all reservations. Concave fixtures
also reserve polygon scratch.
Per-face byte/float color arrays are reserved before allocation for ASCII and
binary sources. ASCII color tokens use a fixed stack array. OFF polygon coordinate
arrays and triangulator normalization/projection/index/output arrays now reserve
before allocation and account for reported capacities. Scratch credit is retained
conservatively until the returned triangles are consumed. A quad test checks exact
scratch admission, one-byte-short rejection and cancellation after allocation.
Remaining OBJ text scratch is not fully managed yet. PLY header
metadata now reserves before string and vector allocation. The owned `decode_off` API cannot retain a
reservation in its returned plain mesh; use `decode_model` for managed lifetime.

`decode_off` reads ASCII and big-endian binary OFF, with optional ST/C/N attributes in their declared order. Normals, RGBA vertex colors, texture coordinates, face colors, original face indices and the declared edge count are retained. ASCII positions remain float64; binary float32 values convert exactly to float64. Headerless ASCII and nOFF with dimension 3 are accepted. Face-color line boundaries remain significant. Syntax follows the original [Geomview OOGL manual](https://manpages.debian.org/testing/geomview/oogl.5gv.en.html).

The loader reserves declared vertex/attribute/face storage, computes bounds during parsing and directly indexes triangular faces. Larger faces reuse scratch coordinates and the existing concave triangulator. Common-model iteration and mapped GPU upload avoid a second owned triangle-coordinate array. Input is bounded at 2 GiB, geometry reservations at 512 MiB, individual faces at 1024 corners and polygon work at 64 million units. Scalar tokens are bounded at 128 bytes; binary header tails at 64 KiB and ASCII face-color tails at 1024 bytes. These limits do not measure total process RSS. Malformed counts, indices, colors, non-finite scalars, truncated payloads and trailing data reject; cancellation is checked for every vertex/face and during polygon work.

Homogeneous/non-3D coordinates, point/line faces, point-cloud rendering, OOGL object wrappers, appearance/colormap loading, displayed colors/normals/textures and complete OFF-family qualification remain pending. Source attributes are retained while the current window uses neutral geometry shading.

Seven CC0 sources from `scripts/generate-off-fixtures.py` cover clockwise and rotated-plane contours, float64 translation, a U contour and ASCII/binary attributes. Four geometry oracles use independent trimesh 4.11.2 readback; binary/attribute and concave references are authored. All 15 source/oracle/manifest files reproduce byte-for-byte with verified hashes and typed oracle lengths. Four decoder tests compare signed surface normals and 37-by-41 coverage, attribute values, every truncated binary prefix, malformed cases, limits and cancellation. Current suites pass 509 decoder tests (3 ignored), 63 application unit tests and three application model GPU readbacks on Metal Apple M4 Max. The OFF readback matches source/oracle pixels, including the concave cutout and translated geometry. This does not establish physical window presentation or all-100 qualification.

Warm-cache CPU loading on 2026-10-05, Apple M4 Max, optimized development build: 250,000 triangles and 251,001 vertices. ASCII input is 7,818,772 bytes; binary is 8,012,055 bytes. Five CLI runs after one warmup measure ASCII 25.36, 26.01, 26.32, 25.76, 26.13 ms (median 26.01 ms), binary 9.29, 9.31, 9.00, 8.98, 9.08 ms (median 9.08 ms). These include read/decode/attributes/index extraction/bounds; startup, cold I/O, GPU upload and first presentation are excluded. These are absolute times for authored grids, not a before/after speedup or a general-model benchmark. Local results: `target/bench/models/off-load-results.txt`.


The common `DecodedModel` owns each native STL/OBJ/PLY/OFF mesh through `Arc`.
Cloning a model now shares its complete source geometry instead of deeply copying
its vectors. Triangle iteration and bounds retain the native geometry contract;
individual decoder mesh APIs remain owned. A regression verifies shared OFF
geometry allocation and survival after the original model handle is dropped.
`DecodedModel::capacity_bytes` now provides a conservative cache-admission weight covering allocated vector capacity, nested PLY properties/lists, OFF face colors and OBJ strings. Shared OBJ strings are charged per face reference; allocator/Arc bookkeeping is excluded. Tests verify reserved geometry counts even when no triangles exist. The foreground viewer now has a 512 MiB model RAM cache using this weight and source fingerprint/image-index keys. Hits retain the shared original mesh and bypass decoder admission; the selected model is pinned and foreground replacement restores the previous pin if admission fails. `--no-cache` refuses model admission. Independent model policy is configured with `--model-cache-mb`, `--model-cache-count` and `--model-cache-ttl-secs`; defaults are 512 MiB and unlimited count/TTL. Zero count rejects admission; zero TTL misses immediately. Hits do not renew TTL. Neighbour model preloading now uses the foreground loading worker during idle queue periods, the configured previous/next window and navigation direction. New foreground generations cancel in-flight decoding and replace pending neighbour paths. Background admission preserves the visible model pin and obeys model cache limits. A real OFF preload regression confirms a subsequent shared-geometry hit without decoder admission; physical navigation latency remains unmeasured. Disk/swap persistence and a total allocation budget remain pending.

## Managed OBJ geometry admission

`decode_model` now reserves before growing OBJ positions, texture coordinates,
normals, corners, faces and triangles. Reported vector capacity is charged and
the reservation transfers to `ModelBuffer`, remaining live through the final
shared owner. Polygon coordinates and triangulator arrays use the same ancestor
budget. Shared face metadata is conservatively charged per reference to match
`capacity_bytes`; library string capacity is also included in retained admission.
Metadata token joining now measures the normalized name first, reserves before
String allocation and writes directly without a temporary token-pointer vector.
A 10,000-token group line rejects early under insufficient memory. Object and
material Arc payloads now reserve before construction. Their active-state credits
merge into the geometry reservation on the first retained face; later references
retain the existing conservative per-face weight. Unused/replaced states release
credit, and cancellation after metadata construction cleans up all reservations.
Group strings and Arc slot payloads now reserve before allocation; a separate
transient credit covers Vec/Arc conversion overlap. Their first-face credits also
merge into the model, with exact retained capacity after parsing. Library-path
strings reserve before copying as well. Arc/allocator bookkeeping and stack data
remain outside managed payload accounting. Continued-line scratch
now reserves before every capacity increase, accounts for actual String capacity
and releases credit after parsing. Failed growth preserves the previous content
and allocation; the logical-line limit also includes inserted continuation spaces.
The plain owned `decode_obj` return cannot retain managed ownership credit.

Managed triangle, negative-index quad and concave fixtures preserve exact geometry,
bounds and retained capacity. Root counters match retained output after decode
and return to zero after the last owner. A source-plus-mesh-header budget rejects
the first geometry growth and releases all reservations. Existing independent
OBJ geometry fixtures continue to pass.

## Managed PLY body admission

`decode_model` reserves before growing native scalar columns, flattened list
values, list offsets and output triangles. The same reservation transfers into
`ModelBuffer` and follows the last shared owner. Fixed stack index scratch replaces
the old 1024-entry heap buffers; nontriangle coordinate and triangulator arrays
use managed temporary reservations. Face-property selection no longer allocates
a candidate array. ASCII, little-endian float64, big-endian offset coordinates
and concave fixtures retain exact columns/list offsets, geometry and metadata.
Memory rejection releases source and geometry reservations. Header comments,
object info, names, element/property arrays and initial list offsets now reserve
before allocation; actual capacities are charged from the start of header parsing.
Header parsing uses borrowed line/token slices. Regressions reject oversized
comment/element allocations before body validation and clean up cancellation
after partially retained metadata.

## Managed model CPU and Metal follow-up (2026-10-05)
After pre-allocation admission changes, forced Metal model readback passed five
GPU resource/geometry tests and four application decode/render tests on Apple M4 Max.
The added application test compares managed STL/OBJ/PLY/OFF rendering against the
same legacy-decoded geometry and checks last-owner CPU reservation release.
Existing application readbacks retain independent/authored geometry comparisons.
This is offscreen readback, not physical window latency or all-device qualification.
Repeated the existing CLI inspection on authored 250,000-triangle grids with
`--inspect --no-cache --managed-memory-mb 128`. One warmup precedes five measured
runs per fixture. OS caches are warm; time includes source read, decoding and
bounds, excluding startup, GPU upload and presentation.
| Fixture | Median ms | Retained bytes | Managed peak bytes |
| --- | ---: | ---: | ---: |
| grid.stl | 5.38 | 13000056 | 25500140 |
| grid.obj | 73.05 | 57235080 | 67805910 |
| grid32.ply | 27.05 | 11568364 | 17830581 |
| grid64.ply | 28.25 | 14714092 | 23988324 |
| grid-ascii.off | 25.79 | 25364288 | 33183060 |
| grid-binary.off | 9.64 | 25364288 | 33376343 |

Peaks count managed reservations, not RSS. Retained bytes are printed while the
model is still alive; ownership-release assertions are covered by decoder and
GPU integration tests. Earlier absolute times used different admission paths;
these separate runs do not prove the cause of a performance difference.
Evidence: `target/bench/models-managed/load.csv` and `load.log` (30 measured rows);
GPU logs `/tmp/rrrah-model-managed-metal.log` and `/tmp/rrrah-managed-model-app-metal.log`.

Tested executable SHA-256: `bbf93d430614459345d6a381965f82c3cc6dc5343b197a2c22b8185de050694c`. Fixture hashes are recorded in
`target/bench/models-managed/sha256.txt`.

### STL streaming swap codec

`stl_swap_payload_len`, `write_stl_swap_payload`, and `read_stl_swap_payload`
provide the versioned `RRSTL001` payload for a future model swap tier. Each
facet stores its twelve original float32 values and raw attribute word in
little-endian order; no vertex welding or full geometry copy is required.
Restoration validates framing, counts, finite scalars and a 512 MiB storage
limit, reserves final vector capacity before allocation, recomputes bounds,
and transfers the reservation into the shared `ModelBuffer`. Memory admission
errors remain typed separately from malformed payload and I/O errors. The
swap store must supply integrity, cancellation and the exact payload length.
The application model cache is not yet connected to this codec; OBJ, PLY and
OFF swap payloads remain pending.

Tests cover bit-exact floats including signed zero, attributes, bounds,
empty meshes, shared ownership and final release, every truncated prefix,
malformed framing/scalars, forged counts rejected before admission, and a
one-byte-short restore budget.

The `rrrah-app` integration target `stl_swap` now exercises this codec through
`ImageSwapCache` and the actual asynchronous `SwapStore`: geometry/bounds
roundtrip, release of source and restored allocations, typed restore pressure
with successful retry from the retained disk entry, cancellation before
restoration, disk byte/count limits, zero TTL, and eviction at one object.
All three integration tests pass. This qualifies the codec/store boundary;
viewer cache routing and model swap CLI controls remain pending.

### Viewer STL swap routing

The foreground model cache now spills STL entries evicted by normal RAM
admission into its asynchronous swap tier, and checks swap after a RAM miss
before invoking the decoder. Restored geometry shares a reservation under the
configured managed root. Foreground restore pressure may release unpinned RAM
models and retry; background loads preserve pins. Emergency RAM release cancels
pending spills rather than retaining the evicted geometry in another queue.
OBJ/PLY/OFF continue through their existing RAM/decode path until their native
payload codecs are implemented.

`--model-swap-mb` enables the tier (default 0). Independent limits are
`--model-swap-count`, `--model-swap-ttl-secs`, `--model-swap-queue-mb` (default
128), `--model-swap-queue-count` (default 4, maximum 1024), and
`--model-swap-restore-mb` (default 512). `--no-cache` disables this tier.
Publication TTL is fixed; zero restore capacity refuses restoration. Disk
storage is temporary and session-owned rather than persistent model caching.
Viewer routing and CLI tests pass alongside the store integration suite;
physical navigation latency has not been measured.

Model swap restore retries now release unpinned RAM only for retryable capacity
pressure (`requested <= limit`), not for a payload that cannot fit the local
restore limit at all. A viewer regression with zero restore capacity verifies
that unrelated unpinned models survive a failed restore/decode attempt. The
STL viewer roundtrip additionally holds the root budget full during background
loading, then releases pressure and restores from the same disk entry; the
visible pin is unchanged and the disk entry records no corruption error.
Background restore under a one-entry RAM limit returns managed geometry without
evicting the pinned visible entry. The application suite passes 104 tests
(3 ignored) after this change.

### OFF streaming swap codec

`off_swap_payload_len`, `write_off_swap_payload`, and `read_off_swap_payload`
implement the versioned `RROFF001` payload. It preserves source encoding,
declared edge count, original float64 vertex attributes, indexed face ranges,
all native face color kinds, and the existing triangle array. Bounds are
recomputed without retriangulation. Vectors and nested color storage are
admitted before allocation; the final shared `ModelBuffer` owns the resulting
reservation. Restore validates payload framing, remaining encoded length,
finite scalars, colors, indices, contiguous face ranges and triangle count.
Managed storage is capped at 512 MiB; checksums and cancellation belong to the
swap store. The viewer adapter has not yet been extended to OFF.

Native attributed binary and concave ASCII fixture roundtrips reproduce the
payload byte-for-byte. Tests cover all face color representations, every
truncated prefix, malformed indices/scalars, forged counts, trailing bytes,
one-byte-short budgets, shared-owner lifetime and error cleanup. The complete
decoder suite passed 546 tests (4 ignored) before the additional malformed
scalar/index cases, which are checked separately in the focused OFF suite.

### OFF viewer swap integration

The viewer model swap adapter now dispatches `RRSTL001` and `RROFF001` by
versioned magic. It replays the eight-byte header from stack storage into the
native streaming reader and retains typed memory admission errors. Normal RAM
admission evictions now spill both STL and OFF; swap hits restore the original
`DecodedModel` variant under the shared managed root. The existing independent
`--model-swap-*` limits apply to both formats. OBJ and PLY remain pending.

The viewer roundtrip regression runs STL, attributed binary OFF, and concave
ASCII OFF through foreground eviction, background pressure/retry, and foreground
restoration with the decoder deliberately unavailable on hits. Restored payloads
match original encoded data byte-for-byte; background restores preserve the
visible pin, and all managed root credit is released after owners/cache drop.
The separate store suite now uses this same production adapter for STL rather
than a duplicate test-only adapter.

OFF payload length calculation now uses checked arithmetic on vector lengths
and face color descriptors rather than running the entire serialization into
a counting sink. Its cost is proportional to face descriptors and does not
scan vertex attributes, indices, triangle indices or color scalar values.
Scalar writes use fixed 4 KiB stack blocks instead of separate writes per
number. A boundary regression compares all bytes including signed zero for
4,500 float64 scalars and observes ten sink writes (count field plus nine
blocks), compared with 4,501 calls in the former scalar loop. Empty blocks
write nothing. Native fixtures and every face color kind still match declared
payload lengths. These are operation-count and correctness checks, not a
measured viewer-latency improvement.

### OBJ swap writer (restore pending)

`write_obj_swap_payload` and `obj_swap_payload_len` define the `RROBJ001`
stream. Geometry retains float32 bits and complete indexed corners, including
optional texture/normal references; triangles retain corner and face indices.
Face ranges, object/group/material names, smoothing groups, and MTL library
paths are encoded. Adjacent faces that share the same Arc allocation use
previous-face metadata references; distinct allocations remain explicit even
when their text is equal. Geometry uses bounded stack block writes. Length
calculation inspects lengths and metadata references rather than scalar values.
The managed restore codec and viewer admission for OBJ remain pending, so OBJ
is not yet eligible for model swap writes.

Tests independently pin the empty stream layout and verify exact writer/length
agreement plus metadata-reference savings on two faces sharing object, groups
and material. All 549 decoder tests passed (4 ignored). These tests qualify the
writer framing and sharing rule; they do not establish a complete OBJ swap
roundtrip.

### Managed OBJ swap restore

`read_obj_swap_payload` now reads `RROBJ001` into a managed native `ObjMesh`.
It validates float finiteness, optional index tags, bounds of vertex/texture/
normal/corner references, contiguous face ranges, triangle counts and face
ownership, UTF-8 metadata, previous-face reference availability and exact
framing. Restored adjacent object/group/material references share Arc storage.
Arrays, strings and conservative per-face shared metadata weights are admitted
before allocation; temporary string-to-Arc and group-array conversion overlap
is also budgeted. The resulting model retains credit until its last owner drops.
The viewer swap adapter has not yet been extended to OBJ.

Tests compare encoded roundtrips and bounds/corner/triangle data for six native
OBJ fixtures. Shared metadata tests verify Arc identity and retained capacity,
last-owner release, every truncated prefix, invalid position indices, first-face
reuse rejection and a one-byte-short budget with complete error cleanup.

### OBJ viewer swap integration

OBJ is now accepted by the production model swap adapter. `RROBJ001` dispatch
preserves typed memory failures and returns the original `DecodedModel::Obj`;
normal RAM admission eviction and subsequent foreground/background restore use
the existing independent model swap policy. PLY remains pending.

The viewer roundtrip regression now includes negative-index attributed OBJ,
concave OBJ, and an authored CC0 `obj-shared-metadata.obj` fixture that changes
object/material/smoothing while retaining groups. It verifies exact payload
roundtrip, decoder-free swap hits, background restore after root pressure,
visible-pin preservation, and final root release. The test RAM budget is 64 KiB
with one entry, and first admission is asserted explicitly; the former 4 KiB
fixture budget was too small for OBJ's retained vector capacities and could not
exercise eviction. The application suite passes 104 tests (3 ignored), focused
OBJ codec tests pass three tests across seven fixtures, and the production
adapter's STL store compatibility suite passes three tests.

### Typed PLY swap writer (restore pending)

`write_ply_swap_payload` and `ply_swap_payload_len` define `RRPLY001` with
source encoding, vertex/position schema selectors, comments/object info,
ordered elements and properties, native typed scalar columns, list count types
and offsets, and the existing triangle array. All eight scalar types write
their original little-endian bytes without passing through float64 or float32
conversion. Output uses bounded stack blocks. Checked length calculation uses
schema/container sizes without scanning numeric columns or offsets. Managed
restoration and viewer swap eligibility for PLY remain pending.

Independent scalar-layout tests cover signed/unsigned integer boundaries and
float32/float64 signed zero. Twelve native PLY fixtures cover ASCII, LE, BE,
float32, float64, large translations and concave contours with exact declared
payload lengths. The full decoder suite passes 553 tests (4 ignored).

### Managed typed PLY swap restore

`read_ply_swap_payload` restores `RRPLY001` directly into budgeted native typed
columns, schema strings, list offsets/values and the existing triangle array.
It validates schema selectors, unique element/property names, scalar row counts,
integer list count types, ordered offsets and matching final value lengths,
count-type ranges, source limits (128 elements, 4096 properties, 1M list items),
face indices, triangle counts/indices and exact framing. Bounds are recomputed
from finite coordinate columns. Non-coordinate float attributes retain their
original NaN/Infinity bits, matching native PLY's attribute-preservation policy;
they are not mistaken for render coordinates. Final managed ownership retains
all capacity credit until the last clone drops. Viewer PLY swap routing remains
pending.

Twelve native fixtures now roundtrip byte-for-byte across all encodings and
precisions, including large float64 translations, concave faces, unknown typed
properties and unused NaN attributes. Tests check exact retained capacity,
shared-owner lifetime, one-byte-short admission, every truncated prefix,
invalid schema selectors/list offsets/triangle indices and trailing data, with
complete root cleanup after errors.

### PLY viewer integration and model swap Metal readback

PLY now dispatches through the production model swap adapter using `RRPLY001`.
All four current native model variants (STL/OBJ/PLY/OFF) participate in normal
RAM-admission spill and foreground/background restoration, under the same
independent `--model-swap-*` limits. Viewer regression cases now include ASCII
float32 PLY, float64 LE, translated float64 BE, and concave ASCII PLY alongside
STL/OBJ/OFF; exact payload roundtrips, pressure retry, visible pins and root
cleanup pass. The application suite passes 104 tests (3 ignored), and the
production adapter store compatibility suite passes three tests.

`real_model_swap_preserves_gpu_pixels_and_releases_restored_geometry` exercises
actual asynchronous disk writes/reads for six fixtures across all four formats.
It compares every 64x64 readback pixel against direct managed decoding and checks
source/restored credit release, six successful writes/reads and zero store
errors. All five `model_readback` tests pass with Metal selected. The new swap
readback also passes separately with `RRRAH_GPU_OPTIONAL=0` and reports
`Metal Apple M4 Max`. This qualifies offscreen GPU output after swap; it does
not measure physical viewer navigation, display HDR, CUDA or NVIDIA hardware.

### Measured large-model swap baseline and buffered reads

`cargo run --locked -p rrrah --example model_swap_timing -- MODEL 8`
compares warm-OS managed CPU decoding with actual session-swap restoration.
It uses one warmup and eight measured rounds, alternates tier order, verifies
the complete canonical payload outside each interval with bounded comparison
scratch, and asserts retained capacity plus last-owner root release. CPU root,
queue, restore and disk limits are 256 MiB. Quantiles use integer nearest rank.
This excludes GPU upload, startup and physical presentation.

A first run exposed direct file reads for each small codec request. `ReadStream`
now uses a fixed 64 KiB stack buffer, while checksum and remaining-byte counters
advance only for bytes delivered to the decoder. Reading ahead cannot conceal
a partially consuming codec; the four-byte/one-byte-consumer regression pins
that invariant. Cancellation is checked on each read.

Observed Apple M4 Max optimized-dev timings, milliseconds:

| Grid | Decode p50 after | Swap p50 before | Swap p50 after | Swap p95 after |
| --- | ---: | ---: | ---: | ---: |
| grid.stl | 4.19 | 101.55 | 24.33 | 24.48 |
| grid.obj | 73.11 | 2411.21 | 148.40 | 259.11 |
| grid32.ply | 36.63 | 790.16 | 56.03 | 80.95 |
| grid64.ply | 48.69 | 802.22 | 93.40 | 104.52 |
| grid-ascii.off | 24.08 | 968.88 | 52.84 | 53.55 |
| grid-binary.off | 8.23 | 970.75 | 53.12 | 54.44 |

Both runs verified all payloads, recorded nine successful restores per model,
zero swap errors and zero root usage after every final owner. Six models each
produce sixteen measured CSV rows. Raw logs/CSV and executable/source SHA-256
are under `target/bench/model-swap-native` and `target/bench/model-swap-buffered`.
The runs are sequential local observations, not a controlled paired load test;
other workspace activity and CPU contention can affect the numbers. Buffered
restore remains slower than warm source decoding for every case here. Spill
writing and per-scalar decode/checksum overhead still require optimization;
there is no claim that this swap tier currently improves navigation latency.

After the reader change, seven swap tests and 100 cache unit/integration tests
pass, including checksum, cancellation and short-consumer validation.

### Bounded streaming write buffer

`WriteStream` now assembles encoder calls in a fixed 64 KiB stack block, writes
complete blocks and flushes the final tail before publication. Remaining length
tracks accepted bytes; checksum updates happen on successfully flushed blocks.
Encoder errors/cancellation still discard the temporary object and quota credit.
A patterned 150,000-byte roundtrip covers block boundaries; explicit flush plus
repeated empty flush preserves all bytes and the checksum. Seven swap and 100
cache tests pass; five model GPU readback tests pass with forced Metal and
`RRRAH_GPU_OPTIONAL=0`. No full serialized model buffer is introduced.

Repeated local benchmark, milliseconds:

| Grid | Previous single spill | Buffered single spill | Restore p50 |
| --- | ---: | ---: | ---: |
| grid.stl | 341.58 | 25.18 | 24.32 |
| grid.obj | 2615.99 | 50.11 | 152.65 |
| grid32.ply | 27.73 | 14.54 | 47.05 |
| grid64.ply | 33.38 | 15.59 | 49.47 |
| grid-ascii.off | 1114.23 | 22.48 | 53.96 |
| grid-binary.off | 980.16 | 22.22 | 54.04 |

Spill values are individual operations, not medians or a controlled paired
performance claim. Each model also has one warmup and eight alternating
decode/restore rounds, full payload verification outside timing, nine reads,
zero errors and zero final root usage. Raw evidence and executable SHA-256 are
in `target/bench/model-swap-buffered-write`. Restore remains slower than warm
decoding. Per-small-read checksum updates and native codec parsing remain
unoptimized; no physical viewer-latency benefit is claimed.

### Read checksum per block

ReadStream now updates BLAKE3 once when filling its bounded 64 KiB buffer,
rather than for each small codec request. The consumed-byte counter remains
independent: it decreases only when delivering data to the decoder. A fetched
whole payload cannot make a partially consuming codec pass length validation;
checksum publication still follows successful complete consumption. Existing
small/large partial-consumer, corruption, truncation, trailing-byte, cancellation
and roundtrip tests pass (seven swap plus 100 cache tests). Five forced Metal
model readback tests pass with hardware qualification required.

Observed milliseconds from one warmup and eight alternating measured rounds:

| Grid | Previous restore p50 | Block checksum p50 | Block checksum p95 | Decode p50 |
| --- | ---: | ---: | ---: | ---: |
| grid.stl | 24.32 | 18.05 | 18.19 | 3.72 |
| grid.obj | 152.65 | 94.29 | 97.04 | 73.00 |
| grid32.ply | 47.05 | 30.35 | 31.07 | 24.93 |
| grid64.ply | 49.47 | 30.38 | 31.67 | 25.67 |
| grid-ascii.off | 53.96 | 31.08 | 31.68 | 25.47 |
| grid-binary.off | 54.04 | 31.68 | 32.30 | 8.32 |

Every round verifies the full canonical payload after timing and zero root usage
after last-owner release. Each model records nine successful reads and zero
errors. Raw CSV/logs and executable hash are under
`target/bench/model-swap-block-checksum`. These sequential local observations
are not a controlled paired load experiment. Restore still loses to warm
source decoding on all six grids; native per-value/per-record restoration
remains a candidate for optimization, and no physical viewer benefit is claimed.

### PLY scalar block restoration

Typed PLY columns now read bounded 16 KiB blocks into stack scratch and unpack
native little-endian values directly into already admitted final vectors. This
removes a reader/cancellation/buffer call per scalar while preserving all eight
native types and arbitrary non-coordinate float bits. One 5,003-element float64
regression observes five underlying reads (type, count and three blocks) and
pins signed zero, a specific NaN payload, Infinity and smallest-normal values.
Existing complete fixture/prefix/error tests still pass; the full decoder suite
passes 556 tests (4 ignored). Five forced Metal readback tests pass with GPU
qualification required.

Optimized-dev Apple M4 Max, one warmup plus eight alternating measured rounds:

| Grid | Prior restore p50 ms | Block restore p50 ms | Block restore p95 ms | Decode p50 ms |
| --- | ---: | ---: | ---: | ---: |
| grid32.ply | 30.35 | 19.13 | 19.48 | 23.86 |
| grid64.ply | 30.38 | 19.94 | 20.83 | 24.87 |

Both cases verify the full serialized payload outside timing, nine successful
restores, zero store errors and zero root usage after every last-owner drop.
Raw CSV/logs and binary SHA-256 are in `target/bench/model-swap-ply-blocks`.
This run observes faster swap restoration than warm source decoding for these
two grids only. It is not a universal PLY or physical navigation speed claim;
other model formats and larger/native variants remain separately qualified.

### OFF numeric record block restoration

OFF coordinates, vertex attributes, indices and triangle arrays now restore
through record-aligned 16 KiB stack blocks directly into admitted final vectors.
Finite-coordinate and index checks remain in each decoded record. A regression
covers 1,367 float64 triples: their 24-byte record width does not divide the
scratch size, yet three payload blocks plus the count read reproduce every bit
including signed zero. Prefix/malformed/color/credit tests pass with the complete
decoder suite (557 tests, 4 ignored), and five forced Metal readback tests pass.

Apple M4 Max optimized-dev, one warmup plus eight alternating measured rounds:

| Grid | Previous restore p50 ms | Block restore p50 ms | Block restore p95 ms | Decode p50 ms |
| --- | ---: | ---: | ---: | ---: |
| grid-ascii.off | 31.08 | 22.47 | 22.87 | 24.07 |
| grid-binary.off | 31.68 | 22.68 | 23.09 | 8.39 |

Both verify full canonical payloads after timing, nine successful reads, zero
swap errors and zero root usage after last-owner drops. CSV/logs and executable
SHA-256 are in `target/bench/model-swap-off-blocks`. These local observations
show a modest restore advantage over ASCII decoding on this grid, while native
binary OFF decoding remains substantially faster. They do not establish
physical navigation latency or general superiority across OFF variants.

### STL facet block restoration

STL restoration now reads complete 50-byte facet records in bounded 16 KiB
stack blocks and unpacks directly into admitted final storage. A 655-facet
regression verifies all float bits and distinct attribute words using four
underlying reads (header plus three blocks), including the unaligned final
record and error cleanup on truncation at block boundaries. The complete
decoder suite passes 558 tests (4 ignored), and five forced Metal readback tests
pass with hardware qualification required.

A warm-OS optimized-dev run on Apple M4 Max (one warmup, eight measured rounds)
observes STL restore p50/p95 17.26/17.46 ms, compared with the previous p50
18.05 ms; direct source decoding is 4.29/4.36 ms. The small timing difference
is not evidence of a substantial latency improvement in this sequential local
comparison. Record/block correctness and reduced reader-call count are proven;
scalar unpacking, validation and bounds work still need profiling. Full payload
verification, nine successful reads, zero errors and zero final root usage pass.
Raw data and executable hash are in `target/bench/model-swap-stl-blocks`.

### STL shared validation and diagnostic codec tier

STL swap now uses the same facet finite-value/capacity checks and bounds update
as the native STL decoder, after unpacking a complete facet. This removes a
duplicate implementation without relaxing validation. All twelve normal/vertex
fields are independently tested with NaN, positive Infinity and negative
Infinity; each failure releases the complete managed allocation. The full
558-test decoder suite passes (4 ignored), four focused STL-swap tests pass,
and five forced Metal readback tests pass.

The immediate two-tier run observes restore p50/p95 16.43/16.51 ms and decode
4.20/4.64 ms; the change does not establish a substantial performance gain.
Evidence is in `target/bench/model-swap-stl-shared`.

`model_swap_timing` now includes a diagnostic `codec_file` tier: it reads the
same canonical payload from a warm file through the native codec, excluding
the swap-store checksum/cancellation wrapper. Production restoration retains
all integrity checks. All three tiers verify their complete result after the
timed interval and release the root before the next tier. Warmup plus eight
measured rounds rotate the first tier, producing 24 measured rows per model.
On the STL grid, p50/p95 milliseconds are decode 3.72/3.75, production swap
16.60/17.02, and diagnostic codec 10.92/11.02. Nine production reads, zero errors
and zero final root usage pass. Raw CSV/log and executable hash are in
`target/bench/model-swap-stl-diagnostic`. This separates observable costs but
is not a causal CPU profile; both native codec execution and store overhead
remain candidates for profiling. It does not justify dropping checksums.

### STL inline experiment and sampled profile

An ordinary `#[inline]` on the shared facet append routine did not establish a
meaningful improvement: sixteen measured rounds observed decode p50 3.51 ms,
swap 16.51 ms and diagnostic file-codec 10.77 ms. The annotation was removed;
there is no retained speculative inline optimization. Raw evidence is in
`target/bench/model-swap-stl-inline`.

A three-second macOS `sample` capture of the complete benchmark initially
mostly observed its untimed full-payload comparison reading tiny records.
The benchmark's reference writer and comparison reader now use bounded 64 KiB
buffers with explicit flush; every byte is still verified outside timing.
This changes the harness, not production cache policy or checksum behavior.
Sixteen measured rounds with buffered verification observe decode p50/p95
3.53/4.09 ms, swap 15.93/16.09 ms and file-codec 10.21/10.52 ms; all 17 production
reads, integrity/byte comparisons and root-release checks pass. These sequential
observations do not establish an application speedup.

A second three-second sampled capture with buffered verification identifies
actual restore stacks in `read_stl_swap_payload`, shared finite/bounds append,
and BLAKE3's NEON subtree hashing. Untimed verification remains present, so
whole-process sample percentages must not be reported as restore percentages.
CSV, logs, executable hash and sampled call tree are in
`target/bench/model-swap-stl-buffered-verify`; the previous mixed capture is in
`target/bench/model-swap-stl-profile`. Isolated unpack/validation profiling is
still needed before selecting a further native optimization.

### STL memory-codec diagnostic and fixed finite validation

The timing example now rotates four tiers: source decode, production swap,
file codec, and memory codec. The last tier restores the same canonical payload
from a separately budgeted immutable buffer. Its 12,500,016-byte allocation is
diagnostic only; production swap remains streaming. Every restored payload is
compared byte-for-byte outside timing, and both allocation budgets must return
to zero after the last owner drops.

The shared STL append routine now checks all twelve normal/coordinate values
using fixed loops instead of a chained iterator. NaN and either infinity remain
invalid; capacity and bounds checks are unchanged. The decoder suite passes
558 tests with four ignored, and forced Metal model readback passes five tests,
including real swap restoration and CPU credit release.

Sixteen measured rounds observe p50/p95 milliseconds: source decode 3.21/3.42,
swap 15.57/16.12, file codec 9.89/11.49, memory codec 9.42/9.76. A second run
observes 3.17/3.27, 15.49/15.99, 9.88/10.59 and 9.29/9.60 respectively.
Each run completes 17 production reads with zero errors and zero final budget
usage. Evidence is in `target/bench/model-swap-stl-fixed-validation` and
`target/bench/model-swap-stl-fixed-repeat`; the preceding memory diagnostic is
in `target/bench/model-swap-stl-memory-codec`. Sequential local observations do
not establish a controlled causal speedup or physical viewer latency. For this
STL fixture, source decoding still costs less than production swap restoration.

### Four-tier observations across OBJ, PLY and OFF

The same executable completed sixteen measured rounds per tier for five more
grid fixtures. Median milliseconds on Apple M4 Max:

| Source | Decode | Production swap | Direct file codec | Memory codec |
| --- | ---: | ---: | ---: | ---: |
| OBJ | 70.66 | 92.82 | 2290.39 | 45.71 |
| PLY float32 | 24.50 | 19.59 | 305.80 | 12.21 |
| PLY float64 | 25.13 | 20.76 | 305.95 | 11.88 |
| OFF ASCII | 24.08 | 21.79 | 236.37 | 11.73 |
| OFF binary | 8.24 | 21.76 | 236.19 | 11.77 |

The direct file tier deliberately bypasses the store's 64 KiB read buffer;
its small codec reads can cause many system calls. It is not an estimate of
production file overhead, and subtracting it from other tiers is invalid.
Memory restoration excludes source I/O and store integrity processing. All
tiers retain managed allocation and full byte verification outside timing.
Each fixture records 17 successful production reads, zero errors, and zero
final allocation usage in both budgets. Raw CSV, logs, source and executable
SHA-256 hashes and machine metadata are in `target/bench/model-swap-four-tier`.

These warm-OS CPU observations favor swap for the two PLY fixtures, slightly
for ASCII OFF, and source decode for OBJ and binary OFF. They do not justify
a universal format policy: content size, encoding and machine matter, and GPU
upload/presentation are excluded. A future adaptive policy should measure
per-source costs and avoid sacrificing correctness or visible-frame priority.

### Buffered file-codec control

The example additionally rotates `codec_buffered`: the same codec reads the
canonical file through a 64 KiB `BufReader`, including file opening and buffer
allocation in timing. Unlike production swap, this diagnostic does not supply
integrity or cancellation and must never replace the production read path.
Eight measured rounds plus warmup produce forty measured rows per fixture.

| Source | Decode p50 | Swap p50 | Buffered codec p50 | Memory codec p50 |
| --- | ---: | ---: | ---: | ---: |
| STL | 3.17 ms | 15.60 ms | 9.88 ms | 9.33 ms |
| OBJ | 71.72 ms | 96.55 ms | 55.11 ms | 46.25 ms |
| PLY float64 | 26.22 ms | 21.09 ms | 13.79 ms | 12.02 ms |

Buffered-codec p95 is 10.13, 56.08 and 14.37 ms respectively. Each fixture
passes complete byte comparisons, nine production reads, zero errors and
last-owner release checks for both managed budgets. Raw evidence and binary
hash are in `target/bench/model-swap-five-tier`. The dramatic direct-file OBJ
and PLY times are therefore avoidable small-read overhead. Differences between
buffered codec and swap still combine multiple costs, including different
reader implementations and integrity; they are not isolated checksum timings.

### OBJ numeric-array block restoration

OBJ positions, texture coordinates and normals now restore through bounded
16 KiB stack blocks, reserving final vector capacity before reading. Float32
bits, finite checks and subsequent index/metadata validation are preserved.
A 2,731-record triple-array regression verifies three block reads plus one
count read, negative-zero bits and credit release after truncated input.
All four OBJ codec tests and five forced Metal readback tests pass.

Eight measured rounds observe OBJ swap p50/p95 91.96/95.39 ms, buffered codec
52.56/53.23 ms and memory codec 43.87/44.77 ms; source decode is 74.07/75.12 ms.
All nine production reads, byte comparisons and final budget releases pass.
Evidence is in `target/bench/model-swap-obj-array-blocks`. Compared with the
preceding sequential run, the observed improvement is modest and is not a
controlled causal result. Corners, triangles and face metadata still use small
reads; OBJ swap remains slower than source decode on this fixture.

### OBJ corner and triangle block restoration

The 14-byte corner records and 16-byte triangle records now read through a
bounded 16 KiB stack block as well. Optional-index tags, index ranges and later
face ownership checks are unchanged. Boundary tests for both record widths
verify exact contents across three blocks and reject a missing final byte.
Four OBJ codec tests and five forced Metal tests pass, including real swap.

An initial eight-round run overlapped GPU qualification and had noisy tails;
it is retained as such, not used for a stable latency claim. A subsequent
sixteen-round run observes source decode p50/p95 71.85/74.69 ms, swap
67.07/68.55 ms, buffered codec 35.12/36.30 ms and memory codec 31.89/33.27 ms.
All seventeen production reads pass byte verification with zero errors and
zero final budget usage. Evidence, both runs and executable hash are in
`target/bench/model-swap-obj-record-blocks`. This fixture now has a lower swap
median than source decode; the sequential before/after comparison remains
uncontrolled and does not prove universal OBJ performance.

### Sustained bounded model spill regression

The application `stl_swap` integration suite now runs 200 transitions through
one-entry RAM and two-entry disk caches, mixing short revisits with scans beyond
disk capacity. Each object is compared to canonical bytes. Real asynchronous
spills finish before the next transition; queue credit returns to zero, exactly
one live frame remains charged, and all CPU credit releases when RAM is dropped.
Both swap hits and source misses occur with zero swap errors or dropped writes.
All four integration tests pass in `/tmp/rrrah-sustained-model.log`.

The shared CPU limit is the measured source-decode peak plus one retained frame,
not merely two final frame capacities: original-file decoding also needs source
and intermediate allocation credit. This scenario validates bounded repeated
transitions and external frame ownership; it does not exercise an unthrottled
queue, physical navigation, large-file throughput or sustained GPU pressure.

### Real model-store corruption qualification

The integration suite additionally mutates real stored STL, shared-metadata
OBJ, attributed binary OFF and ASCII PLY blobs. Each variant is tested with a
changed final byte and a missing final byte. Reads reject the eight damaged
objects, release all managed allocation credit and remove the owned disk file.
A second lookup is a miss without incrementing the error counter again;
replacement under the same key restores successfully and releases its credit
after the final owner drops. All five model swap integration tests pass in
`/tmp/rrrah-model-corruption.log`. The separate memory-pressure test retains
an intact entry for retry, distinguishing capacity rejection from corruption.

### Repeated pressure qualification for all model codecs

STL, shared-metadata OBJ, attributed binary OFF and ASCII PLY each undergo
twenty cycles with their restore budget fully reserved by another owner.
Admission fails without incrementing corruption errors or removing the entry;
after releasing pressure, early cancellation admits no geometry, and the next
restore matches the canonical payload exactly. Every restored allocation
returns to zero after its last owner drops. Each format records one write,
twenty successful reads and zero errors, covering eighty pressure/retry cycles.
All six application model-swap integration tests pass in
`/tmp/rrrah-all-model-pressure.log`. This proves the tested codecs preserve
transient-memory error classification through the real store adapter.
