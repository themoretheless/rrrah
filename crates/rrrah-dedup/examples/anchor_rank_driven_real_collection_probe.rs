use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
    gradient::{GradientCellRecipe, GradientMatchPolicy},
    local::{LocalPolicy, MatchPolicy},
    local_scan::{
        LocalFilePolicy, ProjectiveGradientPyramidFilePolicy, ProjectivePyramidPhotometricFilePolicy,
    },
    warp::{
        ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricFitMode, PhotometricPolicy, WarpPolicy,
    },
};
fn policy() -> ProjectiveGradientPyramidFilePolicy {
    let warp = WarpPolicy {
        tolerance: 0.03,
        max_source_pixels: 409600,
    };
    ProjectiveGradientPyramidFilePolicy {
        search: ProjectivePyramidPhotometricFilePolicy {
            local: LocalFilePolicy {
                decode: AnimationBudget {
                    max_frames: 1,
                    max_pixels: 200000,
                    max_file_bytes: 1048576,
                },
                extract: LocalPolicy {
                    max_pixels: 200000,
                    max_candidates: 200000,
                    max_features: 500,
                    minimum_corner_score: 0.0001,
                },
                matching: MatchPolicy {
                    max_comparisons: 4000000,
                    max_distance: 64,
                },
                geometry: GeometryPolicy {
                    tolerance: 2.,
                    min_inliers: 10,
                    max_points: 2000,
                    max_hypotheses: 4096,
                },
                pixels: warp,
                minimum_compared_pixels: 1000,
                minimum_coverage_fraction: 0.3,
                minimum_matched_fraction: 0.9,
            },
            max_levels: 4,
            max_total_pixels: 400000,
            max_total_features: 2000,
            sampling: ProjectiveSamplingPolicy {
                trials: 4096,
                seed: 0x1234abcd,
            },
            photometric: PhotometricPolicy {
                residual: warp,
                minimum_samples: 16,
                minimum_variance: 1e-5,
                minimum_gain: 0.2,
                maximum_gain: 5.,
                maximum_offset: 0.1,
            },
            filter: ColorFilterPolicy {
                filter: FilterPolicy {
                    radius: 3,
                    max_sample_pairs: 32000000,
                },
                color_space: FilterColorSpace::LinearSrgb,
            },
            fit_mode: PhotometricFitMode::ConstrainedLeastSquares,
        },
        matching: GradientMatchPolicy {
            max_comparisons: 4000000,
            max_squared_distance: 0.5,
            squared_ratio: 0.64,
        },
        max_total_gradient_samples: 1024000,
    }
}
fn original_regions_config(
    blur7: bool,
    grid8: bool,
) -> rrrah_dedup::local_scan::ProjectiveDistinctScaleRegionsFilePolicy {
    let mut p = policy();
    p.search.local.decode.max_pixels = 6400000;
    p.search.local.decode.max_file_bytes = 5242880;
    p.search.local.extract.max_pixels = 6400000;
    p.search.local.extract.max_candidates = 6400000;
    p.search.local.extract.max_features = 2000;
    p.search.local.pixels.max_source_pixels = 8000000;
    p.search.local.geometry.max_points = 14000;
    p.search.max_levels = 7;
    p.search.max_total_features = 14000;
    p.search.max_total_pixels = 12800000;
    p.search.photometric.residual.max_source_pixels = 8000000;
    p.search.filter.filter.max_sample_pairs = 1200000000;
    if blur7 {
        p.search.filter.filter.radius = 7;
        p.search.filter.filter.max_sample_pairs = 16000000000;
    }
    p.max_total_gradient_samples = 7168000;
    p.matching.max_comparisons = 392000000;
    rrrah_dedup::local_scan::ProjectiveDistinctScaleRegionsFilePolicy {
        gradient: p,
        recipe: GradientCellRecipe::Interpolated,
        spatial: rrrah_dedup::local_scan::SpatialFeaturePolicy {
            columns: 4,
            rows: 4,
            max_per_cell: 128,
        },
        competitor_radius: 2.,
        grid: if grid8 { (8, 8) } else { (4, 4) },
        max_regions: if grid8 { 128 } else { 32 },
        max_total_sample_pairs: (if grid8 { 130 } else { 34 }) * p.search.filter.filter.max_sample_pairs,
    }
}

