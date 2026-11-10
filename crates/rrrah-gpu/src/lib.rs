//! GPU RAW-development and display backend.
#![allow(
    clippy::missing_errors_doc,
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    clippy::cast_precision_loss
)]

use std::{
    borrow::Cow,
    time::{Duration, Instant},
};

use bytemuck::{Pod, Zeroable};
use rrrah_core::{DecodedMosaic, FrameError, Photometric, camera_to_linear_srgb};
use thiserror::Error;
use wgpu::util::DeviceExt;

/// RGBE interpolation kernel with explicit normalized four-plane input contract.
/// Dispatch/admission and source-color binding remain separate from this kernel.
pub const RGBE_INTERPOLATION_SHADER: &str = include_str!("rgbe_interpolate.wgsl");

mod histogram;
pub use histogram::{GpuHistogram, HISTOGRAM_JOB_BYTES};
mod edit_panel;
pub use edit_panel::{
    EditPanel, GRAPH as EDIT_GRAPH, HudEditPanelExt, conflict_choice as edit_conflict_choice,
    graph_point as edit_graph_point, panel_contains as edit_panel_contains,
};

mod rgbe_compute;
pub use rgbe_compute::{RgbeCompute, RgbeComputeError};
mod rgbe_plan;
pub use rgbe_plan::{RgbePlan, RgbePlanError, RgbeRowDispatch};

mod compute;
pub use compute::{ExposureError, LinearExposureCompute, ResidentExposure};
mod demosaic_compute;
pub use demosaic_compute::{
    BayerDemosaicCompute, BayerDemosaicError, GpuDemosaicAlgorithm, ResidentBayerDemosaic,
    ResidentLinearBayer,
};
// Backend selection, resource leases, HUD, filmstrip, and mesh/material rendering are shared through Forma's gpu-kit.
pub use gpu_kit::{
    BackendError, FilmstripRenderer, FilmstripTile, FilmstripTileId, FilmstripUploadError, GpuBackend,
    GpuResourceLease, GpuVendor, HUD_SHADER, HudCard, HudCardBounds, HudRenderer, HudVertex,
    MaterialAlpha, MaterialDraw, MaterialParameters, MaterialRaster, MaterialRenderError,
    MaterialRenderer, MaterialSampler, MaterialTexture, MaterialTextureError, MaterialTextureUploader,
    MaterialVertex, ModelRenderer, ModelUploadError, ModelUploadPreparer, PreparedModelUpload,
    STRIP_HEIGHT, STRIP_PADDING, TILE_GAP, TILE_HEIGHT, TILE_STRIDE, TILE_WIDTH, floor_to_usize,
    glyph_rows, hud_card_layout, push_hud_text, push_quad, take_chars, wrap_hud_text,
};
pub use gpu_kit::filmstrip::{
    self, max_scroll, point_in_strip, scroll_to_reveal, strip_band_y, tile_index_at, tile_x,
};
use gpu_kit::{MAX_EAGER_ATLAS_BYTES, QueuedUploadReservation};

mod raster;
pub use raster::{RasterRenderer, RasterUploadError};

/// Hard upper bound for the eager atlas path. A 512 MiB cap prevents a
/// malformed/huge frame from causing an avoidable device-loss while keeping
/// ordinary 45–60 MP cameras within the fast path. Larger images must use the
/// residency/tiled uploader rather than allocating the complete atlas.

#[derive(Debug, Clone, Copy)]
pub struct ViewParameters {
    pub viewport: [f32; 2],
    /// Pan in physical screen pixels.
    pub pan: [f32; 2],
    /// Multiplier over fit-to-window scale.
    pub zoom: f32,
    /// Exposure in photographic stops.
    pub exposure_stops: f32,
}

impl Default for ViewParameters {
    fn default() -> Self {
        Self {
            viewport: [1.0, 1.0],
            pan: [0.0, 0.0],
            zoom: 1.0,
            exposure_stops: 0.0,
        }
    }
}

/// CPU-side timings for preparing and enqueueing one decoded mosaic.
///
/// None of these spans claims completion of GPU texture copies or shader work.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GpuUploadTimings {
    pub validate: Duration,
    pub atlas_plan: Duration,
    pub texture_allocate: Duration,
    pub halo_pack: Duration,
    pub row_pack: Duration,
    pub texture_write_enqueue: Duration,
    pub uniform_write: Duration,
    pub bind: Duration,
    pub total: Duration,
}

/// Default halo of sensor rows/columns duplicated around each tile so bilinear
/// sampling at tile borders never crosses an array-layer boundary.
pub const DEFAULT_TILE_HALO: u32 = 1;
/// Default aligned interior tile target. The experiment-C sweep
/// (`docs/experiments/C.md`) measured the Apple M5 optimum around tile 1024
/// with halo 1. The planner may instead use one frame-sized tile when it fits
/// the adapter and materially reduces the total atlas allocation.
pub const DEFAULT_MAX_TILE_SIZE: u32 = 1024;
/// Minimum interior tile size accepted by the atlas planner.
pub const MIN_TILE_SIZE: u32 = 32;
/// The default tiling aligns the stored texture extent to this many samples so
/// each atlas row is a multiple of `wgpu::COPY_BYTES_PER_ROW_ALIGNMENT`
/// (128 samples * 2 bytes = 256 bytes) and `row_pack` degenerates to a
/// copy-free upload (experiment C2: another 5-15% off total).
const EXTENT_ALIGNMENT_SAMPLES: u32 = 128;
/// An unaligned single-layer texture avoids many tile uploads, but can require
/// a row-repack. Select it only when it saves at least one eighth of the atlas,
/// enough to cover the measured 5–15% alignment advantage.
const SINGLE_TILE_MAX_ATLAS_FRACTION: (u64, u64) = (7, 8);

/// Optional overrides for the atlas tiling used by
/// [`RawRenderer::upload_mosaic_with_tiling`]. `None` keeps the defaults; the
/// measured defaults remain preferred; a configured memory cap may select a
/// more compact geometry when no explicit tile size was supplied.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TilingOverrides {
    /// Interior tile edge length in sensor samples. The stored texture extent
    /// is `tile_size + 2 * tile_halo` and must fit `max_texture_dimension_2d`.
    pub tile_size: Option<u32>,
    /// Duplicated border width around each tile, in sensor samples.
    pub tile_halo: Option<u32>,
}

/// Resolved atlas geometry for one decoded mosaic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TilingPlan {
    pub tile_size: u32,
    pub tile_halo: u32,
    pub tile_grid: [u32; 2],
    pub layer_count: u32,
    pub texture_extent: u32,
    pub atlas_bytes: u64,
}

/// Compute the atlas tiling for a `width`x`height` mosaic, honoring
/// `overrides` where present. Pure and device-free so the policy is unit
/// testable without a GPU adapter.
pub fn plan_tiling(
    width: u32,
    height: u32,
    max_dimension: u32,
    max_array_layers: u32,
    overrides: TilingOverrides,
) -> Result<TilingPlan, GpuError> {
    let tile_halo = overrides.tile_halo.unwrap_or(DEFAULT_TILE_HALO);
    let halo_extent = tile_halo.checked_mul(2).ok_or(GpuError::InvalidTilingOverride {
        tile_size: overrides.tile_size.unwrap_or(DEFAULT_MAX_TILE_SIZE),
        tile_halo,
        max: max_dimension,
    })?;
    let tile_size = overrides
        .tile_size
        .unwrap_or_else(|| default_tile_size(width, height, max_dimension, max_array_layers, tile_halo));
    let texture_extent = tile_size
        .checked_add(halo_extent)
        .ok_or(GpuError::InvalidTilingOverride {
            tile_size,
            tile_halo,
            max: max_dimension,
        })?;
    if tile_size < MIN_TILE_SIZE || texture_extent > max_dimension {
        if overrides.tile_size.is_some() || overrides.tile_halo.is_some() {
            return Err(GpuError::InvalidTilingOverride {
                tile_size,
                tile_halo,
                max: max_dimension,
            });
        }
        return Err(GpuError::TileTooLargeForAdapter { max: max_dimension });
    }
    let tile_grid = [width.div_ceil(tile_size), height.div_ceil(tile_size)];
    let layer_count = tile_grid[0]
        .checked_mul(tile_grid[1])
        .ok_or(GpuError::TooManyTiles)?;
    if layer_count > max_array_layers {
        return Err(GpuError::TooManyTiles);
    }
    let atlas_bytes = u64::from(texture_extent)
        .checked_mul(u64::from(texture_extent))
        .and_then(|value| value.checked_mul(u64::from(layer_count)))
        .and_then(|value| value.checked_mul(2))
        .ok_or(GpuError::AtlasTooLarge {
            bytes: u64::MAX,
            max: MAX_EAGER_ATLAS_BYTES,
        })?;
    if atlas_bytes > MAX_EAGER_ATLAS_BYTES {
        return Err(GpuError::AtlasTooLarge {
            bytes: atlas_bytes,
            max: MAX_EAGER_ATLAS_BYTES,
        });
    }
    Ok(TilingPlan {
        tile_size,
        tile_halo,
        tile_grid,
        layer_count,
        texture_extent,
        atlas_bytes,
    })
}

