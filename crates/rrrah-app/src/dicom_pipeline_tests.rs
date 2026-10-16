use super::*;
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};

struct Case {
    request: DecodeRequest,
    window: Option<rrrah_decode::ScalarWindow>,
    assumed: bool,
    frames: usize,
    oracle: DecodedRaster,
    payload: Vec<u8>,
    sdr: Option<Vec<u8>>,
    hdr: Option<Vec<u16>>,
}
fn same_pixels(frame: &DecodedRaster, oracle: &DecodedRaster) {
    let (RasterPixels::Rgba32Float(a), RasterPixels::Rgba32Float(b)) = (frame.pixels(), oracle.pixels())
    else {
        panic!("float precision lost")
    };
    assert_eq!(
        a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        b.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
    );
    assert_eq!(frame.pixel_aspect(), oracle.pixel_aspect());
    assert_eq!(frame.color_space(), &RasterColorSpace::LinearSrgb);
    assert_eq!((frame.width(), frame.height()), (oracle.width(), oracle.height()));
}
fn view(frame: &DecodedRaster) -> (rrrah_gpu::ViewParameters, [u32; 2]) {
    let large = frame.width() == 256 && frame.height() == 256;
    let viewport = if large { [512, 512] } else { [64, 32] };
    let params = rrrah_gpu::ViewParameters {
        viewport: viewport.map(|v| v as f32),
        zoom: if large { 2. } else { 8. },
        ..Default::default()
    };
    (params, viewport)
}
// Arm only around CPU prefetch. GPU readback latency is outside this gate check.
fn prefetch_watchdog(
    generation: Arc<AtomicU64>,
    timeout: Duration,
) -> (std::sync::mpsc::Sender<bool>, std::thread::JoinHandle<()>) {
    let (sender, receiver) = std::sync::mpsc::channel::<bool>();
    let worker = thread::spawn(move || {
        while let Ok(armed) = receiver.recv() {
            if !armed {
                continue;
            }
            match receiver.recv_timeout(timeout) {
                Ok(false) => {}
                Ok(true) => panic!("overlapping watchdog intervals"),
                Err(_) => {
                    generation.store(2, Ordering::Release);
                    return;
                }
            }
        }
    });
    (sender, worker)
}
#[test]
fn prefetch_watchdog_excludes_gpu_idle_but_cancels_a_blocked_call() {
    let generation = Arc::new(AtomicU64::new(1));
    let (sender, worker) = prefetch_watchdog(generation.clone(), Duration::from_millis(5));
    sender.send(false).unwrap();
    thread::sleep(Duration::from_millis(30));
    assert_eq!(
        generation.load(Ordering::Acquire),
        1,
        "unarmed GPU time cannot cancel prefetch"
    );
    sender.send(true).unwrap();
    worker.join().unwrap();
    assert_eq!(
        generation.load(Ordering::Acquire),
        2,
        "armed blocked load must cancel"
    );
}
fn qualify(gpu: Option<&crate::navigation_metal_readback::GpuReadback>) {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let dir = tempfile::tempdir().unwrap();
    let root = rrrah_cache::MemoryBudget::new(8388608);
    let mut cache = raster_cache_with_swap(
        Some(dir.path()),
        false,
        rrrah_cache::CacheLimits {
            max_bytes: 2097152,
            max_entries: Some(1),
            ttl: Some(Duration::from_millis(50)),
        },
        Some(rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits {
                max_bytes: 8388608,
                max_entries: Some(384),
                ttl: None,
            },
            queue_bytes: 2097152,
            queue_count: 4,
            restore_bytes: 2097152,
        }),
        Some(&root),
    );
    let mut inputs = Vec::new();
    for manifest in [
        "dicom-manifest.tsv",
        "dicom-geometry-scalar-manifest.tsv",
        "dicom-rle-scalar-manifest.tsv",
    ] {
        for line in std::fs::read_to_string(fixtures.join(manifest))
            .unwrap()
            .lines()
            .skip(1)
        {
            let fields: Vec<_> = line.split('\t').collect();
            let name = fields[0];
            if !(name.starts_with("dicom-big-endian-")
                || name.starts_with("dicom-implicit-")
                || name.contains("-float")
                || name.contains("-sequence-")
                || name.contains("-geometry-")
                || name.starts_with("dicom-rle-"))
            {
                continue;
            }
            let width = fields[1].parse::<u32>().unwrap();
            let height = fields[2].parse::<u32>().unwrap();
            let frames = fields[3].parse::<usize>().unwrap();
            let lo = fields[4].parse::<f32>().unwrap();
            let hi = fields[5].parse::<f32>().unwrap();
            for index in 0..frames {
                for alternate in [false, true] {
                    let window = if alternate {
                        rrrah_decode::ScalarWindow::new(lo + (hi - lo) / 4., lo + 3. * (hi - lo) / 4.)
                            .unwrap()
                    } else {
                        rrrah_decode::ScalarWindow::new(lo, hi).unwrap()
                    };
                    let bytes = std::fs::read(fixtures.join(format!(
                        "{name}.{index}.{}-f32",
                        if alternate { "alternate-window" } else { "window" }
                    )))
                    .unwrap();
                    let values: Vec<f32> = bytes
                        .chunks_exact(4)
                        .map(|b| f32::from_bits(u32::from_le_bytes(b.try_into().unwrap())))
                        .collect();
                    let oracle = DecodedRaster::new(
                        width,
                        height,
                        RasterPixels::Rgba32Float(Arc::new(values).into()),
                        RasterColorSpace::LinearSrgb,
                    )
                    .unwrap();
                    let oracle = oracle
                        .with_pixel_aspect(
                            fields
                                .get(6)
                                .filter(|v| **v != "-")
                                .map(|v| v.parse::<f32>().unwrap()),
                        )
                        .unwrap();
                    let mut request = DecodeRequest::new(fixtures.join(name));
                    request.image_index = index;
                    request.memory_budget = Some(root.clone());
                    inputs.push((request, Some(window), frames, oracle, false));
                }
            }
        }
    }
    for manifest in [
        "dicom-rgb-manifest.tsv",
        "dicom-ybr-manifest.tsv",
        "dicom-geometry-color-manifest.tsv",
        "dicom-palette-manifest.tsv",
        "dicom-segmented-manifest.tsv",
        "dicom-rle-color-manifest.tsv",
    ] {
        for line in std::fs::read_to_string(fixtures.join(manifest))
            .unwrap()
            .lines()
            .skip(1)
        {
            let f: Vec<_> = line.split('\t').collect();
            let width = f[1].parse::<u32>().unwrap();
            let height = f[2].parse::<u32>().unwrap();
            let frames = f[3].parse::<usize>().unwrap();
            let bits = f[4].parse::<u16>().unwrap();
            let assumed = f[5] == "0" && f[6] == "-";
            for index in 0..frames {
                let bytes = std::fs::read(fixtures.join(format!("{}.{}.rgba{}", f[0], index, bits))).unwrap();
                let pixels = if bits == 8 {
                    RasterPixels::Rgba8(Arc::new(bytes).into())
                } else {
                    RasterPixels::Rgba16(
                        Arc::new(
                            bytes
                                .chunks_exact(2)
                                .map(|v| u16::from_le_bytes(v.try_into().unwrap()))
                                .collect::<Vec<_>>(),
                        )
                        .into(),
                    )
                };
                let color = if f[5] != "0" {
                    RasterColorSpace::Icc(std::fs::read(fixtures.join("dicom-rgb-profile.icc")).unwrap())
                } else if assumed {
                    RasterColorSpace::AssumedSrgb
                } else {
                    RasterColorSpace::Srgb
                };
                let raw = DecodedRaster::new(width, height, pixels, color)
                    .unwrap()
                    .with_pixel_aspect(f.get(7).filter(|v| **v != "-").map(|v| v.parse::<f32>().unwrap()))
                    .unwrap();
                let oracle = rrrah_decode::prepare_raster_for_display_with_budget(&raw, None).unwrap();
                let mut request = DecodeRequest::new(fixtures.join(f[0]));
                request.image_index = index;
                request.memory_budget = Some(root.clone());
                request.assume_untagged_srgb = assumed;
                inputs.push((request, None, frames, oracle, assumed));
            }
        }
    }
    let mut shape_controls = 0;
    let mut cases = Vec::new();
    let mut full_palette_frames = 0;
    for (request, window, frames, oracle, expected_assumed) in inputs {
        let (params, viewport) = view(&oracle);
        if viewport == [512, 512] {
            full_palette_frames += 1;
        }
        let (frame, assumed) = load_cached_raster_for_display(&request, window, &mut cache).unwrap();
        assert_eq!(assumed, expected_assumed);
        same_pixels(&frame, &oracle);
        assert_eq!(
            (frame.image_index(), frame.image_count()),
            (request.image_index, frames)
        );
        // Check the warm hit before waiting for unrelated disk writes: the
        // insertion TTL is deliberately short and disk latency is unbounded.
        let pressure = root.try_reserve(root.available_bytes()).unwrap();
        let (hit, _) = load_cached_raster_with(&request, window, &mut cache, || {
            panic!("DICOM RAM hit redecoded under pressure")
        })
        .unwrap();
        let (RasterPixels::Rgba32Float(a), RasterPixels::Rgba32Float(b)) = (frame.pixels(), hit.pixels())
        else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        same_pixels(&hit, &oracle);
        drop(pressure);
        drop(hit);
        cache.swap().unwrap().wait_idle().unwrap();
        let mut payload = Vec::new();
        rrrah_cache::SwapPayload::write_payload(&frame, &mut payload).unwrap();
        let sdr = gpu.map(|g| g.render_raster(&oracle, params, viewport).pixels);
        let hdr = gpu.map(|g| g.render_raster_half(&oracle, params, viewport));
        if let Some(g) = gpu {
            if oracle.pixel_aspect().is_some_and(|a| a != 1.) {
                let square = oracle.clone().with_pixel_aspect(None).unwrap();
                assert_ne!(
                    g.render_raster(&square, params, viewport).pixels,
                    *sdr.as_ref().unwrap()
                );
                assert_ne!(
                    g.render_raster_half(&square, params, viewport),
                    *hdr.as_ref().unwrap()
                );
                shape_controls += 2;
            }
            assert_eq!(
                g.render_raster(&frame, params, viewport).pixels,
                *sdr.as_ref().unwrap()
            );
            assert_eq!(
                g.render_raster_half(&frame, params, viewport),
                *hdr.as_ref().unwrap()
            );
        }
        cases.push(Case {
            request,
            window,
            assumed,
            frames,
            oracle,
            payload,
            sdr,
            hdr,
        });
        drop(frame);
    }
    assert_eq!(cases.len(), 352);
    assert_eq!(full_palette_frames, 7);
    assert_eq!(shape_controls, if gpu.is_some() { 120 } else { 0 });
    assert_eq!(cache.len(), 1);
    let used = root.used();
    assert!(load_cached_raster_for_display(&cases[0].request, None, &mut cache).is_err());
    assert_eq!(root.used(), used);
    // The visible full-ramp image exceeds 4 KiB and cannot be evicted while
    // pinned. The smaller policy must be rejected without evicting it.
    assert!(!cache.set_limits_and_spill(rrrah_cache::CacheLimits {
        max_bytes: 4096,
        max_entries: Some(2),
        ttl: Some(Duration::from_millis(50)),
    }));
    thread::sleep(Duration::from_millis(60));
    assert_eq!(
        cache.spill_expired(),
        0,
        "visible frame must remain pinned past TTL"
    );
    assert_eq!(cache.len(), 1);
    // Future insertions get a separate long TTL for the blocked-prefetch phase.
    // Existing 50ms deadlines stay fixed, so the old frame still expires on unpin.
    assert!(cache.set_limits_and_spill(rrrah_cache::CacheLimits {
        max_bytes: 2097152,
        max_entries: Some(2),
        ttl: Some(Duration::from_secs(60)),
    }));
    // A real foreground transition releases the old pin. Its elapsed TTL,
    // rather than a zero-count workaround, spills the previous visible frame.
    let (next, _) = load_cached_raster_with(&cases[0].request, cases[0].window, &mut cache, || {
        panic!("DICOM foreground swap transition decoded again")
    })
    .unwrap();
    same_pixels(&next, &cases[0].oracle);
    let visible_frame = next;
    cache.swap().unwrap().wait_idle().unwrap();
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.swap().unwrap().stats().writes, 352);
    assert_eq!(cache.swap().unwrap().stats().reads, 1);
    let visible_weight = root.used();
    assert!(visible_weight > 0);
    assert!(cache.set_limits_and_spill(rrrah_cache::CacheLimits {
        max_bytes: 4096,
        max_entries: Some(1),
        ttl: None,
    }));
    let gate = Arc::new(DecodeGate::new());
    let ticket = gate.request_foreground();
    let generation = Arc::new(AtomicU64::new(1));
    let mut stale = cases[0].request.clone();
    stale.cancellation = Some(GenerationToken::new(generation.clone(), 0));
    assert!(preload_raster(&stale, cases[0].window, &mut cache, &gate).is_err());
    assert_eq!(cache.swap().unwrap().stats().reads, 1);
    assert_eq!(root.used(), visible_weight);
    let (stop, watchdog) = prefetch_watchdog(generation.clone(), Duration::from_secs(30));
    for case in cases.iter().rev() {
        let (params, viewport) = view(&case.oracle);
        let mut request = case.request.clone();
        request.cancellation = Some(GenerationToken::new(generation.clone(), 1));
        stop.send(true).expect("watchdog remains available");
        let restored = preload_raster(&request, case.window, &mut cache, &gate);
        let _ = stop.send(false);
        let (restored, assumed) = restored.expect("DICOM swap prefetch must bypass blocked decoder");
        assert_eq!(assumed, case.assumed);
        assert_eq!(
            (restored.image_index(), restored.image_count()),
            (request.image_index, case.frames)
        );
        same_pixels(&restored, &case.oracle);
        let mut payload = Vec::new();
        rrrah_cache::SwapPayload::write_payload(&restored, &mut payload).unwrap();
        assert_eq!(payload, case.payload);
        if let Some(g) = gpu {
            assert_eq!(
                g.render_raster(&restored, params, viewport).pixels,
                *case.sdr.as_ref().unwrap()
            );
            assert_eq!(
                g.render_raster_half(&restored, params, viewport),
                *case.hdr.as_ref().unwrap()
            );
        }
        drop(restored);
        assert!(
            cache.get_visible_lease_for(&visible_frame).is_some(),
            "background restore displaced visible DICOM frame"
        );
        assert_eq!(root.used(), visible_weight);
    }
    drop(stop);
    watchdog.join().unwrap();
    drop(ticket);
    let stats = cache.swap().unwrap().stats();
    assert_eq!((stats.writes, stats.reads, stats.errors), (352, 352, 0));
    drop(cache);
    drop(cases);
    assert_eq!(root.used(), visible_weight);
    drop(visible_frame);
    assert_eq!(root.used(), 0);
    eprintln!(
        "DICOM_SCALAR_PIPELINE cases=352 rle_states=31 full_palette_frames=7 ram_pressure_hits=352 swap_writes=352 swap_reads=352 shape_controls={} stale_refused=true decoder_blocked_prefetch=true ttl_nonzero=true visible_ttl_protected=true gpu={} root_peak={} final=0",
        shape_controls,
        gpu.is_some(),
        root.peak()
    );
}
#[test]
fn uncompressed_frames_windows_ram_swap_and_blocked_prefetch() {
    qualify(None);
}
#[test]
#[ignore = "actual Metal uncompressed DICOM window/frame cache and swap SDR/HDR readback"]
fn uncompressed_frames_windows_ram_swap_and_metal_readback() {
    let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual GPU required");
    eprintln!("DICOM_SCALAR_METAL adapter={}", gpu.adapter_name());
    qualify(Some(&gpu));
}

