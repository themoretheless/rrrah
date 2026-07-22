# V3 cache object and Mosaic Payload V1

Status: normative wire-format draft. Integers are unsigned little-endian. Byte
ranges are half-open. Rust layout, native integer width and serde output are
never protocol encodings.

This protocol deliberately has three independent version axes:

| Axis | Current value | Changes when |
|---|---:|---|
| semantic identity protocols | SourceId/Manifest/RecipeId/ArtifactKey V1 | a transcript, field layout or hash protocol changes |
| object envelope | 1 | framing or integrity layout changes |
| payload schema | `(id=1, version=1)` | physical mosaic representation changes |

Payload schema `(1,1)` maps to `RRMPAY1` major `1`, minor `0` exactly. A change
to either inner number requires a new outer schema version and a different
physical locator. Representation changes do not change `ArtifactKey`.
Decoder or `DecodedMosaic` semantic changes invalidate through the recipe's
dependency digest/revisions while the V1 identity transcripts remain intact.

## Complete object

```text
136-byte Object Envelope V1 header
descriptor_bytes bytes of semantic/representation descriptor
payload_bytes bytes of schema-owned payload
```

For Mosaic Payload V1 the outer descriptor is exactly the 106-byte
`MosaicArtifactDescriptorV1` from `CACHE_IDENTITY_SPEC.md`, with no suffix. The
outer payload is the complete Mosaic Payload V1 stream, including its 192-byte
metadata prefix. These two descriptors are different layers and MUST NOT be
interchanged.

### Object Envelope V1 header

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | ASCII `RRRAHOBJ` |
| 8 | 2 | envelope version, `1` |
| 10 | 2 | header bytes, `136` |
| 12 | 4 | flags, zero in V1 |
| 16 | 4 | payload schema ID |
| 20 | 2 | payload schema version |
| 22 | 2 | reserved zero |
| 24 | 4 | descriptor byte count |
| 28 | 4 | reserved zero |
| 32 | 8 | payload byte count |
| 40 | 32 | `ArtifactKey` |
| 72 | 32 | payload digest |
| 104 | 32 | envelope digest |

Payload schema ID zero and version zero are permanently reserved and invalid.

Hard limits are `descriptor_bytes <= 65,536` and
`payload_bytes <= 1,073,741,824`. The file length MUST equal
`136 + descriptor_bytes + payload_bytes`, using checked `u64` arithmetic.
Trailing bytes are invalid.

```text
payload_digest = BLAKE3-DERIVE(
    "rrrah.cache.object-payload.v1",
    exact stored payload bytes
)

envelope_digest = BLAKE3-DERIVE(
    "rrrah.cache.object-envelope.v1",
    header[0..104] || exact descriptor bytes
)
```

The digest is corruption detection, not authentication. A process able to
write the cache can recompute it.

Readers first consume exactly 12 bytes. Unknown envelope versions are a typed
unsupported result and MUST NOT be forced through a 136-byte V1 read. A known
V1 reader then validates flags, reserved bytes, caps, checked total length,
expected `(schema, ArtifactKey)`, the envelope digest over
`header[0..104] || descriptor`, and the payload digest.

`PayloadDigestWriter::try_finish` calls `Write::flush` and returns the byte count
and digest only after that flush succeeds. This completes the generic writer's
userspace buffering contract; it is not an `fsync` or durable-publication
guarantee. Filesystem durability belongs to the object-store epic, which must
separately define file synchronization, atomic rename/publication, parent
directory synchronization and crash recovery. `finish` deliberately performs
no flush and is intended for in-memory or already-finalized sinks.

## Mosaic Payload V1

The schema is canonical, uncompressed, tightly packed row-major U16LE. It is a
latency baseline; tiled or compressed forms receive another payload schema.

