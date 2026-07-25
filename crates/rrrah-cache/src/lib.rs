//! Byte-accounted memory caching and an atomic decoded-mosaic disk cache.
#![allow(
    clippy::missing_errors_doc,
    clippy::format_collect,
    clippy::cast_possible_truncation
)]

mod container_v1;
mod disk;
mod key;
mod mosaic_object_v1;
mod mosaic_payload_v1;
mod ram;
mod weighted_lru;

#[cfg(feature = "bench-internals")]
#[doc(hidden)]
pub mod bench_support;

pub use container_v1::{
    ContainerError, ContainerHeaderV1, MAX_OBJECT_DESCRIPTOR_BYTES, MAX_OBJECT_PAYLOAD_BYTES,
    OBJECT_HEADER_V1_BYTES, ObjectLocator, ObjectPayloadHasher, PayloadDigest, PayloadDigestReader,
    PayloadDigestWriter, PayloadSchema, PayloadSchemaError, object_payload_digest,
};
pub use disk::{
    CacheError, CacheKey, CacheLoad, DEFAULT_MAX_DISK_CACHE_BYTES, DiskCacheUsage, DiskMosaicCache,
    SourceFingerprint,
};
pub use key::{
    ArtifactKey, MOSAIC_ARTIFACT_DESCRIPTOR_V1_BYTES, MosaicArtifactDescriptorV1, MosaicDescriptorError,
    MosaicKey, MosaicRecipeId, SourceId,
};
pub use mosaic_object_v1::{MosaicObjectError, PreparedMosaicObjectV1, validate_mosaic_object_descriptor_v1};
pub use mosaic_payload_v1::{
    MAX_MOSAIC_DESCRIPTOR_BYTES, MAX_MOSAIC_SAMPLES, MOSAIC_PAYLOAD_HEADER_V1_BYTES,
    MOSAIC_PAYLOAD_SCHEMA_ID, MOSAIC_PAYLOAD_SCHEMA_VERSION_V1, MosaicDecodeLimits, MosaicPayloadError,
    MosaicPayloadStatsV1, PreparedMosaicPayloadV1, decode_mosaic_payload_v1,
    decode_mosaic_payload_v1_with_limits, encode_mosaic_payload_v1, mosaic_payload_schema_v1,
    prepare_mosaic_payload_v1,
};
pub use ram::{DEFAULT_RAM_CACHE_BYTES, MosaicRamCache};
pub use weighted_lru::WeightedLru;

// Cross-layer wire caps are part of one composition contract. Keep these as
// compile-time proofs so independently bumping a schema cannot make its valid
// payload unrepresentable by the object container or a 32-bit file offset.
const _: () = assert!(MOSAIC_ARTIFACT_DESCRIPTOR_V1_BYTES <= MAX_OBJECT_DESCRIPTOR_BYTES as usize);
const _: () = assert!(MAX_MOSAIC_DESCRIPTOR_BYTES + 2 * MAX_MOSAIC_SAMPLES <= MAX_OBJECT_PAYLOAD_BYTES);
const _: () = assert!(
    OBJECT_HEADER_V1_BYTES as u64 + MAX_OBJECT_DESCRIPTOR_BYTES as u64 + MAX_OBJECT_PAYLOAD_BYTES
        <= i32::MAX as u64
);

#[cfg(test)]
mod v3_contract_tests {
    use std::sync::Arc;

    use rrrah_core::{
        CfaColor, CfaPattern, DecodedMosaic, KNOWN_MOSAIC_DECODE_FLAGS, LevelGrid, MosaicRecipeManifest,
        Orientation, Photometric, RawMetadata, Rect, WhiteLevel,
    };

    use super::*;
    use crate::key::SourceIdHasher;

