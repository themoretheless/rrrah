#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    cache::FingerprintCache,
    decode::FingerprintPolicy,
    scan::{VisualPolicy, confirm_pixels, scan_visual},
};
fn policy(require_information: bool) -> VisualPolicy {
    VisualPolicy {
        fingerprint: FingerprintPolicy {
            recipe: [71; 32],
            max_file_bytes: 1024 * 1024,
            max_pixels: 30_000,
            max_frames: 10,
            max_cache_entries: 10,
        },
        max_files: 10,
        max_pairs: 10,
        radius: 0,
        allow_transforms: true,
        require_information,
    }
}
fn requests(names: &[&str]) -> Vec<(u64, DecodeRequest)> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/information");
    names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            (
                u64::try_from(i).unwrap() + 1,
                DecodeRequest::new(root.join(format!("{name}.png"))),
            )
        })
        .collect()
}
#[test]
fn informative_hash_collision_remains_candidate_and_fails_full_resolution_confirmation() {
    let files = requests(&["gradient", "gradient-checker"]);
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let report = scan_visual(
        files.clone(),
        policy(true),
        &budget,
        &mut FingerprintCache::default(),
        || false,
    )
    .unwrap();
    assert!(report.issues.is_empty());
    assert_eq!(report.pairs.len(), 1);
    assert!(report.pairs[0].evidence.informative);
    assert_eq!(report.pairs[0].evidence.distance, 0);
    let confirmed = confirm_pixels(files, [(1, 2)], policy(true), &budget, || false).unwrap();
    assert!(confirmed.equal.is_empty());
    assert_eq!(confirmed.different, [(1, 2)]);
    assert!(confirmed.issues.is_empty());
    assert_eq!(budget.used(), 0);
}
#[test]
fn uniform_candidates_and_invisible_rgb_never_invent_visible_pixel_equality() {
    let files = requests(&["black", "white", "transparent-red", "transparent-blue"]);
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let gated = scan_visual(
        files.clone(),
        policy(true),
        &budget,
        &mut FingerprintCache::default(),
        || false,
    )
    .unwrap();
    assert!(gated.issues.is_empty());
    assert!(gated.pairs.is_empty());
    let ungated = scan_visual(
        files.clone(),
        policy(false),
        &budget,
        &mut FingerprintCache::default(),
        || false,
    )
    .unwrap();
    assert_eq!(ungated.pairs.len(), 6);
    assert!(ungated.pairs.iter().all(|p| !p.evidence.informative));
    let confirmed = confirm_pixels(
        files,
        ungated.pairs.iter().map(|p| (p.left, p.right)),
        policy(false),
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(confirmed.equal, [(3, 4)]);
    assert_eq!(confirmed.different.len(), 5);
    assert!(confirmed.issues.is_empty());
    assert_eq!(budget.used(), 0);
}

#[test]
fn uniform_local_search_cannot_invent_geometry_or_color_fit() {
    use rrrah_dedup::{
        animated::AnimationBudget,
        geometry::GeometryPolicy,
        local::{LocalPolicy, MatchPolicy},
        local_scan::{LocalComparisonMode, LocalFilePolicy, LocalSearchPolicy, scan_local_files_with_policy},
        warp::{
            ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricFitMode, PhotometricPolicy,
            WarpPolicy,
        },
    };
    let local = LocalFilePolicy {
        decode: AnimationBudget {
            max_frames: 10,
            max_pixels: 30_000,
            max_file_bytes: 1024 * 1024,
        },
        extract: LocalPolicy {
            max_pixels: 30_000,
            max_candidates: 30_000,
            max_features: 100,
            minimum_corner_score: 0.0001,
        },
        matching: MatchPolicy {
            max_comparisons: 10_000,
            max_distance: 64,
        },
        geometry: GeometryPolicy {
            tolerance: 2.0,
            min_inliers: 10,
            max_points: 100,
            max_hypotheses: 10_000,
        },
        pixels: WarpPolicy {
            tolerance: 0.03,
            max_source_pixels: 30_000,
        },
        minimum_compared_pixels: 1000,
        minimum_coverage_fraction: 0.3,
        minimum_matched_fraction: 0.9,
    };
    let photo = PhotometricPolicy {
        residual: local.pixels,
        minimum_samples: 1000,
        minimum_variance: 1e-5,
        minimum_gain: 0.2,
        maximum_gain: 5.0,
        maximum_offset: 0.1,
    };
    let filter = ColorFilterPolicy {
        filter: FilterPolicy {
            radius: 3,
            max_sample_pairs: 10_000_000,
        },
        color_space: FilterColorSpace::EncodedSrgb,
    };
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    for comparison in [
        LocalComparisonMode::Strict,
        LocalComparisonMode::Photometric(photo),
        LocalComparisonMode::Filtered {
            photometric: photo,
            filter,
            fit: PhotometricFitMode::RejectOutsidePolicy,
        },
        LocalComparisonMode::Filtered {
            photometric: photo,
            filter,
            fit: PhotometricFitMode::ConstrainedLeastSquares,
        },
    ] {
        let report = scan_local_files_with_policy(
            requests(&["black", "white", "transparent-red", "transparent-blue"]),
            LocalSearchPolicy { local, comparison },
            4,
            6,
            &budget,
            || false,
        )
        .unwrap();
        assert_eq!(report.pairs.len(), 6);
        assert!(report.issues.is_empty());
        assert!(report.source_issues.is_empty());
        for pair in report.pairs {
            assert!(!pair.evidence.candidate);
            assert!(pair.evidence.geometry.is_none());
            assert!(pair.evidence.pixels.is_none());
            assert!(pair.evidence.photometric.is_none());
            assert!(pair.evidence.filtered.is_none());
        }
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn standalone_pixel_search_recovers_equal_transparent_files_without_visual_candidates() {
    use rrrah_dedup::scan::{PixelSearchPolicy, scan_equal_pixels};
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let report = scan_equal_pixels(
        requests(&[
            "black",
            "white",
            "transparent-red",
            "transparent-blue",
            "gradient",
            "gradient-checker",
        ]),
        PixelSearchPolicy {
            decode: policy(true).fingerprint,
            max_files: 6,
            max_pairs: 15,
        },
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(report.pixels.equal, [(3, 4)]);
    assert_eq!(report.pixels.different.len(), 14);
    assert!(report.pixels.issues.is_empty());
    assert!(report.source_issues.is_empty());
    assert_eq!(budget.used(), 0);
    let mut limits = PixelSearchPolicy {
        decode: policy(true).fingerprint,
        max_files: 6,
        max_pairs: 14,
    };
    assert!(matches!(
        scan_equal_pixels(
            (0..6).map(|id| (id, DecodeRequest::new("missing"))),
            limits,
            &budget,
            || false
        ),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    limits.decode.max_pixels = 0;
    let input =
        std::iter::from_fn(|| -> Option<(u64, DecodeRequest)> { panic!("invalid limits consumed input") });
    assert!(matches!(
        scan_equal_pixels(input, limits, &budget, || false),
        Err(rrrah_dedup::scan::ScanError::InvalidPolicy)
    ));
}

#[test]
fn indexed_candidates_match_exhaustive_pixel_equality_and_avoid_unrelated_pairs() {
    use rrrah_dedup::{
        pixel_index::scan_indexed_pixels,
        scan::{PixelSearchPolicy, scan_equal_pixels},
    };
    let files = requests(&[
        "black",
        "white",
        "transparent-red",
        "transparent-blue",
        "gradient",
        "gradient-checker",
    ]);
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let mut p = PixelSearchPolicy {
        decode: policy(true).fingerprint,
        max_files: 6,
        max_pairs: 15,
    };
    let all = scan_equal_pixels(files.clone(), p, &budget, || false).unwrap();
    p.max_pairs = 1;
    let index = scan_indexed_pixels(files.clone(), p, &budget, || false).unwrap();
    assert_eq!(index.pixels.equal, all.pixels.equal);
    assert_eq!(index.indexed_decodes, 6);
    assert_eq!(index.candidate_pairs, 1);
    assert_eq!(index.analysed, [1, 2, 3, 4, 5, 6]);
    assert!(index.pixels.different.is_empty());
    assert!(index.issues.is_empty());
    assert!(index.source_issues.is_empty());
    assert_eq!(budget.used(), 0);
    p.max_pairs = 0;
    assert!(matches!(
        scan_indexed_pixels(files, p, &budget, || false),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    assert_eq!(budget.used(), 0);
}