// Preserve the measured default whenever it fits. Under a configured cap,
// search bounded tile geometries before refusing an otherwise admissible sensor.
fn fit_tiling_capacity(
    width: u32,
    height: u32,
    max_dimension: u32,
    max_layers: u32,
    overrides: TilingOverrides,
    capacity: u64,
) -> Result<TilingPlan, GpuError> {
    let default = plan_tiling(width, height, max_dimension, max_layers, overrides);
    match &default {
        Ok(plan) if plan.atlas_bytes <= capacity || overrides.tile_size.is_some() => return default,
        Err(_) if overrides.tile_size.is_some() => return default,
        Err(GpuError::AtlasTooLarge { .. }) => {}
        Err(_) => return default,
        _ => {}
    }
    let halo = overrides.tile_halo.unwrap_or(DEFAULT_TILE_HALO);
    let mut best = default.as_ref().ok().copied();
    let max_tile = max_dimension.saturating_sub(halo.saturating_mul(2));
    // For any fixed layer grid, bytes increase with tile edge. Its minimum
    // therefore occurs where ceil(width/tile) or ceil(height/tile) changes.
    // Enumerate those edges, bounded by the adapter's layer count, without a
    // heap allocation or a coarse step that can miss a tight admissible cap.
    for axis in [width, height] {
        for layers in 1..=max_layers.min(axis.div_ceil(MIN_TILE_SIZE)) {
            let tile = axis.div_ceil(layers).max(MIN_TILE_SIZE);
            if tile > max_tile {
                continue;
            }
            if let Ok(candidate) = plan_tiling(
                width,
                height,
                max_dimension,
                max_layers,
                TilingOverrides {
                    tile_size: Some(tile),
                    tile_halo: Some(halo),
                },
            ) {
                if best
                    .as_ref()
                    .is_none_or(|plan| candidate.atlas_bytes < plan.atlas_bytes)
                {
                    best = Some(candidate);
                }
            }
        }
    }
    best.map(Ok).unwrap_or(default)
}

/// Default interior tile size when no explicit override is given: start at the
/// aligned C2 optimum, grow as needed for the adapter's layer limit, then use a
/// single frame-sized tile when it fits and materially reduces atlas bytes.
fn default_tile_size(
    width: u32,
    height: u32,
    max_dimension: u32,
    max_array_layers: u32,
    tile_halo: u32,
) -> u32 {
    let max_tile = max_dimension.saturating_sub(2_u32.saturating_mul(tile_halo));
    let mut tile = DEFAULT_MAX_TILE_SIZE.min(max_tile);
    tile = align_extent(tile, tile_halo).unwrap_or(tile);
    while layer_count(width, height, tile) > u64::from(max_array_layers) {
        let Some(grown) = grow_tile(tile, tile_halo, max_dimension) else {
            break;
        };
        tile = grown;
    }
    if let Some(single_tile) = materially_smaller_single_tile(width, height, tile, max_tile, tile_halo) {
        tile = single_tile;
    }
    tile
}

/// Returns one frame-sized tile when it fits the adapter and its square atlas
/// is at most 7/8 the size of the aligned multi-layer plan.
fn materially_smaller_single_tile(
    width: u32,
    height: u32,
    current_tile: u32,
    max_tile: u32,
    tile_halo: u32,
) -> Option<u32> {
    // Tiny mosaics still fit one minimum-sized tile. Rejecting their exact
    // edge below MIN_TILE_SIZE would fall back to a wasteful 1024-square atlas.
    let single_tile = width.max(height).max(MIN_TILE_SIZE);
    if !(MIN_TILE_SIZE..=max_tile).contains(&single_tile) {
        return None;
    }
    let current_texels = atlas_texels(width, height, current_tile, tile_halo)?;
    let single_texels = atlas_texels(width, height, single_tile, tile_halo)?;
    let (numerator, denominator) = SINGLE_TILE_MAX_ATLAS_FRACTION;
    (single_texels.checked_mul(denominator)? <= current_texels.checked_mul(numerator)?).then_some(single_tile)
}

fn atlas_texels(width: u32, height: u32, tile_size: u32, tile_halo: u32) -> Option<u64> {
    let extent = tile_size.checked_add(tile_halo.checked_mul(2)?)?;
    u64::from(extent)
        .checked_mul(u64::from(extent))?
        .checked_mul(layer_count(width, height, tile_size))
}

/// Rounds the stored extent `tile_size + 2 * tile_halo` down to a multiple of
/// [`EXTENT_ALIGNMENT_SAMPLES`] and returns the matching interior tile size.
/// Returns `None` when alignment would push the tile below [`MIN_TILE_SIZE`].
fn align_extent(tile_size: u32, tile_halo: u32) -> Option<u32> {
    let halo2 = tile_halo.checked_mul(2)?;
    let extent = tile_size.checked_add(halo2)?;
    let aligned = extent / EXTENT_ALIGNMENT_SAMPLES * EXTENT_ALIGNMENT_SAMPLES;
    let aligned_tile = aligned.checked_sub(halo2)?;
    (aligned >= EXTENT_ALIGNMENT_SAMPLES && aligned_tile >= MIN_TILE_SIZE).then_some(aligned_tile)
}

/// Doubles the stored extent (keeping it a multiple of
/// [`EXTENT_ALIGNMENT_SAMPLES`]) to shrink the tile grid, clamped to
/// `max_dimension`. Returns `None` when the tile cannot grow any further.
fn grow_tile(tile_size: u32, tile_halo: u32, max_dimension: u32) -> Option<u32> {
    let halo2 = tile_halo.checked_mul(2)?;
    let extent = tile_size.checked_add(halo2)?;
    let doubled = extent.checked_mul(2)?.min(max_dimension);
    let aligned = doubled / EXTENT_ALIGNMENT_SAMPLES * EXTENT_ALIGNMENT_SAMPLES;
    let grown = aligned.checked_sub(halo2)?;
    (grown > tile_size).then_some(grown)
}

/// Number of array layers the tile grid for `tile_size` would occupy. A zero
/// tile size (only possible on degenerate adapters) reports the maximum so the
/// caller grows or rejects instead of dividing by zero.
fn layer_count(width: u32, height: u32, tile_size: u32) -> u64 {
    if tile_size == 0 {
        return u64::MAX;
    }
    u64::from(width.div_ceil(tile_size)) * u64::from(height.div_ceil(tile_size))
}

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct GpuParameters {
    viewport: [f32; 2],
    raw_size: [u32; 2],
    texture_size: [u32; 2],
    sample_stride: u32,
    tile_halo: u32,
    tile_grid: [u32; 2],
    crop_origin: [u32; 2],
    crop_size: [u32; 2],
    pan: [f32; 2],
    zoom: f32,
    exposure_stops: f32,
    // WGSL vec4 has 16-byte alignment; Rust arrays only have 4-byte
    // alignment, so this explicit gap keeps `cfa` at offset 80.
    _color_alignment: [u32; 2],
    cfa: [u32; 4],
    black: [f32; 4],
    white: [f32; 4],
    white_balance: [f32; 4],
    camera_to_rgb_0: [f32; 4],
    camera_to_rgb_1: [f32; 4],
    camera_to_rgb_2: [f32; 4],
    orientation: u32,
    algorithm: u32,
    // WGSL rounds the trailing vec4 to offset 208 and the struct to 224 before comparison at offset 224.
    _tail_alignment: [u32; 2],
    _padding: [u32; 4],
    comparison: [f32; 4],
}

impl Default for GpuParameters {
    fn default() -> Self {
        Self {
            viewport: [1.0, 1.0],
            raw_size: [1, 1],
            texture_size: [1, 1],
            sample_stride: 1,
            tile_halo: 0,
            tile_grid: [1, 1],
            crop_origin: [0, 0],
            crop_size: [1, 1],
            pan: [0.0, 0.0],
            zoom: 1.0,
            exposure_stops: 0.0,
            _color_alignment: [0; 2],
            cfa: [0, 1, 1, 2],
            black: [0.0; 4],
            white: [u16::MAX as f32; 4],
            white_balance: [1.0; 4],
            camera_to_rgb_0: [1.0, 0.0, 0.0, 0.0],
            camera_to_rgb_1: [0.0, 1.0, 0.0, 0.0],
            camera_to_rgb_2: [0.0, 0.0, 1.0, 0.0],
            orientation: 0,
            algorithm: 0,
            _tail_alignment: [0; 2],
            _padding: [0; 4],
            comparison: [0.; 4],
        }
    }
}

