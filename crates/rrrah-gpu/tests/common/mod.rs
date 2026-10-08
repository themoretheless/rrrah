//! Headless GPU readback harness for pixel-level pipeline verification.
//!
//! Shared by the integration tests in this directory. The harness renders one
//! decoded mosaic into an offscreen `Rgba8UnormSrgb` texture — the same sRGB
//! target semantics the application picks for its surface — and reads the
//! pixels back to CPU through `copy_texture_to_buffer` + `map_async`.
//!
//! [`GpuReadback::new`] returns `None` when no suitable adapter is available.
//! Hardware qualification uses [`qualification_gpu`], which requires a device
//! unless `RRRAH_GPU_OPTIONAL=1` explicitly opts out. Such a skip is not proof
//! of GPU correctness. Backend/vendor selection applies to adapter admission.
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
        self.pixels[offset..offset + 4]
            .try_into()
            .expect("in-bounds pixel")
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
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    adapter_name: String,
}

/// The same explicit backend mask as the viewer; invalid/unavailable requests
/// fail before hardware qualification and cannot turn into an optional skip.
pub fn headless_instance() -> wgpu::Instance {
    let backend = std::env::var("RRRAH_GPU_BACKEND")
        .unwrap_or_else(|error| match error {
            std::env::VarError::NotPresent => "auto".into(),
            error => panic!("invalid RRRAH_GPU_BACKEND: {error}"),
        })
        .parse::<rrrah_gpu::GpuBackend>()
        .expect("valid RRRAH_GPU_BACKEND");
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    backend
        .configure(&mut descriptor)
        .expect("requested backend enabled in build/platform");
    wgpu::Instance::new(descriptor)
}

pub async fn request_adapter(
    instance: &wgpu::Instance,
    options: &wgpu::RequestAdapterOptions<'_, '_>,
) -> Result<wgpu::Adapter, rrrah_gpu::BackendError> {
    let vendor = std::env::var("RRRAH_GPU_VENDOR")
        .unwrap_or_else(|error| match error {
            std::env::VarError::NotPresent => "any".into(),
            error => panic!("invalid RRRAH_GPU_VENDOR: {error}"),
        })
        .parse::<rrrah_gpu::GpuVendor>()
        .expect("valid RRRAH_GPU_VENDOR");
    let result = vendor.request_adapter(instance, options).await;
    if vendor != rrrah_gpu::GpuVendor::Any && result.is_err() {
        panic!(
            "explicit GPU vendor qualification failed: {}",
            result.as_ref().err().unwrap()
        );
    }
    result
}

impl GpuReadback {
    /// Requests a headless adapter and device. Returns `None` — the signal
    /// for tests to skip — when no adapter or device is available.
    pub fn new() -> Option<Self> {
        pollster::block_on(Self::request())
    }

    async fn request() -> Option<Self> {
        let instance = headless_instance();
        let adapter = request_adapter(
            &instance,
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            },
        )
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

    pub fn expose_interleaved(
        &self,
        samples: &[f32],
        stops: f32,
        gpu: &rrrah_core::MemoryBudget,
        cpu: &rrrah_core::MemoryBudget,
    ) -> Result<rrrah_core::PixelBuffer<f32>, rrrah_gpu::ExposureError> {
        rrrah_gpu::LinearExposureCompute::new(&self.device).execute_interleaved_with_cancel(
            &self.queue,
            samples,
            stops,
            gpu,
            cpu,
            || false,
        )
    }

    pub fn render_existing_raw(&self, renderer: &RawRenderer, size: [u32; 2]) -> RgbaFrame {
        self.read_frame(size, |encoder, target| renderer.encode(encoder, target))
    }

    pub fn render_with_tiling(
        &self,
        mosaic: &DecodedMosaic,
        size: [u32; 2],
        tiling: rrrah_gpu::TilingOverrides,
    ) -> RgbaFrame {
        let (w, h) = mosaic.metadata.display_dimensions();
        let fit = (size[0].saturating_sub(32).max(1) as f32 / w as f32)
            .min(size[1].saturating_sub(32).max(1) as f32 / h as f32);
        let cover = (size[0] as f32 / w as f32).max(size[1] as f32 / h as f32);
        let mut renderer = RawRenderer::new(&self.device, READBACK_FORMAT);
        renderer
            .upload_mosaic_with_tiling(&self.device, &self.queue, mosaic, tiling)
            .unwrap();
        renderer.update_view(
            &self.queue,
            ViewParameters {
                viewport: [size[0] as f32, size[1] as f32],
                zoom: cover / fit,
                ..ViewParameters::default()
            },
        );
        self.read_frame(size, |encoder, target| renderer.encode(encoder, target))
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
        self.read_frame(size, |encoder, target| renderer.encode(encoder, target))
    }

