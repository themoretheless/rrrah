//! Narrow, feature-gated access to the real private key primitives.

use rrrah_core::MosaicRecipeManifest;

use crate::key::{MosaicKey, MosaicRecipeId, SourceId, SourceIdHasher};

#[derive(Debug, Clone, Copy)]
pub struct KeyFixture {
    source: SourceId,
    recipe: MosaicRecipeId,
}

impl KeyFixture {
    pub fn new(source_digest: [u8; 32], manifest: MosaicRecipeManifest) -> Self {
        Self {
            source: SourceId::from_bytes(source_digest),
            recipe: MosaicRecipeId::from_manifest(manifest),
        }
    }

    pub fn artifact_key(self, image_index: u64) -> [u8; 32] {
        MosaicKey::new(self.source, image_index, self.recipe)
            .artifact_key()
            .into_bytes()
    }
}

pub fn hash_source(source: &[u8], chunk_bytes: usize) -> [u8; 32] {
    let mut hasher = SourceIdHasher::new();
    for chunk in source.chunks(chunk_bytes.max(1)) {
        hasher.update(chunk);
    }
    hasher.finalize().into_bytes()
}

pub fn recipe_id(manifest: MosaicRecipeManifest) -> [u8; 32] {
    *MosaicRecipeId::from_manifest(manifest).as_bytes()
}
