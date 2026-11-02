#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::GeometryPolicy,
    local::{LocalPolicy, MatchPolicy},
    local_scan::{LocalFilePolicy, compare_local_files_with_policy},
    warp::{PhotometricPolicy, WarpPolicy},
};
use std::{cell::Cell, fs};
fn policy() -> LocalFilePolicy {
    LocalFilePolicy {
        decode: AnimationBudget {
            max_frames: 10,
            max_pixels: 50_000,
            max_file_bytes: 1024 * 1024,
        },
        extract: LocalPolicy {
            max_pixels: 50_000,
            max_candidates: 50_000,
            max_features: 500,
            minimum_corner_score: 0.0001,
        },
        matching: MatchPolicy {
            max_comparisons: 250_000,
            max_distance: 64,
        },
        geometry: GeometryPolicy {
            tolerance: 2.0,
            min_inliers: 10,
            max_points: 500,
            max_hypotheses: 125_000,
        },
        pixels: WarpPolicy {
            tolerance: 0.03,
            max_source_pixels: 100_000,
        },
        minimum_compared_pixels: 1000,
        minimum_coverage_fraction: 0.3,
        minimum_matched_fraction: 0.9,
    }
}
const FIT: PhotometricPolicy = PhotometricPolicy {
    residual: WarpPolicy {
        tolerance: 0.03,
        max_source_pixels: 100_000,
    },
    minimum_samples: 100,
    minimum_variance: 1e-5,
    minimum_gain: 0.9,
    maximum_gain: 1.1,
    maximum_offset: 0.1,
};
#[test]
fn automatic_geometry_final_callback_cannot_publish_changed_source() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a.png");
    let b = root.path().join("b.png");
    let bytes = include_bytes!("fixtures/rotation/base.png");
    fs::write(&a, bytes).unwrap();
    fs::write(&b, bytes).unwrap();
    let memory = MemoryBudget::new(64 * 1024 * 1024);
    let left = DecodeRequest::new(&a);
    let right = DecodeRequest::new(&b);
    let count = Cell::new(0);
    for anchored in [false, true] {
        fs::write(&a, bytes).unwrap();
        fs::write(&b, bytes).unwrap();
        count.set(0);
        let compare = |cancel: &dyn Fn() -> bool| {
            if anchored {
                rrrah_dedup::local_scan::compare_local_files_white_anchored(
                    &left,
                    &right,
                    policy(),
                    FIT,
                    None,
                    &memory,
                    cancel,
                )
            } else {
                compare_local_files_with_policy(&left, &right, policy().into(), &memory, cancel)
            }
        };
        let baseline = compare(&|| {
            count.set(count.get() + 1);
            false
        })
        .unwrap();
        assert!(baseline.candidate);
        drop(baseline);
        assert_eq!(memory.used(), 0);
        for path in [&a, &b] {
            fs::write(&a, bytes).unwrap();
            fs::write(&b, bytes).unwrap();
            let calls = Cell::new(0);
            let result = compare(&|| {
                calls.set(calls.get() + 1);
                if calls.get() == count.get() {
                    let mut changed = bytes.to_vec();
                    changed.push(0);
                    fs::write(path, changed).unwrap();
                }
                false
            });
            assert!(
                result.is_err(),
                "published geometry result after last callback changed {path:?}"
            );
            drop(result);
            assert_eq!(memory.used(), 0);
        }
    }
}