/// Preserve the decoder's camera-space correction gains at the display
/// boundary.
///
/// Decode backends already resolve metadata to multiplicative gains such that
/// a sensor response to the as-shot neutral becomes achromatic. Their common
/// green-normalized scale is part of that backend contract. Rec.709 luminance
/// weights describe linear display RGB, not camera RGB, so applying them here
/// would introduce an illuminant-dependent exposure shift before the camera
/// matrix. Exposure remains a separate scene-linear control.
#[inline]
const fn display_white_balance(gains: [f32; 4]) -> [f32; 4] {
    gains
}

/// Scene-linear output is intended for floating-point intermediates. Display
/// transfer, primaries and luminance mapping remain the presentation owner's job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawOutputMode {
    DisplaySdr,
    SceneLinear,
}

#[derive(Debug)]
pub struct RawRenderer {
    target_format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    raw_texture: Option<wgpu::Texture>,
    memory_budget: Option<rrrah_core::MemoryBudget>,
    atlas_reservation: Option<std::sync::Arc<rrrah_core::Reservation>>,
    upload_memory_budget: Option<rrrah_core::MemoryBudget>,
    upload_queue_budget: Option<rrrah_core::MemoryBudget>,
    parameters: GpuParameters,
    resident_bytes: u64,
}

impl RawRenderer {
    pub fn resource_lease(&self) -> GpuResourceLease {
        GpuResourceLease::new(self.atlas_reservation.iter().cloned())
    }

