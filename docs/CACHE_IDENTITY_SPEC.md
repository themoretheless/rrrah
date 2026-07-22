# Cache identity protocol

Status: V1 normative draft. The terms MUST, MUST NOT, SHOULD and MAY are used in the RFC 2119 sense.

This document specifies semantic identity only. Cache containers, compression, checksums, catalogs, locks, filesystem layout and eviction have independent versions.

The corresponding V3 object envelope and monolithic mosaic representation are
specified in [CACHE_OBJECT_V3_SPEC.md](CACHE_OBJECT_V3_SPEC.md).

## Invariants and ownership

- `rrrah-core` owns the meaning and canonical encoding of `MosaicRecipeManifest`.
- `rrrah-decode` owns the manifest declared by each decoder adapter.
- `rrrah-cache::key` owns BLAKE3 contexts, typed IDs and the artifact transcript.
- The future stable-source resolver owns `SourceId` provenance.
- A disk object store MUST independently validate its header, complete descriptor and payload checksum. `ArtifactKey` is neither a payload digest nor a MAC.

The running V2 cache still uses a sampled fingerprint. V2 data MUST NOT be re-labelled, promoted or written under a V3 identity. V3 integration is incomplete until one stable opened source is used for full hashing and decoding.

## Registry

Code zero is permanently invalid/reserved. Allocated codes MUST NOT be reused with another meaning.

| Registry | Code | Meaning |
|---|---:|---|
| artifact kind | 1 | full decoded sensor mosaic |
| decoder backend | 1 | Rawler family |

A compatible Rawler update retains backend ID 1 and changes its dependency-closure digest and backend contract revision. A semantically independent fork receives another backend ID. An unknown non-zero backend is structurally parseable, but MUST remain untrusted; dispatch and cache admission compare the complete registered recipe ID, never the backend ID alone.

Decode flags are a `u32` little-endian bitset:

| Bit | Name | Required V1 meaning |
|---:|---|---|
| 0 | `FULL_SENSOR_RAW` | decode sensor data, never embedded preview/dummy output |
| 1 | `INTEGER_U16` | persisted samples are unsigned 16-bit integers |
| 2 | `SENSOR_COORDINATES` | samples remain in sensor row/column order |
| 3 | `CROP_AS_METADATA` | active/crop rectangles do not destructively crop samples |
| 4 | `IMAGE_INDEX_IN_KEY` | frame selection is the artifact transcript's `u64` input |

All five bits are required for a V1 sensor mosaic. Bits 5–31 are reserved and MUST be zero. Adding a known bit does not automatically make a producer claim it; every adapter declares an explicit producer mask.

## SourceId V1

```text
SourceId = BLAKE3-DERIVE("rrrah.cache.source-id.v1", every source byte)
```

The resolver MUST hash every byte from one opened source snapshot, in order, with bounded memory. It MUST use that same snapshot for decode, check identity/stability before and after the operation, and reject cache publication if the source changes. A sampled hash, path, mtime or file token MUST NOT be wrapped as `SourceId`; tokens may only memoize a previously verified full-content ID.

Hashing is streaming and chunk boundaries do not affect the result. Cancellation SHOULD be checked between chunks. The low-level hasher remains crate-private until the stable resolver enforces this contract.

Golden vectors:

| Input bytes | SourceId |
|---|---|
| empty | `ac86ac94caaa88afca2ac28e18aff1927fd9fe727c6c1cf6f5b68668eee971c2` |
| `00 01 02 ff` | `7b8792c318d96b870db65b324c3997769cc773ebbd4e5fb5d620a3a21ce0996b` |

## MosaicRecipeManifest V1

The manifest is exactly 64 bytes. Integers are unsigned little-endian. Rust struct layout and serde output are not protocol encodings.

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | magic `52 4d 52 00` (`RMR\0`) |
| 4 | 2 | manifest version, `1` |
| 6 | 2 | artifact kind, `1` |
| 8 | 4 | decoder backend ID |
| 12 | 4 | backend contract revision |
| 16 | 4 | rrrah adapter contract revision |
| 20 | 4 | `DecodedMosaic` model revision |
| 24 | 4 | explicit producer decode flags |
| 28 | 32 | SHA-256 of the resolved producer dependency closure and enabled features |
| 60 | 4 | reserved zero bytes |