#[test]
fn automatic_white_anchor_matches_direct_confirmation_and_rejects_other_scene() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let a = DecodeRequest::new(root.join("base.png"));
    let b = DecodeRequest::new(root.join("base.png"));
    let memory = MemoryBudget::new(64 * 1024 * 1024);
    let auto = rrrah_dedup::local_scan::compare_local_files_white_anchored(
        &a,
        &b,
        policy(),
        FIT,
        None,
        &memory,
        || false,
    )
    .unwrap();
    assert!(auto.candidate && auto.display_projection);
    let transform = auto.geometry.as_ref().unwrap().transform;
    let direct = rrrah_dedup::white_anchored_file::verify_white_anchored_files(
        &a,
        &b,
        transform,
        policy().decode,
        FIT,
        &memory,
        || false,
    )
    .unwrap();
    assert_eq!(auto.photometric.as_ref().unwrap(), &direct.projected.evidence);
    assert_eq!(auto.pixels.as_ref().unwrap(), &direct.strict);
    drop(auto);
    assert_eq!(memory.used(), 0);
    let other = DecodeRequest::new(root.join("unrelated.png"));
    let negative = rrrah_dedup::local_scan::compare_local_files_white_anchored(
        &a,
        &other,
        policy(),
        FIT,
        None,
        &memory,
        || false,
    )
    .unwrap();
    assert!(!negative.candidate);
    drop(negative);
    assert_eq!(memory.used(), 0);
}

#[test]
fn white_anchor_invalid_spatial_grid_refuses_before_source_io() {
    let root = tempfile::tempdir().unwrap();
    let missing = DecodeRequest::new(root.path().join("absent.png"));
    let error = rrrah_dedup::local_scan::compare_local_files_white_anchored(
        &missing,
        &missing,
        policy(),
        FIT,
        Some(rrrah_dedup::local_scan::SpatialFeaturePolicy {
            columns: 0,
            rows: 1,
            max_per_cell: 1,
        }),
        &MemoryBudget::new(0),
        || false,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        rrrah_dedup::local_scan::LocalFileError::InvalidPolicy
    ));
}

#[test]
fn filtered_white_anchor_is_an_explicit_file_mode_with_strict_evidence() {
    use rrrah_dedup::{
        local_scan::{LocalComparisonMode, LocalSearchPolicy},
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricFitMode},
    };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png");
    let request = DecodeRequest::new(path);
    let memory = MemoryBudget::new(64 * 1024 * 1024);
    let result = compare_local_files_with_policy(
        &request,
        &request,
        LocalSearchPolicy {
            local: policy(),
            comparison: LocalComparisonMode::Filtered {
                photometric: FIT,
                filter: ColorFilterPolicy {
                    filter: FilterPolicy {
                        radius: 1,
                        max_sample_pairs: 2_000_000,
                    },
                    color_space: FilterColorSpace::EncodedSrgb,
                },
                fit: PhotometricFitMode::WhiteAnchoredLeastSquares,
            },
        },
        &memory,
        || false,
    )
    .unwrap();
    assert!(result.candidate);
    assert!(result.pixels.is_some());
    assert_eq!(
        result.filtered.as_ref().unwrap().fit_mode,
        PhotometricFitMode::WhiteAnchoredLeastSquares
    );
    drop(result);
    assert_eq!(memory.used(), 0);
}

// Authored textured BMP with a controlled edit; no third-party image is modified.
fn textured_bmp(edit_side: usize) -> Vec<u8> {
    const SIDE: usize = 128;
    let size = 54 + SIDE * SIDE * 3;
    let mut bytes = vec![0u8; size];
    bytes[..2].copy_from_slice(b"BM");
    for (offset, value) in [
        (2, size as u32),
        (10, 54),
        (14, 40),
        (18, SIDE as u32),
        (22, SIDE as u32),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
    let mut state = 0x1738_5a29u32;
    for y in 0..SIDE {
        for x in 0..SIDE {
            let at = 54 + (y * SIDE + x) * 3;
            for channel in 0..3 {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                bytes[at + channel] = 80 + (state % 150) as u8;
            }
            if x >= (SIDE - edit_side) / 2
                && x < (SIDE + edit_side) / 2
                && y >= (SIDE - edit_side) / 2
                && y < (SIDE + edit_side) / 2
            {
                bytes[at..at + 3].fill(0);
            }
        }
    }
    bytes
}

#[test]
fn automatic_white_anchor_keeps_local_edits_visible_with_valid_geometry() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("base.bmp");
    let b = root.path().join("edited.bmp");
    fs::write(&a, textured_bmp(0)).unwrap();
    let memory = MemoryBudget::new(64 * 1024 * 1024);
    for (edit_side, expected_candidate) in [(0, true), (8, true), (64, false)] {
        fs::write(&b, textured_bmp(edit_side)).unwrap();
        let result = rrrah_dedup::local_scan::compare_local_files_white_anchored(
            &DecodeRequest::new(&a),
            &DecodeRequest::new(&b),
            policy(),
            FIT,
            None,
            &memory,
            || false,
        )
        .unwrap();
        assert!(result.geometry.is_some(), "geometry absent for edit {edit_side}");
        assert_eq!(result.candidate, expected_candidate, "edit {edit_side}");
        let strict = result.pixels.as_ref().unwrap();
        if edit_side > 0 {
            assert!(strict.forward.matched_pixels < strict.forward.compared_pixels);
        }
        let projected = result.photometric.as_ref().unwrap();
        if edit_side == 64 {
            assert!(
                projected.forward.pixels.matched_pixels * 10 < projected.forward.pixels.compared_pixels * 9
            );
        }
        drop(result);
        assert_eq!(memory.used(), 0);
    }
}

