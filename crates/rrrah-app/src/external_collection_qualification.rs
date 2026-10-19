//! Opt-in transport qualification over an external, metadata-only source manifest.
use super::*;

fn digest(frame: &rrrah_core::DecodedRaster) -> String {
    let mut hash = blake3::Hasher::new();
    hash.update(&frame.width().to_le_bytes());
    hash.update(&frame.height().to_le_bytes());
    hash.update(
        format!(
            "{:?}",
            (
                frame.color_space(),
                frame.alpha_association(),
                frame.image_index(),
                frame.image_count(),
                frame.animation_timing(),
                frame.sample_scale(),
                frame.pixel_aspect(),
                frame.hotspot()
            )
        )
        .as_bytes(),
    );
    match frame.pixels() {
        rrrah_core::RasterPixels::Rgba8(v) => {
            hash.update(v.as_ref());
        }
        rrrah_core::RasterPixels::Rgba16(v) => {
            hash.update(bytemuck::cast_slice(v.as_ref()));
        }
        rrrah_core::RasterPixels::Rgba32Float(v) => {
            hash.update(bytemuck::cast_slice(v.as_ref()));
        }
    }
    hash.finalize().to_hex().to_string()
}

fn refusal_category(error: &RasterLoadError) -> &'static str {
    use rrrah_core::RasterError;
    use rrrah_decode::{RasterColorError, RasterDecodeError};
    match error {
        RasterLoadError::Decode(RasterDecodeError::InvalidAvif(
            "AVIF pixel aspect ratio must be positive and square",
        )) => "nonconforming_avif_pasp",
        RasterLoadError::Decode(RasterDecodeError::InvalidExrAlpha(
            "zero-alpha emission requires associated-alpha rendering",
        )) => "unsupported_valid_exr_associated_emission",
        RasterLoadError::Decode(RasterDecodeError::InvalidExrColor(_)) => {
            "invalid_or_unsupported_exr_color_metadata"
        }
        RasterLoadError::Color(RasterColorError::Frame(RasterError::ColorTransformRequired))
        | RasterLoadError::Decode(RasterDecodeError::Frame(RasterError::ColorTransformRequired))
        | RasterLoadError::Decode(RasterDecodeError::Color(RasterColorError::Frame(
            RasterError::ColorTransformRequired,
        ))) => "qualified_source_color_required",
        _ => "other_refusal",
    }
}

