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
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 3);
    diagnose_screen_regional_model_pixels(&args[0], &args[1], &args[2]);
}
fn region_pixel_diagnostic(e: &rrrah_dedup::local_scan::ProjectiveTransformedRegionsFileEvidence) -> String {
    let rows: Vec<_> = e
        .regions
        .iter()
        .map(|r| {
            let domains = r.domains.map(|d| [d.x, d.y, d.width, d.height]);
            let failure = r.fit_failure.map_or("null".to_string(), |f| format!("\"{f:?}\""));
            let pixels = r.pixels.as_ref().map_or("null".to_string(), |p| {
                let f = &p.fitted.forward;
                let b = &p.fitted.reverse;
                format!(
                    "{{\"counts\":[{:?},{:?}],\"gains\":[{:?},{:?}],\"offsets\":[{:?},{:?}]}}",
                    [
                        f.pixels.matched_pixels,
                        f.pixels.compared_pixels,
                        f.pixels.source_pixels
                    ],
                    [
                        b.pixels.matched_pixels,
                        b.pixels.compared_pixels,
                        b.pixels.source_pixels
                    ],
                    f.gain,
                    b.gain,
                    f.offset,
                    b.offset
                )
            });
            format!(
                "{{\"accepted\":{},\"domains\":{domains:?},\"fit_failure\":{failure},\"pixels\":{pixels}}}",
                r.accepted_region
            )
        })
        .collect();
    format!("[{}]", rows.join(","))
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

fn diagnose_screen_regional_model_pixels(left: &str, right: &str, input: &str) {
    let text = std::fs::read_to_string(input).unwrap();
    let mut p = original_regions_config(true, true);
    p.gradient.search.local.pixels.max_source_pixels = 12800000;
    p.gradient.search.photometric.residual.max_source_pixels = 12800000;
    p.gradient.search.filter.filter.radius = 8;
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let mut rows = Vec::new();
    for line in text.lines() {
        let values: Vec<f64> = line.split_whitespace().map(|v| v.parse().unwrap()).collect();
        assert_eq!(values.len(), 13);
        assert!(values.iter().all(|v| v.is_finite()));
        let domain = [values[0], values[1], values[2], values[3]];
        let matrix: [[f64; 3]; 3] = std::array::from_fn(|r| std::array::from_fn(|c| values[4 + r * 3 + c]));
        let result =
            rrrah_dedup::local_scan::compare_local_files_projective_bidirectional_region_grid_transform(
                &rrrah_decode::DecodeRequest::new(left),
                &rrrah_decode::DecodeRequest::new(right),
                rrrah_dedup::geometry::ProjectiveTransform { matrix },
                rrrah_dedup::local_scan::ProjectivePyramidRegionsFilePolicy {
                    search: p.gradient.search,
                    max_regions: 128,
                    max_total_sample_pairs: 129 * p.gradient.search.filter.filter.max_sample_pairs,
                },
                (8, 8),
                &budget,
                || false,
            );
        match result {
            Ok(e) => {
                let local = e
                    .regions
                    .iter()
                    .filter(|r| {
                        let d = r.domains[1];
                        r.accepted_region
                            && f64::from(d.x) >= domain[0]
                            && f64::from(d.y) >= domain[1]
                            && f64::from(d.x) + f64::from(d.width) <= domain[0] + domain[2]
                            && f64::from(d.y) + f64::from(d.height) <= domain[1] + domain[3]
                    })
                    .count();
                rows.push(format!("{{\"fit_target_domain\":{domain:?},\"matrix\":{matrix:?},\"regions\":{},\"region_support_count\":{},\"local_supported_count\":{local}}}",region_pixel_diagnostic(&e),e.region_support_count()));
                drop(e);
            }
            Err(error) => rows.push(format!(
                "{{\"fit_target_domain\":{domain:?},\"matrix\":{matrix:?},\"error\":\"{error:?}\"}}"
            )),
        }
        assert_eq!(budget.used(), 0);
    }
    println!(
        "{{\"status\":\"ok\",\"experiments\":[{}],\"managed_used\":0,\"managed_peak\":{}}}",
        rows.join(","),
        budget.peak()
    );
}
