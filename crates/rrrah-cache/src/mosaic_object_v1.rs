use std::io::Write;

use rrrah_core::DecodedMosaic;
use thiserror::Error;

use crate::{
    ContainerError, ContainerHeaderV1, MOSAIC_ARTIFACT_DESCRIPTOR_V1_BYTES, MosaicArtifactDescriptorV1,
    MosaicDescriptorError, MosaicKey, MosaicPayloadError, MosaicPayloadStatsV1, ObjectLocator, PayloadDigest,
    PreparedMosaicPayloadV1, mosaic_payload_schema_v1, prepare_mosaic_payload_v1,
};

/// A semantic descriptor and a payload plan bound to the same mosaic value.
#[derive(Debug)]
pub struct PreparedMosaicObjectV1<'a> {
    descriptor: MosaicArtifactDescriptorV1,
    payload: PreparedMosaicPayloadV1<'a>,
}

impl<'a> PreparedMosaicObjectV1<'a> {
    pub fn new(key: MosaicKey, mosaic: &'a DecodedMosaic) -> Result<Self, MosaicPayloadError> {
        Ok(Self {
            descriptor: MosaicArtifactDescriptorV1::new(key),
            payload: prepare_mosaic_payload_v1(mosaic)?,
        })
    }

    pub fn locator(&self) -> ObjectLocator {
        ObjectLocator::new(mosaic_payload_schema_v1(), self.descriptor.artifact_key())
    }

    pub fn descriptor_bytes(&self) -> [u8; MOSAIC_ARTIFACT_DESCRIPTOR_V1_BYTES] {
        self.descriptor.encode()
    }

    pub const fn payload_stats(&self) -> MosaicPayloadStatsV1 {
        self.payload.stats()
    }

    pub fn encode_payload(&self, writer: &mut impl Write) -> Result<(), MosaicPayloadError> {
        self.payload.encode(writer)
    }

    pub fn container_header(
        &self,
        payload_digest: PayloadDigest,
    ) -> Result<ContainerHeaderV1, ContainerError> {
        ContainerHeaderV1::new(
            self.locator(),
            &self.descriptor_bytes(),
            self.payload_stats().payload_bytes,
            payload_digest,
        )
    }
}

/// Validates all cross-layer descriptor bindings after the generic envelope
/// header has been parsed for its expected physical locator.
pub fn validate_mosaic_object_descriptor_v1(
    header: ContainerHeaderV1,
    descriptor: &[u8],
) -> Result<MosaicArtifactDescriptorV1, MosaicObjectError> {
    if header.schema() != mosaic_payload_schema_v1() {
        return Err(MosaicObjectError::UnsupportedPayloadSchema {
            id: header.schema().id(),
            version: header.schema().version(),
        });
    }
    let descriptor_bytes: [u8; MOSAIC_ARTIFACT_DESCRIPTOR_V1_BYTES] =
        descriptor
            .try_into()
            .map_err(|_| MosaicObjectError::InvalidDescriptorLength {
                actual: descriptor.len(),
            })?;
    header.verify_descriptor(descriptor)?;
    let semantic = MosaicArtifactDescriptorV1::decode(descriptor_bytes)?;
    if semantic.artifact_key() != header.artifact_key() {
        return Err(MosaicObjectError::DescriptorArtifactKeyMismatch);
    }
    Ok(semantic)
}

#[derive(Debug, Error)]
pub enum MosaicObjectError {
    #[error("mosaic object uses unsupported payload schema {id}/{version}")]
    UnsupportedPayloadSchema { id: u32, version: u16 },
    #[error("mosaic object descriptor must be exactly 106 bytes, got {actual}")]
    InvalidDescriptorLength { actual: usize },
    #[error("mosaic semantic descriptor derives a different artifact key than its envelope")]
    DescriptorArtifactKeyMismatch,
    #[error("invalid object envelope: {0}")]
    Container(#[from] ContainerError),
    #[error("invalid mosaic semantic descriptor: {0}")]
    Descriptor(#[from] MosaicDescriptorError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MosaicRecipeId, ObjectPayloadHasher, SourceId};
    use rrrah_core::{
        CfaColor, CfaPattern, KNOWN_MOSAIC_DECODE_FLAGS, LevelGrid, MosaicRecipeManifest, Orientation,
        Photometric, RawMetadata, WhiteLevel,
    };
    use std::sync::Arc;

    fn key(tag: u8) -> MosaicKey {
        let recipe = MosaicRecipeId::from_manifest(MosaicRecipeManifest::new(
            1,
            1,
            1,
            1,
            KNOWN_MOSAIC_DECODE_FLAGS,
            [tag.max(1); 32],
        ));
        MosaicKey::new(SourceId::from_bytes([tag; 32]), 0, recipe)
    }

    fn mosaic() -> DecodedMosaic {
        DecodedMosaic::new(
            RawMetadata {
                make: String::new(),
                model: String::new(),
                width: 1,
                height: 1,
                components_per_pixel: 1,
                bits_per_sample: 16,
                photometric: Photometric::Cfa,
                cfa: Some(CfaPattern {
                    width: 1,
                    height: 1,
                    cells: vec![CfaColor::Red],
                }),
                black_level: LevelGrid {
                    width: 1,
                    height: 1,
                    components: 1,
                    values: vec![0.0],
                },
                white_level: WhiteLevel(vec![65_535.0]),
                white_balance: [1.0; 4],
                xyz_to_camera: [[0.0; 3]; 4],
                active_area: None,
                crop_area: None,
                orientation: Orientation::Normal,
            },
            Arc::new(vec![7]),
        )
        .unwrap()
    }

    #[test]
    fn semantic_descriptor_is_bound_to_header_key() {
        let value = mosaic();
        let first = PreparedMosaicObjectV1::new(key(1), &value).unwrap();
        let other = PreparedMosaicObjectV1::new(key(2), &value).unwrap();
        let payload_digest = ObjectPayloadHasher::new().finalize();
        let header =
            ContainerHeaderV1::new(first.locator(), &other.descriptor_bytes(), 0, payload_digest).unwrap();
        assert!(matches!(
            validate_mosaic_object_descriptor_v1(header, &other.descriptor_bytes()),
            Err(MosaicObjectError::DescriptorArtifactKeyMismatch)
        ));
    }
}
