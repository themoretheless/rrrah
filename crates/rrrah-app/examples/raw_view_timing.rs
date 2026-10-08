//! RAW decode -> completed offscreen GPU frame, and resident zoom/pan timing.
//! This is not a swapchain/physical-display measurement. --swap-view measures
//! complete session-swap restore, upload and GPU completion instead of decode.
//! Optional RRRAH_VIEW_CPU_MB budgets decode and transient RAW upload packing;
//! RRRAH_VIEW_GPU_MB budgets renderer-owned atlas textures. These apply to view
//! timing only. --cache-only uses RRRAH_CACHE_CPU_MB (default 512 MiB) for
//! managed decode/restore allocations. Invalid/overflowing values fail explicitly.
use rrrah_decode::RawDecoder;
use rrrah_gpu::{RawRenderer, ViewParameters};
use std::{error::Error, hint::black_box, path::PathBuf, time::Instant};
fn env_budget(name: &str) -> Result<Option<rrrah_core::MemoryBudget>, Box<dyn Error>> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(rrrah_core::MemoryBudget::new(
            value
                .parse::<u64>()?
                .checked_mul(1024 * 1024)
                .ok_or("memory budget overflow")?,
        ))),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error.into()),
    }
}
fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.
}
fn percentile(values: &[f64], fraction: f64) -> f64 {
    assert!(!values.is_empty() && fraction > 0. && fraction <= 1.);
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    // Nearest rank: with 16 samples p95 selects rank 16, not rank 15.
    sorted[(sorted.len() as f64 * fraction).ceil() as usize - 1]
}
fn frame(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &RawRenderer,
    target: &wgpu::TextureView,
) -> Result<(), Box<dyn Error>> {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    renderer.encode(&mut encoder, target);
    let submission = queue.submit([encoder.finish()]);
    device.poll(wgpu::PollType::Wait {
        submission_index: Some(submission),
        timeout: None,
    })?;
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    let mut args: Vec<_> = std::env::args_os().skip(1).collect();
    let raster_view = args.first().is_some_and(|arg| arg == "--raster-view");
    if raster_view {
        args.remove(0);
    }
    let compute_stops = if args.first().is_some_and(|arg| arg == "--compute-exposure") {
        if !raster_view || args.len() < 3 {
            return Err("use --raster-view --compute-exposure STOPS SOURCE".into());
        }
        args.remove(0);
        let stops = args
            .remove(0)
            .to_str()
            .ok_or("non-UTF8 exposure")?
            .parse::<f32>()?;
        if !stops.is_finite() || !(-32.0..=32.0).contains(&stops) {
            return Err("compute exposure must be finite and within -32..32".into());
        }
        Some(stops)
    } else {
        None
    };
    let cuda = if args.first().is_some_and(|arg| arg == "--cuda-exposure") {
        if !raster_view || args.len() < 3 {
            return Err("use --raster-view --cuda-exposure STOPS SOURCE".into());
        }
        args.remove(0);
        let stops = args
            .remove(0)
            .to_str()
            .ok_or("non-UTF8 exposure")?
            .parse::<f32>()?;
        if !stops.is_finite() || !(-32.0..=32.0).contains(&stops) {
            return Err("CUDA exposure must be finite and within -32..32".into());
        }
        let budget = env_budget("RRRAH_CUDA_GPU_MB")?
            .unwrap_or_else(|| rrrah_core::MemoryBudget::new(512 * 1024 * 1024));
        Some((rrrah_cuda::CudaExposure::new(0)?, stops, budget))
    } else {
        None
    };
    let compare_upload_submit = args.first().is_some_and(|arg| arg == "--compare-upload-submit");
    if compare_upload_submit {
        args.remove(0);
    }
    let cache_only = args.first().is_some_and(|arg| arg == "--cache-only");
    if cache_only {
        args.remove(0);
    }
    let swap_view = args.first().is_some_and(|arg| arg == "--swap-view");
    if swap_view {
        args.remove(0);
    }
    if swap_view && (cache_only || compare_upload_submit) {
        return Err("--swap-view cannot be combined with other timing modes".into());
    }
    let paths: Vec<PathBuf> = args.into_iter().map(Into::into).collect();
    if paths.is_empty() {
        return Err("pass one or more RAW fixture paths".into());
    }
    if cache_only {
        return cache_timing(&paths);
    }
    let backend = match std::env::var("RRRAH_GPU_BACKEND") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => "auto".into(),
        Err(error) => return Err(error.into()),
    }
    .parse::<rrrah_gpu::GpuBackend>()?;
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    backend.configure(&mut descriptor)?;
    let instance = wgpu::Instance::new(descriptor);
    let vendor = match std::env::var("RRRAH_GPU_VENDOR") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => if cuda.is_some() { "nvidia" } else { "any" }.into(),
        Err(error) => return Err(error.into()),
    }
    .parse::<rrrah_gpu::GpuVendor>()?;
    if cuda.is_some() && vendor != rrrah_gpu::GpuVendor::Nvidia {
        return Err("CUDA view timing requires RRRAH_GPU_VENDOR=nvidia".into());
    }
    let adapter = pollster::block_on(vendor.request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        },
    ))?;
    let info = adapter.get_info();
    eprintln!("adapter: {:?} {}", info.backend, info.name);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("RAW view timing target"),
        size: wgpu::Extent3d {
            width: 1920,
            height: 1080,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let target = target.create_view(&wgpu::TextureViewDescriptor::default());
    let cpu_budget = env_budget("RRRAH_VIEW_CPU_MB")?
        .or_else(|| swap_view.then(|| rrrah_core::MemoryBudget::new(512 * 1024 * 1024)));
    let gpu_budget = env_budget("RRRAH_VIEW_GPU_MB")?;
    if raster_view {
        if cache_only || swap_view || compare_upload_submit {
            return Err("raster mode cannot combine with RAW modes".into());
        }
        return raster_view_timing(
            &paths,
            &device,
            &queue,
            &target,
            format,
            cpu_budget,
            gpu_budget,
            cuda,
            compute_stops,
        );
    }
    let mut renderer = match &gpu_budget {
        Some(budget) => RawRenderer::new_with_budget(&device, format, budget.clone()),
        None => RawRenderer::new(&device, format),
    };
    if let Some(budget) = &cpu_budget {
        renderer = renderer.with_upload_memory_budget(budget.clone());
    }
    if compare_upload_submit {
        compare_submission(
            &paths,
            &device,
            &queue,
            &mut renderer,
            &target,
            cpu_budget.clone(),
        )?;
        drop(renderer);
        for budget in [cpu_budget, gpu_budget].into_iter().flatten() {
            assert_eq!(budget.used(), 0);
            eprintln!("managed peak={} limit={}", budget.peak(), budget.limit());
        }
        return Ok(());
    }
    let load_kind = if swap_view { "swap_restore" } else { "decode" };
    println!(
        "kind,fixture,iteration,width,height,{load_kind}_ms,upload_enqueue_ms,halo_pack_ms,row_pack_ms,enqueue_write_ms,frame_complete_ms,total_ms"
    );
    for path in paths {
        let name = path.file_name().unwrap().to_string_lossy();
        let mut expected = None;
        let mut expected_metadata = None;
        let swap_source = if swap_view {
            use rrrah_cache::{CacheLimits, ImageSwapCache, MosaicSwapConfig};
            let root = cpu_budget.as_ref().unwrap();
            let mut request = rrrah_decode::DecodeRequest::new(&path);
            request.memory_budget = Some(root.clone());
            let mosaic = rrrah_decode::NativeRawDecoder.decode(&request)?.mosaic;
            let weight = mosaic.pixels.capacity_bytes();
            expected = Some(mosaic.pixels.clone());
            expected_metadata = Some(mosaic.metadata.clone());
            let directory = tempfile::tempdir()?;
            let swap = ImageSwapCache::<u8, rrrah_core::DecodedMosaic>::new_with_budgets(
                directory.path(),
                MosaicSwapConfig {
                    limits: CacheLimits {
                        max_bytes: weight.saturating_mul(2),
                        max_entries: Some(1),
                        ttl: None,
                    },
                    queue_bytes: weight,
                    queue_count: 1,
                    restore_bytes: weight,
                },
                rrrah_core::MemoryBudget::new(weight),
                root.clone(),
            )?;
            swap.enqueue(1, mosaic);
            swap.wait_idle()?;
            if swap.stats().writes != 1 || root.used() != weight {
                return Err("swap seed/release failure".into());
            }
            Some((directory, swap))
        } else {
            None
        };
        let mut decode_values = Vec::new();
        let mut upload_values = Vec::new();
        let mut frame_values = Vec::new();
        let mut total_values = Vec::new();
        for iteration in 0..18 {
            let total = Instant::now();
            let decode = Instant::now();
            let mut request = rrrah_decode::DecodeRequest::new(&path);
            request.memory_budget = cpu_budget.clone();
            let mosaic = if let Some((_, swap)) = &swap_source {
                black_box(swap.try_get(&1, || false)?).ok_or("swap restore missed")?
            } else {
                black_box(rrrah_decode::NativeRawDecoder.decode(&request)?).mosaic
            };
            let decode_ms = ms(decode);
            let timing = renderer.upload_mosaic(&device, &queue, &mosaic)?;
            renderer.update_view(
                &queue,
                ViewParameters {
                    viewport: [1920., 1080.],
                    ..Default::default()
                },
            );
            let draw = Instant::now();
            frame(&device, &queue, &renderer, &target)?;
            let frame_ms = ms(draw);
            let total_ms = ms(total);
            if let Some(metadata) = &expected_metadata {
                assert_eq!(metadata, &mosaic.metadata, "swap metadata changed");
            }
            // Pixel identity is checked outside all timed intervals.
            if let Some(pixels) = &expected {
                assert_eq!(pixels, &mosaic.pixels, "load output changed");
            } else {
                expected = Some(mosaic.pixels.clone());
            }
            if iteration >= 3 {
                let upload_ms = timing.total.as_secs_f64() * 1000.;
                decode_values.push(decode_ms);
                upload_values.push(upload_ms);
                frame_values.push(frame_ms);
                total_values.push(total_ms);
                println!(
                    "sample,{name},{},{},{},{decode_ms:.3},{upload_ms:.3},{:.3},{:.3},{:.3},{frame_ms:.3},{total_ms:.3}",
                    iteration - 2,
                    mosaic.metadata.width,
                    mosaic.metadata.height,
                    timing.halo_pack.as_secs_f64() * 1000.,
                    timing.row_pack.as_secs_f64() * 1000.,
                    timing.texture_write_enqueue.as_secs_f64() * 1000.
                );
            }
        }
        eprintln!(
            "{name}: 15 samples / 3 warmups; {load_kind} p50/p95 {:.3}/{:.3} ms; upload enqueue {:.3}/{:.3} ms; frame completion {:.3}/{:.3} ms; total {:.3}/{:.3} ms",
            percentile(&decode_values, 0.5),
            percentile(&decode_values, 0.95),
            percentile(&upload_values, 0.5),
            percentile(&upload_values, 0.95),
            percentile(&frame_values, 0.5),
            percentile(&frame_values, 0.95),
            percentile(&total_values, 0.5),
            percentile(&total_values, 0.95)
        );
        if let Some((_, swap)) = &swap_source {
            assert_eq!(swap.stats().reads, 18);
            assert_eq!(swap.stats().errors, 0);
        }
        let mut resident = Vec::new();
        for iteration in 0..63 {
            let started = Instant::now();
            renderer.update_view(
                &queue,
                ViewParameters {
                    viewport: [1920., 1080.],
                    zoom: if iteration % 2 == 0 { 1.0 } else { 2.0 },
                    pan: [(iteration % 7) as f32 * 10., 0.],
                    ..Default::default()
                },
            );
            frame(&device, &queue, &renderer, &target)?;
            let elapsed = ms(started);
            if iteration >= 3 {
                resident.push(elapsed);
                println!("resident,{name},{iteration},1920,1080,,,,,,,{elapsed:.3}");
            }
        }
        eprintln!(
            "{name}: resident zoom/pan 60 frames; p50/p95 {:.3}/{:.3} ms",
            percentile(&resident, 0.5),
            percentile(&resident, 0.95)
        );
    }
    drop(renderer);
    for (kind, budget) in [("cpu", cpu_budget), ("gpu", gpu_budget)] {
        if let Some(budget) = budget {
            eprintln!(
                "managed_{kind}: used={} peak={} limit={}",
                budget.used(),
                budget.peak(),
                budget.limit()
            );
            assert_eq!(
                budget.used(),
                0,
                "retained {kind} reservation after completed run"
            );
        }
    }
    Ok(())
}

