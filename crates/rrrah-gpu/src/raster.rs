use crate::{MAX_EAGER_ATLAS_BYTES, ViewParameters};
use bytemuck::{Pod, Zeroable};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use thiserror::Error;
use wgpu::util::DeviceExt;

#[derive(Debug, Error)]
pub enum RasterUploadError {
    #[error("raster GPU upload cancelled")]
    Cancelled,
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
    #[error("raster GPU input must be linear sRGB RGBA32F")]
    ColorPreparationRequired,
    #[error("raster contains invalid float/alpha samples")]
    InvalidSamples,
    #[error("raster exceeds the bounded single-texture GPU limit")]
    TextureLimit,
    #[error("pixel aspect must produce finite positive image geometry")]
    InvalidPixelAspect,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Parameters {
    viewport: [f32; 2],
    image_size: [f32; 2],
    pan: [f32; 2],
    zoom: f32,
    exposure: f32,
    pixel_aspect: f32,
    aspect_padding: [f32; 3],
    background: [f32; 4],
    raw_development: [u32; 4],
    curve: [[f32; 4]; 256],
}

/// Linear RGBA renderer with either an sRGB display attachment or a linear
/// RGBA16F intermediate. Float output is not physical HDR display configuration.
#[derive(Debug)]
pub struct RasterRenderer {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    texture: Option<wgpu::Texture>,
    memory_budget: Option<rrrah_core::MemoryBudget>,
    upload_queue_budget: Option<rrrah_core::MemoryBudget>,
    texture_reservation: Option<std::sync::Arc<rrrah_core::Reservation>>,
    parameters: Parameters,
}

impl RasterRenderer {
    /// Bounds aligned upload payload until GPU completion. Driver overhead is excluded.
    pub fn with_upload_queue_budget(mut self, budget: rrrah_core::MemoryBudget) -> Self {
        self.upload_queue_budget = Some(budget);
        self
    }

    pub fn resource_lease(&self) -> crate::GpuResourceLease {
        crate::GpuResourceLease::new(self.texture_reservation.iter().cloned())
    }

    pub fn clear_image(&mut self) {
        self.bind_group = None;
        self.texture = None;
        self.texture_reservation = None;
    }
    pub fn has_image(&self) -> bool {
        self.bind_group.is_some()
    }

    pub fn resident_bytes(&self) -> u64 {
        self.texture.as_ref().map_or(0, |texture| {
            u64::from(texture.width()) * u64::from(texture.height()) * 16
        })
    }

    /// # Panics
    /// Panics if the target is not an sRGB attachment.
    pub fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
        assert!(target.is_srgb(), "raster target must be sRGB");
        Self::create(device, target)
    }

    /// Caps renderer-owned RGBA32F texture bytes, including replacement overlap.
    /// Driver overhead, staging and in-flight command retention are excluded.
    pub fn new_with_budget(
        device: &wgpu::Device,
        target: wgpu::TextureFormat,
        budget: rrrah_core::MemoryBudget,
    ) -> Self {
        let mut renderer = Self::new(device, target);
        renderer.memory_budget = Some(budget);
        renderer
    }

    /// Renders linear HDR into RGBA16F, with binary16 precision and range.
    /// Callers must keep exposed/composited values within that representable range.
    /// No display transfer, tone mapping or HDR surface configuration is applied.
    pub fn new_linear_hdr(device: &wgpu::Device) -> Self {
        Self::create(device, wgpu::TextureFormat::Rgba16Float)
    }

    /// Applies owned-input-texture admission to the linear HDR renderer.
    /// The caller-owned HDR render target is outside this budget.
    pub fn new_linear_hdr_with_budget(device: &wgpu::Device, budget: rrrah_core::MemoryBudget) -> Self {
        let mut renderer = Self::new_linear_hdr(device);
        renderer.memory_budget = Some(budget);
        renderer
    }

