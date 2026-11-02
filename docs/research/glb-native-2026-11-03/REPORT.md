# Native GLB container and JSON checkpoint — 2026-11-03

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
extension semantics are implemented. Full scene/accessor schema and BIN padding
relative to buffer.byteLength are **not yet validated**. No decoded model is
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

- [Mesh tests](mesh-tests.log): 74 passed, 0 failed, 0 ignored. Container cases covering original framing, borrowed ranges/unknown chunks, all
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
  Decimal conversion and writes legal `0e-2`. Final74-test result is after repair.
- [Source pins](sources.json). No new dependency or lockfile changes.

## Required continuation

1. Complete scene/schema and embedded buffer/accessor decoding; retain
   precision, indices, normals, UVs and metadata without unchecked allocations.
2. Scene selection, node transforms/instancing, triangle topology, explicit
   unsupported extension/skin/animation boundaries, materials/textures.
3. Common model API, discovery, bounded ownership, cache identity and lossless
   swap; previous/next prefetch with TTL/size/count and cancellation/pressure.
4. Original independent geometry/material references and actual Metal SDR/HDR
   readback/window presentation, including measured file-to-first-frame latency.
   NVIDIA/CUDA remains deferred by the user.