// Isolate upload submission overhead: decode/cache setup outside timing, alternate
// A/B order by pair, wait for each completed frame before the next sample.
fn compare_submission(
    paths: &[PathBuf],
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &mut RawRenderer,
    target: &wgpu::TextureView,
    budget: Option<rrrah_core::MemoryBudget>,
) -> Result<(), Box<dyn Error>> {
    use rrrah_cache::{CacheKey, MosaicRamCache, SourceFingerprint};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    println!("fixture,pair,mode,upload_ms,submit_ms,frame_ms,total_ms");
    for path in paths {
        let mut request = rrrah_decode::DecodeRequest::new(path);
        request.memory_budget = budget.clone();
        let output = rrrah_decode::NativeRawDecoder.decode(&request)?;
        let key = CacheKey::for_mosaic_recipe(
            &SourceFingerprint::from_path(path)?,
            0,
            rrrah_decode::NativeRawDecoder.mosaic_recipe(&request)?,
        );
        let mut cache = MosaicRamCache::new(output.mosaic.pixels.capacity_bytes());
        assert!(cache.insert(key, output.mosaic));
        let mut values = [Vec::new(), Vec::new()];
        let mut paired = Vec::new();
        for pair in 0..24 {
            let mut totals = [0.; 2];
            for mode in if pair % 2 == 0 { [0, 1] } else { [1, 0] } {
                let lease = cache.get_lease(&key).unwrap();
                let completed = Arc::new(AtomicBool::new(false));
                let start = Instant::now();
                let upload = renderer.upload_mosaic(device, queue, &lease)?;
                let submit = Instant::now();
                if mode == 1 {
                    queue.submit([]);
                    let done = completed.clone();
                    queue.on_submitted_work_done(move || {
                        drop(lease);
                        done.store(true, Ordering::Release);
                    });
                } else {
                    drop(lease);
                }
                let submit_ms = ms(submit);
                renderer.update_view(
                    queue,
                    ViewParameters {
                        viewport: [1920., 1080.],
                        ..Default::default()
                    },
                );
                let draw = Instant::now();
                frame(device, queue, renderer, target)?;
                let frame_ms = ms(draw);
                let total_ms = ms(start);
                if mode == 1 {
                    assert!(completed.load(Ordering::Acquire));
                }
                totals[mode] = total_ms;
                if pair >= 4 {
                    values[mode].push(total_ms);
                    println!(
                        "{},{},{},{:.6},{submit_ms:.6},{frame_ms:.6},{total_ms:.6}",
                        path.file_name().unwrap().to_string_lossy(),
                        pair - 3,
                        if mode == 0 { "combined" } else { "split-lease" },
                        upload.total.as_secs_f64() * 1000.
                    );
                }
            }
            if pair >= 4 {
                paired.push(totals[1] - totals[0]);
            }
        }
        eprintln!(
            "{}: 20 measured pairs / 4 warmup pairs; combined total p50/p95 {:.3}/{:.3} ms; split-lease {:.3}/{:.3} ms; paired delta p50/p95 {:.3}/{:.3} ms",
            path.display(),
            percentile(&values[0], 0.5),
            percentile(&values[0], 0.95),
            percentile(&values[1], 0.5),
            percentile(&values[1], 0.95),
            percentile(&paired, 0.5),
            percentile(&paired, 0.95)
        );
    }
    Ok(())
}

