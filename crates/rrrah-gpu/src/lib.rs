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
use rrrah_core::{
    DecodedMosaic, FrameError, Photometric, camera_to_linear_srgb, luminance_normalize_wb_gains,
};
use thiserror::Error;
use wgpu::util::DeviceExt;

/// Hard upper bound for the eager atlas path. A 512 MiB cap prevents a
/// malformed/huge frame from causing an avoidable device-loss while keeping
/// ordinary 45–60 MP cameras within the fast path. Larger images must use the
/// residency/tiled uploader rather than allocating the complete atlas.
const MAX_EAGER_ATLAS_BYTES: u64 = 512 * 1024 * 1024;

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
/// Default upper bound for the interior tile size. The experiment-C sweep
/// (docs/experiments/C.md) measured the Apple M5 optimum at tile 1024 with
/// halo 1: 81 ms vs 124 ms on a 100 MP frame and 4.1 ms vs 18.2 ms on a 6 MP
/// frame compared with the legacy 4096 bound.
pub const DEFAULT_MAX_TILE_SIZE: u32 = 1024;
/// Minimum interior tile size accepted by the atlas planner.
pub const MIN_TILE_SIZE: u32 = 32;
/// The default tiling aligns the stored texture extent to this many samples so
/// each atlas row is a multiple of `wgpu::COPY_BYTES_PER_ROW_ALIGNMENT`
/// (128 samples * 2 bytes = 256 bytes) and `row_pack` degenerates to a
/// copy-free upload (experiment C2: another 5-15% off total).
const EXTENT_ALIGNMENT_SAMPLES: u32 = 128;

/// Optional overrides for the atlas tiling used by
/// [`RawRenderer::upload_mosaic_with_tiling`]. `None` keeps the defaults; the
/// defaults are unchanged from previous releases.
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
    let tile_size = overrides.tile_size.unwrap_or_else(|| {
        default_tile_size(width, height, max_dimension, max_array_layers, tile_halo)
    });
    let texture_extent = tile_size.saturating_add(2 * tile_halo);
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

/// Default interior tile size when no explicit override is given: the C2
/// optimum (bounded by [`DEFAULT_MAX_TILE_SIZE`]), with the stored texture
/// extent rounded down to a multiple of [`EXTENT_ALIGNMENT_SAMPLES`], grown
/// as needed so the tile grid still fits `max_array_layers`.
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
    tile
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

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct HudVertex {
    position: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct HudUniforms {
    viewport: [f32; 2],
    _padding: [f32; 2],
}

const HUD_CARD_MARGIN: f32 = 16.0;
const HUD_CARD_GAP: f32 = 12.0;
const HUD_CARD_ACCENT: [f32; 4] = [0.16, 0.58, 0.92, 0.96];
const HUD_CARD_NOMINAL_VIEWPORT: [f32; 2] = [780.0, 620.0];
const HUD_CARD_DENSE_NOMINAL_VIEWPORT: [f32; 2] = [1600.0, 920.0];
const HUD_CARD_SINGLE_COLUMN_LIMIT: usize = 4;
const HUD_CARD_TWO_COLUMN_LIMIT: usize = 10;
const HUD_CARD_THREE_COLUMN_LIMIT: usize = 15;
const HUD_CARD_FOUR_COLUMN_LIMIT: usize = 24;
const HUD_CARD_COMPACT_MAX_WIDTH: f32 = 420.0;
const HUD_CARD_COMPACT_MAX_HEIGHT: f32 = 120.0;

/// One structured block in a telemetry HUD.
///
/// `time` is deliberately a string instead of a duration so callers can show
/// a measured value (for example `"14.2 MS"`) or a state such as `"WAITING"`.
/// The optional accent is an RGBA color in linear floating-point components.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudCard<'a> {
    pub title: &'a str,
    pub time: &'a str,
    pub description: &'a str,
    pub status: Option<&'a str>,
    pub accent: Option<[f32; 4]>,
}

impl<'a> HudCard<'a> {
    pub const fn new(title: &'a str, time: &'a str, description: &'a str) -> Self {
        Self {
            title,
            time,
            description,
            status: None,
            accent: None,
        }
    }

    #[must_use]
    pub const fn with_status(mut self, status: &'a str) -> Self {
        self.status = Some(status);
        self
    }

    #[must_use]
    pub const fn with_accent(mut self, accent: [f32; 4]) -> Self {
        self.accent = Some(accent);
        self
    }
}

/// Pixel bounds assigned to a structured HUD card.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudCardBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Computes a responsive card layout without requiring a GPU device. Up to
/// four cards are stacked vertically, then longer pipelines progressively use
/// two, three, four or five columns. At the compact telemetry window's nominal
/// 780x620 size, four cards are 748x138 pixels with 12 pixel gaps and 16 pixel
/// outer margins. Pipelines with at least 16 stages use the dense dashboard's
/// 1600x920 nominal viewport so its geometry and text scale remain readable.
#[must_use]
pub fn hud_card_layout(viewport: [f32; 2], card_count: usize) -> Vec<HudCardBounds> {
    if card_count == 0 {
        return Vec::new();
    }

    let viewport_width = finite_dimension(viewport[0]);
    let viewport_height = finite_dimension(viewport[1]);
    let scale = hud_card_scale_for_count([viewport_width, viewport_height], card_count);
    let horizontal_margin = (HUD_CARD_MARGIN * scale).min(viewport_width * 0.5);
    let vertical_margin = (HUD_CARD_MARGIN * scale).min(viewport_height * 0.5);
    let content_width = (viewport_width - horizontal_margin * 2.0).max(0.0);
    let content_height = (viewport_height - vertical_margin * 2.0).max(0.0);
    let columns = match card_count {
        0..=HUD_CARD_SINGLE_COLUMN_LIMIT => 1,
        5..=HUD_CARD_TWO_COLUMN_LIMIT => 2,
        11..=HUD_CARD_THREE_COLUMN_LIMIT => 3,
        16..=HUD_CARD_FOUR_COLUMN_LIMIT => 4,
        _ => 5,
    };
    let rows = card_count.div_ceil(columns);
    let horizontal_gap_count = columns.saturating_sub(1) as f32;
    let vertical_gap_count = rows.saturating_sub(1) as f32;
    let horizontal_gap = if horizontal_gap_count == 0.0 {
        0.0
    } else {
        (HUD_CARD_GAP * scale).min(content_width / horizontal_gap_count)
    };
    let vertical_gap = if vertical_gap_count == 0.0 {
        0.0
    } else {
        (HUD_CARD_GAP * scale).min(content_height / vertical_gap_count)
    };
    let card_width = ((content_width - horizontal_gap * horizontal_gap_count) / columns as f32).max(0.0);
    let card_height = ((content_height - vertical_gap * vertical_gap_count) / rows as f32).max(0.0);

    (0..card_count)
        .map(|index| {
            let column = index % columns;
            let row = index / columns;
            HudCardBounds {
                x: horizontal_margin + column as f32 * (card_width + horizontal_gap),
                y: vertical_margin + row as f32 * (card_height + vertical_gap),
                width: card_width,
                height: card_height,
            }
        })
        .collect()
}

