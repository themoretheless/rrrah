//! Shared cache value and native spill dispatch for legacy geometry and GLB scenes.
use crate::{CachedModel, ModelLoadError};
use rrrah_cache::{SwapPayload, SwapPayloadError};
use rrrah_core::MemoryBudget;
use rrrah_decode::{DecodeRequest, DecodedModel, GlbAsset};
use std::io::{Read, Write};

#[derive(Clone)]
pub(crate) enum ModelAsset {
    Geometry(DecodedModel),
    Glb(GlbAsset),
}
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) enum Key {
    Geometry(crate::ModelDisplayKey),
    Glb(crate::glb_cache::Key),
}
fn is_glb(request: &DecodeRequest) -> bool {
    request
        .path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("glb"))
}
impl ModelAsset {
    pub(crate) fn decode(request: &DecodeRequest) -> Result<Self, ModelLoadError> {
        if is_glb(request) {
            rrrah_decode::decode_glb_asset(request, None)
                .map(Self::Glb)
                .map_err(Into::into)
        } else {
            rrrah_decode::decode_model(request)
                .map(Self::Geometry)
                .map_err(ModelLoadError::Decode)
        }
    }
}
impl CachedModel for ModelAsset {
    type Key = Key;
    type Payload = Self;
    fn cache_key(request: &DecodeRequest) -> Option<Key> {
        if is_glb(request) {
            crate::glb_cache::key(request, None).map(Key::Glb)
        } else {
            crate::model_display_key(request).map(Key::Geometry)
        }
    }
    fn capacity_bytes(&self) -> u64 {
        SwapPayload::capacity_bytes(self)
    }
    fn shares_storage(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Geometry(a), Self::Geometry(b)) => a.shares_storage(b),
            (Self::Glb(a), Self::Glb(b)) => a.ptr_eq(b),
            _ => false,
        }
    }
    fn into_payload(self) -> Option<Self> {
        Some(self)
    }
    fn from_payload(payload: Self) -> Self {
        payload
    }
}
impl SwapPayload for ModelAsset {
    fn capacity_bytes(&self) -> u64 {
        match self {
            Self::Geometry(m) => m.capacity_bytes(),
            Self::Glb(m) => m.capacity_bytes(),
        }
    }
    fn minimum_restore_weight(&self) -> Option<u64> {
        match self {
            Self::Geometry(m) => Some(m.minimum_restore_weight()),
            Self::Glb(m) => crate::glb_cache::Payload(m.clone()).minimum_restore_weight(),
        }
    }
    fn payload_len(&self) -> std::io::Result<u64> {
        match self {
            Self::Geometry(m) => crate::model_swap::ModelPayload(m.clone()).payload_len(),
            Self::Glb(m) => crate::glb_cache::Payload(m.clone()).payload_len(),
        }
    }
    fn write_payload(&self, writer: &mut impl Write) -> std::io::Result<()> {
        match self {
            Self::Geometry(m) => crate::model_swap::ModelPayload(m.clone()).write_payload(writer),
            Self::Glb(m) => crate::glb_cache::Payload(m.clone()).write_payload(writer),
        }
    }
    fn read_payload(
        reader: &mut impl Read,
        bytes: u64,
        budget: &MemoryBudget,
    ) -> Result<Self, SwapPayloadError> {
        if bytes < 8 {
            return Err(SwapPayloadError::Invalid(std::io::Error::other(
                "short model asset payload",
            )));
        }
        let mut magic = [0; 8];
        reader.read_exact(&mut magic).map_err(SwapPayloadError::Invalid)?;
        let mut stream = std::io::Cursor::new(magic).chain(reader);
        if &magic == b"RRGLB001" {
            crate::glb_cache::Payload::read_payload(&mut stream, bytes, budget).map(|p| Self::Glb(p.0))
        } else {
            crate::model_swap::ModelPayload::read_payload(&mut stream, bytes, budget)
                .map(|p| Self::Geometry(p.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_model_cache_spills_and_restores_both_native_families() {
        let root = MemoryBudget::new(8 * 1024 * 1024);
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        let paths = [
            base.join("models/off-triangle.off"),
            base.join("glb/BoxTextured.glb"),
        ];
        let dir = tempfile::tempdir().unwrap();
        let mut cache = crate::ModelCache::<ModelAsset>::new(rrrah_cache::CacheLimits {
            max_bytes: 4 * 1024 * 1024,
            max_entries: Some(1),
            ttl: None,
        });
        cache.swap = Some(
            rrrah_cache::ImageSwapCache::new_with_budgets(
                dir.path(),
                rrrah_cache::ImageSwapConfig {
                    limits: rrrah_cache::CacheLimits::bytes(8 * 1024 * 1024),
                    queue_bytes: 4 * 1024 * 1024,
                    queue_count: 4,
                    restore_bytes: 4 * 1024 * 1024,
                },
                MemoryBudget::new(4 * 1024 * 1024),
                root.child(4 * 1024 * 1024),
            )
            .unwrap(),
        );
        let requests: Vec<_> = paths
            .into_iter()
            .map(|p| {
                let mut r = DecodeRequest::new(p);
                r.memory_budget = Some(root.clone());
                r
            })
            .collect();
        for r in &requests {
            let first = cache.load(r, false, || ModelAsset::decode(r)).unwrap();
            let hit = cache
                .load(r, false, || panic!("resident hit must bypass decoder"))
                .unwrap();
            assert!(first.shares_storage(&hit));
            drop((first, hit));
        }
        cache.swap.as_ref().unwrap().wait_idle().unwrap();
        for r in &requests {
            let restored = cache
                .load(r, false, || panic!("mixed native spill must bypass decoder"))
                .unwrap();
            assert_eq!(matches!(restored, ModelAsset::Glb(_)), is_glb(r));
            drop(restored);
            cache.swap.as_ref().unwrap().wait_idle().unwrap();
        }
        assert_eq!(cache.swap.as_ref().unwrap().stats().reads, 2);
        drop(cache);
        assert_eq!(root.used(), 0);
    }
}
