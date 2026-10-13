use super::*;

#[test]
fn heif_hdr_cli_reaches_preload_and_thumbnail_without_assuming_srgb() {
    for (name, args) in [
        ("heif-pq12.heic", vec!["rrrah", "--pq-reference-white-nits", "100"]),
        ("heif-hlg12.heic", vec!["rrrah", "--hlg-reference-white-nits", "100",
            "--hlg-peak-nits", "1000", "--hlg-system-gamma", "1.2"]),
        ("heif-pq12-color-alpha.heic", vec!["rrrah", "--pq-reference-white-nits", "100"]),
        ("heif-hlg12-color-alpha.heic", vec!["rrrah", "--hlg-reference-white-nits", "100",
            "--hlg-peak-nits", "1000", "--hlg-system-gamma", "1.2"]),
    ] {
        let policy = Cli::try_parse_from(args).unwrap().raster_interpretation();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster").join(name);
        let root = rrrah_core::MemoryBudget::new(256 * 1024);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(root.clone());
        policy.apply(&mut request);
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(root.limit()));
        let gate = Arc::new(DecodeGate::new());
        let (frame, assumed) = preload_raster(&request, None, &mut cache, &gate).unwrap();
        assert!(!assumed);
        assert_eq!(frame.color_space(), &rrrah_core::RasterColorSpace::LinearSrgb);
        let (hit, _) = load_cached_raster_with(&request, None, &mut cache, || panic!("preload cache miss")).unwrap();
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (frame.pixels(), hit.pixels()) else { panic!("expected linear float pixels") };
        assert!(a.ptr_eq(b));
        drop((frame, hit, cache));
        assert_eq!(root.used(), 0);
        let loader = thumbnail_loader_with_cancel(gate, Some(root.clone()), policy);
        let thumbnail = loader(gallery::ThumbnailJob { index: 0, source: path, edge: 32 },
            GenerationToken::new(Arc::new(AtomicU64::new(1)), 1)).unwrap();
        assert_eq!((thumbnail.width, thumbnail.height), (32, 24));
        if name.contains("alpha") {
            // The fully transparent top quarter contains saturated hidden RGB.
            // Its thumbnail must show only the viewer's linear 0.018 background.
            assert!(thumbnail.pixels[..32*6*4].chunks_exact(4)
                .all(|pixel| pixel == [36, 36, 36, 255]));
        }
        drop(thumbnail);
        assert_eq!(root.used(), 0);
    }
}

#[test]
#[ignore = "requires external 24 MP Canon DPP PQ HEIF and actual Metal"]
fn photographic_heif_preload_cache_hit_under_full_pressure_and_metal() {
    let path = PathBuf::from(std::env::var_os("RRRAH_HEIF_CAMERA_SOURCE").expect("external HEIF required"));
    let gpu = navigation_metal_readback::qualification_gpu().expect("actual GPU required");
    eprintln!("photographic preload adapter: {}", gpu.adapter_name());
    let root = rrrah_core::MemoryBudget::new(1024 * 1024 * 1024);
    let mut request = DecodeRequest::new(&path);
    request.memory_budget = Some(root.clone());
    request.pq_reference_white_nits = Some(100.);
    let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(384_000_000));
    let gate = Arc::new(DecodeGate::new());
    let (frame, assumed) = preload_raster(&request, None, &mut cache, &gate).unwrap();
    assert!(!assumed);
    assert_eq!((frame.width(), frame.height()), (6000, 4000));
    assert_eq!(root.used(), 384_000_000);
    let view = rrrah_gpu::ViewParameters { viewport: [128., 64.], ..Default::default() };
    let expected_sdr = gpu.render_raster(&frame, view, [128, 64]).pixels;
    let expected_hdr = gpu.render_raster_half(&frame, view, [128, 64]);
    let pressure = root.try_reserve(root.available_bytes()).unwrap();
    assert_eq!(root.used(), root.limit());
    let (hit, _) = preload_raster(&request, None, &mut cache, &gate).unwrap();
    let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
        (frame.pixels(), hit.pixels()) else { panic!("expected float pixels") };
    assert!(a.ptr_eq(b));
    assert_eq!(gpu.render_raster(&hit, view, [128, 64]).pixels, expected_sdr);
    assert_eq!(gpu.render_raster_half(&hit, view, [128, 64]), expected_hdr);
    let directory = tempfile::tempdir().unwrap();
    let second_path = directory.path().join("other.HIF");
    std::fs::copy(&path, &second_path).unwrap();
    let mut second_request = request.clone();
    second_request.path = second_path;
    let error = preload_raster(&second_request, None, &mut cache, &gate).unwrap_err();
    assert!(error.contains("insufficient source memory headroom"), "{error}");
    let (still_cached, _) = load_cached_raster_with(&request, None, &mut cache,
        || panic!("failed prefetch evicted the ready frame")).unwrap();
    drop(pressure);
    let (retry, _) = preload_raster(&second_request, None, &mut cache, &gate).unwrap();
    assert_eq!(gpu.render_raster(&retry, view, [128, 64]).pixels, expected_sdr);
    assert_eq!(gpu.render_raster_half(&retry, view, [128, 64]), expected_hdr);
    drop((frame, hit, still_cached, retry, cache));
    assert_eq!(root.used(), 0);
    eprintln!("24 MP viewer preload: full-budget RAM hit shares pixels; SDR/HDR exact; new source refused without eviction; retry succeeds; root=0");
}

