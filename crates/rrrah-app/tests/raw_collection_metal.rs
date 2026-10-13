#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

use rrrah_cache::{
    CacheKey, CacheLimits, MemoryBudget, MosaicRamCache, MosaicSwapCache, MosaicSwapConfig, SourceFingerprint,
};
use rrrah_decode::RawDecoder;

/// Required external fixture list: one path per line, no skipped files.
#[test]
#[ignore = "manual real-camera collection qualification on Metal"]
fn real_raw_collection_eviction_and_swap_preserve_metal_frames() {
    let manifest = std::env::var("RRRAH_RAW_COLLECTION_MANIFEST").expect("fixture manifest required");
    let paths: Vec<_> = std::fs::read_to_string(manifest)
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(std::path::PathBuf::from)
        .collect();
    assert!(paths.len() >= 2);
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("RAW collection adapter: {}", gpu.adapter_name());
    let root = MemoryBudget::new(512 * 1024 * 1024);
    let queue = MemoryBudget::new(128 * 1024 * 1024);
    let directory = tempfile::tempdir().unwrap();
    let mut ram = MosaicRamCache::with_limits(CacheLimits {
        max_bytes: 128 * 1024 * 1024,
        max_entries: Some(1),
        ttl: None,
    });
    ram.enable_swap(
        MosaicSwapCache::new_with_budgets(
            directory.path(),
            MosaicSwapConfig {
                limits: CacheLimits {
                    max_bytes: 2 * 1024 * 1024 * 1024,
                    max_entries: Some(paths.len()),
                    ttl: None,
                },
                queue_bytes: queue.limit(),
                queue_count: 1,
                restore_bytes: 128 * 1024 * 1024,
            },
            queue.clone(),
            root.clone(),
        )
        .unwrap(),
    );
    let mut expected = Vec::new();
    for path in paths {
        let mut request = rrrah_decode::DecodeRequest::new(&path);
        request.memory_budget = Some(root.clone());
        let decode_started = std::time::Instant::now();
        let mosaic = rrrah_decode::NativeRawDecoder.decode(&request).unwrap().mosaic;
        let decode_ms = decode_started.elapsed().as_secs_f64() * 1000.0;
        let key = CacheKey::for_mosaic_recipe(
            &SourceFingerprint::from_path(&path).unwrap(),
            0,
            rrrah_decode::NativeRawDecoder.mosaic_recipe(&request).unwrap(),
        );
        let pixels = mosaic.pixels.to_vec();
        let gpu_started = std::time::Instant::now();
        let frame = gpu.render(&mosaic, [1920, 1080]).pixels;
        let gpu_ms = gpu_started.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "RAW initial latency: source={}; native_decode_ms={decode_ms:.3}; gpu_upload_render_readback_ms={gpu_ms:.3}; output=1920x1080",
            path.display()
        );
        eprintln!(
            "seed {}: {}x{}; {} samples",
            path.display(),
            mosaic.metadata.width,
            mosaic.metadata.height,
            pixels.len()
        );
        let hdr_frame = gpu.render_raw_half(&mosaic, [1920, 1080]);
        assert!(
            hdr_frame.iter().all(|bits| bits & 0x7c00 != 0x7c00),
            "non-finite binary16 RAW output"
        );
        let above_one = hdr_frame
            .chunks_exact(4)
            .flat_map(|p| &p[..3])
            .filter(|bits| **bits > 0x3c00 && **bits < 0x7c00)
            .count();
        let negative = hdr_frame
            .chunks_exact(4)
            .flat_map(|p| &p[..3])
            .filter(|bits| **bits & 0x8000 != 0 && **bits & 0x7fff != 0)
            .count();
        eprintln!(
            "RAW collection HDR seed: source={}; binary16=finite; above_one={above_one}; negative={negative}",
            path.display()
        );
        expected.push((key, mosaic.metadata.clone(), pixels, frame, hdr_frame));
        assert!(ram.insert(key, mosaic));
        assert!(ram.set_limits_and_spill(CacheLimits::bytes(0)));
        ram.swap().unwrap().wait_idle().unwrap();
        assert_eq!(root.used(), 0);
        assert_eq!(queue.used(), 0);
        assert!(ram.set_limits_and_spill(CacheLimits {
            max_bytes: 128 * 1024 * 1024,
            max_entries: Some(1),
            ttl: None
        }));
    }
    assert_eq!(ram.swap().unwrap().stats().writes, expected.len() as u64);
    let seconds: u64 = std::env::var("RRRAH_RAW_COLLECTION_SECONDS")
        .map(|value| value.parse().expect("duration must be an integer"))
        .unwrap_or(0);
    assert!(seconds <= 86400, "duration must be at most one day");
    let started = std::time::Instant::now();
    let gpu_resident = MemoryBudget::new(512 * 1024 * 1024);
    let gpu_upload = MemoryBudget::new(64 * 1024 * 1024);
    let gpu_queue = MemoryBudget::new(256 * 1024 * 1024);
    let mut renderer =
        rrrah_gpu::RawRenderer::new_with_budget(&gpu.device, common::READBACK_FORMAT, gpu_resident.clone())
            .with_upload_memory_budget(gpu_upload.clone())
            .with_upload_queue_budget(gpu_queue.clone());
    let mut hdr_renderer = rrrah_gpu::RawRenderer::new_with_budget(
        &gpu.device,
        wgpu::TextureFormat::Rgba16Float,
        gpu_resident.clone(),
    )
    .with_upload_memory_budget(gpu_upload.clone())
    .with_upload_queue_budget(gpu_queue.clone());
    hdr_renderer
        .set_output_mode(&gpu.queue, rrrah_gpu::RawOutputMode::SceneLinear)
        .unwrap();
    let mut reads = 0;
    let mut previous_index: Option<usize> = None;
    let mut gpu_pressure_refusals = 0;
    let mut gpu_partial_cancellations = 0;
    let mut hdr_pressure_refusals = 0;
    let mut hdr_partial_cancellations = 0;
    let mut round = 0;
    // Bounded timing window: one-day soaks must not grow benchmark bookkeeping.
    let mut restore_us = Vec::with_capacity(4096);
    let mut gpu_us = Vec::with_capacity(4096);
    while round < 10 || started.elapsed().as_secs() < seconds {
        // Forward/backward traversal uses one retained cache and swap worker.
        for position in 0..expected.len() {
            let index = if round % 2 == 0 {
                position
            } else {
                expected.len() - 1 - position
            };
            let (key, metadata, pixels, frame, hdr_frame) = &expected[index];
            let pressure = root.try_reserve(root.available_bytes()).unwrap();
            assert!(ram.get(key).is_none());
            assert_eq!(ram.swap().unwrap().stats().errors, 0);
            drop(pressure);
            assert!(ram.get_background_with_cancel(key, || true).is_none());
            assert_eq!(root.used(), 0);
            let restore_started = std::time::Instant::now();
            let restored = ram.get(key).expect("required collection restore missed");
            let restore_elapsed = restore_started.elapsed().as_secs_f64() * 1_000_000.0;
            reads += 1;
            assert_eq!(&restored.metadata, metadata);
            assert_eq!(&*restored.pixels, pixels.as_slice());
            if let Some(previous) = previous_index {
                let retained = gpu_resident.used();
                let pressure = gpu_resident.try_reserve(gpu_resident.available_bytes()).unwrap();
                assert!(matches!(
                    renderer.upload_mosaic(&gpu.device, &gpu.queue, &restored),
                    Err(rrrah_gpu::GpuError::Memory(_))
                ));
                assert!(matches!(
                    hdr_renderer.upload_mosaic(&gpu.device, &gpu.queue, &restored),
                    Err(rrrah_gpu::GpuError::Memory(_))
                ));
                assert_eq!(gpu_resident.used(), gpu_resident.limit());
                drop(pressure);
                assert_eq!(gpu_resident.used(), retained);
                let previous_frame = gpu.render_existing_raw(&renderer, [1920, 1080]);
                assert_eq!(&previous_frame.pixels, &expected[previous].3);
                assert_eq!(gpu_upload.used(), 0);
                assert_eq!(gpu_queue.used(), 0);
                gpu_pressure_refusals += 1;
                assert_eq!(
                    &gpu.render_existing_raw_half(&hdr_renderer, [1920, 1080]),
                    &expected[previous].4
                );
                assert_eq!(gpu_resident.used(), retained);
                hdr_pressure_refusals += 1;
                // Force small tiles so 64 cooperative checks cross completed
                // tile writes before cancelling a later packing operation.
                let checks = std::cell::Cell::new(0);
                assert!(matches!(
                    renderer.upload_mosaic_with_tiling_and_cancel(
                        &gpu.device,
                        &gpu.queue,
                        &restored,
                        rrrah_gpu::TilingOverrides {
                            tile_size: Some(512),
                            tile_halo: Some(2)
                        },
                        || {
                            let n = checks.get() + 1;
                            checks.set(n);
                            n >= 64
                        },
                    ),
                    Err(rrrah_gpu::GpuError::Cancelled)
                ));
                assert_eq!(checks.get(), 64);
                assert_eq!(gpu_upload.used(), 0);
                assert_eq!(
                    renderer.resident_bytes() + hdr_renderer.resident_bytes(),
                    retained
                );
                let previous_frame = gpu.render_existing_raw(&renderer, [1920, 1080]);
                assert_eq!(&previous_frame.pixels, &expected[previous].3);
                assert_eq!(gpu_resident.used(), retained);
                assert_eq!(gpu_queue.used(), 0);
                gpu_partial_cancellations += 1;
                checks.set(0);
                assert!(matches!(
                    hdr_renderer.upload_mosaic_with_tiling_and_cancel(
                        &gpu.device,
                        &gpu.queue,
                        &restored,
                        rrrah_gpu::TilingOverrides {
                            tile_size: Some(512),
                            tile_halo: Some(2)
                        },
                        || {
                            let n = checks.get() + 1;
                            checks.set(n);
                            n >= 64
                        },
                    ),
                    Err(rrrah_gpu::GpuError::Cancelled)
                ));
                assert_eq!(checks.get(), 64);
                assert_eq!(gpu_upload.used(), 0);
                assert_eq!(
                    renderer.resident_bytes() + hdr_renderer.resident_bytes(),
                    retained
                );
                assert_eq!(
                    &gpu.render_existing_raw_half(&hdr_renderer, [1920, 1080]),
                    &expected[previous].4
                );
                assert_eq!(gpu_resident.used(), retained);
                assert_eq!(gpu_queue.used(), 0);
                hdr_partial_cancellations += 1;
            }
            let gpu_started = std::time::Instant::now();
            let rendered = gpu.render_replacing_raw(&mut renderer, &restored, [1920, 1080]);
            let hdr_rendered = gpu.render_replacing_raw_half(&mut hdr_renderer, &restored, [1920, 1080]);
            assert_eq!(&hdr_rendered, hdr_frame);
            drop(hdr_rendered);
            assert_eq!(
                gpu_resident.used(),
                renderer.resident_bytes() + hdr_renderer.resident_bytes()
            );
            assert_eq!(gpu_upload.used(), 0);
            assert_eq!(gpu_queue.used(), 0);
            let gpu_elapsed = gpu_started.elapsed().as_secs_f64() * 1_000_000.0;
            assert_eq!(&rendered.pixels, frame);
            drop(rendered);
            previous_index = Some(index);
            if restore_us.len() < 4096 {
                restore_us.push(restore_elapsed);
                gpu_us.push(gpu_elapsed);
            } else {
                let slot = (reads as usize - 1) % 4096;
                restore_us[slot] = restore_elapsed;
                gpu_us[slot] = gpu_elapsed;
            }
            drop(restored);
            assert!(
                !ram.set_limits_and_spill(CacheLimits::bytes(0)),
                "visible frame must remain pinned"
            );
            ram.mark_visible(&expected[(index + 1) % expected.len()].0);
            assert!(ram.set_limits_and_spill(CacheLimits::bytes(0)));
            ram.swap().unwrap().wait_idle().unwrap();
            assert!(ram.is_empty());
            assert_eq!(root.used(), 0);
            assert_eq!(queue.used(), 0);
            assert_eq!(ram.swap().unwrap().stats().errors, 0);
            assert!(ram.set_limits_and_spill(CacheLimits {
                max_bytes: 128 * 1024 * 1024,
                max_entries: Some(1),
                ttl: None
            }));
        }
        round += 1;
        eprintln!(
            "RAW collection rounds={round}; elapsed={:.3}s; reads={reads}; root=0; queue=0",
            started.elapsed().as_secs_f64()
        );
    }
    assert!(started.elapsed().as_secs() >= seconds);
    eprintln!(
        "RAW collection complete: rounds={round}; elapsed={:.3}s; reads={reads}; writes={}; root=0; queue=0",
        started.elapsed().as_secs_f64(),
        expected.len()
    );
    for (stage, mut samples) in [("restore", restore_us), ("gpu_upload_render_readback", gpu_us)] {
        samples.sort_by(f64::total_cmp);
        let percentile = |percent: usize| samples[(samples.len() * percent).div_ceil(100) - 1] / 1000.0;
        eprintln!(
            "RAW collection latency: stage={stage}; samples={}; total={reads}; window=last4096; p50_ms={:.3}; p95_ms={:.3}; max_ms={:.3}",
            samples.len(),
            percentile(50),
            percentile(95),
            samples.last().unwrap() / 1000.0
        );
    }
    assert_eq!(ram.swap().unwrap().stats().reads, reads);
    assert_eq!(ram.swap().unwrap().stats().writes, expected.len() as u64);
    assert_eq!(hdr_pressure_refusals, reads - 1);
    assert_eq!(hdr_partial_cancellations, reads - 1);
    eprintln!(
        "RAW HDR replacement failures complete: pressure={hdr_pressure_refusals}; cancellations={hdr_partial_cancellations}; binary16-previous-exact; credit-retained; temporary-credit-zero; retry-exact"
    );
    assert_eq!(gpu_pressure_refusals, reads - 1);
    assert_eq!(gpu_partial_cancellations, reads - 1);
    eprintln!(
        "RAW partial GPU cancellation complete: cancellations={gpu_partial_cancellations}; checks=64; tile=512; halo=2; previous-frame-exact; scratch-queue-released; retry-exact"
    );
    eprintln!(
        "RAW GPU pressure complete: refusals={gpu_pressure_refusals}; previous-frame-exact; reservation-unchanged; retry-exact"
    );
    assert!(gpu_resident.peak() <= gpu_resident.limit());
    eprintln!(
        "RAW collection HDR complete: frames={reads}; binary16-exact; finite; persistent-renderer; SDR-HDR-shared-root"
    );
    drop(renderer);
    drop(hdr_renderer);
    assert_eq!(gpu_resident.used(), 0);
    assert_eq!(gpu_upload.used(), 0);
    assert_eq!(gpu_queue.used(), 0);
    eprintln!(
        "RAW persistent GPU complete: resident_peak={}; upload_peak={}; queue_peak={}; resident=0; upload=0; queue=0",
        gpu_resident.peak(),
        gpu_upload.peak(),
        gpu_queue.peak()
    );
    drop(ram);
    assert_eq!(root.used(), 0);
    assert_eq!(queue.used(), 0);
}
