#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::GeometryPolicy,
    local::{LocalPolicy, MatchPolicy},
    local_scan::LocalFilePolicy,
    warp::WarpPolicy,
};
fn policy() -> LocalFilePolicy {
    LocalFilePolicy {
        decode: AnimationBudget {
            max_frames: 1,
            max_pixels: 200_000,
            max_file_bytes: 1024 * 1024,
        },
        extract: LocalPolicy {
            max_pixels: 200_000,
            max_candidates: 200_000,
            max_features: 32,
            minimum_corner_score: 0.0001,
        },
        matching: MatchPolicy {
            max_comparisons: 4096,
            max_distance: 32,
        },
        geometry: GeometryPolicy {
            tolerance: 1e-5,
            min_inliers: 6,
            max_points: 32,
            max_hypotheses: 40_000,
        },
        pixels: WarpPolicy {
            tolerance: 1e-5,
            max_source_pixels: 400_000,
        },
        minimum_compared_pixels: 1000,
        minimum_coverage_fraction: 0.9,
        minimum_matched_fraction: 1.0,
    }
}

#[test]
fn joined_cumulative_caps_refuse_before_source_io() {
    use rrrah_dedup::{
        geometry::ProjectiveSamplingPolicy,
        local_scan::{
            LocalFileError, ProjectiveComplementaryFilePolicy, ProjectivePortfolioFilePolicy,
            ProjectivePyramidPhotometricFilePolicy,
        },
        warp::{
            ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricFitMode, PhotometricPolicy,
            ProjectiveRegistrationPolicy, ProjectiveRegistrationPortfolioPolicy,
            ProjectiveRegistrationTrustPolicy,
        },
    };
    use std::{cell::Cell, io::Write};
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout");
    let dir = tempfile::tempdir().unwrap();
    let a_path = dir.path().join("a.png");
    let b_path = dir.path().join("b.png");
    std::fs::copy(source.join("2414-base.png"), &a_path).unwrap();
    std::fs::copy(&a_path, &b_path).unwrap();
    let a = DecodeRequest::new(&a_path);
    let b = DecodeRequest::new(&b_path);
    let mut local = policy();
    local.extract.max_features = 128;
    local.matching.max_comparisons = 128 * 128;
    local.matching.max_distance = 64;
    local.geometry.max_points = 128;
    local.geometry.max_hypotheses = 2048;
    local.geometry.min_inliers = 10;
    local.geometry.tolerance = 2.;
    local.pixels.tolerance = 0.03;
    local.pixels.max_source_pixels = 409_600;
    local.minimum_coverage_fraction = 0.3;
    local.minimum_matched_fraction = 0.9;
    let photometric = PhotometricPolicy {
        residual: local.pixels,
        minimum_samples: 16,
        minimum_variance: 1e-5,
        minimum_gain: 0.2,
        maximum_gain: 5.,
        maximum_offset: 0.1,
    };
    let filter = ColorFilterPolicy {
        filter: FilterPolicy {
            radius: 1,
            max_sample_pairs: 4_000_000,
        },
        color_space: FilterColorSpace::LinearSrgb,
    };
    let registration = ProjectiveRegistrationPolicy {
        radius: 1,
        stride: 8,
        rounds: 128,
        max_sample_pairs: 64_000_000,
    };
    let sampling = ProjectiveSamplingPolicy {
        trials: 2048,
        seed: 0x1234abcd,
    };
    let r = ProjectivePortfolioFilePolicy {
        local,
        filter,
        registration: ProjectiveRegistrationPortfolioPolicy {
            anchored: ProjectiveRegistrationTrustPolicy {
                registration,
                photometric,
                maximum_corner_shift: 1.,
            },
            unanchored: registration,
            max_sample_pairs: 128_000_000,
        },
        spatial: None,
        sampling: Some(sampling),
    };
    let mut multiscale = local;
    multiscale.matching.max_comparisons = 384 * 384;
    multiscale.geometry.max_points = 384;
    let p = ProjectivePyramidPhotometricFilePolicy {
        local: multiscale,
        max_levels: 3,
        max_total_pixels: 400_000,
        max_total_features: 384,
        sampling,
        photometric,
        filter: ColorFilterPolicy {
            filter: FilterPolicy {
                max_sample_pairs: 12_000_000,
                ..filter.filter
            },
            ..filter
        },
        fit_mode: PhotometricFitMode::ConstrainedLeastSquares,
    };
    let policy = ProjectiveComplementaryFilePolicy {
        registration: r,
        pyramid: p,
        max_total_comparisons: 128 * 128 + 384 * 384,
        max_total_hypotheses: 4096,
        max_total_sample_pairs: 144_000_000,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);

    use rrrah_dedup::{gradient::GradientMatchPolicy, local_scan::*};
    let q = ProjectiveComplementaryFilterPortfolioPolicy {
        searches: policy,
        secondary: ColorFilterPolicy {
            filter: FilterPolicy {
                radius: 3,
                max_sample_pairs: 32_000_000,
            },
            color_space: FilterColorSpace::LinearSrgb,
        },
        max_total_sample_pairs: 176_000_000,
    };
    let gradient = ProjectiveGradientPyramidFilePolicy {
        search: p,
        matching: GradientMatchPolicy {
            max_comparisons: 384 * 384,
            max_squared_distance: 0.5,
            squared_ratio: 0.64,
        },
        max_total_gradient_samples: 128 * 3 * 512,
    };
    let combined = ProjectiveComplementaryGradientPolicy {
        base: q,
        gradient,
        max_total_comparisons: policy.max_total_comparisons + 384 * 384,
        max_total_hypotheses: policy.max_total_hypotheses + p.sampling.trials,
        max_total_sample_pairs: q.max_total_sample_pairs + p.filter.filter.max_sample_pairs,
        max_total_gradient_samples: 2 * gradient.max_total_gradient_samples,
    };

    let five = ProjectiveComplementaryGradientPortfolioPolicy {
        primary: combined,
        interpolated: gradient,
        max_total_comparisons: combined.max_total_comparisons + gradient.matching.max_comparisons,
        max_total_hypotheses: combined.max_total_hypotheses + gradient.search.sampling.trials,
        max_total_sample_pairs: combined.max_total_sample_pairs
            + gradient.search.filter.filter.max_sample_pairs,
        max_total_gradient_samples: combined.max_total_gradient_samples
            + 2 * gradient.max_total_gradient_samples,
    };

    use rrrah_dedup::{
        local_collection::{
            ProjectiveFiveSearchCollectionPolicy, scan_projective_local_collection_five_searches,
        },
        local_index::FileFeatureBudgets,
    };
    let mut files = vec![
        (1, a.clone()),
        (2, b.clone()),
        (3, DecodeRequest::new(source.join("5495-base.png"))),
    ];
    let config = ProjectiveFiveSearchCollectionPolicy {
        search: five,
        budgets: FileFeatureBudgets {
            max_files: 3,
            max_features: 3840,
            max_hits: 3_000_000,
            max_pair_counts: 3,
            max_pairs: 3,
        },
        max_gradient_retrieval_comparisons: 2_654_208,
    };
    use rrrah_dedup::local_collection::{
        ProjectiveFiveRegionCollectionPolicy, scan_projective_local_collection_five_regions,
    };
    let mut region_search = five.primary.base.searches.pyramid;
    region_search.filter = five.primary.base.secondary;
    let regional_work = 5 * region_search.filter.filter.max_sample_pairs;
    let joined = ProjectiveFiveRegionCollectionPolicy {
        five: config,
        grid: (2, 2),
        max_regions: 4,
        max_total_comparisons: five.max_total_comparisons + region_search.local.matching.max_comparisons,
        max_total_hypotheses: five.max_total_hypotheses + region_search.sampling.trials,
        max_total_sample_pairs: five.max_total_sample_pairs + regional_work,
    };
    let zero_memory = MemoryBudget::new(0);
    let no_files =
        scan_projective_local_collection_five_regions(std::iter::empty(), joined, &zero_memory, || false)
            .unwrap();
    assert!(no_files.local.pairs.is_empty());
    assert_eq!(zero_memory.peak(), 0);
    drop(no_files);
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let empty =
        scan_projective_local_collection_five_regions(std::iter::empty(), joined, &budget, || false).unwrap();
    assert!(empty.analysed.is_empty() && empty.local.pairs.is_empty());
    drop(empty);
    assert_eq!(budget.used(), 0);
    let missing = DecodeRequest::new("missing-joined-admission.png");
    for field in 0..3 {
        let mut short = joined;
        match field {
            0 => short.max_total_comparisons -= 1,
            1 => short.max_total_hypotheses -= 1,
            _ => short.max_total_sample_pairs -= 1,
        };
        let fresh = MemoryBudget::new(64 * 1024 * 1024);
        assert!(matches!(
            scan_projective_local_collection_five_regions([(1, missing.clone())], short, &fresh, || false),
            Err(rrrah_dedup::scan::ScanError::Budget)
        ));
        assert_eq!(fresh.peak(), 0);
        assert_eq!(fresh.used(), 0);
    }
    let mut overflow = joined;
    overflow.five.search.max_total_comparisons = u64::MAX;
    overflow.max_total_comparisons = u64::MAX;
    assert!(matches!(
        scan_projective_local_collection_five_regions([(1, missing.clone())], overflow, &budget, || false),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    assert_eq!(budget.used(), 0);
    let mut zero = joined;
    zero.grid = (0, 2);
    assert!(matches!(
        scan_projective_local_collection_five_regions([(1, missing.clone())], zero, &budget, || false),
        Err(rrrah_dedup::scan::ScanError::InvalidPolicy)
    ));
    let mut cells = joined;
    cells.max_regions = 3;
    assert!(matches!(
        scan_projective_local_collection_five_regions([(1, missing)], cells, &budget, || false),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    assert_eq!(budget.used(), 0);
    use rrrah_dedup::local_collection::scan_projective_local_collection_five_all_regions;
    let mut all = joined;
    all.max_total_sample_pairs = all
        .max_total_sample_pairs
        .checked_add(5 * five.primary.gradient.search.filter.filter.max_sample_pairs)
        .unwrap()
        .checked_add(5 * five.interpolated.search.filter.filter.max_sample_pairs)
        .unwrap();
    let zero_memory = MemoryBudget::new(0);
    let empty =
        scan_projective_local_collection_five_all_regions(std::iter::empty(), all, &zero_memory, || false)
            .unwrap();
    assert!(empty.local.pairs.is_empty());
    drop(empty);
    assert_eq!(zero_memory.peak(), 0);
    for field in 0..3 {
        let mut short = all;
        match field {
            0 => short.max_total_comparisons -= 1,
            1 => short.max_total_hypotheses -= 1,
            _ => short.max_total_sample_pairs -= 1,
        };
        let fresh = MemoryBudget::new(64 * 1024 * 1024);
        assert!(matches!(
            scan_projective_local_collection_five_all_regions(
                [(1, DecodeRequest::new("missing-all-regions.png"))],
                short,
                &fresh,
                || false
            ),
            Err(rrrah_dedup::scan::ScanError::Budget)
        ));
        assert_eq!(fresh.peak(), 0);
    }

    use rrrah_dedup::local_collection::scan_projective_local_collection_five_bidirectional_gradient_regions;
    let mut symmetric = all;
    symmetric.max_regions = 8;
    symmetric.max_total_sample_pairs += 4 * five.primary.gradient.search.filter.filter.max_sample_pairs
        + 4 * five.interpolated.search.filter.filter.max_sample_pairs;
    let zero_memory = MemoryBudget::new(0);
    let empty = scan_projective_local_collection_five_bidirectional_gradient_regions(
        std::iter::empty(),
        symmetric,
        &zero_memory,
        || false,
    )
    .unwrap();
    drop(empty);
    assert_eq!(zero_memory.peak(), 0);
    for field in 0..4 {
        let mut short = symmetric;
        match field {
            0 => short.max_total_comparisons -= 1,
            1 => short.max_total_hypotheses -= 1,
            2 => short.max_total_sample_pairs -= 1,
            _ => short.max_regions = 7,
        };
        let fresh = MemoryBudget::new(64 * 1024 * 1024);
        assert!(matches!(
            scan_projective_local_collection_five_bidirectional_gradient_regions(
                [(1, DecodeRequest::new("missing-bidirectional-regions.png"))],
                short,
                &fresh,
                || false
            ),
            Err(rrrah_dedup::scan::ScanError::Budget)
        ));
        assert_eq!(fresh.peak(), 0);
    }

    use rrrah_dedup::local_collection::{
        ProjectiveSixRegionCollectionPolicy, ProjectiveSixSearchCollectionPolicy,
        scan_projective_local_collection_six_regions, scan_projective_local_collection_six_searches,
    };
    let six = ProjectiveSixSearchCollectionPolicy {
        five: config,
        spatial: rrrah_dedup::local_scan::SpatialFeaturePolicy {
            columns: 4,
            rows: 4,
            max_per_cell: 16,
        },
        max_total_comparisons: five.max_total_comparisons + five.interpolated.matching.max_comparisons,
        max_total_hypotheses: five.max_total_hypotheses + five.interpolated.search.sampling.trials,
        max_total_sample_pairs: five.max_total_sample_pairs
            + five.interpolated.search.filter.filter.max_sample_pairs,
    };
    let six_regions = ProjectiveSixRegionCollectionPolicy {
        six,
        grid: (2, 2),
        max_regions: 8,
        max_total_comparisons: six.max_total_comparisons + region_search.local.matching.max_comparisons,
        max_total_hypotheses: six.max_total_hypotheses + region_search.sampling.trials,
        max_total_sample_pairs: six.max_total_sample_pairs
            + 5 * region_search.filter.filter.max_sample_pairs
            + 9 * five.primary.gradient.search.filter.filter.max_sample_pairs
            + 18 * five.interpolated.search.filter.filter.max_sample_pairs,
    };
    let zero = MemoryBudget::new(0);
    drop(scan_projective_local_collection_six_searches(std::iter::empty(), six, &zero, || false).unwrap());
    drop(
        scan_projective_local_collection_six_regions(std::iter::empty(), six_regions, &zero, || false)
            .unwrap(),
    );
    assert_eq!(zero.peak(), 0);
    assert!(matches!(
        scan_projective_local_collection_six_regions(std::iter::empty(), six_regions, &zero, || true),
        Err(rrrah_dedup::scan::ScanError::Cancelled)
    ));
    assert_eq!(zero.used(), 0);
    for field in 0..4 {
        let mut short = six_regions;
        match field {
            0 => short.max_total_comparisons -= 1,
            1 => short.max_total_hypotheses -= 1,
            2 => short.max_total_sample_pairs -= 1,
            _ => short.max_regions = 7,
        };
        let fresh = MemoryBudget::new(64 * 1024 * 1024);
        assert!(matches!(
            scan_projective_local_collection_six_regions(
                [(1, DecodeRequest::new("missing-six-admission.png"))],
                short,
                &fresh,
                || false
            ),
            Err(rrrah_dedup::scan::ScanError::Budget)
        ));
        assert_eq!(fresh.peak(), 0);
    }
    let mut overflow = six_regions;
    overflow.six.max_total_sample_pairs = u64::MAX;
    assert!(matches!(
        scan_projective_local_collection_six_regions(
            [(1, DecodeRequest::new("missing-six-overflow.png"))],
            overflow,
            &zero,
            || false
        ),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    assert_eq!(zero.peak(), 0);
}