#[test]
#[ignore = "requires Metal adapter supporting textures wider than portable default"]
fn viewer_uses_supported_metal_texture_extent_above_default() {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    rrrah_gpu::GpuBackend::Metal.configure(&mut descriptor).unwrap();
    let instance = wgpu::Instance::new(descriptor);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        apply_limit_buckets: false, ..Default::default()
    })).unwrap();
    assert_eq!(adapter.get_info().backend, wgpu::Backend::Metal);
    let portable = wgpu::Limits::default().max_texture_dimension_2d;
    let supported = adapter.limits().max_texture_dimension_2d;
    assert!(supported > portable, "adapter must exceed default extent");
    let (mut old_device, mut old_queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: viewer_device_limits(&adapter.limits()), ..Default::default()
    })).unwrap();
    assert_eq!(device.limits().max_texture_dimension_2d, supported);
    let mut gpu = navigation_metal_readback::qualification_gpu().expect("actual GPU required");
    gpu.device = device;
    gpu.queue = queue;
    for width in [portable + 1, supported] {
        let cpu = rrrah_core::MemoryBudget::new(u64::from(width) * 128 * 16);
        let mut values = cpu.try_buffer(width as usize * 128 * 4, 0f32).unwrap();
        for (index,pixel) in values.chunks_exact_mut(4).enumerate() {
            let (x,y)=(index % width as usize,index / width as usize);
            pixel.copy_from_slice(&[(x&1) as f32,(y&1) as f32,((x>>10)&1) as f32,1.]);
        }
        let raster = rrrah_core::DecodedRaster::new(width, 128,
            rrrah_core::RasterPixels::Rgba32Float(values.freeze().into()),
            rrrah_core::RasterColorSpace::LinearSrgb).unwrap();
        let portable_budget = rrrah_core::MemoryBudget::new(raster.pixel_capacity_bytes() * 2);
        let mut old = rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&old_device, portable_budget.clone());
        old.upload(&old_device, &old_queue, &raster).unwrap(); // Tiled on portable device.
        drop(old);
        old_device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(portable_budget.used(),0);
        let texture_budget = rrrah_core::MemoryBudget::new(raster.pixel_capacity_bytes());
        let mut renderer = rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&gpu.device, texture_budget.clone());
        renderer.upload(&gpu.device, &gpu.queue, &raster).unwrap();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(renderer.resident_bytes(), raster.pixel_capacity_bytes());
        drop(renderer);
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(texture_budget.used(), 0);
        // Cancel the viewer's 32px fit margin for a full source-sized 1:1 attachment.
        let view = rrrah_gpu::ViewParameters { viewport: [width as f32,128.],
            zoom: 1. / (((width as f32-32.)/width as f32).min(96./128.)), ..Default::default() };
        let sdr = gpu.render_raster(&raster, view, [width,128]);
        for (index,pixel) in sdr.pixels.chunks_exact(4).enumerate() {
            let (x,y)=(index % width as usize,index / width as usize);
            assert_eq!(pixel,&[((x&1)*255) as u8,((y&1)*255) as u8,(((x>>10)&1)*255) as u8,255],"SDR x={x} y={y}");
        }
        let hdr = gpu.render_raster_half(&raster, view, [width,128]);
        for (index,pixel) in hdr.chunks_exact(4).enumerate() {
            let (x,y)=(index % width as usize,index / width as usize);
            assert_eq!(pixel,&[((x&1)*0x3c00) as u16,((y&1)*0x3c00) as u16,(((x>>10)&1)*0x3c00) as u16,0x3c00],"HDR x={x} y={y}");
        }
        // Crop the same logical view into portable-sized attachments to check
        // tiled address lookup at both edges and interior layer boundaries.
        std::mem::swap(&mut gpu.device,&mut old_device);
        std::mem::swap(&mut gpu.queue,&mut old_queue);
        for start in [0,portable/2-64,(portable-64).min(width-256),width-128] {
            let crop_view=rrrah_gpu::ViewParameters {pan:[-(start as f32),0.],..view};
            let tiled=gpu.render_raster_half(&raster,crop_view,[128,128]);
            for (index,pixel) in tiled.chunks_exact(4).enumerate() {
                let (x,y)=(start as usize+index%128,index/128);
                assert_eq!(pixel,&[((x&1)*0x3c00) as u16,((y&1)*0x3c00) as u16,(((x>>10)&1)*0x3c00) as u16,0x3c00],"tiled x={x} y={y}");
            }
        }
        std::mem::swap(&mut gpu.device,&mut old_device);
        std::mem::swap(&mut gpu.queue,&mut old_queue);
        drop(raster);
        assert_eq!(cpu.used(),0);
        eprintln!("viewer Metal full extent: portable={portable}; supported={supported}; width={width}; height=128; every_SDR_HDR_pixel_valid; coordinate_pattern=true; portable_crop_cases=4; root=0; texture=0; portable_texture=0");
    }
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster/transport-linear.pfm")
}