fn hud_card_scale(viewport: [f32; 2]) -> f32 {
    let width_scale = finite_dimension(viewport[0]) / HUD_CARD_NOMINAL_VIEWPORT[0];
    let height_scale = finite_dimension(viewport[1]) / HUD_CARD_NOMINAL_VIEWPORT[1];
    width_scale.min(height_scale).clamp(0.5, 4.0)
}

fn hud_card_scale_for_count(viewport: [f32; 2], card_count: usize) -> f32 {
    if card_count <= HUD_CARD_THREE_COLUMN_LIMIT {
        return hud_card_scale(viewport);
    }

    let width_scale = finite_dimension(viewport[0]) / HUD_CARD_DENSE_NOMINAL_VIEWPORT[0];
    let height_scale = finite_dimension(viewport[1]) / HUD_CARD_DENSE_NOMINAL_VIEWPORT[1];
    width_scale.min(height_scale).clamp(0.5, 4.0)
}

fn finite_dimension(value: f32) -> f32 {
    if value.is_finite() { value.max(0.0) } else { 0.0 }
}

#[allow(clippy::cast_sign_loss)]
fn floor_to_usize(value: f32) -> usize {
    finite_dimension(value).floor() as usize
}

#[derive(Debug, Clone, Copy)]
struct HudCardMetrics {
    content_left_inset: f32,
    content_right_inset: f32,
    heading_scale: f32,
    heading_y_offset: f32,
    divider_y_offset: f32,
    body_scale: f32,
    status_y_offset: f32,
    description_y_offset: f32,
    description_without_status_y_offset: f32,
    bottom_inset: f32,
}

fn hud_card_metrics(bounds: HudCardBounds, scale: f32) -> HudCardMetrics {
    let compact = bounds.width <= HUD_CARD_COMPACT_MAX_WIDTH * scale
        || bounds.height <= HUD_CARD_COMPACT_MAX_HEIGHT * scale;
    let values = if compact {
        (14.0, 12.0, 2.0, 10.0, 30.0, 1.75, 36.0, 54.0, 37.0, 8.0)
    } else {
        (22.0, 18.0, 2.5, 14.0, 43.0, 2.0, 51.0, 75.0, 54.0, 12.0)
    };
    HudCardMetrics {
        content_left_inset: values.0 * scale,
        content_right_inset: values.1 * scale,
        heading_scale: values.2 * scale,
        heading_y_offset: values.3 * scale,
        divider_y_offset: values.4 * scale,
        body_scale: values.5 * scale,
        status_y_offset: values.6 * scale,
        description_y_offset: values.7 * scale,
        description_without_status_y_offset: values.8 * scale,
        bottom_inset: values.9 * scale,
    }
}

fn hud_card_metrics_for_count(bounds: HudCardBounds, scale: f32, card_count: usize) -> HudCardMetrics {
    if card_count <= HUD_CARD_FOUR_COLUMN_LIMIT {
        return hud_card_metrics(bounds, scale);
    }

    HudCardMetrics {
        content_left_inset: 12.0 * scale,
        content_right_inset: 10.0 * scale,
        heading_scale: 1.75 * scale,
        heading_y_offset: 9.0 * scale,
        divider_y_offset: 29.0 * scale,
        body_scale: 1.5 * scale,
        status_y_offset: 35.0 * scale,
        description_y_offset: 52.0 * scale,
        description_without_status_y_offset: 36.0 * scale,
        bottom_inset: 8.0 * scale,
    }
}

/// Small dependency-free bitmap HUD rendered after the RAW pass. Keeping the
/// text rasterizer here avoids coupling the viewer to a font stack while still
/// making decode/cache/upload timings visible in the actual image viewport.
#[derive(Debug)]
pub struct HudRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: u64,
    vertex_count: u32,
    viewport: [f32; 2],
}

