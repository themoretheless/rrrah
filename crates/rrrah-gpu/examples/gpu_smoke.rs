//! Backend smoke/throughput benchmark using an in-memory synthetic Bayer frame.
//!
//! This intentionally does not decode a RAW file. It validates adapter limits,
//! bind-group/pipeline creation, array-tile upload, and render completion on a
//! real backend. Use `WGPU_BACKEND=metal` (or `vulkan`, `dx12`) to pin a backend.
//!
//! Environment knobs (all optional; library defaults are unchanged):
//! - `RRRAH_GPU_TILE_SIZE=<u32>` — interior tile edge for the normal mode.
//! - `RRRAH_GPU_TILE_HALO=<u32>` — halo width for the normal mode.
//! - `RRRAH_GPU_SWEEP=1` — sweep mode: for each RAW size, upload once per
//!   (`tile_size`, `tile_halo`) configuration, `SWEEP_REPS` times, and print one
//!   CSV row per configuration with per-phase p50 timings from
//!   [`GpuUploadTimings`]. Render timing is skipped in sweep mode.
//! - `RRRAH_GPU_SWEEP_REPS=<n>` — override the sweep repetition count.
//! - `RRRAH_GPU_UPLOAD_AB=1` — counterbalanced current-default vs legacy-4096
//!   upload A/B, with raw samples plus p50/p95 summaries.
//! - `RRRAH_GPU_UPLOAD_AB_REPS=<n>` — override A/B repetitions per variant.

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
use rrrah_gpu::{GpuUploadTimings, RawRenderer, TilingOverrides, ViewParameters, plan_tiling};

const RAW_SIZES: &[(u32, u32)] = &[(1024, 1024), (2048, 1536), (4096, 3072)];
const VIEWPORTS: &[(u32, u32)] = &[(1280, 720), (2560, 1440), (3840, 2160)];
const ZOOMS: &[f32] = &[0.5, 1.0, 2.0, 8.0];
const WARMUP: usize = 2;
const REPS: usize = 20;

/// Sweep matrix: RAW sizes cover ~1 MP up to ~50 MP so per-byte and
/// fixed-cost behavior separate across an order of magnitude.
const SWEEP_RAW_SIZES: &[(u32, u32)] = &[(2048, 1536), (4096, 3072), (6240, 4160), (8192, 6144)];
/// Tile-size sweep at the default halo; 4096 is the current heuristic.
const SWEEP_TILE_SIZES: &[u32] = &[256, 512, 1024, 2048, 4096];
/// Halo sweep at a fixed mid-range tile size.
const SWEEP_HALO_TILE_SIZE: u32 = 1024;
const SWEEP_HALOS: &[u32] = &[0, 1, 2, 4];
const SWEEP_REPS: usize = 7;

const ENV_TILE_SIZE: &str = "RRRAH_GPU_TILE_SIZE";
const ENV_TILE_HALO: &str = "RRRAH_GPU_TILE_HALO";
const ENV_SWEEP: &str = "RRRAH_GPU_SWEEP";
const ENV_SWEEP_REPS: &str = "RRRAH_GPU_SWEEP_REPS";
const ENV_UPLOAD_AB: &str = "RRRAH_GPU_UPLOAD_AB";
const ENV_UPLOAD_AB_REPS: &str = "RRRAH_GPU_UPLOAD_AB_REPS";
const UPLOAD_AB_REPS: usize = 16;

fn main() {
    if let Err(error) = pollster::block_on(run()) {
        eprintln!("gpu_smoke: {error}");
        std::process::exit(2);
    }
}

fn env_u32(name: &str) -> Option<u32> {
    let value = std::env::var(name).ok()?;
    if let Ok(parsed) = value.trim().parse::<u32>() {
        Some(parsed)
    } else {
        eprintln!("gpu_smoke: ignoring unparsable {name}={value:?}");
        None
    }
}