fn main() {
    use rrrah_dedup::{anchor_rank::AnchorRankPolicy,anchor_rank_file::{search_anchor_rank_geometry_fallback_files,AnchorGeometryFallbackPolicy},local_rank::LocalRankPolicy,rank_region::RankRegionPolicy};
    let args:Vec<_>=std::env::args().skip(1).collect();assert_eq!(args.len(),4);
    let source_tolerance:f64=args[3].parse().unwrap();
    let mut config = rrrah_dedup::local_scan::ProjectiveCandidateUnionRegionsFilePolicy {
        search: original_regions_config(true, true),
        low_contrast_corner_score: 0.000001,
        smoothing_radius: 2,
        max_smoothing_taps_per_image: 128000000,
        max_area_taps_per_extraction: 128000000,
        max_total_gradient_samples: 28672000,
        max_total_match_comparisons: 784000000,
        max_union_points: 28000,
        max_union_comparisons: 392000000,
    };
    {
        config.search.gradient.search.local.pixels.max_source_pixels = 12800000;
        config
            .search
            .gradient
            .search
            .photometric
            .residual
            .max_source_pixels = 12800000;
    }

    config.search.gradient.search.filter.filter.radius=7;
    let factors:Vec<_>=(0..=6).map(|n|2f64.powf(f64::from(n)/2.)).collect();
    let budget=rrrah_core::MemoryBudget::new(512*1024*1024);
    let fallback=rrrah_dedup::local_scan::ProjectiveCandidateUnionFallbackFilePolicy { search:config,asymmetric_radius:4,max_total_gradient_samples:3*config.max_total_gradient_samples,max_total_match_comparisons:3*config.max_total_match_comparisons,max_total_union_comparisons:3*config.max_union_comparisons };
    let policy=AnchorRankPolicy {local:LocalRankPolicy {rank:RankRegionPolicy {radius:8,minimum_contrast:0.005,minimum_pairs:1000,maximum_sites:80000000,maximum_pixel_reads:400000000},filter_radius:8,minimum_witnesses:10,maximum_points:28000,maximum_point_checks:392014000,minimum_point_separation:2.,target_tolerance:2.,source_tolerance,minimum_coverage:0.3,minimum_agreement:0.9},window_radius:2,maximum_selection_checks:1176014000};
    let native_policy=AnchorGeometryFallbackPolicy {search:fallback,anchors:policy,
        maximum_total_point_checks:3*policy.local.maximum_point_checks,
        maximum_total_selection_checks:3*policy.maximum_selection_checks,
        maximum_total_rank_sites:3*policy.local.rank.maximum_sites,
        maximum_total_pixel_reads:3*policy.local.rank.maximum_pixel_reads};
    let files:Vec<_>=(0..3).map(|i|((i+1) as u64,rrrah_decode::DecodeRequest::new(&args[i]))).collect();
    let collection_policy=rrrah_dedup::local_collection::AnchorGeometryFallbackCollectionPolicy {
        search:native_policy,
        budgets:rrrah_dedup::local_index::FileFeatureBudgets {max_files:3,max_features:336000,max_hits:12000000000,max_pair_counts:3,max_pairs:3},
        max_retrieval_comparisons:12000000000,
    };
    let report=rrrah_dedup::local_collection::scan_anchor_rank_geometry_fallback_collection(files.clone(),collection_policy,&factors,&budget,||false).expect("collection refused");
    assert!(report.file_issues.is_empty() && report.local.issues.is_empty() && report.local.source_issues.is_empty(),"{report:?}");
    for i in 0..3 {for j in i+1..3 {
        let direct=search_anchor_rank_geometry_fallback_files(&files[i].1,&files[j].1,native_policy,&factors,&budget,||false).expect("direct refused");
        let supported=direct.evidence.anchors.is_some_and(|a|a.supported);
        let indexed=report.local.pairs.iter().find(|p|(p.left,p.right)==(files[i].0,files[j].0));
        if let Some(pair)=indexed {
            let e=&pair.evidence;
            assert_eq!(e.attempted_recipes,direct.attempted_recipes);
            assert_eq!(e.smoothing_radii,direct.smoothing_radii);
            assert_eq!(e.evidence.anchors,direct.evidence.anchors);
            let points=|e:&rrrah_dedup::local_scan::ManagedCandidateUnionGeometryFileEvidence|e.correspondences.iter().map(|p|(p.source,p.target)).collect::<Vec<_>>();
            assert_eq!(points(&e.evidence.search),points(&direct.evidence.search));
            assert_eq!(format!("{:?}",e.evidence.search.geometry),format!("{:?}",direct.evidence.search.geometry));
        } else {assert!(!supported,"index missed supported pair");}
        assert_eq!(supported,(i,j)==(0,1),"unexpected support {i}-{j}");
        println!("{{\"left\":{},\"right\":{},\"indexed\":{},\"supported\":{},\"direct_parity\":true}}",files[i].0,files[j].0,indexed.is_some(),supported);
        drop(direct);
    }}
    drop(report);
    assert_eq!(budget.used(),0);
    println!("{{\"status\":\"complete_three_real_file_parity\",\"memory_used\":0}}");
}
