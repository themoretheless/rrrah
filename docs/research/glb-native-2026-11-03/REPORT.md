# Native GLB geometry and node-transform foundations — 2026-11-03

This implements native container/JSON/accessor and selected local-space triangle
primitive decoding toward GLB/glTF scene loading. It does **not** register GLB/glTF
in model discovery or claim automatic scene/hierarchy traversal, viewing, material interpretation,
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

- [Mesh tests](mesh-tests.log): 86 passed, 0 failed, 0 ignored. Container cases covering original framing, borrowed ranges/unknown chunks, all
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
  Decimal conversion and writes legal `0e-2`. Current86-test result includes the repaired controls.
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

Current proof in the86-test mesh run:

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
Quaternion length must be within1e-6 of1; accepted rounded quaternions are
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

Evidence in the current86-test run:

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
- 20 malformed source-node contracts refuse with final credit0. Cancellation and
  new-generation retry preserve the immutable source; final document clone owns
  source/token credit. Transform math creates no additional managed allocation.
- Analytic extreme diagonal cases include1e300/1e-300 and the smallest float64
  subnormal. Unit direction and its expected1e-300/subnormal components are checked,
  rather than only checking finiteness. Valid world shear composes, local shear
  refuses, zero-scale points remain usable, and singular/ill-conditioned normals,
  nonfinite input and overflowing point/composition cases explicitly refuse.
- Six transform-generator artifacts regenerate byte-identically; Python syntax,
  Cargo formatting, production check and diff checks pass. No new dependencies.

## Required continuation

1. Scene selection and node hierarchy/transforms/instancing; retain the local
   primitive geometry rather than flattening source metadata. Complete source
   min/max/schema and usage rules beyond the selected contracts above.
2. Materials/textures, skin/morph/animation handling and point/line geometry.
   External/data URI resources and compressed/quantized extensions remain
   unsupported; required extensions still explicitly refuse.
3. Common model API, discovery, bounded ownership, cache identity and lossless
   swap; previous/next prefetch with TTL/size/count and cancellation/pressure.
4. Original independent geometry/material references and actual Metal SDR/HDR
   readback/window presentation, including measured file-to-first-frame latency.
   NVIDIA/CUDA remains deferred by the user.