    fn create(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Rrrah raster viewport"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/raster_view.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Rrrah raster layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Rrrah raster pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        let parameters = Parameters {
            viewport: [1.0; 2],
            image_size: [1.0; 2],
            pan: [0.0; 2],
            zoom: 1.0,
            exposure: 0.0,
            pixel_aspect: 1.0,
            aspect_padding: [0.0; 3],
            background: [0.018, 0.018, 0.018, 1.0],
            raw_development: [0; 4],
            curve: [[0.0; 4]; 256],
        };
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Rrrah raster parameters"),
            contents: bytemuck::bytes_of(&parameters),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        Self {
            pipeline,
            layout,
            uniform,
            bind_group: None,
            texture: None,
            memory_budget: None,
            upload_queue_budget: None,
            texture_reservation: None,
            parameters,
        }
    }

    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        raster: &DecodedRaster,
    ) -> Result<(), RasterUploadError> {
        self.upload_with_cancel(device, queue, raster, || false)
    }

    /// Cancellation before submission preserves the previous texture and uniforms.
    /// Driver allocation/write calls are not interruptible after the final check.
    pub fn upload_with_cancel<F: FnMut() -> bool>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        raster: &DecodedRaster,
        mut cancelled: F,
    ) -> Result<(), RasterUploadError> {
        if cancelled() {
            return Err(RasterUploadError::Cancelled);
        }
        let RasterPixels::Rgba32Float(pixels) = raster.pixels() else {
            return Err(RasterUploadError::ColorPreparationRequired);
        };
        if raster.color_space() != &RasterColorSpace::LinearSrgb {
            return Err(RasterUploadError::ColorPreparationRequired);
        }
        let aspect = raster.pixel_aspect().unwrap_or(1.0);
        let physical_width = raster.width() as f32 * aspect;
        if !physical_width.is_finite() || physical_width <= 0.0 {
            return Err(RasterUploadError::InvalidPixelAspect);
        }
        if raster.width() > device.limits().max_texture_dimension_2d
            || raster.height() > device.limits().max_texture_dimension_2d
            || pixels.len() as u64 > MAX_EAGER_ATLAS_BYTES / 4
        {
            return Err(RasterUploadError::TextureLimit);
        }
        for (index, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
            if index % 4096 == 0 && cancelled() {
                return Err(RasterUploadError::Cancelled);
            }
            if pixel.iter().any(|v| !v.is_finite()) || !(0.0..=1.0).contains(&pixel[3]) {
                return Err(RasterUploadError::InvalidSamples);
            }
        }
        if cancelled() {
            return Err(RasterUploadError::Cancelled);
        }
        let reservation = self
            .memory_budget
            .as_ref()
            .map(|budget| budget.try_reserve(u64::from(raster.width()) * u64::from(raster.height()) * 16))
            .transpose()?
            .map(std::sync::Arc::new);
        let queued_reservation = self
            .upload_queue_budget
            .as_ref()
            .map(|budget| {
                let row = (u64::from(raster.width()) * 16)
                    .div_ceil(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT))
                    * u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
                budget.try_reserve(row * u64::from(raster.height()))
            })
            .transpose()?;
        if cancelled() {
            return Err(RasterUploadError::Cancelled);
        }
        let _queued_upload = crate::QueuedUploadReservation {
            queue: queue.clone(),
            resources: crate::GpuResourceLease::new(reservation.iter().cloned()),
            reservation: queued_reservation,
        };
        let size = wgpu::Extent3d {
            width: raster.width(),
            height: raster.height(),
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Rrrah linear raster"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(pixels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(raster.width() * 16),
                rows_per_image: Some(raster.height()),
            },
            size,
        );
        self.install_texture(
            device,
            queue,
            texture,
            reservation,
            raster.width(),
            raster.height(),
        );
        self.set_pixel_aspect(queue, aspect)?;
        Ok(())
    }

    /// GPU-only transfer of queue-ordered compute output, explicitly interpreted
    /// as linear sRGB. Single-row copies accept packed, unaligned row widths.
    pub fn upload_resident<F: FnMut() -> bool>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: &crate::ResidentExposure,
        dimensions: [u32; 2],
        cancelled: F,
    ) -> Result<(), RasterUploadError> {
        self.upload_resident_with_aspect(device, queue, source, dimensions, None, cancelled)
    }

    /// Preserve explicitly supplied physical pixel geometry through GPU compute.
    pub fn upload_resident_with_aspect<F: FnMut() -> bool>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: &crate::ResidentExposure,
        dimensions: [u32; 2],
        aspect: Option<f32>,
        mut cancelled: F,
    ) -> Result<(), RasterUploadError> {
        if cancelled() {
            return Err(RasterUploadError::Cancelled);
        }
        let [width, height] = dimensions;
        let aspect = aspect.unwrap_or(1.0);
        let physical_width = width as f32 * aspect;
        if !aspect.is_finite() || aspect <= 0.0 || !physical_width.is_finite() || physical_width <= 0.0 {
            return Err(RasterUploadError::InvalidPixelAspect);
        }
        if width == 0
            || height == 0
            || u64::from(width) * u64::from(height) != u64::from(source.pixels())
            || width > device.limits().max_texture_dimension_2d
            || height > device.limits().max_texture_dimension_2d
            || u64::from(source.pixels()) * 16 > MAX_EAGER_ATLAS_BYTES
        {
            return Err(RasterUploadError::TextureLimit);
        }
        if !source.raster_alpha_valid {
            return Err(RasterUploadError::InvalidSamples);
        }
        let bytes = u64::from(source.pixels()) * 16;
        let reservation = self
            .memory_budget
            .as_ref()
            .map(|b| b.try_reserve(bytes))
            .transpose()?
            .map(std::sync::Arc::new);
        let occupancy = self
            .upload_queue_budget
            .as_ref()
            .map(|b| b.try_reserve(bytes))
            .transpose()?;
        if cancelled() {
            return Err(RasterUploadError::Cancelled);
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("resident compute raster"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        let row_bytes = width * 16;
        let aligned = row_bytes.is_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        // Packed aligned rows can share a single transfer. Unaligned rows
        // require separate copies to avoid allocating a padded staging buffer.
        let copy_height = if aligned { height } else { 1 };
        for row in (0..height).step_by(copy_height as usize) {
            if row % 256 == 0 && cancelled() {
                return Err(RasterUploadError::Cancelled);
            }
            encoder.copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer: source.buffer(),
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: u64::from(row) * u64::from(width) * 16,
                        bytes_per_row: Some(
                            row_bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
                                * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT,
                        ),
                        rows_per_image: None,
                    },
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: row, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width,
                    height: copy_height,
                    depth_or_array_layers: 1,
                },
            );
        }
        if cancelled() {
            return Err(RasterUploadError::Cancelled);
        }
        let mut resources = source.resource_lease();
        resources.extend(crate::GpuResourceLease::new(reservation.iter().cloned()));
        let _in_flight = crate::QueuedUploadReservation {
            queue: queue.clone(),
            reservation: occupancy,
            resources,
        };
        queue.submit([encoder.finish()]);
        self.install_texture(device, queue, texture, reservation, width, height);
        self.set_pixel_aspect(queue, aspect)?;
        Ok(())
    }

    fn install_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: wgpu::Texture,
        reservation: Option<std::sync::Arc<rrrah_core::Reservation>>,
        width: u32,
        height: u32,
    ) {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Rrrah raster bind group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.uniform.as_entire_binding(),
                },
            ],
        }));
        self.texture = Some(texture);
        self.texture_reservation = reservation;
        self.parameters.raw_development = [0; 4];
        self.parameters.image_size = [width as f32, height as f32];
        self.parameters.pixel_aspect = 1.0;
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&self.parameters));
    }

    /// Tone-map developed scene-linear RAW after exposure, then apply the
    /// monotone luminance curve. Uploading an ordinary raster resets this mode.
    pub fn set_raw_development(&mut self, queue: &wgpu::Queue, curve: &rrrah_core::develop::MonotoneCurve) {
        let knots = curve.gpu_knots();
        self.parameters.raw_development = [1, knots.len() as u32, 0, 0];
        self.parameters.curve[..knots.len()].copy_from_slice(&knots);
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&self.parameters));
    }

    /// Adjust display geometry without resampling or changing texture coordinates.
    /// Uploading a new raster resets the aspect to square pixels.
    pub fn set_pixel_aspect(&mut self, queue: &wgpu::Queue, aspect: f32) -> Result<(), RasterUploadError> {
        let width = self.parameters.image_size[0] * aspect;
        if !aspect.is_finite() || aspect <= 0.0 || !width.is_finite() || width <= 0.0 {
            return Err(RasterUploadError::InvalidPixelAspect);
        }
        self.parameters.pixel_aspect = aspect;
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&self.parameters));
        Ok(())
    }

    pub fn update_view(&mut self, queue: &wgpu::Queue, view: ViewParameters) {
        self.parameters.viewport = view
            .viewport
            .map(|v| if v.is_finite() { v.max(1.0) } else { 1.0 });
        self.parameters.pan = view.pan.map(|v| if v.is_finite() { v } else { 0.0 });
        self.parameters.zoom = if view.zoom.is_finite() {
            view.zoom.clamp(0.02, 128.0)
        } else {
            1.0
        };
        self.parameters.exposure = if view.exposure_stops.is_finite() {
            view.exposure_stops.clamp(-10.0, 10.0)
        } else {
            0.0
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&self.parameters));
    }

    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Rrrah raster viewport"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.018,
                        g: 0.018,
                        b: 0.018,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if let Some(bind_group) = &self.bind_group {
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}
