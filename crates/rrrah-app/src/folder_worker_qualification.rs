//! Real headless foreground worker plus optional actual Metal before cover completion.
use super::*;
use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
fn next_raster(
    app: &App,
) -> (
    u64,
    DecodedRaster,
    Option<rrrah_cache::CacheLease<DecodedRaster>>,
    Option<rrrah_swap::SwapStore>,
) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match app
            .receiver
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("real foreground worker did not publish")
        {
            LoadEvent::RasterReady {
                generation,
                raster,
                lease,
                region_store,
                ..
            } => return (generation, raster, lease, region_store),
            LoadEvent::Progress { .. } => {}
            LoadEvent::Failed { error, .. } => panic!("real foreground load failed: {error}"),
            _ => panic!("PNG did not reach raster publication"),
        }
    }
}
fn rendered(gpu: &navigation_metal_readback::GpuReadback, frame: &DecodedRaster) -> (Vec<u8>, Vec<u16>) {
    let view = ViewParameters {
        viewport: [63., 65.],
        ..Default::default()
    };
    (
        gpu.render_raster(frame, view, [63, 65]).pixels,
        gpu.render_raster_half(frame, view, [63, 65]),
    )
}
fn real_worker_app(cache_root: PathBuf, root: &MemoryBudget, raster_limits: rrrah_cache::CacheLimits) -> App {
    real_worker_app_with_window(
        cache_root,
        root,
        raster_limits,
        gallery::PrefetchWindow { behind: 0, ahead: 0 },
        LoaderWake::Headless,
    )
}
fn real_worker_app_with_window(
    cache_root: PathBuf,
    root: &MemoryBudget,
    raster_limits: rrrah_cache::CacheLimits,
    window: gallery::PrefetchWindow,
    wake: LoaderWake,
) -> App {
    real_worker_app_with_resources(
        cache_root,
        root,
        raster_limits,
        window,
        wake,
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits {
                max_bytes: 65536,
                max_entries: Some(2),
                ttl: None,
            },
            queue_bytes: 8192,
            queue_count: 1,
            restore_bytes: 8192,
        },
    )
}
fn real_worker_app_with_resources(
    cache_root: PathBuf,
    root: &MemoryBudget,
    raster_limits: rrrah_cache::CacheLimits,
    window: gallery::PrefetchWindow,
    wake: LoaderWake,
    swap: rrrah_cache::ImageSwapConfig,
) -> App {
    let (mut app, _fake_requests) =
        foreground_loader_tests::app_with_drop_inspector(drop_inspection::DropInspector::new());
    let (sender, receiver) = unbounded();
    let loader = ForegroundLoader::new(
        Some(cache_root),
        false,
        rrrah_cache::CacheLimits::bytes(0),
        sender,
        wake,
        Arc::clone(&app.foreground_loader.decode_gate),
        Arc::clone(&app.foreground_loader.telemetry),
        None,
        Some(swap),
        None,
        rrrah_cache::CacheLimits::bytes(0),
        0,
        Some(root.clone()),
        raster_limits,
        rrrah_cache::CacheLimits::bytes(0),
        window,
        UntaggedColor::Srgb.into(),
    )
    .unwrap();
    app.prefetch_window = window;
    app.receiver = receiver;
    app.foreground_loader = loader;
    app
}
fn qualify(gpu: Option<&navigation_metal_readback::GpuReadback>) {
    let workspace = tempfile::tempdir().unwrap();
    let folder = workspace.path().join("images");
    std::fs::create_dir(&folder).unwrap();
    let source = include_bytes!("../../../tests/fixtures/raster/pattern.png");
    for name in ["a.png", "b.png", "c.png"] {
        std::fs::write(folder.join(name), source).unwrap();
    }
    let root = MemoryBudget::new(1024 * 1024);
    let mut app = real_worker_app(
        workspace.path().join("cache"),
        &root,
        rrrah_cache::CacheLimits {
            max_bytes: 8192,
            max_entries: Some(1),
            ttl: None,
        },
    );
    let (started, observed) = bounded(1);
    let (release, gate) = bounded(1);
    struct Release(Sender<()>);
    impl Drop for Release {
        fn drop(&mut self) {
            let _ = self.0.try_send(());
        }
    }
    let _release_guard = Release(release.clone());
    app.folder_scanner = gallery::FolderScanner::with_tile_reader(move |folder, cancelled| {
        started.send(()).unwrap();
        loop {
            if cancelled() {
                return Vec::new();
            }
            match gate.recv_timeout(Duration::from_millis(1)) {
                Ok(()) => break,
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(_) => return Vec::new(),
            }
        }
        gallery::sibling_folder_tiles(folder)
    });
    app.open_folder(folder);
    observed.recv_timeout(Duration::from_secs(10)).unwrap();
    app.drain_folder_scan();
    let (first_generation, first, first_lease, first_store) = next_raster(&app);
    assert_eq!(first_generation, app.foreground_loader.current_generation());
    assert!(first_lease.is_some());
    assert!(first_store.is_some());
    assert_eq!((first.width(), first.height()), (16, 16));
    assert!(first.capacity_bytes() > 0);
    assert_eq!(
        root.used(),
        first.capacity_bytes(),
        "the real worker must account the one resident pixel allocation"
    );
    let oracle = include_bytes!("../../../tests/fixtures/raster/pattern.png.rgba");
    let linear = oracle
        .chunks_exact(4)
        .flat_map(|p| {
            [0, 1, 2, 3].map(|c| {
                let v = f64::from(p[c]) / 255.;
                (if c == 3 {
                    v
                } else if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }) as f32
            })
        })
        .collect::<Vec<_>>();
    let RasterPixels::Rgba32Float(first_pixels) = first.pixels() else {
        panic!("foreground frame not prepared")
    };
    assert_eq!(first_pixels.len(), linear.len());
    assert!(
        first_pixels
            .iter()
            .zip(&linear)
            .all(|(a, b)| (a - b).abs() <= 2e-7)
    );
    let golden = DecodedRaster::new(
        16,
        16,
        RasterPixels::Rgba32Float(Arc::new(linear).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let before = gpu.map(|gpu| {
        let actual = rendered(gpu, &first);
        let expected = rendered(gpu, &golden);
        assert!(actual.0.iter().zip(&expected.0).all(|(a, b)| a.abs_diff(*b) <= 1));
        assert!(actual.1.iter().zip(&expected.1).all(|(a, b)| a.abs_diff(*b) <= 1));
        actual
    });
    assert!(app.pending_folder_metadata.is_some());
    assert!(!app.folder_scanner.has_ready());
    let pressure = root.try_reserve(root.limit() - root.used()).unwrap();
    assert_eq!(root.used(), root.limit());
    app.open_gallery_index(0);
    let (generation, second, second_lease, second_store) = next_raster(&app);
    assert_ne!(generation, first_generation);
    assert_eq!(generation, app.foreground_loader.current_generation());
    assert!(second_lease.is_some());
    assert!(second_store.is_some());
    let RasterPixels::Rgba32Float(second_pixels) = second.pixels() else {
        panic!()
    };
    assert_eq!(
        first_pixels.as_ptr(),
        second_pixels.as_ptr(),
        "RAM hit allocated a new pixel buffer"
    );
    assert_eq!(&first_pixels[..], &second_pixels[..]);
    assert_eq!(root.used(), root.limit());
    if let (Some(gpu), Some(before)) = (gpu, before) {
        assert_eq!(rendered(gpu, &second), before);
    }
    assert!(app.pending_folder_metadata.is_some());
    assert!(!app.folder_scanner.has_ready());
    drop((
        second,
        second_lease,
        second_store,
        first,
        first_lease,
        first_store,
        pressure,
    ));
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !app.folder_scanner.has_ready() {
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    app.drain_folder_scan();
    assert!(app.pending_folder_metadata.is_none());
    drop(app);
    assert_eq!(root.used(), 0);
    eprintln!(
        "real foreground worker: native PNG/preparation/cache lease ready before blocked cover completion; full-root RAM reselect keeps same allocation{}; final=0",
        if gpu.is_some() {
            " and exact Metal SDR/HDR"
        } else {
            ""
        }
    );
}
#[test]
fn real_worker_ready_and_full_pressure_ram_hit_before_cover_metadata() {
    qualify(None);
}
#[test]
#[ignore = "manual required actual Metal real foreground worker before cover publication"]
fn real_worker_metal_frame_and_full_pressure_ram_hit_before_cover_metadata() {
    let gpu = navigation_metal_readback::qualification_gpu().expect("required Metal");
    eprintln!("foreground cover adapter: {}", gpu.adapter_name());
    qualify(Some(&gpu));
}

fn qualify_swap_navigation(gpu: Option<&navigation_metal_readback::GpuReadback>) {
    for (policy, limits) in [
        (
            "count",
            rrrah_cache::CacheLimits {
                max_bytes: 8192,
                max_entries: Some(1),
                ttl: None,
            },
        ),
        (
            "bytes",
            rrrah_cache::CacheLimits {
                max_bytes: 4096,
                max_entries: None,
                ttl: None,
            },
        ),
        (
            "ttl",
            rrrah_cache::CacheLimits {
                max_bytes: 8192,
                max_entries: Some(2),
                ttl: Some(Duration::from_millis(100)),
            },
        ),
    ] {
        let workspace = tempfile::tempdir().unwrap();
        let a = workspace.path().join("a.png");
        let b = workspace.path().join("b.bmp");
        std::fs::write(&a, include_bytes!("../../../tests/fixtures/raster/pattern.png")).unwrap();
        std::fs::write(&b, include_bytes!("../../../tests/fixtures/raster/pattern.bmp")).unwrap();
        let root = MemoryBudget::new(1024 * 1024);
        let mut app = real_worker_app(workspace.path().join("cache"), &root, limits);
        app.gallery = vec![a.clone(), b];
        app.open_gallery_index(0);
        let (_, first, lease, store) = next_raster(&app);
        let store = store.unwrap();
        assert!(lease.is_some());
        let weight = first.capacity_bytes();
        assert_eq!(weight, 4096);
        let bits = |frame: &DecodedRaster| {
            let RasterPixels::Rgba32Float(pixels) = frame.pixels() else {
                panic!("not prepared")
            };
            pixels.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        };
        let expected = bits(&first);
        let metadata = (
            first.width(),
            first.height(),
            first.color_space().clone(),
            first.alpha_association(),
            first.pixel_aspect(),
            first.sample_scale(),
            first.image_index(),
            first.image_count(),
        );
        let before = gpu.map(|g| rendered(g, &first));
        drop((first, lease));
        app.navigate_gallery(1);
        let (_, second, second_lease, second_store) = next_raster(&app);
        assert!(second_lease.is_some());
        assert_ne!(bits(&second), expected);
        let deadline = Instant::now() + Duration::from_secs(10);
        while store.usage().unwrap().objects != 1 || root.used() != second.capacity_bytes() {
            assert!(
                Instant::now() < deadline,
                "{policy}: eviction did not publish and release A: root={} objects={}",
                root.used(),
                store.usage().unwrap().objects
            );
            thread::sleep(Duration::from_millis(1));
        }
        let pressure = root.try_reserve(root.limit() - root.used() - weight).unwrap();
        let baseline = root.used();
        let mut request = DecodeRequest::new(a);
        request.memory_budget = Some(root.clone());
        let failure = load_raster_for_display_typed(&request, None)
            .expect_err("native control unexpectedly fits exact restore quota");
        assert!(failure.retryable_memory_pressure(), "{policy}: {failure}");
        assert_eq!(root.used(), baseline);
        app.navigate_gallery(-1);
        let (generation, restored, restored_lease, restored_store) = next_raster(&app);
        assert_eq!(generation, app.foreground_loader.current_generation());
        assert_eq!(bits(&restored), expected, "{policy}");
        assert_eq!(
            (
                restored.width(),
                restored.height(),
                restored.color_space().clone(),
                restored.alpha_association(),
                restored.pixel_aspect(),
                restored.sample_scale(),
                restored.image_index(),
                restored.image_count()
            ),
            metadata
        );
        assert_eq!(root.used(), root.limit());
        assert_eq!(restored_lease.is_some(), policy == "ttl");
        if let (Some(g), Some(before)) = (gpu, before) {
            assert_eq!(rendered(g, &restored), before, "{policy}");
        }
        drop((
            restored,
            restored_lease,
            restored_store,
            second,
            second_lease,
            second_store,
            store,
            pressure,
        ));
        drop(app);
        assert_eq!(root.used(), 0);
        eprintln!(
            "real foreground A-B-A {policy}: indexed spill published; native control refused exact remaining quota; restore pixels/metadata{} exact; final=0",
            if gpu.is_some() { "/Metal SDR/HDR" } else { "" }
        );
    }
}
#[test]
fn real_worker_swap_navigation_count_bytes_ttl_under_pressure() {
    qualify_swap_navigation(None);
}
#[test]
#[ignore = "manual actual Metal swap navigation"]
fn real_worker_metal_swap_navigation_count_bytes_ttl_under_pressure() {
    let gpu = navigation_metal_readback::qualification_gpu().expect("required Metal");
    eprintln!("swap navigation adapter: {}", gpu.adapter_name());
    qualify_swap_navigation(Some(&gpu));
}

fn qualify_repeated_navigation(gpu: Option<&navigation_metal_readback::GpuReadback>) {
    let workspace = tempfile::tempdir().unwrap();
    let root = MemoryBudget::new(16384);
    let mut app = real_worker_app(
        workspace.path().join("cache"),
        &root,
        rrrah_cache::CacheLimits {
            max_bytes: 4096,
            max_entries: Some(1),
            ttl: None,
        },
    );
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let mut expected = Vec::new();
    let bits = |frame: &DecodedRaster| {
        let RasterPixels::Rgba32Float(pixels) = frame.pixels() else {
            panic!("not prepared")
        };
        pixels.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
    };
    for name in ["pattern.png", "pattern.bmp", "pattern.webp", "pattern.tif"] {
        let path = workspace.path().join(name);
        std::fs::copy(fixtures.join(name), &path).unwrap();
        let mut request = DecodeRequest::new(&path);
        request.assume_untagged_srgb = true;
        let (frame, _) = load_raster_for_display_typed(&request, None).unwrap();
        expected.push((bits(&frame), gpu.map(|g| rendered(g, &frame))));
        app.gallery.push(path);
    }
    // Alternate sweep directions: exercise both recent swap hits and older
    // objects displaced by the two-object disk limit.
    for step in 0..512 {
        let index = [0, 1, 2, 3, 2, 1][step % 6];
        app.open_gallery_index(index);
        let (generation, frame, lease, store) = next_raster(&app);
        assert_eq!(generation, app.foreground_loader.current_generation());
        assert!(lease.is_some());
        assert_eq!(bits(&frame), expected[index].0, "step {step}");
        if let (Some(g), Some(golden)) = (gpu, &expected[index].1) {
            assert_eq!(&rendered(g, &frame), golden, "step {step}");
        }
        let store = store.unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while root.used() != frame.capacity_bytes() {
            assert!(
                Instant::now() < deadline,
                "spill allocation retained at {step}: {}",
                root.used()
            );
            thread::sleep(Duration::from_millis(1));
        }
        let usage = store.usage().unwrap();
        assert!(usage.objects <= 2, "disk count at {step}");
        assert!(usage.bytes <= 65536, "disk size at {step}");
        assert_eq!(root.used(), 4096);
        drop((frame, lease, store));
    }
    drop(app);
    assert_eq!(root.used(), 0);
    eprintln!(
        "real worker 512 mixed-codec directional selections: exact pixels{}, RAM4096/root16384, swap <=2 objects/65536 bytes, final=0",
        if gpu.is_some() { " and Metal SDR/HDR" } else { "" }
    );
}
#[test]
fn real_worker_repeated_navigation_has_bounded_memory_and_swap() {
    qualify_repeated_navigation(None);
}
#[test]
#[ignore = "manual actual Metal repeated navigation"]
fn real_worker_metal_repeated_navigation_has_bounded_memory_and_swap() {
    let gpu = navigation_metal_readback::qualification_gpu().expect("required Metal");
    eprintln!("repeated navigation adapter: {}", gpu.adapter_name());
    qualify_repeated_navigation(Some(&gpu));
}

fn qualify_real_prefetch(gpu: Option<&navigation_metal_readback::GpuReadback>) {
    for (direction, behind, ahead, indices) in [
        (gallery::NavDirection::Forward, 1, 2, vec![4, 5, 2]),
        (gallery::NavDirection::Backward, 1, 2, vec![2, 1, 4]),
        (gallery::NavDirection::Forward, 2, 0, vec![2, 1]),
        (gallery::NavDirection::Backward, 0, 2, vec![2, 1]),
        (gallery::NavDirection::Forward, 0, 0, vec![]),
    ] {
        let workspace = tempfile::tempdir().unwrap();
        let root = MemoryBudget::new(65536);
        let (tx, rx) = unbounded();
        let window = gallery::PrefetchWindow { behind, ahead };
        let mut app = real_worker_app_with_window(
            workspace.path().join("cache"),
            &root,
            rrrah_cache::CacheLimits {
                max_bytes: 32768,
                max_entries: Some(8),
                ttl: None,
            },
            window,
            LoaderWake::ObservePrefetch(tx),
        );
        for index in 0..7 {
            let path = workspace.path().join(format!("{index}.png"));
            std::fs::write(
                &path,
                include_bytes!("../../../tests/fixtures/raster/pattern.png"),
            )
            .unwrap();
            app.gallery.push(path);
        }
        app.last_nav_direction = direction;
        app.open_gallery_index(3);
        let (generation, frame, lease, store) = next_raster(&app);
        let expected = gpu.map(|g| rendered(g, &frame));
        for index in &indices {
            let (observed, path, retained) = rx
                .recv_timeout(Duration::from_secs(10))
                .expect("real neighbour did not complete");
            assert_eq!(observed, generation);
            assert_eq!(path, app.gallery[*index]);
            assert!(retained, "planned neighbour not retained");
        }
        assert!(
            rx.recv_timeout(Duration::from_millis(350)).is_err(),
            "unplanned neighbour"
        );
        assert_eq!(root.used(), 4096 * (1 + indices.len()) as u64);
        let pressure = root.try_reserve(root.limit() - root.used()).unwrap();
        for index in &indices {
            app.open_gallery_index(*index);
            let (current, hit, hit_lease, hit_store) = next_raster(&app);
            assert_eq!(current, app.foreground_loader.current_generation());
            assert!(hit_lease.is_some());
            let (RasterPixels::Rgba32Float(a), RasterPixels::Rgba32Float(b)) = (hit.pixels(), frame.pixels())
            else {
                panic!()
            };
            assert_eq!(&a[..], &b[..]);
            if let (Some(g), Some(expected)) = (gpu, &expected) {
                assert_eq!(&rendered(g, &hit), expected);
            }
            assert_eq!(root.used(), root.limit());
            drop((hit, hit_lease, hit_store));
        }
        drop((frame, lease, store, pressure));
        drop(app);
        assert_eq!(root.used(), 0);
        eprintln!(
            "real prefetch {direction:?} behind={behind} ahead={ahead}: exact completion order {indices:?}, full-root neighbour hits{}, final=0",
            if gpu.is_some() { "/Metal SDR/HDR" } else { "" }
        );
    }
}
#[test]
fn real_worker_prefetch_window_order_and_full_pressure_hits() {
    qualify_real_prefetch(None);
}
#[test]
#[ignore = "manual actual Metal real prefetch windows"]
fn real_worker_metal_prefetch_window_order_and_full_pressure_hits() {
    let gpu = navigation_metal_readback::qualification_gpu().expect("required Metal");
    eprintln!("real prefetch adapter: {}", gpu.adapter_name());
    qualify_real_prefetch(Some(&gpu));
}

fn qualify_prefetch_reversal(gpu: Option<&navigation_metal_readback::GpuReadback>) {
    let workspace = tempfile::tempdir().unwrap();
    let root = MemoryBudget::new(65536);
    let (started, starts) = unbounded();
    let (release, proceed) = unbounded();
    let (completed, completions) = unbounded();
    // Disconnect on unwind so a failing assertion cannot strand the loader join.
    let wake = LoaderWake::GatePrefetch {
        started,
        proceed,
        completed,
    };
    let mut app = real_worker_app_with_window(
        workspace.path().join("cache"),
        &root,
        rrrah_cache::CacheLimits {
            max_bytes: 32768,
            max_entries: Some(8),
            ttl: None,
        },
        gallery::PrefetchWindow { behind: 1, ahead: 2 },
        wake,
    );
    struct ReleaseOnDrop(Option<Sender<()>>);
    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            self.0.take();
        }
    }
    let release = ReleaseOnDrop(Some(release));
    for index in 0..7 {
        let extension = if index == 2 { "bmp" } else { "png" };
        let path = workspace.path().join(format!("{index}.{extension}"));
        let source: &[u8] = if index == 2 {
            include_bytes!("../../../tests/fixtures/raster/pattern.bmp")
        } else {
            include_bytes!("../../../tests/fixtures/raster/pattern.png")
        };
        std::fs::write(&path, source).unwrap();
        app.gallery.push(path);
    }
    let (reference, _) = load_raster_for_display_typed(&DecodeRequest::new(&app.gallery[2]), None).unwrap();
    let RasterPixels::Rgba32Float(reference_pixels) = reference.pixels() else {
        panic!()
    };
    let expected_pixels = reference_pixels.to_vec();
    let golden = gpu.map(|g| rendered(g, &reference));
    drop(reference);
    app.last_nav_direction = gallery::NavDirection::Forward;
    app.open_gallery_index(3);
    let (old, first, first_lease, first_store) = next_raster(&app);
    let RasterPixels::Rgba32Float(first_pixels) = first.pixels() else {
        panic!()
    };
    assert!(
        first_pixels[..] != expected_pixels[..],
        "selected and old control must differ"
    );
    let (observed, path) = starts.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(observed, old);
    assert_eq!(path, app.gallery[4]);
    let gate = Arc::clone(&app.foreground_loader.decode_gate);
    let ticket = gate.request_foreground();
    let permit = ticket.acquire_decode(|| false).unwrap();
    release.0.as_ref().unwrap().send(()).unwrap();
    assert!(completions.recv_timeout(Duration::from_millis(100)).is_err());
    assert_eq!(root.used(), 4096, "blocked prefetch allocated pixels");
    app.navigate_gallery(-1);
    let new = app.foreground_loader.current_generation();
    assert_ne!(new, old);
    let (cancelled, path, retained) = completions.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(cancelled, old);
    assert_eq!(path, app.gallery[4]);
    assert!(!retained);
    assert_eq!(root.used(), 4096, "cancelled prefetch retained pixels");
    drop((permit, ticket));
    let (observed, selected, selected_lease, selected_store) = next_raster(&app);
    assert_eq!(observed, new, "stale foreground publication");
    assert_eq!(app.gallery_index, Some(2));
    assert!(selected_lease.is_some());
    let RasterPixels::Rgba32Float(selected_pixels) = selected.pixels() else {
        panic!()
    };
    assert_eq!(&selected_pixels[..], &expected_pixels[..]);
    if let (Some(g), Some(golden)) = (gpu, &golden) {
        assert_eq!(&rendered(g, &selected), golden);
    }
    for index in [1, 0, 3] {
        let (generation, path) = starts.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(generation, new);
        assert_eq!(path, app.gallery[index], "reversal kept stale forward plan");
        release.0.as_ref().unwrap().send(()).unwrap();
        let (generation, path, retained) = completions.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(generation, new);
        assert_eq!(path, app.gallery[index]);
        assert!(retained);
    }
    assert!(starts.recv_timeout(Duration::from_millis(350)).is_err());
    assert_eq!(root.used(), 4 * 4096);
    drop((
        first,
        first_lease,
        first_store,
        selected,
        selected_lease,
        selected_store,
    ));
    drop(release);
    drop(app);
    assert_eq!(root.used(), 0);
    eprintln!(
        "real worker reversal: old index4 blocked at decode gate, cancelled without allocation; new selected2 precedes backward [1,0,3]{}; final=0",
        if gpu.is_some() {
            "; Metal SDR/HDR exact"
        } else {
            ""
        }
    );
}
#[test]
fn real_worker_direction_reversal_cancels_blocked_prefetch() {
    qualify_prefetch_reversal(None);
}
#[test]
#[ignore = "manual actual Metal direction reversal"]
fn real_worker_metal_direction_reversal_cancels_blocked_prefetch() {
    let gpu = navigation_metal_readback::qualification_gpu().expect("required Metal");
    eprintln!("reversal adapter: {}", gpu.adapter_name());
    qualify_prefetch_reversal(Some(&gpu));
}