#[test]
#[ignore = "manual DICOM RLE decode/RAM/swap latency measurement; warm OS file cache"]
fn rle_collection_latency() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let source = fixtures.join("dicom-rle-segmented-explicit-full-ramp.dcm");
    let mut reference_request = DecodeRequest::new(&source);
    reference_request.assume_untagged_srgb = true;
    let (oracle, _) = load_raster_for_display_typed(&reference_request, None).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let root = rrrah_cache::MemoryBudget::new(8388608);
    let mut requests = Vec::new();
    for index in 0..64 {
        let path = dir.path().join(format!("frame-{index}.dcm"));
        std::fs::copy(&source, &path).unwrap();
        let mut request = DecodeRequest::new(path);
        request.assume_untagged_srgb = true;
        request.memory_budget = Some(root.clone());
        requests.push(request);
    }
    let mut cache = raster_cache_with_swap(
        Some(&dir.path().join("swap")),
        false,
        rrrah_cache::CacheLimits {
            max_bytes: 2097152,
            max_entries: Some(1),
            ttl: None,
        },
        Some(rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits {
                max_bytes: 134217728,
                max_entries: Some(128),
                ttl: None,
            },
            queue_bytes: 2097152,
            queue_count: 4,
            restore_bytes: 2097152,
        }),
        Some(&root),
    );
    let mut cold = Vec::new();
    for request in &requests {
        let start = Instant::now();
        let (frame, _) = load_cached_raster_for_display(request, None, &mut cache).unwrap();
        cold.push(start.elapsed().as_nanos());
        same_pixels(&frame, &oracle);
        drop(frame);
        cache.swap().unwrap().wait_idle().unwrap();
    }
    let mut warm = Vec::new();
    for _ in 0..64 {
        let start = Instant::now();
        let (frame, _) = load_cached_raster_with(requests.last().unwrap(), None, &mut cache, || {
            panic!("RAM latency measurement decoded")
        })
        .unwrap();
        warm.push(start.elapsed().as_nanos());
        same_pixels(&frame, &oracle);
    }
    let mut fingerprints = Vec::new();
    let mut resolved_ram = Vec::new();
    let last = requests.last().unwrap();
    let expected_key = raster_display_key(last, None).unwrap();
    for _ in 0..64 {
        let start = Instant::now();
        let key = raster_display_key(last, None).unwrap();
        fingerprints.push(start.elapsed().as_nanos());
        assert_eq!(key, expected_key);
        // Key computation and cloning are deliberately outside this interval.
        // Production still recomputes the source fingerprint for every load.
        let start = Instant::now();
        let (frame, _) = load_cached_raster_mode_resolving_key(
            last,
            || Some(key),
            &mut cache,
            true,
            false,
            || panic!("resolved RAM measurement decoded"),
        )
        .unwrap();
        resolved_ram.push(start.elapsed().as_nanos());
        same_pixels(&frame, &oracle);
    }
    let mut restored = Vec::new();
    for request in requests[..63].iter().rev() {
        let start = Instant::now();
        let (frame, _) = load_cached_raster_with(request, None, &mut cache, || {
            panic!("swap latency measurement decoded")
        })
        .unwrap();
        restored.push(start.elapsed().as_nanos());
        same_pixels(&frame, &oracle);
        drop(frame);
        cache.swap().unwrap().wait_idle().unwrap();
    }
    let stats = cache.swap().unwrap().stats();
    assert_eq!(stats.reads, 63);
    eprintln!("RLE_LATENCY_STATS {stats:?}");
    for (name, mut values) in [
        ("decode_prepare", cold),
        ("ram", warm),
        ("fingerprint", fingerprints),
        ("resolved_ram", resolved_ram),
        ("swap", restored),
    ] {
        eprintln!("RLE_LATENCY_RAW stage={name} ns={values:?}");
        values.sort_unstable();
        eprintln!(
            "RLE_LATENCY stage={name} n={} median_ns={} p95_ns={} max_ns={}",
            values.len(),
            values[values.len() / 2],
            values[(values.len() - 1) * 95 / 100],
            values.last().unwrap()
        );
    }
    let peak = root.peak();
    drop(cache);
    assert_eq!(root.used(), 0);
    eprintln!("RLE_LATENCY_MEMORY cap=8388608 peak={peak} final=0");
}