#[test]
fn automatic_white_anchor_cancellation_sweep_and_retry() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("textured.bmp");
    fs::write(&path, textured_bmp(0)).unwrap();
    let request = DecodeRequest::new(path);
    let memory = MemoryBudget::new(64 * 1024 * 1024);
    let mut bounded = policy();
    bounded.extract.max_features = 32;
    bounded.geometry.max_points = 32;
    bounded.geometry.max_hypotheses = 512;
    let compare = |cancel: &dyn Fn() -> bool| {
        rrrah_dedup::local_scan::compare_local_files_white_anchored(
            &request, &request, bounded, FIT, None, &memory, cancel,
        )
    };
    let calls = Cell::new(0usize);
    let baseline = compare(&|| {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert!(baseline.candidate);
    drop(baseline);
    assert_eq!(memory.used(), 0);
    // Full pixel-level sweep is available for dedicated long-running qualification.
    let stops: std::collections::BTreeSet<usize> =
        if std::env::var_os("RRRAH_EXHAUSTIVE_AUTO_CANCEL").is_some() {
            (1..=calls.get()).collect()
        } else {
            (0..128).map(|i| 1 + (calls.get() - 1) * i / 127).collect()
        };
    let checked = stops.len();
    for stop in stops {
        let seen = Cell::new(0usize);
        let result = compare(&|| {
            seen.set(seen.get() + 1);
            seen.get() == stop // One pulse must remain latched.
        });
        assert!(
            matches!(result, Err(rrrah_dedup::local_scan::LocalFileError::Cancelled)),
            "cancellation callback {stop}/{} produced {result:?}",
            calls.get()
        );
        drop(result);
        assert_eq!(memory.used(), 0, "callback {stop}");
    }
    let retry = compare(&|| false).unwrap();
    assert!(retry.candidate);
    drop(retry);
    assert_eq!(memory.used(), 0);
    eprintln!(
        "white-anchor cancellation callbacks checked: {checked}/{}",
        calls.get()
    );
}

#[test]
fn indexed_white_anchor_collection_matches_pair_confirmation_for_local_edits() {
    use rrrah_dedup::{
        local_collection::{LocalCollectionPolicy, scan_local_collection},
        local_index::FileFeatureBudgets,
        local_scan::{LocalComparisonMode, LocalSearchPolicy},
    };
    let root = tempfile::tempdir().unwrap();
    let files: Vec<_> = [0, 8, 64]
        .into_iter()
        .enumerate()
        .map(|(i, side)| {
            let path = root.path().join(format!("{side}.bmp"));
            fs::write(&path, textured_bmp(side)).unwrap();
            (i as u64 + 1, DecodeRequest::new(path))
        })
        .collect();
    let memory = MemoryBudget::new(64 * 1024 * 1024);
    let search = LocalSearchPolicy {
        local: policy(),
        comparison: LocalComparisonMode::WhiteAnchoredDisplay(FIT),
    };
    let report = scan_local_collection(
        files.clone(),
        LocalCollectionPolicy {
            search,
            budgets: FileFeatureBudgets {
                max_files: 3,
                max_features: 1500,
                max_hits: 2_250_000,
                max_pair_counts: 3,
                max_pairs: 3,
            },
        },
        &memory,
        || false,
    )
    .unwrap();
    assert!(
        report.file_issues.is_empty()
            && report.local.issues.is_empty()
            && report.local.source_issues.is_empty()
    );
    for right in [1usize, 2] {
        let direct =
            compare_local_files_with_policy(&files[0].1, &files[right].1, search, &memory, || false).unwrap();
        let indexed = report
            .local
            .pairs
            .iter()
            .find(|p| p.left == 1 && p.right == right as u64 + 1)
            .expect("index must retrieve both controlled edited pairs");
        assert_eq!(indexed.evidence.candidate, right == 1);
        assert_eq!(indexed.evidence.candidate, direct.candidate);
        assert_eq!(indexed.evidence.pixels, direct.pixels);
        assert_eq!(indexed.evidence.photometric, direct.photometric);
        assert!(indexed.evidence.display_projection);
        assert_eq!(indexed.evidence.photometric_fit_range.unwrap().minimum, 0.0);
        drop(direct);
    }
    drop(report);
    assert_eq!(memory.used(), 0);
}

#[test]
fn reflected_white_anchor_collection_matches_pair_and_keeps_large_edit_negative() {
    use rrrah_dedup::{
        local_collection::{LocalCollectionPolicy, scan_reflected_local_collection},
        local_index::FileFeatureBudgets,
        local_scan::{LocalComparisonMode, LocalSearchPolicy, compare_reflected_local_files_with_policy},
    };
    let root = tempfile::tempdir().unwrap();
    let files: Vec<_> = [0, 0, 64]
        .into_iter()
        .enumerate()
        .map(|(i, side)| {
            let path = root.path().join(format!("{i}.bmp"));
            let mut bytes = textured_bmp(side);
            if i != 0 {
                for y in 0..128 {
                    for x in 0..64 {
                        for channel in 0..3 {
                            bytes.swap(
                                54 + (y * 128 + x) * 3 + channel,
                                54 + (y * 128 + 127 - x) * 3 + channel,
                            );
                        }
                    }
                }
            }
            fs::write(&path, bytes).unwrap();
            (i as u64 + 1, DecodeRequest::new(path))
        })
        .collect();
    let memory = MemoryBudget::new(64 * 1024 * 1024);
    let search = LocalSearchPolicy {
        local: policy(),
        comparison: LocalComparisonMode::WhiteAnchoredDisplay(FIT),
    };
    let report = scan_reflected_local_collection(
        files.clone(),
        LocalCollectionPolicy {
            search,
            budgets: FileFeatureBudgets {
                max_files: 3,
                max_features: 3000,
                max_hits: 9_000_000,
                max_pair_counts: 3,
                max_pairs: 3,
            },
        },
        &memory,
        || false,
    )
    .unwrap();
    assert!(report.file_issues.is_empty() && report.issues.is_empty() && report.source_issues.is_empty());
    for right in [1usize, 2] {
        let direct = compare_reflected_local_files_with_policy(
            &files[0].1,
            &files[right].1,
            search,
            None,
            &memory,
            || false,
        )
        .unwrap();
        assert!(direct.geometry.is_some(), "edit must retain reflected geometry");
        assert_eq!(direct.candidate, right == 1);
        let indexed = report
            .pairs
            .iter()
            .find(|p| p.left == 1 && p.right == right as u64 + 1)
            .expect("both mirrored pairs must be retrieved");
        assert_eq!(indexed.evidence.candidate, direct.candidate);
        assert_eq!(indexed.evidence.pixels, direct.pixels);
        assert_eq!(indexed.evidence.photometric, direct.photometric);
        assert!(indexed.evidence.display_projection);
        assert_eq!(indexed.evidence.photometric_fit_range.unwrap().maximum, 1.0);
        drop(direct);
    }
    drop(report);
    assert_eq!(memory.used(), 0);
}
