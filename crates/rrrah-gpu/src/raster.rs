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
    aspect_padding: [f32; 3],
    background: [f32; 4],
    raw_development: [u32; 4],
    curve: [[f32; 4]; 256],
    resident_rect: [u32; 4],
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
            aspect_padding: [0.0; 3],
            background: [0.018, 0.018, 0.018, 1.0],
            raw_development: [0; 4],
            curve: [[0.0; 4]; 256],
            resident_rect: [0, 0, 1, 1],
        };
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Rrrah raster parameters"),
            contents: bytemuck::bytes_of(&parameters),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        Self {
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
        if width == 0 || height == 0 || u64::from(width) * u64::from(height) != u64::from(source.pixels()) {
            return Err(RasterUploadError::TextureLimit);
        }
        let tiles = RasterTiles::new(width, height, &device.limits())?;
        if !source.raster_alpha_valid {
            return Err(RasterUploadError::InvalidSamples);
        }
        let bytes = u64::from(source.pixels()) * 16;
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
                        buffer: source.buffer(),
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
        self.parameters.image_size = [width as f32, height as f32];
        self.parameters.resident_rect = [0, 0, width, height];
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
        let view = normalized_view(view);
        self.parameters.viewport = view.viewport;
        self.parameters.pan = view.pan;
        self.parameters.zoom = view.zoom;
        self.parameters.exposure = view.exposure_stops;
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&self.parameters));
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
                    self.parameters.image_size[0] as u32,
                    self.parameters.image_size[1] as u32,
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
