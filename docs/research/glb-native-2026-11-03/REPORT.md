# Native GLB scene, geometry and core material foundations — 2026-11-03

This implements native container/JSON/accessor and selected local-space triangle
primitive decoding and automatic static scene/hierarchy traversal. It does **not**
register GLB/glTF in model discovery or claim full viewing/material interpretation,
cache/swap, Metal, HDR, first-frame timing or format qualification.
The raster catalogue remains 94 implemented subsets, 6 pending, 0 fully qualified.

`rrrah_mesh::glb` parses GLB2 framing directly over borrowed bytes. It validates
magic, version, exact total length, aligned and bounded chunk extents, required
first JSON, optional second BIN, and rejects duplicate/reordered known chunks.
Unknown chunks are skipped with validated extents. Implementation limits are
16 MiB JSON and 65,536 chunks. Cancellation is checked before header parsing,
between chunks and before returning the layout. Parsing allocates no heap and
does not scan or copy BIN payloads. The public file source uses the existing
2 GiB bounded immutable input reader: when given a memory budget, source credit
follows the last clone. Allocation/Arc bookkeeping and OS RSS are separate.

`GlbLayout`/`GlbSource` remain container-only APIs. `GlbDocument` now adds a
native bounded JSON token tape, UTF-8/syntax validation and asset.version2.0 /
minVersion2.0 checks. Nonempty extensionsRequired explicitly refuses until
extension semantics are implemented. The new accessor layer checks requested embedded buffer/view layouts and BIN
padding. The primitive layer additionally checks POSITION bounds, attribute and
index usage. Full scene/schema validation remains incomplete. The common
DecodedModel API is unchanged. Source JSON bytes and BIN padding remain exact.

The parser first counts/validates tokens without heap allocation, then admits
one exact fixed-token buffer and fills it in a second pass. Spans refer to the
immutable original JSON; strings and BIN are not copied. Token storage is limited
to1,048,576 entries (16 bytes each), nesting depth64, source JSON16MiB.
The supplied root budget accounts for source and tape together; with no supplied
budget, the tape uses an internal bounded budget. UTF-8 validation and scalar
conversion are linear over bounded input. Parsing checks cancellation at tokens
and after advancing approximately4KiB of long strings/numbers/whitespace; an
escape crossing a boundary cannot skip future checks. Bookkeeping/stack and
physical RSS are outside the byte credit. Array iteration, field lookup and
scalar conversion do not allocate. Field lookup is linear and uses the last
matching decoded key, as Khronos recommends; no quadratic duplicate-key pass.

Escaped property names, Unicode surrogate pairs, a leading UTF-8 BOM and exact
unsigned integer decimal/exponent forms are supported. Isolated surrogates and
invalid UTF-8 explicitly refuse. Mathematical integer conversion never uses
binary64; huge exponents, fractional/negative nonzero values and u64 overflow
return no integer. The original numeric/string bytes remain retained. Float64
conversion refuses non-finite output. This is selected native JSON evidence,
not full glTF schema or standards certification.

