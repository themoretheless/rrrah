use thiserror::Error;

pub const MOSAIC_RECIPE_MANIFEST_BYTES: usize = 64;
pub const MOSAIC_RECIPE_MANIFEST_VERSION_V1: u16 = 1;
pub const SENSOR_MOSAIC_ARTIFACT_KIND_CODE: u16 = 1;

pub const DECODE_FULL_SENSOR_RAW: u32 = 1 << 0;
pub const DECODE_INTEGER_U16: u32 = 1 << 1;
pub const DECODE_SENSOR_COORDINATES: u32 = 1 << 2;
pub const DECODE_CROP_AS_METADATA: u32 = 1 << 3;
pub const DECODE_IMAGE_INDEX_IN_KEY: u32 = 1 << 4;
pub const REQUIRED_SENSOR_MOSAIC_DECODE_FLAGS: u32 = DECODE_FULL_SENSOR_RAW
    | DECODE_INTEGER_U16
    | DECODE_SENSOR_COORDINATES
    | DECODE_CROP_AS_METADATA
    | DECODE_IMAGE_INDEX_IN_KEY;
pub const KNOWN_MOSAIC_DECODE_FLAGS: u32 = REQUIRED_SENSOR_MOSAIC_DECODE_FLAGS;

const MANIFEST_MAGIC: [u8; 4] = *b"RMR\0";

/// Language-neutral description of the semantic contract that produced a
/// decoded sensor mosaic.
///
/// The Rust layout is not a wire format. `canonical_bytes` explicitly writes
/// the fixed 64-byte V1 transcript used by cache recipe IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MosaicRecipeManifest {
    decoder_backend_id: u32,
    backend_contract_revision: u32,
    adapter_contract_revision: u32,
    mosaic_model_revision: u32,
    decode_flags: u32,
    producer_dependency_closure_sha256: [u8; 32],
}

impl MosaicRecipeManifest {
    /// Builds a trusted producer manifest.
    ///
    /// # Panics
    ///
    /// Panics for zero IDs/revisions/digest or flags outside the V1 registry.
    /// Use `from_canonical_bytes` for untrusted persisted input.
    pub const fn new(
        decoder_backend_id: u32,
        backend_contract_revision: u32,
        adapter_contract_revision: u32,
        mosaic_model_revision: u32,
        decode_flags: u32,
        producer_dependency_closure_sha256: [u8; 32],
    ) -> Self {
        assert!(decoder_backend_id != 0, "decoder backend ID must be non-zero");
        assert!(
            backend_contract_revision != 0,
            "backend revision must be non-zero"
        );
        assert!(
            adapter_contract_revision != 0,
            "adapter revision must be non-zero"
        );
        assert!(
            mosaic_model_revision != 0,
            "mosaic model revision must be non-zero"
        );
        assert!(
            decode_flags & !KNOWN_MOSAIC_DECODE_FLAGS == 0,
            "unknown decode flags"
        );
        assert!(
            decode_flags & REQUIRED_SENSOR_MOSAIC_DECODE_FLAGS == REQUIRED_SENSOR_MOSAIC_DECODE_FLAGS,
            "required decode flags are missing"
        );
        assert!(
            !all_zero(&producer_dependency_closure_sha256),
            "producer dependency-closure digest must be non-zero"
        );
        Self {
            decoder_backend_id,
            backend_contract_revision,
            adapter_contract_revision,
            mosaic_model_revision,
            decode_flags,
            producer_dependency_closure_sha256,
        }
    }

    pub const fn decoder_backend_id(self) -> u32 {
        self.decoder_backend_id
    }

