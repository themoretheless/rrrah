use std::fmt;

use thiserror::Error;

use rrrah_core::{MosaicRecipeManifest, SENSOR_MOSAIC_ARTIFACT_KIND_CODE};

const SOURCE_ID_CONTEXT: &str = "rrrah.cache.source-id.v1";
const RECIPE_ID_CONTEXT: &str = "rrrah.cache.recipe-id.v1";
const ARTIFACT_KEY_CONTEXT: &str = "rrrah.cache.artifact-key.v1";
/// Canonical `NoVariant` tag. All-zero is reserved for a whole sensor mosaic.
const SENSOR_MOSAIC_VARIANT: [u8; 32] = [0; 32];
pub const MOSAIC_ARTIFACT_DESCRIPTOR_V1_BYTES: usize = 2 + 32 + 8 + 32 + 32;

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
enum DigestParseError {
    #[error("expected 64 lowercase hexadecimal characters, got {actual}")]
    InvalidHexLength { actual: usize },
    #[error("invalid lowercase hexadecimal byte at offset {index}: 0x{byte:02x}")]
    InvalidHexByte { index: usize, byte: u8 },
}

macro_rules! digest_type {
    ($name:ident) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            #[allow(dead_code)]
            pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            #[allow(dead_code)]
            pub const fn into_bytes(self) -> [u8; 32] {
                self.0
            }
        }

        impl fmt::LowerHex for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write_hex_digest(formatter, &self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::LowerHex::fmt(self, formatter)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "("))?;
                fmt::LowerHex::fmt(self, formatter)?;
                formatter.write_str(")")
            }
        }
    };
}

digest_type!(SourceId);
digest_type!(RecipeId);
digest_type!(ArtifactKey);

impl SourceId {
    /// Derives a path-independent identity from every byte of one stable RAW
    /// snapshot. File opening and stability checks belong to the resolver.
    #[cfg(test)]
    fn from_content(bytes: &[u8]) -> Self {
        let mut hasher = SourceIdHasher::new();
        hasher.update(bytes);
        hasher.finalize()
    }
}

/// Incremental full-content hash used by the stable-source resolver.
///
/// Creation is deliberately crate-private. Epic 6's stable-source resolver must
/// use one opened snapshot for this full hash and decode, with stability checks
/// before it can expose the resulting `SourceId`.
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct SourceIdHasher(blake3::Hasher);

#[allow(dead_code)]
impl SourceIdHasher {
    pub(crate) fn new() -> Self {
        Self(blake3::Hasher::new_derive_key(SOURCE_ID_CONTEXT))
    }

    pub(crate) fn update(&mut self, chunk: &[u8]) {
        self.0.update(chunk);
    }

    pub(crate) fn finalize(self) -> SourceId {
        SourceId::from_bytes(*self.0.finalize().as_bytes())
    }
}

impl RecipeId {
    fn from_manifest(manifest: MosaicRecipeManifest) -> Self {
        Self(blake3::derive_key(RECIPE_ID_CONTEXT, &manifest.canonical_bytes()))
    }
}

/// An artifact-kind-tagged recipe ID for a full decoded sensor mosaic.
///
/// The decoder owns the semantic manifest; the cache only hashes its canonical
/// bytes. This prevents thumbnail/tile recipes from entering `MosaicKey`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MosaicRecipeId(RecipeId);

impl MosaicRecipeId {
    pub fn from_manifest(manifest: MosaicRecipeManifest) -> Self {
        Self(RecipeId::from_manifest(manifest))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }

    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(RecipeId::from_bytes(bytes))
    }
}

impl fmt::LowerHex for MosaicRecipeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, formatter)
    }
}

impl fmt::Display for MosaicRecipeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(self, formatter)
    }
}

