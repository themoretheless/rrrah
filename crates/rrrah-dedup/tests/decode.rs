#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::decode::decode_selected_frame;

#[test]
fn layered_xcf_duplicates_preserve_color_errors_and_managed_ownership() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("layered.XCF");
    let renamed = dir.path().join("renamed.cr3");
    let untagged = dir.path().join("untagged.xcf");
    let other = dir.path().join("other.bmp");
    let profiled = include_bytes!("../../../tests/fixtures/xcf/profiled-normal-overlay-v1.xcf");
    std::fs::write(&source, profiled).unwrap();
    std::fs::write(&renamed, profiled).unwrap();
    std::fs::write(
        &untagged,
        include_bytes!("../../../tests/fixtures/xcf/normal-overlay-v1.xcf"),
    )
    .unwrap();
    let mut solid = bmp([0, 0, 255], 0);
    solid.resize(62, 0);
    solid[2..6].copy_from_slice(&62_u32.to_le_bytes());
    solid[18..22].copy_from_slice(&2_u32.to_le_bytes());
    solid[34..38].copy_from_slice(&8_u32.to_le_bytes());
    solid[57..60].copy_from_slice(&[0, 0, 255]);
    std::fs::write(&other, solid).unwrap();
    let budget = MemoryBudget::new(1024 * 1024);
    {
        let left = decode_selected_frame(&DecodeRequest::new(&source), 10, &budget, || false).unwrap();
        let right = decode_selected_frame(&DecodeRequest::new(&renamed), 10, &budget, || false).unwrap();
        assert!(left.same_selected_frame(&right, || false).unwrap());
        let different = decode_selected_frame(&DecodeRequest::new(&other), 10, &budget, || false).unwrap();
        assert!(!left.same_selected_frame(&different, || false).unwrap());
    }
    assert_eq!(budget.used(), 0);
    assert!(decode_selected_frame(&DecodeRequest::new(&untagged), 10, &budget, || false).is_err());
    assert_eq!(budget.used(), 0);
    assert!(decode_selected_frame(&DecodeRequest::new(&source), 10, &budget, || true).is_err());
    assert_eq!(budget.used(), 0);
}

#[test]
fn actual_files_ignore_comments_but_not_changed_pixels() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.bmp");
    let b = dir.path().join("b.bmp");
    let c = dir.path().join("c.bmp");
    std::fs::write(&a, bmp([0, 0, 255], 0)).unwrap();
    std::fs::write(&b, bmp([0, 0, 255], 100)).unwrap();
    std::fs::write(&c, bmp([0, 255, 0], 0)).unwrap();
    let budget = MemoryBudget::new(1024 * 1024);
    let load = |path| decode_selected_frame(&DecodeRequest::new(path), 1, &budget, || false).unwrap();
    let left = load(a);
    assert!(left.same_selected_frame(&load(b), || false).unwrap());
    assert!(!left.same_selected_frame(&load(c), || false).unwrap());
}

#[test]
fn corrupt_missing_cancelled_and_frame_out_of_range_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.ppm");
    let budget = MemoryBudget::new(1024 * 1024);
    let mut request = DecodeRequest::new(&path);
    assert!(decode_selected_frame(&request, 10, &budget, || false).is_err());
    std::fs::write(&path, b"P6\n1 1\n255\n").unwrap();
    assert!(decode_selected_frame(&request, 10, &budget, || false).is_err());
    std::fs::write(&path, b"P6\n1 1\n255\n\xff\0\0").unwrap();
    assert!(decode_selected_frame(&request, 10, &budget, || true).is_err());
    request.image_index = 1;
    assert!(decode_selected_frame(&request, 10, &budget, || false).is_err());
}

fn bmp(bgr: [u8; 3], resolution: u32) -> Vec<u8> {
    let mut bytes = vec![0; 58];
    bytes[..2].copy_from_slice(b"BM");
    for (offset, value) in [
        (2, 58_u32),
        (10, 54),
        (14, 40),
        (18, 1),
        (22, 1),
        (34, 4),
        (38, resolution),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24_u16.to_le_bytes());
    bytes[54..57].copy_from_slice(&bgr);
    bytes
}

