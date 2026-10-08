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
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 2);
    let mut search = rrrah_dedup::local_scan::ProjectiveCandidateUnionRegionsFilePolicy {
        search: original_regions_config(true, true),
        low_contrast_corner_score: 0.000001,
        smoothing_radius: 2,
        max_smoothing_taps_per_image: 64000000,
        max_area_taps_per_extraction: 128000000,
        max_total_gradient_samples: 28672000,
        max_total_match_comparisons: 784000000,
        max_union_points: 28000,
        max_union_comparisons: 392000000,
    };
    search.search.gradient.search.local.pixels.max_source_pixels = 12800000;
    search
        .search
        .gradient
        .search
        .photometric
        .residual
        .max_source_pixels = 12800000;
    let grid = rrrah_dedup::affine_region_grid::AffineGridPolicy {
        region: rrrah_dedup::affine_region::AffineRegionPolicy {
            radius: 8,
            maximum_sites: 100000,
            maximum_pixel_reads: 500000000,
            color: rrrah_dedup::affine_color::AffineColorPolicy {
                minimum_samples: 1000,
                maximum_samples: 100000,
                minimum_variance: 1e-5,
                minimum_relative_pivot: 1e-6,
                maximum_coefficient: 5.,
                maximum_offset: 0.1,
            },
            tolerance: 0.03,
        },
        grid: (8, 8),
        maximum_regions: 64,
        maximum_source_radius: 16,
        maximum_total_pixel_reads: 64000000000,
    };
    let factors: Vec<_> = (0..=6).map(|n| 2f64.powf(f64::from(n) / 2.)).collect();
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let result = rrrah_dedup::affine_region_file::search_affine_color_regions(
        &rrrah_decode::DecodeRequest::new(&args[0]),
        &rrrah_decode::DecodeRequest::new(&args[1]),
        search,
        &factors,
        grid,
        &budget,
        || false,
    );
    let mut detail = "null".to_string();
    let (status, points, inliers, matrix, regions, supports) = match result {
        Ok(e) => {
            let pairs = e
                .search
                .correspondences
                .iter()
                .map(|p| format!("{{\"source\":{:?},\"target\":{:?}}}", p.source, p.target))
                .collect::<Vec<_>>()
                .join(",");
            let inlier_ids = e
                .search
                .geometry
                .as_ref()
                .map_or("[]".to_string(), |g| format!("{:?}", &*g.inliers));
            let rows=e.color_regions.as_ref().map_or(String::new(),|regions|regions.iter().map(|row|{
                let domains=row.domains.map(|v|[v.x,v.y,v.width,v.height]);
                let directions=row.directions.map(|r|match r {
                    Ok(v)=>format!("{{\"training_samples\":{},\"samples\":{},\"matched\":{},\"squared_error\":{},\"matrix\":{:?},\"offset\":{:?},\"pixel_reads\":{}}}",v.training_samples,v.heldout.samples,v.heldout.matched,v.heldout.squared_error,v.model.matrix,v.model.offset,v.pixel_reads),
                    Err(error)=>format!("{{\"error\":\"{error:?}\"}}"),
                });
                format!("{{\"domains\":{domains:?},\"source_radius\":{},\"directions\":[{},{}]}}",row.source_radius,directions[0],directions[1])
            }).collect::<Vec<_>>().join(","));
            detail =
                format!("{{\"correspondences\":[{pairs}],\"inlier_ids\":{inlier_ids},\"regions\":[{rows}]}}");
            let count = e.color_regions.as_ref().map_or(0, |r| r.len());
            let supports = e.color_regions.as_ref().map_or(0, |r| {
                r.iter()
                    .filter(|row| {
                        row.directions.iter().all(|v| {
                            v.as_ref()
                                .is_ok_and(|v| v.heldout.matched as f64 >= 0.9 * v.heldout.samples as f64)
                        })
                    })
                    .count()
            });
            let matrix = e
                .search
                .geometry
                .as_ref()
                .map_or("null".to_string(), |g| format!("{:?}", g.transform.matrix));
            (
                "ok".to_string(),
                e.search.correspondences.len(),
                e.search.geometry.as_ref().map_or(0, |g| g.inliers.len()),
                matrix,
                count,
                supports,
            )
        }
        Err(e) => (format!("refusal:{e:?}"), 0, 0, "null".to_string(), 0, 0),
    };
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"{status}\",\"points\":{points},\"inliers\":{inliers},\"matrix\":{matrix},\"regions\":{regions},\"supports\":{supports},\"detail\":{detail},\"managed_used\":0,\"managed_peak\":{}}}",
        budget.peak()
    );
}