#[test]
fn linear_cli_pfm_preload_swap_and_strict_cache_boundaries() {
    qualify(None);
}

#[test]
#[ignore = "requires actual Metal device"]
fn linear_cli_pfm_preload_swap_preserves_metal_sdr_and_hdr() {
    let gpu = navigation_metal_readback::qualification_gpu().expect("actual GPU required");
    eprintln!("linear PFM viewer adapter: {}", gpu.adapter_name());
    qualify(Some(&gpu));
}

fn qualify(gpu: Option<&navigation_metal_readback::GpuReadback>) {
    qualify_source(
        gpu,
        fixture(),
        include_bytes!("../../../tests/fixtures/raster/transport-linear.pfm.rgba32f"),
        (2, 2),
    );
}

#[test]
fn linear_cli_astc_hdr_preload_swap_and_strict_cache_boundaries() {
    qualify_astc(None);
}

#[test]
#[ignore = "requires actual Metal device"]
fn linear_cli_astc_hdr_preload_swap_preserves_metal_sdr_and_hdr() {
    let gpu = navigation_metal_readback::qualification_gpu().expect("actual GPU required");
    eprintln!("linear ASTC HDR viewer adapter: {}", gpu.adapter_name());
    qualify_astc(Some(&gpu));
}

fn qualify_astc(gpu: Option<&navigation_metal_readback::GpuReadback>) {
    qualify_source(
        gpu,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster/hdr-compressed.astc"),
        include_bytes!("../../../tests/fixtures/raster/hdr-compressed.astc.rgba32f"),
        (23, 17),
    );
}

#[test]
fn linear_cli_ktx2_hdr_preload_swap_and_strict_cache_boundaries() {
    qualify_ktx2(None);
}

#[test]
#[ignore = "requires actual Metal device"]
fn linear_cli_ktx2_hdr_preload_swap_preserves_metal_sdr_and_hdr() {
    let gpu = navigation_metal_readback::qualification_gpu().expect("actual GPU required");
    eprintln!("linear KTX2 HDR viewer adapter: {}", gpu.adapter_name());
    qualify_ktx2(Some(&gpu));
}