Backend ID, all three revisions and the dependency digest MUST be non-zero. All required flags MUST be present; unknown flags and non-zero reserved bytes are invalid. External framing MUST reject input whose length is not exactly 64 bytes, including trailing bytes.

The dependency digest is computed by `scripts/recipe_lock.py` from the Rawler node and its complete resolved Cargo dependency graph. A digest change fails CI and requires the RAW corpus, an explicit backend revision decision and an updated manifest constant.

```text
MosaicRecipeId = BLAKE3-DERIVE(
    "rrrah.cache.recipe-id.v1",
    exact MosaicRecipeManifest V1 bytes
)
```

For the conformance manifest `(backend=1, backend_rev=1, adapter_rev=1, model_rev=1, flags=0x1f, dependency_digest=5a repeated 32 times)`, the recipe ID is:

```text
932219a5b11ca7978d9e4857d92b807a155932d789d60dc5ae770a6dfb8cbf4f
```

## ArtifactKey transcript V1

The transcript is exactly 106 bytes and is only a canonical hash input, not a disk container.

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 2 | artifact kind `1`, `u16` LE |
| 2 | 32 | `SourceId` |
| 34 | 8 | image index, `u64` LE |
| 42 | 32 | variant; all-zero is canonical `NoVariant` for a whole mosaic |
| 74 | 32 | `MosaicRecipeId` |

```text
ArtifactKey = BLAKE3-DERIVE(
    "rrrah.cache.artifact-key.v1",
    exact 106-byte transcript
)
```

Using SourceId `7b8792...996b` and the conformance recipe above:

| Image index | ArtifactKey |
|---:|---|
| 0 | `c47466531caafd8f3926077f8f68c4ed0080a5508db791c28891b38b57ecd46e` |
| 1 | `d87c45fbd12477c9cd9ccc1bf53671e08bca082ca66b949a151234207cafd9e7` |
| `u64::MAX` | `8862806b5991bb261fe8f1f4df4b0adc03c69ebb6913eeebbd3e6654836424d5` |

Binary IDs are opaque 32-byte values. Canonical filename text is exactly 64 lowercase ASCII hex bytes. A filename parser MUST consume bytes before joining a path, reject non-ASCII/invalid UTF-8, separators, uppercase, whitespace, short/trailing input and then return a typed key.

## Version and invalidation rules

| Change | Required identity change |
|---|---|
| RAW content | `SourceId` changes |
| image/frame | `image_index` changes |
| Rawler algorithm, camera data, features or semantic dependency closure | dependency digest and backend revision |
| CFA/levels/WB/matrix/crop/orientation adaptation | adapter revision |
| `DecodedMosaic` field meaning, sample layout or invariants | model revision |
| declared producer guarantee | explicit flags and appropriate revision |
| manifest field layout/meaning | new manifest version and recipe context |
| source hash protocol | new source context |
| artifact transcript layout/hash | new artifact context |
| cache container, compression, checksum, catalog, path layout | no semantic identity change |
| exposure, tone map, GPU upload, zoom, prefetch, telemetry | no mosaic identity change |

Published V1 meanings are immutable. Readers return typed unsupported/corrupt outcomes; they MUST NOT guess endian, clear reserved data, reinterpret an unknown version, or fall back under the same key.

## Conformance gates

- Golden manifest/recipe/transcript/artifact vectors on 32- and 64-bit, little- and big-endian targets.
- All structured input bits affect the derived key.
- One-shot and every streaming split produce the same `SourceId`.
- Missing required flags, unknown flags, zero required fields, unknown kind/version and non-zero reserved bytes are rejected.
- The dependency-closure gate and decoder manifest constant match.
- V2 and V3 roots, parsers, key types and pruning domains are disjoint.
- A backend is admitted only after corpus-backed pixel and persisted-metadata conformance; a manifest is identity metadata, not authentication.