    pub fn clear_image(&mut self) {
        self.bind_group = None;
        self.raw_texture = None;
        self.atlas_reservation = None;
        self.resident_bytes = 0;
    }
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Rrrah full-RAW viewport shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/raw_view.wgsl").into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Rrrah RAW bind-group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Rrrah RAW pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Rrrah RAW render pipeline"),
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
                    format: target_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        let parameters = GpuParameters::default();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Rrrah RAW parameters"),
            contents: bytemuck::bytes_of(&parameters),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
        });
        Self {
            target_format,
            pipeline,
            bind_group_layout,
            uniform_buffer,
            bind_group: None,
            raw_texture: None,
            memory_budget: None,
            atlas_reservation: None,
            upload_memory_budget: None,
            upload_queue_budget: None,
            parameters,
            resident_bytes: 0,
        }
    }

    /// Admits owned R16Uint atlas footprint, including halos and replacement overlap.
    /// CPU tile packing, staging, driver and in-flight retention are excluded.
    pub fn new_with_budget(
        device: &wgpu::Device,
        target: wgpu::TextureFormat,
        budget: rrrah_core::MemoryBudget,
    ) -> Self {
        let mut renderer = Self::new(device, target);
        renderer.memory_budget = Some(budget);
        renderer
    }

    /// Applies CPU admission to transient halo and aligned-row packing buffers.
    pub fn with_upload_memory_budget(mut self, budget: rrrah_core::MemoryBudget) -> Self {
        self.upload_memory_budget = Some(budget);
        self
    }

    /// Admits aligned queued atlas uploads before texture creation. Occupancy is
    /// retained until submitted GPU work completes, even if the renderer drops.
    /// This is logical payload accounting, excluding driver allocation overhead.
    pub fn with_upload_queue_budget(mut self, budget: rrrah_core::MemoryBudget) -> Self {
        self.upload_queue_budget = Some(budget);
        self
    }

    pub fn has_image(&self) -> bool {
        self.bind_group.is_some()
    }

    /// Approximate bytes reserved by the current eager GPU atlas. This is the
    /// texture allocation, not a vendor-reported total VRAM counter.
    pub fn resident_bytes(&self) -> u64 {
        self.resident_bytes
    }

    pub fn upload_mosaic(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mosaic: &DecodedMosaic,
    ) -> Result<GpuUploadTimings, GpuError> {
        self.upload_mosaic_with_tiling(device, queue, mosaic, TilingOverrides::default())
    }

    pub fn upload_mosaic_with_cancel(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mosaic: &DecodedMosaic,
        cancelled: impl Fn() -> bool,
    ) -> Result<GpuUploadTimings, GpuError> {
        self.upload_mosaic_with_tiling_and_cancel(
            device,
            queue,
            mosaic,
            TilingOverrides::default(),
            cancelled,
        )
    }

    /// Upload one decoded mosaic with an explicit tiling override. Overrides
    /// exist for experimentation (see `examples/gpu_smoke.rs`); the
    /// production path keeps [`TilingOverrides::default`].
    pub fn upload_mosaic_with_tiling(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mosaic: &DecodedMosaic,
        tiling: TilingOverrides,
    ) -> Result<GpuUploadTimings, GpuError> {
        self.upload_mosaic_with_tiling_and_cancel(device, queue, mosaic, tiling, || false)
    }

    /// Cancel between admission, tile preparation and publication. A refused
    /// upload preserves the previous texture and display uniforms.
    pub fn upload_mosaic_with_tiling_and_cancel(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mosaic: &DecodedMosaic,
        tiling: TilingOverrides,
        cancelled: impl Fn() -> bool,
    ) -> Result<GpuUploadTimings, GpuError> {
        let total_started = Instant::now();
        if cancelled() {
            return Err(GpuError::Cancelled);
        }
        let validate_started = Instant::now();
        let metadata = &mosaic.metadata;
        let max_dimension = device.limits().max_texture_dimension_2d;
        if metadata.photometric != Photometric::Cfa || metadata.components_per_pixel != 1 {
            return Err(GpuError::UnsupportedPhotometric);
        }
        let cfa_pattern = metadata.cfa.as_ref().ok_or(GpuError::MissingCfa)?;
        metadata.validate()?;
        let expected_samples = (metadata.width as usize)
            .checked_mul(metadata.height as usize)
            .ok_or(rrrah_core::FrameError::DimensionOverflow)?;
        if mosaic.pixels.len() != expected_samples {
            return Err(rrrah_core::FrameError::InvalidPixelCount {
                expected: expected_samples,
                actual: mosaic.pixels.len(),
            }
            .into());
        }
        let rgbe = cfa_pattern.rgbe_quad().ok();
        let cfa = match rgbe {
            Some(quad) => quad,
            None => cfa_pattern.bayer_quad()?,
        };
        if rgbe.is_some() {
            rrrah_core::rgbe::Calibration::new(metadata).map_err(|_| GpuError::InvalidColorProfile)?;
        }
        let grid = &metadata.black_level;
        if grid.components != 1 {
            return Err(rrrah_core::FrameError::UnsupportedBlackLevelGrid.into());
        }
        let quad = grid.bayer_quad().ok();
        let black = quad.unwrap_or([0.; 4]);
        let grid_bytes = if quad.is_none() {
            grid.values.len() as u64 * 4
        } else {
            0
        };
        let white = metadata.white_level.bayer_quad(cfa_pattern)?;
        let gains = display_white_balance(metadata.white_balance);
        let first_green = usize::from(cfa[0] != 1);
        let phase_gain = std::array::from_fn::<_, 4, _>(|phase| {
            if rgbe.is_none() && cfa[phase] == 1 && phase != first_green {
                gains[3]
            } else {
                gains[cfa[phase] as usize]
            }
        });
        let camera_to_rgb = if rgbe.is_some() {
            rrrah_core::camera4_to_linear_srgb_precise(metadata.xyz_to_camera.map(|r| r.map(f64::from)))
                .map_err(|_| GpuError::InvalidColorProfile)?
                .map(|r| r.map(|v| v as f32))
        } else {
            camera_to_linear_srgb(metadata.xyz_to_camera)
                .ok_or(GpuError::InvalidColorProfile)?
                .map(|r| [r[0], r[1], r[2], 0.])
        };
        if camera_to_rgb.iter().flatten().any(|v| !v.is_finite()) {
            return Err(GpuError::InvalidColorProfile);
        }
        // Bound unnormalized filter accumulation even for a full-sensor footprint.
        let filter_sample_limit =
            f64::from(f32::MAX) / (f64::from(metadata.width) * f64::from(metadata.height) * 4.0).max(16.0);
        let matrix_norm = camera_to_rgb
            .iter()
            .map(|row| row.iter().map(|v| f64::from(*v).abs()).sum::<f64>())
            .fold(1.0_f64, f64::max);
        // Reserve exposure (+10 stops), gamut mapping and quadratic ACES headroom.
        let color_sample_limit = f64::from(f32::MAX).sqrt() / (16.0 * 1024.0 * matrix_norm);
        let filter_sample_limit = filter_sample_limit.min(color_sample_limit);
        let mut inspect_samples = false;
        // Validate every intersection of grid repetition and CFA phase, including
        // parity flips at odd-width/height boundaries, before replacing resources.
        for row in 0..u32::from(grid.height) * 2 {
            for column in 0..u32::from(grid.width) * 2 {
                let level = grid.at(row, column, 0).unwrap();
                let phase = ((row % 2) * 2 + column % 2) as usize;
                if white[phase] <= level || !(white[phase] - level).is_finite() {
                    return Err(GpuError::InvalidLevels {
                        black: level,
                        white: white[phase],
                    });
                }
                let range = white[phase] - level;
                let largest = (65535.0 - level).max(0.0) / range * phase_gain[phase];
                inspect_samples |=
                    !largest.is_finite() || range.is_subnormal() || f64::from(largest) > filter_sample_limit;
            }
        }
        if inspect_samples {
            for (y, row) in mosaic.pixels.chunks(metadata.width as usize).enumerate() {
                if cancelled() {
                    return Err(GpuError::Cancelled);
                }
                for (x, encoded) in row.iter().enumerate() {
                    let phase = (y % 2) * 2 + x % 2;
                    let level = grid.at(y as u32, x as u32, 0).unwrap();
                    let range = white[phase] - level;
                    let normalized = (f32::from(*encoded) - level).max(0.0) / range;
                    // Metal may flush subnormal divisors. Exact zero sites are
                    // handled explicitly in the shader and remain admissible.
                    if !normalized.is_finite()
                        || !(normalized * phase_gain[phase]).is_finite()
                        || (normalized != 0.0 && range.is_subnormal())
                    {
                        return Err(GpuError::NonFiniteSensorCorrection {
                            x: x as u32,
                            y: y as u32,
                        });
                    }
                    if f64::from(normalized * phase_gain[phase]) > filter_sample_limit {
                        return Err(GpuError::ShaderFilterRangeExceeded {
                            x: x as u32,
                            y: y as u32,
                        });
                    }
                }
            }
        }
        let crop = metadata.effective_crop();
        let validate = validate_started.elapsed();

        let atlas_plan_started = Instant::now();
        let mut tiling = tiling;
        if tiling.tile_halo.is_none() {
            tiling.tile_halo = Some(DEFAULT_TILE_HALO.max(self.demosaic_algorithm().min_tile_halo()));
        }
        let plan = fit_tiling_capacity(
            metadata.width,
            metadata.height,
            max_dimension,
            device.limits().max_texture_array_layers,
            tiling,
            self.memory_budget.as_ref().map_or(u64::MAX, |budget| {
                budget.available_bytes().saturating_sub(grid_bytes)
            }),
        )?;
        let tile_halo = plan.tile_halo;
        let tile_size = plan.tile_size;
        let tile_grid = plan.tile_grid;
        let layer_count = plan.layer_count;
        let texture_width = plan.texture_extent;
        let texture_height = plan.texture_extent;
        let atlas_bytes = plan.atlas_bytes + grid_bytes;
        if cancelled() {
            return Err(GpuError::Cancelled);
        }
        let reservation = self
            .memory_budget
            .as_ref()
            .map(|budget| budget.try_reserve(atlas_bytes))
            .transpose()?
            .map(std::sync::Arc::new);
        let row_bytes = u64::from(texture_width) * 2;
        let row_pitch = row_bytes.div_ceil(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT))
            * u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let packed_bytes = if cfg!(target_endian = "big") || row_pitch != row_bytes {
            row_pitch * u64::from(texture_height)
        } else {
            0
        };
        let tile_bytes = row_bytes * u64::from(texture_height);
        let mut upload_reservation = self
            .upload_memory_budget
            .as_ref()
            .map(|budget| budget.try_reserve(tile_bytes + packed_bytes))
            .transpose()?;
        let _queued_upload = QueuedUploadReservation {
            queue: queue.clone(),
            resources: GpuResourceLease::new(reservation.iter().cloned()),
            reservation: self
                .upload_queue_budget
                .as_ref()
                .map(|budget| {
                    budget.try_reserve(row_pitch * u64::from(texture_height) * u64::from(layer_count))
                })
                .transpose()?,
        };
        let mut parameters = self.parameters;
        parameters.raw_size = [metadata.width, metadata.height];
        parameters.texture_size = [texture_width, texture_height];
        parameters.sample_stride = tile_size;
        parameters.tile_halo = tile_halo;
        parameters.tile_grid = tile_grid;
        parameters.crop_origin = [crop.x, crop.y];
        parameters.crop_size = [crop.width, crop.height];
        parameters.cfa = cfa;
        parameters._padding[0] = u32::from(rgbe.is_some());
        parameters._padding[2] = if quad.is_none() { u32::from(grid.width) } else { 0 };
        parameters._padding[3] = u32::from(grid.height);
        parameters.black = black;
        parameters.white = white;
        parameters.white_balance = display_white_balance(metadata.white_balance);
        parameters.camera_to_rgb_0 = camera_to_rgb[0];
        parameters.camera_to_rgb_1 = camera_to_rgb[1];
        parameters.camera_to_rgb_2 = camera_to_rgb[2];
        parameters.orientation = metadata.orientation.shader_code();
        let atlas_plan = atlas_plan_started.elapsed();

        if cancelled() {
            return Err(GpuError::Cancelled);
        }
        let texture_allocate_started = Instant::now();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Rrrah decoded u16 sensor mosaic"),
            size: wgpu::Extent3d {
                width: texture_width,
                height: texture_height,
                depth_or_array_layers: layer_count,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R16Uint,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let texture_allocate = texture_allocate_started.elapsed();
        let mut halo_pack = Duration::ZERO;
        let mut row_pack = Duration::ZERO;
        let mut texture_write_enqueue = Duration::ZERO;
        let mut tile_scratch = Vec::new();
        for tile_y in 0..tile_grid[1] {
            for tile_x in 0..tile_grid[0] {
                if cancelled() {
                    return Err(GpuError::Cancelled);
                }
                let halo_pack_started = Instant::now();
                tile_with_halo_into(
                    &mut tile_scratch,
                    &mosaic.pixels,
                    metadata.width,
                    metadata.height,
                    tile_x,
                    tile_y,
                    tile_size,
                    tile_halo,
                    &cancelled,
                )?;
                halo_pack += halo_pack_started.elapsed();
                let row_pack_started = Instant::now();
                let (bytes, row_pitch) = mosaic_bytes_with_cancel(&tile_scratch, texture_width, &cancelled)?;
                row_pack += row_pack_started.elapsed();
                if let Some(reservation) = &mut upload_reservation {
                    let padding_capacity = match &bytes {
                        Cow::Borrowed(_) => 0,
                        Cow::Owned(values) => values.capacity() as u64,
                    };
                    reservation.ensure_bytes(tile_scratch.capacity() as u64 * 2 + padding_capacity)?;
                }
                if cancelled() {
                    return Err(GpuError::Cancelled);
                }
                let layer = tile_y * tile_grid[0] + tile_x;
                let texture_write_started = Instant::now();
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x: 0, y: 0, z: layer },
                        aspect: wgpu::TextureAspect::All,
                    },
                    bytes.as_ref(),
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(row_pitch),
                        rows_per_image: Some(texture_height),
                    },
                    wgpu::Extent3d {
                        width: texture_width,
                        height: texture_height,
                        depth_or_array_layers: 1,
                    },
                );
                texture_write_enqueue += texture_write_started.elapsed();
            }
        }
        if cancelled() {
            return Err(GpuError::Cancelled);
        }
        let black_grid_buffer = (quad.is_none()).then(|| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Rrrah exact black-level repeat grid"),
                contents: bytemuck::cast_slice(&grid.values),
                usage: wgpu::BufferUsages::STORAGE,
            })
        });
        if cancelled() {
            return Err(GpuError::Cancelled);
        }
        let uniform_write_started = Instant::now();
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&parameters));
        let uniform_write = uniform_write_started.elapsed();

        // The shader indexes one independent sensor tile per array layer. The
        // default view descriptor would infer `D2` even for a layered texture,
        // which then fails bind-group validation against the `D2Array` layout.
        let bind_started = Instant::now();
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            array_layer_count: Some(layer_count),
            ..wgpu::TextureViewDescriptor::default()
        });
        self.bind_group = Some(
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Rrrah RAW bind group"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        // Fast 2x2 uses uniforms; the unused storage binding aliases
                        // the same allocation, avoiding a dummy GPU allocation.
                        resource: black_grid_buffer
                            .as_ref()
                            .unwrap_or(&self.uniform_buffer)
                            .as_entire_binding(),
                    },
                ],
            }),
        );
        self.parameters = parameters;
        self.raw_texture = Some(texture);
        self.atlas_reservation = reservation;
        self.resident_bytes = atlas_bytes;
        let bind = bind_started.elapsed();
        Ok(GpuUploadTimings {
            validate,
            atlas_plan,
            texture_allocate,
            halo_pack,
            row_pack,
            texture_write_enqueue,
            uniform_write,
            bind,
            total: total_started.elapsed(),
        })
    }

    /// Select linear HDR output only on targets that retain signed HDR values.
    /// Float16 output saturates outside [-65504, 65504]; float32 retains its range.
    /// Refusal leaves the current mode and uniforms unchanged.
    pub fn set_output_mode(&mut self, queue: &wgpu::Queue, mode: RawOutputMode) -> Result<(), GpuError> {
        if mode == RawOutputMode::SceneLinear
            && !matches!(
                self.target_format,
                wgpu::TextureFormat::Rgba16Float | wgpu::TextureFormat::Rgba32Float
            )
        {
            return Err(GpuError::InvalidLinearTarget(self.target_format));
        }
        self.parameters._padding[1] = match mode {
            RawOutputMode::DisplaySdr => 0,
            RawOutputMode::SceneLinear if self.target_format == wgpu::TextureFormat::Rgba16Float => 2,
            RawOutputMode::SceneLinear => 1,
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&self.parameters));
        Ok(())
    }

    /// Select the GPU fragment-shader demosaicing algorithm.
    ///
    /// The required halo includes nested neighbour filters: two samples for
    /// MalvarHeCutler/SmoothHue, three for Directional/VNG, four for RCD.
    /// Calling `set_demosaic_algorithm` before `upload_mosaic` automatically raises the default
    /// `tile_halo` to `algorithm.min_tile_halo()`; if a mosaic is already resident with a smaller
    /// `tile_halo`, the shader safely falls back to `Bilinear` until the mosaic is re-uploaded
    /// with sufficient halo.
    pub fn set_demosaic_algorithm(&mut self, queue: &wgpu::Queue, algorithm: GpuDemosaicAlgorithm) {
        self.parameters.algorithm = algorithm.shader_code();
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&self.parameters));
    }

    #[must_use]
    pub const fn demosaic_algorithm(&self) -> GpuDemosaicAlgorithm {
        match self.parameters.algorithm {
            1 => GpuDemosaicAlgorithm::MalvarHeCutler,
            2 => GpuDemosaicAlgorithm::Directional,
            3 => GpuDemosaicAlgorithm::SmoothHue,
            4 => GpuDemosaicAlgorithm::Vng,
            5 => GpuDemosaicAlgorithm::Rcd,
            _ => GpuDemosaicAlgorithm::Bilinear,
        }
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
            queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&self.parameters));
        }
    }

    pub fn update_view(&mut self, queue: &wgpu::Queue, view: ViewParameters) {
        // Invalid control input must not poison an otherwise valid resident frame.
        let finite = |value: f32, previous: f32| if value.is_finite() { value } else { previous };
        for axis in 0..2 {
            self.parameters.viewport[axis] =
                finite(view.viewport[axis], self.parameters.viewport[axis]).max(1.0);
            self.parameters.pan[axis] = finite(view.pan[axis], self.parameters.pan[axis]);
        }
        self.parameters.zoom = finite(view.zoom, self.parameters.zoom).clamp(0.02, 128.0);
        self.parameters.exposure_stops =
            finite(view.exposure_stops, self.parameters.exposure_stops).clamp(-10.0, 10.0);
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&self.parameters));
    }

    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Rrrah RAW viewport"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.012,
                        g: 0.014,
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
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }
    }
}