    #[test]
    fn complete_v3_object_has_one_canonical_cross_layer_layout() {
        let mut source_hasher = SourceIdHasher::new();
        source_hasher.update(&[0x00, 0x01, 0x02, 0xff]);
        let source = source_hasher.finalize();
        let manifest = MosaicRecipeManifest::new(1, 1, 1, 1, KNOWN_MOSAIC_DECODE_FLAGS, [0x5a; 32]);
        let mosaic_key = MosaicKey::new(source, 0, MosaicRecipeId::from_manifest(manifest));

        let mosaic = DecodedMosaic::new(
            RawMetadata {
                make: "Canon".into(),
                model: "Golden".into(),
                width: 2,
                height: 2,
                components_per_pixel: 1,
                bits_per_sample: 14,
                photometric: Photometric::Cfa,
                cfa: Some(CfaPattern {
                    width: 2,
                    height: 2,
                    cells: vec![CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue],
                }),
                black_level: LevelGrid {
                    width: 1,
                    height: 1,
                    components: 1,
                    values: vec![512.0],
                },
                white_level: WhiteLevel(vec![16_383.0]),
                white_balance: [2.0, 1.0, 1.5, 1.0],
                xyz_to_camera: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]],
                active_area: Some(Rect::new(0, 0, 2, 2)),
                crop_area: None,
                orientation: Orientation::Normal,
            },
            Arc::new(vec![0, 512, 8192, 16_383]),
        )
        .unwrap();
        let prepared = PreparedMosaicObjectV1::new(mosaic_key, &mosaic).unwrap();
        let descriptor_bytes = prepared.descriptor_bytes();
        let locator = prepared.locator();
        let mut payload_writer = PayloadDigestWriter::new(Vec::new());
        prepared.encode_payload(&mut payload_writer).unwrap();
        let (payload, payload_bytes, payload_digest) = payload_writer.try_finish().unwrap();
        assert_eq!(payload_bytes, prepared.payload_stats().payload_bytes);
        let header = prepared.container_header(payload_digest).unwrap();

        let mut object = Vec::new();
        object.extend_from_slice(&header.encode());
        object.extend_from_slice(&descriptor_bytes);
        object.extend_from_slice(&payload);
        assert_eq!(object.len(), 465);
        assert_eq!(&object[..8], b"RRRAHOBJ");
        assert_eq!(&object[136..138], &[1, 0]);

        let header_bytes: [u8; OBJECT_HEADER_V1_BYTES] = object[..OBJECT_HEADER_V1_BYTES].try_into().unwrap();
        let parsed = ContainerHeaderV1::parse_header(&header_bytes, object.len() as u64, locator).unwrap();
        let decoded_descriptor = validate_mosaic_object_descriptor_v1(parsed, &object[136..242]).unwrap();
        assert_eq!(decoded_descriptor.mosaic_key(), mosaic_key);
        assert_eq!(decoded_descriptor.artifact_key(), parsed.artifact_key());
        let mut payload_reader = PayloadDigestReader::new(&object[242..]);
        let decoded = decode_mosaic_payload_v1(&mut payload_reader, parsed.payload_bytes()).unwrap();
        let (_, payload_bytes, payload_digest) = payload_reader.finish();
        parsed
            .verify_payload_digest(payload_bytes, payload_digest)
            .unwrap();
        assert_eq!(decoded.metadata, mosaic.metadata);
        assert_eq!(&*decoded.pixels, &*mosaic.pixels);

        // This is an unkeyed diagnostic hash of the complete fixture, not one
        // of the two protocol BLAKE3-DERIVE integrity fields.
        assert_eq!(
            blake3::hash(&object).to_hex().as_str(),
            "792767cd35eeef24511c390f6c19cfa6c8e93027b00d87e711c5b1e085ff51c5"
        );
        let fixture_hex = include_str!("../tests/fixtures/v3_mosaic_v1_object.hex")
            .trim()
            .as_bytes();
        assert_eq!(fixture_hex.len() % 2, 0);
        let fixture = fixture_hex
            .chunks_exact(2)
            .map(|pair| {
                let nibble = |byte| match byte {
                    b'0'..=b'9' => byte - b'0',
                    b'a'..=b'f' => byte - b'a' + 10,
                    _ => panic!("invalid checked-in V3 fixture hex"),
                };
                (nibble(pair[0]) << 4) | nibble(pair[1])
            })
            .collect::<Vec<_>>();
        assert_eq!(object, fixture);
    }
}