impl HudRenderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat, viewport: [f32; 2]) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Rrrah timing HUD shader"),
            source: wgpu::ShaderSource::Wgsl(HUD_SHADER.into()),
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Rrrah timing HUD layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Rrrah timing HUD pipeline layout"),
            bind_group_layouts: &[Some(&uniform_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Rrrah timing HUD pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<HudVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: size_of::<[f32; 2]>() as u64,
                            shader_location: 1,
                        },
                    ],
                })],
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
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        let uniforms = HudUniforms {
            viewport,
            _padding: [0.0; 2],
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Rrrah timing HUD uniforms"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Rrrah timing HUD bind group"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let vertex_capacity = 4096_u64;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Rrrah timing HUD vertices"),
            size: vertex_capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            uniform_buffer,
            uniform_bind_group,
            vertex_buffer,
            vertex_capacity,
            vertex_count: 0,
            viewport,
        }
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, viewport: [f32; 2]) {
        self.viewport = [viewport[0].max(1.0), viewport[1].max(1.0)];
        let uniforms = HudUniforms {
            viewport: self.viewport,
            _padding: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, text: &str) {
        let lines: Vec<Vec<char>> = text
            .lines()
            .map(|line| line.chars().map(|ch| ch.to_ascii_uppercase()).collect())
            .collect();
        if lines.is_empty() {
            self.vertex_count = 0;
            return;
        }
        let scale = 2.5_f32;
        let padding = 12.0_f32;
        let cell_width = 6.0 * scale;
        let cell_height = 8.0 * scale;
        let max_chars = lines.iter().map(Vec::len).max().unwrap_or(0) as f32;
        let panel_width = (padding * 2.0 + max_chars * cell_width).min(self.viewport[0] - 16.0);
        let panel_height = (padding * 2.0 + lines.len() as f32 * cell_height).min(self.viewport[1] - 16.0);
        let mut vertices = Vec::new();
        push_quad(
            &mut vertices,
            8.0,
            8.0,
            panel_width.max(32.0),
            panel_height.max(24.0),
            [0.16, 0.58, 0.92, 0.96],
        );
        push_quad(
            &mut vertices,
            10.0,
            10.0,
            (panel_width - 4.0).max(28.0),
            (panel_height - 4.0).max(20.0),
            [0.004, 0.008, 0.016, 0.97],
        );
        for (line_index, line) in lines.iter().enumerate() {
            let y = 8.0 + padding + line_index as f32 * cell_height;
            for (char_index, character) in line.iter().enumerate() {
                let glyph = glyph_rows(*character);
                let x = 8.0 + padding + char_index as f32 * cell_width;
                for (row, bits) in glyph.iter().enumerate() {
                    for column in 0..5 {
                        if bits & (1 << (4 - column)) != 0 {
                            push_quad(
                                &mut vertices,
                                x + column as f32 * scale,
                                y + row as f32 * scale,
                                scale,
                                scale,
                                [0.96, 0.99, 1.0, 1.0],
                            );
                        }
                    }
                }
            }
        }
        let bytes = bytemuck::cast_slice::<HudVertex, u8>(&vertices);
        if bytes.len() as u64 > self.vertex_capacity {
            self.vertex_capacity = (bytes.len() as u64).next_power_of_two();
            self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Rrrah timing HUD vertices"),
                size: self.vertex_capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        queue.write_buffer(&self.vertex_buffer, 0, bytes);
        self.vertex_count = u32::try_from(vertices.len()).unwrap_or(u32::MAX);
    }

    /// Replaces the HUD contents with structured telemetry cards.
    ///
    /// Cards are laid out in input order and descriptions are word-wrapped to
    /// stay inside each card. Longer pipelines use up to five columns.
    /// This is independent of [`Self::update`], which retains the compact
    /// free-form HUD used by the main image viewport.
    pub fn update_cards(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, cards: &[HudCard<'_>]) {
        if cards.is_empty() {
            self.vertex_count = 0;
            return;
        }

        let bounds = hud_card_layout(self.viewport, cards.len());
        let scale = hud_card_scale_for_count(self.viewport, cards.len());
        let mut vertices = Vec::new();
        for (card, bounds) in cards.iter().zip(bounds) {
            push_hud_card(&mut vertices, card, bounds, scale, cards.len());
        }
        if vertices.is_empty() {
            self.vertex_count = 0;
            return;
        }

        let bytes = bytemuck::cast_slice::<HudVertex, u8>(&vertices);
        if bytes.len() as u64 > self.vertex_capacity {
            self.vertex_capacity = (bytes.len() as u64).next_power_of_two();
            self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Rrrah timing HUD vertices"),
                size: self.vertex_capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        queue.write_buffer(&self.vertex_buffer, 0, bytes);
        self.vertex_count = u32::try_from(vertices.len()).unwrap_or(u32::MAX);
    }

    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        if self.vertex_count == 0 {
            return;
        }
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Rrrah timing HUD"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..self.vertex_count, 0..1);
    }
}

fn push_hud_card(
    vertices: &mut Vec<HudVertex>,
    card: &HudCard<'_>,
    bounds: HudCardBounds,
    scale: f32,
    card_count: usize,
) {
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return;
    }

    let accent = card.accent.unwrap_or(HUD_CARD_ACCENT);
    push_quad(vertices, bounds.x, bounds.y, bounds.width, bounds.height, accent);
    let border = scale;
    if bounds.width > border * 2.0 && bounds.height > border * 2.0 {
        push_quad(
            vertices,
            bounds.x + border,
            bounds.y + border,
            bounds.width - border * 2.0,
            bounds.height - border * 2.0,
            [0.012, 0.022, 0.038, 0.98],
        );
        push_quad(
            vertices,
            bounds.x + border,
            bounds.y + border,
            (5.0 * scale).min(bounds.width - border * 2.0),
            bounds.height - border * 2.0,
            accent,
        );
    }

    let metrics = hud_card_metrics_for_count(bounds, scale, card_count);
    let content_left = bounds.x + metrics.content_left_inset;
    let content_right = bounds.x + bounds.width - metrics.content_right_inset;
    let content_width = (content_right - content_left).max(0.0);
    if content_width == 0.0 {
        return;
    }

    let heading_scale = metrics.heading_scale;
    let heading_cell_width = 6.0 * heading_scale;
    let time_limit = floor_to_usize((content_width * 0.4) / heading_cell_width);
    let time = take_chars(card.time, time_limit);
    let time_width = time.chars().count() as f32 * heading_cell_width;
    let title_width = (content_width - time_width - heading_cell_width).max(0.0);
    let title_limit = floor_to_usize(title_width / heading_cell_width);
    let heading_y = bounds.y + metrics.heading_y_offset;
    if heading_y + 7.0 * heading_scale <= bounds.y + bounds.height {
        push_hud_text(
            vertices,
            &take_chars(card.title, title_limit),
            content_left,
            heading_y,
            heading_scale,
            [0.96, 0.99, 1.0, 1.0],
        );
        push_hud_text(
            vertices,
            &time,
            content_right - time_width,
            heading_y,
            heading_scale,
            accent,
        );
    }

    let divider_y = bounds.y + metrics.divider_y_offset;
    if divider_y < bounds.y + bounds.height - scale {
        push_quad(
            vertices,
            content_left,
            divider_y,
            content_width,
            scale,
            [0.14, 0.18, 0.24, 0.9],
        );
    }

    let body_scale = metrics.body_scale;
    let body_cell_width = 6.0 * body_scale;
    let body_cell_height = 8.0 * body_scale;
    let body_limit = floor_to_usize(content_width / body_cell_width);
    let status = card.status.filter(|status| !status.is_empty());
    let status_y = bounds.y + metrics.status_y_offset;
    let description_y = if let Some(status) = status {
        if status_y + 7.0 * body_scale <= bounds.y + bounds.height {
            push_hud_text(
                vertices,
                &take_chars(status, body_limit),
                content_left,
                status_y,
                body_scale,
                accent,
            );
        }
        bounds.y + metrics.description_y_offset
    } else {
        bounds.y + metrics.description_without_status_y_offset
    };
    let description_height = (bounds.y + bounds.height - metrics.bottom_inset - description_y).max(0.0);
    let description_lines = floor_to_usize(description_height / body_cell_height);
    for (line_index, line) in wrap_hud_text(card.description, body_limit, description_lines)
        .iter()
        .enumerate()
    {
        push_hud_text(
            vertices,
            line,
            content_left,
            description_y + line_index as f32 * body_cell_height,
            body_scale,
            [0.68, 0.75, 0.84, 1.0],
        );
    }
}

fn take_chars(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

fn wrap_hud_text(text: &str, max_chars: usize, max_lines: usize) -> Vec<String> {
    if max_chars == 0 || max_lines == 0 {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let word: Vec<char> = word.chars().collect();
        if word.len() <= max_chars {
            let required = word.len() + usize::from(!current.is_empty());
            if current.chars().count() + required > max_chars {
                lines.push(std::mem::take(&mut current));
                if lines.len() == max_lines {
                    return lines;
                }
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.extend(word);
            continue;
        }

        if !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            if lines.len() == max_lines {
                return lines;
            }
        }
        for chunk in word.chunks(max_chars) {
            if chunk.len() == max_chars {
                lines.push(chunk.iter().collect());
                if lines.len() == max_lines {
                    return lines;
                }
            } else {
                current.extend(chunk);
            }
        }
    }
    if !current.is_empty() && lines.len() < max_lines {
        lines.push(current);
    }
    lines
}

fn push_hud_text(vertices: &mut Vec<HudVertex>, text: &str, x: f32, y: f32, scale: f32, color: [f32; 4]) {
    for (character_index, character) in text.chars().enumerate() {
        let glyph = glyph_rows(character.to_ascii_uppercase());
        let glyph_x = x + character_index as f32 * 6.0 * scale;
        for (row, bits) in glyph.iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) != 0 {
                    push_quad(
                        vertices,
                        glyph_x + column as f32 * scale,
                        y + row as f32 * scale,
                        scale,
                        scale,
                        color,
                    );
                }
            }
        }
    }
}

