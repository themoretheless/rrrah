//! Owned selected-scene preparation with shared source geometry and decoded images.
use crate::glb::{GlbDocument, GlbError, GlbImageSource, GlbMaterial, GlbSampler, GlbScene, GlbTextureRole};
use crate::{
    DecodeRequest, GltfTextureError, GltfTextureImage, GltfTexturePixels, decode_gltf_texture_bytes,
};
use rrrah_core::{BufferError, MemoryBudget, PixelBuffer, Reservation, SharedBuffer};
const NONE: usize = usize::MAX;
const MAX_TABLE: usize = 65_536;
const MAX_VERTICES: usize = 16_777_216;
#[derive(Debug, thiserror::Error)]
pub enum GlbAssetError {
    #[error(transparent)]
    Source(#[from] crate::DecodeError),
    #[error(transparent)]
    Container(#[from] GlbError),
    #[error(transparent)]
    Texture(#[from] GltfTextureError),
    #[error(transparent)]
    Memory(#[from] BufferError),
}
#[derive(Debug, Clone, Copy)]
pub struct GlbAssetTexture {
    /// Index in this asset's unique decoded image table (not source texture index).
    pub image: usize,
    pub texcoord: usize,
    pub sampler: GlbSampler,
}
#[derive(Debug, Clone)]
pub struct GlbAssetPrimitive {
    pub mesh: usize,
    pub primitive: usize,
    pub triangles: usize,
    pub material: GlbMaterial,
    /// Base, metallic/roughness, normal, occlusion, emissive bindings.
    pub textures: [Option<GlbAssetTexture>; 5],
    pub local_bounds: [[f64; 3]; 2],
}
#[derive(Debug, Clone)]
pub struct GlbAssetImage {
    pub source_index: usize,
    pixels: GltfTextureImage,
}
impl GlbAssetImage {
    pub fn for_role(&self, role: GlbTextureRole) -> GltfTextureImage {
        self.pixels.with_role(role)
    }
}
#[derive(Debug, Clone, Copy)]
struct MeshSpan {
    first: usize,
    count: usize,
    selected: bool,
}
const EMPTY: MeshSpan = MeshSpan {
    first: 0,
    count: 0,
    selected: false,
};
#[derive(Debug, Clone)]
pub struct GlbAsset {
    scene: GlbScene,
    meshes: PixelBuffer<MeshSpan>,
    primitives: SharedBuffer<GlbAssetPrimitive>,
    images: SharedBuffer<GlbAssetImage>,
    bounds: Option<[[f64; 3]; 2]>,
}
impl GlbAsset {
    pub fn scene(&self) -> &GlbScene {
        &self.scene
    }
    pub fn primitives(&self) -> &[GlbAssetPrimitive] {
        &self.primitives
    }
    pub fn images(&self) -> &[GlbAssetImage] {
        &self.images
    }
    /// Conservative union of transformed local AABBs, not tight per-vertex fitting.
    pub fn bounds(&self) -> Option<[[f64; 3]; 2]> {
        self.bounds
    }
    pub fn primitives_for_mesh(&self, mesh: usize) -> Option<&[GlbAssetPrimitive]> {
        let span = self.meshes.get(mesh).filter(|v| v.selected)?;
        self.primitives.get(span.first..span.first + span.count)
    }
    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.primitives.ptr_eq(&other.primitives)
    }
    /// Retained managed allocations once per asset; excludes inline/Arc/allocator
    /// overhead and untracked codec internals, not a process RSS measurement.
    pub fn capacity_bytes(&self) -> u64 {
        self.scene.capacity_bytes()
            + self.meshes.capacity_bytes()
            + self.primitives.capacity_bytes()
            + self.images.capacity_bytes()
            + self
                .images
                .iter()
                .map(|v| match v.pixels.pixels() {
                    GltfTexturePixels::Rgba8(p) => p.capacity_bytes(),
                    GltfTexturePixels::Rgba16(p) => p.capacity_bytes(),
                })
                .sum::<u64>()
    }
}
struct Table<T> {
    values: Vec<T>,
    credit: Reservation,
}
impl<T> Table<T> {
    fn new(count: usize, budget: &MemoryBudget) -> Result<Self, BufferError> {
        let mut credit =
            budget.try_reserve(count.checked_mul(size_of::<T>()).ok_or(BufferError::Overflow)? as u64)?;
        let mut values = Vec::new();
        values.try_reserve_exact(count).map_err(BufferError::Allocate)?;
        credit.ensure_bytes(
            values
                .capacity()
                .checked_mul(size_of::<T>())
                .ok_or(BufferError::Overflow)? as u64,
        )?;
        Ok(Self { values, credit })
    }
    fn finish(self) -> Result<SharedBuffer<T>, BufferError> {
        self.credit.try_adopt(self.values)
    }
}
fn count(document: &GlbDocument, name: &str, request: &DecodeRequest) -> Result<usize, GlbError> {
    let Some(value) = document.root().field(name, request)? else {
        return Ok(0);
    };
    let mut n = 0;
    for _ in value.array().ok_or(GlbError::Invalid("asset table array"))? {
        request.check_cancelled()?;
        n += 1;
        if n > MAX_TABLE {
            return Err(GlbError::Limit("asset table count"));
        }
    }
    Ok(n)
}
/// Read/validate each selected source mesh primitive once, decode each referenced
/// embedded image once. Repeated nodes retain transforms and share geometry/images.
/// This does not register GLB for ordinary viewing or prepare GPU buffers.
pub fn decode_glb_asset(
    request: &DecodeRequest,
    selection: Option<usize>,
) -> Result<GlbAsset, GlbAssetError> {
    request.check_cancelled()?;
    let mut request = request.clone();
    let budget = request
        .memory_budget
        .clone()
        .unwrap_or_else(|| MemoryBudget::new(1024 * 1024 * 1024));
    request.memory_budget = Some(budget.clone());
    let request = &request;
    let document = GlbDocument::read(request)?;
    let scene = document.scene(selection, request)?;
    let mesh_count = count(&document, "meshes", request)?;
    let image_count = count(&document, "images", request)?;
    let mut meshes = budget.try_buffer(mesh_count, EMPTY)?;
    for instance in scene.instances() {
        request.check_cancelled()?;
        meshes[instance.mesh].selected = true;
    }
    let mut primitive_count = 0usize;
    for (index, mesh) in document.meshes(request)?.enumerate() {
        request.check_cancelled()?;
        if !meshes[index].selected {
            continue;
        }
        let count = mesh.primitive_count(request)?;
        meshes[index].first = primitive_count;
        meshes[index].count = count;
        primitive_count = primitive_count
            .checked_add(count)
            .filter(|n| *n <= MAX_TABLE)
            .ok_or(GlbError::Limit("asset primitive count"))?;
    }
    let mut primitives = Table::new(primitive_count, &budget)?;
    let mut image_slots = budget.try_buffer(image_count, NONE)?;
    let mut needed = 0;
    let mut vertices = 0usize;
    for (mesh_index, mesh) in document.meshes(request)?.enumerate() {
        request.check_cancelled()?;
        if !meshes[mesh_index].selected {
            continue;
        }
        for (ordinal, primitive) in mesh.primitives(request)?.enumerate() {
            let primitive = primitive?;
            let streams = primitive.vertex_streams(request)?;
            vertices = vertices
                .checked_add(streams.vertex_count())
                .filter(|n| *n <= MAX_VERTICES)
                .ok_or(GlbError::Limit("asset source vertex work"))?;
            let material = streams.material();
            let bindings = [
                material.base_color_texture,
                material.metallic_roughness_texture,
                material.normal_texture.map(|v| v.binding),
                material.occlusion_texture.map(|v| v.binding),
                material.emissive_texture,
            ];
            let mut textures = [None; 5];
            for (role, binding) in bindings.into_iter().enumerate() {
                let Some(binding) = binding else {
                    continue;
                };
                let texture = document.texture(binding.texture, request)?;
                if image_slots[texture.image] == NONE {
                    image_slots[texture.image] = NONE - 1;
                    needed += 1;
                }
                textures[role] = Some(GlbAssetTexture {
                    image: texture.image,
                    texcoord: binding.tex_coord,
                    sampler: texture.sampler,
                });
            }
            // Capacity was admitted and allocated for this exact count before validation.
            if primitives.values.len() == primitive_count {
                return Err(GlbError::Invalid("asset primitive count changed").into());
            }
            primitives.values.push(GlbAssetPrimitive {
                mesh: mesh_index,
                primitive: ordinal,
                triangles: primitive.triangle_count(),
                material,
                textures,
                local_bounds: primitive.bounds(),
            });
        }
    }
    if primitives.values.len() != primitive_count {
        return Err(GlbError::Invalid("asset primitive count changed").into());
    }
    let mut images = Table::new(needed, &budget)?;
    for (source_index, slot) in image_slots.iter_mut().enumerate() {
        request.check_cancelled()?;
        if *slot == NONE {
            continue;
        }
        let pixels = match document.image_source(source_index, request)? {
            GlbImageSource::Embedded { mime, bytes } => {
                decode_gltf_texture_bytes(bytes, mime, GlbTextureRole::Normal, request)?
            }
            GlbImageSource::Uri { .. } => {
                return Err(GltfTextureError::Unsupported("external/data URI image resolution").into());
            }
        };
        *slot = images.values.len();
        images.values.push(GlbAssetImage { source_index, pixels });
    }
    for primitive in &mut primitives.values {
        request.check_cancelled()?;
        for binding in primitive.textures.iter_mut().flatten() {
            binding.image = image_slots[binding.image];
        }
    }
    drop(image_slots);
    let mut bounds: Option<[[f64; 3]; 2]> = None;
    let mut instance_primitives = 0usize;
    for instance in scene.instances() {
        request.check_cancelled()?;
        let span = meshes[instance.mesh];
        instance_primitives = instance_primitives
            .checked_add(span.count)
            .filter(|n| *n <= 1_048_576)
            .ok_or(GlbError::Limit("asset instance primitive work"))?;
        for primitive in &primitives.values[span.first..span.first + span.count] {
            request.check_cancelled()?;
            for corner in 0..8 {
                let point = instance.transform.point(std::array::from_fn(|c| {
                    primitive.local_bounds[(corner >> c) & 1][c]
                }))?;
                if let Some(b) = &mut bounds {
                    for c in 0..3 {
                        b[0][c] = b[0][c].min(point[c]);
                        b[1][c] = b[1][c].max(point[c]);
                    }
                } else {
                    bounds = Some([point, point]);
                }
            }
        }
    }
    request.check_cancelled()?;
    Ok(GlbAsset {
        scene,
        meshes: meshes.freeze().into(),
        primitives: primitives.finish()?,
        images: images.finish()?,
        bounds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/glb")
    }
    fn request(name: &str, budget: &MemoryBudget) -> DecodeRequest {
        let mut r = DecodeRequest::new(root().join(name));
        r.memory_budget = Some(budget.clone());
        r
    }
    fn altered(change: impl FnOnce(&mut serde_json::Value), budget: &MemoryBudget) -> DecodeRequest {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let b = std::fs::read(root().join("BoxTextured.glb")).unwrap();
        let length = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
        let mut doc: serde_json::Value = serde_json::from_slice(&b[20..20 + length]).unwrap();
        change(&mut doc);
        let mut json = serde_json::to_vec(&doc).unwrap();
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        let mut bytes = b"glTF\x02\0\0\0".to_vec();
        bytes.extend_from_slice(&((20 + json.len() + b.len() - 20 - length) as u32).to_le_bytes());
        bytes.extend_from_slice(&(json.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0x4e4f534au32.to_le_bytes());
        bytes.extend(json);
        bytes.extend_from_slice(&b[20 + length..]);
        let path = std::env::temp_dir().join(format!(
            "rrrah-glb-asset-{}-{}.glb",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, bytes).unwrap();
        let mut r = DecodeRequest::new(path);
        r.memory_budget = Some(budget.clone());
        r
    }
    #[test]
    fn original_asset_retained_weight_pixels_and_clone_lifetime_are_exact() {
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let r = request("BoxTextured.glb", &budget);
        let asset = decode_glb_asset(&r, None).unwrap();
        assert_eq!(
            asset.scene().document().source().container_bytes(),
            include_bytes!("../../../tests/fixtures/glb/BoxTextured.glb")
        );
        assert_eq!(asset.primitives().len(), 1);
        assert_eq!(asset.images().len(), 1);
        assert_eq!(asset.primitives()[0].triangles, 12);
        let used = budget.used();
        assert_eq!(asset.capacity_bytes(), used);
        let pixels = asset.images()[0].for_role(GlbTextureRole::BaseColor);
        let GltfTexturePixels::Rgba8(p) = pixels.pixels() else {
            panic!()
        };
        assert_eq!(
            &p[..],
            include_bytes!("../../../tests/fixtures/glb/texture-codec/BoxTextured.rgba")
        );
        let clone = asset.clone();
        assert!(asset.ptr_eq(&clone));
        assert_eq!(budget.used(), used);
        drop(asset);
        assert_eq!(budget.used(), used);
        drop(pixels);
        drop(clone);
        assert_eq!(budget.used(), 0);
        eprintln!(
            "GLB asset original retained={used} managed_peak={}",
            budget.peak()
        );
    }
    #[test]
    fn repeated_instances_and_color_data_sampler_aliases_share_one_image_and_primitive() {
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let r = altered(
            |d| {
                d["nodes"] = serde_json::json!([{"mesh":0},{"mesh":0,"translation":[2,0,0]},{"mesh":0,"translation":[-2,0,0]}]);
                d["scenes"] = serde_json::json!([{"nodes":[0,1,2]}]);
                d["scene"] = 0.into();
                let sampler = d["samplers"].as_array().unwrap().len();
                d["samplers"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"magFilter":9728,"minFilter":9728,"wrapS":33071,"wrapT":33648}));
                d["textures"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"source":0,"sampler":sampler}));
                d["materials"][0]["normalTexture"] = serde_json::json!({"index":1});
                // Inactive unsupported content must not be mistaken for selected geometry.
                let mut unused = d["meshes"][0].clone();
                unused["primitives"][0]["mode"] = 0.into();
                d["meshes"].as_array_mut().unwrap().push(unused);
                d["images"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"uri":"unselected.png"}));
            },
            &budget,
        );
        let asset = decode_glb_asset(&r, None).unwrap();
        std::fs::remove_file(&r.path).unwrap();
        assert_eq!(asset.scene().instances().count(), 3);
        assert_eq!(asset.primitives().len(), 1);
        assert_eq!(asset.images().len(), 1);
        assert!(asset.primitives_for_mesh(1).is_none());
        let p = &asset.primitives()[0];
        assert_eq!(p.textures[0].unwrap().image, p.textures[2].unwrap().image);
        assert_eq!(p.textures[0].unwrap().sampler.mag_filter, Some(9729));
        assert_eq!(p.textures[2].unwrap().sampler.mag_filter, Some(9728));
        assert_eq!(asset.bounds(), Some([[-2.5, -0.5, -0.5], [2.5, 0.5, 0.5]]));
        let color = asset.images()[0].for_role(GlbTextureRole::BaseColor);
        let data = asset.images()[0].for_role(GlbTextureRole::Normal);
        let (GltfTexturePixels::Rgba8(a), GltfTexturePixels::Rgba8(b)) = (color.pixels(), data.pixels())
        else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        assert_eq!(asset.capacity_bytes(), budget.used());
        drop(color);
        drop(data);
        drop(asset);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn quota_cancellation_selected_errors_and_partial_image_failure_release_every_owner() {
        let probe = MemoryBudget::new(4 * 1024 * 1024);
        let r = request("BoxTextured.glb", &probe);
        drop(decode_glb_asset(&r, None).unwrap());
        let peak = probe.peak();
        assert_eq!(probe.used(), 0);
        for (limit, success) in [(peak - 1, false), (peak, true)] {
            let budget = MemoryBudget::new(limit);
            let r = request("BoxTextured.glb", &budget);
            let result = decode_glb_asset(&r, None);
            assert_eq!(result.is_ok(), success);
            drop(result);
            assert_eq!(budget.used(), 0);
        }
        let mut r = request("BoxTextured.glb", &probe);
        let token = std::sync::Arc::new(AtomicU64::new(2));
        r.cancellation = Some(crate::GenerationToken::new(token.clone(), 1));
        assert!(decode_glb_asset(&r, None).is_err());
        assert_eq!(probe.used(), 0);
        r.cancellation = Some(crate::GenerationToken::new(token, 2));
        drop(decode_glb_asset(&r, None).unwrap());
        assert_eq!(probe.used(), 0);
        let r = altered(|d| d["meshes"][0]["primitives"][0]["mode"] = 0.into(), &probe);
        assert!(decode_glb_asset(&r, None).is_err());
        std::fs::remove_file(&r.path).unwrap();
        assert_eq!(probe.used(), 0);
        let empty = altered(|d| d["meshes"][0]["primitives"] = serde_json::json!([]), &probe);
        assert!(decode_glb_asset(&empty, None).is_err());
        std::fs::remove_file(&empty.path).unwrap();
        assert_eq!(probe.used(), 0);
        let r = altered(
            |d| {
                d["images"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"uri":"unresolved.png","mimeType":"image/png"}));
                d["textures"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"source":1}));
                d["materials"][0]["emissiveTexture"] = serde_json::json!({"index":1});
            },
            &probe,
        );
        assert!(matches!(
            decode_glb_asset(&r, None),
            Err(GlbAssetError::Texture(GltfTextureError::Unsupported(_)))
        ));
        std::fs::remove_file(&r.path).unwrap();
        assert_eq!(probe.used(), 0);
        eprintln!(
            "GLB asset exact managed peak {peak}, one-byte refusal, retry/cancel/partial image cleanup passed"
        );
    }
    #[test]
    fn asset_cache_leases_size_count_and_ttl_keep_the_source_and_pixels_until_last_owner() {
        use rrrah_memory::{CacheLimits, LeaseCache};
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let r = request("BoxTextured.glb", &budget);
        let asset = decode_glb_asset(&r, None).unwrap();
        let weight = asset.capacity_bytes();
        let mut cache = LeaseCache::new(CacheLimits {
            max_bytes: weight,
            max_entries: Some(1),
            ttl: None,
        });
        assert!(cache.insert(0, asset, weight).unwrap().is_empty());
        let lease = cache.get(&0).unwrap();
        let shared = lease.clone();
        assert!(lease.ptr_eq(&shared));
        cache
            .set_limits(CacheLimits {
                max_bytes: weight,
                max_entries: Some(1),
                ttl: Some(std::time::Duration::ZERO),
            })
            .unwrap();
        assert_eq!(cache.prune_expired(), 0);
        drop(lease);
        assert_eq!(cache.prune_expired(), 0);
        drop(shared);
        // Policy TTL changes affect future insertions, not existing deadlines.
        assert_eq!(cache.prune_expired(), 0);
        let retained = cache.remove(&0).unwrap();
        cache.insert(1, retained.clone(), weight).unwrap();
        assert_eq!(cache.prune_expired(), 1);
        assert!(cache.get(&1).is_none());
        assert_eq!(budget.used(), weight); // consumer clone survives TTL eviction
        drop(retained);
        assert_eq!(budget.used(), 0);
        let asset = decode_glb_asset(&r, None).unwrap();
        let mut small = LeaseCache::new(CacheLimits::bytes(weight - 1));
        let refused = small.insert(0, asset, weight).unwrap_err();
        assert_eq!(budget.used(), weight);
        drop(refused);
        assert_eq!(budget.used(), 0);
        let asset = decode_glb_asset(&r, None).unwrap();
        let mut cache = LeaseCache::new(CacheLimits {
            max_bytes: weight * 2,
            max_entries: Some(1),
            ttl: None,
        });
        cache.insert(0, asset, weight).unwrap();
        let lease = cache.get(&0).unwrap();
        let next = decode_glb_asset(&r, None).unwrap();
        let next = cache.insert(1, next, weight).unwrap_err();
        assert_eq!(budget.used(), 2 * weight);
        drop(lease);
        let victims = cache.insert(1, next, weight).unwrap();
        assert_eq!(victims.len(), 1);
        drop(victims);
        assert_eq!(budget.used(), weight);
        drop(cache);
        assert_eq!(budget.used(), 0);
    }
}