#[test]
#[ignore = "manual actual worker 60s large-frame collection soak; optional Metal via RRRAH_SOAK_METAL=1"]
fn real_worker_large_collection_pressure_soak() {
    let injected = std::env::var_os("RRRAH_QUALIFY_SWAP_DELAY_MS").is_some()
        || std::env::var_os("RRRAH_QUALIFY_SWAP_CHUNK_BYTES").is_some();
    assert!(
        !injected || cfg!(feature = "qualification-slow-io"),
        "requested slow I/O requires the qualification-slow-io build feature"
    );
    #[cfg(feature = "qualification-slow-io")]
    if let Some(config) = rrrah_cache::qualification_io::Config::from_environment().unwrap() {
        eprintln!(
            "large soak injected swap codec I/O: delay_ms={} chunk_bytes={}",
            config.delay.as_millis(),
            config.chunk_bytes
        );
    }
    let duration = std::env::var("RRRAH_SOAK_SECONDS")
        .ok()
        .map(|v| v.parse::<u64>().unwrap())
        .unwrap_or(60);
    assert!(duration >= 60, "soak must run at least60 seconds");
    let gpu = if std::env::var("RRRAH_SOAK_METAL").as_deref() == Ok("1") {
        Some(navigation_metal_readback::qualification_gpu().expect("required Metal"))
    } else {
        None
    };
    if let Some(g) = &gpu {
        eprintln!("large soak adapter: {}", g.adapter_name());
    }
    let workspace = tempfile::tempdir().unwrap();
    let root = MemoryBudget::new(128 * 1024 * 1024);
    let pressure = root.try_reserve(64 * 1024 * 1024).unwrap();
    let prefetch = std::env::var("RRRAH_SOAK_PREFETCH").as_deref() == Ok("1");
    let dynamic_window = std::env::var("RRRAH_SOAK_DYNAMIC_WINDOW").as_deref() == Ok("1");
    assert!(
        !dynamic_window || prefetch,
        "dynamic windows require observed prefetch"
    );
    let (observed_tx, observed_rx) = unbounded();
    let window = if prefetch {
        gallery::PrefetchWindow { behind: 1, ahead: 2 }
    } else {
        gallery::PrefetchWindow { behind: 0, ahead: 0 }
    };
    let wake = if prefetch {
        LoaderWake::ObservePrefetch(observed_tx)
    } else {
        LoaderWake::Headless
    };
    let mut app = real_worker_app_with_resources(
        workspace.path().join("cache"),
        &root,
        rrrah_cache::CacheLimits {
            max_bytes: 32 * 1024 * 1024,
            max_entries: Some(2),
            ttl: Some(Duration::from_millis(250)),
        },
        window,
        wake,
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits {
                max_bytes: 64 * 1024 * 1024,
                max_entries: Some(3),
                ttl: None,
            },
            queue_bytes: 16 * 1024 * 1024,
            queue_count: 1,
            restore_bytes: if prefetch {
                32 * 1024 * 1024
            } else {
                16 * 1024 * 1024
            },
        },
    );
    let digest = |frame: &DecodedRaster| {
        let RasterPixels::Rgba32Float(p) = frame.pixels() else {
            panic!("not prepared")
        };
        blake3::hash(bytemuck::cast_slice(&p[..]))
    };
    let heterogeneous = std::env::var("RRRAH_SOAK_MIXED").as_deref() == Ok("1");
    let mut references = Vec::new();
    for index in 0..64usize {
        let extension = if heterogeneous {
            ["ppm", "tga", "bmp"][index % 3]
        } else {
            "ppm"
        };
        let path = workspace.path().join(format!("{index:03}.{extension}"));
        let mut rgb = Vec::with_capacity(1024 * 1024 * 3);
        for y in 0..1024usize {
            for x in 0..1024usize {
                rgb.extend_from_slice(&[
                    (x.wrapping_add(index * 17) % 256) as u8,
                    (y.wrapping_add(index * 29) % 256) as u8,
                    ((x ^ y).wrapping_add(index * 41) % 256) as u8,
                ]);
            }
        }
        let bytes = encode_soak_rgb(extension, &rgb, 1024, 1024);
        std::fs::write(&path, bytes).unwrap();
        let mut request = DecodeRequest::new(&path);
        request.assume_untagged_srgb = true;
        request.memory_budget = Some(root.clone());
        let (frame, _) = load_raster_for_display_typed(&request, None).unwrap();
        assert_eq!(frame.capacity_bytes(), 16 * 1024 * 1024);
        references.push((digest(&frame), gpu.as_ref().map(|g| rendered(g, &frame))));
        app.gallery.push(path);
    }
    assert_eq!(root.used(), 64 * 1024 * 1024);
    let started = Instant::now();
    let mut selections = 0usize;
    let mut largest_settled = 0;
    let mut previous = 0;
    let mut retained_neighbours = 0;
    let mut refused_neighbours = 0;
    let mut selections_with_neighbours = 0;
    let windows = [
        (0, 0),
        (3, 0),
        (0, 3),
        (1, 2),
        (2, 5),
        (0, usize::MAX),
        (usize::MAX, usize::MAX),
    ];
    let mut window_selections = [0usize; 7];
    while selections < 256 || started.elapsed() < Duration::from_secs(duration) {
        // Adjacent forward/back steps plus long jumps exceed both cache tiers.
        let index = match selections % 4 {
            0 => (selections / 4) % 64,
            1 => (selections / 4 + 1) % 64,
            2 => (selections / 4) % 64,
            _ => (selections / 4 + 31) % 64,
        };
        app.last_nav_direction = if index < previous {
            gallery::NavDirection::Backward
        } else {
            gallery::NavDirection::Forward
        };
        previous = index;
        let window = if dynamic_window {
            let slot = (selections / 16) % windows.len();
            window_selections[slot] += 1;
            let (behind, ahead) = windows[slot];
            gallery::PrefetchWindow { behind, ahead }
        } else {
            window
        };
        app.prefetch_window = window;
        // Independent gallery scan: signed distance in the travel direction,
        // then travel-side neighbors before the opposite side. No planner call.
        let mut candidates = Vec::new();
        for candidate in 0..app.gallery.len() {
            let delta = candidate as isize - index as isize;
            let travel = if app.last_nav_direction == gallery::NavDirection::Forward {
                delta
            } else {
                -delta
            };
            let distance = travel.unsigned_abs();
            if travel > 0 && distance <= window.ahead {
                candidates.push((0, distance, candidate));
            } else if travel < 0 && distance <= window.behind {
                candidates.push((1, distance, candidate));
            }
        }
        candidates.sort_unstable();
        let planned: Vec<_> = candidates
            .into_iter()
            .map(|(_, _, candidate)| app.gallery[candidate].clone())
            .collect();
        selections_with_neighbours += usize::from(!planned.is_empty());
        app.open_gallery_index(index);
        let (generation, frame, lease, store) = next_raster(&app);
        assert_eq!(generation, app.foreground_loader.current_generation());
        assert!(lease.is_some());
        assert_eq!((frame.width(), frame.height()), (1024, 1024));
        assert_eq!(digest(&frame), references[index].0, "selection {selections}");
        if let (Some(g), Some(expected)) = (&gpu, &references[index].1) {
            assert_eq!(&rendered(g, &frame), expected);
        }
        if prefetch {
            for (rank, path) in planned.iter().enumerate() {
                let (observed, actual, retained) = observed_rx
                    .recv_timeout(Duration::from_secs(10))
                    .expect("large-frame prefetch did not complete");
                assert_eq!(observed, generation, "stale completion");
                assert_eq!(&actual, path, "large-frame prefetch priority");
                assert_eq!(
                    retained,
                    rank == 0,
                    "selection={selections} rank={rank} path={} root={}: nearest must fit; farther slots protected",
                    path.display(),
                    root.used()
                );
                if retained {
                    retained_neighbours += 1;
                } else {
                    refused_neighbours += 1;
                }
            }
            assert!(observed_rx.try_recv().is_err());
        }
        let store = store.unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while root.used() > 96 * 1024 * 1024 {
            assert!(Instant::now() < deadline, "spill allocations failed to settle");
            thread::sleep(Duration::from_millis(1));
        }
        largest_settled = largest_settled.max(root.used());
        let usage = store.usage().unwrap();
        assert!(usage.objects <= 3);
        assert!(usage.bytes <= 64 * 1024 * 1024);
        drop((frame, lease, store));
        selections += 1;
        if selections % 64 == 0 {
            eprintln!(
                "large soak selections={selections} seconds={:.1} settled={} root_peak={}",
                started.elapsed().as_secs_f64(),
                root.used(),
                root.peak()
            );
        }
    }
    let elapsed = started.elapsed();
    if prefetch {
        assert_eq!(retained_neighbours, selections_with_neighbours);
        assert!(refused_neighbours > 0);
    }
    if dynamic_window {
        assert!(window_selections.iter().all(|&count| count >= 16));
        eprintln!("dynamic prefetch windows={windows:?} selections={window_selections:?}");
    }
    drop(app);
    assert_eq!(root.used(), 64 * 1024 * 1024);
    drop(pressure);
    assert_eq!(root.used(), 0);
    eprintln!(
        "large real worker soak PASS: sources=64 dimensions=1024x1024 frame_bytes=16777216 selections={selections} seconds={:.3} fixed_pressure=67108864 root_limit={} root_peak={} largest_settled={largest_settled} swap_objects<=3 swap_bytes<=67108864 prefetch={prefetch} dynamic_window={dynamic_window} mixed_ppm_tga_bmp={heterogeneous} nearest_retained={retained_neighbours} farther_refused={refused_neighbours}{} final=0",
        elapsed.as_secs_f64(),
        root.limit(),
        root.peak(),
        if gpu.is_some() {
            " exact Metal SDR/HDR every selection"
        } else {
            ""
        }
    );
}

