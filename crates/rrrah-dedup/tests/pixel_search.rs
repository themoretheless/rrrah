#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    decode::FingerprintPolicy,
    scan::{PixelSearchPolicy, ScanError, scan_equal_pixel_roots, scan_equal_pixels},
};
fn policy() -> PixelSearchPolicy {
    PixelSearchPolicy {
        decode: FingerprintPolicy {
            recipe: [0; 32],
            max_file_bytes: 1024 * 1024,
            max_pixels: 30_000,
            max_frames: 10,
            max_cache_entries: 0,
        },
        max_files: 4,
        max_pairs: 6,
    }
}
fn triple() -> (tempfile::TempDir, Vec<(u64, DecodeRequest)>) {
    let folder = tempfile::tempdir().unwrap();
    let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/information/transparent-red.png");
    let files = (1..=3)
        .map(|id| {
            let path = folder.path().join(format!("{id}.png"));
            std::fs::copy(&source, &path).unwrap();
            (id, DecodeRequest::new(path))
        })
        .collect();
    (folder, files)
}
#[test]
fn indexed_two_key_groups_are_deterministic_across_request_orders() {
    let (folder, mut files) = triple();
    let white =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/information/white.png");
    std::fs::copy(&white, &files[2].1.path).unwrap();
    let fourth = folder.path().join("fourth.png");
    std::fs::copy(white, &fourth).unwrap();
    files.push((4, DecodeRequest::new(fourth)));
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    for order in [[0, 1, 2, 3], [3, 2, 1, 0], [2, 0, 3, 1], [1, 3, 0, 2]] {
        let report = rrrah_dedup::pixel_index::scan_indexed_pixels(
            order.map(|i| files[i].clone()),
            policy(),
            &budget,
            || false,
        )
        .unwrap();
        assert_eq!(report.analysed, [1, 2, 3, 4]);
        assert_eq!(report.pixels.equal, [(1, 2), (3, 4)]);
        assert!(report.pixels.different.is_empty());
        assert!(report.issues.is_empty() && report.source_issues.is_empty());
        assert_eq!(report.indexed_decodes, 4);
        assert_eq!(report.candidate_pairs, 2);
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn changed_source_discards_every_stale_decision_and_preserves_healthy_pairs() {
    use std::io::Write;
    let (_folder, files) = triple();
    let path = files[0].1.path.clone();
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let active = std::cell::Cell::new(false);
    let changed = std::cell::Cell::new(false);
    let report = scan_equal_pixels(files, policy(), &budget, || {
        if budget.used() > 0 {
            active.set(true);
        } else if active.get() && !changed.replace(true) {
            std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap()
                .write_all(&[0])
                .unwrap();
        }
        false
    })
    .unwrap();
    assert!(changed.get());
    assert_eq!(report.pixels.equal, [(2, 3)]);
    assert!(report.pixels.different.is_empty());
    assert_eq!(report.source_issues.len(), 1);
    assert_eq!(report.source_issues[0].0, 1);
    assert!(matches!(
        report.source_issues[0].1,
        rrrah_dedup::exact::SnapshotError::Changed
    ));
    assert_eq!(budget.used(), 0);
}
#[test]
fn final_generation_change_and_transient_user_cancel_return_no_report() {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    let (_folder, mut files) = triple();
    let generation = Arc::new(AtomicU64::new(7));
    for (_, request) in &mut files {
        request.cancellation = Some(rrrah_decode::GenerationToken::new(Arc::clone(&generation), 7));
    }
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let calls = std::cell::Cell::new(0usize);
    let report = scan_equal_pixels(files.clone(), policy(), &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(report.pixels.equal.len(), 3);
    let last = calls.get();
    calls.set(0);
    assert!(matches!(
        scan_equal_pixels(files.clone(), policy(), &budget, || {
            calls.set(calls.get() + 1);
            if calls.get() == last {
                generation.store(8, Ordering::Release);
            }
            false
        }),
        Err(ScanError::Cancelled)
    ));
    generation.store(7, Ordering::Release);
    calls.set(0);
    assert!(matches!(
        scan_equal_pixels(files, policy(), &budget, || {
            calls.set(calls.get() + 1);
            calls.get() == last - 1
        }),
        Err(ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
}
#[test]
fn recursive_pixel_search_keeps_corrupt_and_missing_sources_as_diagnostics() {
    let (folder, mut files) = triple();
    let nested = folder.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    let broken = nested.join("broken.png");
    std::fs::write(&broken, b"bad image").unwrap();
    files.push((4, DecodeRequest::new(folder.path().join("missing.png"))));
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let report = scan_equal_pixels(files, policy(), &budget, || false).unwrap();
    assert_eq!(report.pixels.equal.len(), 3);
    assert_eq!(report.source_issues.len(), 1);
    assert_eq!(report.source_issues[0].0, 4);
    let traversal = rrrah_dedup::exact::Options {
        follow_symlinks: false,
        max_depth: 10,
        max_entries: 100,
        max_file_bytes: 1024 * 1024,
    };
    let report = scan_equal_pixel_roots(
        &[folder.path().to_path_buf(), nested],
        &traversal,
        policy(),
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(report.files.len(), 4);
    assert_eq!(report.search.pixels.equal.len(), 3);
    assert_eq!(report.search.pixels.issues.len(), 3);
    assert!(report.search.source_issues.is_empty());
    assert_eq!(budget.used(), 0);
}

#[test]
fn opaque_uniform_png_and_independently_encoded_bmp_are_discovered_as_equal() {
    let folder = tempfile::tempdir().unwrap();
    let bmp = folder.path().join("white.bmp");
    let size = 54 + 144 * 144 * 3;
    let mut bytes = vec![0; size];
    bytes[..2].copy_from_slice(b"BM");
    for (offset, value) in [
        (2, u32::try_from(size).unwrap()),
        (10, 54),
        (14, 40),
        (18, 144),
        (22, 144),
        (34, 144 * 144 * 3),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
    bytes[54..].fill(255);
    std::fs::write(&bmp, bytes).unwrap();
    let png =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/information/white.png");
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let report = scan_equal_pixels(
        [(1, DecodeRequest::new(png)), (2, DecodeRequest::new(bmp))],
        policy(),
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(report.pixels.equal, [(1, 2)]);
    assert!(report.pixels.different.is_empty());
    assert!(report.pixels.issues.is_empty());
    assert!(report.source_issues.is_empty());
    assert_eq!(budget.used(), 0);
}

#[test]
fn indexed_changed_source_discards_stale_pairs() {
    use std::io::Write;
    let (_folder, files) = triple();
    let path = files[0].1.path.clone();
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let active = std::cell::Cell::new(false);
    let changed = std::cell::Cell::new(false);
    let report = rrrah_dedup::pixel_index::scan_indexed_pixels(files, policy(), &budget, || {
        if budget.used() > 0 {
            active.set(true);
        } else if active.get() && !changed.replace(true) {
            std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap()
                .write_all(&[0])
                .unwrap();
        }
        false
    })
    .unwrap();
    assert!(changed.get());
    assert_eq!(report.pixels.equal, [(2, 3)]);
    assert!(report.pixels.different.is_empty());
    assert_eq!(report.source_issues.len(), 1);
    assert_eq!(report.source_issues[0].0, 1);
    assert!(matches!(
        report.source_issues[0].1,
        rrrah_dedup::exact::SnapshotError::Changed
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn indexed_final_generation_and_transient_cancel_return_no_report() {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    let (_folder, mut files) = triple();
    let generation = Arc::new(AtomicU64::new(7));
    for (_, request) in &mut files {
        request.cancellation = Some(rrrah_decode::GenerationToken::new(Arc::clone(&generation), 7));
    }
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let calls = std::cell::Cell::new(0usize);
    let report = rrrah_dedup::pixel_index::scan_indexed_pixels(files.clone(), policy(), &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(report.pixels.equal.len(), 3);
    let last = calls.get();
    calls.set(0);
    assert!(matches!(
        rrrah_dedup::pixel_index::scan_indexed_pixels(files.clone(), policy(), &budget, || {
            calls.set(calls.get() + 1);
            if calls.get() == last {
                generation.store(8, Ordering::Release);
            }
            false
        }),
        Err(ScanError::Cancelled)
    ));
    generation.store(7, Ordering::Release);
    calls.set(0);
    assert!(matches!(
        rrrah_dedup::pixel_index::scan_indexed_pixels(files, policy(), &budget, || {
            calls.set(calls.get() + 1);
            calls.get() == last - 1
        }),
        Err(ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn indexed_recursive_search_isolates_corruption_and_preserves_overlapping_roots() {
    let (folder, mut files) = triple();
    let nested = folder.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    std::fs::write(nested.join("bad.png"), b"broken").unwrap();
    files.push((4, DecodeRequest::new(folder.path().join("missing.png"))));
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let report = rrrah_dedup::pixel_index::scan_indexed_pixels(files, policy(), &budget, || false).unwrap();
    assert_eq!(report.pixels.equal.len(), 3);
    assert_eq!(report.source_issues.len(), 1);
    assert_eq!(report.source_issues[0].0, 4);
    let traversal = rrrah_dedup::exact::Options {
        follow_symlinks: false,
        max_depth: 10,
        max_entries: 100,
        max_file_bytes: 1024 * 1024,
    };
    let report = rrrah_dedup::pixel_index::scan_indexed_pixel_roots(
        &[folder.path().to_path_buf(), nested],
        &traversal,
        policy(),
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(report.files.len(), 4);
    assert_eq!(report.indexed.pixels.equal.len(), 3);
    assert_eq!(report.indexed.issues.len(), 1);
    assert_eq!(report.indexed.candidate_pairs, 3);
    assert!(report.indexed.pixels.issues.is_empty());
    assert_eq!(budget.used(), 0);
}
#[test]
fn indexed_unique_collection_fits_zero_pair_budget_and_decodes_once_per_key() {
    let folder = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    for id in 0u8..128 {
        let mut bytes = vec![0u8; 58];
        bytes[..2].copy_from_slice(b"BM");
        for (offset, value) in [(2, 58u32), (10, 54), (14, 40), (18, 1), (22, 1), (34, 4)] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
        bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
        bytes[54] = id;
        let path = folder.path().join(format!("{id}.bmp"));
        std::fs::write(&path, bytes).unwrap();
        files.push((u64::from(id), DecodeRequest::new(path)));
    }
    let mut p = policy();
    p.max_files = 128;
    p.max_pairs = 0;
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let report = rrrah_dedup::pixel_index::scan_indexed_pixels(files, p, &budget, || false).unwrap();
    assert_eq!(report.analysed.len(), 128);
    assert_eq!(report.indexed_decodes, 128);
    assert_eq!(report.candidate_pairs, 0);
    assert!(report.pixels.equal.is_empty());
    assert!(report.issues.is_empty());
    assert!(report.source_issues.is_empty());
    assert_eq!(budget.used(), 0);
}

#[test]
fn indexed_source_changed_before_its_decode_is_not_admitted_as_fresh() {
    use std::io::Write;
    let (_folder, files) = triple();
    let path = files[1].1.path.clone();
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let active = std::cell::Cell::new(false);
    let changed = std::cell::Cell::new(false);
    let report = rrrah_dedup::pixel_index::scan_indexed_pixels(files, policy(), &budget, || {
        if budget.used() > 0 {
            active.set(true);
        } else if active.get() && !changed.replace(true) {
            std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap()
                .write_all(&[0])
                .unwrap();
        }
        false
    })
    .unwrap();
    assert!(changed.get());
    assert_eq!(report.pixels.equal, [(1, 3)]);
    assert!(report.pixels.different.is_empty());
    assert_eq!(report.source_issues.len(), 1);
    assert_eq!(report.source_issues[0].0, 2);
    assert!(matches!(
        report.source_issues[0].1,
        rrrah_dedup::exact::SnapshotError::Changed
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn untagged_hdr_requires_color_qualification_even_with_encoded_srgb_assumption() {
    let folder = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    for (id, little, scale, blue) in [
        (1, true, 1.0, 0.1234f32),
        (2, false, 1.0, 0.1234),
        (3, true, 1.0, 0.1235),
        (4, true, 2.0, 0.1234),
    ] {
        let signed_scale = if little { -scale } else { scale };
        let mut bytes = format!("PF\n1 1\n{signed_scale}\n").into_bytes();
        for value in [-0.1f32, 2.0, blue] {
            bytes.extend_from_slice(&if little {
                value.to_le_bytes()
            } else {
                value.to_be_bytes()
            });
        }
        let path = folder.path().join(format!("{id}.pfm"));
        std::fs::write(&path, bytes).unwrap();
        files.push((id, DecodeRequest::new(path)));
    }
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let unknown =
        rrrah_dedup::pixel_index::scan_indexed_pixels(files.clone(), policy(), &budget, || false).unwrap();
    assert_eq!(unknown.issues.len(), 4);
    assert!(unknown.analysed.is_empty());
    assert!(unknown.pixels.equal.is_empty());
    // PFM has no primaries tag; the conventional encoded-sRGB assumption is
    // deliberately insufficient authority to interpret unspecified linear samples.
    for (_, request) in &mut files {
        request.assume_untagged_srgb = true;
    }
    let direct = scan_equal_pixels(files.clone(), policy(), &budget, || false).unwrap();
    let indexed = rrrah_dedup::pixel_index::scan_indexed_pixels(files, policy(), &budget, || false).unwrap();
    assert_eq!(direct.pixels.issues.len(), 6);
    assert!(direct.pixels.equal.is_empty());
    assert!(direct.pixels.different.is_empty());
    assert_eq!(indexed.issues.len(), 4);
    for issue in &indexed.issues {
        assert!(matches!(
            issue.error,
            rrrah_dedup::decode::CachedError::Decode(rrrah_dedup::decode::FileError::Color(
                rrrah_decode::RasterColorError::Frame(rrrah_core::RasterError::ColorTransformRequired)
            ))
        ));
    }
    assert!(indexed.analysed.is_empty());
    assert_eq!(indexed.candidate_pairs, 0);
    assert!(indexed.pixels.equal.is_empty());
    assert!(indexed.source_issues.is_empty());
    assert_eq!(budget.used(), 0);
}

#[test]
fn explicitly_qualified_linear_hdr_matches_endianness_without_losing_precision_or_scale() {
    let folder = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    for (id, little, scale, blue) in [
        (1, true, 1.0, 0.1234f32),
        (2, false, 1.0, 0.1234),
        (3, true, 1.0, 0.1235),
        (4, true, 2.0, 0.1234),
    ] {
        let signed_scale = if little { -scale } else { scale };
        let mut bytes = format!("PF\n1 1\n{signed_scale}\n").into_bytes();
        for value in [-0.1f32, 2.0, blue] {
            bytes.extend_from_slice(&if little {
                value.to_le_bytes()
            } else {
                value.to_be_bytes()
            });
        }
        let path = folder.path().join(format!("{id}.pfm"));
        std::fs::write(&path, bytes).unwrap();
        files.push((id, DecodeRequest::new(path)));
    }
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    for (_, request) in &mut files {
        request.assume_untagged_linear_srgb = true;
    }
    let direct = scan_equal_pixels(files.clone(), policy(), &budget, || false).unwrap();
    let indexed =
        rrrah_dedup::pixel_index::scan_indexed_pixels(files.clone(), policy(), &budget, || false).unwrap();
    assert_eq!(direct.pixels.equal, [(1, 2)]);
    assert_eq!(direct.pixels.different.len(), 5);
    assert!(direct.pixels.issues.is_empty());
    assert_eq!(indexed.pixels.equal, direct.pixels.equal);
    assert_eq!(indexed.candidate_pairs, 1);
    assert_eq!(indexed.indexed_decodes, 4);
    assert!(indexed.issues.is_empty());
    assert!(indexed.source_issues.is_empty());
    let frame = rrrah_dedup::decode::decode_selected_frame(&files[0].1, 30_000, &budget, || false).unwrap();
    let expected = [-0.1f32, 2.0, 0.1234, 1.0];
    let known = rrrah_dedup::linear::LinearRgbaView::new(1, 1, &expected, 1, || false).unwrap();
    assert!(
        frame
            .view(|| false)
            .unwrap()
            .same_pixels(&known, || false)
            .unwrap()
    );
    drop(frame);
    // Cache recipes separate explicit linear assumptions from strict requests.
    let mut cache = rrrah_dedup::cache::FingerprintCache::default();
    let mut limits = policy().decode;
    limits.max_cache_entries = 10;
    assert!(
        !rrrah_dedup::decode::fingerprint_file(&files[0].1, limits, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    assert!(
        rrrah_dedup::decode::fingerprint_file(&files[0].1, limits, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    let mut strict = files[0].1.clone();
    strict.assume_untagged_linear_srgb = false;
    assert!(rrrah_dedup::decode::fingerprint_file(&strict, limits, &budget, &mut cache, || false).is_err());
    assert_eq!(budget.used(), 0);
}

#[test]
fn recursive_request_factory_preserves_explicit_hdr_color_and_discovered_paths() {
    let folder = tempfile::tempdir().unwrap();
    let nested = folder.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    for (path, little) in [(folder.path().join("a.pfm"), true), (nested.join("b.pfm"), false)] {
        let mut bytes = if little {
            b"PF\n1 1\n-1\n".to_vec()
        } else {
            b"PF\n1 1\n1\n".to_vec()
        };
        for v in [-0.1f32, 2.0, 0.1234] {
            bytes.extend_from_slice(&if little { v.to_le_bytes() } else { v.to_be_bytes() });
        }
        std::fs::write(path, bytes).unwrap();
    }
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let traversal = traversal_policy();
    let roots = [folder.path().to_path_buf(), nested];
    let strict =
        rrrah_dedup::pixel_index::scan_indexed_pixel_roots(&roots, &traversal, policy(), &budget, || false)
            .unwrap();
    assert_eq!(strict.indexed.issues.len(), 2);
    assert!(strict.indexed.pixels.equal.is_empty());
    let explicit = rrrah_dedup::pixel_index::scan_indexed_pixel_roots_with_requests(
        &roots,
        &traversal,
        policy(),
        &budget,
        |path| {
            let mut request = DecodeRequest::new(path);
            request.assume_untagged_linear_srgb = true;
            request
        },
        || false,
    )
    .unwrap();
    assert_eq!(explicit.files.len(), 2);
    assert_eq!(explicit.indexed.pixels.equal, [(0, 1)]);
    assert!(explicit.indexed.issues.is_empty());
    assert!(matches!(
        rrrah_dedup::pixel_index::scan_indexed_pixel_roots_with_requests(
            &roots,
            &traversal,
            policy(),
            &budget,
            |_| DecodeRequest::new("substituted.pfm"),
            || false
        ),
        Err(ScanError::InvalidPolicy)
    ));
    let unavailable = rrrah_dedup::pixel_index::scan_indexed_pixel_roots_with_requests(
        &roots,
        &traversal,
        policy(),
        &budget,
        |path| {
            let mut request = DecodeRequest::new(path);
            request.image_index = 1;
            request.assume_untagged_linear_srgb = true;
            request
        },
        || false,
    )
    .unwrap();
    assert_eq!(unavailable.indexed.issues.len(), 2);
    assert!(unavailable.indexed.analysed.is_empty());
    let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(8));
    assert!(matches!(
        rrrah_dedup::pixel_index::scan_indexed_pixel_roots_with_requests(
            &roots,
            &traversal,
            policy(),
            &budget,
            |path| {
                let mut request = DecodeRequest::new(path);
                request.cancellation = Some(rrrah_decode::GenerationToken::new(
                    std::sync::Arc::clone(&generation),
                    7,
                ));
                request
            },
            || false,
        ),
        Err(ScanError::Cancelled)
    ));
    let mut invalid = policy();
    invalid.decode.max_pixels = 0;
    assert!(matches!(
        rrrah_dedup::pixel_index::scan_indexed_pixel_roots_with_requests(
            &roots,
            &traversal,
            invalid,
            &budget,
            |_| panic!("invalid limits called factory"),
            || false
        ),
        Err(ScanError::InvalidPolicy)
    ));
    assert_eq!(budget.used(), 0);
}

fn traversal_policy() -> rrrah_dedup::exact::Options {
    rrrah_dedup::exact::Options {
        follow_symlinks: false,
        max_depth: 10,
        max_entries: 100,
        max_file_bytes: 1024 * 1024,
    }
}

#[cfg(unix)]
#[test]
fn indexed_roots_preserve_non_utf8_diagnostics_without_losing_healthy_equality() {
    use std::{
        ffi::OsString,
        os::unix::ffi::{OsStrExt, OsStringExt},
    };
    let (folder, requests) = triple();
    let missing = folder
        .path()
        .join(OsString::from_vec(vec![b'x', 0xff, b'.', b'p', b'n', b'g']));
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let report = rrrah_dedup::pixel_index::scan_indexed_pixel_roots(
        &[folder.path().to_path_buf(), missing.clone()],
        &rrrah_dedup::exact::Options::default(),
        policy(),
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(report.files.len(), requests.len());
    assert_eq!(report.traversal_issues.len(), 1);
    assert_eq!(
        report.traversal_issues[0].path.as_os_str().as_bytes(),
        missing.as_os_str().as_bytes()
    );
    assert!(matches!(
        report.traversal_issues[0].kind,
        rrrah_dedup::exact::IssueKind::Io(_)
    ));
    assert!(report.indexed.issues.is_empty() && report.indexed.source_issues.is_empty());
    assert_eq!(report.indexed.pixels.equal, [(0, 1), (0, 2), (1, 2)]);
    assert_eq!(budget.used(), 0);
}

#[cfg(unix)]
#[test]
fn indexed_pixel_permission_error_preserves_healthy_pairs_and_recovers() {
    use std::os::unix::fs::PermissionsExt;
    struct Restore(std::path::PathBuf);
    impl Drop for Restore {
        fn drop(&mut self) {
            std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
    }
    let (folder, requests) = triple();
    let denied = requests[2].1.path.clone();
    let restore = Restore(denied.clone());
    std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o000)).unwrap();
    let error = std::fs::read(&denied);
    if error.is_ok() {
        drop(restore);
        return; // Privileged runners do not qualify permission denial.
    }
    assert_eq!(error.unwrap_err().kind(), std::io::ErrorKind::PermissionDenied);
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let run = || {
        rrrah_dedup::pixel_index::scan_indexed_pixel_roots(
            &[folder.path().to_path_buf()],
            &rrrah_dedup::exact::Options::default(),
            policy(),
            &budget,
            || false,
        )
        .unwrap()
    };
    let report = run();
    assert_eq!(report.files.len(), 3);
    assert_eq!(report.indexed.analysed, [0, 1]);
    assert_eq!(report.indexed.pixels.equal, [(0, 1)]);
    assert_eq!(report.indexed.source_issues.len(), 1);
    assert_eq!(report.indexed.source_issues[0].0, 2);
    assert!(report.indexed.issues.is_empty());
    assert!(report.traversal_issues.is_empty());
    drop(restore);
    let recovered = run();
    assert_eq!(recovered.indexed.analysed, [0, 1, 2]);
    assert_eq!(recovered.indexed.pixels.equal, [(0, 1), (0, 2), (1, 2)]);
    assert!(recovered.indexed.source_issues.is_empty());
    assert_eq!(budget.used(), 0);
}

#[test]
fn indexed_wal_palette_mutation_discards_stale_source_decisions() {
    let folder = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    let mut first_palette = std::path::PathBuf::new();
    for id in 1..=3 {
        let root = folder.path().join(format!("game-{id}"));
        std::fs::create_dir_all(root.join("textures")).unwrap();
        std::fs::create_dir(root.join("pics")).unwrap();
        let path = root.join("textures/source.wal");
        let palette = root.join("pics/colormap.pcx");
        std::fs::write(&path, include_bytes!("../../../tests/fixtures/raster/palette-grid.wal")).unwrap();
        std::fs::write(&palette, include_bytes!("../../../tests/fixtures/raster/wal-colormap.pcx")).unwrap();
        if id == 1 { first_palette = palette; }
        files.push((id, DecodeRequest::new(path)));
    }
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let active = std::cell::Cell::new(false);
    let changed = std::cell::Cell::new(false);
    let report = rrrah_dedup::pixel_index::scan_indexed_pixels(files, policy(), &budget, || {
        if budget.used() > 0 { active.set(true); }
        else if active.get() && !changed.replace(true) {
            let mut bytes = std::fs::read(&first_palette).unwrap();
            let start = bytes.len() - 768;
            for channel in &mut bytes[start..] { *channel = 255 - *channel; }
            std::fs::write(&first_palette, bytes).unwrap();
        }
        false
    }).unwrap();
    assert!(changed.get(), "mutation must occur after retained pixel observation");
    assert_eq!(report.pixels.equal, [(2, 3)]);
    assert!(report.pixels.different.is_empty());
    assert!(report.source_issues.iter().any(|(id, error)| *id == 1
        && matches!(error, rrrah_dedup::exact::SnapshotError::Changed)));
    assert_eq!(budget.used(), 0);
}