fn qualify_ktx2(gpu: Option<&navigation_metal_readback::GpuReadback>) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("unspecified-primaries.ktx2");
    let mut bytes = include_bytes!("../../../tests/fixtures/raster/precision-32-0.ktx2").to_vec();
    let dfd = u32::from_le_bytes(bytes[48..52].try_into().unwrap()) as usize;
    bytes[dfd + 13] = 0;
    std::fs::write(&source, bytes).unwrap();
    let oracle: Vec<u8> = [0.5_f32, 0.1, 0.3, 1.0, 2.0, 0.2, 0.4, 1.0]
        .into_iter().flat_map(f32::to_le_bytes).collect();
    qualify_source(gpu, source, &oracle, (2, 1));
}

#[test]
fn linear_cli_dpx_preload_swap_and_strict_cache_boundaries() {
    qualify_dpx(None);
}

#[test]
#[ignore = "requires actual Metal device"]
fn linear_cli_dpx_preload_swap_preserves_metal_sdr_and_hdr() {
    let gpu = navigation_metal_readback::qualification_gpu().expect("actual GPU required");
    eprintln!("linear DPX viewer adapter: {}", gpu.adapter_name());
    qualify_dpx(Some(&gpu));
}

fn qualify_dpx(gpu: Option<&navigation_metal_readback::GpuReadback>) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("normalized-linear.dpx");
    let mut bytes = include_bytes!("../../../tests/fixtures/raster/dpx-8-be-50-p0.dpx").to_vec();
    bytes[801] = 2;
    bytes[792..796].copy_from_slice(&255u32.to_be_bytes());
    bytes[796..800].copy_from_slice(&1.0_f32.to_bits().to_be_bytes());
    std::fs::write(&source, bytes).unwrap();
    let oracle: Vec<u8> = include_bytes!("../../../tests/fixtures/raster/dpx-8-be-50-p0.dpx.u8")
        .iter().flat_map(|v| ((f64::from(*v) / 255.0) as f32).to_le_bytes()).collect();
    qualify_source(gpu, source, &oracle, (5, 3));
}

fn qualify_source(
    gpu: Option<&navigation_metal_readback::GpuReadback>,
    source: PathBuf,
    oracle: &[u8],
    dimensions: (u32, u32),
) {
    let cli = Cli::try_parse_from([
        "rrrah",
        "--untagged-color",
        "linear-srgb",
        "--raster-swap-mb",
        "1",
        "--raster-swap-count",
        "1",
        "--raster-swap-queue-mb",
        "1",
        "--raster-swap-restore-mb",
        "1",
    ])
    .unwrap();
    assert!(Cli::try_parse_from(["rrrah", "--inspect", "--untagged-color", "linear-srgb"]).is_err());
    let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
    let directory = tempfile::tempdir().unwrap();
    let mut cache = raster_cache_with_swap(
        Some(directory.path()),
        false,
        rrrah_cache::CacheLimits {
            max_bytes: root.limit(),
            max_entries: Some(1),
            ttl: Some(Duration::ZERO),
        },
        cli.raster_swap_config(),
        Some(&root),
    );
    let mut request = DecodeRequest::new(source);
    request.memory_budget = Some(root.clone());
    cli.raster_interpretation().apply(&mut request);
    assert!(request.assume_untagged_linear_srgb && !request.assume_untagged_srgb);
    let (frame, assumed_srgb) =
        preload_raster(&request, None, &mut cache, &Arc::new(DecodeGate::new())).unwrap();
    assert!(!assumed_srgb);
    let expected: Vec<_> = oracle
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let check = |frame: &rrrah_core::DecodedRaster| {
        assert_eq!((frame.width(), frame.height()), dimensions);
        assert_eq!(frame.color_space(), &rrrah_core::RasterColorSpace::LinearSrgb);
        let rrrah_core::RasterPixels::Rgba32Float(values) = frame.pixels() else {
            panic!()
        };
        assert_eq!(values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), expected);
    };
    check(&frame);
    let view = rrrah_gpu::ViewParameters {
        viewport: [63., 65.],
        exposure_stops: 1.,
        ..Default::default()
    };
    let expected_frames = gpu.map(|gpu| {
        (
            gpu.render_raster(&frame, view, [63, 65]).pixels,
            gpu.render_raster_half(&frame, view, [63, 65]),
        )
    });
    drop(frame);
    assert_eq!(cache.spill_expired(), 1);
    cache.swap().unwrap().wait_idle().unwrap();
    assert_eq!(root.used(), 0);
    for color in [UntaggedColor::Strict, UntaggedColor::Srgb] {
        let mut incompatible = request.clone();
        color.apply(&mut incompatible);
        assert!(
            load_cached_raster_mode(&incompatible, None, &mut cache, false, || {
                load_raster_for_display_typed(&incompatible, None)
            })
            .is_err()
        );
        assert_eq!(root.used(), 0);
        assert_eq!(cache.swap().unwrap().stats().reads, 0);
    }
    let (restored, assumed_srgb) = load_cached_raster_mode(&request, None, &mut cache, false, || {
        panic!("linear CLI swap hit must not decode again")
    })
    .unwrap();
    assert!(!assumed_srgb);
    check(&restored);
    if let (Some(gpu), Some((sdr, hdr))) = (gpu, expected_frames) {
        assert_eq!(gpu.render_raster(&restored, view, [63, 65]).pixels, sdr);
        assert_eq!(gpu.render_raster_half(&restored, view, [63, 65]), hdr);
        eprintln!(
            "linear raster viewer: source={}; CLI/preload/swap bits exact; Metal SDR/HDR exact; incompatible policies refused",
            request.path.display()
        );
    }
    assert_eq!(cache.swap().unwrap().stats().reads, 1);
    assert_eq!(cache.swap().unwrap().stats().writes, 1);
    assert_eq!(cache.swap().unwrap().stats().errors, 0);
    drop(restored);
    drop(cache);
    assert_eq!(root.used(), 0);
}