#[test]
fn rle_multiframe_prefetch_windows_preserve_nearest_under_count_and_byte_limits() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let mut oracles = Vec::new();
    for index in 0..3 {
        let mut request = DecodeRequest::new(fixtures.join("dicom-rle-rgb-explicit-8-planar0-plain.dcm"));
        request.image_index = index;
        request.assume_untagged_srgb = true;
        oracles.push(load_raster_for_display_typed(&request, None).unwrap().0);
    }
    let mut scenarios = 0;
    for selected in [0usize, 1, 31, 62, 63] {
        for direction in [gallery::NavDirection::Forward, gallery::NavDirection::Backward] {
            for (behind, ahead) in [(0, 0), (0, 5), (2, 0), (2, 5), (5, 2), (usize::MAX, usize::MAX)] {
                for count_limited in [false, true] {
                    let root = rrrah_cache::MemoryBudget::new(65536);
                    let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits {
                        max_bytes: if count_limited { 1048576 } else { 96 },
                        max_entries: count_limited.then_some(2),
                        ttl: None,
                    });
                    let mut request = DecodeRequest::new(fixtures.join("dicom-rle-prefetch-series64.dcm"));
                    request.image_index = selected;
                    request.assume_untagged_srgb = true;
                    request.memory_budget = Some(root.clone());
                    let (visible, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
                    same_pixels(&visible, &oracles[selected % 3]);
                    assert_eq!((visible.image_index(), visible.image_count()), (selected, 64));
                    let indices = gallery::neighbour_prefetch_indices(
                        64,
                        selected,
                        direction,
                        gallery::PrefetchWindow { behind, ahead },
                    );
                    let gate = Arc::new(DecodeGate::new());
                    let mut protected = Vec::new();
                    let mut skipped = 0;
                    for &index in &indices {
                        request.image_index = index;
                        match preload_raster_with_lease(&request, None, &mut cache, &gate) {
                            Ok((frame, _, lease)) => {
                                same_pixels(&frame, &oracles[index % 3]);
                                assert_eq!((frame.image_index(), frame.image_count()), (index, 64));
                                if let Some(lease) = lease { protected.push(lease); }
                            }
                            Err(error) => {
                                assert!(count_limited && !protected.is_empty());
                                assert_eq!(error, "raster prefetch skipped: all count slots are protected");
                                skipped += 1;
                            }
                        }
                    }
                    assert_eq!(skipped, if count_limited { indices.len().saturating_sub(1) } else { 0 });
                    assert_eq!(cache.len(), if indices.is_empty() { 1 } else { 2 });
                    assert_eq!(protected.len(), usize::from(!indices.is_empty()));
                    if let Some(&nearest) = indices.first() {
                        request.image_index = nearest;
                        let (hit, _) = load_cached_raster_mode(&request, None, &mut cache, false, || {
                            panic!("farther RLE prefetch evicted nearest")
                        })
                        .unwrap();
                        same_pixels(&hit, &oracles[nearest % 3]);
                    }
                    request.image_index = selected;
                    let (hit, _) = load_cached_raster_with(&request, None, &mut cache, || {
                        panic!("RLE prefetch evicted visible frame")
                    })
                    .unwrap();
                    same_pixels(&hit, &oracles[selected % 3]);
                    drop(hit);
                    drop(protected);
                    drop(visible);
                    drop(cache);
                    assert_eq!(root.used(), 0);
                    scenarios += 1;
                }
            }
        }
    }
    assert_eq!(scenarios, 120);
    eprintln!(
        "RLE_PREFETCH_WINDOWS scenarios={scenarios} frames=64 directions=2 policies=count,bytes zero_and_unbounded=true nearest_retained=true visible_retained=true final=0"
    );
}

