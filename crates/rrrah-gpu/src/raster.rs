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
    #[error("raster exceeds the bounded GPU texture-array limit")]
    TextureLimit,
    #[error("resident raster rectangle must be nonempty and inside the source")]
    InvalidRegion,
    #[error(transparent)]
    LodPreparation(#[from] rrrah_core::RasterError),
    #[error("no HDR level fits the available CPU/GPU upload budgets")]
    LodUnavailable,
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
    alpha_associated: u32,
    aspect_padding: [f32; 2],
    background: [f32; 4],
    raw_development: [u32; 4],
    curve: [[f32; 4]; 256],
    resident_rect: [u32; 4],
    comparison: [f32; 4],
    sensor_geometry: [u32; 4],
    crop: [u32; 4],
    authoring: [u32; 4],
    author_curve: [[f32; 4]; 32],
    original_curve: [[f32; 4]; 32],
}

// Balanced tiles minimize padding while respecting negotiated device extents.
// CPU samples stay packed: uploads address source rectangles without a staging Vec.
struct RasterTiles {
    width: u32,
    height: u32,
    columns: u32,
    layers: u32,
    bytes: u64,
}
impl RasterTiles {
    fn new(width: u32, height: u32, limits: &wgpu::Limits) -> Result<Self, RasterUploadError> {
        let cap = limits.max_texture_dimension_2d;
        if width == 0 || height == 0 || cap == 0 {
            return Err(RasterUploadError::TextureLimit);
        }
        let columns = width.div_ceil(cap);
        let rows = height.div_ceil(cap);
        let layers = columns.checked_mul(rows).ok_or(RasterUploadError::TextureLimit)?;
        let tile_width = width.div_ceil(columns);
        let tile_height = height.div_ceil(rows);
        let bytes = u64::from(tile_width)
            .checked_mul(u64::from(tile_height))
            .and_then(|v| v.checked_mul(u64::from(layers)))
            .and_then(|v| v.checked_mul(16))
            .ok_or(RasterUploadError::TextureLimit)?;
        if layers > limits.max_texture_array_layers || bytes > MAX_EAGER_ATLAS_BYTES {
            return Err(RasterUploadError::TextureLimit);
        }
        Ok(Self {
            width: tile_width,
            height: tile_height,
            columns,
            layers,
            bytes,
        })
    }
}