// Deliberately authored uncompressed files: no production encoder is used as an oracle.
fn encode_soak_rgb(extension: &str, rgb: &[u8], width: u32, height: u32) -> Vec<u8> {
    assert_eq!(rgb.len(), width as usize * height as usize * 3);
    match extension {
        "ppm" => {
            let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
            bytes.extend_from_slice(rgb);
            bytes
        }
        "tga" => {
            let mut bytes = vec![0; 18];
            bytes[2] = 2; // Uncompressed true color, top-left origin.
            bytes[12..14].copy_from_slice(&u16::try_from(width).unwrap().to_le_bytes());
            bytes[14..16].copy_from_slice(&u16::try_from(height).unwrap().to_le_bytes());
            bytes[16] = 24;
            bytes[17] = 0x20;
            for pixel in rgb.chunks_exact(3) {
                bytes.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
            }
            bytes
        }
        "bmp" => {
            let stride = (width as usize * 3 + 3) & !3;
            let payload = stride * height as usize;
            let mut bytes = vec![0; 54 + payload];
            bytes[..2].copy_from_slice(b"BM");
            bytes[2..6].copy_from_slice(&u32::try_from(54 + payload).unwrap().to_le_bytes());
            bytes[10..14].copy_from_slice(&54u32.to_le_bytes());
            bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
            bytes[18..22].copy_from_slice(&i32::try_from(width).unwrap().to_le_bytes());
            bytes[22..26].copy_from_slice(&(-i32::try_from(height).unwrap()).to_le_bytes());
            bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
            bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
            bytes[34..38].copy_from_slice(&u32::try_from(payload).unwrap().to_le_bytes());
            for (y, row) in rgb.chunks_exact(width as usize * 3).enumerate() {
                for (x, pixel) in row.chunks_exact(3).enumerate() {
                    bytes[54 + y * stride + x * 3..54 + y * stride + x * 3 + 3]
                        .copy_from_slice(&[pixel[2], pixel[1], pixel[0]]);
                }
            }
            bytes
        }
        _ => panic!("unsupported authored soak format"),
    }
}