fn qualify_managed_scientific_arrays(gpu: Option<&crate::navigation_metal_readback::GpuReadback>) {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let dir = tempfile::tempdir().unwrap();
    let root = rrrah_cache::MemoryBudget::new(1048576);
    let mut cache = raster_cache_with_swap(
        Some(dir.path()),
        false,
        rrrah_cache::CacheLimits {
            max_bytes: 4096,
            max_entries: Some(1),
            ttl: None,
        },
        Some(rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits {
                max_bytes: 4194304,
                max_entries: Some(512),
                ttl: None,
            },
            queue_bytes: 65536,
            queue_count: 4,
            restore_bytes: 65536,
        }),
        Some(&root),
    );
    let mut cases = Vec::new();
    let mut shape_controls = 0;
    for manifest in [
        include_str!("../../../tests/fixtures/raster/nrrd-manifest.tsv"),
        include_str!("../../../tests/fixtures/raster/mrc-manifest.tsv"),
        include_str!("../../../tests/fixtures/raster/fits-manifest.tsv"),
    ] {
        for line in manifest.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let f: Vec<_> = line.split('\t').collect();
            let index: usize = f[1].parse().unwrap();
            let fits = f[0].ends_with(".fits");
            let frames = if fits {
                manifest
                    .lines()
                    .filter(|l| l.starts_with(f[0]) && l.as_bytes().get(f[0].len()) == Some(&b'\t'))
                    .map(|l| l.split('\t').nth(1).unwrap().parse::<usize>().unwrap())
                    .max()
                    .unwrap()
                    + 1
            } else {
                2
            };
            let raw = std::fs::read(fixtures.join(if fits { f[7] } else { f[4] })).unwrap();
            if fits {
                let physical = std::fs::read(fixtures.join(f[6])).unwrap();
                if physical
                    .chunks_exact(8)
                    .any(|v| !f64::from_le_bytes(v.try_into().unwrap()).is_finite())
                {
                    let mut request = DecodeRequest::new(fixtures.join(f[0]));
                    request.image_index = index;
                    request.memory_budget = Some(root.clone());
                    let used = root.used();
                    let window = rrrah_decode::ScalarWindow::new(-100., 1000.).unwrap();
                    assert!(load_raster_for_display_typed(&request, Some(window)).is_err());
                    assert_eq!(root.used(), used);
                    continue;
                }
            }
            let windows = if fits {
                vec![(-100f32, 1000f32)]
            } else {
                vec![(-100f32, 100f32), (0., 255.)]
            };
            let variants = if f[0].ends_with(".nrrd") {
                vec![(None, None), (Some(2f32), None), (Some(0.5f32), None)]
            } else if f[0].ends_with(".mrc") {
                vec![
                    (None, None),
                    (Some(0.5), Some([1u32, 2, 3])),
                    (Some(0.25), Some([1, 3, 2])),
                    (Some(2.), Some([2, 1, 3])),
                    (Some(0.5), Some([2, 3, 1])),
                    (Some(4.), Some([3, 1, 2])),
                    (Some(2.), Some([3, 2, 1])),
                ]
            } else {
                vec![(None, None)]
            };
            for (aspect, axes) in variants {
                let path = if let Some(aspect) = aspect {
                    let code = axes.map_or(0, |[a, b, c]| a * 100 + b * 10 + c);
                    let path = dir
                        .path()
                        .join(format!("aspect-{:08x}-map{code}-{}", aspect.to_bits(), f[0]));
                    if !path.exists() {
                        let original = std::fs::read(fixtures.join(f[0])).unwrap();
                        let changed = if let Some(axes) = axes {
                            let mut changed = original.clone();
                            let be = original[212] == 0x11;
                            let mut set = |at: usize, value: u32| {
                                changed[at..at + 4].copy_from_slice(&if be {
                                    value.to_be_bytes()
                                } else {
                                    value.to_le_bytes()
                                });
                            };
                            for (i, (grid, cell)) in
                                [(4u32, 8f32), (5, 20.), (6, 48.)].into_iter().enumerate()
                            {
                                set(28 + i * 4, grid);
                                set(40 + i * 4, cell.to_bits());
                                set(52 + i * 4, 90f32.to_bits());
                                set(64 + i * 4, axes[i]);
                            }
                            changed
                        } else {
                            let end = original.windows(2).position(|v| v == b"\n\n").unwrap();
                            let mut changed = original[..end].to_vec();
                            let spacing = if aspect == 2. { "-2 1 1" } else { "1 2 1" };
                            changed.extend_from_slice(format!("\nspacings: {spacing}\n\n").as_bytes());
                            changed.extend_from_slice(&original[end + 2..]);
                            changed
                        };
                        std::fs::write(&path, changed).unwrap();
                    }
                    path
                } else {
                    fixtures.join(f[0])
                };
                for &(minimum, maximum) in &windows {
                    let mut expected = Vec::new();
                    for pixel in raw.chunks_exact(16) {
                        let value = f32::from_le_bytes(pixel[..4].try_into().unwrap());
                        let v = if fits {
                            value
                        } else {
                            ((value - minimum) / (maximum - minimum)).clamp(0., 1.)
                        };
                        expected.extend_from_slice(&[v, v, v, 1.]);
                    }
                    let oracle = DecodedRaster::new(
                        f[2].parse().unwrap(),
                        f[3].parse().unwrap(),
                        RasterPixels::Rgba32Float(Arc::new(expected).into()),
                        RasterColorSpace::LinearSrgb,
                    )
                    .unwrap()
                    .with_pixel_aspect(aspect)
                    .unwrap();
                    let mut request = DecodeRequest::new(&path);
                    request.image_index = index;
                    request.memory_budget = Some(root.clone());
                    let window = Some(rrrah_decode::ScalarWindow::new(minimum, maximum).unwrap());
                    let (frame, assumed) =
                        load_cached_raster_for_display(&request, window, &mut cache).unwrap();
                    assert!(!assumed);
                    same_pixels(&frame, &oracle);
                    assert_eq!((frame.image_index(), frame.image_count()), (index, frames));
                    let pressure = root.try_reserve(root.available_bytes()).unwrap();
                    let (hit, _) = load_cached_raster_with(&request, window, &mut cache, || {
                        panic!("scientific RAM hit decoded")
                    })
                    .unwrap();
                    same_pixels(&hit, &oracle);
                    drop(hit);
                    drop(pressure);
                    let (params, viewport) = view(&oracle);
                    let sdr = gpu.map(|g| g.render_raster(&oracle, params, viewport).pixels);
                    let hdr = gpu.map(|g| g.render_raster_half(&oracle, params, viewport));
                    if let Some(g) = gpu {
                        if aspect.is_some() {
                            let square = oracle.clone().with_pixel_aspect(None).unwrap();
                            assert_ne!(
                                g.render_raster(&square, params, viewport).pixels,
                                *sdr.as_ref().unwrap()
                            );
                            assert_ne!(
                                g.render_raster_half(&square, params, viewport),
                                *hdr.as_ref().unwrap()
                            );
                            shape_controls += 2;
                        }
                        assert_eq!(
                            g.render_raster(&frame, params, viewport).pixels,
                            *sdr.as_ref().unwrap()
                        );
                        assert_eq!(
                            g.render_raster_half(&frame, params, viewport),
                            *hdr.as_ref().unwrap()
                        );
                    }
                    cases.push(Case {
                        request,
                        window,
                        assumed,
                        frames,
                        oracle,
                        payload: Vec::new(),
                        sdr,
                        hdr,
                    });
                    drop(frame);
                    cache.swap().unwrap().wait_idle().unwrap();
                }
            }
        }
    }
    assert_eq!(cases.len(), 492);
    assert_eq!(shape_controls, if gpu.is_some() { 736 } else { 0 });
    let (visible, _) = load_cached_raster_with(&cases[0].request, cases[0].window, &mut cache, || {
        panic!("scientific transition decoded")
    })
    .unwrap();
    cache.swap().unwrap().wait_idle().unwrap();
    let gate = Arc::new(DecodeGate::new());
    let ticket = gate.request_foreground();
    let generation = Arc::new(AtomicU64::new(1));
    let (sender, watchdog) = prefetch_watchdog(generation.clone(), Duration::from_secs(30));
    for case in cases.iter().rev() {
        let mut request = case.request.clone();
        request.cancellation = Some(GenerationToken::new(generation.clone(), 1));
        sender.send(true).unwrap();
        let result = preload_raster(&request, case.window, &mut cache, &gate);
        sender.send(false).unwrap();
        let (restored, assumed) = result.expect("scientific swap restoration must bypass blocked decoder");
        assert!(!assumed);
        same_pixels(&restored, &case.oracle);
        assert_eq!(
            (restored.image_index(), restored.image_count()),
            (request.image_index, case.frames)
        );
        if let Some(g) = gpu {
            let (params, viewport) = view(&case.oracle);
            assert_eq!(
                g.render_raster(&restored, params, viewport).pixels,
                *case.sdr.as_ref().unwrap()
            );
            assert_eq!(
                g.render_raster_half(&restored, params, viewport),
                *case.hdr.as_ref().unwrap()
            );
        }
        drop(restored);
        assert!(cache.get_visible_lease_for(&visible).is_some());
    }
    drop(sender);
    watchdog.join().unwrap();
    drop(ticket);
    let stats = cache.swap().unwrap().stats();
    assert_eq!(
        (stats.writes, stats.reads, stats.errors, stats.dropped),
        (cases.len() as u64, cases.len() as u64, 0, 0)
    );
    let count = cases.len();
    drop(cases);
    drop(cache);
    drop(visible);
    assert_eq!(root.used(), 0);
    eprintln!(
        "SCIENTIFIC_ARRAY_PIPELINE cases={count} gpu={} readbacks={} shape_controls={shape_controls} ram_pressure_hits={count} swap_writes={count} swap_reads={count} blocked_decoder=true final=0",
        gpu.is_some(),
        if gpu.is_some() {
            count * 6 + shape_controls
        } else {
            0
        }
    );
}
#[test]
fn managed_nrrd_mrc_fits_windows_ram_swap_and_blocked_prefetch() {
    qualify_managed_scientific_arrays(None);
}
#[test]
#[ignore = "actual Metal SDR/HDR scientific NRRD/MRC/FITS cache and swap verification"]
fn managed_nrrd_mrc_fits_windows_ram_swap_and_metal() {
    let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual GPU required");
    eprintln!("SCIENTIFIC_ARRAY_METAL adapter={}", gpu.adapter_name());
    qualify_managed_scientific_arrays(Some(&gpu));
}


// Authored 2x3 RGBA PNGs use a single stored DEFLATE block. Neither the encoder
// nor the explicit six-pixel permutations call the decoder's orientation code.
fn png_orientation_cases(dir: &Path) -> Vec<(PathBuf, usize, usize, DecodedRaster)> {
    fn chunk(output: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(kind);
        output.extend_from_slice(data);
        let mut crc = !0u32;
        for &byte in kind.iter().chain(data) {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
            }
        }
        output.extend_from_slice(&(!crc).to_be_bytes());
    }
    let pixels: [[u8; 4]; 6] = [
        [255,0,0,255], [0,255,0,128], [0,0,255,64],
        [255,255,0,0], [255,0,255,192], [0,255,255,255],
    ];
    let mut rows = Vec::new();
    for row in pixels.chunks_exact(2) {
        rows.push(0); // PNG filter None.
        for pixel in row { rows.extend_from_slice(pixel); }
    }
    let mut compressed = vec![0x78,0x01,0x01]; // zlib header, final stored block.
    let len = rows.len() as u16;
    compressed.extend_from_slice(&len.to_le_bytes());
    compressed.extend_from_slice(&(!len).to_le_bytes());
    compressed.extend_from_slice(&rows);
    let (mut a, mut b) = (1u32,0u32);
    for &byte in &rows { a=(a+u32::from(byte))%65521; b=(b+a)%65521; }
    compressed.extend_from_slice(&((b<<16)|a).to_be_bytes());
    let permutations = [
        [0,1,2,3,4,5], [1,0,3,2,5,4], [5,4,3,2,1,0], [4,5,2,3,0,1],
        [0,2,4,1,3,5], [4,2,0,5,3,1], [5,3,1,4,2,0], [1,3,5,0,2,4],
    ];
    permutations.into_iter().enumerate().map(|(index, permutation)| {
        let orientation = index as u8 + 1;
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&2u32.to_be_bytes());
        ihdr.extend_from_slice(&3u32.to_be_bytes());
        ihdr.extend_from_slice(&[8,6,0,0,0]);
        chunk(&mut png,b"IHDR",&ihdr);
        chunk(&mut png,b"sRGB",&[0]);
        let mut exif = b"II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x01\0\0\0\0\0\0\0".to_vec();
        exif[18] = orientation;
        chunk(&mut png,b"eXIf",&exif);
        chunk(&mut png,b"IDAT",&compressed);
        chunk(&mut png,b"IEND",&[]);
        let path = dir.join(format!("orientation-{orientation}.png"));
        std::fs::write(&path,png).unwrap();
        let expected: Vec<u8> = permutation.into_iter().flat_map(|at|pixels[at]).collect();
        let (width,height) = if orientation <= 4 {(2,3)} else {(3,2)};
        (path,0,1,DecodedRaster::new(width,height,
            RasterPixels::Rgba8(Arc::new(expected).into()),RasterColorSpace::Srgb).unwrap())
    }).collect()
}

