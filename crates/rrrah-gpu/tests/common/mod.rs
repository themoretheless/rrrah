//! Headless GPU readback harness for pixel-level pipeline verification.
//!
//! Shared by the integration tests in this directory. The harness renders one
//! decoded mosaic into an offscreen `Rgba8UnormSrgb` texture — the same sRGB
//! target semantics the application picks for its surface — and reads the
//! pixels back to CPU through `copy_texture_to_buffer` + `map_async`.
//!
//! Everything degrades to "skip" when no GPU adapter is available:
//! [`GpuReadback::new`] returns `None` and every test returns early, the same
//! way the CR3 regression skips without `RRRAH_CR3_REGRESSION_DIR`.
//!
//! Future checks (white balance, Bradford adaptation, gamut) plug in by
//! building an input frame, calling [`GpuReadback::render`], and comparing
//! against an expectation with a tolerance — one call per case.

// Each integration-test binary compiles this module independently and may not
// use every helper.
#![allow(dead_code)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::{sync::Arc, sync::mpsc};

use rrrah_core::{
    CfaColor, CfaPattern, DecodedMosaic, LevelGrid, Orientation, Photometric, RawMetadata, WhiteLevel,
};
use rrrah_gpu::{RawRenderer, ViewParameters};

/// Output texture format. Matches the application surface semantics: the
/// fragment shader emits linear values and the sRGB transfer function is
/// applied by the hardware on write into the sRGB target.
pub const READBACK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// One rendered frame read back to CPU memory, tightly packed RGBA8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl RgbaFrame {
    /// RGBA byte quad at (`x`, `y`), origin top-left.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let offset = (y as usize * self.width as usize + x as usize) * 4;
        self.pixels[offset..offset + 4].try_into().expect("in-bounds pixel")
    }

    /// Center pixel; the geometric focus of every fill-view render.
    pub fn center(&self) -> [u8; 4] {
        self.pixel(self.width / 2, self.height / 2)
    }

    /// Largest per-channel absolute difference from `reference` over the
    /// whole frame (alpha included).
    pub fn max_channel_deviation(&self, reference: [u8; 4]) -> u8 {
        self.pixels
            .chunks_exact(4)
            .flat_map(|pixel| pixel.iter().copied().zip(reference).map(|(a, e)| a.abs_diff(e)))
            .max()
            .unwrap_or(0)
    }
}

/// Headless wgpu device/queue pair plus the adapter description for logs.
#[derive(Debug)]
pub struct GpuReadback {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_name: String,
}

impl GpuReadback {
    /// Requests a headless adapter and device. Returns `None` — the signal
    /// for tests to skip — when no adapter or device is available.
    pub fn new() -> Option<Self> {
        pollster::block_on(Self::request())
    }