### Fixed metadata prefix

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | `RRMPAY1\0` |
| 8 | 2 | major `1` |
| 10 | 2 | minor `0` |
| 12 | 4 | fixed header bytes `192` |
| 16 | 4 | complete metadata-prefix byte count |
| 20 | 4 | presence flags |
| 24 | 8 | reserved zero |
| 32 | 4 | sensor width |
| 36 | 4 | sensor height |
| 40 | 8 | sample count |
| 48 | 8 | pixel byte count |
| 56 | 4 | make UTF-8 byte count |
| 60 | 4 | model UTF-8 byte count |
| 64 | 4 | CFA cell count |
| 68 | 4 | black-level value count |
| 72 | 4 | white-level value count |
| 76 | 2 | sample format `1` = U16LE interleaved |
| 78 | 1 | photometric code |
| 79 | 1 | orientation code |
| 80 | 1 | components per pixel |
| 81 | 1 | bits per sample, `1..=16` |
| 82 | 1 | CFA repeat width |
| 83 | 1 | CFA repeat height |
| 84 | 1 | black grid width |
| 85 | 1 | black grid height |
| 86 | 1 | black grid components |
| 87 | 9 | reserved zero |
| 96 | 16 | active rectangle: `x,y,width,height` as four U32 |
| 112 | 16 | crop rectangle: `x,y,width,height` as four U32 |
| 128 | 16 | WB `[R,G,B,G2]`, four binary32 values |
| 144 | 48 | row-major `XYZ -> camera`, `[camera][XYZ]`, 4x3 binary32 |

Presence flags are bit 0 CFA, bit 1 active rectangle and bit 2 crop rectangle.
All other bits are zero. Absent fields use canonical zero dimensions/counts or
rectangle bytes. A present empty rectangle remains distinct from absence.
Width, height and components-per-pixel are nonzero. A CFA flag requires
`cfa_width * cfa_height == C != 0`; without it, both dimensions and `C` are
zero. `Photometric::Cfa` additionally requires one component and a CFA.
`black_width * black_height * black_components == K != 0`, and `W != 0`.
Present rectangles use checked `x + width <= sensor_width` and
`y + height <= sensor_height`; crop need not be contained by active area.

Immediately after byte 192, variable metadata appears without padding:

```text
make UTF-8 bytes
model UTF-8 bytes
CFA cell codes
black-level binary32 values
white-level binary32 values
U16LE pixels
```

No Unicode normalization, trimming or NUL termination occurs. Embedded NUL
and empty strings are valid. Invalid UTF-8 is rejected.

### Registries

Photometric: `1=Cfa`, `2=LinearRaw`, `3=BlackIsZero`.

Orientation follows TIFF: `0=Unknown`, `1=Normal`, `2=HorizontalFlip`,
`3=Rotate180`, `4=VerticalFlip`, `5=Transpose`, `6=Rotate90`,
`7=Transverse`, `8=Rotate270`.

CFA colors: `0=Red`, `1=Green`, `2=Blue`, `3=Cyan`, `4=Magenta`,
`5=Yellow`, `6=White`, `255=Unknown`.

Every unallocated code is invalid. Registry values are explicit and do not
derive from Rust discriminants.

### Equations and limits

Let `A,B,C,K,W` be make bytes, model bytes, CFA cells, black values and white
values. Let `S = width * height * components_per_pixel`.

```text
metadata_prefix_bytes = 192 + A + B + C + 4K + 4W
pixel_bytes            = 2S
payload_bytes          = metadata_prefix_bytes + pixel_bytes
```

All operations use checked arithmetic before allocation. `A,B <= 65,536`,
`C,K,W <= 65,536`, `metadata_prefix_bytes <= 1 MiB`, and
`S <= 250,000,000`. Per-operation limits may be lower and are resource
rejections, not corruption. The writer and reader use the same validated
layout calculation.

`sample_count` MUST equal `width * height * components_per_pixel`, and
`pixel_bytes` MUST equal `2 * sample_count`. Samples may use any U16 value even
when `bits_per_sample < 16`; V1 does not mask or reject unused high bits.
White-level vectors may contain any `1..=65,536` values and are not inferred
from CFA size or component count.

Pixels remain in sensor coordinates and use index
`((y * width + x) * components_per_pixel + component)`. There is no row
padding. Repeating-grid indices are exactly:

```text
cfa_index = (row % cfa_height) * cfa_width + (column % cfa_width)
black_index =
  (((row % black_height) * black_width + (column % black_width))
   * black_components + component)
```

The component is the fastest-changing index inside a black-grid cell.
Orientation is display metadata.

All floats are IEEE-754 binary32 LE and finite. White levels are additionally
strictly positive. Writers canonicalize `-0` to `+0`; readers reject the
non-canonical negative-zero bit pattern. Subnormal finite values are retained.

Readers validate all bounded metadata and `RawMetadata` invariants before
allocating or reading pixels.

## Physical namespace and migration

The intended V3 namespace is disjoint from frozen V2:

```text
mosaics/                                      # legacy V2 only
mosaic-objects-v3/objects/eEEEE/sIIIIIIII-vVVVV/HH/
  KKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKKK.rro
```

All locator fields are fixed-width lowercase hexadecimal: `EEEE` is the
nonzero U16 envelope version, `IIIIIIII` the nonzero U32 schema ID, `VVVV` the
nonzero U16 schema version, `HH` the first key byte, and `K...` exactly 64 hex
characters for the complete key. No uppercase, alternate width, repeated or
trailing separator, whitespace, backslash or non-ASCII spelling is canonical.

The current Rust `ObjectLocator` is scoped to the Envelope V1 store and holds
only `(PayloadSchema, ArtifactKey)`; the store contributes `e0001`. The path
formatter/parser and filesystem store are not implemented by this wire-format
epic. A conforming store MUST build paths only from typed values and verify
schema, key, shard and filename against the opened file.

V2 uses sampled identity and a native-width frame field. A V2 object MUST NOT
be copied, relabelled or re-encoded into V3. V3 publication requires a fresh
decode associated with a stable full-content `SourceId` snapshot.

## Canonical conformance object

The unit conformance object is 465 bytes:

```text
136 envelope + 106 semantic descriptor + 223 mosaic payload
BLAKE3(object) = 792767cd35eeef24511c390f6c19cfa6c8e93027b00d87e711c5b1e085ff51c5
```

It uses source bytes `00 01 02 ff`, the identity conformance manifest
`(backend=1, backend_rev=1, adapter_rev=1, model_rev=1, flags=0x1f,
dependency_digest=5a repeated 32 times)` and image index zero. Its ArtifactKey
is `c47466531caafd8f3926077f8f68c4ed0080a5508db791c28891b38b57ecd46e`.

The mosaic is make/model `Canon`/`Golden`, 2x2, cpp 1, 14-bit, CFA, Normal,
with CFA 2x2 `[R,G,G,B]`, black 1x1x1 `[512]`, white `[16383]`, WB
`[2,1,1.5,1]`, identity XYZ-to-camera in the first three rows and a zero fourth
row, active rectangle `(0,0,2,2)`, absent crop and pixels
`[0,512,8192,16383]`. The exact object is
`crates/rrrah-cache/tests/fixtures/v3_mosaic_v1_object.hex`.

`BLAKE3(object)` above is an unkeyed diagnostic fixture fingerprint, not a
wire digest or derive-key context. The committed V2 fixture is separate and
exists only to prevent accidental legacy reader/writer drift.
Both objects have sidecar manifests with decoded-byte SHA-256 section hashes.
`scripts/v3_conformance.py` independently reconstructs/parses the layouts with
Python `struct` and compares them byte-for-byte without importing Rust code.

## Release gates

These are pre-release requirements, not claims that the V3 filesystem store is
already complete:

- Rust writer byte-for-byte canonical re-encode and complete-object golden.
- Independent language-neutral fixture generator/parser.
- Native, 32-bit little-endian and 64-bit big-endian conformance.
- Single-field mutation and truncation at every structural boundary.
- Small-limit fuzz entrypoint proving no panic, unbounded read or allocation.
- Cross-layer validation that descriptor-derived key, header key and locator
  key are identical before payload decode.
- One-pass payload transfer, byte count and digest; decoded data is not
  published before final checksum verification.
