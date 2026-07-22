//! GPU RAW-development and display backend.
#![allow(
    clippy::missing_errors_doc,
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    clippy::cast_precision_loss
)]

use std::borrow::Cow;

use bytemuck::{Pod, Zeroable};
use rrrah_core::{DecodedMosaic, FrameError, Photometric, camera_to_linear_srgb};
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
    ) -> Result<(), GpuError> {
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
        let tile_halo = 1_u32;
        let tile_size = max_dimension.saturating_sub(2 * tile_halo).min(4096);
        if tile_size < 32 {
            return Err(GpuError::TileTooLargeForAdapter { max: max_dimension });
        }
        let tile_grid = [
            metadata.width.div_ceil(tile_size),
            metadata.height.div_ceil(tile_size),
        ];
        let layer_count = tile_grid[0]
            .checked_mul(tile_grid[1])
            .ok_or(GpuError::TooManyTiles)?;
        if layer_count > device.limits().max_texture_array_layers {
            return Err(GpuError::TooManyTiles);
        }
        let texture_width = tile_size + 2 * tile_halo;
        let texture_height = texture_width;
        let atlas_bytes = u64::from(texture_width)
            .checked_mul(u64::from(texture_height))
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
        self.parameters.white_balance = metadata.white_balance;
        self.parameters.camera_to_rgb_0 =
            [camera_to_rgb[0][0], camera_to_rgb[0][1], camera_to_rgb[0][2], 0.0];
        self.parameters.camera_to_rgb_1 =
            [camera_to_rgb[1][0], camera_to_rgb[1][1], camera_to_rgb[1][2], 0.0];
        self.parameters.camera_to_rgb_2 =
            [camera_to_rgb[2][0], camera_to_rgb[2][1], camera_to_rgb[2][2], 0.0];
        self.parameters.orientation = metadata.orientation.shader_code();

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
        for tile_y in 0..tile_grid[1] {
            for tile_x in 0..tile_grid[0] {
                let tile = tile_with_halo(
                    &mosaic.pixels,
                    metadata.width,
                    metadata.height,
                    tile_x,
                    tile_y,
                    tile_size,
                    tile_halo,
                );
                let (bytes, row_pitch) = mosaic_bytes(&tile, texture_width);
                let layer = tile_y * tile_grid[0] + tile_x;
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
            }
        }
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&self.parameters));

        // The shader indexes one independent sensor tile per array layer. The
        // default view descriptor would infer `D2` even for a layered texture,
        // which then fails bind-group validation against the `D2Array` layout.
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
        Ok(())
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
    #[error("RAW tile grid exceeds the GPU texture-array layer limit")]
    TooManyTiles,
    #[error("eager GPU atlas requires {bytes} bytes; limit is {max} bytes")]
    AtlasTooLarge { bytes: u64, max: u64 },
    #[error("unsupported decoded frame: {0}")]
    Frame(#[from] FrameError),
}

#[cfg(test)]
mod tests {
    use super::{GpuParameters, HUD_SHADER, glyph_rows, mosaic_bytes, tile_with_halo};

    #[test]
    fn hud_space_is_blank_instead_of_unknown_glyph() {
        assert_eq!(glyph_rows(' '), [0; 7]);
        assert_ne!(glyph_rows('?'), [0; 7]);
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
}