    pub fn render_raster(
        &self,
        raster: &rrrah_core::DecodedRaster,
        view: ViewParameters,
        size: [u32; 2],
    ) -> RgbaFrame {
        let mut renderer = rrrah_gpu::RasterRenderer::new(&self.device, READBACK_FORMAT);
        renderer
            .upload(&self.device, &self.queue, raster)
            .expect("prepared raster must upload");
        renderer.update_view(&self.queue, view);
        self.read_frame(size, |encoder, target| renderer.encode(encoder, target))
    }

    pub fn render_raster_with_aspect(
        &self,
        raster: &rrrah_core::DecodedRaster,
        view: ViewParameters,
        size: [u32; 2],
        aspect: f32,
    ) -> RgbaFrame {
        let mut renderer = rrrah_gpu::RasterRenderer::new(&self.device, READBACK_FORMAT);
        renderer.upload(&self.device, &self.queue, raster).unwrap();
        renderer.set_pixel_aspect(&self.queue, aspect).unwrap();
        renderer.update_view(&self.queue, view);
        self.read_frame(size, |encoder, target| renderer.encode(encoder, target))
    }

    pub fn render_raster_aspect_transition(
        &self,
        raster: &rrrah_core::DecodedRaster,
        view: ViewParameters,
        size: [u32; 2],
        aspect: f32,
        invalid: &[f32],
    ) -> (RgbaFrame, RgbaFrame) {
        let mut renderer = rrrah_gpu::RasterRenderer::new(&self.device, READBACK_FORMAT);
        renderer.upload(&self.device, &self.queue, raster).unwrap();
        renderer.set_pixel_aspect(&self.queue, aspect).unwrap();
        renderer.update_view(&self.queue, view);
        let before = self.read_frame(size, |encoder, target| renderer.encode(encoder, target));
        for &bad in invalid {
            assert!(renderer.set_pixel_aspect(&self.queue, bad).is_err());
            let frame = self.read_frame(size, |encoder, target| renderer.encode(encoder, target));
            assert_eq!(
                frame.pixels, before.pixels,
                "invalid aspect changed display: {bad}"
            );
        }
        renderer.upload(&self.device, &self.queue, raster).unwrap();
        let after = self.read_frame(size, |encoder, target| renderer.encode(encoder, target));
        (before, after)
    }
    pub fn render_raster_frame_transition(
        &self,
        first: &rrrah_core::DecodedRaster,
        second: &rrrah_core::DecodedRaster,
        view: ViewParameters,
        size: [u32; 2],
    ) -> (RgbaFrame, RgbaFrame) {
        let mut renderer = rrrah_gpu::RasterRenderer::new(&self.device, READBACK_FORMAT);
        renderer.upload(&self.device, &self.queue, first).unwrap();
        renderer.update_view(&self.queue, view);
        let before = self.read_frame(size, |encoder, target| renderer.encode(encoder, target));
        renderer.upload(&self.device, &self.queue, second).unwrap();
        renderer.update_view(&self.queue, view);
        let after = self.read_frame(size, |encoder, target| renderer.encode(encoder, target));
        (before, after)
    }

    pub fn verify_cancelled_raster_upload_keeps_previous_frame(
        &self,
        first: &rrrah_core::DecodedRaster,
        replacement: &rrrah_core::DecodedRaster,
        view: ViewParameters,
        size: [u32; 2],
    ) {
        let first_bytes = u64::from(first.width()) * u64::from(first.height()) * 16;
        let replacement_bytes = u64::from(replacement.width()) * u64::from(replacement.height()) * 16;
        let gpu = rrrah_core::MemoryBudget::new(first_bytes + replacement_bytes);
        let uploads = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut renderer =
            rrrah_gpu::RasterRenderer::new_with_budget(&self.device, READBACK_FORMAT, gpu.clone())
                .with_upload_queue_budget(uploads.clone());
        renderer.upload(&self.device, &self.queue, first).unwrap();
        renderer.update_view(&self.queue, view);
        let before = self.read_frame(size, |encoder, target| renderer.encode(encoder, target));
        assert_eq!((gpu.used(), uploads.used()), (first_bytes, 0));
        // Initial check, three validation blocks, before admission and after admission.
        for checkpoint in 1..=6 {
            let mut polls = 0;
            assert!(matches!(
                renderer.upload_with_cancel(&self.device, &self.queue, replacement, || {
                    polls += 1;
                    polls == checkpoint
                }),
                Err(rrrah_gpu::RasterUploadError::Cancelled)
            ));
            assert_eq!((gpu.used(), uploads.used()), (first_bytes, 0));
            let after = self.read_frame(size, |encoder, target| renderer.encode(encoder, target));
            assert_eq!(before.pixels, after.pixels);
        }
        renderer
            .upload_with_cancel(&self.device, &self.queue, replacement, || false)
            .unwrap();
        renderer.update_view(&self.queue, view);
        let after = self.read_frame(size, |encoder, target| renderer.encode(encoder, target));
        assert_ne!(before.pixels, after.pixels);
        assert_eq!(after.center(), [0, 0, 255, 255]);
        assert_eq!((gpu.used(), uploads.used()), (replacement_bytes, 0));
        drop(renderer);
        assert_eq!((gpu.used(), uploads.used()), (0, 0));
    }