fn push_quad(vertices: &mut Vec<HudVertex>, x: f32, y: f32, width: f32, height: f32, color: [f32; 4]) {
    let x2 = x + width;
    let y2 = y + height;
    vertices.extend_from_slice(&[
        HudVertex {
            position: [x, y],
            color,
        },
        HudVertex {
            position: [x2, y],
            color,
        },
        HudVertex {
            position: [x2, y2],
            color,
        },
        HudVertex {
            position: [x, y],
            color,
        },
        HudVertex {
            position: [x2, y2],
            color,
        },
        HudVertex {
            position: [x, y2],
            color,
        },
    ]);
}

fn glyph_rows(character: char) -> [u8; 7] {
    match character {
        ' ' => [0; 7],
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b11100],
        ':' => [0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00110],
        ',' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00100, 0b01000],
        '/' => [0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b00000, 0b00000],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        '+' => [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000],
        '=' => [0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        '_' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111],
        '%' => [0b11001, 0b11010, 0b00100, 0b01000, 0b10110, 0b01011, 0b10011],
        '(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
        '×' => [0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b00000],
        _ => [0b01110, 0b10001, 0b00010, 0b00100, 0b00100, 0b00000, 0b00100],
    }
}

const HUD_SHADER: &str = r"
struct HudUniforms {
    viewport: vec2<f32>,
    _padding: vec2<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: HudUniforms;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let x = input.position.x / uniforms.viewport.x * 2.0 - 1.0;
    let y = 1.0 - input.position.y / uniforms.viewport.y * 2.0;
    output.position = vec4<f32>(x, y, 0.0, 1.0);
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
";

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
    // WGSL rounds the trailing vec4 to offset 208 and the struct to 224.
    _tail_alignment: [u32; 2],
    _padding: [u32; 4],
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
        }
    }
}

/// Luminance-normalizes green-relative WB gains at the uniform boundary
/// (`docs/EDITOR_MATH.md:24-27`, experiment `docs/experiments/d.md` H2).
///
/// Decode backends deliver green-relative gains `[gR, gG, gB, gG2]` (DNG:
/// `AsShotNeutral^-1` normalized to green; CR3: CTMD R/G and B/G ratios).
/// Applied raw, their Rec.709 weighted luminance differs from one, which
/// shifts display exposure by a light-source-dependent amount (measured
/// −0.16…−0.21 stops in experiment D). Dividing every component by that
/// luminance keeps the channel ratios — and thus the white balance — while
/// making the operation exposure-neutral. The fourth (second green)
/// component is scaled by the same factor so both green planes stay equal.
///
/// Defensive: backends already validate their WB evidence, so `None` from
/// the core helper is unexpected; invalid gains are uploaded unchanged with
/// a warning instead of failing the whole mosaic upload.
fn display_ready_white_balance(gains: [f32; 4]) -> [f32; 4] {
    let rgb = [gains[0], gains[1], gains[2]];
    let Some(normalized) = luminance_normalize_wb_gains(rgb) else {
        log::warn!("invalid white-balance gains {gains:?}; uploading them without luminance normalization");
        return gains;
    };
    let scale = normalized[1] / gains[1];
    [normalized[0], normalized[1], normalized[2], gains[3] * scale]
}

#[derive(Debug)]
pub struct RawRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    raw_texture: Option<wgpu::Texture>,
    parameters: GpuParameters,
    resident_bytes: u64,
}

impl RawRenderer {
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
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        Self {
            pipeline,
            bind_group_layout,
            uniform_buffer,
            bind_group: None,
            raw_texture: None,
            parameters,
            resident_bytes: 0,
        }
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
        let total_started = Instant::now();
        let validate_started = Instant::now();
        let metadata = &mosaic.metadata;
        let max_dimension = device.limits().max_texture_dimension_2d;
        if metadata.photometric != Photometric::Cfa || metadata.components_per_pixel != 1 {
            return Err(GpuError::UnsupportedPhotometric);
        }
        let cfa_pattern = metadata.cfa.as_ref().ok_or(GpuError::MissingCfa)?;
        let cfa = cfa_pattern.bayer_quad()?;
        let black = metadata.black_level.bayer_quad()?;
        let white = metadata.white_level.bayer_quad(cfa_pattern)?;
        for index in 0..4 {
            if white[index] <= black[index] {
                return Err(GpuError::InvalidLevels {
                    black: black[index],
                    white: white[index],
                });
            }
        }
        let camera_to_rgb = camera_to_linear_srgb(metadata.xyz_to_camera).unwrap_or_else(|| {
            log::warn!("camera calibration matrix is singular; displaying camera RGB as linear sRGB");
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
        });
        let crop = metadata.effective_crop();
        let validate = validate_started.elapsed();