#[test]
fn linear_cli_pfm_thumbnail_policy_and_last_owner_release() {
    let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
    let gate = Arc::new(DecodeGate::new());
    let job = || gallery::ThumbnailJob {
        index: 4,
        source: fixture(),
        edge: 2,
    };
    let token = || GenerationToken::new(Arc::new(AtomicU64::new(0)), 0);
    let strict = thumbnail_loader_with_cancel(gate.clone(), Some(root.clone()), UntaggedColor::Strict.into());
    assert!(strict(job(), token()).is_none());
    assert_eq!(root.used(), 0);
    let cli = Cli::try_parse_from(["rrrah", "--untagged-color", "linear-srgb"]).unwrap();
    let loader = thumbnail_loader_with_cancel(gate, Some(root.clone()), cli.raster_interpretation());
    let thumbnail = loader(job(), token()).unwrap();
    assert_eq!((thumbnail.width, thumbnail.height), (2, 2));
    assert_eq!(&thumbnail.pixels[..4], &[255, 0, 188, 255]);
    let retained = thumbnail.pixels.clone();
    drop(thumbnail);
    assert_eq!(root.used(), 16);
    drop(retained);
    assert_eq!(root.used(), 0);
}

#[test]
fn linear_thumbnail_worker_reversal_cancels_astc_and_publishes_only_latest_pfm() {
    let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
    let loader = thumbnail_loader_with_cancel(
        Arc::new(DecodeGate::new()), Some(root.clone()),
        Cli::try_parse_from(["rrrah", "--untagged-color", "linear-srgb"])
            .unwrap().raster_interpretation(),
    );
    let (started_tx, started_rx) = bounded(1);
    let (release_tx, release_rx) = bounded(1);
    let (observed_tx, observed_rx) = bounded(3);
    let worker = gallery::Prefetcher::new_with_cancel(1, move |job, token| {
        if job.index == 0 {
            started_tx.send(token.clone()).unwrap();
            release_rx.recv().unwrap();
        }
        let index = job.index;
        let ready = loader(job, token);
        observed_tx.send((index, ready.is_some())).unwrap();
        ready
    });
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let job = |index, name: &str| gallery::ThumbnailJob { index, source: base.join(name), edge: 2 };
    worker.submit([job(0, "hdr-compressed.astc")]);
    let old = started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    worker.submit([job(1, "precision-32-0.ktx2")]);
    worker.submit([job(2, "transport-linear.pfm")]);
    assert!(old.is_cancelled());
    assert_eq!(root.used(), 0);
    release_tx.send(()).unwrap();
    assert_eq!(observed_rx.recv_timeout(Duration::from_secs(5)).unwrap(), (0, false));
    assert_eq!(observed_rx.recv_timeout(Duration::from_secs(5)).unwrap(), (2, true));
    let deadline = Instant::now() + Duration::from_secs(5);
    let ready = loop {
        if let Some(ready) = worker.try_recv() { break ready; }
        assert!(Instant::now() < deadline, "latest thumbnail not published");
        thread::sleep(Duration::from_millis(1));
    };
    assert_eq!((ready.index, ready.width, ready.height), (2, 2, 2));
    assert_eq!(&ready.pixels[..4], &[255, 0, 188, 255]);
    assert!(worker.try_recv().is_none());
    assert!(observed_rx.try_recv().is_err());
    drop(ready);
    drop(worker);
    assert_eq!(root.used(), 0);
}

