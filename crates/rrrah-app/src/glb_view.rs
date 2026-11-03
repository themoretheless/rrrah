//! Selected GLB scene preparation and offscreen/viewport rendering.
use rrrah_core::{BufferError, MemoryBudget, Reservation};
use rrrah_decode::glb::{GlbAlphaMode, GlbError, GlbSampler, GlbTextureRole};
use rrrah_decode::{DecodeRequest, GlbAsset, GltfTexturePixels};
use rrrah_gpu::{
    GpuResourceLease, MaterialAlpha, MaterialDraw, MaterialParameters, MaterialRenderer, MaterialSampler,
    MaterialTextureUploader, MaterialVertex,
};
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};
#[derive(Debug, thiserror::Error)]
pub enum ViewError {
    #[error(transparent)]
    Source(#[from] rrrah_decode::DecodeError),
    #[error(transparent)]
    Container(#[from] GlbError),
    #[error(transparent)]
    Texture(#[from] rrrah_gpu::MaterialTextureError),
    #[error(transparent)]
    Render(#[from] rrrah_gpu::MaterialRenderError),
    #[error(transparent)]
    Memory(#[from] BufferError),
    #[error("GLB view cancelled")]
    Cancelled,
    #[error("unsupported GLB view: {0}")]
    Unsupported(&'static str),
}
struct Owned<T> {
    items: Vec<T>,
    _credit: Reservation,
}
impl<T> Owned<T> {
    fn new(count: usize, budget: &MemoryBudget) -> Result<Self, BufferError> {
        let mut credit =
            budget.try_reserve(count.checked_mul(size_of::<T>()).ok_or(BufferError::Overflow)? as u64)?;
        let mut items = Vec::new();
        items.try_reserve_exact(count).map_err(BufferError::Allocate)?;
        credit.ensure_bytes(
            items
                .capacity()
                .checked_mul(size_of::<T>())
                .ok_or(BufferError::Overflow)? as u64,
        )?;
        Ok(Self {
            items,
            _credit: credit,
        })
    }
}
impl<T> Deref for Owned<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.items
    }
}
impl<T> DerefMut for Owned<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.items
    }
}
#[derive(Clone, Copy)]
struct Plan {
    used: bool,
    mips: bool,
}
struct Entry {
    draw: MaterialDraw,
    center: [f32; 3],
    blend: bool,
    order: usize,
}
pub struct GlbViewport {
    renderer: Arc<MaterialRenderer>,
    device: wgpu::Device,
    budget: MemoryBudget,
    entries: Owned<Entry>,
    depth: Option<(wgpu::Texture, wgpu::TextureView, Arc<Reservation>)>,
    texture_uploads: usize,
}
/// Cloneable worker context sharing texture-mip and material pipelines across scenes.
#[derive(Clone)]
pub struct GlbViewportPreparer {
    uploader: MaterialTextureUploader,
    renderer: Arc<MaterialRenderer>,
    device: wgpu::Device,
    budget: MemoryBudget,
}
impl GlbViewportPreparer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        budget: MemoryBudget,
        target: wgpu::TextureFormat,
    ) -> Result<Self, ViewError> {
        let uploader = MaterialTextureUploader::new(device, queue);
        let renderer = Arc::new(MaterialRenderer::new(uploader.clone(), target)?);
        Ok(Self {
            uploader,
            renderer,
            device: device.clone(),
            budget,
        })
    }
}
fn check(request: &DecodeRequest, cancelled: &mut impl FnMut() -> bool) -> Result<(), ViewError> {
    request.check_cancelled()?;
    if cancelled() {
        return Err(ViewError::Cancelled);
    }
    Ok(())
}
fn sampler(s: GlbSampler) -> MaterialSampler {
    MaterialSampler {
        mag: s.mag_filter,
        min: s.min_filter,
        wrap_s: s.wrap_s,
        wrap_t: s.wrap_t,
    }
}
fn color_role(role: usize) -> bool {
    role == 0 || role == 4
}
fn f32s<const N: usize>(values: [f64; N]) -> Result<[f32; N], ViewError> {
    if values.iter().any(|v| !v.is_finite() || v.abs() > 1e15) {
        return Err(ViewError::Unsupported("GPU attribute arithmetic range"));
    }
    Ok(values.map(|v| v as f32))
}
fn midpoint(b: [[f64; 3]; 2]) -> [f64; 3] {
    std::array::from_fn(|c| b[0][c] * 0.5 + b[1][c] * 0.5)
}
fn unit(v: [f64; 3]) -> Result<[f64; 3], ViewError> {
    let scale = v.iter().map(|v| v.abs()).fold(0., f64::max);
    if scale == 0. || !scale.is_finite() {
        return Err(ViewError::Unsupported("degenerate tangent frame"));
    }
    let v = v.map(|x| x / scale);
    let n = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    Ok(v.map(|x| x / n))
}
const EMPTY_VERTEX: MaterialVertex = MaterialVertex {
    position: [0.; 3],
    normal: [0.; 3],
    tangent: [0.; 4],
    color: [0.; 4],
    texcoords: [[0.; 2]; 5],
};
impl GlbViewport {
    /// Builds a new candidate without changing an installed/visible viewport.
    /// Source geometry stays shared in the asset; this first GPU path packs
    /// world-space vertices per instance. GPU instancing is a remaining optimization.
    pub fn prepare(
        asset: &GlbAsset,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        budget: MemoryBudget,
        target: wgpu::TextureFormat,
        request: &DecodeRequest,
        cancelled: impl FnMut() -> bool,
    ) -> Result<Self, ViewError> {
        GlbViewportPreparer::new(device, queue, budget, target)?.prepare(asset, request, cancelled)
    }
}
impl GlbViewportPreparer {
    pub fn prepare(
        &self,
        asset: &GlbAsset,
        request: &DecodeRequest,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<GlbViewport, ViewError> {
        check(request, &mut cancelled)?;
        let uploader = self.uploader.clone();
        let renderer = self.renderer.clone();
        let device = &self.device;
        let budget = self.budget.clone();
        let origin = asset.bounds().map_or([0.; 3], midpoint);
        let mut bounds: Option<[[f64; 3]; 2]> = None;
        let mut count = 0usize;
        for instance in asset.scene().instances() {
            let transform = asset.scene().rebased_transform(instance.node, origin, request)?;
            for primitive in asset
                .primitives_for_mesh(instance.mesh)
                .ok_or(ViewError::Unsupported("selected mesh plan"))?
            {
                check(request, &mut cancelled)?;
                count = count
                    .checked_add(1)
                    .filter(|n| *n <= 1_048_576)
                    .ok_or(ViewError::Unsupported("draw count limit"))?;
                for corner in 0..8 {
                    let p = transform.point(std::array::from_fn(|c| {
                        primitive.local_bounds[(corner >> c) & 1][c]
                    }))?;
                    if let Some(b) = &mut bounds {
                        for c in 0..3 {
                            b[0][c] = b[0][c].min(p[c]);
                            b[1][c] = b[1][c].max(p[c]);
                        }
                    } else {
                        bounds = Some([p, p]);
                    }
                }
            }
        }
        let center = bounds.map_or([0.; 3], midpoint);
        let radius = bounds.map_or(1., |b| {
            (0..3).map(|c| b[1][c] * 0.5 - b[0][c] * 0.5).fold(0., f64::max)
        });
        let radius = if radius == 0. { 1. } else { radius };
        let normalize = |point: [f64; 3]| f32s(std::array::from_fn(|c| (point[c] - center[c]) / radius));
        if count == 0 {
            return Ok(GlbViewport {
                renderer,
                device: device.clone(),
                budget: budget.clone(),
                entries: Owned::new(0, &budget)?,
                depth: None,
                texture_uploads: 0,
            });
        }
        // Borrowed layouts are resolved once per source primitive, not per instance.
        let mut resolved = Owned::new(asset.primitives().len(), &budget)?;
        for primitive in asset.primitives() {
            check(request, &mut cancelled)?;
            let stream = asset
                .scene()
                .document()
                .primitive(primitive.mesh, primitive.primitive, request)?
                .vertex_streams(request)?;
            if primitive.material.normal_texture.is_some() {
                let vertex = stream.vertex(0, request)?;
                if vertex.normal.is_none() || vertex.tangent.is_none() {
                    return Err(ViewError::Unsupported(
                        "normal maps without a supplied tangent frame require MikkTSpace",
                    ));
                }
            }
            resolved.items.push(stream);
        }
        let slots = asset.images().len().checked_mul(2).ok_or(BufferError::Overflow)?;
        let mut plan = budget.try_buffer(
            slots,
            Plan {
                used: false,
                mips: false,
            },
        )?;
        for primitive in asset.primitives() {
            for (role, binding) in primitive.textures.iter().enumerate() {
                if let Some(binding) = binding {
                    let slot = binding.image * 2 + usize::from(color_role(role));
                    plan[slot].used = true;
                    plan[slot].mips |= !matches!(binding.sampler.min_filter, Some(9728 | 9729));
                }
            }
        }
        let mut images = Owned::new(slots, &budget)?;
        images.items.resize_with(slots, || None);
        let mut texture_uploads = 0;
        for (slot, p) in plan.iter().enumerate() {
            check(request, &mut cancelled)?;
            if !p.used {
                continue;
            }
            let image = asset.images()[slot / 2].for_role(if slot % 2 == 1 {
                GlbTextureRole::BaseColor
            } else {
                GlbTextureRole::Normal
            });
            let parameters = MaterialSampler {
                min: if p.mips { None } else { Some(9729) },
                ..Default::default()
            };
            let stop = || cancelled() || request.check_cancelled().is_err();
            let texture = match image.pixels() {
                GltfTexturePixels::Rgba8(data) => uploader.upload_rgba8(
                    &budget,
                    [image.width(), image.height()],
                    data,
                    slot % 2 == 1,
                    parameters,
                    stop,
                )?,
                GltfTexturePixels::Rgba16(data) => uploader.upload_rgba16(
                    &budget,
                    [image.width(), image.height()],
                    data,
                    slot % 2 == 1,
                    parameters,
                    stop,
                )?,
            };
            images.items[slot] = Some(texture);
            texture_uploads += 1;
        }
        let white_color = uploader.upload_rgba8(
            &budget,
            [1, 1],
            &[255; 4],
            true,
            MaterialSampler::default(),
            || cancelled() || request.check_cancelled().is_err(),
        )?;
        let white_data = uploader.upload_rgba8(
            &budget,
            [1, 1],
            &[255; 4],
            false,
            MaterialSampler::default(),
            || cancelled() || request.check_cancelled().is_err(),
        )?;
        let mut materials = Owned::new(asset.primitives().len(), &budget)?;
        for primitive in asset.primitives() {
            check(request, &mut cancelled)?;
            let mut textures = [
                white_color.clone(),
                white_data.clone(),
                white_data.clone(),
                white_data.clone(),
                white_color.clone(),
            ];
            for (role, binding) in primitive.textures.iter().enumerate() {
                if let Some(binding) = binding {
                    let slot = binding.image * 2 + usize::from(color_role(role));
                    textures[role] = uploader.with_sampler(
                        images[slot]
                            .as_ref()
                            .ok_or(ViewError::Unsupported("missing image plan"))?,
                        sampler(binding.sampler),
                    )?;
                }
            }
            materials.items.push(textures);
        }
        let mut entries = Owned::new(count, &budget)?;
        for instance in asset.scene().instances() {
            check(request, &mut cancelled)?;
            let transform = asset.scene().rebased_transform(instance.node, origin, request)?;
            let orientation = transform.orientation()?;
            for plan_index in asset.primitive_range(instance.mesh).unwrap() {
                let primitive = &asset.primitives()[plan_index];
                check(request, &mut cancelled)?;
                let streams = resolved[plan_index];
                let vertices = primitive.triangles.checked_mul(3).ok_or(BufferError::Overflow)?;
                let mut packed = budget.try_buffer(vertices, EMPTY_VERTEX)?;
                let normal_map = primitive.material.normal_texture.is_some();
                for triangle in 0..primitive.triangles {
                    check(request, &mut cancelled)?;
                    let corners = streams
                        .triangle(triangle, request)?
                        .ok_or(ViewError::Unsupported("triangle plan"))?;
                    for (corner, v) in corners.into_iter().enumerate() {
                        let normal = v.normal.map(|n| transform.normal(n)).transpose()?;
                        let tangent = if normal_map {
                            let (Some(normal), Some(tangent)) = (normal, v.tangent) else {
                                return Err(ViewError::Unsupported(
                                    "normal maps without a supplied tangent frame require MikkTSpace",
                                ));
                            };
                            let t = transform.direction([tangent[0], tangent[1], tangent[2]])?;
                            let dot = (0..3).map(|c| normal[c] * t[c]).sum::<f64>();
                            let t = unit(std::array::from_fn(|c| t[c] - normal[c] * dot))?;
                            [t[0], t[1], t[2], tangent[3] * f64::from(orientation)]
                        } else {
                            [0.; 4]
                        };
                        packed[triangle * 3 + corner] = MaterialVertex {
                            position: normalize(transform.point(v.position)?)?,
                            normal: f32s(normal.unwrap_or([0.; 3]))?,
                            tangent: f32s(tangent)?,
                            color: f32s(v.color)?,
                            texcoords: [
                                f32s(v.texcoords[0].unwrap_or([0.; 2]))?,
                                f32s(v.texcoords[1].unwrap_or([0.; 2]))?,
                                f32s(v.texcoords[2].unwrap_or([0.; 2]))?,
                                f32s(v.texcoords[3].unwrap_or([0.; 2]))?,
                                f32s(v.texcoords[4].unwrap_or([0.; 2]))?,
                            ],
                        };
                    }
                }
                let m = primitive.material;
                let mut parameters = MaterialParameters::default();
                parameters.base_color = f32s(m.base_color)?;
                let e = f32s(m.emissive)?;
                parameters.emissive = [e[0], e[1], e[2], 1.];
                parameters.factors = f32s([
                    m.metallic,
                    m.roughness,
                    m.normal_texture.map_or(1., |v| v.scale),
                    m.occlusion_texture.map_or(1., |v| v.strength),
                ])?;
                parameters.alpha = [
                    f32s([m.alpha_cutoff])?[0],
                    0.,
                    f32::from(u8::from(normal_map)),
                    0.,
                ];
                parameters.light_direction = [0.3, 0.6, 1., 3.];
                let alpha = match m.alpha_mode {
                    GlbAlphaMode::Opaque => MaterialAlpha::Opaque,
                    GlbAlphaMode::Mask => MaterialAlpha::Mask,
                    GlbAlphaMode::Blend => MaterialAlpha::Blend,
                };
                let textures = std::array::from_fn(|i| &materials[plan_index][i]);
                let draw = renderer.prepare(
                    &budget,
                    &packed,
                    parameters,
                    textures,
                    alpha,
                    m.double_sided,
                    orientation < 0,
                    || cancelled() || request.check_cancelled().is_err(),
                )?;
                entries.items.push(Entry {
                    draw,
                    center: normalize(transform.point(midpoint(primitive.local_bounds))?)?,
                    blend: matches!(m.alpha_mode, GlbAlphaMode::Blend),
                    order: entries.len(),
                });
            }
        }
        check(request, &mut cancelled)?;
        let mut view = GlbViewport {
            renderer,
            device: device.clone(),
            budget,
            entries,
            depth: None,
            texture_uploads,
        };
        view.update_view(1., 0., 0., 1.)?;
        Ok(view)
    }
}
impl GlbViewport {
    pub fn draw_count(&self) -> usize {
        self.entries.len()
    }
    pub fn texture_upload_count(&self) -> usize {
        self.texture_uploads
    }
    pub fn resize(&mut self, size: [u32; 2]) -> Result<(), ViewError> {
        if size.contains(&0)
            || size
                .iter()
                .any(|v| *v > self.device.limits().max_texture_dimension_2d)
        {
            return Err(ViewError::Unsupported("viewport dimensions"));
        }
        if self
            .depth
            .as_ref()
            .is_some_and(|(texture, _, _)| [texture.width(), texture.height()] == size)
        {
            return Ok(());
        }
        let credit = Arc::new(
            self.budget
                .try_reserve(u64::from(size[0]) * u64::from(size[1]) * 4)?,
        );
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GLB viewport depth"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        self.depth = Some((texture, view, credit));
        Ok(())
    }
    pub fn update_view(&mut self, aspect: f32, yaw: f32, pitch: f32, zoom: f32) -> Result<(), ViewError> {
        if [aspect, yaw, pitch, zoom].into_iter().any(|v| !v.is_finite()) || aspect <= 0. || zoom <= 0. {
            return Err(ViewError::Unsupported("camera values"));
        }
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        let scale = 0.8 * zoom.clamp(0.02, 128.);
        let rows = [[cy, 0., sy], [sp * sy, cp, -sp * cy], [-cp * sy, sp, cp * cy]];
        let clip = std::array::from_fn(|c| {
            if c == 3 {
                [0., 0., 0.5, 1.]
            } else {
                [
                    rows[0][c] * scale / aspect,
                    rows[1][c] * scale,
                    -rows[2][c] * 0.25,
                    0.,
                ]
            }
        });
        let eye = [rows[2][0], rows[2][1], rows[2][2], 0.03];
        self.renderer
            .update_views(&self.budget, self.entries.iter().map(|e| &e.draw), clip, eye)?;
        self.entries.items.sort_unstable_by(|a, b| {
            a.blend.cmp(&b.blend).then_with(|| {
                if a.blend {
                    let depth = |e: &Entry| (0..3).map(|c| e.center[c] * rows[2][c]).sum::<f32>();
                    depth(a).total_cmp(&depth(b)).then(a.order.cmp(&b.order))
                } else {
                    a.order.cmp(&b.order)
                }
            })
        });
        Ok(())
    }
    pub fn resource_lease(&self) -> GpuResourceLease {
        let mut lease = GpuResourceLease::default();
        for entry in &self.entries.items {
            lease.extend(entry.draw.resource_lease());
        }
        if let Some((_, _, credit)) = &self.depth {
            lease.extend(GpuResourceLease::from_reservation(credit.clone()));
        }
        lease
    }
    pub fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        background: wgpu::Color,
    ) -> Result<(), ViewError> {
        let (_, depth, _) = self
            .depth
            .as_ref()
            .ok_or(ViewError::Unsupported("viewport has not been resized"))?;
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("GLB material viewport"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(background),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        for entry in &self.entries.items {
            self.renderer.encode(&entry.draw, &mut pass)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(path: impl Into<std::path::PathBuf>, budget: &MemoryBudget) -> DecodeRequest {
        let mut r = DecodeRequest::new(path.into());
        r.memory_budget = Some(budget.clone());
        r
    }
    fn fixture() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/glb/BoxTextured.glb")
    }
    fn frame(
        view: &mut GlbViewport,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        camera: [f32; 3],
    ) -> Vec<u8> {
        view.resize([128, 128]).unwrap();
        view.update_view(1., camera[0], camera[1], camera[2]).unwrap();
        read_frame(view, device, queue, format)
    }
    fn read_frame(
        view: &mut GlbViewport,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Vec<u8> {
        let size = 128u32;
        view.resize([size, size]).unwrap();
        let bytes = if format == wgpu::TextureFormat::Rgba16Float {
            8
        } else {
            4
        };
        let row = size * bytes;
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GLB viewport proof"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(row) * u64::from(size),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        view.encode(
            &mut encoder,
            &target.create_view(&Default::default()),
            wgpu::Color::TRANSPARENT,
        )
        .unwrap();
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &read,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(size),
                },
            },
            target.size(),
        );
        let lease = view.resource_lease();
        queue.submit([encoder.finish()]);
        lease.retain_until_complete(queue);
        let (tx, rx) = std::sync::mpsc::channel();
        read.slice(..)
            .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let mapped = read.slice(..).get_mapped_range().unwrap();
        let bytes = mapped.to_vec();
        drop(mapped);
        read.unmap();
        bytes
    }
    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "actual Metal original GLB viewport, camera, snapshot and resource ownership"]
    fn glb_viewport_metal_original_snapshot_camera_and_failure_atomicity() {
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual Metal required");
        let device = &gpu.device;
        let queue = &gpu.queue;
        let budget = MemoryBudget::new(8 * 1024 * 1024);
        let request = request(fixture(), &budget);
        let asset = rrrah_decode::decode_glb_asset(&request, None).unwrap();
        let mut snapshot = Vec::new();
        rrrah_decode::write_glb_asset_swap_payload(&asset, &mut snapshot, &request).unwrap();
        let cameras = [[0., 0., 1.], [0.5, 0.2, 0.8], [-0.7, -0.4, 1.2]];
        let mut sdr_reference: Option<Vec<Vec<u8>>> = None;
        for format in [
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureFormat::Rgba16Float,
        ] {
            let preparer = GlbViewportPreparer::new(device, queue, budget.clone(), format).unwrap();
            let mut view = preparer.prepare(&asset, &request, || false).unwrap();
            assert_eq!(view.draw_count(), 1);
            assert_eq!(view.texture_upload_count(), 1);
            let before: Vec<_> = cameras
                .into_iter()
                .map(|camera| frame(&mut view, device, queue, format, camera))
                .collect();
            assert_ne!(before[0], before[1]);
            assert_ne!(before[1], before[2]);
            if format == wgpu::TextureFormat::Rgba8UnormSrgb {
                assert!(before[0].chunks_exact(4).filter(|p| p[3] != 0).count() > 1000);
                if let Some(path) = std::env::var_os("RRRAH_GLB_VIEW_RGBA") {
                    std::fs::write(path, &before[1]).unwrap();
                }
            }
            if format == wgpu::TextureFormat::Rgba8UnormSrgb {
                sdr_reference = Some(before.clone());
            } else {
                let decode_half = |bits: u16| {
                    let e = (bits >> 10) & 31;
                    let f = bits & 1023;
                    assert_eq!(bits >> 15, 0);
                    assert!(e < 31);
                    if e == 0 {
                        f64::from(f) * 2f64.powi(-24)
                    } else {
                        (1. + f64::from(f) / 1024.) * 2f64.powi(i32::from(e) - 15)
                    }
                };
                for (linear, encoded) in before.iter().zip(sdr_reference.as_ref().unwrap()) {
                    for (i, (sample, &expected)) in linear.chunks_exact(2).zip(encoded.iter()).enumerate() {
                        let value = decode_half(u16::from_ne_bytes(sample.try_into().unwrap()));
                        let encoded = if i % 4 == 3 {
                            value
                        } else if value <= 0.0031308 {
                            12.92 * value
                        } else {
                            1.055 * value.powf(1. / 2.4) - 0.055
                        };
                        assert!(
                            ((encoded.clamp(0., 1.) * 255.).round() as u8).abs_diff(expected) <= 2,
                            "HDR/SDR transfer pixel channel {i}"
                        );
                    }
                }
            }
            let used = budget.used();
            assert!(
                GlbViewport::prepare(&asset, device, queue, budget.clone(), format, &request, || true)
                    .is_err()
            );
            assert_eq!(budget.used(), used);
            let mut steps = 0;
            assert!(
                preparer
                    .prepare(&asset, &request, || {
                        steps += 1;
                        steps >= 30
                    })
                    .is_err()
            );
            assert_eq!(read_frame(&mut view, device, queue, format), before[2]);
            assert_eq!(budget.used(), used);
            let pressure = budget.try_reserve(budget.available_bytes()).unwrap();
            assert!(view.update_view(1., 1., 0.2, 1.).is_err());
            drop(pressure);
            assert_eq!(read_frame(&mut view, device, queue, format), before[2]);
            drop(view);
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let restored = rrrah_decode::read_glb_asset_swap_payload(
                &mut &snapshot[..],
                snapshot.len() as u64,
                &request,
            )
            .unwrap();
            let mut view = preparer.clone().prepare(&restored, &request, || false).unwrap();
            for (camera, expected) in cameras.into_iter().zip(&before) {
                assert_eq!(&frame(&mut view, device, queue, format, camera), expected);
            }
            drop(view);
            drop(restored);
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            eprintln!(
                "GLB viewport {format:?}: original producer, 3 camera frames differ; swap frames byte-exact; failed candidate/view admission preserves frame"
            );
        }
        drop(asset);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(budget.used(), 0);
    }
    #[test]
    #[ignore = "actual Metal asynchronous GLB/legacy worker publication proof"]
    fn glb_viewport_metal_shared_upload_worker_frames_and_replacement() {
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual Metal required");
        let source = MemoryBudget::new(4 * 1024 * 1024);
        let root = MemoryBudget::new(4 * 1024 * 1024);
        let request = request(fixture(), &source);
        let asset = rrrah_decode::decode_glb_asset(&request, None).unwrap();
        let generation = Arc::new(std::sync::atomic::AtomicU64::new(7));
        let token = || rrrah_decode::GenerationToken::new(generation.clone(), 7);
        for format in [
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureFormat::Rgba16Float,
        ] {
            let preparer = GlbViewportPreparer::new(&gpu.device, &gpu.queue, root.clone(), format).unwrap();
            let mut reference = preparer.prepare(&asset, &request, || false).unwrap();
            let expected = frame(&mut reference, &gpu.device, &gpu.queue, format, [0.4, -0.2, 1.]);
            drop(reference);
            let mut worker = crate::model_upload_worker::ModelUploadWorker::default();
            let (tx, rx) = std::sync::mpsc::channel();
            worker.set_waker(move || {
                tx.send(()).unwrap();
            });
            let id = worker
                .submit_glb(asset.clone(), preparer.clone(), request.clone(), 7, token())
                .unwrap();
            rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
            assert!(worker.try_take().is_none());
            let completed = worker.try_take_glb().unwrap();
            assert_eq!((completed.id, completed.generation), (id, 7));
            let mut view = completed.result.unwrap();
            assert_eq!(
                frame(&mut view, &gpu.device, &gpu.queue, format, [0.4, -0.2, 1.]),
                expected
            );
            drop(view);
            worker
                .submit_glb(asset.clone(), preparer.clone(), request.clone(), 7, token())
                .unwrap();
            rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
            generation.store(8, std::sync::atomic::Ordering::Release);
            assert!(
                worker.try_take_glb().is_none(),
                "stale GLB completion is discarded"
            );
            generation.store(7, std::sync::atomic::Ordering::Release);
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(root.used(), 0);
            worker
                .submit_glb(asset.clone(), preparer, request.clone(), 7, token())
                .unwrap();
            rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
            let geometry = rrrah_decode::decode_model(&rrrah_decode::DecodeRequest::new(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/models/triangle.stl"),
            ))
            .unwrap();
            worker
                .submit_new(
                    geometry,
                    gpu.device.clone(),
                    format,
                    Some(root.clone()),
                    7,
                    token(),
                )
                .unwrap();
            assert!(
                worker.try_take_glb().is_none(),
                "geometry replacement clears completed GLB"
            );
            rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
            drop(worker.try_take().unwrap().result.unwrap());
            worker.cancel();
            drop(worker);
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(root.used(), 0);
        }
        drop(asset);
        assert_eq!(source.used(), 0);
    }

    fn instances_file(translation: f64) -> tempfile::NamedTempFile {
        let source = std::fs::read(fixture()).unwrap();
        let n = u32::from_le_bytes(source[12..16].try_into().unwrap()) as usize;
        let mut doc: serde_json::Value = serde_json::from_slice(&source[20..20 + n]).unwrap();
        doc["nodes"][0]["matrix"][12] = translation.into();
        let original = doc["nodes"][1].clone();
        doc["nodes"] = serde_json::json!([doc["nodes"][0].clone()]);
        doc["nodes"][0]["children"] = serde_json::json!([1, 2, 3]);
        for x in [-2., 0., 2.] {
            let mut node = original.clone();
            node["translation"] = serde_json::json!([x, 0, 0]);
            doc["nodes"].as_array_mut().unwrap().push(node);
        }
        let mut json = serde_json::to_vec(&doc).unwrap();
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        let mut bytes = b"glTF\x02\0\0\0".to_vec();
        bytes.extend_from_slice(&((source.len() - n + json.len()) as u32).to_le_bytes());
        bytes.extend_from_slice(&(json.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0x4e4f534au32.to_le_bytes());
        bytes.extend(json);
        bytes.extend_from_slice(&source[20 + n..]);
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), bytes).unwrap();
        file
    }
    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "actual Metal GLB instancing/image sharing and rebased 1e24 translation"]
    fn glb_viewport_metal_rebased_instances_share_image_and_preserve_child_offsets() {
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual Metal required");
        let budget = MemoryBudget::new(8 * 1024 * 1024);
        let mut reference = None;
        for translation in [0., 1e24] {
            let file = instances_file(translation);
            let request = request(file.path(), &budget);
            let asset = rrrah_decode::decode_glb_asset(&request, None).unwrap();
            let mut view = GlbViewport::prepare(
                &asset,
                &gpu.device,
                &gpu.queue,
                budget.clone(),
                wgpu::TextureFormat::Rgba8UnormSrgb,
                &request,
                || false,
            )
            .unwrap();
            assert_eq!(view.draw_count(), 3);
            assert_eq!(view.texture_upload_count(), 1);
            let pixels = frame(
                &mut view,
                &gpu.device,
                &gpu.queue,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                [0.3, 0.2, 1.],
            );
            assert!(pixels.chunks_exact(4).filter(|p| p[3] != 0).count() > 100);
            if let Some(expected) = &reference {
                assert_eq!(&pixels, expected);
            } else {
                reference = Some(pixels);
            }
            drop(view);
            drop(asset);
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(budget.used(), 0);
        }
        eprintln!(
            "GLB viewport 3 instances / 1 image upload: rebased 1e24 root preserves small local offsets, exact frame and final root zero"
        );
    }
}
