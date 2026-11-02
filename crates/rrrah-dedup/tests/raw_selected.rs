#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::decode::decode_selected_frame;

#[test]
fn explicit_raw_recipes_have_separate_memory_and_persisted_cache_entries() {
    use rrrah_decode::RawDevelopmentRecipe;
    use rrrah_dedup::{
        cache::FingerprintCache,
        decode::{FingerprintPolicy, fingerprint_file},
    };
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("sensor.dng");
    let disk = root.path().join("fingerprints.bin");
    let mut bytes = color_fixture(true, true);
    for (index, value) in (1000_u16..1016).enumerate() {
        bytes[512 + index * 2..514 + index * 2].copy_from_slice(&value.to_le_bytes());
    }
    std::fs::write(&path, bytes).unwrap();
    let native = DecodeRequest::new(&path);
    assert_eq!(native.raw_development_recipe, RawDevelopmentRecipe::NativeDefault);
    let mut bilinear = native.clone();
    bilinear.raw_development_recipe = RawDevelopmentRecipe::Bilinear;
    let policy = FingerprintPolicy {
        recipe: [0; 32],
        max_file_bytes: 1024,
        max_pixels: 16,
        max_frames: 1,
        max_cache_entries: 8,
    };
    let budget = MemoryBudget::new(32 * 1024 * 1024);
    let mut cache = FingerprintCache::default();
    let a = fingerprint_file(&native, policy, &budget, &mut cache, || false).unwrap();
    let b = fingerprint_file(&bilinear, policy, &budget, &mut cache, || false).unwrap();
    assert!(!a.cache_hit && !b.cache_hit);
    for (request, expected) in [(&native, &a.fingerprint), (&bilinear, &b.fingerprint)] {
        let warm = fingerprint_file(request, policy, &budget, &mut cache, || false).unwrap();
        assert!(warm.cache_hit);
        assert_eq!(&warm.fingerprint, expected);
    }
    cache.save_atomic(&disk, || false).unwrap();
    let mut loaded = FingerprintCache::load_file(&disk, 8, || false).unwrap();
    for (request, expected) in [(&bilinear, &b.fingerprint), (&native, &a.fingerprint)] {
        let warm = fingerprint_file(request, policy, &budget, &mut loaded, || false).unwrap();
        assert!(warm.cache_hit);
        assert_eq!(&warm.fingerprint, expected);
    }
    assert_eq!(budget.used(), 0);
}
// Authored TIFF/DNG construction follows the existing native backend test fixture.
fn color_fixture(with_wb: bool, with_matrix: bool) -> Vec<u8> {
    let mut bytes = vec![0; 544];
    bytes[..8].copy_from_slice(b"II*\0\x08\0\0\0");
    let mut entries = vec![
        (256_u16, 4_u16, 1_u32, 4_u32),
        (257, 4, 1, 4),
        (258, 3, 1, 16),
        (259, 3, 1, 1),
        (262, 3, 1, 32803),
        (273, 4, 1, 512),
        (274, 3, 1, 1),
        (277, 3, 1, 1),
        (278, 4, 1, 4),
        (279, 4, 1, 32),
        (33421, 3, 2, 0x0002_0002),
        (33422, 1, 4, 0x0201_0100),
        (50706, 1, 4, 0x0000_0401),
        (50708, 2, 5, 300),
    ];
    bytes[300..305].copy_from_slice(b"TEST\0");
    if with_matrix {
        entries.push((50721, 10, 9, 320));
        for (i, value) in [6000_i32, -1000, -1000, -5000, 15000, 1000, -1000, 2000, 6000]
            .iter()
            .enumerate()
        {
            bytes[320 + i * 8..324 + i * 8].copy_from_slice(&value.to_le_bytes());
            bytes[324 + i * 8..328 + i * 8].copy_from_slice(&10000_i32.to_le_bytes());
        }
    }
    if with_wb {
        entries.push((50728, 5, 3, 400));
        for (i, (n, d)) in [(1_u32, 2_u32), (1, 1), (1, 4)].iter().enumerate() {
            bytes[400 + i * 8..404 + i * 8].copy_from_slice(&n.to_le_bytes());
            bytes[404 + i * 8..408 + i * 8].copy_from_slice(&d.to_le_bytes());
        }
    }
    entries.sort_unstable_by_key(|e| e.0);
    bytes[8..10].copy_from_slice(&u16::try_from(entries.len()).unwrap().to_le_bytes());
    for (i, (tag, kind, count, value)) in entries.iter().enumerate() {
        let at = 10 + i * 12;
        bytes[at..at + 2].copy_from_slice(&tag.to_le_bytes());
        bytes[at + 2..at + 4].copy_from_slice(&kind.to_le_bytes());
        bytes[at + 4..at + 8].copy_from_slice(&count.to_le_bytes());
        bytes[at + 8..at + 12].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[test]
fn sensor_dng_development_preserves_copy_and_changed_mosaic_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sensor.dng");
    let mut original = color_fixture(true, true);
    for (index, value) in (1000_u16..1016).enumerate() {
        original[512 + index * 2..514 + index * 2].copy_from_slice(&value.to_le_bytes());
    }
    std::fs::write(&path, &original).unwrap();
    let request = DecodeRequest::new(&path);
    assert_eq!(
        rrrah_decode::image_source_kind(&request).unwrap(),
        rrrah_decode::ImageSourceKind::Sensor
    );
    let short = MemoryBudget::new(4 * 1024 * 1024);
    assert!(matches!(
        decode_selected_frame(&request, 16, &short, || false),
        Err(rrrah_dedup::decode::FileError::Develop(
            rrrah_core::develop::DevelopError::Buffer(_)
        ))
    ));
    assert_eq!(short.used(), 0);
    let budget = MemoryBudget::new(32 * 1024 * 1024);
    let base = decode_selected_frame(&request, 16, &budget, || false).unwrap();
    let copy = decode_selected_frame(&request, 16, &budget, || false).unwrap();
    assert!(base.same_selected_frame(&copy, || false).unwrap());
    assert_eq!(
        base.selected_frame_digest(|| false).unwrap(),
        copy.selected_frame_digest(|| false).unwrap()
    );
    drop(copy);
    let mut changed = original.clone();
    changed[522..524].copy_from_slice(&30000_u16.to_le_bytes());
    std::fs::write(&path, changed).unwrap();
    let other = decode_selected_frame(&request, 16, &budget, || false).unwrap();
    assert!(!base.same_selected_frame(&other, || false).unwrap());
    drop((base, other));
    assert_eq!(budget.used(), 0);
    std::fs::write(&path, color_fixture(true, false)).unwrap();
    assert!(decode_selected_frame(&request, 16, &budget, || false).is_err());
    assert_eq!(budget.used(), 0);
    std::fs::write(&path, &original).unwrap();
    assert!(decode_selected_frame(&request, 16, &budget, || true).is_err());
    let retry = decode_selected_frame(&request, 16, &budget, || false).unwrap();
    drop(retry);
    assert_eq!(budget.used(), 0);
}

#[test]
fn sensor_cache_revalidates_changed_mosaic_with_restored_size_and_mtime() {
    use rrrah_dedup::{
        cache::FingerprintCache,
        decode::{FingerprintPolicy, fingerprint_file},
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cached.dng");
    let disk = dir.path().join("cache.bin");
    let mut original = color_fixture(true, true);
    for (index, value) in (1000_u16..1016).enumerate() {
        original[512 + index * 2..514 + index * 2].copy_from_slice(&value.to_le_bytes());
    }
    std::fs::write(&path, &original).unwrap();
    let stamp = std::time::UNIX_EPOCH + std::time::Duration::new(1_700_000_000, 123_400_100);
    let set_stamp = || {
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(stamp))
            .unwrap()
    };
    set_stamp();
    let before = std::fs::metadata(&path).unwrap();
    let request = DecodeRequest::new(&path);
    let budget = MemoryBudget::new(32 * 1024 * 1024);
    let policy = FingerprintPolicy {
        recipe: [0; 32],
        max_file_bytes: 1024,
        max_pixels: 16,
        max_frames: 1,
        max_cache_entries: 4,
    };
    let mut memory = FingerprintCache::default();
    let base = fingerprint_file(&request, policy, &budget, &mut memory, || false).unwrap();
    assert!(!base.cache_hit);
    assert!(
        fingerprint_file(&request, policy, &budget, &mut memory, || false)
            .unwrap()
            .cache_hit
    );
    memory.save_atomic(&disk, || false).unwrap();
    let mut edited = original;
    edited[522..524].copy_from_slice(&30000_u16.to_le_bytes());
    std::fs::write(&path, edited).unwrap();
    set_stamp();
    let after = std::fs::metadata(&path).unwrap();
    assert_eq!(before.len(), after.len());
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    let mut persisted = FingerprintCache::load_file(&disk, 4, || false).unwrap();
    let fresh = fingerprint_file(
        &request,
        policy,
        &budget,
        &mut FingerprintCache::default(),
        || false,
    )
    .unwrap();
    assert_ne!(base.fingerprint, fresh.fingerprint);
    for cache in [&mut memory, &mut persisted] {
        let changed = fingerprint_file(&request, policy, &budget, cache, || false).unwrap();
        assert!(!changed.cache_hit);
        assert_eq!(changed.fingerprint, fresh.fingerprint);
        let reused = fingerprint_file(&request, policy, &budget, cache, || false).unwrap();
        assert!(reused.cache_hit);
        assert_eq!(reused.fingerprint, fresh.fingerprint);
    }
    assert_eq!(budget.used(), 0);
}

#[test]
fn sensor_first_pair_admission_avoids_retaining_raster_during_raw_scratch() {
    use rrrah_dedup::{
        animated::AnimationBudget,
        geometry::GeometryPolicy,
        local::{LocalPolicy, MatchPolicy},
        local_scan::{LocalFilePolicy, compare_local_files},
        warp::WarpPolicy,
    };
    let root = tempfile::tempdir().unwrap();
    let raw_path = root.path().join("sensor.dng");
    let mut dng = color_fixture(true, true);
    for (index, value) in (1000_u16..1016).enumerate() {
        dng[512 + index * 2..514 + index * 2].copy_from_slice(&value.to_le_bytes());
    }
    std::fs::write(&raw_path, dng).unwrap();
    let raster_path = root.path().join("peer.bmp");
    let mut bmp = vec![0_u8; 54 + 32 * 32 * 3];
    bmp[..2].copy_from_slice(b"BM");
    for (offset, value) in [(2, bmp.len() as u32), (10, 54), (14, 40), (18, 32), (22, 32)] {
        bmp[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bmp[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&24_u16.to_le_bytes());
    for (i, pixel) in bmp[54..].chunks_exact_mut(3).enumerate() {
        pixel.copy_from_slice(&[(i % 251) as u8, ((i * 7) % 251) as u8, ((i * 13) % 251) as u8]);
    }
    std::fs::write(&raster_path, bmp).unwrap();
    let raw = DecodeRequest::new(raw_path);
    let mut raster = DecodeRequest::new(raster_path);
    raster.assume_untagged_srgb = true;
    let probe = MemoryBudget::new(32 * 1024 * 1024);
    let image = decode_selected_frame(&raw, 1024, &probe, || false).unwrap();
    let limit = probe.peak();
    drop(image);
    assert_eq!(probe.used(), 0);
    // Independent reproduction of the original raster-first lifetime overlap.
    let old_budget = MemoryBudget::new(limit);
    let retained = decode_selected_frame(&raster, 1024, &old_budget, || false).unwrap();
    let failed = decode_selected_frame(&raw, 1024, &old_budget, || false);
    assert!(
        matches!(
            failed,
            Err(rrrah_dedup::decode::FileError::Develop(
                rrrah_core::develop::DevelopError::Buffer(_)
            ))
        ),
        "{failed:?}"
    );
    drop(failed);
    drop(retained);
    assert_eq!(old_budget.used(), 0);
    let policy = LocalFilePolicy {
        decode: AnimationBudget {
            max_frames: 1,
            max_pixels: 1024,
            max_file_bytes: 8192,
        },
        extract: LocalPolicy {
            max_pixels: 1024,
            max_candidates: 1024,
            max_features: 8,
            minimum_corner_score: 0.0001,
        },
        matching: MatchPolicy {
            max_comparisons: 64,
            max_distance: 64,
        },
        geometry: GeometryPolicy {
            tolerance: 2.0,
            min_inliers: 3,
            max_points: 8,
            max_hypotheses: 64,
        },
        pixels: WarpPolicy {
            tolerance: 0.03,
            max_source_pixels: 1024,
        },
        minimum_compared_pixels: 1,
        minimum_coverage_fraction: 0.1,
        minimum_matched_fraction: 0.9,
    };
    for (left, right) in [(&raster, &raw), (&raw, &raster)] {
        for reflected in [false, true] {
            let budget = MemoryBudget::new(limit);
            let compare = |cancel: &dyn Fn() -> bool| {
                if reflected {
                    rrrah_dedup::local_scan::compare_reflected_local_files(
                        left, right, policy, &budget, cancel,
                    )
                    .map(|e| (e.candidate, e.geometry.is_some()))
                } else {
                    compare_local_files(left, right, policy, &budget, cancel)
                        .map(|e| (e.candidate, e.geometry.is_some()))
                }
            };
            let calls = std::cell::Cell::new(0usize);
            // This fixture qualifies allocation admission, not visual matching.
            assert_eq!(
                compare(&|| {
                    calls.set(calls.get() + 1);
                    false
                })
                .unwrap(),
                (false, false)
            );
            assert_eq!(budget.used(), 0);
            assert!(budget.peak() <= limit);
            assert!(calls.get() > 2);
            for stop in [1, calls.get() / 2, calls.get()] {
                let seen = std::cell::Cell::new(0usize);
                let result = compare(&|| {
                    seen.set(seen.get() + 1);
                    seen.get() == stop
                });
                assert!(
                    matches!(result, Err(rrrah_dedup::local_scan::LocalFileError::Cancelled)),
                    "reflected={reflected}, stop={stop}: {result:?}"
                );
                assert_eq!(budget.used(), 0);
            }
            assert_eq!(compare(&|| false).unwrap(), (false, false));
            assert_eq!(budget.used(), 0);
        }
    }
}
