//! Material texture upload; encoded color is decoded by sRGB views before filtering.
use rrrah_core::{MemoryBudget, Reservation};
use std::sync::{Arc, OnceLock};

#[derive(Debug, thiserror::Error)]
pub enum MaterialTextureError {
    #[error("invalid material texture dimensions or RGBA byte count")]
    InvalidLayout,
    #[error("unsupported glTF sampler enum")]
    InvalidSampler,
    #[error("material texture upload cancelled")]
    Cancelled,
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
}

/// glTF core sampler fields. Defaults select linear magnification and trilinear minification.
#[derive(Debug, Clone, Copy)]
pub struct MaterialSampler {
    pub mag: Option<u32>,
    pub min: Option<u32>,
    pub wrap_s: u32,
    pub wrap_t: u32,
}
impl Default for MaterialSampler {
    fn default() -> Self {
        Self {
            mag: None,
            min: None,
            wrap_s: 10497,
            wrap_t: 10497,
        }
    }
}
impl MaterialSampler {
    fn descriptor(self) -> Result<wgpu::SamplerDescriptor<'static>, MaterialTextureError> {
        use wgpu::{AddressMode as A, FilterMode as F, MipmapFilterMode as M};
        let wrap = |value| match value {
            10497 => Ok(A::Repeat),
            33071 => Ok(A::ClampToEdge),
            33648 => Ok(A::MirrorRepeat),
            _ => Err(MaterialTextureError::InvalidSampler),
        };
        let mag_filter = match self.mag.unwrap_or(9729) {
            9728 => F::Nearest,
            9729 => F::Linear,
            _ => return Err(MaterialTextureError::InvalidSampler),
        };
        let (min_filter, mipmap_filter, lod_max_clamp) = match self.min.unwrap_or(9987) {
            9728 => (F::Nearest, M::Nearest, 0.),
            9729 => (F::Linear, M::Nearest, 0.),
            9984 => (F::Nearest, M::Nearest, 32.),
            9985 => (F::Linear, M::Nearest, 32.),
            9986 => (F::Nearest, M::Linear, 32.),
            9987 => (F::Linear, M::Linear, 32.),
            _ => return Err(MaterialTextureError::InvalidSampler),
        };
        Ok(wgpu::SamplerDescriptor {
            label: Some("glTF material sampler"),
            address_mode_u: wrap(self.wrap_s)?,
            address_mode_v: wrap(self.wrap_t)?,
            mag_filter,
            min_filter,
            mipmap_filter,
            lod_max_clamp,
            ..Default::default()
        })
    }
}

/// Reusable device/queue-bound upload context. Clones share lazy mip pipelines.
/// Driver pipeline allocations are not included in per-texture logical credits.
#[derive(Debug, Clone)]
pub struct MaterialTextureUploader {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipelines: Arc<[OnceLock<wgpu::RenderPipeline>; 2]>,
}
impl MaterialTextureUploader {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            device: device.clone(),
            queue: queue.clone(),
            pipelines: Arc::new(std::array::from_fn(|_| OnceLock::new())),
        }
    }
}

