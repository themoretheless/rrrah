//! Explicit large-file qualification through the production segmented viewport codec.
use super::*;

#[test]
#[ignore = "manual 36MP PSD display cache, segmented swap and actual Metal qualification"]
fn psd_36mp_prepared_cache_segmented_swap_and_metal() {
    qualify_large_psd(false);
}
#[test]
#[ignore = "manual 36MP ICC PSD display cache, async viewport, segmented swap and Metal qualification"]
fn icc_psd_36mp_prepared_cache_segmented_swap_and_metal() {
    qualify_large_psd(true);
}
fn qualify_large_psd(icc: bool) {
    use std::io::Write;
    let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual Metal required");
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("large.psd");
    let (width, height) = (6000usize, 6000usize);
    let pattern = icc.then(|| {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/pattern.profiled.png");
        rrrah_decode::decode_raster(&DecodeRequest::new(fixture)).unwrap()
    });
    let mut resource = Vec::new();
    let profile_bytes = if let Some(pattern) = &pattern {
        let rrrah_core::RasterColorSpace::Icc(profile) = pattern.color_space() else {
            panic!()
        };
        resource.extend_from_slice(b"8BIM");
        resource.extend(1039u16.to_be_bytes());
        resource.extend([0, 0]);
        resource.extend((profile.len() as u32).to_be_bytes());
        resource.extend(profile);
        if profile.len() % 2 != 0 {
            resource.push(0);
        }
        profile.len() as u64
    } else {
        0
    };
    let mut header = b"8BPS".to_vec();
    header.extend(1u16.to_be_bytes());
    header.extend([0; 6]);
    header.extend(3u16.to_be_bytes());
    header.extend((height as u32).to_be_bytes());
    header.extend((width as u32).to_be_bytes());
    header.extend(8u16.to_be_bytes());
    header.extend(3u16.to_be_bytes());
    header.extend(0u32.to_be_bytes());
    header.extend((resource.len() as u32).to_be_bytes());
    header.extend(resource);
    header.extend(0u32.to_be_bytes());
    header.extend(0u16.to_be_bytes());
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(&header).unwrap();
    let mut row = vec![0u8; width];
    for channel in 0..3 {
        for y in 0..height {
            for (x, value) in row.iter_mut().enumerate() {
                *value = if let Some(pattern) = &pattern {
                    let rrrah_core::RasterPixels::Rgba8(pixels) = pattern.pixels() else {
                        panic!()
                    };
                    pixels[((y % 16) * 16 + x % 16) * 4 + channel]
                } else {
                    (match channel {
                        0 => x,
                        1 => y,
                        _ => x + y,
                    } % 251) as u8
                };
            }
            file.write_all(&row).unwrap();
        }
    }
    file.sync_all().unwrap();
    drop(file);
    drop(row);
    let color_peak = 720_000_000 + profile_bytes + if icc { (width * 3 * 8 * 2) as u64 } else { 0 };
    let next_path = directory.path().join("next.psd");
    let mut next_header = header.clone();
    next_header[14..18].copy_from_slice(&3000u32.to_be_bytes());
    next_header[18..22].copy_from_slice(&3000u32.to_be_bytes());
    let mut next_file = std::fs::File::create(&next_path).unwrap();
    next_file.write_all(&next_header).unwrap();
    let mut next_row = vec![0u8; 3000];
    for channel in 0..3 {
        for y in 0..3000 {
            for (x, value) in next_row.iter_mut().enumerate() {
                *value = if let Some(pattern) = &pattern {
                    let rrrah_core::RasterPixels::Rgba8(pixels) = pattern.pixels() else {
                        panic!()
                    };
                    pixels[((y % 16) * 16 + x % 16) * 4 + channel]
                } else {
                    (match channel {
                        0 => x,
                        1 => y,
                        _ => x + y,
                    } % 251) as u8
                };
            }
            next_file.write_all(&next_row).unwrap();
        }
    }
    next_file.sync_all().unwrap();
    drop(next_file);
    drop(next_row);
    let root = rrrah_core::MemoryBudget::new(color_peak);
    let mut request = DecodeRequest::new(&path);
    request.memory_budget = Some(root.clone());
    request.assume_untagged_srgb = true;
    let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(576_000_000));
    let (frame, assumed) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
    assert_eq!(assumed, !icc);
    assert_eq!((root.used(), root.peak()), (576_000_000, color_peak));
    let pressure = root.try_reserve(root.available_bytes()).unwrap();
    let (hit, _) =
        load_cached_raster_with(&request, None, &mut cache, || panic!("large PSD RAM hit decoded")).unwrap();
    let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
        (frame.pixels(), hit.pixels())
    else {
        panic!()
    };
    assert!(a.ptr_eq(b));
    drop(hit);
    drop(pressure);
    let mut next_request = request.clone();
    next_request.path = next_path.clone();
    let gate = Arc::new(DecodeGate::new());
    assert!(
        preload_raster(&next_request, None, &mut cache, &gate).is_err(),
        "large visible pin must bound background memory"
    );
    assert_eq!(root.used(), 576_000_000);
    let (retained, _) = load_cached_raster_with(&request, None, &mut cache, || {
        panic!("failed neighbour prefetch lost visible PSD")
    })
    .unwrap();
    let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
        (frame.pixels(), retained.pixels())
    else {
        panic!()
    };
    assert!(a.ptr_eq(b));
    drop(retained);
    let rectangles = [[0u32, 0, 17, 11], [2991, 2994, 17, 11], [5983, 5989, 17, 11]];
    let expected: Vec<_> = rectangles
        .iter()
        .map(|&[x, y, w, h]| {
            let mut values = Vec::with_capacity((w * h * 4) as usize);
            for row in y..y + h {
                let start = (row as usize * width + x as usize) * 4;
                values.extend_from_slice(&a[start..start + w as usize * 4]);
            }
            rrrah_core::DecodedRaster::new(
                w,
                h,
                rrrah_core::RasterPixels::Rgba32Float(Arc::new(values).into()),
                rrrah_core::RasterColorSpace::LinearSrgb,
            )
            .unwrap()
        })
        .collect();
    let lod_target = [1280u32, 720u32];
    let lod_bytes = u64::from(lod_target[0]) * u64::from(lod_target[1]) * 16;
    let lod_reference = frame
        .downsample_linear_with_budget_and_cancel(
            lod_target,
            &rrrah_core::MemoryBudget::new(lod_bytes),
            || false,
        )
        .unwrap();
    assert!(
        rrrah_cache::raster_payload_len(&frame).is_err(),
        "full payload cap remains explicit"
    );
    fn files_under(path: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                files.extend(files_under(&entry.path()));
            } else {
                files.push(entry.path());
            }
        }
        files.sort();
        files
    }
    for (label, disk, count, index_limit) in [
        ("disk-zero", 0, Some(1), 300_000),
        ("disk-short", 576_000_063, Some(1), 300_000),
        ("count-zero", 576_000_064, Some(0), 300_000),
        ("index-zero", 576_000_064, Some(1), 0),
        ("index-short", 576_000_064, Some(1), 281_279),
    ] {
        let rejected_directory = tempfile::tempdir().unwrap();
        let rejected = rrrah_swap::SwapStore::new(
            rejected_directory.path(),
            rrrah_swap::SwapLimits {
                max_bytes: disk,
                max_objects: count,
            },
        )
        .unwrap();
        let before = files_under(rejected_directory.path());
        let index = root.child(index_limit);
        assert!(
            rrrah_cache::SwappedRaster::write(&rejected, &frame, &index, || false).is_err(),
            "{label}"
        );
        assert_eq!(index.used(), 0);
        assert_eq!(root.used(), 576_000_000);
        let usage = rejected.usage().unwrap();
        assert_eq!((usage.bytes, usage.objects), (0, 0));
        assert_eq!(
            files_under(rejected_directory.path()),
            before,
            "{label} left temporary files"
        );
        let pressure = root.try_reserve(root.available_bytes()).unwrap();
        let (retained, _) = load_cached_raster_with(&request, None, &mut cache, || {
            panic!("swap refusal lost visible PSD")
        })
        .unwrap();
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (frame.pixels(), retained.pixels())
        else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        drop(retained);
        drop(pressure);
        eprintln!(
            "PSD_36MP_DISK_REFUSAL case={label} disk={disk} count={count:?} index_limit={index_limit} ram_retained=true files_unchanged=true final_temporary_credit=0"
        );
    }
    let store = rrrah_swap::SwapStore::new(
        directory.path(),
        rrrah_swap::SwapLimits {
            max_bytes: 600_000_000,
            max_objects: Some(2),
        },
    )
    .unwrap();
    let index = root.child(300_000);
    let swapped = rrrah_cache::SwappedRaster::write(&store, &frame, &index, || false).unwrap();
    let viewport_budget = root.child(32 * 1024 * 1024);
    let preview = frame
        .downsample_linear_with_budget_and_cancel([720, 720], &viewport_budget, || false)
        .unwrap();
    let fit = rrrah_gpu::ViewParameters {
        viewport: [1280., 720.],
        ..Default::default()
    };
    let fit_reference = gpu.render_raster_half(&preview, fit, [1280, 720]);
    let texture_budget = rrrah_core::MemoryBudget::new(16 * 1024 * 1024);
    let upload_budget = rrrah_core::MemoryBudget::new(16 * 1024 * 1024);
    let mut viewport_renderer =
        rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&gpu.device, texture_budget.clone())
            .with_upload_queue_budget(upload_budget.clone());
    let mut resident = crate::raster_viewport::ResidentSource::install_with_prepared_lod(
        &mut viewport_renderer,
        &gpu.device,
        &gpu.queue,
        &frame,
        fit,
        None,
        &viewport_budget,
        Some(preview),
        || false,
    )
    .unwrap()
    .expect("16MiB texture quota must select viewport fallback");
    viewport_renderer.update_view(&gpu.queue, fit);
    assert_eq!(
        gpu.render_existing_raster_half(&viewport_renderer, [1280, 720]),
        fit_reference
    );
    assert!(!resident.adopt_swapped(resident.identity() + 1, swapped.clone()));
    assert!(resident.adopt_swapped(resident.identity(), swapped.clone()));
    assert!(resident.is_swapped());
    assert!(resident.persistence_source().is_none());
    let mut viewport_cases = Vec::new();
    for pan in [-256., 0., 256.] {
        let view = rrrah_gpu::ViewParameters {
            zoom: 32.,
            pan: [pan, 0.],
            ..fit
        };
        let (_, rect, _, _) = resident
            .region_job(&viewport_renderer, &gpu.device, view)
            .unwrap()
            .unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(source) = frame.pixels() else {
            panic!()
        };
        let mut values = Vec::with_capacity((rect[2] * rect[3] * 4) as usize);
        for y in rect[1]..rect[1] + rect[3] {
            let start = (y as usize * width + rect[0] as usize) * 4;
            values.extend_from_slice(&source[start..start + rect[2] as usize * 4]);
        }
        let crop = rrrah_core::DecodedRaster::new(
            rect[2],
            rect[3],
            rrrah_core::RasterPixels::Rgba32Float(Arc::new(values).into()),
            rrrah_core::RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let mut reference = rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(
            &gpu.device,
            rrrah_core::MemoryBudget::new(16 * 1024 * 1024),
        );
        reference
            .upload_packed_region_with_cancel(
                &gpu.device,
                &gpu.queue,
                &crop,
                [6000, 6000],
                [rect[0], rect[1]],
                || false,
            )
            .unwrap();
        reference.update_view(&gpu.queue, view);
        viewport_cases.push((
            view,
            rect,
            gpu.render_existing_raster_half(&reference, [1280, 720]),
        ));
    }
    drop(frame);
    drop(cache);
    assert_eq!(root.used(), index.used() + 720 * 720 * 16);
    assert!(index.used() < 300_000);
    let mut next_cache = RasterDisplayCache::new(rrrah_cache::CacheLimits {
        max_bytes: 144_000_000,
        max_entries: Some(1),
        ttl: None,
    });
    let (neighbour, next_assumed) = preload_raster(&next_request, None, &mut next_cache, &gate).unwrap();
    assert_eq!((neighbour.width(), neighbour.height()), (3000, 3000));
    assert_eq!(next_assumed, !icc);
    let pressure = root.try_reserve(root.available_bytes()).unwrap();
    let (next_hit, _) = load_cached_raster_with(&next_request, None, &mut next_cache, || {
        panic!("recovered neighbour prefetch decoded again")
    })
    .unwrap();
    let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
        (neighbour.pixels(), next_hit.pixels())
    else {
        panic!()
    };
    assert!(a.ptr_eq(b));
    drop(next_hit);
    drop(pressure);
    drop(neighbour);
    drop(next_cache);
    assert_eq!(root.used(), index.used() + 720 * 720 * 16);
    std::fs::remove_file(&next_path).unwrap();
    eprintln!(
        "PSD_LARGE_NEIGHBOUR visible_pixels=36000000 next_pixels=9000000 pin_pressure_refused=true recovered_after_migration=true full_pressure_ram_hit=true temporary_credit=0"
    );
    std::fs::remove_file(&path).unwrap();
    assert_eq!(store.usage().unwrap().bytes, 576_000_064);
    let output = root.child(100_000);
    let scratch = output.child(65_536);
    let view = rrrah_gpu::ViewParameters {
        viewport: [128., 96.],
        ..Default::default()
    };
    for (rectangle, expected) in rectangles.into_iter().zip(&expected) {
        let region = swapped
            .restore_region(rectangle, &output, &scratch, || false)
            .unwrap();
        assert_eq!(region.source_size, [6000, 6000]);
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (region.frame.pixels(), expected.pixels())
        else {
            panic!()
        };
        assert_eq!(
            a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            b.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
        assert_eq!(output.used(), 2992);
        assert_eq!(
            gpu.render_raster(&region.frame, view, [128, 96]).pixels,
            gpu.render_raster(expected, view, [128, 96]).pixels
        );
        assert_eq!(
            gpu.render_raster_half(&region.frame, view, [128, 96]),
            gpu.render_raster_half(expected, view, [128, 96])
        );
        drop(region);
        assert_eq!(output.used(), 0);
        let crop_peak = output.peak();
        for limit in [crop_peak - 1, crop_peak] {
            let limited = root.child(limit);
            let scratch = limited.child(65536);
            let result = swapped.restore_region(rectangle, &limited, &scratch, || false);
            if limit < crop_peak {
                assert!(result.is_err(), "one-byte-short crop quota must refuse");
            } else {
                let crop = result.unwrap();
                assert_eq!(limited.used(), 2992);
                let held = crop.frame.clone();
                drop(crop);
                assert_eq!(limited.used(), 2992);
                assert!(
                    swapped
                        .restore_region(rectangle, &limited, &scratch, || false)
                        .is_err(),
                    "pinned crop must retain its credit"
                );
                drop(held);
            }
            assert_eq!(limited.used(), 0);
            assert_eq!(scratch.used(), 0);
        }
        let cancelled = root.child(crop_peak);
        let cancel_scratch = cancelled.child(65536);
        let mut observed_admission = false;
        let result = swapped.restore_region(rectangle, &cancelled, &cancel_scratch, || {
            observed_admission |= cancelled.used() == crop_peak;
            observed_admission
        });
        assert!(observed_admission);
        assert!(matches!(
            result,
            Err(rrrah_cache::RasterRegionError::Swap(
                rrrah_swap::SwapError::Cancelled
            ))
        ));
        assert_eq!((cancelled.used(), cancel_scratch.used()), (0, 0));
        let recovered = swapped
            .restore_region(rectangle, &cancelled, &cancel_scratch, || false)
            .unwrap();
        assert_eq!(
            gpu.render_raster(&recovered.frame, view, [128, 96]).pixels,
            gpu.render_raster(expected, view, [128, 96]).pixels
        );
        assert_eq!(
            gpu.render_raster_half(&recovered.frame, view, [128, 96]),
            gpu.render_raster_half(expected, view, [128, 96])
        );
        drop(recovered);
        assert_eq!((cancelled.used(), cancel_scratch.used()), (0, 0));
        eprintln!(
            "PSD_36MP_CROP_QUOTA rect={rectangle:?} exact_peak={crop_peak} short_refused=true cancel_after_admission=true retry=true"
        );
    }
    let mut worker = crate::raster_lod_worker::LodWorker::default();
    let (wake_tx, wake_rx) = crossbeam_channel::bounded(1);
    worker.set_waker(move || {
        let _ = wake_tx.try_send(());
    });
    let (started_tx, started_rx) = crossbeam_channel::bounded(1);
    let (release_tx, release_rx) = crossbeam_channel::bounded(1);
    let first_job = std::sync::atomic::AtomicBool::new(true);
    worker.before_filter = Some(Arc::new(move |_| {
        if first_job.swap(false, std::sync::atomic::Ordering::AcqRel) {
            started_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        }
    }));
    let epoch = Arc::new(std::sync::atomic::AtomicU64::new(resident.identity()));
    let token = |generation| rrrah_decode::GenerationToken::new(epoch.clone(), generation);
    let old = resident.identity();
    assert!(
        !resident
            .refresh_swapped(
                &mut viewport_renderer,
                &gpu.device,
                &gpu.queue,
                viewport_cases[0].0,
                &mut worker,
                token(old)
            )
            .unwrap()
    );
    started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(
        gpu.render_existing_raster_half(&viewport_renderer, [1280, 720]),
        fit_reference,
        "pending background work must not replace fit preview"
    );
    epoch.store(old + 1, std::sync::atomic::Ordering::Release);
    assert!(
        !resident
            .refresh_swapped(
                &mut viewport_renderer,
                &gpu.device,
                &gpu.queue,
                viewport_cases[2].0,
                &mut worker,
                token(old + 1)
            )
            .unwrap()
    );
    release_tx.send(()).unwrap();
    wake_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(
        resident
            .refresh_swapped(
                &mut viewport_renderer,
                &gpu.device,
                &gpu.queue,
                viewport_cases[2].0,
                &mut worker,
                token(old + 1)
            )
            .unwrap()
    );
    viewport_renderer.update_view(&gpu.queue, viewport_cases[2].0);
    assert_eq!(
        gpu.render_existing_raster_half(&viewport_renderer, [1280, 720]),
        viewport_cases[2].2
    );
    assert!(
        wake_rx.try_recv().is_err(),
        "obsolete request must not publish an extra completion"
    );
    for (view, expected_rect, expected) in viewport_cases {
        let (_, rect, _, _) = resident
            .region_job(&viewport_renderer, &gpu.device, view)
            .unwrap()
            .unwrap();
        assert_eq!(rect, expected_rect);
        assert!(
            !resident
                .refresh_swapped(
                    &mut viewport_renderer,
                    &gpu.device,
                    &gpu.queue,
                    view,
                    &mut worker,
                    token(old + 1)
                )
                .unwrap()
        );
        wake_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(
            resident
                .refresh_swapped(
                    &mut viewport_renderer,
                    &gpu.device,
                    &gpu.queue,
                    view,
                    &mut worker,
                    token(old + 1)
                )
                .unwrap()
        );
        viewport_renderer.update_view(&gpu.queue, view);
        assert_eq!(
            gpu.render_existing_raster_half(&viewport_renderer, [1280, 720]),
            expected
        );
        assert!(
            resident
                .region_job(&viewport_renderer, &gpu.device, view)
                .unwrap()
                .is_none(),
            "contained region must not upload again"
        );
        assert!(
            !resident
                .refresh_swapped(
                    &mut viewport_renderer,
                    &gpu.device,
                    &gpu.queue,
                    view,
                    &mut worker,
                    token(old + 1)
                )
                .unwrap()
        );
        assert!(wake_rx.try_recv().is_err());
    }
    drop(worker);
    drop(resident);
    drop(viewport_renderer);
    assert_eq!(
        (
            texture_budget.used(),
            upload_budget.used(),
            viewport_budget.used()
        ),
        (0, 0, 0)
    );
    eprintln!(
        "PSD_36MP_RESIDENT_VIEWPORT fit=720x720 pan_states=4 full_readbacks=10 async_worker=true stale_job_rejected=true texture_limit=16777216 source_released=true stale_adoption_refused=true"
    );
    let lod_view = rrrah_gpu::ViewParameters {
        viewport: lod_target.map(|v| v as f32),
        ..Default::default()
    };
    let lod_peak = lod_bytes + 6000 * 16 + u64::from(lod_target[0]) * 32 + 65536;
    let short = root.child(lod_peak - 1);
    let short_scratch = short.child(65536);
    assert!(
        swapped
            .downsample_linear(lod_target, &short, &short_scratch, || false)
            .is_err()
    );
    assert_eq!((short.used(), short_scratch.used()), (0, 0));
    let lod_budget = root.child(lod_peak);
    let lod_scratch = lod_budget.child(65536);
    let lod = swapped
        .downsample_linear(lod_target, &lod_budget, &lod_scratch, || false)
        .unwrap();
    assert_eq!((lod_budget.used(), lod_budget.peak()), (lod_bytes, lod_peak));
    let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
        (lod.pixels(), lod_reference.pixels())
    else {
        panic!()
    };
    assert_eq!(
        a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        b.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
    );
    assert_eq!(
        gpu.render_raster(&lod, lod_view, lod_target).pixels,
        gpu.render_raster(&lod_reference, lod_view, lod_target).pixels
    );
    assert_eq!(
        gpu.render_raster_half(&lod, lod_view, lod_target),
        gpu.render_raster_half(&lod_reference, lod_view, lod_target)
    );
    drop(lod);
    assert_eq!(lod_budget.used(), 0);
    assert_eq!(store.usage().unwrap().objects, 2);
    // This cap cannot admit a source row + output + checksum scratch, so a
    // successful second call must use the persisted exact level.
    let cached_budget = root.child(lod_bytes + 65536 + 64);
    let cached_scratch = cached_budget.child(65536);
    let cached = swapped
        .downsample_linear(lod_target, &cached_budget, &cached_scratch, || false)
        .unwrap();
    assert_eq!(
        gpu.render_raster(&cached, lod_view, lod_target).pixels,
        gpu.render_raster(&lod_reference, lod_view, lod_target).pixels
    );
    assert_eq!(
        gpu.render_raster_half(&cached, lod_view, lod_target),
        gpu.render_raster_half(&lod_reference, lod_view, lod_target)
    );
    assert_eq!(cached_budget.used(), lod_bytes);
    drop(cached);
    assert_eq!((cached_budget.used(), cached_scratch.used()), (0, 0));
    eprintln!(
        "PSD_36MP_LOD target=1280x720 output_bytes=14745600 exact_peak={lod_peak} cached_peak={} cache_hit_below_streaming_quota=true readbacks=8",
        cached_budget.peak()
    );
    drop(swapped);
    drop(store);
    assert_eq!(root.used(), 0);
    eprintln!(
        "PSD_36MP_SEGMENTED_METAL adapter={} icc={icc} native_color_peak={color_peak} disk=576000064 crops=3 readbacks=42 crop_bytes=2992 scratch_peak={} final=0",
        gpu.adapter_name(),
        scratch.peak()
    );
}