#[test]
#[ignore = "external mixed-source manifest; RRRAH_COLLECTION_MANIFEST and RRRAH_COLLECTION_REPORT"]
fn external_mixed_collection_preserves_navigation_prefetch_ttl_swap_and_pressure() {
    use std::collections::BTreeMap;
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(std::env::var_os("RRRAH_COLLECTION_MANIFEST").unwrap()).unwrap(),
    )
    .unwrap();
    let sources = manifest["sources"].as_array().unwrap();
    assert!(sources.len() >= 100);
    let started = Instant::now();
    let root = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let mut rows = Vec::new();
    let mut valid = Vec::new();
    let mut accepted = BTreeMap::<String, usize>::new();
    for (index, source) in sources.iter().enumerate() {
        let path = PathBuf::from(source["path"].as_str().unwrap());
        eprintln!("COLLECTION_SOURCE index={index} sha256={}", source["sha256"]);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(root.clone());
        let mut row = serde_json::json!({"sha256":source["sha256"],"extension":source["extension"]});
        match load_raster_for_display_typed(&request, None) {
            Ok((frame, assumed)) => {
                row["status"] = "accepted".into();
                row["width"] = frame.width().into();
                row["height"] = frame.height().into();
                row["capacity"] = frame.capacity_bytes().into();
                row["assumed"] = assumed.into();
                row["digest"] = digest(&frame).into();
                *accepted
                    .entry(source["extension"].as_str().unwrap().to_owned())
                    .or_default() += 1;
                if frame.capacity_bytes() <= 16 * 1024 * 1024 {
                    valid.push((
                        path,
                        row["digest"].as_str().unwrap().to_owned(),
                        assumed,
                        rrrah_cache::SwapPayload::minimum_restore_weight(&frame).unwrap(),
                    ));
                    row["navigation_eligible"] = true.into();
                } else {
                    row["navigation_eligible"] = false.into();
                }
            }
            Err(error) => {
                row["status"] = "refused".into();
                row["refusal_category"] = refusal_category(&error).into();
                row["error"] = error
                    .to_string()
                    .replace(&path.display().to_string(), "<source>")
                    .into();
            }
        }
        rows.push(row);
        assert_eq!(root.used(), 0, "source {index} retained managed data");
    }
    let baseline_peak = root.peak();
    let report_path = std::env::var_os("RRRAH_COLLECTION_REPORT").unwrap();
    std::fs::write(
        &report_path,
        serde_json::to_vec_pretty(
            &serde_json::json!({"stage":"baseline_complete","rows":rows,"navigation_sources":valid.len()}),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(valid.len() >= 32, "insufficient displayable bounded sources");
    let directory = tempfile::tempdir().unwrap();
    let swap = rrrah_cache::RasterSwapCache::new_with_budgets(
        directory.path(),
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits {
                max_bytes: 64 * 1024 * 1024,
                max_entries: Some(8),
                ttl: None,
            },
            queue_bytes: 32 * 1024 * 1024,
            queue_count: 2,
            restore_bytes: 32 * 1024 * 1024,
        },
        rrrah_core::MemoryBudget::new(32 * 1024 * 1024),
        root.clone(),
    )
    .unwrap();
    let limits = rrrah_cache::CacheLimits {
        max_bytes: 64 * 1024 * 1024,
        max_entries: Some(2),
        ttl: None,
    };
    let mut cache = RasterDisplayCache::new(limits);
    cache.enable_swap(swap);
    let generation = Arc::new(AtomicU64::new(0));
    let gate = Arc::new(DecodeGate::new());
    let paths: Vec<_> = valid.iter().map(|(p, ..)| p.clone()).collect();
    let varied_windows = std::env::var_os("RRRAH_COLLECTION_VARY_PREFETCH").is_some_and(|v| v == "1");
    let mut window_steps = BTreeMap::<String, usize>::new();
    let mut prefetched_hits = 0usize;
    let partial_bytes = std::env::var_os("RRRAH_COLLECTION_PARTIAL_BYTES").is_some_and(|v| v == "1");
    let mut far_refusals = 0;
    let mut far_byte_refusals = 0;
    let mut ttl_restores = 0;
    let mut max_pre_pressure_used = 0;
    for step in 0..valid.len() {
        let epoch = step as u64 * 2 + 1;
        generation.store(epoch, Ordering::Release);
        let request_for = |path: &Path| {
            let mut r = DecodeRequest::new(path);
            r.memory_budget = Some(root.clone());
            r.cancellation = Some(GenerationToken::new(generation.clone(), epoch));
            r
        };
        let selected = if step % 2 == 0 {
            step / 2
        } else {
            valid.len() - 1 - step / 2
        };
        let request = request_for(&paths[selected]);
        let (visible, assumed) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        assert_eq!(digest(&visible), valid[selected].1);
        assert_eq!(assumed, valid[selected].2);
        let direction = if step % 2 == 0 {
            NavDirection::Forward
        } else {
            NavDirection::Backward
        };
        let window = if varied_windows {
            // Cycle independently of direction: both directions see every policy.
            match (step / 2) % 4 {
                0 => gallery::PrefetchWindow { behind: 0, ahead: 0 },
                1 => gallery::PrefetchWindow { behind: 0, ahead: 3 },
                2 => gallery::PrefetchWindow { behind: 3, ahead: 0 },
                _ => gallery::PrefetchWindow { behind: 1, ahead: 2 },
            }
        } else {
            gallery::PrefetchWindow { behind: 1, ahead: 2 }
        };
        *window_steps
            .entry(format!(
                "{direction:?}:behind={}:ahead={}",
                window.behind, window.ahead
            ))
            .or_default() += 1;
        let neighbours = gallery::neighbour_prefetch_paths(&paths, selected, direction, window);
        let (left, right) = if direction == NavDirection::Forward {
            (window.behind, window.ahead)
        } else {
            (window.ahead, window.behind)
        };
        assert_eq!(
            neighbours.len(),
            left.min(selected) + right.min(paths.len() - selected - 1)
        );
        for path in &neighbours {
            let index = paths.iter().position(|p| p == path).unwrap();
            assert_ne!(index, selected);
            assert!(if index < selected {
                selected - index <= left
            } else {
                index - selected <= right
            });
        }
        let mut protected = Vec::new();
        let partial_headroom = neighbours
            .iter()
            .skip(1)
            .map(|path| valid.iter().find(|(p, ..)| p == path).unwrap().3)
            .min()
            .map(|minimum| minimum / 2);
        let mut active_limits = limits;
        for (n, path) in neighbours.into_iter().enumerate() {
            let before_admission = if partial_bytes && n > 0 {
                if n == 1 {
                    assert_eq!(cache.len(), 2);
                    active_limits = rrrah_cache::CacheLimits {
                        max_bytes: cache.resident_bytes() + partial_headroom.unwrap(),
                        max_entries: None,
                        ttl: None,
                    };
                    assert!(cache.set_limits_and_spill(active_limits));
                }
                // Seed a verified payload so admission must use the swap index,
                // rather than cold decode or the full-count fast refusal.
                let (frame, assumption) = load_raster_for_display_typed(&request_for(&path), None).unwrap();
                let reference = valid.iter().find(|(p, ..)| p == &path).unwrap();
                assert_eq!(digest(&frame), reference.1);
                assert_eq!(assumption, reference.2);
                let mut key = raster_display_key(&request_for(&path), None).unwrap();
                key.6 = assumption;
                assert!(cache.can_admit_key_by_count(&key));
                assert!(cache.can_admit_key_by_minimum_weight(&key, partial_headroom.unwrap()));
                assert!(!cache.can_admit_key_by_minimum_weight(&key, reference.3));
                cache.swap().unwrap().enqueue(key.clone(), frame);
                cache.swap().unwrap().wait_idle().unwrap();
                assert_eq!(
                    cache.swap().unwrap().minimum_restore_weight(&key),
                    Some(reference.3)
                );
                Some((cache.swap().unwrap().stats().reads, root.used(), root.peak()))
            } else {
                None
            };
            match preload_raster_with_lease(&request_for(&path), None, &mut cache, &gate) {
                Ok((frame, assumption, lease)) => {
                    let reference = valid.iter().find(|(p, ..)| p == &path).unwrap();
                    assert_eq!(digest(&frame), reference.1);
                    assert_eq!(assumption, reference.2);
                    if n == 0 {
                        assert!(lease.is_some());
                        // Prove the warmed item can be navigated to without decoding.
                        let (hit, hit_assumed) =
                            load_cached_raster_with(&request_for(&path), None, &mut cache, || {
                                panic!("nearest prefetched neighbour decoded again")
                            })
                            .unwrap();
                        assert_eq!(digest(&hit), reference.1);
                        assert_eq!(hit_assumed, reference.2);
                        // This visible lookup must not steal protection from selection.
                        let mut selected_key = raster_display_key(&request, None).unwrap();
                        selected_key.6 = assumed;
                        cache.mark_visible(selected_key);
                        prefetched_hits += 1;
                    }
                    if let Some(lease) = lease {
                        protected.push(lease);
                    }
                }
                Err(error) => {
                    assert!(n > 0, "nearest: {error}");
                    if partial_bytes {
                        assert_eq!(error, "raster prefetch skipped: protected byte budget exhausted");
                        far_byte_refusals += 1;
                    } else {
                        assert_eq!(error, "raster prefetch skipped: all count slots are protected");
                        far_refusals += 1;
                    }
                }
            }
            if let Some(before) = before_admission {
                assert_eq!(
                    (cache.swap().unwrap().stats().reads, root.used(), root.peak()),
                    before
                );
            }
            assert!(cache.swap().unwrap().stats().queued_bytes <= 32 * 1024 * 1024);
            assert!(cache.len() <= 2);
            assert!(cache.resident_bytes() <= active_limits.max_bytes);
        }
        max_pre_pressure_used = max_pre_pressure_used.max(root.used());
        let blocker = root.try_reserve(root.available_bytes()).unwrap();
        let (hit, _) = load_cached_raster_with(&request, None, &mut cache, || {
            panic!("visible RAM hit decoded under full pressure")
        })
        .unwrap();
        assert_eq!(digest(&hit), valid[selected].1);
        drop(hit);
        drop(blocker);
        generation.store(epoch + 1, Ordering::Release);
        let before = (cache.len(), cache.resident_bytes());
        assert!(preload_raster(&request, None, &mut cache, &gate).is_err());
        assert_eq!((cache.len(), cache.resident_bytes()), before);
        let ttl_frame = if step % 16 == 0 {
            Some(visible.clone())
        } else {
            None
        };
        drop(protected);
        drop(visible);
        assert!(cache.set_limits_and_spill(limits));
        if step % 16 == 0 {
            cache.swap().unwrap().wait_idle().unwrap();
            let mut key = raster_display_key(&request, None).unwrap();
            key.6 = assumed;
            let mut sentinel = key.clone();
            sentinel.4 = usize::MAX;
            cache.mark_visible(sentinel.clone());
            assert!(cache.set_limits_and_spill(rrrah_cache::CacheLimits {
                max_bytes: 0,
                max_entries: Some(0),
                ttl: None
            }));
            cache.swap().unwrap().wait_idle().unwrap();
            // TTL is fixed at insertion; changing policy does not rewrite old
            // deadlines. Reinsert this held frame under the zero-TTL policy.
            assert!(cache.set_limits_and_spill(rrrah_cache::CacheLimits {
                ttl: Some(Duration::ZERO),
                ..limits
            }));
            assert!(cache.insert_visible(key.clone(), ttl_frame.unwrap()));
            cache.mark_visible(sentinel);
            assert_eq!(cache.spill_expired(), 1);
            cache.swap().unwrap().wait_idle().unwrap();
            assert_eq!(cache.len(), 0);
            let blocker = root.try_reserve(root.available_bytes()).unwrap();
            assert!(matches!(
                cache.swap().unwrap().try_get(&key, || false),
                Err(rrrah_core::BufferError::Capacity { .. })
            ));
            drop(blocker);
            let mut fresh = request.clone();
            fresh.cancellation = None;
            let (restored, restored_assumed) = load_cached_raster_with(&fresh, None, &mut cache, || {
                panic!("TTL swap entry decoded again")
            })
            .unwrap();
            assert_eq!(digest(&restored), valid[selected].1);
            assert_eq!(restored_assumed, valid[selected].2);
            drop(restored);
            ttl_restores += 1;
            assert!(cache.set_limits_and_spill(limits));
        }
        if step % 32 == 0 {
            eprintln!(
                "COLLECTION_NAV step={step} root={} peak={}",
                root.used(),
                root.peak()
            );
        }
    }
    cache.swap().unwrap().wait_idle().unwrap();
    let stats = cache.swap().unwrap().stats();
    let disk = cache.swap().unwrap().disk_usage().unwrap();
    assert!(disk.bytes <= 64 * 1024 * 1024 && disk.objects <= 8);
    assert_eq!(stats.queued_bytes, 0);
    assert_eq!(stats.errors, 0);
    assert!(far_refusals + far_byte_refusals > 0 && ttl_restores > 0);
    assert!(stats.reads >= ttl_restores);
    cache.swap().unwrap().shutdown();
    drop(cache);
    assert_eq!(root.used(), 0);
    let mut refusal_categories = BTreeMap::<String, usize>::new();
    for row in &rows {
        if let Some(category) = row["refusal_category"].as_str() {
            *refusal_categories.entry(category.to_owned()).or_default() += 1;
        }
    }
    let report = serde_json::json!({"scope":"external content-unique corpus; strict application color defaults; same-decoder baseline hashes test transport, not independent color correctness",
        "legacy_prefetch_swap_reads":std::env::var("RRRAH_BENCH_LEGACY_PREFETCH_SWAP").as_deref()==Ok("1"),"attempted":sources.len(),"refusal_categories":refusal_categories,"accepted_extensions":accepted,"navigation_sources":valid.len(),"far_count_refusals":far_refusals,"partial_byte_pressure":partial_bytes,"far_byte_refusals":far_byte_refusals,"ttl_restores":ttl_restores,"varied_prefetch_windows":varied_windows,"window_steps":window_steps,"nearest_navigation_hits":prefetched_hits,
        "swap":{"writes":stats.writes,"reads":stats.reads,"dropped":stats.dropped,"errors":stats.errors},"disk_debug":format!("{disk:?}"),
        "baseline_peak":baseline_peak,"max_observed_pre_pressure_used":max_pre_pressure_used,"root_peak_includes_synthetic_pressure":true,"root_peak":root.peak(),"root_limit":root.limit(),"root_final":root.used(),"seconds":started.elapsed().as_secs_f64(),"rows":rows});
    std::fs::write(
        std::env::var_os("RRRAH_COLLECTION_REPORT").unwrap(),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    eprintln!(
        "COLLECTION_DONE attempted={} navigation={} root_peak={} final=0",
        sources.len(),
        valid.len(),
        root.peak()
    );
}