// Warm OS-cache timings. Swap serialization and setup are outside read intervals.
// Each swap sample allocates and verifies a new complete mosaic; no GPU work.
fn cache_timing(paths: &[PathBuf]) -> Result<(), Box<dyn Error>> {
    use rrrah_cache::{
        CacheKey, CacheLimits, DiskMosaicCache, MosaicRamCache, MosaicSwapCache, MosaicSwapConfig,
        SourceFingerprint,
    };
    println!("kind,fixture,iteration,pixel_bytes,elapsed_ms");
    for path in paths {
        let root = env_budget("RRRAH_CACHE_CPU_MB")?
            .unwrap_or_else(|| rrrah_core::MemoryBudget::new(512 * 1024 * 1024));
        let mut request = rrrah_decode::DecodeRequest::new(path);
        request.memory_budget = Some(root.clone());
        let recipe = rrrah_decode::NativeRawDecoder.mosaic_recipe(&request)?;
        let expected = rrrah_decode::NativeRawDecoder.decode(&request)?.mosaic;
        assert!(expected.pixels.is_managed());
        let bytes = expected.pixels.capacity_bytes();
        let key = CacheKey::for_mosaic_recipe(&SourceFingerprint::from_path(path)?, 0, recipe);
        let parent = tempfile::tempdir()?;
        let disk = DiskMosaicCache::with_max_bytes(parent.path().join("persistent"), bytes.saturating_mul(2));
        disk.store(key, &expected)?;
        let restore_budget = root.child(bytes.saturating_mul(2));
        let swap = MosaicSwapCache::new_with_budgets(
            parent.path(),
            MosaicSwapConfig {
                limits: CacheLimits::bytes(bytes.saturating_mul(2)),
                queue_bytes: bytes,
                queue_count: 4,
                restore_bytes: bytes.saturating_mul(2),
            },
            rrrah_core::MemoryBudget::new(bytes),
            restore_budget.clone(),
        )?;
        swap.enqueue(key, expected.clone());
        swap.wait_idle()?;
        assert_eq!(swap.stats().writes, 1);
        let mut ram = MosaicRamCache::new(bytes);
        assert!(ram.insert_visible(key, expected.clone()));
        let mut decode_times = Vec::new();
        let mut swap_times = Vec::new();
        let mut disk_times = Vec::new();
        let mut ram_times = Vec::new();
        let name = path.file_name().unwrap().to_string_lossy();
        for iteration in 0..20 {
            let mut values = [0.0; 4];
            // Each tier appears in each position once per four-round cycle.
            for offset in 0..4 {
                let kind = (iteration + offset) % 4;
                let start = Instant::now();
                match kind {
                    0 => {
                        let decoded = black_box(rrrah_decode::NativeRawDecoder.decode(&request)?).mosaic;
                        values[kind] = ms(start);
                        assert_eq!(decoded.metadata, expected.metadata);
                        assert_eq!(decoded.pixels, expected.pixels);
                    }
                    1 => {
                        let cached = black_box(disk.load_with_cancel(key, Some(&restore_budget), || false)?)
                            .ok_or("persistent cache missed")?
                            .mosaic;
                        values[kind] = ms(start);
                        assert!(cached.pixels.is_managed());
                        assert_eq!(cached.metadata, expected.metadata);
                        assert_eq!(cached.pixels, expected.pixels);
                    }
                    2 => {
                        let restored =
                            black_box(swap.try_get(&key, || false)?).ok_or("swap restore missed")?;
                        values[kind] = ms(start);
                        assert!(restored.pixels.is_managed());
                        assert_eq!(restored.metadata, expected.metadata);
                        assert_eq!(restored.pixels, expected.pixels);
                    }
                    _ => {
                        for _ in 0..1000 {
                            black_box(ram.get(&key).ok_or("RAM miss")?);
                        }
                        values[kind] = ms(start) / 1000.;
                        assert!(ram.get(&key).unwrap().pixels.ptr_eq(&expected.pixels));
                    }
                }
            }
            if iteration >= 4 {
                decode_times.push(values[0]);
                disk_times.push(values[1]);
                swap_times.push(values[2]);
                ram_times.push(values[3]);
                for (kind, value) in ["decode", "disk", "swap", "ram"].into_iter().zip(values) {
                    println!("{kind},{name},{},{bytes},{value:.6}", iteration - 4);
                }
            }
        }
        assert_eq!(swap.stats().errors, 0);
        for (kind, times) in [
            ("decode", decode_times),
            ("disk", disk_times),
            ("swap", swap_times),
            ("ram", ram_times),
        ] {
            eprintln!(
                "{name} {kind} p50={:.6}ms p95={:.6}ms",
                percentile(&times, 0.5),
                percentile(&times, 0.95)
            );
        }
        drop(ram);
        drop(swap);
        drop(expected);
        drop(disk);
        eprintln!(
            "{name} managed root used={} peak={} limit={}; restore used={} peak={} limit={}",
            root.used(),
            root.peak(),
            root.limit(),
            restore_budget.used(),
            restore_budget.peak(),
            restore_budget.limit()
        );
        assert_eq!(root.used(), 0);
        assert_eq!(restore_budget.used(), 0);
    }
    Ok(())
}