/// Immutable candidate: publication/replacement is owned by the caller.
/// GPU use after upload must retain `resource_lease` until the draw completes.
#[derive(Debug)]
pub struct MaterialTexture {
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    credit: Arc<Reservation>,
}
impl MaterialTexture {
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
    pub fn resource_lease(&self) -> crate::GpuResourceLease {
        crate::GpuResourceLease::new([self.credit.clone()])
    }
}
impl MaterialTextureUploader {
    /// Upload straight RGBA8. Color roles use sRGB RGB; alpha and data remain linear.
    /// Resident mip texels and aligned queued upload bytes are admitted separately.
    pub fn upload_rgba8(
        &self,
        budget: &MemoryBudget,
        size: [u32; 2],
        pixels: &[u8],
        srgb: bool,
        sampler: MaterialSampler,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<MaterialTexture, MaterialTextureError> {
        let device = &self.device;
        let queue = &self.queue;
        if cancelled() {
            return Err(MaterialTextureError::Cancelled);
        }
        let descriptor = sampler.descriptor()?;
        let [width, height] = size;
        if width == 0
            || height == 0
            || width > device.limits().max_texture_dimension_2d
            || height > device.limits().max_texture_dimension_2d
            || u64::from(width) * u64::from(height) * 4 != pixels.len() as u64
        {
            return Err(MaterialTextureError::InvalidLayout);
        }
        let mip_count = if descriptor.lod_max_clamp == 0. {
            1
        } else {
            width.max(height).ilog2() + 1
        };
        let bytes: u64 = (0..mip_count)
            .map(|m| u64::from((width >> m).max(1)) * u64::from((height >> m).max(1)) * 4)
            .sum();
        let credit = Arc::new(budget.try_reserve(bytes)?);
        let row = (u64::from(width) * 4).div_ceil(256) * 256;
        let queued = budget.try_reserve(row * u64::from(height))?;
        let _upload = crate::QueuedUploadReservation {
            queue: queue.clone(),
            reservation: Some(queued),
            resources: crate::GpuResourceLease::new([credit.clone()]),
        };
        let format = if srgb {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glTF material texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        if cancelled() {
            return Err(MaterialTextureError::Cancelled);
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        if mip_count > 1 {
            let pipeline = self.pipelines[usize::from(srgb)].get_or_init(|| mip_pipeline(device, format));
            generate_mips(device, queue, &texture, pipeline, mip_count, &mut cancelled)?;
        }
        if cancelled() {
            return Err(MaterialTextureError::Cancelled);
        }
        let view = texture.create_view(&Default::default());
        Ok(MaterialTexture {
            texture,
            view,
            sampler: device.create_sampler(&descriptor),
            credit,
        })
    }
}

fn mip_pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("material linear mip filter"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/material_mip.wgsl").into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("material mip generation"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn generate_mips(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    pipeline: &wgpu::RenderPipeline,
    count: u32,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<(), MaterialTextureError> {
    let mut encoder = device.create_command_encoder(&Default::default());
    for mip in 1..count {
        if cancelled() {
            return Err(MaterialTextureError::Cancelled);
        }
        let view = |base| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: base,
                mip_level_count: Some(1),
                ..Default::default()
            })
        };
        let input = view(mip - 1);
        let output = view(mip);
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&input),
            }],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("material mip"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &output,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.draw(0..3, 0..1);
    }
    if cancelled() {
        return Err(MaterialTextureError::Cancelled);
    }
    queue.submit([encoder.finish()]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn material_mip_shader_validates() {
        let module = naga::front::wgsl::parse_str(include_str!("../shaders/material_mip.wgsl")).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap();
    }
    #[test]
    fn sampler_all_core_modes_and_rejections() {
        for min in [9728, 9729, 9984, 9985, 9986, 9987] {
            for wrap in [10497, 33071, 33648] {
                let d = MaterialSampler {
                    min: Some(min),
                    wrap_s: wrap,
                    wrap_t: wrap,
                    ..Default::default()
                }
                .descriptor()
                .unwrap();
                assert_eq!(d.lod_max_clamp == 0., min < 9984);
            }
        }
        for s in [
            MaterialSampler {
                mag: Some(9987),
                ..Default::default()
            },
            MaterialSampler {
                min: Some(0),
                ..Default::default()
            },
            MaterialSampler {
                wrap_s: 0,
                ..Default::default()
            },
        ] {
            assert!(matches!(
                s.descriptor(),
                Err(MaterialTextureError::InvalidSampler)
            ));
        }
    }
    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "actual Metal material sRGB/data mip readback and admission"]
    fn material_texture_metal_readback_and_budget() {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(desc);
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        assert_eq!(adapter.get_info().backend, wgpu::Backend::Metal);
        eprintln!("MATERIAL_TEXTURE_ADAPTER {:?}", adapter.get_info());
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let uploader = MaterialTextureUploader::new(&device, &queue);
        let uploader = uploader.clone();
        // Black/white checker: linear RGB average .5 encodes near 188 in sRGB,
        // whereas data averages to 128. Alpha must stay linear in either role.
        let pixels = [0, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 0];
        // Non-mip sampling requires no shader/pipeline construction.
        let non_mip_budget = MemoryBudget::new(528);
        let non_mip = uploader
            .upload_rgba8(
                &non_mip_budget,
                [2, 2],
                &pixels,
                true,
                MaterialSampler {
                    min: Some(9729),
                    ..Default::default()
                },
                || false,
            )
            .unwrap();
        assert_eq!(non_mip.texture().mip_level_count(), 1);
        assert!(uploader.pipelines.iter().all(|p| p.get().is_none()));
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(non_mip_budget.used(), 16);
        drop(non_mip);
        assert_eq!(non_mip_budget.used(), 0);
        for (size, input, resident, peak, data_expected, color_expected) in [
            ([2, 2], pixels.to_vec(), 20, 532, 128, 188),
            (
                [3, 1],
                vec![255, 255, 255, 255, 0, 0, 0, 0, 255, 255, 255, 255],
                16,
                272,
                170,
                213,
            ),
            (
                [1, 3],
                vec![255, 255, 255, 255, 0, 0, 0, 0, 255, 255, 255, 255],
                16,
                784,
                170,
                213,
            ),
        ] {
            for srgb in [false, true] {
                let budget = MemoryBudget::new(peak);
                let candidate = uploader
                    .upload_rgba8(&budget, size, &input, srgb, MaterialSampler::default(), || false)
                    .unwrap();
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                assert_eq!(budget.used(), resident);
                let read = device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: 256,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                let mut encoder = device.create_command_encoder(&Default::default());
                encoder.copy_texture_to_buffer(
                    wgpu::TexelCopyTextureInfo {
                        texture: candidate.texture(),
                        mip_level: 1,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyBufferInfo {
                        buffer: &read,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(256),
                            rows_per_image: Some(1),
                        },
                    },
                    wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                );
                queue.submit([encoder.finish()]);
                let lease = candidate.resource_lease();
                drop(candidate);
                assert_eq!(budget.used(), resident);
                lease.retain_until_complete(&queue);
                let (tx, rx) = std::sync::mpsc::channel();
                read.slice(..)
                    .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                rx.recv().unwrap().unwrap();
                let mapped = read.slice(..).get_mapped_range().unwrap();
                let expected: i16 = if srgb { color_expected } else { data_expected };
                for c in 0..3 {
                    assert!((i16::from(mapped[c]) - expected).abs() <= 1);
                }
                assert!((i16::from(mapped[3]) - data_expected).abs() <= 1);
                eprintln!("size={size:?} srgb={srgb} mip={:?}", &mapped[..4]);
                drop(mapped);
                read.unmap();
                assert_eq!(budget.used(), 0);
            }
        }
        let shared = uploader.clone();
        assert!(Arc::ptr_eq(&shared.pipelines, &uploader.pipelines));
        assert_eq!(shared.pipelines.iter().filter(|p| p.get().is_some()).count(), 2);
        eprintln!("six mip candidates share two format pipelines; non-mip upload builds none");
        let budget = MemoryBudget::new(531);
        assert!(matches!(
            uploader.upload_rgba8(&budget, [2, 2], &pixels, true, MaterialSampler::default(), || {
                false
            }),
            Err(MaterialTextureError::Memory(_))
        ));
        assert_eq!(budget.used(), 0);
        let budget = MemoryBudget::new(532);
        let mut checks = 0;
        assert!(matches!(
            uploader.upload_rgba8(&budget, [2, 2], &pixels, true, MaterialSampler::default(), || {
                checks += 1;
                checks == 3
            }),
            Err(MaterialTextureError::Cancelled)
        ));
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(budget.used(), 0);
    }
}