#[derive(Clone, Copy)]
enum RasterCorpus { Legacy, KtxProducers, GrayAlpha, HighAlpha, MultiAlpha, PngTiffMetadata }
fn qualify_legacy_rasters(gpu: Option<&crate::navigation_metal_readback::GpuReadback>, corpus: RasterCorpus) {
    let png_tiff = matches!(corpus, RasterCorpus::PngTiffMetadata);
    let producers = matches!(corpus,RasterCorpus::KtxProducers);
    let gray_alpha = matches!(corpus,RasterCorpus::GrayAlpha);
    let multi_alpha = matches!(corpus,RasterCorpus::MultiAlpha);
    let high_alpha = matches!(corpus,RasterCorpus::HighAlpha | RasterCorpus::MultiAlpha);
    let alpha = gray_alpha || high_alpha;
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let dir = tempfile::tempdir().unwrap();
    let root = rrrah_cache::MemoryBudget::new(if alpha { 8388608 } else if producers { 2097152 } else { 131072 });
    let mut cache = raster_cache_with_swap(
        Some(dir.path()),
        false,
        rrrah_cache::CacheLimits {
            max_bytes: if alpha { 3145728 } else if producers { 524288 } else { 8192 },
            max_entries: Some(1),
            ttl: None,
        },
        Some(rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits {
                max_bytes: if alpha { 16777216 } else { 1048576 },
                max_entries: Some(if multi_alpha {64} else if high_alpha {16} else if gray_alpha {4} else if producers { 16 } else { 2048 }),
                ttl: None,
            },
            queue_bytes: if alpha { 3145728 } else if producers { 1048576 } else { 65536 },
            queue_count: 4,
            restore_bytes: if alpha { 6291456 } else if producers { 1048576 } else { 65536 },
        }),
        Some(&root),
    );
    let mut cases = Vec::new();
    let params = |frame: &DecodedRaster| {
        let scale = if alpha {1} else {8};
        let viewport = if high_alpha {[128,64]} else {[frame.width()*scale,frame.height()*scale]};
        (
            rrrah_gpu::ViewParameters {
                viewport: viewport.map(|v| v as f32),
                zoom: scale as f32,
                ..Default::default()
            },
            viewport,
        )
    };
    let mut inputs = Vec::new();
    for line in include_str!("../../../tests/fixtures/raster/manifest.tsv").lines() {
        let f: Vec<_> = line.split('\t').collect();
        if !f[0].ends_with(".ras")
            && !f[0].ends_with(".pkm")
            && !f[0].ends_with(".psd")
            && !f[0].ends_with(".psb")
            && !matches!(f[0], "pattern.rgb" | "pattern.rgba")
        {
            continue;
        }
        let raw = DecodedRaster::new(
            f[1].parse().unwrap(),
            f[2].parse().unwrap(),
            RasterPixels::Rgba8(Arc::new(std::fs::read(fixtures.join(f[4])).unwrap()).into()),
            RasterColorSpace::AssumedSrgb,
        )
        .unwrap();
        inputs.push((fixtures.join(f[0]), 0usize, 1usize, raw));
    }
    for line in include_str!("../../../tests/fixtures/raster/pkm-eac-manifest.tsv").lines().filter(|line| !line.starts_with('#')) {
        let fields: Vec<_> = line.split('\t').collect();
        let bytes = std::fs::read(fixtures.join(fields[4])).unwrap();
        let values: Vec<f32> = bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
        let raw = DecodedRaster::new(fields[1].parse().unwrap(),fields[2].parse().unwrap(),
            RasterPixels::Rgba32Float(Arc::new(values).into()),RasterColorSpace::LinearSrgb).unwrap();
        inputs.push((fixtures.join(fields[0]),0usize,1usize,raw));
    }
    let pcx_oracle = vec![
        11, 22, 33, 255, 192, 193, 255, 255, 1, 2, 3, 255, 44, 55, 66, 255, 77, 88, 99, 255, 0,
        255, 127, 255,
    ];
    for name in ["rgb.pcx", "indexed.pcx"] {
        let raw = DecodedRaster::new(
            3,
            2,
            RasterPixels::Rgba8(Arc::new(pcx_oracle.clone()).into()),
            RasterColorSpace::AssumedSrgb,
        )
        .unwrap();
        inputs.push((fixtures.join("../pcx").join(name), 0, 1, raw));
    }
    for index in 0..2 {
        let pixels =
            std::fs::read(fixtures.join(format!("two-pages.dcx.page{index}.rgba"))).unwrap();
        let raw = DecodedRaster::new(
            3,
            2,
            RasterPixels::Rgba8(Arc::new(pixels).into()),
            RasterColorSpace::AssumedSrgb,
        )
        .unwrap();
        inputs.push((fixtures.join("two-pages.dcx"), index, 2, raw));
    }
    for line in include_str!("../../../tests/fixtures/raster/ktx-mip-manifest.tsv")
        .lines()
        .chain(include_str!("../../../tests/fixtures/raster/ktx-etc1-manifest.tsv").lines())
        .chain(include_str!("../../../tests/fixtures/raster/ktx-eac-manifest.tsv").lines())
        .chain(include_str!("../../../tests/fixtures/raster/ktx-rg-manifest.tsv").lines())
        .chain(include_str!("../../../tests/fixtures/raster/ktx-half-manifest.tsv").lines())
        .chain(include_str!("../../../tests/fixtures/raster/ktx2-mip-manifest.tsv").lines())
        .chain(include_str!("../../../tests/fixtures/raster/ktx2-half-manifest.tsv").lines())
        .chain(include_str!("../../../tests/fixtures/raster/ktx2-rg-manifest.tsv").lines())
        .chain(include_str!("../../../tests/fixtures/raster/pvr-manifest.tsv").lines())
        .chain(include_str!("../../../tests/fixtures/raster/pvr-etc1-manifest.tsv").lines())
        .chain(include_str!("../../../tests/fixtures/raster/pvr-bc-manifest.tsv").lines())
        .chain(include_str!("../../../tests/fixtures/raster/pvr-associated-bc-manifest.tsv").lines())
        .chain(include_str!("../../../tests/fixtures/raster/pvr-eac-manifest.tsv").lines())
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let f: Vec<_> = line.split('\t').collect();
        let bytes = std::fs::read(fixtures.join(f[6])).unwrap();
        let pixels = match f[4] {
            "rgba8" => RasterPixels::Rgba8(Arc::new(bytes).into()),
            "rgba16" => RasterPixels::Rgba16(
                Arc::new(
                    bytes
                        .chunks_exact(2)
                        .map(|v| u16::from_le_bytes(v.try_into().unwrap()))
                        .collect::<Vec<_>>(),
                )
                .into(),
            ),
            "rgba32f" => RasterPixels::Rgba32Float(
                Arc::new(
                    bytes
                        .chunks_exact(4)
                        .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
                        .collect::<Vec<_>>(),
                )
                .into(),
            ),
            other => panic!("unsupported oracle type {other}"),
        };
        let raw = DecodedRaster::new(
            f[2].parse().unwrap(),
            f[3].parse().unwrap(),
            pixels,
            if matches!(f[5], "srgb" | "1") {
                RasterColorSpace::Srgb
            } else {
                RasterColorSpace::LinearSrgb
            },
        )
        .unwrap();
        let frames = if f[0].ends_with(".pvr") {
            let source = std::fs::read(fixtures.join(f[0])).unwrap();
            let bytes = source[44..48].try_into().unwrap();
            (if source.starts_with(b"PVR\x03") {
                u32::from_le_bytes(bytes)
            } else {
                u32::from_be_bytes(bytes)
            }) as usize
        } else {
            4
        };
        inputs.push((fixtures.join(f[0]), f[1].parse().unwrap(), frames, raw));
    }
    // EAC has no hardware sRGB variant. Exercise the explicit PVR color
    // declaration through software display preparation, using unsigned color
    // samples only; signed EAC data has no independently qualified color model.
    for line in include_str!("../../../tests/fixtures/raster/pvr-eac-manifest.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let fields: Vec<_> = line.split('\t').collect();
        let mut source = std::fs::read(fixtures.join(fields[0])).unwrap();
        let channel_type = u32::from_le_bytes(source[20..24].try_into().unwrap());
        if !matches!(channel_type, 0 | 2 | 4 | 6 | 8 | 10 | 13) {
            continue;
        }
        source[16..20].copy_from_slice(&1u32.to_le_bytes());
        let path = dir.path().join(format!("srgb-{}", fields[0]));
        std::fs::write(&path, source).unwrap();
        let bytes = std::fs::read(fixtures.join(fields[6])).unwrap();
        let mut values: Vec<f32> = bytes.chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
        for rgba in values.chunks_exact_mut(4) {
            assert_eq!(rgba[3].to_bits(), 1f32.to_bits());
            for value in &mut rgba[..3] {
                let encoded = *value;
                let x = f64::from(encoded);
                let reference = if x <= 0.04045 { x / 12.92 }
                    else { ((x + 0.055) / 1.055).powf(2.4) };
                *value = if encoded <= 0.04045 { encoded / 12.92 }
                    else { ((encoded + 0.055) / 1.055).powf(2.4) };
                assert!((f64::from(*value) - reference).abs() < 0.0000002);
            }
        }
        // Already-linear independent oracle: preparing it must be an identity.
        let raw = DecodedRaster::new(fields[2].parse().unwrap(), fields[3].parse().unwrap(),
            RasterPixels::Rgba32Float(Arc::new(values).into()), RasterColorSpace::LinearSrgb).unwrap();
        inputs.push((path, fields[1].parse().unwrap(), 4, raw));
    }
    // Exercise the managed metadata index through the public loader and swap.
    let original = std::fs::read(fixtures.join("rgba8-0-rd.ktx")).unwrap();
    let old_len = u32::from_le_bytes(original[60..64].try_into().unwrap()) as usize;
    let mut metadata = original[64..64 + old_len].to_vec();
    for i in (0..128).rev() {
        let key = format!("unknown-{i:03}");
        let n = key.len() + 2;
        metadata.extend((n as u32).to_le_bytes());
        metadata.extend(key.as_bytes());
        metadata.extend([0, 42]);
        metadata.resize(metadata.len().next_multiple_of(4), 0);
    }
    let mut source = original[..64].to_vec();
    source[60..64].copy_from_slice(&(metadata.len() as u32).to_le_bytes());
    source.extend(metadata);
    source.extend(&original[64 + old_len..]);
    let path = dir.path().join("managed-metadata.ktx");
    std::fs::write(&path, source).unwrap();
    let raw = DecodedRaster::new(
        3,
        2,
        RasterPixels::Rgba8(
            Arc::new(std::fs::read(fixtures.join("rgba8-0-rd.ktx.rgba")).unwrap()).into(),
        ),
        RasterColorSpace::Srgb,
    )
    .unwrap();
    inputs.push((path, 0, 1, raw));
    for line in include_str!("../../../tests/fixtures/raster/dds-mip-manifest.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let f: Vec<_> = line.split('\t').collect();
        let color = match f[4] {
            "linear" => RasterColorSpace::LinearSrgb,
            "srgb" => RasterColorSpace::Srgb,
            "assumed" => RasterColorSpace::AssumedSrgb,
            other => panic!("unexpected DDS transfer {other}"),
        };
        let raw = DecodedRaster::new(
            f[2].parse().unwrap(),
            f[3].parse().unwrap(),
            RasterPixels::Rgba8(Arc::new(std::fs::read(fixtures.join(f[5])).unwrap()).into()),
            color,
        )
        .unwrap();
        inputs.push((fixtures.join(f[0]), f[1].parse().unwrap(), 4, raw));
    }
    let mut source = std::fs::read(fixtures.join("ktx2-mips-29-0-rd.ktx2")).unwrap();
    let start = u32::from_le_bytes(source[56..60].try_into().unwrap()) as usize;
    let old_len = u32::from_le_bytes(source[60..64].try_into().unwrap()) as usize;
    let end = start + old_len;
    let mut records = Vec::new();
    for i in (0..128).rev() {
        let key = if i == 127 {
            format!("unknown-{i:03}-padding")
        } else {
            format!("unknown-{i:03}")
        };
        let n = key.len() + 2;
        records.extend((n as u32).to_le_bytes());
        records.extend(key.as_bytes());
        records.extend([0, 42]);
        records.resize(records.len().next_multiple_of(4), 0);
    }
    let added = records.len();
    source.splice(end..end, records);
    source[60..64].copy_from_slice(&((old_len + added) as u32).to_le_bytes());
    for level in 0..4 {
        let at = 80 + level * 24;
        let offset = u64::from_le_bytes(source[at..at + 8].try_into().unwrap());
        assert!(offset >= end as u64);
        source[at..at + 8].copy_from_slice(&(offset + added as u64).to_le_bytes());
    }
    let path = dir.path().join("managed-metadata.ktx2");
    std::fs::write(&path, source).unwrap();
    let raw = DecodedRaster::new(
        9,
        5,
        RasterPixels::Rgba8(
            Arc::new(std::fs::read(fixtures.join("ktx2-mips-29-0-rd.ktx2.mip0.rgba8")).unwrap())
                .into(),
        ),
        RasterColorSpace::Srgb,
    )
    .unwrap();
    inputs.push((path, 0, 4, raw));
    for line in include_str!("../../../tests/fixtures/raster/wad-manifest.tsv").lines() {
        let f: Vec<_> = line.split('\t').collect();
        let raw = DecodedRaster::new(
            f[2].parse().unwrap(),
            f[3].parse().unwrap(),
            RasterPixels::Rgba8(Arc::new(std::fs::read(fixtures.join(f[4])).unwrap()).into()),
            RasterColorSpace::AssumedSrgb,
        )
        .unwrap();
        inputs.push((fixtures.join(f[0]), f[1].parse().unwrap(), 2, raw));
    }
    let game = dir.path().join("wal-game");
    std::fs::create_dir_all(game.join("textures")).unwrap();
    std::fs::create_dir_all(game.join("pics")).unwrap();
    std::fs::copy(
        fixtures.join("wal-colormap.pcx"),
        game.join("pics/colormap.pcx"),
    )
    .unwrap();
    for line in include_str!("../../../tests/fixtures/raster/wal-manifest.tsv").lines() {
        let f: Vec<_> = line.split('\t').collect();
        let path = game.join("textures").join(f[0]);
        std::fs::copy(fixtures.join(f[0]), &path).unwrap();
        let raw = DecodedRaster::new(
            f[1].parse().unwrap(),
            f[2].parse().unwrap(),
            RasterPixels::Rgba8(Arc::new(std::fs::read(fixtures.join(f[3])).unwrap()).into()),
            RasterColorSpace::AssumedSrgb,
        )
        .unwrap();
        inputs.push((path, 0, 1, raw));
    }
    let sti: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/sti/manifest.json")).unwrap();
    for case in sti["cases"].as_array().unwrap() {
        let name = case["file"].as_str().unwrap();
        let oracles = case["rgba"].as_array().unwrap();
        for (index, values) in oracles.iter().enumerate() {
            let values = values.as_array().unwrap();
            let pixels = if case["sample_bits"].as_u64().unwrap() == 8 {
                RasterPixels::Rgba8(
                    Arc::new(
                        values
                            .iter()
                            .map(|v| u8::try_from(v.as_u64().unwrap()).unwrap())
                            .collect::<Vec<_>>(),
                    )
                    .into(),
                )
            } else {
                RasterPixels::Rgba16(
                    Arc::new(
                        values
                            .iter()
                            .map(|v| u16::try_from(v.as_u64().unwrap()).unwrap())
                            .collect::<Vec<_>>(),
                    )
                    .into(),
                )
            };
            let raw = DecodedRaster::new(3, 2, pixels, RasterColorSpace::AssumedSrgb).unwrap();
            inputs.push((
                fixtures.join("../sti").join(name),
                index,
                oracles.len(),
                raw,
            ));
        }
    }
    let bytes = std::fs::read(fixtures.join("rla-16-c3-a16-mixed0.rla.u16")).unwrap();
    let raw = DecodedRaster::new(
        140,
        3,
        RasterPixels::Rgba16(
            Arc::new(
                bytes
                    .chunks_exact(2)
                    .map(|v| u16::from_le_bytes(v.try_into().unwrap()))
                    .collect::<Vec<_>>(),
            )
            .into(),
        ),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    inputs.push((fixtures.join("rla-16-c3-a16-mixed0.rla"), 0, 1, raw));
    let raw = DecodedRaster::new(
        2,
        1,
        RasterPixels::Rgba32Float(Arc::new(vec![4.0, 2.0, -0.5, 1.0, 4.0, 2.0, -0.5, 1.0]).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    inputs.push((fixtures.join("rla-float32-le-hdr-oiio.rla"), 0, 1, raw));
    // BC2/BC3 always retain four-color interpolation, even with reversed endpoints.
    for internal in [0x83f2u32, 0x83f3] {
        let mut source = b"\xabKTX 11\xbb\r\n\x1a\n".to_vec();
        for word in [0x04030201, 0, 1, 0, internal, 0x1908, 4, 4, 0, 0, 1, 1, 0] {
            source.extend(word.to_le_bytes());
        }
        source.extend(16u32.to_le_bytes());
        source.extend(if internal == 0x83f2 { [255u8; 8] } else { [255, 0, 0, 0, 0, 0, 0, 0] });
        source.extend([0, 0, 255, 255, 0xee, 0xee, 0xee, 0xee]);
        let path = dir.path().join(format!("bc-reversed-{internal}.ktx"));
        std::fs::write(&path, source).unwrap();
        let pixels: Vec<u8> = (0..16).flat_map(|i| {
            let color = if i % 2 == 0 { 85 } else { 170 };
            [color, color, color, 255]
        }).collect();
        let raw = DecodedRaster::new(4, 4, RasterPixels::Rgba8(Arc::new(pixels).into()),
            RasterColorSpace::LinearSrgb).unwrap();
        inputs.push((path, 0, 1, raw));
    }
    // Pixel references are independent asefile/authored composition dumps.
    // ICC expected metadata comes from a separately pinned profile file.
    for line in include_str!("../../../tests/fixtures/raster/aseprite-manifest.tsv")
        .lines().filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let fields: Vec<_> = line.split('\t').collect();
        let color = if fields[0].starts_with("ase-profile-icc.") { RasterColorSpace::Icc(std::fs::read(fixtures.join("dicom-rgb-profile.icc")).unwrap()) }
            else if fields[0].starts_with("ase-profile-linear.") { RasterColorSpace::LinearSrgb }
            else if fields[0].starts_with("ase-profile-assumed.") { RasterColorSpace::AssumedSrgb }
            else { RasterColorSpace::Srgb };
        let raw = DecodedRaster::new(fields[2].parse().unwrap(), fields[3].parse().unwrap(),
            RasterPixels::Rgba8(Arc::new(std::fs::read(fixtures.join(fields[7])).unwrap()).into()), color).unwrap();
        inputs.push((fixtures.join(fields[0]), fields[1].parse().unwrap(), fields[4].parse().unwrap(), raw));
    }
    for line in include_str!("../../../tests/fixtures/raster/psd-tools-16bit-manifest.tsv")
        .lines().filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let fields: Vec<_> = line.split('\t').collect();
        let bytes = std::fs::read(fixtures.join(fields[3])).unwrap();
        let values = bytes.chunks_exact(2).map(|b| u16::from_le_bytes(b.try_into().unwrap())).collect::<Vec<_>>();
        let raw = DecodedRaster::new(fields[1].parse().unwrap(), fields[2].parse().unwrap(),
            RasterPixels::Rgba16(Arc::new(values).into()),
            RasterColorSpace::Icc(std::fs::read(fixtures.join(fields[5])).unwrap())).unwrap();
        inputs.push((fixtures.join(fields[0]), 0, 1, raw));
    }
    for name in ["4x4_16bit_rgb.psd", "4x4_16bit_grayscale.psd", "4x4_32bit_rgb.psd", "4x4_32bit_grayscale.psd", "32bit5x5.psb"] {
        let name = format!("psd-tools-{name}");
        let bytes = std::fs::read(fixtures.join(format!("{name}.rgba-le"))).unwrap();
        let pixels = if name.contains("16bit") {
            RasterPixels::Rgba16(Arc::new(bytes.chunks_exact(2).map(|b| u16::from_le_bytes(b.try_into().unwrap())).collect::<Vec<_>>()).into())
        } else {
            RasterPixels::Rgba32Float(Arc::new(bytes.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect::<Vec<_>>()).into())
        };
        let edge = if name.ends_with(".psb") {5} else {4};
        let raw = DecodedRaster::new(edge,edge,pixels,RasterColorSpace::Icc(std::fs::read(fixtures.join(format!("{name}.icc"))).unwrap())).unwrap();
        inputs.push((fixtures.join(name),0,1,raw));
    }
    if producers {
        inputs.clear();
        for line in include_str!("../../../tests/fixtures/raster/ktx2-khronos-producer-manifest.tsv")
            .lines().filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let fields: Vec<_> = line.split('\t').collect();
            let raw = DecodedRaster::new(fields[2].parse().unwrap(), fields[3].parse().unwrap(),
                RasterPixels::Rgba8(Arc::new(std::fs::read(fixtures.join(fields[5])).unwrap()).into()),
                RasterColorSpace::Srgb).unwrap();
            inputs.push((fixtures.join(fields[0]), fields[1].parse().unwrap(), fields[4].parse().unwrap(), raw));
        }
        eprintln!("OFFICIAL_KTX2_PIPELINE root=2097152 ram=524288/count1 queue=1048576/count4 restore=1048576 swap=1048576/count16");
    }
    if gray_alpha {
        inputs.clear();
        for ext in ["psd","psb"] {
            let raw = DecodedRaster::new(400,359,
                RasterPixels::Rgba8(Arc::new(std::fs::read(fixtures.join("psd-tools-gray0.rgba")).unwrap()).into()),
                RasterColorSpace::Icc(std::fs::read(fixtures.join("psd-tools-gray0.icc")).unwrap())).unwrap();
            inputs.push((fixtures.join(format!("psd-tools-gray0.{ext}")),0,1,raw));
        }
        eprintln!("GRAY_ALPHA_PIPELINE root=8388608 ram=3145728/count1 swap=16777216/count4 queue=3145728/count4 restore=6291456");
    }
    if high_alpha && !multi_alpha {
        inputs.clear();
        for line in include_str!("../../../tests/fixtures/raster/psd-tools-alpha-manifest.tsv").lines().filter(|s| !s.starts_with('#')) {
            let fields:Vec<_>=line.split('\t').collect();
            let bytes=std::fs::read(fixtures.join(fields[4])).unwrap();
            let pixels=if fields[3]=="16" {
                RasterPixels::Rgba16(Arc::new(bytes.chunks_exact(2).map(|b|u16::from_le_bytes(b.try_into().unwrap())).collect::<Vec<_>>()).into())
            } else {
                RasterPixels::Rgba32Float(Arc::new(bytes.chunks_exact(4).map(|b|f32::from_le_bytes(b.try_into().unwrap())).collect::<Vec<_>>()).into())
            };
            let raw=DecodedRaster::new(4,1,pixels,RasterColorSpace::LinearSrgb).unwrap();
            inputs.push((fixtures.join(fields[0]),0,1,raw));
        }
        eprintln!("HIGH_ALPHA_PIPELINE cases=8 explicit_linear=true root=8388608 ram_count=1 swap_count=16");
    }
    if multi_alpha {
        inputs.clear();
        for line in include_str!("../../../tests/fixtures/raster/psd-tools-alpha-multi-manifest.tsv")
            .lines().filter(|s| !s.starts_with('#') && !s.is_empty())
        {
            let f: Vec<_> = line.split('\t').collect();
            let bytes = std::fs::read(fixtures.join(f[1])).unwrap();
            let pixels = if f[4] == "16" {
                RasterPixels::Rgba16(Arc::new(bytes.chunks_exact(2)
                    .map(|b| u16::from_le_bytes(b.try_into().unwrap())).collect::<Vec<_>>()).into())
            } else {
                RasterPixels::Rgba32Float(Arc::new(bytes.chunks_exact(4)
                    .map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect::<Vec<_>>()).into())
            };
            let raw = DecodedRaster::new(f[2].parse().unwrap(), f[3].parse().unwrap(), pixels,
                RasterColorSpace::LinearSrgb).unwrap();
            inputs.push((fixtures.join(f[0]), 0, 1, raw));
        }
        eprintln!("MULTI_ALPHA_PIPELINE cases=32 all_compressions=true explicit_linear=true root=8388608 ram_count=1 swap_count=64");
    }
    if png_tiff {
        inputs.clear();
        // Pixel oracles come from the producer fixtures. Metadata and CMM preparation
        // are shared: this proves transport/rendering, not independent ICC accuracy.
        for name in ["pattern.png", "pattern.tif", "pattern.indexed.png", "pattern.profiled.png"] {
            let path = fixtures.join(name);
            let decoded = rrrah_decode::decode_raster_file(&path).unwrap();
            let color = if matches!(decoded.color_space(), RasterColorSpace::Unspecified) {
                RasterColorSpace::AssumedSrgb
            } else { decoded.color_space().clone() };
            let raw = DecodedRaster::new(16,16,
                RasterPixels::Rgba8(Arc::new(std::fs::read(fixtures.join(format!("{name}.rgba"))).unwrap()).into()), color).unwrap();
            inputs.push((path,0,1,raw));
        }
        // Fixed coefficient reference avoids calling the production xy matrix builder.
        // The authored generator specifies black, red, white with full-range half alpha.
        for (name, primaries, matrix) in [
            ("cicp-linear-p3.png",12, [
                [1.224940176,-0.224940176,0.],
                [-0.042056955,1.042056955,0.],
                [-0.019637555,-0.078636046,1.098273601],
            ]),
            ("cicp-linear-2020.png",9, [
                [1.660491002,-0.587641139,-0.072849863],
                [-0.124550475,1.132899897,-0.008349423],
                [-0.018150763,-0.100578898,1.118729661],
            ]),
        ] {
            let samples: Vec<u16> = vec![0,0,0,32768,65535,0,0,32768,65535,65535,65535,32768];
            let raw = DecodedRaster::new(3,1,RasterPixels::Rgba16(Arc::new(samples.clone()).into()),
                RasterColorSpace::Cicp {primaries,transfer:8,matrix:0,full_range:true}).unwrap();
            let oracle = rrrah_decode::prepare_raster_for_display_with_budget(&raw,None).unwrap();
            let RasterPixels::Rgba32Float(values) = oracle.pixels() else {panic!("linear float required")};
            for (source, result) in samples.chunks_exact(4).zip(values.chunks_exact(4)) {
                for channel in 0..3 {
                    let expected: f64 = (0..3).map(|i|matrix[channel][i]*f64::from(source[i])/65535.).sum();
                    assert!((f64::from(result[channel])-expected).abs()<2e-6,"{name} channel={channel}");
                }
                assert_eq!(result[3],32768_f32/65535.);
            }
            assert!(values[4]>1. && values[5]<0. && values[6]<0.,"extended gamut lost: {name}");
            inputs.push((fixtures.join(name),0,1,raw));
        }
        inputs.extend(png_orientation_cases(dir.path()));
        let path = fixtures.join("oriented.profiled.tif");
        let color = rrrah_decode::decode_raster_file(&path).unwrap().color_space().clone();
        inputs.push((path,0,1,DecodedRaster::new(1,2,
            RasterPixels::Rgba8(Arc::new(vec![255,0,0,255,0,255,0,128]).into()),color).unwrap()));
        for (name, transparent) in [("gray-gamma2-alpha16.png",true),("gray-gamma2-16.tif",false)] {
            let path = fixtures.join(name);
            let color = rrrah_decode::decode_raster_file(&path).unwrap().color_space().clone();
            let pixels: Vec<u16> = [0,32768,32769,65535].into_iter().zip([0,16384,32768,65535])
                .flat_map(|(v,a)| [v,v,v,if transparent {a} else {65535}]).collect();
            inputs.push((path,0,1,DecodedRaster::new(4,1,RasterPixels::Rgba16(Arc::new(pixels).into()),color).unwrap()));
        }
    }
    let count = inputs.len() as u64;
    assert_eq!(count, if png_tiff {17} else if multi_alpha {32} else if gray_alpha {2} else if high_alpha {8} else if producers { 8 } else { 2019 });
    for (path, index, frames, raw) in inputs {
        let raw = raw.with_image_selection(index, frames).unwrap();
        let expected_assumed = raw.color_space() == &RasterColorSpace::AssumedSrgb;
        let oracle = rrrah_decode::prepare_raster_for_display_with_budget(&raw, None).unwrap();
        let mut request = DecodeRequest::new(path);
        request.image_index = index;
        request.memory_budget = Some(root.clone());
        request.assume_untagged_srgb = true;
        if high_alpha {request.assume_untagged_srgb=false;request.assume_untagged_linear_srgb=true;}
        if request.path.file_name().is_some_and(|name| name.to_string_lossy().starts_with("pkm-eac-") || name.to_string_lossy().starts_with("ktx-eac-") || name.to_string_lossy().starts_with("ktx-rg-") || name.to_string_lossy().starts_with("ktx-half-")) {
            request.assume_untagged_linear_srgb = true;
        }
        if request.path.extension().is_some_and(|ext| ext == "rla") {
            request.rla_alpha_mode = Some(rrrah_decode::RlaAlphaMode::Straight);
            request.rla_color_space = Some(rrrah_decode::RlaColorSpace::LinearSrgb);
            if request.path.file_name().unwrap() == "rla-float32-le-hdr-oiio.rla" {
                request.rla_float_byte_order = Some(rrrah_decode::RlaFloatByteOrder::Little);
            }
        }

        let (frame, assumed) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        assert_eq!(assumed, expected_assumed);
        same_pixels(&frame, &oracle);
        assert_eq!((frame.image_index(), frame.image_count()), (index, frames));
        let pressure = root.try_reserve(root.available_bytes()).unwrap();
        let (hit, _) = load_cached_raster_with(&request, None, &mut cache, || {
            panic!("SGI/Sun/PCX/DCX RAM hit decoded")
        })
        .unwrap();
        same_pixels(&hit, &oracle);
        drop(hit);
        drop(pressure);
        let (view, viewport) = params(&oracle);
        let sdr = gpu.map(|g| g.render_raster(&oracle, view, viewport).pixels);
        let hdr = gpu.map(|g| g.render_raster_half(&oracle, view, viewport));
        if let Some(g) = gpu {
            assert_eq!(
                g.render_raster(&frame, view, viewport).pixels,
                *sdr.as_ref().unwrap()
            );
            assert_eq!(
                g.render_raster_half(&frame, view, viewport),
                *hdr.as_ref().unwrap()
            );
        }
        cases.push(Case {
            request,
            window: None,
            assumed,
            frames,
            oracle,
            payload: Vec::new(),
            sdr,
            hdr,
        });
        drop(frame);
        cache.swap().unwrap().wait_idle().unwrap();
    }
    assert_eq!(cases.len() as u64, count);
    let (visible, _) = load_cached_raster_with(&cases[0].request, None, &mut cache, || {
        panic!("SGI/Sun/PCX/DCX/PKM foreground swap decoded")
    })
    .unwrap();
    cache.swap().unwrap().wait_idle().unwrap();
    let gate = Arc::new(DecodeGate::new());
    let ticket = gate.request_foreground();
    let generation = Arc::new(AtomicU64::new(1));
    let (sender, watchdog) = prefetch_watchdog(generation.clone(), Duration::from_secs(30));
    for case in cases.iter().rev() {
        let mut request = case.request.clone();
        request.cancellation = Some(GenerationToken::new(generation.clone(), 1));
        sender.send(true).unwrap();
        let result = preload_raster(&request, None, &mut cache, &gate);
        sender.send(false).unwrap();
        let (restored, assumed) = result.expect("SGI/Sun/PCX/DCX swap must bypass blocked decoder");
        assert_eq!(assumed, case.assumed);
        same_pixels(&restored, &case.oracle);
        assert_eq!(
            (restored.image_index(), restored.image_count()),
            (case.request.image_index, case.frames)
        );
        if let Some(g) = gpu {
            let (view, viewport) = params(&case.oracle);
            assert_eq!(
                g.render_raster(&restored, view, viewport).pixels,
                *case.sdr.as_ref().unwrap()
            );
            assert_eq!(
                g.render_raster_half(&restored, view, viewport),
                *case.hdr.as_ref().unwrap()
            );
        }
        drop(restored);
        assert!(cache.get_visible_lease_for(&visible).is_some());
    }
    drop(sender);
    watchdog.join().unwrap();
    drop(ticket);
    let stats = cache.swap().unwrap().stats();
    assert_eq!(
        (stats.writes, stats.reads, stats.errors, stats.dropped),
        (count, count, 0, 0)
    );
    drop(cache);
    drop(cases);
    drop(visible);
    assert_eq!(root.used(), 0);
    eprintln!(
        "LEGACY_RASTER_PIPELINE cases={count} gpu={} readbacks={} ram_pressure_hits={count} swap_writes={count} swap_reads={count} blocked_decoder=true final=0",
        gpu.is_some(),
        if gpu.is_some() { count * 6 } else { 0 }
    );
}