    pub const fn canonical_bytes(self) -> [u8; MOSAIC_RECIPE_MANIFEST_BYTES] {
        let mut bytes = [0_u8; MOSAIC_RECIPE_MANIFEST_BYTES];
        copy_array(&mut bytes, 0, &MANIFEST_MAGIC);
        copy_array(&mut bytes, 4, &MOSAIC_RECIPE_MANIFEST_VERSION_V1.to_le_bytes());
        copy_array(&mut bytes, 6, &SENSOR_MOSAIC_ARTIFACT_KIND_CODE.to_le_bytes());
        copy_array(&mut bytes, 8, &self.decoder_backend_id.to_le_bytes());
        copy_array(&mut bytes, 12, &self.backend_contract_revision.to_le_bytes());
        copy_array(&mut bytes, 16, &self.adapter_contract_revision.to_le_bytes());
        copy_array(&mut bytes, 20, &self.mosaic_model_revision.to_le_bytes());
        copy_array(&mut bytes, 24, &self.decode_flags.to_le_bytes());
        copy_array(&mut bytes, 28, &self.producer_dependency_closure_sha256);
        // 60..64 are reserved and remain zero.
        bytes
    }

    pub fn from_canonical_bytes(bytes: [u8; MOSAIC_RECIPE_MANIFEST_BYTES]) -> Result<Self, ManifestError> {
        if bytes[..4] != MANIFEST_MAGIC {
            return Err(ManifestError::InvalidMagic);
        }
        let version = read_u16(&bytes, 4);
        if version != MOSAIC_RECIPE_MANIFEST_VERSION_V1 {
            return Err(ManifestError::UnsupportedVersion(version));
        }
        let kind = read_u16(&bytes, 6);
        if kind != SENSOR_MOSAIC_ARTIFACT_KIND_CODE {
            return Err(ManifestError::UnsupportedArtifactKind(kind));
        }
        let decode_flags = read_u32(&bytes, 24);
        if decode_flags & !KNOWN_MOSAIC_DECODE_FLAGS != 0 {
            return Err(ManifestError::UnknownDecodeFlags(decode_flags));
        }
        if decode_flags & REQUIRED_SENSOR_MOSAIC_DECODE_FLAGS != REQUIRED_SENSOR_MOSAIC_DECODE_FLAGS {
            return Err(ManifestError::MissingRequiredDecodeFlags(decode_flags));
        }
        if bytes[60..].iter().any(|byte| *byte != 0) {
            return Err(ManifestError::NonZeroReserved);
        }
        let mut producer_dependency_closure_sha256 = [0_u8; 32];
        producer_dependency_closure_sha256.copy_from_slice(&bytes[28..60]);
        let fields = [
            read_u32(&bytes, 8),
            read_u32(&bytes, 12),
            read_u32(&bytes, 16),
            read_u32(&bytes, 20),
        ];
        if fields.contains(&0) {
            return Err(ManifestError::ZeroRequiredField);
        }
        if all_zero(&producer_dependency_closure_sha256) {
            return Err(ManifestError::ZeroProducerDependencyDigest);
        }
        Ok(Self {
            decoder_backend_id: fields[0],
            backend_contract_revision: fields[1],
            adapter_contract_revision: fields[2],
            mosaic_model_revision: fields[3],
            decode_flags,
            producer_dependency_closure_sha256,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ManifestError {
    #[error("invalid mosaic recipe manifest magic")]
    InvalidMagic,
    #[error("unsupported mosaic recipe manifest version {0}")]
    UnsupportedVersion(u16),
    #[error("unsupported recipe artifact kind {0}")]
    UnsupportedArtifactKind(u16),
    #[error("mosaic recipe manifest has unknown decode flags 0x{0:08x}")]
    UnknownDecodeFlags(u32),
    #[error("mosaic recipe manifest is missing required decode flags: 0x{0:08x}")]
    MissingRequiredDecodeFlags(u32),
    #[error("mosaic recipe manifest has a zero required field")]
    ZeroRequiredField,
    #[error("mosaic recipe manifest has an all-zero producer dependency-closure digest")]
    ZeroProducerDependencyDigest,
    #[error("mosaic recipe manifest reserved bytes must be zero")]
    NonZeroReserved,
}

const fn all_zero(bytes: &[u8; 32]) -> bool {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

const fn copy_array<const N: usize>(
    destination: &mut [u8; MOSAIC_RECIPE_MANIFEST_BYTES],
    offset: usize,
    source: &[u8; N],
) {
    let mut index = 0;
    while index < N {
        destination[offset + index] = source[index];
        index += 1;
    }
}

fn read_u16(bytes: &[u8; MOSAIC_RECIPE_MANIFEST_BYTES], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8; MOSAIC_RECIPE_MANIFEST_BYTES], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: [u8; 32] = [0x5a; 32];

    fn manifest() -> MosaicRecipeManifest {
        MosaicRecipeManifest::new(7, 11, 13, 17, KNOWN_MOSAIC_DECODE_FLAGS, DIGEST)
    }

    #[test]
    fn manifest_v1_has_fixed_language_neutral_layout() {
        let bytes = manifest().canonical_bytes();
        assert_eq!(bytes.len(), 64);
        assert_eq!(&bytes[..4], b"RMR\0");
        assert_eq!(&bytes[4..8], &[1, 0, 1, 0]);
        assert_eq!(&bytes[8..12], &7_u32.to_le_bytes());
        assert_eq!(&bytes[12..16], &11_u32.to_le_bytes());
        assert_eq!(&bytes[16..20], &13_u32.to_le_bytes());
        assert_eq!(&bytes[20..24], &17_u32.to_le_bytes());
        assert_eq!(&bytes[24..28], &KNOWN_MOSAIC_DECODE_FLAGS.to_le_bytes());
        assert_eq!(&bytes[28..60], &DIGEST);
        assert_eq!(&bytes[60..], &[0; 4]);
        assert_eq!(MosaicRecipeManifest::from_canonical_bytes(bytes), Ok(manifest()));
    }

    #[test]
    fn parser_rejects_every_reserved_byte_and_unknown_discriminant() {
        for index in 60..64 {
            let mut bytes = manifest().canonical_bytes();
            bytes[index] = 1;
            assert_eq!(
                MosaicRecipeManifest::from_canonical_bytes(bytes),
                Err(ManifestError::NonZeroReserved)
            );
        }
        let mut bytes = manifest().canonical_bytes();
        bytes[4..6].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            MosaicRecipeManifest::from_canonical_bytes(bytes),
            Err(ManifestError::UnsupportedVersion(2))
        );
        let mut bytes = manifest().canonical_bytes();
        bytes[24..28].copy_from_slice(&(KNOWN_MOSAIC_DECODE_FLAGS | (1 << 31)).to_le_bytes());
        assert!(matches!(
            MosaicRecipeManifest::from_canonical_bytes(bytes),
            Err(ManifestError::UnknownDecodeFlags(_))
        ));

        for bit in 0..u32::BITS {
            if KNOWN_MOSAIC_DECODE_FLAGS & (1 << bit) == 0 {
                continue;
            }
            let mut bytes = manifest().canonical_bytes();
            let flags = KNOWN_MOSAIC_DECODE_FLAGS & !(1 << bit);
            bytes[24..28].copy_from_slice(&flags.to_le_bytes());
            assert_eq!(
                MosaicRecipeManifest::from_canonical_bytes(bytes),
                Err(ManifestError::MissingRequiredDecodeFlags(flags))
            );
        }
    }

    #[test]
    fn parser_rejects_magic_kind_zero_fields_and_zero_dependency_digest() {
        let mut bytes = manifest().canonical_bytes();
        bytes[0] ^= 1;
        assert_eq!(
            MosaicRecipeManifest::from_canonical_bytes(bytes),
            Err(ManifestError::InvalidMagic)
        );

        let mut bytes = manifest().canonical_bytes();
        bytes[6..8].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            MosaicRecipeManifest::from_canonical_bytes(bytes),
            Err(ManifestError::UnsupportedArtifactKind(2))
        );

        for offset in [8, 12, 16, 20] {
            let mut bytes = manifest().canonical_bytes();
            bytes[offset..offset + 4].fill(0);
            assert_eq!(
                MosaicRecipeManifest::from_canonical_bytes(bytes),
                Err(ManifestError::ZeroRequiredField),
                "zero field at offset {offset}"
            );
        }

        let mut bytes = manifest().canonical_bytes();
        bytes[28..60].fill(0);
        assert_eq!(
            MosaicRecipeManifest::from_canonical_bytes(bytes),
            Err(ManifestError::ZeroProducerDependencyDigest)
        );
    }
}