        let atlas_plan_started = Instant::now();
        let plan = plan_tiling(
            metadata.width,
            metadata.height,
            max_dimension,
            device.limits().max_texture_array_layers,
            tiling,
        )?;
        let tile_halo = plan.tile_halo;
        let tile_size = plan.tile_size;
        let tile_grid = plan.tile_grid;
        let layer_count = plan.layer_count;
        let texture_width = plan.texture_extent;
        let texture_height = plan.texture_extent;
        let atlas_bytes = plan.atlas_bytes;
        self.parameters.raw_size = [metadata.width, metadata.height];
        self.parameters.texture_size = [texture_width, texture_height];
        self.parameters.sample_stride = tile_size;
        self.parameters.tile_halo = tile_halo;
        self.parameters.tile_grid = tile_grid;
        self.parameters.crop_origin = [crop.x, crop.y];
        self.parameters.crop_size = [crop.width, crop.height];
        self.parameters.cfa = cfa;
        self.parameters.black = black;
        self.parameters.white = white;
        self.parameters.white_balance = display_ready_white_balance(metadata.white_balance);
        self.parameters.camera_to_rgb_0 =
            [camera_to_rgb[0][0], camera_to_rgb[0][1], camera_to_rgb[0][2], 0.0];
        self.parameters.camera_to_rgb_1 =
            [camera_to_rgb[1][0], camera_to_rgb[1][1], camera_to_rgb[1][2], 0.0];
        self.parameters.camera_to_rgb_2 =
            [camera_to_rgb[2][0], camera_to_rgb[2][1], camera_to_rgb[2][2], 0.0];
        self.parameters.orientation = metadata.orientation.shader_code();
        let atlas_plan = atlas_plan_started.elapsed();

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
        for tile_y in 0..tile_grid[1] {
            for tile_x in 0..tile_grid[0] {
                let halo_pack_started = Instant::now();
                let tile = tile_with_halo(
                    &mosaic.pixels,
                    metadata.width,
                    metadata.height,
                    tile_x,
                    tile_y,
                    tile_size,
                    tile_halo,
                );
                halo_pack += halo_pack_started.elapsed();
                let row_pack_started = Instant::now();
                let (bytes, row_pitch) = mosaic_bytes(&tile, texture_width);
                row_pack += row_pack_started.elapsed();
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
        let uniform_write_started = Instant::now();
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&self.parameters));
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
        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
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
            ],
        }));
        self.raw_texture = Some(texture);
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

    pub fn update_view(&mut self, queue: &wgpu::Queue, view: ViewParameters) {
        self.parameters.viewport = [view.viewport[0].max(1.0), view.viewport[1].max(1.0)];
        self.parameters.pan = view.pan;
        self.parameters.zoom = view.zoom.clamp(0.02, 128.0);
        self.parameters.exposure_stops = view.exposure_stops.clamp(-10.0, 10.0);
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

#[cfg(target_endian = "little")]
fn mosaic_bytes(pixels: &[u16], width: u32) -> (Cow<'_, [u8]>, u32) {
    if width == 0 {
        return (Cow::Owned(Vec::new()), 0);
    }
    let row_bytes = usize::try_from(width).unwrap_or(usize::MAX).saturating_mul(2);
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
    let row_pitch = (row_bytes.saturating_add(alignment - 1) / alignment).saturating_mul(alignment);
    if row_pitch == row_bytes {
        (Cow::Borrowed(bytemuck::cast_slice(pixels)), row_pitch as u32)
    } else {
        let height = pixels
            .len()
            .checked_div(usize::try_from(width).unwrap_or(1))
            .unwrap_or(0);
        let mut padded = vec![0_u8; row_pitch.saturating_mul(height)];
        let source = bytemuck::cast_slice::<u16, u8>(pixels);
        for row in 0..height {
            let source_range = row * row_bytes..(row + 1) * row_bytes;
            let destination_range = row * row_pitch..row * row_pitch + row_bytes;
            padded[destination_range].copy_from_slice(&source[source_range]);
        }
        (Cow::Owned(padded), row_pitch as u32)
    }
}

fn tile_with_halo(
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

    // Every output row samples one contiguous source-row interval, with only
    // the left and right sensor edges requiring replicated values. Copy that
    // interval in bulk instead of repeating saturating coordinate arithmetic
    // and a two-dimensional source lookup for every sample. This preserves
    // the exact clamp-to-edge policy, including oversized edge tiles.
    let left_fill = halo.saturating_sub(tile_x).min(extent);
    let source_x = tile_x.saturating_sub(halo).min(width.saturating_sub(1));
    let copy_len = width
        .saturating_sub(source_x)
        .min(extent.saturating_sub(left_fill));
    for local_y in 0..extent {
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
    output
}

#[cfg(target_endian = "big")]
fn mosaic_bytes(pixels: &[u16], width: u32) -> (Cow<'_, [u8]>, u32) {
    if width == 0 {
        return (Cow::Owned(Vec::new()), 0);
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
        let row = index / usize::try_from(width).unwrap_or(1);
        let column = index % usize::try_from(width).unwrap_or(1);
        padded[row * row_pitch + column * 2..row * row_pitch + column * 2 + 2]
            .copy_from_slice(&sample.to_le_bytes());
    }
    (Cow::Owned(padded), row_pitch as u32)
}

#[derive(Debug, Error)]
pub enum GpuError {
    #[error("GPU fast path currently supports single-plane CFA RAW only")]
    UnsupportedPhotometric,
    #[error("CFA metadata is missing")]
    MissingCfa,
    #[error("invalid sensor levels: black {black}, white {white}")]
    InvalidLevels { black: f32, white: f32 },
    #[error("RAW dimensions {width}x{height} exceed GPU texture limit {max}")]
    DimensionsTooLarge { width: u32, height: u32, max: u32 },
    #[error("GPU adapter cannot fit a tile in texture limit {max}")]
    TileTooLargeForAdapter { max: u32 },
    #[error("invalid tiling override: tile_size {tile_size}, halo {tile_halo}, texture limit {max}")]
    InvalidTilingOverride { tile_size: u32, tile_halo: u32, max: u32 },
    #[error("RAW tile grid exceeds the GPU texture-array layer limit")]
    TooManyTiles,
    #[error("eager GPU atlas requires {bytes} bytes; limit is {max} bytes")]
    AtlasTooLarge { bytes: u64, max: u64 },
    #[error("unsupported decoded frame: {0}")]
    Frame(#[from] FrameError),
}

#[cfg(test)]
mod tests {
    use super::{
        GpuError, GpuParameters, HUD_SHADER, TilingOverrides, display_ready_white_balance, floor_to_usize,
        glyph_rows, hud_card_layout, hud_card_metrics, hud_card_metrics_for_count, hud_card_scale,
        hud_card_scale_for_count, mosaic_bytes, plan_tiling, tile_with_halo, wrap_hud_text,
    };
    use rrrah_core::WB_LUMINANCE_WEIGHTS;

    fn wb_luminance(gains: [f32; 3]) -> f32 {
        gains[0] * WB_LUMINANCE_WEIGHTS[0]
            + gains[1] * WB_LUMINANCE_WEIGHTS[1]
            + gains[2] * WB_LUMINANCE_WEIGHTS[2]
    }

    /// Contract for the uniform boundary (`docs/EDITOR_MATH.md:24-27`): the
    /// uploaded gains must have weighted luminance exactly one while keeping
    /// the green-relative channel ratios of the source gains.
    fn assert_exposure_neutral_white_balance(raw_gains: [f32; 4]) {
        let uploaded = display_ready_white_balance(raw_gains);
        let luminance = wb_luminance([uploaded[0], uploaded[1], uploaded[2]]);
        assert!((luminance - 1.0).abs() < 1.0e-6, "luminance {luminance} != 1.0");
        // Green-relative ratios are preserved: the white balance itself is
        // unchanged, only the overall scale moves.
        assert!((uploaded[0] / uploaded[1] - raw_gains[0] / raw_gains[1]).abs() < 1.0e-6);
        assert!((uploaded[2] / uploaded[1] - raw_gains[2] / raw_gains[1]).abs() < 1.0e-6);
        // The second green plane is scaled by the same factor as the first.
        assert!((uploaded[3] / uploaded[1] - raw_gains[3] / raw_gains[1]).abs() < 1.0e-6);
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
    fn uploaded_white_balance_is_exposure_neutral_for_tungsten() {
        // Representative green-relative gains around ~2850 K (experiment D).
        assert_exposure_neutral_white_balance([1.9, 1.0, 1.4, 1.0]);
        let uploaded = display_ready_white_balance([1.9, 1.0, 1.4, 1.0]);
        // Unnormalized luminance is 1.22022 (+0.287 stops); normalization
        // must pull every channel down by that factor.
        assert!(uploaded.iter().all(|gain| *gain < 1.9));
        assert!(uploaded[1] < 1.0);
    }

    #[test]
    fn uploaded_white_balance_is_exposure_neutral_for_cr3_fixture() {
        // Canon EOS R8 CTMD fixture gains 1678/1024 and 1659/1024.
        let red = 1_678.0 / 1_024.0;
        let blue = 1_659.0 / 1_024.0;
        let raw_gains = [red, 1.0, blue, 1.0];
        // The fixture must actually exercise the normalization (luminance
        // 1.18055, i.e. +0.240 stops before this fix).
        assert!(wb_luminance([red, 1.0, blue]) > 1.1);
        assert_exposure_neutral_white_balance(raw_gains);
        let uploaded = display_ready_white_balance(raw_gains);
        assert!(uploaded[0] < red && uploaded[2] < blue);
    }

    #[test]
    fn uploaded_white_balance_keeps_neutral_gains_neutral() {
        let uploaded = display_ready_white_balance([1.0; 4]);
        for gain in uploaded {
            assert!((gain - 1.0).abs() < 1.0e-6, "gain {gain} != 1.0");
        }
    }

    #[test]
    fn invalid_white_balance_gains_are_uploaded_unchanged() {
        // Decode backends validate WB evidence before this point; if invalid
        // gains still arrive, upload them verbatim rather than failing the
        // whole mosaic upload (bit-exact comparison avoids float_cmp).
        let nan_gains = [f32::NAN, 1.0, 1.0, 1.0];
        assert_eq!(
            display_ready_white_balance(nan_gains).map(f32::to_bits),
            nan_gains.map(f32::to_bits)
        );
        let zero_red = [0.0, 1.0, 1.4, 1.0];
        assert_eq!(
            display_ready_white_balance(zero_red).map(f32::to_bits),
            zero_red.map(f32::to_bits)
        );
    }

    #[test]
    fn four_cards_fill_the_nominal_telemetry_dashboard() {
        let cards = hud_card_layout([780.0, 620.0], 4);
        assert_eq!(cards.len(), 4);
        assert_close(cards[0].x, 16.0);
        assert_close(cards[0].y, 16.0);
        assert_close(cards[0].width, 748.0);
        assert_close(cards[0].height, 138.0);
        assert_close(cards[1].y, 166.0);
        assert_close(cards[2].y, 316.0);
        assert_close(cards[3].y, 466.0);
        assert_close(cards[3].y + cards[3].height, 604.0);
    }

    #[test]
    fn card_layout_scales_for_a_retina_surface() {
        let cards = hud_card_layout([1560.0, 1240.0], 4);
        assert_close(cards[0].x, 32.0);
        assert_close(cards[0].width, 1496.0);
        assert_close(cards[0].height, 276.0);
        assert_close(cards[1].y, 332.0);
    }

    #[test]
    fn ten_cards_fill_a_two_column_five_row_grid() {
        let cards = hud_card_layout([780.0, 620.0], 10);
        assert_eq!(cards.len(), 10);
        assert_close(cards[0].x, 16.0);
        assert_close(cards[0].y, 16.0);
        assert_close(cards[0].width, 368.0);
        assert_close(cards[0].height, 108.0);
        assert_close(cards[1].x, 396.0);
        assert_close(cards[1].y, 16.0);
        assert_close(cards[2].x, 16.0);
        assert_close(cards[2].y, 136.0);
        assert_close(cards[8].x, 16.0);
        assert_close(cards[8].y, 496.0);
        assert_close(cards[9].x, 396.0);
        assert_close(cards[9].y, 496.0);
        assert_close(cards[9].x + cards[9].width, 764.0);
        assert_close(cards[9].y + cards[9].height, 604.0);
    }

    #[test]
    fn eleven_cards_use_three_columns_for_the_total_block() {
        let cards = hud_card_layout([780.0, 620.0], 11);
        let expected_width = (748.0 - 24.0) / 3.0;
        assert_eq!(cards.len(), 11);
        assert_close(cards[0].x, 16.0);
        assert_close(cards[0].width, expected_width);
        assert_close(cards[1].x, 28.0 + expected_width);
        assert_close(cards[2].x, 40.0 + expected_width * 2.0);
        assert_close(cards[3].x, 16.0);
        assert_close(cards[3].y, 166.0);
        assert_close(cards[9].x, 16.0);
        assert_close(cards[9].y, 466.0);
        assert_close(cards[10].x, 28.0 + expected_width);
        assert_close(cards[10].y + cards[10].height, 604.0);
    }

    #[test]
    fn twenty_four_cards_fill_a_four_column_six_row_dashboard() {
        let cards = hud_card_layout([1600.0, 920.0], 24);
        assert_eq!(cards.len(), 24);
        assert_close(cards[0].x, 16.0);
        assert_close(cards[0].y, 16.0);
        assert_close(cards[0].width, 383.0);
        assert_close(cards[0].height, 138.0);
        assert_close(cards[1].x, 411.0);
        assert_close(cards[2].x, 806.0);
        assert_close(cards[3].x, 1201.0);
        assert_close(cards[4].x, 16.0);
        assert_close(cards[4].y, 166.0);
        assert_close(cards[20].y, 766.0);
        assert_close(cards[23].x + cards[23].width, 1584.0);
        assert_close(cards[23].y + cards[23].height, 904.0);
    }

    #[test]
    fn twenty_eight_cards_fill_a_five_column_six_row_dashboard() {
        let cards = hud_card_layout([1600.0, 920.0], 28);
        assert_eq!(cards.len(), 28);
        assert_close(cards[0].x, 16.0);
        assert_close(cards[0].y, 16.0);
        assert_close(cards[0].width, 304.0);
        assert_close(cards[0].height, 138.0);
        assert_close(cards[1].x, 332.0);
        assert_close(cards[2].x, 648.0);
        assert_close(cards[3].x, 964.0);
        assert_close(cards[4].x, 1280.0);
        assert_close(cards[5].x, 16.0);
        assert_close(cards[5].y, 166.0);
        assert_close(cards[27].x, 648.0);
        assert_close(cards[27].y, 766.0);
        assert_close(cards[4].x + cards[4].width, 1584.0);
        assert_close(cards[27].y + cards[27].height, 904.0);
    }

    #[test]
    fn thirty_eight_cards_fill_a_five_column_eight_row_dashboard() {
        let cards = hud_card_layout([1600.0, 920.0], 38);
        assert_eq!(cards.len(), 38);
        assert_close(cards[0].x, 16.0);
        assert_close(cards[0].y, 16.0);
        assert_close(cards[0].width, 304.0);
        assert_close(cards[0].height, 100.5);
        assert_close(cards[5].y, 128.5);
        assert_close(cards[35].y, 803.5);
        assert_close(cards[37].x, 648.0);
        assert_close(cards[37].y, 803.5);
        assert_close(cards[4].x + cards[4].width, 1584.0);
        assert_close(cards[37].y + cards[37].height, 904.0);

        let scale = hud_card_scale_for_count([1600.0, 920.0], 38);
        let metrics = hud_card_metrics_for_count(cards[0], scale, 38);
        let description_height = cards[0].height - metrics.bottom_inset - metrics.description_y_offset;
        let description_lines = floor_to_usize(description_height / (8.0 * metrics.body_scale));
        assert!(description_lines >= 3);
    }

    #[test]
    fn forty_one_cards_keep_algorithm_descriptions_readable() {
        let viewport = [1600.0, 1080.0];
        let cards = hud_card_layout(viewport, 41);
        assert_eq!(cards.len(), 41);
        assert_close(cards[0].x, 16.0);
        assert_close(cards[40].x, 16.0);
        assert_close(cards[40].y + cards[40].height, 1064.0);
        let scale = hud_card_scale_for_count(viewport, 41);
        let metrics = hud_card_metrics_for_count(cards[0], scale, 41);
        let description_height = cards[0].height - metrics.bottom_inset - metrics.description_y_offset;
        let description_lines = floor_to_usize(description_height / (8.0 * metrics.body_scale));
        assert!(description_lines >= 3);
    }

    #[test]
    fn five_column_dashboard_scales_exactly_for_a_retina_surface() {
        let cards = hud_card_layout([3200.0, 1840.0], 28);
        assert_close(hud_card_scale_for_count([3200.0, 1840.0], 28), 2.0);
        assert_close(cards[0].x, 32.0);
        assert_close(cards[0].y, 32.0);
        assert_close(cards[0].width, 608.0);
        assert_close(cards[0].height, 276.0);
        assert_close(cards[1].x, 664.0);
        assert_close(cards[4].x, 2560.0);
        assert_close(cards[5].y, 332.0);
        assert_close(cards[27].x, 1296.0);
        assert_close(cards[27].y, 1532.0);
        assert_close(cards[4].x + cards[4].width, 3168.0);
        assert_close(cards[27].y + cards[27].height, 1808.0);
    }

    #[test]
    fn odd_dense_card_count_keeps_row_major_order() {
        let cards = hud_card_layout([780.0, 620.0], 9);
        assert_eq!(cards.len(), 9);
        assert_close(cards[0].x, 16.0);
        assert_close(cards[1].x, 396.0);
        assert_close(cards[0].y, cards[1].y);
        assert_close(cards[8].x, 16.0);
        assert_close(cards[8].y, 496.0);
    }

    #[test]
    fn dense_card_grid_scales_for_a_retina_surface() {
        let cards = hud_card_layout([1560.0, 1240.0], 10);
        assert_close(cards[0].x, 32.0);
        assert_close(cards[0].y, 32.0);
        assert_close(cards[0].width, 736.0);
        assert_close(cards[0].height, 216.0);
        assert_close(cards[1].x, 792.0);
        assert_close(cards[8].y, 992.0);
        assert_close(cards[9].x + cards[9].width, 1528.0);
        assert_close(cards[9].y + cards[9].height, 1208.0);
    }

    #[test]
    fn compact_card_metrics_keep_dense_descriptions_readable() {
        let viewport = [780.0, 620.0];
        let bounds = hud_card_layout(viewport, 10)[0];
        let scale = hud_card_scale(viewport);
        let metrics = hud_card_metrics(bounds, scale);
        let content_width = bounds.width - metrics.content_left_inset - metrics.content_right_inset;
        let time_limit = floor_to_usize((content_width * 0.4) / (6.0 * metrics.heading_scale));
        let body_limit = floor_to_usize(content_width / (6.0 * metrics.body_scale));
        let description_height = bounds.height - metrics.bottom_inset - metrics.description_y_offset;
        let description_lines = floor_to_usize(description_height / (8.0 * metrics.body_scale));

        assert_close(metrics.heading_scale, 2.0);
        assert!(time_limit >= 10);
        assert!(body_limit >= 32);
        assert!(description_lines >= 3);
    }

    #[test]
    fn five_column_metrics_preserve_timing_title_and_description_capacity() {
        let viewport = [1600.0, 920.0];
        let bounds = hud_card_layout(viewport, 28)[0];
        let scale = hud_card_scale_for_count(viewport, 28);
        let metrics = hud_card_metrics_for_count(bounds, scale, 28);
        let content_width = bounds.width - metrics.content_left_inset - metrics.content_right_inset;
        let heading_cell_width = 6.0 * metrics.heading_scale;
        let time_limit = floor_to_usize((content_width * 0.4) / heading_cell_width);
        let timing_width = "23.50 MS".chars().count() as f32 * heading_cell_width;
        let title_width = (content_width - timing_width - heading_cell_width).max(0.0);
        let title_limit = floor_to_usize(title_width / heading_cell_width);
        let body_limit = floor_to_usize(content_width / (6.0 * metrics.body_scale));
        let description_height = bounds.height - metrics.bottom_inset - metrics.description_y_offset;
        let description_lines = floor_to_usize(description_height / (8.0 * metrics.body_scale));

        assert_close(scale, 1.0);
        assert!(time_limit >= 10);
        assert!(title_limit >= 17);
        assert!(body_limit >= 31);
        assert!(description_lines >= 6);
    }

    #[test]
    fn card_layout_is_safe_for_empty_and_tiny_viewports() {
        assert!(hud_card_layout([780.0, 620.0], 0).is_empty());
        let cards = hud_card_layout([8.0, 8.0], 4);
        assert_eq!(cards.len(), 4);
        assert!(cards.iter().all(|card| card.width >= 0.0));
        assert!(cards.iter().all(|card| card.height >= 0.0));
        assert!(cards.iter().all(|card| card.x.is_finite() && card.y.is_finite()));

        let dense_cards = hud_card_layout([8.0, 8.0], 10);
        assert_eq!(dense_cards.len(), 10);
        assert!(dense_cards.iter().all(|card| card.width >= 0.0));
        assert!(dense_cards.iter().all(|card| card.height >= 0.0));
        assert!(
            dense_cards
                .iter()
                .all(|card| card.x.is_finite() && card.y.is_finite())
        );

        let maximal_cards = hud_card_layout([8.0, 8.0], 38);
        assert_eq!(maximal_cards.len(), 38);
        assert!(maximal_cards.iter().all(|card| card.width >= 0.0));
        assert!(maximal_cards.iter().all(|card| card.height >= 0.0));
        assert!(
            maximal_cards
                .iter()
                .all(|card| card.x.is_finite() && card.y.is_finite())
        );
    }

    #[test]
    fn card_description_wraps_and_clips_by_line_count() {
        assert_eq!(
            wrap_hud_text("load source bytes and parse metadata", 12, 3),
            ["load source", "bytes and", "parse"]
        );
        assert_eq!(wrap_hud_text("abcdefgh", 3, 2), ["abc", "def"]);
        assert!(wrap_hud_text("description", 0, 4).is_empty());
    }

    #[test]
    fn hud_space_is_blank_instead_of_unknown_glyph() {
        assert_eq!(glyph_rows(' '), [0; 7]);
        assert_eq!(glyph_rows(','), [0, 0, 0, 0, 0b00110, 0b00100, 0b01000]);
        assert_ne!(glyph_rows('?'), [0; 7]);
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= f32::EPSILON,
            "{actual} != {expected}"
        );
    }

    #[test]
    fn viewport_shader_parses_and_validates_with_naga() {
        // This is deliberately backend-independent. wgpu performs the same
        // Naga validation at device creation, but a host-side test gives us a
        // fast CI gate even on machines without Metal/Vulkan/DX12/WebGPU.
        let module = naga::front::wgsl::parse_str(include_str!("../shaders/raw_view.wgsl"))
            .expect("raw_view.wgsl must remain valid WGSL");
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            // Validate against the portable baseline. Optional subgroup,
            // f16, or vendor capabilities must be requested explicitly by a
            // future shader/backend rather than being hidden by this test.
            naga::valid::Capabilities::empty(),
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
    fn hud_shader_parses_and_validates_with_naga() {
        let module = naga::front::wgsl::parse_str(HUD_SHADER).expect("HUD shader must be valid WGSL");
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        );
        validator
            .validate(&module)
            .expect("HUD shader must pass Naga validation");
    }

    #[test]
    fn uniform_layout_remains_wgsl_compatible() {
        assert_eq!(size_of::<GpuParameters>(), 224);
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
    fn default_tiling_grows_to_fit_array_layer_limit() {
        // 8192x8192 at tile 1022 would need 9x9 = 81 layers; with a budget of
        // 8 the default path doubles the extent until the grid fits.
        let plan = plan_tiling(8192, 8192, 16_384, 8, TilingOverrides::default()).unwrap();
        assert_eq!(plan.tile_size, 8190);
        assert_eq!(plan.texture_extent, 8192);
        assert_eq!(plan.tile_grid, [2, 2]);
        assert_eq!(plan.layer_count, 4);
    }

    #[test]
    fn default_tiling_errors_when_layer_limit_cannot_be_met() {
        // The tile cannot grow past the texture dimension, so an 8192x8192
        // mosaic with a 4-layer budget is rejected.
        let error = plan_tiling(8192, 8192, 4096, 4, TilingOverrides::default()).unwrap_err();
        assert!(matches!(error, GpuError::TooManyTiles));
    }

    #[test]
    fn tiling_override_is_applied_verbatim() {
        let overrides = TilingOverrides { tile_size: Some(512), tile_halo: Some(2) };
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
        let overrides = TilingOverrides { tile_size: Some(16), tile_halo: None };
        let error = plan_tiling(64, 64, 16_384, 2048, overrides).unwrap_err();
        assert!(matches!(
            error,
            GpuError::InvalidTilingOverride { tile_size: 16, tile_halo: 1, max: 16_384 }
        ));
    }

    #[test]
    fn tiling_override_exceeding_texture_limit_is_rejected() {
        let overrides = TilingOverrides { tile_size: Some(4096), tile_halo: Some(4) };
        let error = plan_tiling(4096, 4096, 4096, 2048, overrides).unwrap_err();
        assert!(matches!(
            error,
            GpuError::InvalidTilingOverride { tile_size: 4096, tile_halo: 4, max: 4096 }
        ));
    }

    #[test]
    fn tiling_override_respects_array_layer_limit() {
        let overrides = TilingOverrides { tile_size: Some(32), tile_halo: None };
        // 128x64 at tile 32 -> 4x2 = 8 layers, limit 4.
        let error = plan_tiling(128, 64, 16_384, 4, overrides).unwrap_err();
        assert!(matches!(error, GpuError::TooManyTiles));
    }
}
