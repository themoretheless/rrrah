# Native GLB container, JSON and accessor checkpoint — 2026-11-03

This is the first layer toward native GLB/glTF scene loading. It does **not**
register GLB/glTF in model discovery or claim geometry, viewing, materials,
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
padding. Full scene/schema, usage-specific geometry validation, and agreement of
min/max metadata with binary values remain incomplete. No decoded model is
returned. Source JSON bytes and BIN padding remain exact.

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

- [Mesh tests](mesh-tests.log): 79 passed, 0 failed, 0 ignored. Container cases covering original framing, borrowed ranges/unknown chunks, all
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
  Decimal conversion and writes legal `0e-2`. Current79-test result includes the repaired controls.
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

## Required continuation

1. Complete scene/schema and usage-specific accessor validation (including
   source min/max), then assemble geometry while retaining indices, normals,
   UVs and metadata without unchecked allocations. External/data URI resources
   and compressed/quantized extensions remain unsupported.
2. Scene selection, node transforms/instancing, triangle topology, explicit
   unsupported extension/skin/animation boundaries, materials/textures.
3. Common model API, discovery, bounded ownership, cache identity and lossless
   swap; previous/next prefetch with TTL/size/count and cancellation/pressure.
4. Original independent geometry/material references and actual Metal SDR/HDR
   readback/window presentation, including measured file-to-first-frame latency.
   NVIDIA/CUDA remains deferred by the user.