#[test]
fn pfm_top_down_cli_separates_ram_and_swap_rows() {
    qualify_pfm_rows(None);
}

#[test]
#[ignore = "requires actual Metal device"]
fn pfm_top_down_cli_swap_preserves_metal_sdr_hdr_rows() {
    let gpu = navigation_metal_readback::qualification_gpu().expect("actual GPU required");
    eprintln!("PFM top-down adapter: {}", gpu.adapter_name());
    qualify_pfm_rows(Some(&gpu));
}

fn qualify_pfm_rows(gpu: Option<&navigation_metal_readback::GpuReadback>) {
    let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
    let directory = tempfile::tempdir().unwrap();
    let cli = Cli::try_parse_from(["rrrah", "--untagged-color", "linear-srgb", "--pfm-top-down"]).unwrap();
    assert!(Cli::try_parse_from(["rrrah", "--inspect", "--pfm-top-down"]).is_err());
    let mut request = DecodeRequest::new(fixture());
    request.memory_budget = Some(root.clone());
    cli.raster_interpretation().apply(&mut request);
    assert!(request.pfm_top_down);
    let mut cache = raster_cache_with_swap(Some(directory.path()), false,
        rrrah_cache::CacheLimits { max_bytes: 1024 * 1024, max_entries: Some(1), ttl: None },
        Some(rrrah_cache::ImageSwapConfig { limits: rrrah_cache::CacheLimits::bytes(1024 * 1024),
            queue_bytes: 1024 * 1024, queue_count: 2, restore_bytes: 1024 * 1024 }), Some(&root));
    let gate = Arc::new(DecodeGate::new());
    let (top, _) = preload_raster(&request, None, &mut cache, &gate).unwrap();
    let rrrah_core::RasterPixels::Rgba32Float(p) = top.pixels() else { panic!() };
    assert_eq!(&p[..4], &[2.0, -1.0, 0.25, 1.0]);
    let view = rrrah_gpu::ViewParameters { viewport: [63., 65.], ..Default::default() };
    let expected_frames = gpu.map(|gpu| (gpu.render_raster(&top, view, [63, 65]).pixels,
        gpu.render_raster_half(&top, view, [63, 65])));
    drop(top);
    let mut bottom = request.clone();
    Cli::try_parse_from(["rrrah", "--untagged-color", "linear-srgb"]).unwrap()
        .raster_interpretation().apply(&mut bottom);
    assert!(!bottom.pfm_top_down);
    let (normal, _) = preload_raster(&bottom, None, &mut cache, &gate).unwrap();
    let rrrah_core::RasterPixels::Rgba32Float(p) = normal.pixels() else { panic!() };
    assert_eq!(&p[..4], &[1.0, 0.0, 0.5, 1.0]);
    drop(normal);
    cache.swap().unwrap().wait_idle().unwrap();
    let (restored, _) = load_cached_raster_mode(&request, None, &mut cache, false,
        || panic!("top-down swap hit must preserve its distinct row interpretation")).unwrap();
    let rrrah_core::RasterPixels::Rgba32Float(p) = restored.pixels() else { panic!() };
    assert_eq!(&p[..4], &[2.0, -1.0, 0.25, 1.0]);
    assert_eq!(cache.swap().unwrap().stats().reads, 1);
    if let (Some(gpu), Some((sdr, hdr))) = (gpu, expected_frames) {
        assert_eq!(gpu.render_raster(&restored, view, [63, 65]).pixels, sdr);
        assert_eq!(gpu.render_raster_half(&restored, view, [63, 65]), hdr);
        eprintln!("PFM top-down rows: CLI, distinct cache/swap keys, decoder-free restore, SDR/HDR exact");
    }
    drop(restored);
    cache.swap().unwrap().wait_idle().unwrap();
    drop(cache);
    assert_eq!(root.used(), 0);
}