#[cfg(test)]
fn mosaic_bytes(pixels: &[u16], width: u32) -> (Cow<'_, [u8]>, u32) {
    mosaic_bytes_with_cancel(pixels, width, &|| false).unwrap()
}

#[cfg(target_endian = "little")]
fn mosaic_bytes_with_cancel<'a>(
    pixels: &'a [u16],
    width: u32,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Cow<'a, [u8]>, u32), GpuError> {
    if cancelled() {
        return Err(GpuError::Cancelled);
    }
    if width == 0 {
        return Ok((Cow::Owned(Vec::new()), 0));
    }
    let row_bytes = usize::try_from(width).unwrap_or(usize::MAX).saturating_mul(2);
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
    let row_pitch = (row_bytes.saturating_add(alignment - 1) / alignment).saturating_mul(alignment);
    if row_pitch == row_bytes {
        Ok((Cow::Borrowed(bytemuck::cast_slice(pixels)), row_pitch as u32))
    } else {
        let height = pixels
            .len()
            .checked_div(usize::try_from(width).unwrap_or(1))
            .unwrap_or(0);
        let mut padded = vec![0_u8; row_pitch.saturating_mul(height)];
        let source = bytemuck::cast_slice::<u16, u8>(pixels);
        for row in 0..height {
            if row % 64 == 0 && cancelled() {
                return Err(GpuError::Cancelled);
            }
            let source_range = row * row_bytes..(row + 1) * row_bytes;
            let destination_range = row * row_pitch..row * row_pitch + row_bytes;
            padded[destination_range].copy_from_slice(&source[source_range]);
        }
        {
            if cancelled() {
                return Err(GpuError::Cancelled);
            }
            Ok((Cow::Owned(padded), row_pitch as u32))
        }
    }
}

#[cfg(test)]
fn tile_with_halo(
    pixels: &[u16],
    width: u32,
    height: u32,
    tile_x: u32,
    tile_y: u32,
    tile_size: u32,
    halo: u32,
) -> Vec<u16> {
    tile_with_halo_with_cancel(pixels, width, height, tile_x, tile_y, tile_size, halo, &|| false).unwrap()
}

fn tile_with_halo_into(
    output: &mut Vec<u16>,
    pixels: &[u16],
    width: u32,
    height: u32,
    tile_x: u32,
    tile_y: u32,
    tile_size: u32,
    halo: u32,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), GpuError> {
    if cancelled() {
        return Err(GpuError::Cancelled);
    }
    let extent_u32 = tile_size.checked_add(halo.saturating_mul(2)).unwrap_or(0);
    let extent = usize::try_from(extent_u32).unwrap_or(0);
    let total_samples = extent.saturating_mul(extent);
    output.clear();
    output.resize(total_samples, 0);
    let width = usize::try_from(width).unwrap_or(0);
    let height = usize::try_from(height).unwrap_or(0);
    let tile_x = usize::try_from(tile_x.saturating_mul(tile_size)).unwrap_or(0);
    let tile_y = usize::try_from(tile_y.saturating_mul(tile_size)).unwrap_or(0);
    let halo = usize::try_from(halo).unwrap_or(0);
    if width == 0 || height == 0 || pixels.len() < width.saturating_mul(height) {
        return Ok(());
    }

    let left_fill = halo.saturating_sub(tile_x).min(extent);
    let source_x = tile_x.saturating_sub(halo).min(width.saturating_sub(1));
    let copy_len = width
        .saturating_sub(source_x)
        .min(extent.saturating_sub(left_fill));
    for local_y in 0..extent {
        if local_y % 64 == 0 && cancelled() {
            return Err(GpuError::Cancelled);
        }
        let source_y = tile_y
            .saturating_add(local_y)
            .saturating_sub(halo)
            .min(height.saturating_sub(1));
        let source_row = &pixels[source_y * width..(source_y + 1) * width];
        let output_row = &mut output[local_y * extent..(local_y + 1) * extent];

        if left_fill != 0 {
            output_row[..left_fill].fill(source_row[source_x]);
        }
        if copy_len != 0 {
            output_row[left_fill..left_fill + copy_len]
                .copy_from_slice(&source_row[source_x..source_x + copy_len]);
        }
        if left_fill + copy_len < extent {
            output_row[left_fill + copy_len..].fill(source_row[width - 1]);
        }
    }
    if cancelled() {
        return Err(GpuError::Cancelled);
    }
    Ok(())
}