    pub fn render_resident_exposure(
        &self,
        pixels: &[[f32; 4]],
        dimensions: [u32; 2],
        stops: f32,
        view: ViewParameters,
        size: [u32; 2],
    ) -> RgbaFrame {
        self.render_resident_exposure_with_aspect(pixels, dimensions, stops, view, size, None)
    }

    pub fn render_resident_exposure_with_aspect(
        &self,
        pixels: &[[f32; 4]],
        dimensions: [u32; 2],
        stops: f32,
        view: ViewParameters,
        size: [u32; 2],
        aspect: Option<f32>,
    ) -> RgbaFrame {
        let bytes = pixels.len() as u64 * 16;
        let compute_root = rrrah_core::MemoryBudget::new(bytes * 2 + 16);
        let texture_root = rrrah_core::MemoryBudget::new(bytes + 16);
        let queue_root = rrrah_core::MemoryBudget::new(bytes.max(256));
        let compute = rrrah_gpu::LinearExposureCompute::new(&self.device);
        let result = compute
            .execute_resident(&self.queue, pixels, stops, &compute_root, || false)
            .unwrap();
        let oracle = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("test resident buffer values"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(result.buffer(), 0, &oracle, 0, bytes);
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        oracle
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        self.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let mapped = oracle.slice(..).get_mapped_range().unwrap();
        let values: &[[f32; 4]] = bytemuck::cast_slice(&mapped);
        for (actual, p) in values.iter().zip(pixels) {
            assert_eq!(
                *actual,
                [
                    p[0] * stops.exp2(),
                    p[1] * stops.exp2(),
                    p[2] * stops.exp2(),
                    p[3]
                ]
            );
        }
        drop(mapped);
        oracle.unmap();
        let mut renderer =
            rrrah_gpu::RasterRenderer::new_with_budget(&self.device, READBACK_FORMAT, texture_root.clone())
                .with_upload_queue_budget(queue_root.clone());
        let first = rrrah_core::DecodedRaster::new(
            1,
            1,
            rrrah_core::RasterPixels::Rgba32Float(std::sync::Arc::new(vec![1., 0., 0., 1.]).into()),
            rrrah_core::RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        renderer.upload(&self.device, &self.queue, &first).unwrap();
        renderer.update_view(&self.queue, view);
        let before = self.read_frame(size, |e, t| renderer.encode(e, t));
        for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::MAX] {
            assert!(matches!(
                renderer.upload_resident_with_aspect(
                    &self.device,
                    &self.queue,
                    &result,
                    dimensions,
                    Some(invalid),
                    || false
                ),
                Err(rrrah_gpu::RasterUploadError::InvalidPixelAspect)
            ));
            assert_eq!((texture_root.used(), queue_root.used()), (16, 0));
            let after = self.read_frame(size, |e, t| renderer.encode(e, t));
            assert_eq!(before.pixels, after.pixels);
        }
        let row_polls = if (dimensions[0] * 16).is_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) {
            1
        } else {
            dimensions[1].div_ceil(256)
        };
        for checkpoint in 1..=3 + row_polls {
            let mut polls = 0;
            assert!(matches!(
                renderer.upload_resident_with_aspect(
                    &self.device,
                    &self.queue,
                    &result,
                    dimensions,
                    aspect,
                    || {
                        polls += 1;
                        polls == checkpoint
                    }
                ),
                Err(rrrah_gpu::RasterUploadError::Cancelled)
            ));
            assert_eq!((texture_root.used(), queue_root.used()), (16, 0));
            let after = self.read_frame(size, |e, t| renderer.encode(e, t));
            assert_eq!(before.pixels, after.pixels);
        }
        renderer
            .upload_resident_with_aspect(&self.device, &self.queue, &result, dimensions, aspect, || false)
            .unwrap();
        drop(result);
        renderer.update_view(&self.queue, view);
        let frame = self.read_frame(size, |e, t| renderer.encode(e, t));
        assert_eq!(
            (compute_root.used(), texture_root.used(), queue_root.used()),
            (0, bytes, 0)
        );
        drop(renderer);
        assert_eq!(texture_root.used(), 0);
        frame
    }