fn env_flag(name: &str) -> bool {
    matches!(std::env::var(name).ok().as_deref(), Some("1" | "true" | "yes"))
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

    if env_flag(ENV_UPLOAD_AB) {
        let reps = env_u32(ENV_UPLOAD_AB_REPS).map_or(UPLOAD_AB_REPS, |value| value.max(1) as usize);
        run_upload_ab(&device, &queue, &info, reps)
    } else if env_flag(ENV_SWEEP) {
        let reps = env_u32(ENV_SWEEP_REPS).map_or(SWEEP_REPS, |value| value.max(1) as usize);
        run_sweep(&device, &queue, &info, reps)
    } else {
        let tiling = TilingOverrides {
            tile_size: env_u32(ENV_TILE_SIZE),
            tile_halo: env_u32(ENV_TILE_HALO),
        };
        if tiling.tile_size.is_some() || tiling.tile_halo.is_some() {
            eprintln!(
                "gpu_smoke: tiling override tile_size={:?} tile_halo={:?}",
                tiling.tile_size, tiling.tile_halo
            );
        }
        run_normal(&device, &queue, &info, &limits, tiling)
    }
}

fn run_normal(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    info: &wgpu::AdapterInfo,
    limits: &wgpu::Limits,
    tiling: TilingOverrides,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "adapter_backend,adapter_name,vendor_id,device_id,max_texture_dimension_2d,max_texture_array_layers,raw_width,raw_height,tile_size,tile_halo,viewport_width,viewport_height,zoom,validate_ms,atlas_plan_ms,texture_allocate_ms,halo_pack_ms,row_pack_ms,write_enqueue_ms,uniform_write_ms,bind_ms,upload_total_ms,upload_enqueue_ms,upload_wait_ms,render_p50_ms,render_p95_ms,render_p99_ms,gpu_complete"
    );

    for &(raw_width, raw_height) in RAW_SIZES {
        let mosaic = synthetic_mosaic(raw_width, raw_height)?;
        // Reusing one renderer makes the per-view rows isolate render work;
        // upload_ms still records a full eager atlas upload for this RAW size.
        let mut renderer = RawRenderer::new(device, wgpu::TextureFormat::Rgba8UnormSrgb);
        let upload_started = Instant::now();
        let upload_timings = renderer.upload_mosaic_with_tiling(device, queue, &mosaic, tiling)?;
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
        // Report the exact geometry selected by the production planner.
        let plan = plan_tiling(
            raw_width,
            raw_height,
            limits.max_texture_dimension_2d,
            limits.max_texture_array_layers,
            tiling,
        )?;
        let tile_halo = plan.tile_halo;
        let tile_size = plan.tile_size;

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
                renderer.update_view(queue, view);
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
                    "{:?},{:?},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.2},{:.3},{:.3},{:.3},{:.3},true",
                    info.backend,
                    info.name,
                    info.vendor,
                    info.device,
                    limits.max_texture_dimension_2d,
                    limits.max_texture_array_layers,
                    raw_width,
                    raw_height,
                    tile_size,
                    tile_halo,
                    viewport_width,
                    viewport_height,
                    zoom,
                    ms(upload_timings.validate),
                    ms(upload_timings.atlas_plan),
                    ms(upload_timings.texture_allocate),
                    ms(upload_timings.halo_pack),
                    ms(upload_timings.row_pack),
                    ms(upload_timings.texture_write_enqueue),
                    ms(upload_timings.uniform_write),
                    ms(upload_timings.bind),
                    ms(upload_timings.total),
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

/// Counterbalanced upload comparison between the current planner default and
/// the legacy 4096/halo-1 geometry. Alternating AB/BA order limits warm-up and
/// thermal drift bias; every sample is followed by an empty submission wait so
/// deferred queue work cannot leak into the next variant.
fn run_upload_ab(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    info: &wgpu::AdapterInfo,
    reps: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    const VARIANTS: [(&str, TilingOverrides); 2] = [
        (
            "current_default",
            TilingOverrides {
                tile_size: None,
                tile_halo: None,
            },
        ),
        (
            "legacy_4096",
            TilingOverrides {
                tile_size: Some(4096),
                tile_halo: Some(1),
            },
        ),
    ];
    eprintln!("gpu_smoke: counterbalanced upload A/B, {reps} reps per variant");
    println!(
        "kind,adapter_backend,adapter_name,raw_width,raw_height,variant,round,sequence,tile_size,tile_halo,tile_count,atlas_bytes,upload_total_ms,upload_total_p95_ms,enqueue_wall_ms,completed_wall_ms"
    );
    let limits = device.limits();

    for &(raw_width, raw_height) in SWEEP_RAW_SIZES {
        let mosaic = synthetic_mosaic(raw_width, raw_height)?;
        // Two warm-up rounds, AB then BA: each variant gets two uploads and
        // occupies each sequence position once before measurement.
        for warmup_round in 0..2 {
            let order = if warmup_round % 2 == 0 { [0, 1] } else { [1, 0] };
            for index in order {
                let tiling = VARIANTS[index].1;
                let mut renderer = RawRenderer::new(device, wgpu::TextureFormat::Rgba8UnormSrgb);
                renderer.upload_mosaic_with_tiling(device, queue, &mosaic, tiling)?;
                let submission = queue.submit([]);
                device.poll(wgpu::PollType::Wait {
                    submission_index: Some(submission),
                    timeout: None,
                })?;
            }
        }

        let mut timings = [Vec::with_capacity(reps), Vec::with_capacity(reps)];
        let mut enqueue_walls = [Vec::with_capacity(reps), Vec::with_capacity(reps)];
        let mut completed_walls = [Vec::with_capacity(reps), Vec::with_capacity(reps)];
        for round in 0..reps {
            let order = if round % 2 == 0 { [0, 1] } else { [1, 0] };
            for (sequence, index) in order.into_iter().enumerate() {
                let (variant, tiling) = VARIANTS[index];
                let plan = plan_tiling(
                    raw_width,
                    raw_height,
                    limits.max_texture_dimension_2d,
                    limits.max_texture_array_layers,
                    tiling,
                )?;
                let mut renderer = RawRenderer::new(device, wgpu::TextureFormat::Rgba8UnormSrgb);
                let started = Instant::now();
                let upload = renderer.upload_mosaic_with_tiling(device, queue, &mosaic, tiling)?;
                let enqueue_wall = started.elapsed().as_secs_f64() * 1e3;
                let submission = queue.submit([]);
                device.poll(wgpu::PollType::Wait {
                    submission_index: Some(submission),
                    timeout: None,
                })?;
                let completed_wall = started.elapsed().as_secs_f64() * 1e3;
                println!(
                    "sample,{:?},{:?},{raw_width},{raw_height},{variant},{},{},{},{},{},{},{:.4},,{enqueue_wall:.4},{completed_wall:.4}",
                    info.backend,
                    info.name,
                    round + 1,
                    sequence + 1,
                    plan.tile_size,
                    plan.tile_halo,
                    plan.layer_count,
                    plan.atlas_bytes,
                    ms(upload.total),
                );
                timings[index].push(upload);
                enqueue_walls[index].push(enqueue_wall);
                completed_walls[index].push(completed_wall);
            }
        }

        for (index, &(variant, tiling)) in VARIANTS.iter().enumerate() {
            let plan = plan_tiling(
                raw_width,
                raw_height,
                limits.max_texture_dimension_2d,
                limits.max_texture_array_layers,
                tiling,
            )?;
            let total_p50 = phase_percentile(&timings[index], |sample| sample.total, 0.50);
            let total_p95 = phase_percentile(&timings[index], |sample| sample.total, 0.95);
            enqueue_walls[index].sort_by(f64::total_cmp);
            completed_walls[index].sort_by(f64::total_cmp);
            println!(
                "summary,{:?},{:?},{raw_width},{raw_height},{variant},{reps},0,{},{},{},{},{total_p50:.4},{:.4},{:.4},{:.4}",
                info.backend,
                info.name,
                plan.tile_size,
                plan.tile_halo,
                plan.layer_count,
                plan.atlas_bytes,
                total_p95,
                percentile(&enqueue_walls[index], 0.50),
                percentile(&completed_walls[index], 0.50),
            );
        }
    }
    Ok(())
}

/// One upload-only sweep row: per-phase p50 over `reps` full uploads.
fn run_sweep(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    info: &wgpu::AdapterInfo,
    reps: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("gpu_smoke: sweep mode, {reps} reps per configuration");
    println!(
        "adapter_backend,adapter_name,raw_width,raw_height,frame_bytes,tile_size,tile_halo,tile_count,atlas_bytes,reps,validate_p50_ms,atlas_plan_p50_ms,texture_allocate_p50_ms,halo_pack_p50_ms,row_pack_p50_ms,write_enqueue_p50_ms,uniform_write_p50_ms,bind_p50_ms,total_p50_ms,total_min_ms,wait_p50_ms"
    );
    // Configurations: the library default, then the tile-size sweep at the
    // default halo, then the halo sweep at a fixed tile size.
    let mut configs: Vec<TilingOverrides> = vec![TilingOverrides::default()];
    configs.extend(SWEEP_TILE_SIZES.iter().map(|&tile_size| TilingOverrides {
        tile_size: Some(tile_size),
        tile_halo: None,
    }));
    configs.extend(SWEEP_HALOS.iter().map(|&tile_halo| TilingOverrides {
        tile_size: Some(SWEEP_HALO_TILE_SIZE),
        tile_halo: Some(tile_halo),
    }));

    for &(raw_width, raw_height) in SWEEP_RAW_SIZES {
        let mosaic = synthetic_mosaic(raw_width, raw_height)?;
        let frame_bytes = u64::from(raw_width) * u64::from(raw_height) * 2;
        for &tiling in &configs {
            let mut renderer = RawRenderer::new(device, wgpu::TextureFormat::Rgba8UnormSrgb);
            // Probe once to skip configurations the adapter/atlas cap rejects.
            if let Err(error) = renderer.upload_mosaic_with_tiling(device, queue, &mosaic, tiling) {
                eprintln!(
                    "gpu_smoke: skip {raw_width}x{raw_height} tile_size={:?} halo={:?}: {error}",
                    tiling.tile_size, tiling.tile_halo
                );
                continue;
            }
            let mut runs: Vec<GpuUploadTimings> = Vec::with_capacity(reps);
            let mut waits = Vec::with_capacity(reps);
            for _ in 0..reps {
                let mut renderer = RawRenderer::new(device, wgpu::TextureFormat::Rgba8UnormSrgb);
                let timings = renderer.upload_mosaic_with_tiling(device, queue, &mosaic, tiling)?;
                let wait_started = Instant::now();
                device.poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: None,
                })?;
                waits.push(wait_started.elapsed().as_secs_f64() * 1e3);
                runs.push(timings);
            }
            waits.sort_by(f64::total_cmp);
            // Report the exact geometry selected by the production planner so
            // rows remain valid when its default policy changes.
            let plan = plan_tiling(
                raw_width,
                raw_height,
                device.limits().max_texture_dimension_2d,
                device.limits().max_texture_array_layers,
                tiling,
            )?;
            println!(
                "{:?},{:?},{},{},{},{},{},{},{},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.3}",
                info.backend,
                info.name,
                raw_width,
                raw_height,
                frame_bytes,
                plan.tile_size,
                plan.tile_halo,
                plan.layer_count,
                plan.atlas_bytes,
                reps,
                phase_p50(&runs, |t| t.validate),
                phase_p50(&runs, |t| t.atlas_plan),
                phase_p50(&runs, |t| t.texture_allocate),
                phase_p50(&runs, |t| t.halo_pack),
                phase_p50(&runs, |t| t.row_pack),
                phase_p50(&runs, |t| t.texture_write_enqueue),
                phase_p50(&runs, |t| t.uniform_write),
                phase_p50(&runs, |t| t.bind),
                phase_p50(&runs, |t| t.total),
                runs.iter().map(|t| ms(t.total)).fold(f64::INFINITY, f64::min),
                percentile(&waits, 0.50),
            );
        }
    }
    Ok(())
}

fn phase_p50(runs: &[GpuUploadTimings], phase: impl Fn(&GpuUploadTimings) -> std::time::Duration) -> f64 {
    phase_percentile(runs, phase, 0.50)
}

fn phase_percentile(
    runs: &[GpuUploadTimings],
    phase: impl Fn(&GpuUploadTimings) -> std::time::Duration,
    fraction: f64,
) -> f64 {
    let mut samples: Vec<f64> = runs.iter().map(|run| ms(phase(run))).collect();
    samples.sort_by(f64::total_cmp);
    percentile(&samples, fraction)
}

fn ms(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1e3
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