#[cfg(test)]
fn tile_with_halo_with_cancel(
    pixels: &[u16],
    width: u32,
    height: u32,
    tile_x: u32,
    tile_y: u32,
    tile_size: u32,
    halo: u32,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u16>, GpuError> {
    let mut output = Vec::new();
    tile_with_halo_into(
        &mut output,
        pixels,
        width,
        height,
        tile_x,
        tile_y,
        tile_size,
        halo,
        cancelled,
    )?;
    Ok(output)
}

#[cfg(target_endian = "big")]
fn mosaic_bytes_with_cancel<'a>(
    pixels: &'a [u16],
    width: u32,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Cow<'a, [u8]>, u32), GpuError> {
    if cancelled() {
        return Err(GpuError::Cancelled);
    }
    if width == 0 {
        return Ok((Cow::Owned(Vec::new()), 0));
    }
    let row_bytes = usize::try_from(width).unwrap_or(usize::MAX).saturating_mul(2);
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
    let row_pitch = (row_bytes.saturating_add(alignment - 1) / alignment).saturating_mul(alignment);
    let height = pixels
        .len()
        .checked_div(usize::try_from(width).unwrap_or(1))
        .unwrap_or(0);
    let mut padded = vec![0_u8; row_pitch.saturating_mul(height)];
    for (index, sample) in pixels.iter().enumerate() {
        if index % 4096 == 0 && cancelled() {
            return Err(GpuError::Cancelled);
        }
        let row = index / usize::try_from(width).unwrap_or(1);
        let column = index % usize::try_from(width).unwrap_or(1);
        padded[row * row_pitch + column * 2..row * row_pitch + column * 2 + 2]
            .copy_from_slice(&sample.to_le_bytes());
    }
    {
        if cancelled() {
            return Err(GpuError::Cancelled);
        }
        Ok((Cow::Owned(padded), row_pitch as u32))
    }
}

#[derive(Debug, Error)]
pub enum GpuError {
    #[error("RAW GPU upload cancelled")]
    Cancelled,
    #[error("scene-linear RAW output requires a signed float attachment, got {0:?}")]
    InvalidLinearTarget(wgpu::TextureFormat),
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
    #[error("camera calibration cannot produce a valid color transform")]
    InvalidColorProfile,
    #[error("GPU fast path currently supports single-plane CFA RAW only")]
    UnsupportedPhotometric,
    #[error("CFA metadata is missing")]
    MissingCfa,
    #[error("invalid sensor levels: black {black}, white {white}")]
    InvalidLevels { black: f32, white: f32 },
    #[error("sensor correction at ({x}, {y}) cannot remain finite on the GPU")]
    NonFiniteSensorCorrection { x: u32, y: u32 },
    #[error("RAW correction exceeds safe shader filter range at ({x}, {y})")]
    ShaderFilterRangeExceeded { x: u32, y: u32 },
    #[error("RAW dimensions {width}x{height} exceed GPU texture limit {max}")]
    DimensionsTooLarge { width: u32, height: u32, max: u32 },
    #[error("GPU adapter cannot fit a tile in texture limit {max}")]
    TileTooLargeForAdapter { max: u32 },
    #[error("invalid tiling override: tile_size {tile_size}, halo {tile_halo}, texture limit {max}")]
    InvalidTilingOverride {
        tile_size: u32,
        tile_halo: u32,
        max: u32,
    },
    #[error("RAW tile grid exceeds the GPU texture-array layer limit")]
    TooManyTiles,
    #[error("eager GPU atlas requires {bytes} bytes; limit is {max} bytes")]
    AtlasTooLarge { bytes: u64, max: u64 },
    #[error("unsupported decoded frame: {0}")]
    Frame(#[from] FrameError),
}

#[cfg(test)]
mod tests {
    use rrrah_core::{apply_3x3, camera_to_linear_srgb};

    use super::{
        GpuError, GpuParameters, TilingOverrides, display_white_balance, mosaic_bytes,
        mosaic_bytes_with_cancel, plan_tiling, tile_with_halo, tile_with_halo_with_cancel,
    };

    #[test]
    fn padded_upload_rows_cancel_and_preserve_bytes() {
        let pixels: Vec<u16> = (0..257 * 513).map(|value| (value % 65536) as u16).collect();
        let (bytes, pitch) = mosaic_bytes_with_cancel(&pixels, 257, &|| false).unwrap();
        assert_eq!(pitch, 768);
        for (source, row) in pixels.chunks_exact(257).zip(bytes.chunks_exact(pitch as usize)) {
            for (sample, encoded) in source.iter().zip(row[..514].chunks_exact(2)) {
                assert_eq!(*sample, u16::from_le_bytes(encoded.try_into().unwrap()));
            }
            assert!(row[514..].iter().all(|value| *value == 0));
        }
        let polls = std::cell::Cell::new(0usize);
        mosaic_bytes_with_cancel(&pixels, 257, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        assert!(polls.get() >= 11);
        for stop in 1..=polls.get() {
            let calls = std::cell::Cell::new(0usize);
            assert!(matches!(
                mosaic_bytes_with_cancel(&pixels, 257, &|| {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }),
                Err(GpuError::Cancelled)
            ));
        }
    }

    #[test]
    fn halo_packing_cancels_inside_large_tiles_and_preserves_scalar_samples() {
        let pixels: Vec<u16> = (0..513 * 257).map(|value| (value % 65536) as u16).collect();
        let expected = scalar_tile_with_halo(&pixels, 513, 257, 0, 0, 512, 1);
        assert_eq!(
            tile_with_halo_with_cancel(&pixels, 513, 257, 0, 0, 512, 1, &|| false).unwrap(),
            expected
        );
        // Initial check, nine 64-row intervals in a 514-row tile, final check.
        for stop in 1..=11 {
            let calls = std::cell::Cell::new(0usize);
            assert!(
                matches!(
                    tile_with_halo_with_cancel(&pixels, 513, 257, 0, 0, 512, 1, &|| {
                        calls.set(calls.get() + 1);
                        calls.get() == stop
                    }),
                    Err(GpuError::Cancelled)
                ),
                "checkpoint {stop}"
            );
            assert_eq!(calls.get(), stop);
        }
    }

    /// Deliberately scalar oracle for the optimized row-wise tile packer.
    ///
    /// Keep the coordinate expression independent from the production
    /// copy/fill decomposition so randomized comparisons catch boundary and
    /// saturation mistakes rather than reproducing them.
    fn scalar_tile_with_halo(
        pixels: &[u16],
        width: u32,
        height: u32,
        tile_x: u32,
        tile_y: u32,
        tile_size: u32,
        halo: u32,
    ) -> Vec<u16> {
        let extent_u32 = tile_size.checked_add(halo.saturating_mul(2)).unwrap_or(0);
        let extent = usize::try_from(extent_u32).unwrap_or(0);
        let width = usize::try_from(width).unwrap_or(0);
        let height = usize::try_from(height).unwrap_or(0);
        let tile_x = usize::try_from(tile_x.saturating_mul(tile_size)).unwrap_or(0);
        let tile_y = usize::try_from(tile_y.saturating_mul(tile_size)).unwrap_or(0);
        let halo = usize::try_from(halo).unwrap_or(0);
        let mut output = vec![0_u16; extent.saturating_mul(extent)];
        if width == 0 || height == 0 || pixels.len() < width.saturating_mul(height) {
            return output;
        }
        for local_y in 0..extent {
            for local_x in 0..extent {
                let source_x = tile_x
                    .saturating_add(local_x)
                    .saturating_sub(halo)
                    .min(width.saturating_sub(1));
                let source_y = tile_y
                    .saturating_add(local_y)
                    .saturating_sub(halo)
                    .min(height.saturating_sub(1));
                output[local_y * extent + local_x] = pixels[source_y * width + source_x];
            }
        }
        output
    }

    fn next_random(state: &mut u64) -> u32 {
        // Fixed xorshift64 sequence: deterministic and dependency-free.
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state as u32
    }

    #[test]
    fn display_keeps_decoder_white_balance_gains_bit_exact() {
        let gains = [1_678.0 / 1_024.0, 1.0, 1_659.0 / 1_024.0, 1.0];
        assert_eq!(
            display_white_balance(gains).map(f32::to_bits),
            gains.map(f32::to_bits)
        );
    }

    #[test]
    fn direct_white_balance_preserves_backend_neutral_contract() {
        let decoder_cases: [[f32; 4]; 2] = [
            [1_678.0 / 1_024.0, 1.0, 1_659.0 / 1_024.0, 1.0],
            [2.0, 1.0, 1.25, 1.0],
        ];
        let camera_to_rgb = camera_to_linear_srgb([
            [0.9539, -0.2795, -0.1224],
            [-0.4175, 1.1998, 0.2458],
            [-0.0465, 0.1755, 0.6048],
            [0.0; 3],
        ])
        .expect("test camera matrix must be invertible");

        for decoder_gains in decoder_cases {
            let as_shot_neutral = [
                decoder_gains[0].recip(),
                decoder_gains[1].recip(),
                decoder_gains[2].recip(),
            ];
            let uploaded_gains = display_white_balance(decoder_gains);
            let corrected = [
                uploaded_gains[0] * as_shot_neutral[0],
                uploaded_gains[1] * as_shot_neutral[1],
                uploaded_gains[2] * as_shot_neutral[2],
            ];
            for channel in corrected {
                assert!((channel - 1.0).abs() < f32::EPSILON);
            }
            let display_neutral = apply_3x3(camera_to_rgb, corrected);
            for channel in display_neutral {
                assert!((channel - 1.0).abs() < 2.0e-5);
            }
        }
    }

    #[test]
    fn viewport_shader_parses_and_validates_with_naga() {
        // This is deliberately backend-independent. wgpu performs the same
        // Naga validation at device creation, but a host-side test gives us a
        // fast CI gate even on machines without Metal/Vulkan/DX12/WebGPU.
        let module = wgpu::naga::front::wgsl::parse_str(include_str!("../shaders/raw_view.wgsl"))
            .expect("raw_view.wgsl must remain valid WGSL");
        let mut validator = wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            // Validate against the portable baseline. Optional subgroup,
            // f16, or vendor capabilities must be requested explicitly by a
            // future shader/backend rather than being hidden by this test.
            wgpu::naga::valid::Capabilities::empty(),
        );
        validator
            .validate(&module)
            .expect("raw_view.wgsl must pass Naga validation");
        let entry_points: Vec<_> = module
            .entry_points
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert!(entry_points.contains(&"vs_main"));
        assert!(entry_points.contains(&"fs_main"));
    }