    async fn request() -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .ok()?;
        let adapter_name = format!("{:?} {}", adapter.get_info().backend, adapter.get_info().name);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .ok()?;
        Some(Self {
            device,
            queue,
            adapter_name,
        })
    }

    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    /// Renders `mosaic` into a `size` offscreen target with the image scaled
    /// to cover the whole target, then reads the frame back.
    ///
    /// The zoom is derived from the shader's fit math (`available =
    /// viewport - 32`, `scale = fit * zoom`) so every target pixel lands
    /// inside the image: no background-clear pixels pollute the readback.
    pub fn render(&self, mosaic: &DecodedMosaic, size: [u32; 2]) -> RgbaFrame {
        let (crop_width, crop_height) = mosaic.metadata.display_dimensions();
        let fit = (size[0].saturating_sub(32).max(1) as f32 / crop_width as f32)
            .min(size[1].saturating_sub(32).max(1) as f32 / crop_height as f32);
        let cover = (size[0] as f32 / crop_width as f32).max(size[1] as f32 / crop_height as f32);
        let view = ViewParameters {
            viewport: [size[0] as f32, size[1] as f32],
            zoom: cover / fit,
            ..ViewParameters::default()
        };
        self.render_with_view(mosaic, view, size)
    }

    /// Renders `mosaic` into a `size` offscreen target with an explicit view
    /// (custom pan/zoom/exposure) and reads the frame back.
    pub fn render_with_view(
        &self,
        mosaic: &DecodedMosaic,
        view: ViewParameters,
        size: [u32; 2],
    ) -> RgbaFrame {
        let mut renderer = RawRenderer::new(&self.device, READBACK_FORMAT);
        renderer
            .upload_mosaic(&self.device, &self.queue, mosaic)
            .expect("synthetic mosaic must upload");
        renderer.update_view(&self.queue, view);

        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rrrah readback target"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: READBACK_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

        let row_pitch = (size[0] * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rrrah readback buffer"),
            size: u64::from(row_pitch) * u64::from(size[1]),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rrrah readback encoder"),
            });
        renderer.encode(&mut encoder, &target_view);
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_pitch),
                    rows_per_image: Some(size[1]),
                },
            },
            wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
        );
        let submission = self.queue.submit(Some(encoder.finish()));

        let slice = buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            sender
                .send(result)
                .expect("readback callback receiver must outlive the map");
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: None,
            })
            .expect("readback device poll must succeed");
        receiver
            .recv()
            .expect("map callback must fire")
            .expect("readback buffer must map");

        let mut pixels = vec![0_u8; (size[0] * size[1] * 4) as usize];
        {
            let mapped = slice.get_mapped_range().expect("mapped readback range");
            for row in 0..size[1] as usize {
                let source = row * row_pitch as usize..row * row_pitch as usize + size[0] as usize * 4;
                let destination = row * size[0] as usize * 4..(row + 1) * size[0] as usize * 4;
                pixels[destination].copy_from_slice(&mapped[source]);
            }
        }
        buffer.unmap();
        RgbaFrame {
            width: size[0],
            height: size[1],
            pixels,
        }
    }
}

/// Builds a uniform-gray Bayer mosaic: every CFA sample equals `level`.
/// Neutral by construction, with an identity camera profile and unit WB, so
/// the expected output reduces to `sRGB(ACES(level/white))` per channel.
pub fn uniform_mosaic(width: u32, height: u32, level: u16, white_level: f32) -> DecodedMosaic {
    pattern_mosaic(width, height, white_level, |_x, _y| level)
}

/// Builds a Bayer mosaic from a per-sample generator; shared metadata with
/// [`uniform_mosaic`].
pub fn pattern_mosaic(
    width: u32,
    height: u32,
    white_level: f32,
    sample: impl Fn(u32, u32) -> u16,
) -> DecodedMosaic {
    let sample = &sample;
    let pixels = Arc::new(
        (0..height)
            .flat_map(|y| (0..width).map(move |x| sample(x, y)))
            .collect::<Vec<u16>>(),
    );
    let metadata = RawMetadata {
        make: "synthetic".into(),
        model: "readback".into(),
        width,
        height,
        components_per_pixel: 1,
        bits_per_sample: 16,
        photometric: Photometric::Cfa,
        cfa: Some(CfaPattern {
            width: 2,
            height: 2,
            cells: vec![CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue],
        }),
        black_level: LevelGrid {
            width: 1,
            height: 1,
            components: 1,
            values: vec![0.0],
        },
        white_level: WhiteLevel(vec![white_level]),
        white_balance: [1.0; 4],
        xyz_to_camera: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]],
        active_area: None,
        crop_area: None,
        orientation: Orientation::Normal,
    };
    DecodedMosaic::new(metadata, pixels).expect("synthetic mosaic must be valid")
}

/// CPU reference for one normalized linear value through the same curve the
/// WGSL fragment shader plus the sRGB target apply: `aces_fitted` (identical
/// coefficients in `rrrah-core`) followed by the IEC 61966-2-1 transfer
/// function, quantized to an 8-bit byte.
pub fn cpu_reference_byte(normalized_linear: f64) -> u8 {
    let mapped = f64::from(rrrah_core::aces_fitted(normalized_linear as f32));
    let encoded = if mapped <= 0.003_130_8 {
        12.92 * mapped
    } else {
        1.055 * mapped.powf(1.0 / 2.4) - 0.055
    };
    (encoded.clamp(0.0, 1.0) * 255.0).round() as u8
}