The rules come from [Khronos glTF2 GLB specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#glb-file-format-specification).

## Evidence

- [Mesh tests](mesh-tests.log): 96 passed, 0 failed, 0 ignored. Container cases covering original framing, borrowed ranges/unknown chunks, all
  truncated prefixes, malformed order/alignment/length/version, chunk/JSON
  limits, cancellation, last-owner source credit, exact and one-byte-short
  admission, malformed-input cleanup.
- [Production check](check.log): `cargo check -p rrrah-mesh --lib --locked` passed.
- Original unmodified Cesium Box from Khronos sample-assets commit
  `8f492390ddc7a2ae221a1369ef560589cf8c70fb`, SHA256
  `ed52f7192b8311d700ac0ce80644e3852cd01537e4d62241b9acba023da3d54e`.
  Python `struct.unpack_from('<II', ...)` independently reads JSON20..1008,
  BIN1016..1664; JSON parsing confirms asset.version2.0 and buffer length648.
  Native file-read regression checks those ranges, last-clone ownership,
  1664-byte admission and 1663-byte refusal. This is container agreement,
  not independent geometry/rendering validation. Fixture attribution and
  CC BY4.0 license are in `tests/fixtures/glb/README.md`.
- Original Box document adds165 native tokens to the1664-byte source. It admits
  exactly4304 bytes, holds that credit until the final document clone, and
  refuses4303 bytes with source-only peak1664/final0. Accessor counts36/24/24
  are read through borrowed native JSON fields, not geometry decoding.
- Python stdlib json independently classifies316 authored syntax/token controls;
  Decimal provides60 exact unsigned-integer controls, including2^53+1 and u64
  boundaries. Both fixture files regenerate byte-identically. Duplicate-key and
  escaped-name last-value behavior is tested separately. Depth/token limits
  refuse during the first counting pass before token-buffer admission; one-byte
  short token budget also refuses with zero peak. Document version/schema
  failures release source and token credit.
- The numeric fixture generator initially produced invalid `000e-2`; Rust
  correctly refused. The generator now independently checks JSON grammar before
  Decimal conversion and writes legal `0e-2`. Current96-test result includes the repaired controls.
- [Source pins](sources.json). No new dependency or lockfile changes.

## Borrowed embedded accessors

`GlbDocument::accessor` now resolves embedded buffer0 into `GlbAccessor` without
expanding typed arrays. The immutable source remains the owner. Scalar/vector
and MAT2/MAT3/MAT4 layouts support the six core component types; normalized
signed/unsigned8/16-bit conversion clamps signed minima to-1. U32/float
normalization rejects. FLOAT conversion preserves all finite source float32
values exactly in float64; non-finite values refuse **when read**, not during
layout construction. Raw original bytes and metadata remain in the document.
U32 scalar reads are exact; primitive restart/index range rules are not yet
applied by this low-level accessor API.

The layer checks buffer.byteLength against BIN, accepts only0..3 zero padding
bytes, and never exposes that padding as buffer data. View/accessor bounds use
checked arithmetic. Optional interleaved stride is checked against limits,
component/element size and alignment. Matrix columns include their required
padding; only the final element's unused trailing column padding may be absent.
ARRAY_BUFFER hints enforce vertex alignment, including an explicit stride for
small component types. Usage validation for views without hints remains the
future geometry consumer's responsibility.

Sparse accessors borrow strictly increasing in-range indices and packed values;
base data is either borrowed or implicit zero. Sparse index/value views refuse
stride/target metadata. Index validation is cancellable and linear; each sample
uses bounded binary search and a fixed-size scalar conversion, with no dense
materialization. Accessors are limited to16,777,216 elements. External/data URI
buffers explicitly refuse. Descriptor resolution and samples do not allocate
additional managed storage; this is source/accounting evidence, not a measured
OS heap/RSS or throughput claim. Cancellation during resolution preserves the
immutable document for retry with a new generation token.

[Primary layout rules](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#accessors)
and [current Khronos specification](https://github.com/KhronosGroup/glTF/blob/main/specification/2.0/Specification.adoc)
were consulted. The native layout reader is independently authored; Khronos
validator source was consulted only to clarify matrix alignment, not linked into
the runtime or used as an executed oracle.

Current evidence:

- 180 original Box components (36 scalar indices,24 three-component normals and24 three-component positions) match
  independent Python struct reads, including all source declared strides/offsets.
- 48 authored accessors /714 component samples match bit-exact float64 oracle
  values. Cases cover all core component encodings, signed normalization limits,
  minimum float32 subnormal and signed zero, matrix column/final padding,
  interleaved data, implicit zero, all three sparse index encodings, sparse
  replacement of dense values and sparse padded matrices. U32 non-scalar
  configurations are not authored as core-valid usage.
- 43 authored malformed layouts refuse: range/overflow, component/matrix/vertex
  alignment, stride/target, missing/external buffers, count/type/normalization,
  BIN padding, and sparse count/order/index/stride/target/value-range failures.
  These are explicit contract controls, not independent full-glTF certification.
- NaN/+Inf/-Inf component reads refuse. Cancellation/new-generation retry and
  final-owner cleanup pass. Across repeated descriptor/sample reads, supplied
  budget usage and peak remain unchanged; final document release returns0.
- All five artifacts from `scripts/generate-glb-accessor-fixtures.py` regenerate
  byte-identically; Python syntax, Cargo formatting and production check pass.
  The sample oracles and malformed controls have distinct provenance in
  `tests/fixtures/glb/accessor-manifest.json`. Source hashes are pinned in `sources.json`.

## Borrowed local triangle primitives

`GlbDocument::primitive(mesh, primitive, request)` now validates a source-local
triangle primitive and returns borrowed `GlbPrimitive` views. Indexed and
nonindexed TRIANGLES, TRIANGLE_STRIP and TRIANGLE_FAN preserve source ordering,
winding and degenerate triangles; no triangle vector or duplicate vertex array
is allocated. Triangle lookup/iteration use the immutable validated accessors.
Point/line modes, missing POSITION and morph targets explicitly refuse at this
checkpoint. Node transforms/instancing, skins and animations are not applied.

POSITION must be unnormalized float32 VEC3 with3-component min/max metadata.
Values are read finitely and promoted exactly to float64; POSITION bounds are
accumulated in that same validation pass rather than rereading every position.
The already resolved POSITION accessor is reused. No throughput speedup is
claimed without the forthcoming scene-to-presentation measurement. Declared bounds are
interpreted in the accessor's float32 type and compared to actual complete
POSITION bounds. The specification's implicit-zero/no-storage exception is
retained. Geometric bounds separately include only referenced vertices, so an
unused distant vertex does not distort fitting. Indices must be unnormalized
unsigned scalars with proper view usage; every value must fit POSITION.count
and must not be the component type's reserved maximum. Topology counts are
checked before returning a primitive.

Active attributes must have equal counts and supported core component/type
combinations. Shared vertex views require explicit stride; small components and
view targets obey vertex/index usage rules. Normal/tangent squared lengths use
a1e-4 acceptance tolerance around1 without modifying source values; tangent W
must be+1/-1. Weight components cannot be negative. Indexed semantic families
must start at0 without gaps/leading zeroes. Custom underscore attributes remain
available through borrowed accessors. Material references must resolve to an
object; all source material/primitive JSON remains retained, but PBR/texture
interpretation, color clamping at shading, combined skin weights and full
metadata/schema conformance are still implementation/qualification work.

Last decoded duplicate attribute names win, including escaped spellings. Raw
attributes are capped at64 and each key at4096 source bytes, bounding the stack
scratch and duplicate-name comparison. Aggregate attribute component validation
is capped at128 million. Scans check cancellation every1024 elements. These are
implementation limits; broad large-scene timing and OS memory are not qualified.

Current proof in the96-test mesh run:

- 12 original Box local triangles match independent Python struct index/position
  reads. The source node matrix is deliberately not applied by this API.
- 20 authored triangles across9 meshes match independent source/index references:
  indexed/nonindexed triangles, strips/fans, sparse positions, translated float32
  geometry, unused far vertices and a retained degenerate triangle. Analytic
  signed areas separately verify clockwise strip / counterclockwise fan and
  square footprints. These controls are not scene/material interoperability.
- Source normals, tangents, two UV sets, color and a custom temperature attribute
  remain accessible. The first authored attributes object intentionally contains
  an invalid earlier POSITION reference followed by the valid escaped-name
  override, checking last-value semantics without dropping original JSON.
- 30 malformed contracts refuse without additional source/tape credit: index
  range/restart (including U8 restart within a256-position range), topology,
  attribute count/semantics/sets/limits/shared-view stride, non-unit normals,
  nonfinite positions, declared bounds, missing material and unsupported modes.
- Cancellation/new-generation retry and final-owner release pass. Repeated
  primitive resolution/triangle iteration leaves managed usage and peak unchanged;
  this is not an independent measurement of OS allocations or GPU performance.
- All six primitive-generator artifacts regenerate byte-identically. Python
  syntax, production check, Cargo formatting and diff checks pass. No dependency
  or lockfile change. Existing JSON/accessor proofs are included in the same
  current test log; old intermediate logs are replaced rather than duplicated.

## Local node transforms and affine math

`node_local_transform` now reads a node's matrix or TRS into a fixed128-byte
column-major `GlbTransform`. Matrix and any TRS property may not coexist, even
if a TRS field is null or default-valued. Transform arrays require exact sizes
and finite float64 interpretation. Raw original JSON remains unchanged.

Local matrix bottom row must be affine. Normalized nonzero columns must be
orthogonal within1e-6, accepting negative/zero scales but refusing local shear.
Quaternion components must stay in[-1,1] and length must be within1e-6 of1; accepted rounded quaternions are
normalized for calculation. TRS composition uses T*R*S. Parent.compose(local)
uses parent*local and deliberately permits world shear produced by valid nested
nonuniform TRS; it does not reapply the local decomposability restriction.

Point transformation checks arithmetic range. Normal transformation uses the
inverse-transpose direction with per-column scale extraction and binary-exponent
ratios, then normalizes. It avoids forming a huge reciprocal matrix. Orientation
reports mirrored/preserving/singular for supported numerical conditions. A
normalized determinant magnitude below1e-12 explicitly refuses reliable normal
or nonzero-orientation interpretation; this is an Unsupported numeric boundary,
not evidence a valid scene is corrupt. Singular geometry can still transform
points, while its inverse normal transform explicitly refuses. Arithmetic range
exhaustion returns Limit; malformed/nonfinite source values remain Invalid.
Values outside float64 precision/range are not universally qualified; ordinary
floating underflow may discard unrepresentable components. No exact-arithmetic
or all-extreme-transform guarantee is asserted.

Evidence in the current96-test run:

- 40 point transformations across8 source nodes agree with independent80-digit
  Decimal Hamilton quaternion multiplication / explicit matrix math. Cases cover
  identity, translation, reflection, nonuniform/zero scale, source matrices and
  quaternions with all X/Y/Z/W components nonzero.
- 35 transformed normals agree with an independent Decimal Gauss-Jordan inverse
  followed by normalization. A composed parent/child world-shear case is included.
  The point/normal comparison tolerance is2e-14*max(1,abs(expected)).
- All12 original Box triangles are transformed using its original node0 matrix
  composed with node1's identity and match the independent transformed-coordinate
  oracle. This is manual known-node composition, **not automatic hierarchy or
  scene-selection qualification**. The original source plus token peak remains4304.
- 21 malformed source-node contracts refuse with final credit0. Cancellation and
  new-generation retry preserve the immutable source; final document clone owns
  source/token credit. Transform math creates no additional managed allocation.
- Analytic extreme diagonal cases include1e300/1e-300 and the smallest float64
  subnormal. Unit direction and its expected1e-300/subnormal components are checked,
  rather than only checking finiteness. Valid world shear composes, local shear
  refuses, zero-scale points remain usable, and singular/ill-conditioned normals,
  nonfinite input and overflowing point/composition cases explicitly refuse.
- Six transform-generator artifacts regenerate byte-identically; Python syntax,
  Cargo formatting, production check and diff checks pass. No new dependencies.

## Owned static scene hierarchy and instancing

`GlbDocument::scene(selection, request)` now builds an owned admitted static scene.
It clones the immutable document/source/token owner and allocates one fixed160-byte
node descriptor per source node. Geometry remains in the original shared BIN.
Cloning a scene shares both source and node-table allocation; final owners retain
their credits. Internal JSON handles avoid self-referential borrowed scene storage.
Bookkeeping and bounded stack scratch remain outside managed heap accounting.

Every node and child edge is processed with checked references and single-parent
admission; duplicate children, cycles and multiple parents refuse. Parent-chain
resolution is iterative, using at most64 stack entries, and computes each world
matrix once. Depth is checked even when a parent was already resolved. Limits
are65,536 nodes /64 hierarchy levels. All source scene root lists are checked
for duplicates, bounds and true-root status; the same root can belong to different
scenes. This is structure/transform validation, not complete glTF schema validation.

Explicit selection overrides a valid default asset.scene; without either, scene0
is selected deterministically. An absent scenes array explicitly reports an
unsupported scene-less asset; an empty scene object is supported. Explicit empty
nodes/children/root arrays refuse per the schema. Selected traversal retains
root/child order, including nodes without meshes. Instance iteration emits one
node/mesh/world-matrix record per selected mesh node; it does not duplicate mesh
arrays. Selected skin or morph-weight nodes explicitly refuse. Animations are
not applied: this API describes the declared static source pose.

`scene.primitive(node, primitive, request)` resolves local geometry lazily using
the existing primitive validator, then supplies fallible world-space triangle
reads. World-point arithmetic range errors propagate; no complete model is
published by this layer. It does **not** prevalidate every mesh/material, compute
whole-scene fitting bounds, schedule animation or integrate the application,
cache/swap or GPU renderer. Those remain required continuation.

Evidence in the current96-test mesh run:

- Automatic default Box selection visits nodes0,1 and emits node1/mesh0. All12
  triangles match the independent original-node transformed-coordinate oracle.
  No manual parent selection is used by this new test.
- Four authored scenes /7 nodes emit8 total selected instances in the independent
  expected order, with18 world triangles matching recursive80-digit Decimal
  source-pose references. Controls include reused meshes, source root order,
  child order differing from node-index order, translation/rotation/nonuniform
  scale, reflection, composed world shear, alternate/default and empty scenes.
- Box admits exactly4624 bytes:4304 original source/tokens plus320 for two nodes.
  A4623 limit refuses metadata before allocation, retaining source-only peak4304.
  A one-byte external reservation causes a retryable metadata refusal; releasing
  it permits the same immutable document to build the scene. Cancellation/new
  generation retry and last-scene-clone cleanup return final0. Source BIN pointer
  identity proves source sharing; this is not a total physical heap/RSS claim.
- 23 authored structure/boundary controls refuse, covering cycles/self-cycles,
  multiple parents, duplicate children/roots, invalid refs/types/default selection,
  scene-less/empty-array boundaries and selected skin/weight refusal. They are
  contract controls, not independent full-glTF conformance certification.
- Reverse-index parent chains succeed at64 and refuse65; the leaf translation
  is analytically64. A2048-node flat forest is traversed, while65,537 nodes refuse
  before node-table admission. An empty scene without nodes succeeds.
- Five scene-generator artifacts regenerate byte-identically. Python syntax,
  production check, Cargo formatting and diff checks pass. Latest logs replace
  intermediate runs in this same report; no dependency/lockfile change.

Hierarchy/root constraints were checked against the primary
[Khronos scene schema](https://raw.githubusercontent.com/KhronosGroup/glTF/main/specification/2.0/schema/scene.schema.json)
and [node schema](https://raw.githubusercontent.com/KhronosGroup/glTF/main/specification/2.0/schema/node.schema.json).
The runtime remains native Rust; no external glTF runtime is invoked.

## Core material data and point-texel reference

`GlbDocument::material` now reads core metallic-roughness factors/defaults,
base/emissive colors, alpha mode/cutoff, double-sided metadata and five texture
bindings. Values and lengths are checked against core constraints. Normal scale
may be negative; occlusion strength stays within[0,1]. Alpha cutoff may exceed1,
which makes MASK fully transparent; OPAQUE/BLEND ignore that cutoff. Texture
indices, image references, sampler descriptors and embedded image bounds resolve
without copying image bytes. Missing filter fields remain unspecified; wrap
fields default toREPEAT. Unknown sampler enums/alpha modes explicitly report
Unsupported, distinguishing an unimplemented future value from malformed types.

Images expose either a borrowed PNG/JPEG BIN slice with required MIME, or a
retained URI string. Conflicting/missing sources and image stride/target refuse.
URI/data resources are **not** resolved or fetched. Encoded pixels, color/profile
handling, filtering and mip generation are not decoded/executed by this layer.
Material texCoord references can be checked against a validated primitive's UV
sets; the current set-index resource limit remains64. Original unknown metadata
and optional extension JSON remain preserved rather than being interpreted.

`GlbMaterial::evaluate_texel` is a fixed-size CPU reference for unfiltered sample
semantics, **not a BRDF or rendering fallback**. Base/emissive texture RGB is
sRGB decoded; factors and vertex colors remain linear, and vertex color clamps
to[0,1]. Base alpha is linear. Metallic uses B, roughness G, occlusion R with
strength interpolation, and the normal map uses linear RGB with XY scale then
normalization. Unused channels are ignored. Bound-but-missing texture samples,
nonfinite used values and zero reconstructed tangent normals refuse. Output
RGB is straight/unpremultiplied; MASK returns discard/coverage separately.
Occlusion remains a separate parameter rather than incorrectly darkening all
base/emissive light. Double-sided metadata is retained; culling/back-face normal
handling and lighting are not implemented here. The texture-role enum separates
sRGB roles from data roles; it does not yet constitute GPU cache-key validation.

Current evidence in the96-test mesh run:

- Eight authored materials have32 surface evaluations matching an independent
  80-digit Decimal oracle within2e-13*max(1,abs(expected)). Cases cover factors,
  channel swizzles, both sRGB branches, linear vertex color/clamping, straight
  alpha, OPAQUE/MASK/BLEND, cutoff equality/above1, negative/zero normal scale and
  occlusion strength. Publicly edited numeric factors are also checked on use.
- 28 metadata/type/reference/unsupported-boundary controls refuse. Missing bound
  samples and nonfinite used channels refuse; nonfinite ignored channels remain
  irrelevant. UV linkage succeeds for two source UV sets and refuses a mesh
  without the referenced set. No original codec or sampler output is claimed.
- Original unmodified Cesium BoxTextured from Khronos sample-assets commit
  `8f492390ddc7a2ae221a1369ef560589cf8c70fb`,5956 bytes, SHA256
  `b510eca2e2ef33f62f9ed57d6e7ce2d10ebb2bdebc4a8e59d347719ba81abdf4`:
  native material0 preserves white linear base, metallic0, default roughness1,
  OPAQUE and base-color texture0/UV0. Sampler9729/9986/REPEAT is exact. Its3750-byte
  PNG at absolute2204..5954 matches the independent source byte-range census and
  stays pointer-shared across document clones. This is **byte/metadata evidence,
  not image decoding, texture filtering, rendered likeness or HDR qualification**.
  Attribution/CC BY4.0 and original mark notice are retained with the fixture.
- Metadata reads and reference evaluation do not increase managed usage/peak;
  final document owner returns0. Cancellation/new-generation retry passes. This
  does not measure physical heap/RSS, source-to-frame latency or GPU allocations.
- All six material-generator outputs regenerate byte-identically. Negative
  metadata controls reuse one BIN rather than duplicating payloads. Python
  syntax, formatting, production check and diff checks pass; no dependencies or
  lockfile edits. Tests/check used isolated target
  `/Users/themoretheless/.codex/tmp/rrrah-glb-material-target-20261103`: the queued
  shared-target run was deliberately stopped after a live independent dedup
  build on another toolchain was confirmed to hold that target lock.

Core rules were checked against primary Khronos
[material](https://raw.githubusercontent.com/KhronosGroup/glTF/main/specification/2.0/schema/material.schema.json),
[PBR](https://raw.githubusercontent.com/KhronosGroup/glTF/main/specification/2.0/schema/material.pbrMetallicRoughness.schema.json),
[sampler](https://raw.githubusercontent.com/KhronosGroup/glTF/main/specification/2.0/schema/sampler.schema.json)
and [image](https://raw.githubusercontent.com/KhronosGroup/glTF/main/specification/2.0/schema/image.schema.json)
schemas. Normal/occlusion metadata follows the associated primary schema files.
Full PBR lighting, texture pixels/ICC-EXIF policy, extension semantics, sampler
filtering/mips, material GPU upload/cache identity and live presentation remain
required; GLB is still not registered as normally viewable.

## glTF-specific embedded PNG/JPEG pixel decoding

The unified `rrrah-decode` facade now exposes `glb` and
`decode_glb_texture`/`decode_gltf_texture_bytes`. This adapter uses the existing
pinned vendored image / Rust PNG / zune JPEG backends; it does not implement a
new entropy codec or invoke an external runtime decoder. FFmpeg is oracle-only.
PNG8/16 grayscale, gray-alpha, RGB, RGBA and palette expansion preserve source
samples in admitted RGBA8/RGBA16 buffers. Core JPEG is currently8-bit,1/3-component
baseline/extended-sequential/progressive with fixed positive dimensions; other
coding/depth/components and deferred height/DNL explicitly refuse.

The adapter bypasses the ordinary photo display importer. A bounded admitted
range table creates virtual input over the original bytes, dropping non-pixel
metadata before codec construction. PNG ICC/gamma/chromaticity/EXIF/text/physical
aspect and animation controls are ignored; the default PNG IDAT image remains.
Palette/tRNS remain because they affect pixels. JPEG APP1/2 and other non-pixel
APP/comment segments are omitted while JFIF/Adobe hints and coded data remain.
Orientation is intentionally not applied. PNG framing/CRC and consecutive IDAT
rules, JPEG marker/extents/entropy boundaries, exact end framing and MIME magic
are checked. Original caller bytes stay immutable. This is not validation of
ignored metadata payload semantics or every PNG/JPEG variant.

PNG input is not copied into an intermediate sanitized file. The existing JPEG
backend still owns a compressed-input copy: its exact Vec capacity is admitted
before retention. Final RGBA capacity is reserved before codec construction;
native-channel overlap and range tables also consume root credit. The output
retains only its shared pixel allocation. Color/data role is explicit; raw RGB
stays encoded for sRGB roles and numeric for data roles, with straight alpha.
`with_role` shares identical pixels rather than decoding/copying again. No ICC,
orientation, display transfer or tone mapping is inherited from normal raster
import. Image filtering/mips and color decoding before GPU filtering remain
separate rendering work. External/data URI resolution still explicitly refuses.

**Accounting boundary:** backend-internal row/entropy/progressive scratch is not
fully charged to the shared root. Codec/dimension settings bound the supported
image shape, but these checks do not prove a process/RSS bound equal to the root
quota. A JPEG CPU decode cannot be interrupted inside the backend call; source
reads/conversion and pre/post publication checks are cancellable. This limitation
remains open for complete working-memory/latency qualification.

Implementation resource limits are2GiB encoded input,65536 retained ranges /
PNG chunks,65536 per dimension and512MiB final RGBA. Dimension limits are checked
before output multiplication, including adversarial uint32-max dimensions.

Current evidence:

- [Decode regression](texture-decode-regression.log):280 passed,0 failed,9 existing
  manual cases ignored. The four new tests include15 authored/reused PNG/JPEG
  controls: exact PNG8/16 codes and alpha, palette+tRNS, ICC/EXIF/gamma/aspect/text
  bypass, APNG default image, baseline/progressive JPEG and tagged counterparts
  (existing independent JPEG oracle tolerance2 levels).
- All262144 RGBA bytes of the original BoxTextured256x256 PNG match independent
  FFmpeg9.0.2 readback, not only metadata or dimensions. Original source/license
  attribution remains with the model. This still does not qualify a rendered
  scene, sampler or GPU texture.
- Role retagging preserves pixel pointer identity and final-owner credit.
  Source/codec transients release before returning the output. For the authored
  RGBA3x2 case, managed peak120 =96 range bytes +24 output; a one-byte external
  reservation refuses, releasing it allows same-source retry, and final credit0.
  This deliberately excludes caller-owned encoded input and backend scratch;
  it is not a claim the codec runs in120 physical bytes.
- Every truncated prefix of selected PNG/baseline/progressive sources refuses.
  CRC/MIME failures, oversized dimensions and unsupported deferred JPEG height
  refuse without leaked owned buffers or integer-overflow panic. Cancellation/
  new-generation retry passes. Broad malformed entropy and mid-codec cancellation
  remain additional qualification work.
- [Production check](texture-decode-check.log), formatting and diff checks pass.
  Check retains the pre-existing jxl-color lifetime warning; no unrelated fix was
  made. All codec fixture files regenerate byte-identically with the recorded
  [oracle provenance](texture-codec-provenance.json). No dependency/lockfile edits.

The metadata override follows the primary
[Khronos image rules](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#file-extensions-and-media-types).
No runtime LibRaw, FFmpeg or external viewer is used by this texture path.

## Resolved local vertex streams for material rendering

`GlbPrimitive::vertex_streams` resolves its actual declared/default material,
NORMAL/TANGENT/COLOR_0 and material-selected TEXCOORD sets once. Its borrowed
`GlbVertexStreams` samples indexed vertices or winding-preserving triangles
without expanding whole meshes or reparsing JSON for each corner. Five texture
roles retain separate UV bindings; roles sharing a set reuse both resolution
and the sampled UV pair. Source indices, strip/fan winding, normalized and sparse
accessor semantics stay delegated to the validated primitive/accessors.

`GlbVertex` contains local position, optional normal/tangent, clamped linear
vertex color (default white, VEC3 alpha defaults to one), and optional UV per
material role. Missing normal/tangent stays explicit: no fabricated frame is
installed. Instance transforms remain with GlbScenePrimitive; this layer does
not apply them, generate derivative frames, light, draw, cache or publish GLB.

- [Regression](vertex-stream-regression.log): 98 mesh tests passed, zero failures.
- All 36 indexed producer BoxTextured corners agree with a separate Python
  json/struct reference for source indices, positions, normals and UV0.
- Authored material controls verify normal map UV1 versus four UV0 roles,
  tangent handedness, vertex RGBA, missing attributes/unbound UV defaults,
  invalid/missing UV sets, cancellation and recovery with a new generation.
- Stream resolution/sampling leaves the document's managed used/peak credit
  unchanged; after the document drops the root reaches zero. Corner values
  are copied into small stack records; whole geometry remains borrowed.
- [Production check](vertex-stream-check.log), formatting and diff checks pass.
- Reference generator and source/reference hashes are pinned in sources.json
  and tests/fixtures/glb/vertex-reference-manifest.json. Original model/reference
  remains subject to the existing Cesium attribution, not a new CC0 asset.

## Owned GLB scene/image asset and real cache composition

`rrrah_decode::decode_glb_asset(request, selection)` now returns an immutable,
cloneable `GlbAsset` with the selected owned hierarchy, unique source-primitive
metadata and decoded-image table. It validates each selected source primitive's
geometry/material/UV bindings, decodes each referenced embedded image once and
retains the original container/tape. Nodes sharing a mesh retain separate world
transforms and share source geometry. Different texture indices, samplers and
color/data roles can point to the same decoded image; retagging shares pixels.

Borrowed `GlbDocument::meshes`/`GlbMesh::primitives` permit linear mesh traversal
without repeated root mesh-array lookup. The source is not expanded to world
triangle/vertex arrays. Asset bounds are a conservative union of transformed
local AABB corners, **not tight per-vertex world fitting**. Raw whole-container
bytes, including unknown chunks/padding, remain available for future lossless
swap. This change does not implement that swap path or wire the app's ModelReady
worker/cache/renderer. GLB remains excluded from ordinary discovery/viewing.

All owned metadata vectors and pixels use the request's shared root. Tables are
admitted before allocation and actual capacities reconciled. Source/node/JSON
capacity accessors and `GlbAsset::capacity_bytes` count retained managed data once
per asset; clones share allocations. Inline, Arc, allocator, codec-internal and
process/driver overhead remain excluded. Absent an explicit root, this API uses
a finite 1 GiB logical budget. Limits: 65536 source meshes/images/unique selected
primitives, 16777216 selected source vertices and 1048576 instance-primitive
bound contributions, in addition to the existing container/codec limits.
Referenced external/data URI images still refuse. Unsupported inactive meshes
or images are not mistaken for selected content. No partial asset is published
when a later selected primitive or image fails.

[Controls](asset-controls.log) and [combined regression](asset-regression.log):

- Original BoxTextured: complete container and all 262144 RGBA bytes agree with
  retained source/independent FFmpeg fixture; one selected primitive, one image,
  twelve triangles. On this Mac its retained managed weight is 272620 bytes and
  preparation peak 469356; capacity weight equals the root's retained occupancy.
- A budget of 469355 refuses and releases everything; 469356 succeeds. These
  are logical managed counters, not a physical RSS bound.
- Three instances share one primitive/image. Base-color and normal texture
  indices share image0 despite different samplers; color/data pixel handles are
  pointer-equal. Known translated AABB union is [-2.5,-.5,-.5]..[2.5,.5,.5].
- Real `rrrah_memory::LeaseCache<_, GlbAsset>` verifies exact byte refusal,
  count-limit protection by live consumer leases, later replacement/eviction,
  TTL eviction and retained consumer ownership, with final root zero. TTL policy
  changes affect future insertions; existing deadlines remain fixed. This is
  composition with the actual cache, **not integration with ModelDisplayCache**.
- Stale generation, new-generation retry, selected point primitives/empty
  primitive arrays and failure on a second URI image after decoding the first
  all release transient and retained credits on error.
- Regression: decode 288 passed/9 manual ignored, mesh 98 passed, GPU 80
  passed/10 manual ignored; no failures. The six material controls, including
  three actual M4 Max/Metal probes, pass again after the native changes.
- [Production checks](asset-check.log), formatting and diff checks pass. The only
  dependency change is a local rrrah-memory **test dependency**; no third-party
  version or production dependency changes.

## Streaming lossless GLB snapshots

`glb_asset_swap_payload_len`, `write_glb_asset_swap_payload` and
`read_glb_asset_swap_payload` now implement an owned-asset native snapshot.
Restoration never opens the request path and does not invoke PNG/JPEG entropy
decoding. Source framing/JSON/selected geometry/materials are reconstructed and
validated, and cached pixel layouts are checked against encoded image headers
before pixel allocation. A shared preparation function keeps fresh and restored
scene/material/image-index semantics identical. This is a native payload API
with actual SwapStore composition, **not app ModelPayload/cache/navigation wiring**.

The RRGLB001 wire representation is independent of native struct/ABI layout:

| Part | Encoding |
| --- | --- |
| Header (32 bytes) | magic RRGLB001 (8), selected scene u64, source length u64, active image count u64 |
| Source | Complete original GLB, including padding and unknown chunks |
| Per-image header (32 bytes) | source image index u64, width/height u32, sample bits u32, reserved zero u32, pixel byte length u64 |
| Per-image data | Straight RGBA8 bytes or canonical little-endian RGBA16 samples |
| Footer (32 bytes) | BLAKE3 of every preceding payload byte |

All integers above are little-endian. Active image records follow ascending
source-image order, including sparse source IDs, and each referenced image is
stored once. Counts/layouts, reserved fields and checksum must agree. Reads are
bounded by the supplied payload size and leave the next envelope record unread.
Writes stream directly; there is no whole serialized heap buffer. Fixed 16 KiB
stack scratch handles u16 conversion/I/O chunks. Interrupted/short reads/writes
retry while checking cancellation; a blocking OS call itself is not interruptible.
The caller owns identity/versioning, spill policy and atomic file publication.

[Four snapshot controls](swap-controls.log) now prove:

- Native PNG8, PNG16, JPEG baseline and progressive round-trip to identical
  serialized bytes, source bytes and retained asset weights; the original source
  file is absent for the authored-image cases. Per-thread instrumentation records
  zero additional texture decode calls during restore.
- PNG16 wire bytes agree with the independent LE16 fixture oracle, preserving
  low bits. Unknown XTRA chunks round-trip unchanged. A progressive case retains
  active source image1 while source image0 is an unused URI; record indices stay
  distinct from compact decoded-table indices.
- Explicit scene0 is preserved when source default is scene1; empty scene3 also
  round-trips. A following sentinel envelope record is not consumed.
- Real SwapStore streaming writes/reads the 268196-byte original BoxTextured
  snapshot at exact disk payload quota. A second object refuses, cloned handles
  retain disk credit, and final handle/asset drops release disk and RAM roots.
  Cancellation partway through pixel writing rolls back the temporary file and
  disk object/byte admission; no partial handle is published.
- Snapshot restore retains 272620 managed bytes and peaks at 272628 on this Mac;
  272627 refuses and releases all owners, while 272628 succeeds. Fresh decode
  still peaks at 469356 for this fixture. These are managed allocation counters,
  **not physical RSS/disk-block measurements or a general latency benchmark**.
- All header prefixes and source/image/footer boundary cuts refuse; corrupted
  magic/scene/counts/image records/pixels/footer refuse. A rechecksummed image
  dimension mismatch still refuses. Short/interrupted reads/writes, cancellation
  during pixel reads and pre-cancelled writes leave no retained allocation.
- Full regression is decode 288 passed/9 manual ignored, mesh 98 passed, GPU
  80 passed/10 manual ignored; the six material controls with three actual Metal
  probes pass again. Production/format/diff checks pass; no dependency changes.

There is still no cross-process persistent cache key/versioning policy, app spill
queue, GLB prefetch or visible generation publication for this payload. A native
write failure can leave bytes in an arbitrary caller-owned Write sink; atomic
rollback is the SwapStore/caller's responsibility and is tested above.

## Application model-cache specialization for GLB

The application's existing ModelDisplayCache state machine is now a typed
`ModelCache<M: CachedModel>`; `ModelDisplayCache` remains the concrete alias for
the four existing DecodedModel formats. Their production RAM, visible-pin,
lease, asynchronous spill, retention preflight and pressure-retry algorithm is
shared unchanged with `GlbAsset`. `glb_cache::Payload` connects RRGLB001 to the
real ImageSwapCache worker instead of a separate GLB-only cache algorithm.

The GLB key contains the existing path/size/mtime/sampled fingerprint and frame
index, plus optional scene selection and an explicit interpretation policy
revision. Unsupported nonzero image indices cannot produce a GLB key. The raw
asset retains its own selected-scene index in the payload. Worker dispatch must
couple the requested scene with the key/decoder; ordinary app GLB dispatch is
still not connected and no viewability registration is added by this change.

Nested managed allocation errors from the container/texture layers remain typed
`SwapPayloadError::Memory`, so transient pressure keeps the valid disk blob.
Impossible individual allocations do not trigger futile LRU eviction. Minimum
restore weight is a conservative source-byte plus pixel-sample lower bound;
source-side spare capacity is not assumed to survive reconstruction. Hash-scratch
parallelism remains disabled because JSON/scene metadata can expand after the
source stream has already been read. The existing stream layer supplies spill/
restore cancellation; CPU-only metadata work currently observes that outer
cancellation at the next I/O/final publication check, not inside the callback's
dummy DecodeRequest. Native standalone calls with a request token retain their
finer internal cancellation checks. No latency bound is claimed for this adapter.

[Five application cache controls](app-cache-controls.log) pass:

- Resident GLB hit shares storage and bypasses the decoder. A visible consumer
  lease prevents eviction; after release the actual asynchronous spill completes
  and RAM returns to zero. Retained restore uses the stored blob with the decoder
  replaced by a panic guard; final lease/cache/asset drops return the root to zero.
- Actual TTL expiration calls the common spill path, then restores original
  pixel-exact BoxTextured data without native decode.
- A 6000-byte restore parent refuses in nested JSON admission, leaves the blob
  valid and increments no corruption/read count. A larger-budget retry succeeds.
- A protected count slot refuses before decode. A cold replacement under global
  memory pressure reclaims the old unpinned asset and retries successfully.
- Scene/policy keys differ, pre-cancelled jobs never decode, nested memory errors
  are classified across layers, and a one-byte hard cap is non-retryable.

[Existing model regression](app-model-regression.log):33 passed,0 failures,
7 manual checks ignored. This covers their lease/TTL/restore/prefetch behavior
after the shared-cache refactor. The separate
[actual Metal pressure regression](app-model-metal.log) passes for STL, OFF,
shared-metadata OBJ and 64-bit PLY, preserving exact SDR/HDR output after retry.
That hardware probe is regression evidence for the existing four models,
**not a GLB cache-to-render or physical HDR-display qualification**.
[Production app check](app-cache-check.log), formatting and diff checks pass;
only the previously existing jxl-color lifetime warning remains. No dependencies
or toolchains changed.

The GLB specialization is compiled and exercised through the application's actual
cache implementation. It is not yet selected by the production loader/worker or
mixed-model cache. Navigation, previous/next scheduling, generation handoff and
GPU renderer installation for GLB remain required before ordinary viewing.

## Application integration update

The intermediate loader exclusions above have been superseded. Ordinary GLB
discovery, mixed model cache/spill dispatch, retained neighbour prefetch, bounded
GPU preparation, viewport installation, camera/resize and submission leases are
now connected. The current application regression passes 362 tests, and actual
Metal readback and live-window device recreation/navigation pass. See
[the current GPU/viewer report](../glb-material-gpu-2026-11-03/REPORT.md#ordinary-viewer-integration-and-current-qualification)
for pinned source/binary/log evidence and exact limits.

## Required continuation

1. Broader independent producer/render comparisons and whole-schema rules;
   general MikkTSpace/seams, transparent intersection ordering and texture variants.
2. External/data resources, extensions, skin/morph/animation and point/line geometry.
3. Broader slow/cold-disk and large-collection pressure, source-identity races,
   cancellation latency and measured file-to-first-frame performance. The ordinary
   viewer now passes 1000 count-pressure native-swap cycles with decoder fallback
   forbidden; the no-cache negative control fails as required (GPU report).
4. Physical HDR/display qualification beyond offscreen linear HDR. NVIDIA/CUDA
   remains deferred by the user.

Schema follow-up: a component just above1 with near-unit quaternion length now refuses before normalization. The21 source-node controls include this case; the component range is independent of the length tolerance.