fn raster_view_timing(
    paths: &[PathBuf],
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &wgpu::TextureView,
    format: wgpu::TextureFormat,
    cpu: Option<rrrah_core::MemoryBudget>,
    gpu: Option<rrrah_core::MemoryBudget>,
    cuda: Option<(rrrah_cuda::CudaExposure, f32, rrrah_core::MemoryBudget)>,
    compute_stops: Option<f32>,
) -> Result<(), Box<dyn Error>> {
    let cpu = cpu.unwrap_or_else(|| rrrah_core::MemoryBudget::new(512 * 1024 * 1024));
    let gpu = gpu.unwrap_or_else(|| rrrah_core::MemoryBudget::new(128 * 1024 * 1024));
    let mut renderer = rrrah_gpu::RasterRenderer::new_with_budget(device, format, gpu.clone());
    if cuda.is_some() && compute_stops.is_some() {
        return Err("select one exposure backend".into());
    }
    let compute = compute_stops.map(|stops| (rrrah_gpu::LinearExposureCompute::new(device), stops));
    for path in paths {
        let mut times = Vec::new();
        let mut stage_times = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        for iteration in 0..18 {
            let started = Instant::now();
            let mut request = rrrah_decode::DecodeRequest::new(path);
            request.memory_budget = Some(cpu.clone());
            if rrrah_decode::image_source_kind(&request)? != rrrah_decode::ImageSourceKind::Raster {
                return Err("raster mode requires a raster source".into());
            }
            let source = rrrah_decode::decode_raster(&request)?;
            let decode_ms = ms(started);
            let prepare_started = Instant::now();
            let raster =
                rrrah_decode::prepare_raster_for_display_with_budget_and_cancel(&source, Some(&cpu), || {
                    false
                })?;
            drop(source);
            let prepare_ms = ms(prepare_started);
            let transfer_started = Instant::now();
            let raster = if let Some((compute, stops, cuda_gpu)) = &cuda {
                let rrrah_core::RasterPixels::Rgba32Float(pixels) = raster.pixels() else {
                    return Err("CUDA exposure requires prepared RGBA32F".into());
                };
                let output = compute.execute_interleaved(pixels, *stops, cuda_gpu, &cpu, || false)?;
                let result = rrrah_core::DecodedRaster::new(
                    raster.width(),
                    raster.height(),
                    rrrah_core::RasterPixels::Rgba32Float(output.into()),
                    rrrah_core::RasterColorSpace::LinearSrgb,
                )?
                .with_pixel_aspect(raster.pixel_aspect())?
                .with_sample_scale(raster.sample_scale())?
                .with_image_selection(raster.image_index(), raster.image_count())?
                .with_hotspot(raster.hotspot())?;
                drop(raster);
                result
            } else {
                raster
            };
            if let Some((compute, stops)) = &compute {
                let rrrah_core::RasterPixels::Rgba32Float(pixels) = raster.pixels() else {
                    return Err("compute exposure requires prepared RGBA32F".into());
                };
                let (pixels, remainder) = pixels.as_chunks::<4>();
                if !remainder.is_empty() {
                    return Err("incomplete prepared RGBA32F".into());
                }
                let output = compute.execute_resident(queue, pixels, *stops, &gpu, || false)?;
                renderer.upload_resident_with_aspect(
                    device,
                    queue,
                    &output,
                    [raster.width(), raster.height()],
                    raster.pixel_aspect(),
                    || false,
                )?;
            } else {
                renderer.upload(device, queue, &raster)?;
            }
            let transfer_ms = ms(transfer_started);
            let frame_started = Instant::now();
            renderer.update_view(
                queue,
                ViewParameters {
                    viewport: [1920.0, 1080.0],
                    ..Default::default()
                },
            );
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            renderer.encode(&mut encoder, target);
            let submission = queue.submit([encoder.finish()]);
            device.poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: None,
            })?;
            let elapsed = ms(started);
            if iteration >= 3 {
                times.push(elapsed);
                for (samples, value) in
                    stage_times
                        .iter_mut()
                        .zip([decode_ms, prepare_ms, transfer_ms, ms(frame_started)])
                {
                    samples.push(value);
                }
            }
            drop(raster);
            assert_eq!(cpu.used(), 0);
        }
        println!(
            "{} raster decode+prepare{}+upload+completed-frame p50={:.3} p95={:.3} ms n={}",
            path.display(),
            if cuda.is_some() {
                "+CUDA-exposure-transfers"
            } else if compute.is_some() {
                "+WGSL-resident-exposure-GPU-copy"
            } else {
                ""
            },
            percentile(&times, 0.5),
            percentile(&times, 0.95),
            times.len()
        );
        for (name, samples) in ["decode", "prepare", "exposure+transfer-submit", "completed-frame"]
            .into_iter()
            .zip(&stage_times)
        {
            println!(
                "stage,{name},p50={:.3},p95={:.3},ms,n={}",
                percentile(samples, 0.5),
                percentile(samples, 0.95),
                samples.len()
            );
        }
    }
    drop(renderer);
    assert_eq!(gpu.used(), 0);
    assert_eq!(cpu.used(), 0);
    if let Some((_, _, budget)) = cuda {
        assert_eq!(budget.used(), 0);
        eprintln!("CUDA device input/output peak={} released=0", budget.peak());
    }
    eprintln!(
        "raster managed CPU peak={} GPU peak={}; released CPU=0 GPU=0",
        cpu.peak(),
        gpu.peak()
    );
    Ok(())
}