    pub fn render_developed_raw(
        &self,
        raster: &rrrah_core::DecodedRaster,
        view: ViewParameters,
        size: [u32; 2],
        curve: &rrrah_core::develop::MonotoneCurve,
    ) -> RgbaFrame {
        let mut renderer = rrrah_gpu::RasterRenderer::new(&self.device, READBACK_FORMAT);
        renderer.upload(&self.device, &self.queue, raster).unwrap();
        renderer.set_raw_development(&self.queue, curve);
        renderer.update_view(&self.queue, view);
        self.read_frame(size, |encoder, target| renderer.encode(encoder, target))
    }

    pub fn render_model<T: Copy + Into<f64>>(
        &self,
        triangles: &[[[T; 3]; 3]],
        size: [u32; 2],
        yaw: f32,
        pitch: f32,
    ) -> RgbaFrame {
        let mut renderer = rrrah_gpu::ModelRenderer::new(&self.device, READBACK_FORMAT);
        renderer
            .upload(&self.device, triangles.iter().copied())
            .expect("model uploads");
        renderer.resize(&self.device, size).expect("depth fits");
        renderer.update_view(&self.queue, size[0] as f32 / size[1] as f32, yaw, pitch, 1.0);
        self.read_frame(size, |encoder, target| renderer.encode(encoder, target))
    }

    fn read_frame(
        &self,
        size: [u32; 2],
        encode: impl FnOnce(&mut wgpu::CommandEncoder, &wgpu::TextureView),
    ) -> RgbaFrame {
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

        let row_pitch =
            (size[0] * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
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
        encode(&mut encoder, &target_view);
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
    profiled_pattern_mosaic(
        width,
        height,
        white_level,
        [1.0; 4],
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]],
        sample,
    )
}

/// Builds a Bayer mosaic with explicit camera-space WB gains and profile.
/// This is the integration-test boundary for color-policy cases that cannot
/// be represented by the identity metadata in [`pattern_mosaic`].
pub fn profiled_pattern_mosaic(
    width: u32,
    height: u32,
    white_level: f32,
    white_balance: [f32; 4],
    xyz_to_camera: [[f32; 3]; 4],
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
        white_balance,
        xyz_to_camera,
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
///
/// Achromatic inputs pass through the hue-preserving [`cpu_reference_rgb`]
/// unchanged: `r == g == b` reduces to the scalar curve per channel.
pub fn cpu_reference_byte(normalized_linear: f64) -> u8 {
    srgb_byte(f64::from(rrrah_core::aces_fitted(normalized_linear as f32)))
}

/// CPU reference for a linear RGB triplet through the hue-preserving ACES
/// tone map (`rrrah_core::aces_tone_map_rgb`, mirroring the WGSL shader) and
/// the sRGB transfer function, quantized per channel to 8-bit bytes.
pub fn cpu_reference_rgb(linear_rgb: [f64; 3]) -> [u8; 3] {
    let mapped =
        rrrah_core::aces_tone_map_rgb([linear_rgb[0] as f32, linear_rgb[1] as f32, linear_rgb[2] as f32]);
    mapped.map(|channel| srgb_byte(f64::from(channel)))
}

/// IEC 61966-2-1 opto-electronic transfer function and 8-bit quantization,
/// matching the hardware sRGB conversion on write into the readback target.
fn srgb_byte(linear: f64) -> u8 {
    let encoded = if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (encoded.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The dedicated color gate requires a device. CI may explicitly opt out on
/// runners without graphics; that skip never counts as color qualification.
pub fn qualification_gpu() -> Option<GpuReadback> {
    let gpu = GpuReadback::new();
    if gpu.is_none() {
        if std::env::var_os("RRRAH_GPU_OPTIONAL").is_some_and(|value| value == "1") {
            eprintln!("SKIP: no GPU; RRRAH_GPU_OPTIONAL=1 explicitly disables hardware qualification");
        } else {
            panic!("RAW qualification requires a GPU adapter");
        }
    }
    gpu
}