#[test]
fn legacy_rasters_ram_swap_and_blocked_prefetch() {
    qualify_legacy_rasters(None, RasterCorpus::Legacy);
}
#[test]
#[ignore = "actual Metal SGI/Sun/PCX/DCX/PKM/KTX/KTX2/DDS full-image SDR/HDR cache and swap readback"]
fn legacy_rasters_ram_swap_and_metal() {
    let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual Metal required");
    eprintln!("LEGACY_RASTER_METAL adapter={}", gpu.adapter_name());
    qualify_legacy_rasters(Some(&gpu), RasterCorpus::Legacy);
}

#[test]
fn official_ktx2_producers_ram_swap_and_blocked_prefetch() {
    qualify_legacy_rasters(None, RasterCorpus::KtxProducers);
}
#[test]
#[ignore = "actual Metal official KTX2 producer full-image SDR/HDR RAM/swap readback"]
fn official_ktx2_producers_ram_swap_and_metal() {
    let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual Metal required");
    eprintln!("OFFICIAL_KTX2_METAL adapter={}",gpu.adapter_name());
    qualify_legacy_rasters(Some(&gpu), RasterCorpus::KtxProducers);
}

#[test]
fn original_gray_alpha_ram_swap_and_blocked_prefetch() {
    qualify_legacy_rasters(None,RasterCorpus::GrayAlpha);
}
#[test]
#[ignore = "actual Metal original grayscale PSD/PSB transparency cache and swap"]
fn original_gray_alpha_ram_swap_and_metal() {
    let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual Metal required");
    eprintln!("GRAY_ALPHA_METAL adapter={}",gpu.adapter_name());
    qualify_legacy_rasters(Some(&gpu),RasterCorpus::GrayAlpha);
}