/// Linear RGBA renderer with either an sRGB display attachment or a linear
/// RGBA16F intermediate. Float output is not physical HDR display configuration.
#[derive(Debug)]
pub struct RasterRenderer {
    edit_states: Option<(rrrah_core::edit::EditState, rrrah_core::edit::EditState)>,
    pipeline: wgpu::RenderPipeline,
    single_layer_pipeline: wgpu::RenderPipeline,
    region_pipeline: Option<wgpu::RenderPipeline>,
    single_layer_region_pipeline: Option<wgpu::RenderPipeline>,
    shader: wgpu::ShaderModule,
    pipeline_layout: wgpu::PipelineLayout,
    target: wgpu::TextureFormat,
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
            u64::from(texture.width())
                * u64::from(texture.height())
                * u64::from(texture.depth_or_array_layers())
                * 16
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
    /// Finite exposed/composited values saturate to the binary16 range at output.
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
                        view_dimension: wgpu::TextureViewDimension::D2Array,
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
        let make_pipeline = |single: bool, region: bool| {
            Self::create_pipeline(device, &shader, &pipeline_layout, target, single, region)
        };
        let pipeline = make_pipeline(false, false);
        let single_layer_pipeline = make_pipeline(true, false);
        let parameters = Parameters {
            viewport: [1.0; 2],
            image_size: [1.0; 2],
            pan: [0.0; 2],
            zoom: 1.0,
            exposure: 0.0,
            pixel_aspect: 1.0,
            alpha_associated: 0,
            aspect_padding: [0.0; 2],
            background: [0.018, 0.018, 0.018, 1.0],
            raw_development: [0; 4],
            curve: [[0.0; 4]; 256],
            authoring: [0; 4],
            author_curve: [[0.; 4]; 32],
            original_curve: [[0.; 4]; 32],
            resident_rect: [0, 0, 1, 1],
            comparison: [0.; 4],
            sensor_geometry: [1, 1, 0, 0],
            crop: [0, 0, 1, 1],
        };
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Rrrah raster parameters"),
            contents: bytemuck::bytes_of(&parameters),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        Self {
            edit_states: None,
            pipeline,
            single_layer_pipeline,
            region_pipeline: None,
            single_layer_region_pipeline: None,
            shader,
            pipeline_layout,
            target,
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

    fn create_pipeline(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        pipeline_layout: &wgpu::PipelineLayout,
        target: wgpu::TextureFormat,
        single: bool,
        region: bool,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Rrrah raster pipeline"),
            layout: Some(pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &[
                        ("SINGLE_LAYER", if single { 1.0 } else { 0.0 }),
                        ("RESIDENT_REGION", if region { 1.0 } else { 0.0 }),
                    ],
                    ..Default::default()
                },
            }),
            multiview_mask: None,
            cache: None,
        })
    }

    /// Actual array allocation, including edge-tile padding, before upload.
    pub fn texture_bytes_for_extent(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> Result<u64, RasterUploadError> {
        Ok(RasterTiles::new(width, height, &device.limits())?.bytes)
    }

    /// Advisory current headroom; upload still atomically admits texture/queue credit.
    /// Source validation, concurrent reservations and shared ancestor contention are
    /// checked by the eventual upload, not guaranteed by this planning query.
    pub fn can_admit_texture_extent(&self, device: &wgpu::Device, width: u32, height: u32) -> bool {
        self.can_admit_texture_copies(device, width, height, 1)
    }

    /// Advisory admission for a region plus an equally sized future replacement.
    /// Plans optional panning overscan with replacement headroom in each budget.
    /// Concurrent reservations and shared ancestors still require upload admission.
    pub fn can_admit_texture_extent_with_spare(
        &self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> bool {
        self.can_admit_texture_copies(device, width, height, 2)
    }

    fn can_admit_texture_copies(&self, device: &wgpu::Device, width: u32, height: u32, copies: u64) -> bool {
        let Ok(layout) = RasterTiles::new(width, height, &device.limits()) else {
            return false;
        };
        let row = (u64::from(layout.width) * 16).div_ceil(256) * 256;
        let texture_bytes = layout.bytes * copies;
        let queue_bytes = row * u64::from(layout.height) * u64::from(layout.layers) * copies;
        match (&self.memory_budget, &self.upload_queue_budget) {
            (Some(texture), Some(queue)) => texture.can_reserve_with(texture_bytes, queue, queue_bytes),
            (Some(texture), None) => texture.available_bytes() >= texture_bytes,
            (None, Some(queue)) => queue.available_bytes() >= queue_bytes,
            (None, None) => true,
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

    /// Cancellation preserves the previous texture and uniforms. Partial tile
    /// transfers retain their admission credit until queued GPU work completes.
    /// Individual driver allocation/write calls are not interruptible.
    pub fn upload_with_cancel<F: FnMut() -> bool>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        raster: &DecodedRaster,
        cancelled: F,
    ) -> Result<(), RasterUploadError> {
        self.upload_region_with_cancel(
            device,
            queue,
            raster,
            [0, 0, raster.width(), raster.height()],
            cancelled,
        )
    }

    /// Conservative visible source footprint, including the bilinear halo.
    /// Returns None when the whole image is outside the viewport. Uses the same
    /// view normalization, fit margin and pixel aspect as rendering. Planning does
    /// not allocate pixels, require a device, or imply GPU memory admission.
    pub fn visible_source_region(
        raster: &DecodedRaster,
        view: ViewParameters,
    ) -> Result<Option<[u32; 4]>, RasterUploadError> {
        Self::visible_source_region_for_geometry(
            [raster.width(), raster.height()],
            raster.pixel_aspect(),
            view,
        )
    }

    /// Plan a viewport from metadata alone, including when full pixels are swapped out.
    pub fn visible_source_region_for_geometry(
        dimensions: [u32; 2],
        pixel_aspect: Option<f32>,
        view: ViewParameters,
    ) -> Result<Option<[u32; 4]>, RasterUploadError> {
        if dimensions.contains(&0) {
            return Err(RasterUploadError::InvalidRegion);
        }
        let view = normalized_view(view);
        let aspect = pixel_aspect.unwrap_or(1.0);
        let physical_width = dimensions[0] as f32 * aspect;
        if !physical_width.is_finite() || physical_width <= 0.0 {
            return Err(RasterUploadError::InvalidPixelAspect);
        }
        // Match shader f32 fit/scale rounding before inverting in f64. This matters
        // for narrow pixel aspects and prevents a different fit estimate at high zoom.
        let fit = ((view.viewport[0] - 32.0).max(1.0) / physical_width)
            .min((view.viewport[1] - 32.0).max(1.0) / dimensions[1] as f32);
        let scales = [fit * view.zoom * aspect, fit * view.zoom];
        if scales.iter().any(|s| !s.is_finite() || *s <= 0.0) {
            return Err(RasterUploadError::InvalidPixelAspect);
        }
        let mut origin = [0; 2];
        let mut extent = [0; 2];
        for axis in 0..2 {
            let viewport = f64::from(view.viewport[axis]);
            let center = f64::from(dimensions[axis]) * 0.5;
            let scale = f64::from(scales[axis]);
            let low = (-viewport * 0.5 - f64::from(view.pan[axis])) / scale + center;
            let high = (viewport * 0.5 - f64::from(view.pan[axis])) / scale + center;
            if high <= 0.0 || low >= f64::from(dimensions[axis]) {
                return Ok(None);
            }
            // Include both floor(location-.5) taps and conservative f32 coordinate
            // rounding slack, including very wide sources where one ULP exceeds a pixel.
            let halo = (f64::from(dimensions[axis]) * f64::from(f32::EPSILON) * 8.0)
                .ceil()
                .max(2.0);
            origin[axis] = (low.floor() - halo).clamp(0.0, f64::from(dimensions[axis])) as u32;
            let end = (high.ceil() + halo).clamp(0.0, f64::from(dimensions[axis])) as u32;
            extent[axis] = end - origin[axis];
        }
        Ok(Some([origin[0], origin[1], extent[0], extent[1]]))
    }

    /// Upload a source rectangle without copying CPU pixels or changing full-image geometry.
    /// Include a one-pixel bilinear halo around the visible source footprint. Pixels whose
    /// sampling footprint is not resident show background; no unavailable texels are read.
    /// Admission and cancellation preserve the previously installed region on failure.
    /// This is a region-transfer primitive, not an automatic viewport cache or decoder.
    pub fn upload_region_with_cancel<F: FnMut() -> bool>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        raster: &DecodedRaster,
        rect: [u32; 4],
        cancelled: F,
    ) -> Result<(), RasterUploadError> {
        self.upload_region_pixels(
            device,
            queue,
            raster,
            rect,
            [raster.width(), raster.height()],
            [rect[0], rect[1]],
            cancelled,
        )
    }

    /// Upload packed cropped pixels while retaining the original image geometry.
    /// The crop must already be prepared as linear sRGB; no full source is retained.
    pub fn upload_packed_region_with_cancel<F: FnMut() -> bool>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        crop: &DecodedRaster,
        source_size: [u32; 2],
        origin: [u32; 2],
        cancelled: F,
    ) -> Result<(), RasterUploadError> {
        self.upload_region_pixels(
            device,
            queue,
            crop,
            [origin[0], origin[1], crop.width(), crop.height()],
            source_size,
            [0, 0],
            cancelled,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn upload_region_pixels<F: FnMut() -> bool>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        raster: &DecodedRaster,
        rect: [u32; 4],
        source_size: [u32; 2],
        pixel_origin: [u32; 2],
        mut cancelled: F,
    ) -> Result<(), RasterUploadError> {
        let [origin_x, origin_y, region_width, region_height] = rect;
        if region_width == 0
            || region_height == 0
            || origin_x
                .checked_add(region_width)
                .is_none_or(|v| v > source_size[0])
            || origin_y
                .checked_add(region_height)
                .is_none_or(|v| v > source_size[1])
        {
            return Err(RasterUploadError::InvalidRegion);
        }
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
        let physical_width = source_size[0] as f32 * aspect;
        if !physical_width.is_finite() || physical_width <= 0.0 {
            return Err(RasterUploadError::InvalidPixelAspect);
        }
        let layout = RasterTiles::new(region_width, region_height, &device.limits())?;
        for row in pixel_origin[1]..pixel_origin[1] + region_height {
            let begin =
                (u64::from(row) * u64::from(raster.width()) + u64::from(pixel_origin[0])) as usize * 4;
            let end = begin + region_width as usize * 4;
            for (index, pixel) in pixels[begin..end].as_chunks::<4>().0.iter().enumerate() {
                if index % 4096 == 0 && cancelled() {
                    return Err(RasterUploadError::Cancelled);
                }
                if pixel.iter().any(|v| !v.is_finite()) || !(0.0..=1.0).contains(&pixel[3]) {
                    return Err(RasterUploadError::InvalidSamples);
                }
            }
        }
        if cancelled() {
            return Err(RasterUploadError::Cancelled);
        }
        let reservation = self
            .memory_budget
            .as_ref()
            .map(|budget| budget.try_reserve(layout.bytes))
            .transpose()?
            .map(std::sync::Arc::new);
        let queued_reservation = self
            .upload_queue_budget
            .as_ref()
            .map(|budget| {
                let row = (u64::from(layout.width) * 16)
                    .div_ceil(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT))
                    * u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
                budget.try_reserve(row * u64::from(layout.height) * u64::from(layout.layers))
            })
            .transpose()?;
        if cancelled() {
            return Err(RasterUploadError::Cancelled);
        }
        // Ordinary uploads retain their two-pipeline startup cost. Compile region variants
        // only when a region is first admitted, before any texture transfer is published.
        if rect != [0, 0, source_size[0], source_size[1]] {
            if self.region_pipeline.is_none() {
                self.region_pipeline = Some(Self::create_pipeline(
                    device,
                    &self.shader,
                    &self.pipeline_layout,
                    self.target,
                    false,
                    true,
                ));
            }
            if self.single_layer_region_pipeline.is_none() {
                self.single_layer_region_pipeline = Some(Self::create_pipeline(
                    device,
                    &self.shader,
                    &self.pipeline_layout,
                    self.target,
                    true,
                    true,
                ));
            }
            if cancelled() {
                return Err(RasterUploadError::Cancelled);
            }
        }
        let _queued_upload = crate::QueuedUploadReservation {
            queue: queue.clone(),
            resources: crate::GpuResourceLease::new(reservation.iter().cloned()),
            reservation: queued_reservation,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Rrrah tiled linear raster"),
            size: wgpu::Extent3d {
                width: layout.width,
                height: layout.height,
                depth_or_array_layers: layout.layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for layer in 0..layout.layers {
            if cancelled() {
                return Err(RasterUploadError::Cancelled);
            }
            let local_x = (layer % layout.columns) * layout.width;
            let local_y = (layer / layout.columns) * layout.height;
            let x = pixel_origin[0] + local_x;
            let y = pixel_origin[1] + local_y;
            let width = layout.width.min(region_width - local_x);
            let height = layout.height.min(region_height - local_y);
            let begin = (u64::from(y) * u64::from(raster.width()) + u64::from(x)) as usize * 4;
            let end = begin + ((height - 1) as usize * raster.width() as usize + width as usize) * 4;
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: layer },
                    aspect: wgpu::TextureAspect::All,
                },
                bytemuck::cast_slice(&pixels[begin..end]),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(raster.width() * 16),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
        if cancelled() {
            return Err(RasterUploadError::Cancelled);
        }
        self.install_texture(
            device,
            queue,
            texture,
            reservation,
            source_size[0],
            source_size[1],
        );
        self.parameters.alpha_associated =
            u32::from(raster.alpha_association() == rrrah_core::RasterAlphaAssociation::Associated);
        self.parameters.resident_rect = rect;
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
        cancelled: F,
    ) -> Result<(), RasterUploadError> {
        self.upload_resident_buffer(
            device,
            queue,
            source.buffer(),
            u64::from(source.pixels()),
            source.resource_lease(),
            source.raster_alpha_valid(),
            dimensions,
            aspect,
            cancelled,
        )?;
        self.parameters.alpha_associated =
            u32::from(source.alpha_association() == rrrah_core::RasterAlphaAssociation::Associated);
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&self.parameters));
        Ok(())
    }

    /// Upload calibrated scene-linear Bayer output directly from its GPU buffer.
    /// RGBA16F presentation explicitly saturates to its finite signed range after exposure.
    /// Exposure, display transfer and RAW tone mapping remain caller controls.
    pub fn upload_bayer_resident<F: FnMut() -> bool>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: &crate::ResidentLinearBayer,
        cancelled: F,
    ) -> Result<(), RasterUploadError> {
        let dimensions = source.dimensions();
        self.upload_resident_buffer(
            device,
            queue,
            source.buffer(),
            u64::from(dimensions[0]) * u64::from(dimensions[1]),
            source.resource_lease(),
            true,
            dimensions,
            None,
            cancelled,
        )?;
        let crop = source.crop;
        self.parameters.crop = [crop.x, crop.y, crop.width, crop.height];
        // Calibrated Bayer input is range-admitted; its area sum stays far below f32 overflow.
        self.parameters.sensor_geometry = [dimensions[0], dimensions[1], source.orientation.shader_code(), 1];
        self.parameters.image_size = if source.orientation.swaps_dimensions() {
            [crop.height as f32, crop.width as f32]
        } else {
            [crop.width as f32, crop.height as f32]
        };
        // Match the fragment RAW path without relying on backend conversion behavior.
        self.parameters.raw_development[2] = u32::from(self.target == wgpu::TextureFormat::Rgba16Float);
        self.parameters.raw_development[3] = 1;
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&self.parameters));
        Ok(())
    }

    fn upload_resident_buffer<F: FnMut() -> bool>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: &wgpu::Buffer,
        pixels: u64,
        source_lease: crate::GpuResourceLease,
        alpha_valid: bool,
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
        if width == 0 || height == 0 || u64::from(width) * u64::from(height) != pixels {
            return Err(RasterUploadError::TextureLimit);
        }
        let tiles = RasterTiles::new(width, height, &device.limits())?;
        if !alpha_valid {
            return Err(RasterUploadError::InvalidSamples);
        }
        let bytes = pixels * 16;
        let reservation = self
            .memory_budget
            .as_ref()
            .map(|b| b.try_reserve(tiles.bytes))
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
                width: tiles.width,
                height: tiles.height,
                depth_or_array_layers: tiles.layers,
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
        // Rectangle copies retain the packed source stride. Unaligned source
        // rows are copied individually, without an intermediate GPU/CPU buffer.
        for layer in 0..tiles.layers {
            if cancelled() {
                return Err(RasterUploadError::Cancelled);
            }
            let x = (layer % tiles.columns) * tiles.width;
            let y = (layer / tiles.columns) * tiles.height;
            let copy_width = tiles.width.min(width - x);
            let tile_height = tiles.height.min(height - y);
            let copy_height = if aligned { tile_height } else { 1 };
            for row in (0..tile_height).step_by(copy_height as usize) {
                if row % 256 == 0 && cancelled() {
                    return Err(RasterUploadError::Cancelled);
                }
                encoder.copy_buffer_to_texture(
                    wgpu::TexelCopyBufferInfo {
                        buffer: source,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: (u64::from(y + row) * u64::from(width) + u64::from(x)) * 16,
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
                        origin: wgpu::Origin3d {
                            x: 0,
                            y: row,
                            z: layer,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::Extent3d {
                        width: copy_width,
                        height: copy_height,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
        if cancelled() {
            return Err(RasterUploadError::Cancelled);
        }
        let mut resources = source_lease;
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
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
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
        self.parameters.authoring = [0; 4];
        self.edit_states = None;
        self.parameters.image_size = [width as f32, height as f32];
        self.parameters.sensor_geometry = [width, height, 0, 0];
        self.parameters.crop = [0, 0, width, height];
        self.parameters.resident_rect = [0, 0, width, height];
        self.parameters.pixel_aspect = 1.0;
        self.parameters.alpha_associated = 0;
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&self.parameters));
    }

    /// Tone-map developed scene-linear RAW after exposure, then apply the
    /// monotone luminance curve. Uploading an ordinary raster resets this mode.
    pub fn set_raw_development(&mut self, queue: &wgpu::Queue, curve: &rrrah_core::develop::MonotoneCurve) {
        let knots = curve.gpu_knots();
        self.parameters.raw_development = [
            1,
            knots.len() as u32,
            self.parameters.raw_development[2],
            self.parameters.raw_development[3],
        ];
        self.parameters.curve[..knots.len()].copy_from_slice(&knots);
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&self.parameters));
    }

    /// Display-only document curves after source development/exposure. Identity
    /// skips evaluation, preserving signed/HDR samples exactly. Before/after has
    /// its own baseline curve; source textures and cache identities stay unchanged.
    pub fn set_edit_curves(
        &mut self,
        queue: &wgpu::Queue,
        current: &rrrah_core::edit::EditState,
        original: &rrrah_core::edit::EditState,
    ) -> Result<(), rrrah_core::edit::EditError> {
        current.validate()?;
        original.validate()?;
        if self
            .edit_states
            .as_ref()
            .is_some_and(|(a, b)| a == current && b == original)
        {
            return Ok(());
        }

        let mut curves = [[[0.; 4]; 32]; 2];
        let mut sizes = [0; 4];
        for (i, state) in [current, original].into_iter().enumerate() {
            if state.points() == rrrah_core::edit::EditState::default().points() {
                continue;
            }
            let points: Vec<_> = state.points().iter().map(|p| (p[0], p[1])).collect();
            let curve = rrrah_core::develop::MonotoneCurve::new(&points)
                .map_err(|_| rrrah_core::edit::EditError::Invalid)?;
            let knots = curve.gpu_knots();
            curves[i][..knots.len()].copy_from_slice(&knots);
            sizes[i] = knots.len() as u32;
        }
        self.edit_states = Some((*current, *original));
        self.parameters.authoring = sizes;
        self.parameters.author_curve = curves[0];
        self.parameters.original_curve = curves[1];
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&self.parameters));
        Ok(())
    }

    /// Integrate premultiplied linear pixels over the output pixel footprint when shrinking.
    /// Ordinary uploads reset to bilinear; calibrated Bayer uploads enable this automatically.
    pub fn set_area_minification(&mut self, queue: &wgpu::Queue, enabled: bool) {
        self.parameters.raw_development[3] = u32::from(enabled);
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&self.parameters));
    }

    /// Opaque scene-linear background for associated/straight over compositing.
    pub fn set_background(&mut self, queue: &wgpu::Queue, rgb: [f32; 3]) -> Result<(), RasterUploadError> {
        if rgb.iter().any(|v| !v.is_finite()) {
            return Err(RasterUploadError::InvalidSamples);
        }
        self.parameters.background = [rgb[0], rgb[1], rgb[2], 1.0];
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&self.parameters));
        Ok(())
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

    /// A shared-geometry split: left uses baseline exposure, right uses the current view.
    /// Zero disables comparison. Invalid inputs preserve the previous state.
    pub fn set_comparison(&mut self, queue: &wgpu::Queue, split: f32, baseline: f32) {
        if !split.is_finite() || !baseline.is_finite() {
            return;
        }
        let comparison = [split.clamp(0., 1.), baseline.clamp(-10., 10.), 0., 0.];
        if self.parameters.comparison != comparison {
            self.parameters.comparison = comparison;
            queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&self.parameters));
        }
    }

    pub fn update_view(&mut self, queue: &wgpu::Queue, view: ViewParameters) {
        let view_bits = |p: &Parameters| {
            [
                p.viewport[0],
                p.viewport[1],
                p.pan[0],
                p.pan[1],
                p.zoom,
                p.exposure,
            ]
            .map(f32::to_bits)
        };
        let previous = view_bits(&self.parameters);
        let view = normalized_view(view);
        self.parameters.viewport = view.viewport;
        self.parameters.pan = view.pan;
        self.parameters.zoom = view.zoom;
        self.parameters.exposure = view.exposure_stops;
        if view_bits(&self.parameters) != previous {
            queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&self.parameters));
        }
    }

    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        self.encode_with_timestamps(encoder, target, None);
    }

    /// Optional render-pass GPU timestamps. The caller must enable TIMESTAMP_QUERY
    /// and own query resolution/readback; normal rendering needs no extra feature.
    pub fn encode_with_timestamps(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        timestamps: Option<wgpu::RenderPassTimestampWrites<'_>>,
    ) {
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
            timestamp_writes: timestamps,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if let Some(bind_group) = &self.bind_group {
            let single = self
                .texture
                .as_ref()
                .is_some_and(|texture| texture.depth_or_array_layers() == 1);
            let region = self.parameters.resident_rect
                != [
                    0,
                    0,
                    self.parameters.sensor_geometry[0],
                    self.parameters.sensor_geometry[1],
                ];
            pass.set_pipeline(match (single, region) {
                (true, false) => &self.single_layer_pipeline,
                (false, false) => &self.pipeline,
                (true, true) => self
                    .single_layer_region_pipeline
                    .as_ref()
                    .expect("region pipeline installed"),
                (false, true) => self.region_pipeline.as_ref().expect("region pipeline installed"),
            });
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

fn normalized_view(view: ViewParameters) -> ViewParameters {
    ViewParameters {
        viewport: view
            .viewport
            .map(|v| if v.is_finite() { v.max(1.0) } else { 1.0 }),
        pan: view.pan.map(|v| if v.is_finite() { v } else { 0.0 }),
        zoom: if view.zoom.is_finite() {
            view.zoom.clamp(0.02, 128.0)
        } else {
            1.0
        },
        exposure_stops: if view.exposure_stops.is_finite() {
            view.exposure_stops.clamp(-10.0, 10.0)
        } else {
            0.0
        },
    }
}

#[cfg(test)]
mod tile_admission_tests {
    use super::*;
    #[test]
    fn authoring_shader_and_host_layout_validate() {
        let module = wgpu::naga::front::wgsl::parse_str(include_str!("../shaders/raster_view.wgsl")).unwrap();
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
        let (_, ty) = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("Parameters"))
            .unwrap();
        let wgpu::naga::TypeInner::Struct { span, .. } = &ty.inner else {
            panic!("uniform not a struct")
        };
        assert_eq!(*span as usize, std::mem::size_of::<Parameters>());
    }
    #[test]
    #[cfg(target_os = "macos")]
    fn authoring_curve_before_after_and_hdr_readback_metal() {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let adapter =
            pollster::block_on(instance.request_adapter(&Default::default())).expect("actual Metal required");
        eprintln!("authoring curve adapter: {:?}", adapter.get_info());
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let mut renderer = RasterRenderer::create(&device, wgpu::TextureFormat::Rgba32Float);
        let current = rrrah_core::edit::EditState::default()
            .with_curve(&[[0., 0.], [0.25, 0.5], [1., 1.]])
            .unwrap();
        for value in [0.25f32, 2., -0.125, 0., f32::MAX] {
            let raster = DecodedRaster::new(
                1,
                1,
                RasterPixels::Rgba32Float(std::sync::Arc::new(vec![value, value, value, 1.]).into()),
                RasterColorSpace::LinearSrgb,
            )
            .unwrap();
            renderer.upload(&device, &queue, &raster).unwrap();
            renderer.update_view(
                &queue,
                ViewParameters {
                    viewport: [2., 1.],
                    zoom: 4.,
                    ..Default::default()
                },
            );
            renderer.set_comparison(&queue, 0.5, 0.);
            let edited = if value == 0. {
                rrrah_core::edit::EditState::default()
                    .with_curve(&[[0., 0.125], [1., 1.]])
                    .unwrap()
            } else {
                current
            };
            renderer
                .set_edit_curves(&queue, &edited, &rrrah_core::edit::EditState::default())
                .unwrap();
            let target = device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: 2,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let read = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 256,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            renderer.encode(&mut encoder, &target.create_view(&Default::default()));
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
                        bytes_per_row: Some(256),
                        rows_per_image: Some(1),
                    },
                },
                wgpu::Extent3d {
                    width: 2,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
            queue.submit([encoder.finish()]);
            let (tx, rx) = std::sync::mpsc::channel();
            read.slice(..)
                .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            rx.recv().unwrap().unwrap();
            let mapped = read.slice(..).get_mapped_range().unwrap();
            let pixels = bytemuck::cast_slice::<u8, f32>(&mapped[..32]);
            let after = if value == 0.25 {
                0.5
            } else if value == 0. {
                0.125
            } else {
                value
            };
            for c in 0..3 {
                assert!((pixels[c] - value).abs() < 1e-5, "before: {pixels:?}");
                assert!((pixels[c + 4] - after).abs() < 1e-5, "after: {pixels:?}");
            }
            drop(mapped);
            read.unmap();
        }
        eprintln!(
            "authoring curve Metal: before identity, authored midpoint and lifted black, signed and >1 HDR preserved"
        );
    }
    #[test]
    #[cfg(target_os = "macos")]
    fn area_float32_extreme_channel_scaling_metal() {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let adapter = match pollster::block_on(instance.request_adapter(&Default::default())) {
            Ok(adapter) => adapter,
            Err(error) => {
                assert!(
                    std::env::var("RRRAH_GPU_OPTIONAL").as_deref() != Ok("0"),
                    "required Metal unavailable: {error}"
                );
                return;
            }
        };
        eprintln!("area float32 scaling adapter: {:?}", adapter.get_info());
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        for red in [f32::MAX, -f32::MAX, 0.] {
            let pixels: Vec<f32> = (0..16)
                .flat_map(|i| {
                    [
                        if red == 0. {
                            if i % 4 < 2 { f32::MAX } else { -f32::MAX }
                        } else {
                            red
                        },
                        1.,
                        0.25,
                        1.,
                    ]
                })
                .collect();
            let raster = DecodedRaster::new(
                4,
                4,
                rrrah_core::RasterPixels::Rgba32Float(std::sync::Arc::new(pixels).into()),
                rrrah_core::RasterColorSpace::LinearSrgb,
            )
            .unwrap();
            let mut renderer = RasterRenderer::create(&device, wgpu::TextureFormat::Rgba32Float);
            renderer.upload(&device, &queue, &raster).unwrap();
            renderer.set_area_minification(&queue, true);
            renderer.update_view(
                &queue,
                ViewParameters {
                    viewport: [1., 1.],
                    zoom: 1.,
                    ..Default::default()
                },
            );
            let target = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("area float32 range oracle"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 256,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            renderer.encode(&mut encoder, &target.create_view(&Default::default()));
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &target,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
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
            let (tx, rx) = std::sync::mpsc::channel();
            readback.slice(..).map_async(wgpu::MapMode::Read, move |status| {
                tx.send(status).unwrap();
            });
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            rx.recv().unwrap().unwrap();
            let mapped = readback.slice(..).get_mapped_range().unwrap();
            let value = bytemuck::cast_slice::<u8, f32>(&mapped[..16]);
            assert_eq!(value, [red, 1., 0.25, 1.]);
            assert!(value.iter().all(|v| v.is_finite()));
            drop(mapped);
            readback.unmap();
        }
        eprintln!(
            "area float32 range: +/-MAX and signed cancellation exact, small channels intact, all finite"
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn associated_exr_extreme_float32_and_half_metal() {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let adapter = match pollster::block_on(instance.request_adapter(&Default::default())) {
            Ok(adapter) => adapter,
            Err(error) => {
                assert!(
                    std::env::var("RRRAH_GPU_OPTIONAL").as_deref() != Ok("0"),
                    "required Metal unavailable: {error}"
                );
                return;
            }
        };
        eprintln!("area float32 scaling adapter: {:?}", adapter.get_info());
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/raster/associated-overflow-rec709.exr"),
        );
        request.memory_budget = Some(root.clone());
        let decoded = rrrah_decode::decode_raster(&request).unwrap();
        let raster = rrrah_decode::prepare_raster_for_display_with_budget(&decoded, Some(&root)).unwrap();
        assert_eq!(
            raster.alpha_association(),
            rrrah_core::RasterAlphaAssociation::Associated
        );
        let compute = crate::LinearExposureCompute::new(&device);
        let refused = rrrah_core::MemoryBudget::new(4096);
        assert!(matches!(
            compute.execute_raster_resident(&queue, &raster, 1., &refused, || false),
            Err(crate::ExposureError::Invalid(
                "exposure requires finite input and output"
            ))
        ));
        assert_eq!(refused.peak(), 0, "overflow must fail before GPU admission");
        for (half, resident, stops) in [
            (false, false, 0.),
            (true, false, 0.),
            (false, true, 0.),
            (true, true, 0.),
            (false, true, -1.),
            (true, true, -1.),
        ] {
            let format = if half {
                wgpu::TextureFormat::Rgba16Float
            } else {
                wgpu::TextureFormat::Rgba32Float
            };
            let mut renderer = RasterRenderer::create(&device, format);
            let gpu_budget = rrrah_core::MemoryBudget::new(8192);
            renderer.memory_budget = Some(gpu_budget.clone());
            let output = if resident {
                let result = compute
                    .execute_raster_resident(&queue, &raster, stops, &gpu_budget, || false)
                    .unwrap();
                assert_eq!(result.alpha_association(), raster.alpha_association());
                renderer
                    .upload_resident_with_aspect(
                        &device,
                        &queue,
                        &result,
                        [raster.width(), raster.height()],
                        raster.pixel_aspect(),
                        || false,
                    )
                    .unwrap();
                Some(result)
            } else {
                renderer.upload(&device, &queue, &raster).unwrap();
                None
            };
            renderer.set_background(&queue, [0.; 3]).unwrap();
            renderer.set_area_minification(&queue, true);
            renderer.update_view(
                &queue,
                ViewParameters {
                    viewport: [1., 1.],
                    zoom: 1.,
                    ..Default::default()
                },
            );
            let target = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("area float32 range oracle"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 256,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            renderer.encode(&mut encoder, &target.create_view(&Default::default()));
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &target,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
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
            let (tx, rx) = std::sync::mpsc::channel();
            readback.slice(..).map_async(wgpu::MapMode::Read, move |status| {
                tx.send(status).unwrap();
            });
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            rx.recv().unwrap().unwrap();
            let mapped = readback.slice(..).get_mapped_range().unwrap();
            if half {
                let value = bytemuck::cast_slice::<u8, u16>(&mapped[..8]);
                assert_eq!(
                    value,
                    [0x7bff, 0xfbff, if stops == 0. { 0x3d00 } else { 0x3900 }, 0x3c00]
                );
            } else {
                let value = bytemuck::cast_slice::<u8, f32>(&mapped[..16]);
                let gain = if stops == 0. { 1. } else { 0.5 };
                assert_eq!(
                    value,
                    [f32::MAX / 4. * gain, -f32::MAX / 4. * gain, 1.25 * gain, 1.]
                );
                assert!(value.iter().all(|v| v.is_finite()));
            }
            drop(mapped);
            readback.unmap();
            drop((renderer, output));
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(
                gpu_budget.used(),
                0,
                "resident/input credit retained after completed render"
            );
        }
        let gpu_root = rrrah_core::MemoryBudget::new(4096);
        let rrrah_core::RasterPixels::Rgba32Float(values) = raster.pixels() else {
            panic!()
        };
        let result = compute
            .execute_interleaved_with_cancel(&queue, values, 0., &gpu_root, &root, || false)
            .unwrap();
        assert_eq!(
            result.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            values.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
        drop(result);
        assert_eq!(gpu_root.used(), 0);
        drop((raster, decoded));
        assert_eq!(root.used(), 0);
        eprintln!(
            "associated EXR Metal: float32 mean exact; half output finite saturation; compute bits exact; resident 0/-1 stops exact; +1 overflow refused before admission; managed roots zero"
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "actual Metal full-frame A/A and A/B including native allocation samples"]
    fn view_uniform_full_frame_pairs() {
        use objc2_metal::{MTLCopyAllDevices, MTLDevice};
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        eprintln!("FRAME_UNIFORM_ADAPTER {:?}", adapter.get_info());
        assert!(adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY));
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: wgpu::Features::TIMESTAMP_QUERY,
            ..Default::default()
        }))
        .unwrap();
        let all = MTLCopyAllDevices();
        assert_eq!(all.len(), 1);
        let native = all.objectAtIndex(0);
        assert_eq!(native.name().to_string(), adapter.get_info().name);
        let before = native.currentAllocatedSize();
        let challenge = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 8 * 1024 * 1024,
            usage: wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        challenge.get_mapped_range_mut(..).unwrap().slice(..).fill(1);
        challenge.unmap();
        assert!(native.currentAllocatedSize() >= before + 8 * 1024 * 1024);
        drop(challenge);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let budget = rrrah_core::MemoryBudget::new(65536);
        let mut renderer = RasterRenderer::new_linear_hdr_with_budget(&device, budget.clone());
        let values: Vec<f32> = (0..4096)
            .flat_map(|i| [0.1 + (i % 64) as f32 / 128., 0.2, 0.3, 1.])
            .collect();
        let source = DecodedRaster::new(
            64,
            64,
            RasterPixels::Rgba32Float(std::sync::Arc::new(values).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        renderer.upload(&device, &queue, &source).unwrap();
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 1920,
                height: 1080,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let target_view = target.create_view(&Default::default());
        let view = ViewParameters {
            viewport: [1920., 1080.],
            zoom: 2.,
            ..Default::default()
        };
        renderer.update_view(&queue, view);
        queue.submit([]);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: None,
            ty: wgpu::QueryType::Timestamp,
            count: 2,
        });
        let resolved = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        for control in [true, false] {
            for pair in 0..13 {
                for mode in [pair % 2, 1 - pair % 2] {
                    let mut times = Vec::new();
                    let mut cpu = Vec::new();
                    let mut gpu = Vec::new();
                    let mut peak = 0;
                    for _ in 0..30 {
                        let start = std::time::Instant::now();
                        if !control && mode == 0 {
                            queue.write_buffer(
                                &renderer.uniform,
                                0,
                                bytemuck::bytes_of(&renderer.parameters),
                            );
                        } else {
                            renderer.update_view(&queue, view);
                        }
                        let mut encoder = device.create_command_encoder(&Default::default());
                        renderer.encode_with_timestamps(
                            &mut encoder,
                            &target_view,
                            Some(wgpu::RenderPassTimestampWrites {
                                query_set: &queries,
                                beginning_of_pass_write_index: Some(0),
                                end_of_pass_write_index: Some(1),
                            }),
                        );
                        queue.submit([encoder.finish()]);
                        cpu.push(start.elapsed().as_nanos());
                        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                        times.push(start.elapsed().as_nanos());
                        let mut encoder = device.create_command_encoder(&Default::default());
                        encoder.resolve_query_set(&queries, 0..2, &resolved, 0);
                        encoder.copy_buffer_to_buffer(&resolved, 0, &read, 0, 16);
                        queue.submit([encoder.finish()]);
                        let (tx, rx) = std::sync::mpsc::channel();
                        read.slice(..)
                            .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
                        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                        rx.recv().unwrap().unwrap();
                        let mapped = read.slice(..).get_mapped_range().unwrap();
                        let begin = u64::from_le_bytes(mapped[..8].try_into().unwrap());
                        let end = u64::from_le_bytes(mapped[8..16].try_into().unwrap());
                        assert!(end > begin);
                        gpu.push(((end - begin) as f64 * f64::from(queue.get_timestamp_period())) as u128);
                        drop(mapped);
                        read.unmap();
                        peak = peak.max(native.currentAllocatedSize());
                    }
                    times.sort_unstable();
                    cpu.sort_unstable();
                    gpu.sort_unstable();
                    assert_eq!(budget.used(), 65536);
                    if pair >= 3 {
                        eprintln!(
                            "FRAME_UNIFORM control={control} pair={} mode={mode} p50_ns={} p95_ns={} cpu_p95_ns={} gpu_p95_ns={} frames=30 native_peak={peak} source_budget=65536",
                            pair - 3,
                            times[14],
                            times[28],
                            cpu[28],
                            gpu[28]
                        );
                    }
                }
            }
        }
        drop(renderer);
        drop(target_view);
        drop(target);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(budget.used(), 0);
        eprintln!("FRAME_UNIFORM_COMPLETE managed_final=0");
    }
    #[test]
    #[ignore = "manual actual Metal paired uniform-update timing for RR-0027"]
    fn repeated_view_uniform_ab_probe() {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
        eprintln!("uniform AB adapter: {:?}", adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let mut renderer = RasterRenderer::new_linear_hdr(&device);
        let view = ViewParameters {
            viewport: [64.; 2],
            ..Default::default()
        };
        renderer.update_view(&queue, view);
        queue.submit([]);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        for pair in 0..13 {
            let mut ns = [0u128; 2];
            for mode in [pair % 2, 1 - pair % 2] {
                let started = std::time::Instant::now();
                for _ in 0..1000 {
                    if mode == 0 {
                        // Exact pre-change setter, retained only as the A/B control.
                        let v = normalized_view(view);
                        renderer.parameters.viewport = v.viewport;
                        renderer.parameters.pan = v.pan;
                        renderer.parameters.zoom = v.zoom;
                        renderer.parameters.exposure = v.exposure_stops;
                        queue.write_buffer(&renderer.uniform, 0, bytemuck::bytes_of(&renderer.parameters));
                    } else {
                        renderer.update_view(&queue, view);
                    }
                }
                queue.submit([]);
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                ns[mode] = started.elapsed().as_nanos();
            }
            if pair >= 3 {
                eprintln!(
                    "uniform AB: pair={}; baseline_ns={}; candidate_ns={}; iterations=1000; uniform_bytes={}",
                    pair - 3,
                    ns[0],
                    ns[1],
                    std::mem::size_of::<Parameters>()
                );
            }
        }
    }

    #[test]
    fn metadata_region_rejects_invalid_geometry_without_source_allocation() {
        for size in [[0, 1], [1, 0], [0, 0]] {
            assert!(matches!(
                RasterRenderer::visible_source_region_for_geometry(size, None, ViewParameters::default()),
                Err(RasterUploadError::InvalidRegion)
            ));
        }
        for aspect in [0., -1., f32::NAN, f32::INFINITY] {
            assert!(matches!(
                RasterRenderer::visible_source_region_for_geometry(
                    [6000, 4000],
                    Some(aspect),
                    ViewParameters::default()
                ),
                Err(RasterUploadError::InvalidPixelAspect)
            ));
        }
        assert_eq!(
            RasterRenderer::visible_source_region_for_geometry(
                [6000, 4000],
                Some(1.),
                ViewParameters {
                    pan: [1e20, 0.],
                    ..Default::default()
                }
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn visible_region_normalizes_view_and_skips_wholly_offscreen_images() {
        let raster = DecodedRaster::new(
            8,
            6,
            RasterPixels::Rgba32Float(std::sync::Arc::new(vec![0.; 8 * 6 * 4]).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let ordinary = ViewParameters {
            viewport: [64., 64.],
            ..Default::default()
        };
        assert_eq!(
            RasterRenderer::visible_source_region(&raster, ordinary).unwrap(),
            Some([0, 0, 8, 6])
        );
        let invalid = ViewParameters {
            viewport: [f32::NAN, 0.],
            pan: [f32::INFINITY, f32::NAN],
            zoom: f32::NAN,
            exposure_stops: f32::INFINITY,
        };
        let normalized = normalized_view(invalid);
        assert_eq!(normalized.viewport, [1., 1.]);
        assert_eq!(normalized.pan, [0., 0.]);
        assert_eq!(normalized.zoom, 1.);
        assert_eq!(normalized.exposure_stops, 0.);
        assert_eq!(
            RasterRenderer::visible_source_region(&raster, invalid).unwrap(),
            RasterRenderer::visible_source_region(&raster, normalized).unwrap()
        );
        assert_eq!(
            RasterRenderer::visible_source_region(
                &raster,
                ViewParameters {
                    pan: [1e20, 0.],
                    ..ordinary
                }
            )
            .unwrap(),
            None
        );
        assert_eq!(
            RasterRenderer::visible_source_region(
                &raster,
                ViewParameters {
                    pan: [0., -1e20],
                    ..ordinary
                }
            )
            .unwrap(),
            None
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "actual Metal constrained-device upload rollback and exact SDR/HDR readback"]
    fn constrained_metal_refusal_partial_cancel_and_retry_keep_previous_frame() {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let adapter =
            pollster::block_on(instance.request_adapter(&Default::default())).expect("required Metal");
        let limits = wgpu::Limits {
            max_texture_dimension_2d: 64,
            max_texture_array_layers: 2,
            ..Default::default()
        };
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: limits,
            ..Default::default()
        }))
        .unwrap();
        assert_eq!(device.limits().max_texture_dimension_2d, 64);
        assert_eq!(device.limits().max_texture_array_layers, 2);
        eprintln!("constrained Metal adapter={:?}", adapter.get_info());
        let frame = |width, height, color: [f32; 4]| {
            DecodedRaster::new(
                width,
                height,
                RasterPixels::Rgba32Float(
                    std::sync::Arc::new(color.repeat(width as usize * height as usize)).into(),
                ),
                RasterColorSpace::LinearSrgb,
            )
            .unwrap()
        };
        for format in [
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureFormat::Rgba16Float,
        ] {
            let root = rrrah_core::MemoryBudget::new(192);
            let uploads = rrrah_core::MemoryBudget::new(1024);
            let mut renderer = RasterRenderer::create(&device, format);
            renderer.memory_budget = Some(root.clone());
            renderer.upload_queue_budget = Some(uploads.clone());
            renderer
                .upload(&device, &queue, &frame(1, 1, [0.25, 0.5, 4., 1.]))
                .unwrap();
            renderer.update_view(
                &queue,
                ViewParameters {
                    viewport: [4., 4.],
                    zoom: 4.,
                    ..Default::default()
                },
            );
            let readback = |renderer: &RasterRenderer| {
                let target = device.create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width: 4,
                        height: 4,
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
                    size: 1024,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let mut encoder = device.create_command_encoder(&Default::default());
                renderer.encode(&mut encoder, &target.create_view(&Default::default()));
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
                            bytes_per_row: Some(256),
                            rows_per_image: Some(4),
                        },
                    },
                    wgpu::Extent3d {
                        width: 4,
                        height: 4,
                        depth_or_array_layers: 1,
                    },
                );
                queue.submit([encoder.finish()]);
                let (tx, rx) = std::sync::mpsc::channel();
                read.slice(..)
                    .map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                rx.recv().unwrap().unwrap();
                let mapped = read.slice(..).get_mapped_range().unwrap();
                let mut pixels = Vec::new();
                let row = if format == wgpu::TextureFormat::Rgba16Float {
                    32
                } else {
                    16
                };
                for y in 0..4 {
                    pixels.extend_from_slice(&mapped[y * 256..y * 256 + row]);
                }
                drop(mapped);
                read.unmap();
                pixels
            };
            let before = readback(&renderer);
            let parameters = bytemuck::bytes_of(&renderer.parameters).to_vec();
            assert_eq!((root.used(), uploads.used()), (16, 0));
            let error_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
            assert!(matches!(
                renderer.upload(&device, &queue, &frame(129, 1, [0., 0., 0., 1.])),
                Err(RasterUploadError::TextureLimit)
            ));
            assert_eq!((root.used(), uploads.used()), (16, 0));
            let replacement = frame(2, 2, [0.75, 0.125, 2., 1.]);
            let pressure = root.try_reserve(root.limit() - root.used()).unwrap();
            assert!(matches!(
                renderer.upload(&device, &queue, &replacement),
                Err(RasterUploadError::Memory(_))
            ));
            drop(pressure);
            let mut admitted_polls = 0;
            assert!(matches!(
                renderer.upload_with_cancel(&device, &queue, &replacement, || {
                    if root.used() > 16 && uploads.used() > 0 {
                        admitted_polls += 1;
                    }
                    admitted_polls >= 3
                }),
                Err(RasterUploadError::Cancelled)
            ));
            assert_eq!(admitted_polls, 3); // After admission, layer entry, then one queued tile write.
            assert_eq!(bytemuck::bytes_of(&renderer.parameters), parameters);
            assert_eq!(readback(&renderer), before);
            assert_eq!((root.used(), uploads.used()), (16, 0));
            assert!(pollster::block_on(error_scope.pop()).is_none());
            renderer.upload(&device, &queue, &replacement).unwrap();
            assert_ne!(readback(&renderer), before);
            assert_eq!((root.used(), uploads.used()), (64, 0));
            drop(renderer);
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!((root.used(), uploads.used()), (0, 0));
            eprintln!(
                "{format:?}: texture-limit/memory/partial-write cancellation; old attachment+uniforms exact; retry changed frame; final GPU/upload credit=0"
            );
        }
    }

    #[test]
    fn rejects_unrepresentable_extents_layers_and_padded_over_budget() {
        let limits = wgpu::Limits {
            max_texture_dimension_2d: 64,
            ..Default::default()
        };
        for (width, height) in [
            (0, 1),
            (1, 0),
            (64 * (limits.max_texture_array_layers + 1), 1),
            (u32::MAX, u32::MAX),
        ] {
            assert!(matches!(
                RasterTiles::new(width, height, &limits),
                Err(RasterUploadError::TextureLimit)
            ));
        }
        let limits = wgpu::Limits {
            max_texture_dimension_2d: 4096,
            ..Default::default()
        };
        // One padded edge column consumes the remaining credit at the cap.
        let exact = RasterTiles::new(8191, 4096, &limits).unwrap();
        assert_eq!(exact.bytes, MAX_EAGER_ATLAS_BYTES);
        assert!(matches!(
            RasterTiles::new(8193, 4096, &limits),
            Err(RasterUploadError::TextureLimit)
        ));
    }
}