    #[test]
    fn uniform_layout_remains_wgsl_compatible() {
        assert_eq!(size_of::<GpuParameters>(), 240);
        assert_eq!(align_of::<GpuParameters>(), 16);
    }

    #[test]
    fn tile_halo_matches_full_frame_at_internal_boundary() {
        // A monotonic sensor ramp makes an off-by-one in the halo immediately
        // observable. The right halo of tile 0 must be the first sample of
        // tile 1, not a clamped edge value.
        let width = 8;
        let height = 4;
        let pixels: Vec<u16> = (0..width * height).map(|index| index as u16).collect();
        let left = tile_with_halo(&pixels, width, height, 0, 0, 4, 1);
        let right = tile_with_halo(&pixels, width, height, 1, 0, 4, 1);
        // row 2 is the first interior row (row 1 is the top halo). The right halo of tile 0 is
        // sensor x=4; the left halo of tile 1 is sensor x=3. Together with
        // each tile's interior they cover the exact same boundary samples.
        let width_usize = width as usize;
        assert_eq!(left[2 * 6 + 4], pixels[width_usize + 3]);
        assert_eq!(left[2 * 6 + 5], pixels[width_usize + 4]);
        assert_eq!(right[2 * 6], pixels[width_usize + 3]);
        assert_eq!(right[2 * 6 + 1], pixels[width_usize + 4]);
    }

    #[test]
    fn edge_tiles_clamp_without_out_of_bounds_reads() {
        let pixels = vec![11_u16, 12, 21, 22];
        let tile = tile_with_halo(&pixels, 2, 2, 0, 0, 4, 1);
        assert_eq!(tile.len(), 36);
        // Top-left and bottom-right corners clamp to the corresponding edge
        // samples. This is the defined border policy for the preview path.
        assert_eq!(tile[0], 11);
        assert_eq!(tile[35], 22);
    }

    #[test]
    fn rowwise_tile_halo_matches_scalar_oracle_across_randomized_geometry() {
        let mut random = 0x6a09_e667_f3bc_c909_u64;
        for case in 0..512 {
            let width = next_random(&mut random) % 65;
            let height = next_random(&mut random) % 65;
            let tile_size = next_random(&mut random) % 32 + 1;
            let halo = next_random(&mut random) % 9;
            let tile_columns = width.div_ceil(tile_size);
            let tile_rows = height.div_ceil(tile_size);
            // Include one-past-grid coordinates: the helper's historical
            // behavior clamps those oversized tiles and must stay panic-free.
            let tile_x = next_random(&mut random) % (tile_columns + 2);
            let tile_y = next_random(&mut random) % (tile_rows + 2);
            let sample_count = usize::try_from(width.saturating_mul(height)).unwrap();
            let mut pixels = (0..sample_count)
                .map(|_| next_random(&mut random) as u16)
                .collect::<Vec<_>>();
            if case % 17 == 0 && !pixels.is_empty() {
                // Malformed/truncated input must retain the all-zero fallback.
                pixels.pop();
            }

            let expected = scalar_tile_with_halo(&pixels, width, height, tile_x, tile_y, tile_size, halo);
            let actual = tile_with_halo(&pixels, width, height, tile_x, tile_y, tile_size, halo);
            assert_eq!(
                actual, expected,
                "case {case}: frame={width}x{height}, tile=({tile_x},{tile_y}), \
                 tile_size={tile_size}, halo={halo}"
            );
        }
    }

