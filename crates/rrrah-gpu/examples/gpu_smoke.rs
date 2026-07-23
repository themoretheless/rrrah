//! Backend smoke/throughput benchmark using an in-memory synthetic Bayer frame.
//!
//! This intentionally does not decode a RAW file. It validates adapter limits,
//! bind-group/pipeline creation, array-tile upload, and render completion on a
//! real backend. Use `WGPU_BACKEND=metal` (or `vulkan`, `dx12`) to pin a backend.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]

use std::{sync::Arc, time::Instant};

use rrrah_core::{
    CfaColor, CfaPattern, DecodedMosaic, LevelGrid, Orientation, Photometric, RawMetadata, WhiteLevel,
};
use rrrah_gpu::{RawRenderer, ViewParameters};

const RAW_SIZES: &[(u32, u32)] = &[(1024, 1024), (2048, 1536), (4096, 3072)];
const VIEWPORTS: &[(u32, u32)] = &[(1280, 720), (2560, 1440), (3840, 2160)];
const ZOOMS: &[f32] = &[0.5, 1.0, 2.0, 8.0];
const WARMUP: usize = 2;
const REPS: usize = 20;

fn main() {
    if let Err(error) = pollster::block_on(run()) {
        eprintln!("gpu_smoke: {error}");
        std::process::exit(2);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        })
        .await?;
    let info = adapter.get_info();
    let limits = adapter.limits();
    let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor::default()).await?;
    eprintln!(
        "adapter={:?} backend={:?} vendor={} device={} max_texture_dimension_2d={} max_texture_array_layers={} max_buffer_size={}",
        info.name,
        info.backend,
        info.vendor,
        info.device,
        limits.max_texture_dimension_2d,
        limits.max_texture_array_layers,
        limits.max_buffer_size
    );
    println!(
        "adapter_backend,adapter_name,vendor_id,device_id,max_texture_dimension_2d,max_texture_array_layers,raw_width,raw_height,viewport_width,viewport_height,zoom,upload_enqueue_ms,upload_wait_ms,render_p50_ms,render_p95_ms,render_p99_ms,gpu_complete"
    );

    for &(raw_width, raw_height) in RAW_SIZES {
        let mosaic = synthetic_mosaic(raw_width, raw_height)?;
        // Reusing one renderer makes the per-view rows isolate render work;
        // upload_ms still records a full eager atlas upload for this RAW size.
        let mut renderer = RawRenderer::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
        let upload_started = Instant::now();
        let _upload_timings = renderer.upload_mosaic(&device, &queue, &mosaic)?;
        // Keep queue enqueue/allocation separate from the explicit wait. The
        // latter prevents the first render's timing from absorbing pending
        // write_texture work; it is not a claim about a GPU copy timestamp.
        let upload_enqueue_ms = upload_started.elapsed().as_secs_f64() * 1e3;
        let upload_wait_started = Instant::now();
        device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })?;
        let upload_wait_ms = upload_wait_started.elapsed().as_secs_f64() * 1e3;

        for &(viewport_width, viewport_height) in VIEWPORTS {
            // Match the render target to the logical viewport. A larger target
            // would execute unnecessary fragments outside `parameters.viewport`.
            let target = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("rrrah gpu smoke target"),
                size: wgpu::Extent3d {
                    width: viewport_width,
                    height: viewport_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
            for &zoom in ZOOMS {
                let view = ViewParameters {
                    viewport: [viewport_width as f32, viewport_height as f32],
                    zoom,
                    ..ViewParameters::default()
                };
                renderer.update_view(&queue, view);
                for _ in 0..WARMUP {
                    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("rrrah gpu smoke warmup"),
                    });
                    renderer.encode(&mut encoder, &target_view);
                    let submission = queue.submit(Some(encoder.finish()));
                    device.poll(wgpu::PollType::Wait {
                        submission_index: Some(submission),
                        timeout: None,
                    })?;
                }
                let mut samples = Vec::with_capacity(REPS);
                for _ in 0..REPS {
                    let started = Instant::now();
                    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("rrrah gpu smoke render"),
                    });
                    renderer.encode(&mut encoder, &target_view);
                    let submission = queue.submit(Some(encoder.finish()));
                    device.poll(wgpu::PollType::Wait {
                        submission_index: Some(submission),
                        timeout: None,
                    })?;
                    samples.push(started.elapsed().as_secs_f64() * 1e3);
                }
                samples.sort_by(f64::total_cmp);
                let p50 = percentile(&samples, 0.50);
                let p95 = percentile(&samples, 0.95);
                let p99 = percentile(&samples, 0.99);
                println!(
                    "{:?},{:?},{},{},{},{},{},{},{},{},{},{:.2},{:.3},{:.3},{:.3},{:.3},true",
                    info.backend,
                    info.name,
                    info.vendor,
                    info.device,
                    limits.max_texture_dimension_2d,
                    limits.max_texture_array_layers,
                    raw_width,
                    raw_height,
                    viewport_width,
                    viewport_height,
                    zoom,
                    upload_enqueue_ms,
                    upload_wait_ms,
                    p50,
                    p95,
                    p99,
                );
            }
        }
    }
    Ok(())
}

fn percentile(samples: &[f64], fraction: f64) -> f64 {
    if samples.is_empty() {
        return f64::NAN;
    }
    let index = ((samples.len() - 1) as f64 * fraction).round() as usize;
    samples[index.min(samples.len() - 1)]
}

fn synthetic_mosaic(width: u32, height: u32) -> Result<DecodedMosaic, Box<dyn std::error::Error>> {
    let pixels = Arc::new(
        (0..width.checked_mul(height).ok_or("synthetic dimensions overflow")?)
            .map(|index| ((index.wrapping_mul(13)) % 16_384) as u16)
            .collect::<Vec<_>>(),
    );
    let metadata = RawMetadata {
        make: "synthetic".into(),
        model: "gpu-smoke".into(),
        width,
        height,
        components_per_pixel: 1,
        bits_per_sample: 14,
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
        white_level: WhiteLevel(vec![16_383.0]),
        white_balance: [1.0; 4],
        xyz_to_camera: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0]],
        active_area: None,
        crop_area: None,
        orientation: Orientation::Normal,
    };
    Ok(DecodedMosaic::new(metadata, pixels)?)
}