#[test]
fn high_precision_alpha_ram_swap_and_blocked_prefetch() {
    qualify_legacy_rasters(None,RasterCorpus::HighAlpha);
}
#[test]
#[ignore = "actual Metal authored16/32bit RGB/gray PSD/PSB alpha and HDR"]
fn high_precision_alpha_ram_swap_and_metal() {
    let gpu=crate::navigation_metal_readback::qualification_gpu().expect("actual Metal required");
    eprintln!("HIGH_ALPHA_METAL adapter={}",gpu.adapter_name());
    qualify_legacy_rasters(Some(&gpu),RasterCorpus::HighAlpha);
}


#[test]
fn multirow_alpha_ram_swap_and_blocked_prefetch() {
    qualify_legacy_rasters(None, RasterCorpus::MultiAlpha);
}
#[test]
#[ignore = "actual Metal multirow16/32bit alpha PSD/PSB all compression modes"]
fn multirow_alpha_ram_swap_and_metal() {
    let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual Metal required");
    eprintln!("MULTI_ALPHA_METAL adapter={}", gpu.adapter_name());
    qualify_legacy_rasters(Some(&gpu), RasterCorpus::MultiAlpha);
}

#[test]
fn png_tiff_metadata_ram_swap_and_blocked_prefetch() {
    qualify_legacy_rasters(None, RasterCorpus::PngTiffMetadata);
}
#[test]
#[ignore = "actual Metal PNG/TIFF ICC/orientation/16-bit RAM and swap readback"]
fn png_tiff_metadata_ram_swap_and_metal() {
    let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual Metal required");
    eprintln!("PNG_TIFF_METADATA_METAL adapter={}", gpu.adapter_name());
    qualify_legacy_rasters(Some(&gpu), RasterCorpus::PngTiffMetadata);
}