    #[test]
    fn upload_rows_are_padded_to_webgpu_alignment() {
        let (bytes, pitch) = mosaic_bytes(&[1_u16, 2, 3], 3);
        assert_eq!(pitch as usize % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize, 0);
        assert_eq!(pitch, 256);
        assert_eq!(&bytes.as_ref()[..6], &[1, 0, 2, 0, 3, 0]);
        assert!(bytes.as_ref()[6..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn malformed_dimensions_do_not_panic_helper_paths() {
        let (bytes, pitch) = mosaic_bytes(&[1_u16], 0);
        assert!(bytes.is_empty());
        assert_eq!(pitch, 0);

        let tile = tile_with_halo(&[1_u16], 0, 1, 0, 0, 4, 1);
        assert_eq!(tile.len(), 36);
        assert!(tile.iter().all(|sample| *sample == 0));

        let tile = tile_with_halo(&[1_u16], 2, 2, 0, 0, 4, 1);
        assert_eq!(tile.len(), 36);
        assert!(tile.iter().all(|sample| *sample == 0));
    }

    #[test]
    fn default_tiling_uses_c2_optimum_with_aligned_extent() {
        // Metal-class adapter: 16384 texture dimension, 2048 array layers.
        // Experiment C2: tile ~= 1024 with halo 1 is the M5 optimum; the
        // stored extent snaps down to 1024 (a multiple of 128 samples) so
        // `row_pack` degenerates to a copy-free upload.
        let plan = plan_tiling(8256, 6192, 16_384, 2048, TilingOverrides::default()).unwrap();
        assert_eq!(plan.tile_halo, 1);
        assert_eq!(plan.tile_size, 1022);
        assert_eq!(plan.texture_extent, 1024);
        assert_eq!(plan.texture_extent % 128, 0);
        assert_eq!(plan.tile_grid, [9, 7]);
        assert_eq!(plan.layer_count, 63);
        assert_eq!(plan.atlas_bytes, 1024_u64 * 1024 * 63 * 2);
    }

    #[test]
    fn default_tiling_uses_one_small_frame_tile_when_atlas_is_materially_smaller() {
        let plan = plan_tiling(2048, 1536, 16_384, 2048, TilingOverrides::default()).unwrap();
        assert_eq!(plan.tile_size, 2048);
        assert_eq!(plan.texture_extent, 2050);
        assert_eq!(plan.tile_grid, [1, 1]);
        assert_eq!(plan.atlas_bytes, 2050_u64 * 2050 * 2);
    }

    #[test]
    fn default_tiling_avoids_the_4096_frame_regression() {
        // The aligned 1022 plan needs 20 layers (40 MiB), while one exact
        // frame-sized tile needs about 32 MiB and measured 15–24% faster.
        let plan = plan_tiling(4096, 3072, 16_384, 2048, TilingOverrides::default()).unwrap();
        assert_eq!(plan.tile_size, 4096);
        assert_eq!(plan.texture_extent, 4098);
        assert_eq!(plan.tile_grid, [1, 1]);
        assert_eq!(plan.atlas_bytes, 4098_u64 * 4098 * 2);
    }

    #[test]
    fn default_tiling_keeps_aligned_tiles_when_one_layer_is_not_smaller() {
        let plan = plan_tiling(6240, 4160, 16_384, 2048, TilingOverrides::default()).unwrap();
        assert_eq!(plan.tile_size, 1022);
        assert_eq!(plan.texture_extent, 1024);
        assert_eq!(plan.tile_grid, [7, 5]);
    }

    #[test]
    fn default_tiling_shrinks_for_small_adapters() {
        // Adapter limited to 4096: the C2 default still fits, so the tile
        // stays at the aligned 1022 rather than the legacy 4094.
        let plan = plan_tiling(6000, 4000, 4096, 256, TilingOverrides::default()).unwrap();
        assert_eq!(plan.tile_size, 1022);
        assert_eq!(plan.texture_extent, 1024);
        assert_eq!(plan.tile_grid, [6, 4]);
    }

    #[test]
    fn default_tiling_rejects_tiny_adapters() {
        let error = plan_tiling(64, 64, 33, 256, TilingOverrides::default()).unwrap_err();
        assert!(matches!(error, GpuError::TileTooLargeForAdapter { max: 33 }));
    }

    #[test]
    fn default_tiling_uses_one_frame_tile_when_layer_growth_would_waste_atlas_space() {
        // 8192x8192 at tile 1022 needs 81 layers. One exact tile fits this
        // adapter and is much smaller than the aligned 2x2 fallback.
        let plan = plan_tiling(8192, 8192, 16_384, 8, TilingOverrides::default()).unwrap();
        assert_eq!(plan.tile_size, 8192);
        assert_eq!(plan.texture_extent, 8194);
        assert_eq!(plan.tile_grid, [1, 1]);
        assert_eq!(plan.layer_count, 1);
    }

    #[test]
    fn default_tiling_errors_when_layer_limit_cannot_be_met() {
        // The tile cannot grow past the texture dimension, so an 8192x8192
        // mosaic with a 4-layer budget is rejected.
        let error = plan_tiling(8192, 8192, 4096, 4, TilingOverrides::default()).unwrap_err();
        assert!(matches!(error, GpuError::TooManyTiles));
    }

    #[test]
    fn tiny_mosaics_use_one_minimum_tile_instead_of_full_default_atlas() {
        for (width, height) in [(1, 1), (16, 16), (7, 31), (31, 4)] {
            let plan = plan_tiling(width, height, 4096, 256, TilingOverrides::default()).unwrap();
            assert_eq!(plan.tile_size, crate::MIN_TILE_SIZE);
            assert_eq!(plan.tile_grid, [1, 1]);
            assert_eq!(plan.texture_extent, 34);
            assert_eq!(plan.atlas_bytes, 2312);
        }
    }

    #[test]
    fn tiling_override_is_applied_verbatim() {
        let overrides = TilingOverrides {
            tile_size: Some(512),
            tile_halo: Some(2),
        };
        let plan = plan_tiling(2048, 1536, 16_384, 2048, overrides).unwrap();
        assert_eq!(plan.tile_size, 512);
        assert_eq!(plan.tile_halo, 2);
        assert_eq!(plan.texture_extent, 516);
        assert_eq!(plan.tile_grid, [4, 3]);
        assert_eq!(plan.layer_count, 12);
        assert_eq!(plan.atlas_bytes, 516_u64 * 516 * 12 * 2);
    }

    #[test]
    fn tiling_override_below_minimum_is_rejected() {
        let overrides = TilingOverrides {
            tile_size: Some(16),
            tile_halo: None,
        };
        let error = plan_tiling(64, 64, 16_384, 2048, overrides).unwrap_err();
        assert!(matches!(
            error,
            GpuError::InvalidTilingOverride {
                tile_size: 16,
                tile_halo: 1,
                max: 16_384
            }
        ));
    }

    #[test]
    fn tiling_override_exceeding_texture_limit_is_rejected() {
        let overrides = TilingOverrides {
            tile_size: Some(4096),
            tile_halo: Some(4),
        };
        let error = plan_tiling(4096, 4096, 4096, 2048, overrides).unwrap_err();
        assert!(matches!(
            error,
            GpuError::InvalidTilingOverride {
                tile_size: 4096,
                tile_halo: 4,
                max: 4096
            }
        ));
    }

    #[test]
    fn tiling_override_respects_array_layer_limit() {
        let overrides = TilingOverrides {
            tile_size: Some(32),
            tile_halo: None,
        };
        // 128x64 at tile 32 -> 4x2 = 8 layers, limit 4.
        let error = plan_tiling(128, 64, 16_384, 4, overrides).unwrap_err();
        assert!(matches!(error, GpuError::TooManyTiles));
    }
}

#[cfg(test)]
mod capacity_tiling_tests {
    use super::*;
    #[test]
    fn extreme_explicit_halos_and_edges_refuse_without_overflow() {
        for (tile, halo) in [(32, u32::MAX), (32, 1 << 31), (u32::MAX, 1), (u32::MAX - 1, 1)] {
            let overrides = TilingOverrides {
                tile_size: Some(tile),
                tile_halo: Some(halo),
            };
            assert!(matches!(
                plan_tiling(64, 64, u32::MAX, 2048, overrides),
                Err(GpuError::InvalidTilingOverride { .. })
            ));
            assert!(matches!(
                fit_tiling_capacity(64, 64, u32::MAX, 2048, overrides, u64::MAX),
                Err(GpuError::InvalidTilingOverride { .. })
            ));
        }
        assert!(matches!(
            plan_tiling(
                64,
                64,
                16384,
                2048,
                TilingOverrides {
                    tile_size: None,
                    tile_halo: Some(u32::MAX)
                }
            ),
            Err(GpuError::InvalidTilingOverride { .. })
        ));
    }

    #[test]
    fn default_eager_cap_refusal_still_searches_admissible_geometry() {
        let overrides = TilingOverrides::default();
        assert!(matches!(
            plan_tiling(16370, 16370, 16384, 2048, overrides),
            Err(GpuError::AtlasTooLarge { .. })
        ));
        let fit = fit_tiling_capacity(16370, 16370, 16384, 2048, overrides, MAX_EAGER_ATLAS_BYTES).unwrap();
        assert!(fit.atlas_bytes <= MAX_EAGER_ATLAS_BYTES);
        assert_eq!(fit.tile_grid, [1, 1]);
        assert_eq!(fit.tile_size, 16370);
        let explicit = TilingOverrides {
            tile_size: Some(1022),
            tile_halo: Some(1),
        };
        assert!(matches!(
            fit_tiling_capacity(16370, 16370, 16384, 2048, explicit, u64::MAX),
            Err(GpuError::AtlasTooLarge { .. })
        ));
    }

    #[test]
    fn tight_capacity_finds_unaligned_edge_and_matches_exhaustive_minimum() {
        let overrides = TilingOverrides {
            tile_size: None,
            tile_halo: Some(1),
        };
        let tight = fit_tiling_capacity(63, 129, 1024, 256, overrides, 18000).unwrap();
        assert_eq!((tight.tile_size, tight.atlas_bytes), (65, 17956));
        for width in [2, 31, 63, 95, 163, 257, 500] {
            for height in [3, 47, 79, 129, 211, 417] {
                for halo in [1, 2, 3] {
                    let overrides = TilingOverrides {
                        tile_size: None,
                        tile_halo: Some(halo),
                    };
                    let actual = fit_tiling_capacity(width, height, 1024, 256, overrides, 0).unwrap();
                    let expected = (MIN_TILE_SIZE..=1024 - 2 * halo)
                        .filter_map(|tile| {
                            plan_tiling(
                                width,
                                height,
                                1024,
                                256,
                                TilingOverrides {
                                    tile_size: Some(tile),
                                    ..overrides
                                },
                            )
                            .ok()
                        })
                        .map(|plan| plan.atlas_bytes)
                        .min()
                        .unwrap();
                    assert_eq!(actual.atlas_bytes, expected, "{width}x{height} halo={halo}");
                }
            }
        }
    }

    #[test]
    fn real_cr3_shape_fits_64_mib_without_changing_unlimited_default() {
        for halo in [1, 2, 3] {
            let overrides = TilingOverrides {
                tile_halo: Some(halo),
                tile_size: None,
            };
            let default = plan_tiling(6188, 4120, 16384, 2048, overrides).unwrap();
            assert_eq!(
                fit_tiling_capacity(6188, 4120, 16384, 2048, overrides, u64::MAX).unwrap(),
                default
            );
            let constrained =
                fit_tiling_capacity(6188, 4120, 16384, 2048, overrides, 64 * 1024 * 1024).unwrap();
            assert!(constrained.atlas_bytes <= 64 * 1024 * 1024);
            assert_eq!(constrained.tile_halo, halo);
            assert!(constrained.tile_grid[0] * constrained.tile_size >= 6188);
            assert!(constrained.tile_grid[1] * constrained.tile_size >= 4120);
            let explicit = TilingOverrides {
                tile_size: Some(default.tile_size),
                ..overrides
            };
            assert_eq!(
                fit_tiling_capacity(6188, 4120, 16384, 2048, explicit, 1).unwrap(),
                default
            );
        }
    }
}

#[cfg(test)]
mod raw_reuse_qualification;
#[cfg(test)]
mod telemetry_pass_qualification;