#[test]
fn cached_file_fingerprints_reuse_only_current_content_and_recipe() {
    use rrrah_dedup::{
        cache::FingerprintCache,
        decode::{FingerprintPolicy, fingerprint_file},
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.bmp");
    std::fs::write(&path, bmp([0, 0, 255], 0)).unwrap();
    let budget = MemoryBudget::new(1024 * 1024);
    let policy = FingerprintPolicy {
        recipe: [1; 32],
        max_file_bytes: 1024,
        max_pixels: 1,
        max_frames: 10,
        max_cache_entries: 10,
    };
    let mut cache = FingerprintCache::default();
    let request = DecodeRequest::new(&path);
    let first = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
    assert!(!first.cache_hit);
    let second = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
    assert!(second.cache_hit);
    assert_eq!(first.fingerprint, second.fingerprint);
    let changed_recipe = fingerprint_file(
        &request,
        FingerprintPolicy {
            recipe: [2; 32],
            ..policy
        },
        &budget,
        &mut cache,
        || false,
    )
    .unwrap();
    assert!(!changed_recipe.cache_hit);
    std::fs::write(&path, bmp([0, 255, 0], 0)).unwrap();
    let changed = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
    assert!(!changed.cache_hit);
    assert_ne!(
        changed.fingerprint.mean_linear_rgb[0].to_bits(),
        first.fingerprint.mean_linear_rgb[0].to_bits()
    );
    assert!(
        fingerprint_file(
            &request,
            FingerprintPolicy {
                max_pixels: 0,
                ..policy
            },
            &budget,
            &mut cache,
            || false
        )
        .is_err()
    );
    let disk = dir.path().join("cache.bin");
    cache.save_atomic(&disk, || false).unwrap();
    let mut restored = FingerprintCache::load_file(&disk, 10, || false).unwrap();
    assert!(
        fingerprint_file(&request, policy, &budget, &mut restored, || false)
            .unwrap()
            .cache_hit
    );
}

#[test]
fn mutation_during_processing_never_admits_cached_evidence() {
    use rrrah_dedup::{
        cache::FingerprintCache,
        decode::{CachedError, FingerprintPolicy, fingerprint_file},
        exact::SnapshotError,
    };
    use std::cell::Cell;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.bmp");
    std::fs::write(&path, bmp([0, 0, 255], 0)).unwrap();
    let budget = MemoryBudget::new(1024 * 1024);
    let policy = FingerprintPolicy {
        recipe: [1; 32],
        max_file_bytes: 1024,
        max_pixels: 1,
        max_frames: 10,
        max_cache_entries: 10,
    };
    let mut cache = FingerprintCache::default();
    let request = DecodeRequest::new(&path);
    let checks = Cell::new(0);
    let result = fingerprint_file(&request, policy, &budget, &mut cache, || {
        checks.set(checks.get() + 1);
        if checks.get() == 10 {
            std::fs::write(&path, bmp([0, 255, 0], 0)).unwrap();
        }
        false
    });
    assert!(matches!(result, Err(CachedError::Source(SnapshotError::Changed))));
    assert!(
        !fingerprint_file(&request, policy, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
}

#[test]
fn icc_conversion_cancellation_releases_partial_output() {
    use rrrah_core::{DecodedRaster, RasterColorSpace, RasterError, RasterPixels};
    use rrrah_decode::{RasterColorError, prepare_raster_for_display_with_budget_and_cancel};
    use std::{cell::Cell, sync::Arc};
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/raster/pattern.profiled.png");
    let profiled = rrrah_decode::decode_raster_file(path).unwrap();
    let RasterColorSpace::Icc(profile) = profiled.color_space() else {
        panic!("missing profile")
    };
    let frame = DecodedRaster::new(
        16384,
        2,
        RasterPixels::Rgba8(Arc::new(vec![127; 16384 * 2 * 4]).into()),
        RasterColorSpace::Icc(profile.clone()),
    )
    .unwrap();
    // Covers setup, source filling, chunked transform, output append and final admission.
    for stop in 1..=27 {
        let calls = Cell::new(0);
        let budget = MemoryBudget::new(2_000_000);
        let result = prepare_raster_for_display_with_budget_and_cancel(&frame, Some(&budget), || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        });
        assert!(
            matches!(result, Err(RasterColorError::Frame(RasterError::Cancelled))),
            "stop {stop}"
        );
        assert_eq!(budget.used(), 0, "stop {stop}");
    }
}

#[test]
fn tiff_shaped_raw_never_uses_generic_page_fallback() {
    use rrrah_dedup::{
        cache::FingerprintCache,
        decode::{CachedError, FingerprintPolicy, fingerprint_file},
    };
    let dir = tempfile::tempdir().unwrap();
    let policy = FingerprintPolicy {
        recipe: [0; 32],
        max_file_bytes: 1024,
        max_pixels: 100,
        max_frames: 10,
        max_cache_entries: 10,
    };
    let budget = MemoryBudget::new(1024 * 1024);
    for name in ["camera.cr2", "camera.jpg", "camera.nef"] {
        let path = dir.path().join(name);
        let bytes = if name.ends_with("nef") {
            &b"II*\0\x08\0\0\0\0\0\0\0\0\0"[..]
        } else {
            &b"II*\0\x10\0\0\0CR\x02\0\0\0\0\0"[..]
        };
        std::fs::write(&path, bytes).unwrap();
        let mut cache = FingerprintCache::default();
        let direct = decode_selected_frame(&DecodeRequest::new(&path), 100, &budget, || false).unwrap_err();
        assert!(
            matches!(
                direct,
                rrrah_dedup::decode::FileError::RawDecode(rrrah_decode::DecodeError::NativeCamera { .. })
                    | rrrah_dedup::decode::FileError::Decode(rrrah_decode::RasterDecodeError::Source(
                        rrrah_decode::DecodeError::NativeCamera { .. }
                    ))
            ),
            "{name}: {direct:?}"
        );
        let error =
            fingerprint_file(&DecodeRequest::new(path), policy, &budget, &mut cache, || false).unwrap_err();
        assert!(matches!(error, CachedError::Decode(_)), "{name}: {error:?}");
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn public_selected_decode_rejects_a_source_changed_after_output_admission() {
    use rrrah_dedup::{
        decode::{CachedError, FileError},
        exact::SnapshotError,
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.bmp");
    std::fs::write(&path, bmp([0, 0, 255], 0)).unwrap();
    let budget = MemoryBudget::new(1024 * 1024);
    let changed = std::cell::Cell::new(false);
    let result = decode_selected_frame(&DecodeRequest::new(&path), 1, &budget, || {
        if budget.used() != 0 && !changed.replace(true) {
            std::fs::write(&path, bmp([0, 255, 0], 0)).unwrap();
        }
        false
    });
    assert!(changed.get());
    let Err(FileError::Prepared(error)) = result else {
        panic!("changed source accepted: {result:?}")
    };
    assert!(matches!(*error, CachedError::Source(SnapshotError::Changed)));
    assert_eq!(budget.used(), 0);
}

#[test]
fn pre_icc_fix_persisted_recipe_cannot_reuse_current_fingerprint() {
    use rrrah_dedup::{
        cache::{CacheKey, FingerprintCache},
        decode::{FingerprintPolicy, fingerprint_file},
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sample.bmp");
    let bytes = bmp([0, 0, 255], 0);
    std::fs::write(&path, &bytes).unwrap();
    let policy = FingerprintPolicy {
        recipe: [1; 32],
        max_file_bytes: 1024,
        max_pixels: 1,
        max_frames: 10,
        max_cache_entries: 10,
    };
    let budget = MemoryBudget::new(1024 * 1024);
    let request = DecodeRequest::new(&path);
    let current = fingerprint_file(
        &request,
        policy,
        &budget,
        &mut FingerprintCache::default(),
        || false,
    )
    .unwrap();
    // Reproduce the persisted pre-fix identity independently of current code.
    let mut cache = FingerprintCache::default();
    for version in [
        b"rrrah-file-linear-fingerprint-v4",
        b"rrrah-file-linear-fingerprint-v5",
    ] {
        let mut old = blake3::Hasher::new();
        old.update(version);
        old.update(&policy.recipe);
        old.update(&[0, 0]);
        old.update(&policy.max_pixels.to_le_bytes());
        old.update(&policy.max_file_bytes.to_le_bytes());
        old.update(&10u64.to_le_bytes());
        cache
            .insert(
                CacheKey {
                    content: *blake3::hash(&bytes).as_bytes(),
                    recipe: *old.finalize().as_bytes(),
                    frame_index: 0,
                },
                current.fingerprint.clone(),
                10,
            )
            .unwrap();
    }
    let disk = dir.path().join("old-cache.bin");
    cache.save_atomic(&disk, || false).unwrap();
    let mut cache = FingerprintCache::load_file(&disk, 10, || false).unwrap();
    assert!(
        !fingerprint_file(&request, policy, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    assert!(
        fingerprint_file(&request, policy, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    assert_eq!(budget.used(), 0);
}

#[test]
#[allow(clippy::too_many_lines)] // One cache fixture spans source mutation, both cancellation mechanisms and retries.
fn cache_hits_revalidate_source_and_cancellation_before_returning_evidence() {
    use rrrah_dedup::{
        cache::FingerprintCache,
        decode::{CachedError, FingerprintPolicy, fingerprint_file},
        exact::{ContentSnapshot, SnapshotError},
    };
    use std::cell::Cell;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.bmp");
    std::fs::write(&path, bmp([0, 0, 255], 0)).unwrap();
    let request = DecodeRequest::new(&path);
    let policy = FingerprintPolicy {
        recipe: [1; 32],
        max_file_bytes: 1024,
        max_pixels: 1,
        max_frames: 10,
        max_cache_entries: 10,
    };
    let budget = MemoryBudget::new(1024 * 1024);
    let mut cache = FingerprintCache::default();
    let original = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
    assert!(!original.cache_hit);
    let calls = Cell::new(0);
    ContentSnapshot::read(&path, policy.max_file_bytes, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let read_checks = calls.get();
    calls.set(0);
    let changed = Cell::new(false);
    let outcome = fingerprint_file(&request, policy, &budget, &mut cache, || {
        calls.set(calls.get() + 1);
        if calls.get() == read_checks + 1 {
            std::fs::write(&path, bmp([0, 255, 0], 0)).unwrap();
            changed.set(true);
        }
        false
    });
    assert!(changed.get());
    assert!(
        matches!(outcome, Err(CachedError::Source(SnapshotError::Changed))),
        "{outcome:?}"
    );
    let fresh = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
    assert!(!fresh.cache_hit);
    assert_ne!(fresh.fingerprint, original.fingerprint);
    calls.set(0);
    let outcome = fingerprint_file(&request, policy, &budget, &mut cache, || {
        calls.set(calls.get() + 1);
        calls.get() == read_checks + 1
    });
    assert!(
        matches!(outcome, Err(CachedError::Source(SnapshotError::Cancelled))),
        "{outcome:?}"
    );
    let retry = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
    assert!(retry.cache_hit);
    assert_eq!(retry.fingerprint, fresh.fingerprint);
    calls.set(0);
    fingerprint_file(&request, policy, &budget, &mut cache, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let checkpoints = calls.get();
    assert!(checkpoints > read_checks);
    for stop in 1..=checkpoints {
        calls.set(0);
        let cancelled = fingerprint_file(&request, policy, &budget, &mut cache, || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        });
        assert!(
            matches!(cancelled, Err(CachedError::Source(SnapshotError::Cancelled))),
            "stop={stop}: {cancelled:?}"
        );
        assert_eq!(budget.used(), 0);
        let recovered = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
        assert!(recovered.cache_hit);
        assert_eq!(recovered.fingerprint, fresh.fingerprint);
    }
    for stop in 1..=checkpoints {
        let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1));
        let mut guarded = request.clone();
        guarded.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 1));
        calls.set(0);
        let cancelled = fingerprint_file(&guarded, policy, &budget, &mut cache, || {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                generation.store(2, std::sync::atomic::Ordering::Release);
            }
            false
        });
        assert!(
            matches!(cancelled, Err(CachedError::Source(SnapshotError::Cancelled))),
            "generation stop={stop}: {cancelled:?}"
        );
        assert_eq!(budget.used(), 0);
    }
    println!("cache-hit cancellation checkpoints={checkpoints}");
    assert_eq!(budget.used(), 0);
}

#[test]
fn multiblock_cache_hit_cancellation_preserves_retry_and_releases_memory() {
    use rrrah_dedup::{
        cache::FingerprintCache,
        decode::{CachedError, FingerprintPolicy, fingerprint_file},
        exact::SnapshotError,
    };
    use std::cell::Cell;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.bmp");
    // BMP permits trailing bytes; independent pixel content stays the one-pixel
    // fixture while full-source identity and cancellation traverse many blocks.
    let mut bytes = bmp([0, 0, 255], 0);
    bytes.resize(8 * 1024 * 1024, 0x5a);
    std::fs::write(&path, bytes).unwrap();
    let request = DecodeRequest::new(&path);
    let policy = FingerprintPolicy {
        recipe: [1; 32],
        max_file_bytes: 8 * 1024 * 1024,
        max_pixels: 1,
        max_frames: 10,
        max_cache_entries: 10,
    };
    let budget = MemoryBudget::new(32 * 1024 * 1024);
    let mut cache = FingerprintCache::default();
    let first = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
    assert!(!first.cache_hit);
    let calls = Cell::new(0);
    let hit = fingerprint_file(&request, policy, &budget, &mut cache, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert!(hit.cache_hit);
    assert_eq!(hit.fingerprint, first.fingerprint);
    let checkpoints = calls.get();
    assert!(checkpoints > 128);
    let mut maximum_return = std::time::Duration::ZERO;
    for stop in 1..=checkpoints {
        calls.set(0);
        let requested_at = Cell::new(None);
        let outcome = fingerprint_file(&request, policy, &budget, &mut cache, || {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                requested_at.set(Some(std::time::Instant::now()));
                true
            } else {
                false
            }
        });
        let returned_at = std::time::Instant::now();
        assert!(
            matches!(outcome, Err(CachedError::Source(SnapshotError::Cancelled))),
            "stop={stop}: {outcome:?}"
        );
        maximum_return = maximum_return.max(returned_at.duration_since(requested_at.get().unwrap()));
        assert_eq!(budget.used(), 0);
    }
    let retry = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
    assert!(retry.cache_hit);
    assert_eq!(retry.fingerprint, first.fingerprint);
    println!(
        "multiblock-cache bytes=8388608 cancellation_checkpoints={checkpoints} max_request_to_return_us={}",
        maximum_return.as_micros()
    );
}

#[test]
fn cached_page_selection_and_color_assumptions_never_cross_reuse() {
    use rrrah_dedup::{
        cache::FingerprintCache,
        decode::{FingerprintPolicy, fingerprint_file},
    };
    let dir = tempfile::tempdir().unwrap();
    // Standard PPM has a specified BT.709 transfer; use a genuinely untagged
    // TGA to verify that a cached explicit interpretation cannot bypass policy.
    let path = dir.path().join("untagged.tga");
    let mut tga = vec![0_u8; 18];
    tga[2] = 2; // Uncompressed true color.
    tga[12] = 1;
    tga[14] = 1;
    tga[16] = 24;
    tga[17] = 0x20;
    tga.extend_from_slice(&[0x20, 0x40, 0x80]);
    std::fs::write(&path, tga).unwrap();
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let policy = FingerprintPolicy {
        recipe: [1; 32],
        max_file_bytes: 1_000_000,
        max_pixels: 100,
        max_frames: 10,
        max_cache_entries: 20,
    };
    let mut cache = FingerprintCache::default();
    let mut interpreted = DecodeRequest::new(&path);
    interpreted.assume_untagged_srgb = true;
    let first = fingerprint_file(&interpreted, policy, &budget, &mut cache, || false).unwrap();
    assert!(!first.cache_hit);
    assert!(
        fingerprint_file(&interpreted, policy, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    // The saved conventional interpretation must not bypass strict color admission.
    assert!(fingerprint_file(&DecodeRequest::new(&path), policy, &budget, &mut cache, || false).is_err());
    interpreted.assume_untagged_linear_srgb = true;
    assert!(
        !fingerprint_file(&interpreted, policy, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    assert!(
        fingerprint_file(&interpreted, policy, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    let tiff = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/tiff/classic-little-none-same.tif");
    let mut fingerprints = Vec::new();
    for image_index in 0..3 {
        let mut request = DecodeRequest::new(&tiff);
        request.image_index = image_index;
        let first = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
        assert!(!first.cache_hit);
        let hit = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
        assert!(hit.cache_hit);
        assert_eq!(hit.fingerprint, first.fingerprint);
        fingerprints.push(first.fingerprint);
        assert!(
            fingerprint_file(
                &request,
                FingerprintPolicy {
                    max_frames: 2,
                    ..policy
                },
                &budget,
                &mut cache,
                || false
            )
            .is_err()
        );
    }
    // Independent fixture recipe gives each page a different channel offset.
    for left in 0..3 {
        for right in left + 1..3 {
            assert_ne!(fingerprints[left], fingerprints[right]);
        }
    }
    let mut invalid = DecodeRequest::new(tiff);
    invalid.image_index = 3;
    assert!(fingerprint_file(&invalid, policy, &budget, &mut cache, || false).is_err());
    assert_eq!(budget.used(), 0);
}

#[test]
fn cached_content_does_not_bypass_extension_selected_decoder_errors() {
    use rrrah_dedup::{
        cache::FingerprintCache,
        decode::{FingerprintPolicy, fingerprint_file},
    };
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.bmp");
    let alternate = dir.path().join("source.pict");
    let bytes = bmp([0, 0, 255], 0);
    std::fs::write(&source, &bytes).unwrap();
    std::fs::write(&alternate, &bytes).unwrap();
    let budget = MemoryBudget::new(1024 * 1024);
    let policy = FingerprintPolicy {
        recipe: [51; 32],
        max_file_bytes: 1024,
        max_pixels: 1,
        max_frames: 1,
        max_cache_entries: 4,
    };
    let mut cache = FingerprintCache::default();
    fingerprint_file(&DecodeRequest::new(source), policy, &budget, &mut cache, || false).unwrap();
    let renamed = dir.path().join("renamed.BMP");
    std::fs::write(&renamed, &bytes).unwrap();
    let reused = fingerprint_file(&DecodeRequest::new(renamed), policy, &budget, &mut cache, || {
        false
    })
    .unwrap();
    assert!(
        reused.cache_hit,
        "renaming and extension case must preserve valid reuse"
    );
    assert!(
        fingerprint_file(
            &DecodeRequest::new(&alternate),
            policy,
            &budget,
            &mut FingerprintCache::default(),
            || false
        )
        .is_err()
    );
    assert!(
        fingerprint_file(
            &DecodeRequest::new(alternate),
            policy,
            &budget,
            &mut cache,
            || false
        )
        .is_err(),
        "cache must retain the fresh decoder's error behavior"
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn cached_wal_tracks_external_palette_changes_and_removal() {
    use rrrah_dedup::{
        cache::FingerprintCache,
        decode::{FingerprintPolicy, fingerprint_file},
    };
    let dir = tempfile::tempdir().unwrap();
    let textures = dir.path().join("textures");
    let pictures = dir.path().join("pics");
    std::fs::create_dir(&textures).unwrap();
    std::fs::create_dir(&pictures).unwrap();
    let source = textures.join("pattern.wal");
    let palette = pictures.join("colormap.pcx");
    std::fs::write(
        &source,
        include_bytes!("../../../tests/fixtures/raster/palette-grid.wal"),
    )
    .unwrap();
    let mut bytes = include_bytes!("../../../tests/fixtures/raster/wal-colormap.pcx").to_vec();
    std::fs::write(&palette, &bytes).unwrap();
    let request = DecodeRequest::new(source);
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let policy = FingerprintPolicy {
        recipe: [53; 32],
        max_file_bytes: 1024 * 1024,
        max_pixels: 1024 * 1024,
        max_frames: 1,
        max_cache_entries: 4,
    };
    let mut cache = FingerprintCache::default();
    let first = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
    let start = bytes.len() - 768;
    for channel in &mut bytes[start..] {
        *channel = 255 - *channel;
    }
    std::fs::write(&palette, bytes).unwrap();
    let fresh = fingerprint_file(
        &request,
        policy,
        &budget,
        &mut FingerprintCache::default(),
        || false,
    )
    .unwrap();
    assert_ne!(
        first.fingerprint, fresh.fingerprint,
        "palette mutation must affect this fixture"
    );
    let changed = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
    assert!(
        !changed.cache_hit,
        "external palette changes must invalidate the entry"
    );
    assert_eq!(changed.fingerprint, fresh.fingerprint);
    std::fs::remove_file(&palette).unwrap();
    assert!(fingerprint_file(&request, policy, &budget, &mut cache, || false).is_err());
    std::fs::write(
        &palette,
        include_bytes!("../../../tests/fixtures/raster/wal-colormap.pcx"),
    )
    .unwrap();
    let restored = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
    assert!(restored.cache_hit);
    assert_eq!(restored.fingerprint, first.fingerprint);
    assert_eq!(budget.used(), 0);
}
