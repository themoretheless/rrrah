//! GLB specialization of the same RAM/spill state machine used by visible models.
use rrrah_cache::{SwapPayload, SwapPayloadError};
use rrrah_core::{BufferError, MemoryBudget};
use rrrah_decode::glb::GlbError;
use rrrah_decode::{DecodeRequest, GlbAsset, GlbAssetError, GltfTextureError};
use std::io::{Read, Write};
const POLICY_REVISION: u32 = 1;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Key {
    source: crate::ModelDisplayKey,
    scene: Option<usize>,
    policy: u32,
}
pub(crate) fn key(request: &DecodeRequest, scene: Option<usize>) -> Option<Key> {
    if request.image_index != 0 {
        return None;
    }
    Some(Key {
        source: crate::model_display_key(request)?,
        scene,
        policy: POLICY_REVISION,
    })
}
#[derive(Clone)]
pub(crate) struct Payload(pub GlbAsset);
pub(crate) fn memory_error(error: &GlbAssetError) -> Option<&BufferError> {
    use rrrah_decode::DecodeError as D;
    match error {
        GlbAssetError::Memory(e)
        | GlbAssetError::Source(D::Memory(e))
        | GlbAssetError::Container(GlbError::Source(D::Memory(e)))
        | GlbAssetError::Texture(GltfTextureError::Memory(e))
        | GlbAssetError::Texture(GltfTextureError::Source(D::Memory(e)))
        | GlbAssetError::Texture(GltfTextureError::Container(GlbError::Source(D::Memory(e)))) => Some(e),
        _ => None,
    }
}
fn codec_error(error: GlbAssetError) -> SwapPayloadError {
    use rrrah_decode::DecodeError as D;
    match error {
        GlbAssetError::Memory(e)
        | GlbAssetError::Source(D::Memory(e))
        | GlbAssetError::Container(GlbError::Source(D::Memory(e)))
        | GlbAssetError::Texture(GltfTextureError::Memory(e))
        | GlbAssetError::Texture(GltfTextureError::Source(D::Memory(e)))
        | GlbAssetError::Texture(GltfTextureError::Container(GlbError::Source(D::Memory(e)))) => {
            SwapPayloadError::Memory(e)
        }
        e => SwapPayloadError::Invalid(std::io::Error::other(e)),
    }
}
impl SwapPayload for Payload {
    fn capacity_bytes(&self) -> u64 {
        self.0.capacity_bytes()
    }
    fn minimum_restore_weight(&self) -> Option<u64> {
        // Serialized source/sample lengths are guaranteed retained storage;
        // source-side spare capacity and rebuilt metadata are not a safe floor.
        Some(
            self.0.scene().document().source().container_bytes().len() as u64
                + self
                    .0
                    .images()
                    .iter()
                    .map(|image| {
                        let image = image.for_role(rrrah_decode::glb::GlbTextureRole::Normal);
                        u64::from(image.width())
                            * u64::from(image.height())
                            * 4
                            * match image.pixels() {
                                rrrah_decode::GltfTexturePixels::Rgba8(_) => 1,
                                rrrah_decode::GltfTexturePixels::Rgba16(_) => 2,
                            }
                    })
                    .sum::<u64>(),
        )
    }
    fn payload_len(&self) -> std::io::Result<u64> {
        rrrah_decode::glb_asset_swap_payload_len(&self.0).map_err(std::io::Error::other)
    }
    fn write_payload(&self, writer: &mut impl Write) -> std::io::Result<()> {
        rrrah_decode::write_glb_asset_swap_payload(&self.0, writer, &DecodeRequest::new("GLB spill"))
            .map_err(std::io::Error::other)
    }
    fn read_payload(
        reader: &mut impl Read,
        bytes: u64,
        budget: &MemoryBudget,
    ) -> Result<Self, SwapPayloadError> {
        let mut request = DecodeRequest::new("GLB restore");
        request.memory_budget = Some(budget.clone());
        rrrah_decode::read_glb_asset_swap_payload(reader, bytes, &request)
            .map(Self)
            .map_err(codec_error)
    }
}
impl crate::CachedModel for GlbAsset {
    type Key = Key;
    type Payload = Payload;
    fn cache_key(request: &DecodeRequest) -> Option<Key> {
        key(request, None)
    }
    fn capacity_bytes(&self) -> u64 {
        self.capacity_bytes()
    }
    fn shares_storage(&self, other: &Self) -> bool {
        self.ptr_eq(other)
    }
    fn into_payload(self) -> Option<Payload> {
        Some(Payload(self))
    }
    fn from_payload(payload: Payload) -> Self {
        payload.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CachedModel, ModelCache, ModelLoadError};
    use std::sync::{Arc, atomic::AtomicU64};
    fn request(budget: &MemoryBudget) -> DecodeRequest {
        let mut r = DecodeRequest::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/glb/BoxTextured.glb"),
        );
        r.memory_budget = Some(budget.clone());
        r
    }
    fn decode(r: &DecodeRequest) -> Result<GlbAsset, ModelLoadError> {
        rrrah_decode::decode_glb_asset(r, None).map_err(Into::into)
    }
    fn swap(parent: &std::path::Path, root: &MemoryBudget) -> rrrah_cache::ImageSwapCache<Key, Payload> {
        rrrah_cache::ImageSwapCache::new_with_budgets(
            parent,
            rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits {
                    max_bytes: 2 * 1024 * 1024,
                    max_entries: Some(4),
                    ttl: None,
                },
                queue_bytes: 1024 * 1024,
                queue_count: 4,
                restore_bytes: 1024 * 1024,
            },
            MemoryBudget::new(1024 * 1024),
            root.child(1024 * 1024),
        )
        .unwrap()
    }
    #[test]
    fn glb_app_cache_real_ram_hit_async_spill_restore_and_visible_lease() {
        let root = MemoryBudget::new(4 * 1024 * 1024);
        let r = request(&root);
        let mut cache = ModelCache::<GlbAsset>::new(rrrah_cache::CacheLimits {
            max_bytes: 1024 * 1024,
            max_entries: Some(1),
            ttl: None,
        });
        let dir = tempfile::tempdir().unwrap();
        cache.swap = Some(swap(dir.path(), &root));
        let key = GlbAsset::cache_key(&r).unwrap();
        let (first, held) = cache
            .load_with_cancel_resolving_key_mode_with_lease(
                || Some(key.clone()),
                true,
                false,
                true,
                || decode(&r),
                || false,
            )
            .unwrap();
        let weight = first.capacity_bytes();
        assert!(held.is_none()); // Cold visible path uses owner pin.
        let visible = cache.visible_lease_for(&first).unwrap();
        let hit = cache
            .load(&r, true, || panic!("resident GLB must bypass native decode"))
            .unwrap();
        assert!(hit.ptr_eq(&first));
        assert_eq!(root.used(), weight);
        cache.entries.unpin(&key);
        cache.visible = None;
        assert!(cache.entries.take_lru().is_none());
        drop(visible);
        drop(hit);
        drop(first);
        let (victim_key, victim) = cache.entries.take_lru().unwrap();
        let floor = Payload(victim.clone()).minimum_restore_weight().unwrap();
        assert!(floor <= victim.capacity_bytes());
        cache.swap.as_ref().unwrap().enqueue(victim_key, Payload(victim));
        cache.swap.as_ref().unwrap().wait_idle().unwrap();
        assert_eq!(root.used(), 0);
        let (restored, lease) = cache
            .load_with_cancel_resolving_key_mode_with_lease(
                || Some(key.clone()),
                false,
                true,
                true,
                || panic!("completed GLB spill must bypass decoder"),
                || false,
            )
            .unwrap();
        let lease = lease.unwrap();
        assert!(lease.ptr_eq(&restored));
        assert_eq!(restored.capacity_bytes(), weight);
        assert_eq!(root.used(), weight);
        assert_eq!(cache.swap.as_ref().unwrap().stats().reads, 1);
        assert_eq!(cache.swap.as_ref().unwrap().stats().writes, 1);
        drop(cache);
        assert_eq!(root.used(), weight);
        drop(restored);
        assert_eq!(root.used(), weight);
        drop(lease);
        assert_eq!(root.used(), 0);
        eprintln!(
            "GLB app cache RAM hit -> async ImageSwapCache spill -> retained restore, decoder bypassed; final root zero"
        );
    }
    #[test]
    fn glb_app_ttl_expiration_spills_then_restores_without_native_decode() {
        let root = MemoryBudget::new(4 * 1024 * 1024);
        let r = request(&root);
        let mut cache = ModelCache::<GlbAsset>::new(rrrah_cache::CacheLimits {
            max_bytes: 1024 * 1024,
            max_entries: Some(1),
            ttl: Some(std::time::Duration::ZERO),
        });
        let dir = tempfile::tempdir().unwrap();
        cache.swap = Some(swap(dir.path(), &root));
        let asset = cache.load(&r, false, || decode(&r)).unwrap();
        let weight = asset.capacity_bytes();
        assert_eq!(cache.entries.len(), 1);
        cache.spill_expired();
        assert!(cache.entries.is_empty());
        cache.swap.as_ref().unwrap().wait_idle().unwrap();
        assert_eq!(root.used(), weight);
        drop(asset);
        assert_eq!(root.used(), 0);
        let restored = cache
            .load(&r, false, || panic!("expired GLB must restore completed spill"))
            .unwrap();
        let image = restored.images()[0].for_role(rrrah_decode::glb::GlbTextureRole::BaseColor);
        let rrrah_decode::GltfTexturePixels::Rgba8(pixels) = image.pixels() else {
            panic!()
        };
        assert_eq!(
            &pixels[..],
            include_bytes!("../../../tests/fixtures/glb/texture-codec/BoxTextured.rgba")
        );
        assert_eq!(cache.swap.as_ref().unwrap().stats().writes, 1);
        assert_eq!(cache.swap.as_ref().unwrap().stats().reads, 1);
        drop(image);
        drop(restored);
        drop(cache);
        assert_eq!(root.used(), 0);
        eprintln!(
            "GLB app TTL expiration -> real async spill -> original pixel-exact restore; decoder bypassed; root zero"
        );
    }
    #[test]
    fn glb_app_swap_nested_pressure_keeps_blob_for_budget_retry() {
        let root = MemoryBudget::new(4 * 1024 * 1024);
        let r = request(&root);
        let asset = decode(&r).unwrap();
        let k = key(&r, None).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut tier = swap(dir.path(), &root);
        assert!(!Payload::permits_hash_scratch());
        tier.enqueue(k.clone(), Payload(asset));
        tier.wait_idle().unwrap();
        assert_eq!(root.used(), 0);
        tier.set_restore_budget(root.child(6000));
        let result = tier.try_get(&k, || false);
        assert!(matches!(result, Err(BufferError::Capacity { .. })));
        assert_eq!(root.used(), 0);
        assert_eq!(tier.stats().errors, 0);
        assert_eq!(tier.stats().reads, 0);
        tier.set_restore_budget(root.child(1024 * 1024));
        let restored = tier.try_get(&k, || false).unwrap().unwrap();
        assert_eq!(tier.stats().reads, 1);
        assert_eq!(tier.stats().errors, 0);
        drop(restored);
        drop(tier);
        assert_eq!(root.used(), 0);
        eprintln!(
            "GLB app nested JSON admission pressure retains swap blob; larger-budget retry succeeds; roots zero"
        );
    }
    #[test]
    fn glb_app_key_scene_policy_cancel_and_nested_memory_errors_are_distinct() {
        let root = MemoryBudget::new(4 * 1024 * 1024);
        let mut r = request(&root);
        assert_ne!(key(&r, None), key(&r, Some(0)));
        let mut revised = key(&r, None).unwrap();
        revised.policy += 1;
        assert_ne!(Some(revised), key(&r, None));
        let mut cache = ModelCache::<GlbAsset>::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
        let token = Arc::new(AtomicU64::new(2));
        r.cancellation = Some(rrrah_decode::GenerationToken::new(token, 1));
        assert!(
            cache
                .load(&r, true, || panic!("cancelled GLB must not decode"))
                .is_err()
        );
        assert!(cache.entries.is_empty());
        assert_eq!(root.used(), 0);
        for limit in [1, 6000, 272619] {
            let budget = MemoryBudget::new(limit);
            let r = request(&budget);
            let error = rrrah_decode::decode_glb_asset(&r, None).unwrap_err();
            assert!(memory_error(&error).is_some(), "limit={limit}: {error}");
            assert_eq!(
                ModelLoadError::from(error).retryable_memory_pressure(),
                limit != 1
            );
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn glb_app_retention_preflight_count_and_memory_retry_preserve_protected_entries() {
        let root = MemoryBudget::new(1024 * 1024);
        let r = request(&root);
        let mut cache = ModelCache::<GlbAsset>::new(rrrah_cache::CacheLimits {
            max_bytes: 600000,
            max_entries: Some(1),
            ttl: None,
        });
        let k = key(&r, None).unwrap();
        let original = cache.load(&r, true, || decode(&r)).unwrap();
        let lease = cache.visible_lease_for(&original).unwrap();
        let mut other = k.clone();
        other.scene = Some(0);
        assert!(
            cache
                .load_with_cancel_resolving_key_mode(
                    || Some(other),
                    false,
                    true,
                    || panic!("protected count must reject before decode"),
                    || false
                )
                .is_err()
        );
        assert!(cache.entries.get(&k).unwrap().ptr_eq(&original));
        drop(original);
        drop(lease);
        cache.entries.unpin(&k);
        cache.visible = None;
        // Make a cold replacement require reclaiming the previous unpinned asset.
        let tight = root.child(470000);
        let rq = request(&tight);
        let mut next = k.clone();
        next.policy += 1;
        let pressure = root.try_reserve(400000).unwrap();
        let replaced = cache
            .load_with_cancel_resolving_key_mode(|| Some(next), false, true, || decode(&rq), || false)
            .unwrap();
        assert_eq!(cache.entries.len(), 1);
        drop(pressure);
        drop(replaced);
        drop(cache);
        assert_eq!(root.used(), 0);
    }
}