impl fmt::Debug for MosaicRecipeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MosaicRecipeId(")?;
        fmt::LowerHex::fmt(self, formatter)?;
        formatter.write_str(")")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MosaicKey {
    source: SourceId,
    image_index: u64,
    recipe: MosaicRecipeId,
}

impl MosaicKey {
    pub const fn new(source: SourceId, image_index: u64, recipe: MosaicRecipeId) -> Self {
        Self {
            source,
            image_index,
            recipe,
        }
    }

    pub const fn source(self) -> SourceId {
        self.source
    }

    pub const fn image_index(self) -> u64 {
        self.image_index
    }

    pub const fn recipe(self) -> MosaicRecipeId {
        self.recipe
    }

    pub fn artifact_key(self) -> ArtifactKey {
        let preimage = self.canonical_preimage();
        ArtifactKey::from_bytes(blake3::derive_key(ARTIFACT_KEY_CONTEXT, &preimage))
    }

    fn canonical_preimage(self) -> [u8; MOSAIC_ARTIFACT_DESCRIPTOR_V1_BYTES] {
        let mut preimage = [0_u8; MOSAIC_ARTIFACT_DESCRIPTOR_V1_BYTES];
        preimage[..2].copy_from_slice(&SENSOR_MOSAIC_ARTIFACT_KIND_CODE.to_le_bytes());
        preimage[2..34].copy_from_slice(self.source.as_bytes());
        preimage[34..42].copy_from_slice(&self.image_index.to_le_bytes());
        preimage[42..74].copy_from_slice(&SENSOR_MOSAIC_VARIANT);
        preimage[74..].copy_from_slice(self.recipe.as_bytes());
        preimage
    }
}

/// Complete semantic descriptor stored alongside a V3 mosaic object.
/// Its bytes are also the canonical `ArtifactKey` transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MosaicArtifactDescriptorV1(MosaicKey);

impl MosaicArtifactDescriptorV1 {
    pub const fn new(key: MosaicKey) -> Self {
        Self(key)
    }

    pub const fn mosaic_key(self) -> MosaicKey {
        self.0
    }

    pub fn artifact_key(self) -> ArtifactKey {
        self.0.artifact_key()
    }

    pub fn encode(self) -> [u8; MOSAIC_ARTIFACT_DESCRIPTOR_V1_BYTES] {
        self.0.canonical_preimage()
    }

    pub fn decode(bytes: [u8; MOSAIC_ARTIFACT_DESCRIPTOR_V1_BYTES]) -> Result<Self, MosaicDescriptorError> {
        let kind = u16::from_le_bytes([bytes[0], bytes[1]]);
        if kind != SENSOR_MOSAIC_ARTIFACT_KIND_CODE {
            return Err(MosaicDescriptorError::UnsupportedArtifactKind(kind));
        }
        if bytes[42..74].iter().any(|byte| *byte != 0) {
            return Err(MosaicDescriptorError::UnsupportedVariant);
        }
        let mut source = [0_u8; 32];
        source.copy_from_slice(&bytes[2..34]);
        let mut image_index = [0_u8; 8];
        image_index.copy_from_slice(&bytes[34..42]);
        let mut recipe = [0_u8; 32];
        recipe.copy_from_slice(&bytes[74..106]);
        let source = SourceId::from_bytes(source);
        let image_index = u64::from_le_bytes(image_index);
        let recipe = MosaicRecipeId::from_bytes(recipe);
        Ok(Self(MosaicKey::new(source, image_index, recipe)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MosaicDescriptorError {
    #[error("unsupported artifact kind {0} in mosaic descriptor")]
    UnsupportedArtifactKind(u16),
    #[error("sensor mosaic descriptor must use the all-zero NoVariant tag")]
    UnsupportedVariant,
}

fn write_hex_digest(formatter: &mut fmt::Formatter<'_>, bytes: &[u8; 32]) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

#[cfg(test)]
fn parse_hex_digest(value: &str) -> Result<[u8; 32], DigestParseError> {
    let value = value.as_bytes();
    if value.len() != 64 {
        return Err(DigestParseError::InvalidHexLength { actual: value.len() });
    }
    let mut digest = [0_u8; 32];
    for (index, pair) in value.chunks_exact(2).enumerate() {
        let high = parse_lower_hex_nibble(pair[0], index * 2)?;
        let low = parse_lower_hex_nibble(pair[1], index * 2 + 1)?;
        digest[index] = (high << 4) | low;
    }
    Ok(digest)
}

#[cfg(test)]
fn parse_lower_hex_nibble(byte: u8, index: usize) -> Result<u8, DigestParseError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(DigestParseError::InvalidHexByte { index, byte }),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rrrah_core::KNOWN_MOSAIC_DECODE_FLAGS;

    use super::*;

    const TEST_PRODUCER_DIGEST: [u8; 32] = [0x5a; 32];

    fn manifest_with_parts(
        backend: u32,
        backend_revision: u32,
        adapter_revision: u32,
        model_revision: u32,
        dependency_digest: [u8; 32],
    ) -> MosaicRecipeManifest {
        MosaicRecipeManifest::new(
            backend,
            backend_revision,
            adapter_revision,
            model_revision,
            KNOWN_MOSAIC_DECODE_FLAGS,
            dependency_digest,
        )
    }

    fn recipe_with_parts(
        backend: u32,
        backend_revision: u32,
        adapter_revision: u32,
        model_revision: u32,
        dependency_digest: [u8; 32],
    ) -> MosaicRecipeId {
        MosaicRecipeId::from_manifest(manifest_with_parts(
            backend,
            backend_revision,
            adapter_revision,
            model_revision,
            dependency_digest,
        ))
    }

    fn recipe_from_adapter_revision(adapter_revision: u32) -> MosaicRecipeId {
        MosaicRecipeId::from_manifest(manifest_with_parts(
            1,
            1,
            adapter_revision,
            1,
            TEST_PRODUCER_DIGEST,
        ))
    }

    #[test]
    fn canonical_preimage_has_fixed_width_and_little_endian_index() {
        let source = SourceId::from_bytes([0x11; 32]);
        let recipe = MosaicRecipeId(RecipeId::from_bytes([0x22; 32]));
        let preimage = MosaicKey::new(source, 0x0102_0304_0506_0708, recipe).canonical_preimage();

        assert_eq!(preimage.len(), 106);
        assert_eq!(&preimage[..2], &[1, 0]);
        assert_eq!(&preimage[2..34], &[0x11; 32]);
        assert_eq!(&preimage[34..42], &[8, 7, 6, 5, 4, 3, 2, 1]);
        assert_eq!(&preimage[42..74], &[0; 32]);
        assert_eq!(&preimage[74..], &[0x22; 32]);

        let descriptor =
            MosaicArtifactDescriptorV1::new(MosaicKey::new(source, 0x0102_0304_0506_0708, recipe));
        assert_eq!(
            MosaicArtifactDescriptorV1::decode(descriptor.encode()),
            Ok(descriptor)
        );
        assert_eq!(descriptor.artifact_key(), descriptor.mosaic_key().artifact_key());
    }

    #[test]
    fn mosaic_descriptor_rejects_unknown_kind_and_variant() {
        let descriptor = MosaicArtifactDescriptorV1::new(MosaicKey::new(
            SourceId::from_bytes([0x11; 32]),
            7,
            MosaicRecipeId(RecipeId::from_bytes([0x22; 32])),
        ));
        let mut bytes = descriptor.encode();
        bytes[..2].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            MosaicArtifactDescriptorV1::decode(bytes),
            Err(MosaicDescriptorError::UnsupportedArtifactKind(2))
        );

        let mut bytes = descriptor.encode();
        bytes[73] = 1;
        assert_eq!(
            MosaicArtifactDescriptorV1::decode(bytes),
            Err(MosaicDescriptorError::UnsupportedVariant)
        );

        for variant_offset in 42..74 {
            let mut bytes = descriptor.encode();
            bytes[variant_offset] = 1;
            assert_eq!(
                MosaicArtifactDescriptorV1::decode(bytes),
                Err(MosaicDescriptorError::UnsupportedVariant),
                "variant byte {variant_offset} was accepted"
            );
        }
    }

    #[test]
    fn digest_hex_format_and_parse_are_strict() {
        let digest = SourceId::from_bytes([0xab; 32]);
        let encoded = digest.to_string();
        assert_eq!(encoded, "ab".repeat(32));
        assert_eq!(source_id_from_hex(&encoded), Ok(digest));
        assert!(matches!(
            source_id_from_hex(&"AB".repeat(32)),
            Err(DigestParseError::InvalidHexByte { index: 0, byte: b'A' })
        ));
        assert_eq!(
            source_id_from_hex("00"),
            Err(DigestParseError::InvalidHexLength { actual: 2 })
        );
        assert!(matches!(
            source_id_from_hex(&format!("{}g", "0".repeat(63))),
            Err(DigestParseError::InvalidHexByte {
                index: 63,
                byte: b'g'
            })
        ));
    }

    #[test]
    fn artifact_key_is_domain_separated_by_source_recipe_and_frame() {
        let source = SourceId::from_content(b"full RAW bytes");
        let other_source = SourceId::from_content(b"full RAW byteS");
        let recipe = recipe_from_adapter_revision(1);
        let other_recipe = recipe_from_adapter_revision(2);
        let baseline = MosaicKey::new(source, 0, recipe).artifact_key();

        assert_ne!(baseline, MosaicKey::new(other_source, 0, recipe).artifact_key());
        assert_ne!(baseline, MosaicKey::new(source, 1, recipe).artifact_key());
        assert_ne!(baseline, MosaicKey::new(source, 0, other_recipe).artifact_key());
        assert_ne!(source.as_bytes(), recipe.as_bytes());
    }

    #[test]
    fn every_structured_field_bit_changes_the_artifact_key() {
        let baseline_source = [0_u8; 32];
        let baseline_recipe = [0_u8; 32];
        let baseline = MosaicKey::new(
            SourceId::from_bytes(baseline_source),
            0,
            MosaicRecipeId(RecipeId::from_bytes(baseline_recipe)),
        )
        .artifact_key();
        let mut observed = HashSet::from([baseline]);

        for bit in 0..256 {
            let mut source = baseline_source;
            source[bit / 8] ^= 1 << (bit % 8);
            let key = MosaicKey::new(
                SourceId::from_bytes(source),
                0,
                MosaicRecipeId(RecipeId::from_bytes(baseline_recipe)),
            )
            .artifact_key();
            assert!(observed.insert(key), "source bit {bit} was ignored or aliased");
        }
        for bit in 0..64 {
            let key = MosaicKey::new(
                SourceId::from_bytes(baseline_source),
                1_u64 << bit,
                MosaicRecipeId(RecipeId::from_bytes(baseline_recipe)),
            )
            .artifact_key();
            assert!(
                observed.insert(key),
                "image-index bit {bit} was ignored or aliased"
            );
        }
        for bit in 0..256 {
            let mut recipe = baseline_recipe;
            recipe[bit / 8] ^= 1 << (bit % 8);
            let key = MosaicKey::new(
                SourceId::from_bytes(baseline_source),
                0,
                MosaicRecipeId(RecipeId::from_bytes(recipe)),
            )
            .artifact_key();
            assert!(observed.insert(key), "recipe bit {bit} was ignored or aliased");
        }
    }

    #[test]
    fn every_manifest_identity_field_bit_changes_the_recipe_id() {
        let baseline_scalar = u32::MAX;
        let baseline_digest = [0xa5; 32];
        let baseline = recipe_with_parts(
            baseline_scalar,
            baseline_scalar,
            baseline_scalar,
            baseline_scalar,
            baseline_digest,
        );
        let mut observed = HashSet::from([baseline]);
        for field in 0..4 {
            for bit in 0..32 {
                let mut values = [baseline_scalar; 4];
                values[field] ^= 1_u32 << bit;
                let recipe = recipe_with_parts(values[0], values[1], values[2], values[3], baseline_digest);
                assert!(
                    observed.insert(recipe),
                    "manifest scalar field {field} bit {bit} was ignored or aliased"
                );
            }
        }
        for bit in 0..256 {
            let mut digest = baseline_digest;
            digest[bit / 8] ^= 1 << (bit % 8);
            let recipe = recipe_with_parts(
                baseline_scalar,
                baseline_scalar,
                baseline_scalar,
                baseline_scalar,
                digest,
            );
            assert!(
                observed.insert(recipe),
                "dependency digest bit {bit} was ignored or aliased"
            );
        }
    }

    #[test]
    fn invalid_hex_reports_every_nibble_position_exactly() {
        for invalid_index in 0..64 {
            let mut encoded = [b'0'; 64];
            encoded[invalid_index] = b'g';
            let encoded = std::str::from_utf8(&encoded).unwrap();
            assert_eq!(
                source_id_from_hex(encoded),
                Err(DigestParseError::InvalidHexByte {
                    index: invalid_index,
                    byte: b'g',
                })
            );
        }
    }

    #[test]
    fn every_byte_value_round_trips_through_canonical_hex() {
        for byte in u8::MIN..=u8::MAX {
            let digest = SourceId::from_bytes([byte; 32]);
            let encoded = digest.to_string();
            assert_eq!(source_id_from_hex(&encoded), Ok(digest), "byte 0x{byte:02x}");
        }
    }

    #[test]
    fn streaming_source_id_is_independent_of_chunk_boundaries() {
        let mut content = Vec::with_capacity(16_387);
        for index in 0..16_387_u32 {
            content.push(index.wrapping_mul(31).wrapping_add(17).to_le_bytes()[1]);
        }
        let expected = SourceId::from_content(&content);

        for chunk_size in [1, 2, 3, 7, 64, 1024, 4096, content.len()] {
            let mut hasher = SourceIdHasher::new();
            for chunk in content.chunks(chunk_size) {
                hasher.update(chunk);
            }
            assert_eq!(hasher.finalize(), expected, "chunk size {chunk_size}");
        }
    }

    #[test]
    fn canonical_golden_vectors_are_stable() {
        let empty = SourceId::from_content(b"");
        let sample = SourceId::from_content(&[0x00, 0x01, 0x02, 0xff]);
        let recipe = recipe_from_adapter_revision(1);
        assert_eq!(
            empty.to_string(),
            "ac86ac94caaa88afca2ac28e18aff1927fd9fe727c6c1cf6f5b68668eee971c2"
        );
        assert_eq!(
            sample.to_string(),
            "7b8792c318d96b870db65b324c3997769cc773ebbd4e5fb5d620a3a21ce0996b"
        );
        assert_eq!(
            recipe.to_string(),
            "932219a5b11ca7978d9e4857d92b807a155932d789d60dc5ae770a6dfb8cbf4f"
        );
        assert_eq!(
            MosaicKey::new(sample, 0, recipe).artifact_key().to_string(),
            "c47466531caafd8f3926077f8f68c4ed0080a5508db791c28891b38b57ecd46e"
        );
        assert_eq!(
            MosaicKey::new(sample, 1, recipe).artifact_key().to_string(),
            "d87c45fbd12477c9cd9ccc1bf53671e08bca082ca66b949a151234207cafd9e7"
        );
        assert_eq!(
            MosaicKey::new(sample, u64::MAX, recipe)
                .artifact_key()
                .to_string(),
            "8862806b5991bb261fe8f1f4df4b0adc03c69ebb6913eeebbd3e6654836424d5"
        );
    }

    #[test]
    fn endian_sensitive_recipe_and_artifact_vectors_are_stable() {
        let recipe = recipe_from_adapter_revision(0x0102_0304);
        assert_eq!(
            recipe.to_string(),
            "7916d7dfa069d7ace4cb14ec71833bb01a801c3b01cfcca37773cb26c7ffe5b3"
        );

        let key = MosaicKey::new(
            SourceId::from_bytes([0x11; 32]),
            0x0102_0304_0506_0708,
            MosaicRecipeId(RecipeId::from_bytes([0x22; 32])),
        )
        .artifact_key();
        assert_eq!(
            key.to_string(),
            "2a7af010feb83550e5cbcafda28dfb1376b882db54436d8f2ca687c217f234fb"
        );
    }

    #[test]
    fn derive_key_contexts_are_independently_domain_separated() {
        assert_eq!(
            hex(blake3::derive_key(SOURCE_ID_CONTEXT, b"abc")),
            "f5f74bcb57f67b9f469be2381bd4bc25a6eb944d924258013d3d2cce171455e4"
        );
        assert_eq!(
            hex(blake3::derive_key(RECIPE_ID_CONTEXT, b"abc")),
            "29b9cdb7131e3d17706bbf15fc98e1c127c0a8b930251431d65629cf9f3e2a5c"
        );
        assert_eq!(
            hex(blake3::derive_key(ARTIFACT_KEY_CONTEXT, b"abc")),
            "e1ed53d6892c80fe9f6a290949b2a32ca727bedc7ca7436166ead352e56c5129"
        );
    }

    fn hex(bytes: [u8; 32]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn source_id_from_hex(value: &str) -> Result<SourceId, DigestParseError> {
        parse_hex_digest(value).map(SourceId::from_bytes)
    }
}
