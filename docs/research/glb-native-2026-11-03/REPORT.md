# Native GLB container checkpoint — 2026-11-03

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

JSON encoding/syntax, asset.version, scene schema, required extensions and BIN
padding relative to buffer.byteLength are deliberately **not yet validated**.
The API calls this a container layout, never a decoded model. Source JSON bytes
and BIN padding remain exact for the upcoming semantic decoder.

The rules come from [Khronos glTF2 GLB specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#glb-file-format-specification).

## Evidence

- [Mesh tests](mesh-tests.log): 67 passed, 0 failed, 0 ignored, including five
  GLB cases covering original framing, borrowed ranges/unknown chunks, all
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
- [Source pins](sources.json). No new dependency or lockfile changes.

## Required continuation

1. Bounded native JSON/schema and embedded buffer/accessor decoding; retain
   precision, indices, normals, UVs and metadata without unchecked allocations.
2. Scene selection, node transforms/instancing, triangle topology, explicit
   unsupported extension/skin/animation boundaries, materials/textures.
3. Common model API, discovery, bounded ownership, cache identity and lossless
   swap; previous/next prefetch with TTL/size/count and cancellation/pressure.
4. Original independent geometry/material references and actual Metal SDR/HDR
   readback/window presentation, including measured file-to-first-frame latency.
   NVIDIA/CUDA remains deferred by the user.
