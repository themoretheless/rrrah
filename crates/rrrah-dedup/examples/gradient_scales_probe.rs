use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
    gradient::{GradientCellRecipe, GradientMatchPolicy},
    local::{LocalPolicy, MatchPolicy},
    local_scan::{
        LocalFilePolicy, ProjectiveGradientPyramidFilePolicy, ProjectivePyramidPhotometricFilePolicy,
        compare_local_files_projective_gradient_scales_with_recipe,
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
    if args
        .first()
        .is_some_and(|mode| mode == "screen-regional-model-pixels")
    {
        assert_eq!(args.len(), 4);
        diagnose_screen_regional_model_pixels(&args[1], &args[2], &args[3]);
        return;
    }
    if args.first().is_some_and(|mode| mode == "screen-translation-grid") {
        assert_eq!(args.len(), 4);
        diagnose_screen_translation_grid(&args[1], &args[2], &args[3]);
        return;
    }
    assert!(args.len() == 3);
    if args[0] == "original-managed-candidate-union-fallback-collection" {
        diagnose_candidate_union_fallback(&args[1], &args[2], true);
        return;
    }
    if args[0] == "original-managed-candidate-union-fallback" {
        diagnose_candidate_union_fallback(&args[1], &args[2], false);
        return;
    }
    if args[0] == "original-managed-candidate-union-asymmetric4-reverse" {
        diagnose_fresh_candidate_union(
            &args[1], &args[2], false, 4, false, false, true, false, true, true,
        );
        return;
    }
    if args[0] == "original-managed-candidate-union-asymmetric4" {
        diagnose_fresh_candidate_union(
            &args[1], &args[2], false, 4, false, false, true, false, true, false,
        );
        return;
    }
    if args[0] == "original-managed-candidate-union-filter8" {
        diagnose_fresh_candidate_union(
            &args[1], &args[2], false, 2, false, false, true, true, false, false,
        );
        return;
    }
    if args[0] == "original-managed-candidate-union-domain-budget" {
        diagnose_fresh_candidate_union(
            &args[1], &args[2], false, 2, false, false, true, false, false, false,
        );
        return;
    }
    if args[0] == "original-managed-candidate-union-encoded" {
        diagnose_fresh_candidate_union(
            &args[1], &args[2], false, 2, false, true, false, false, false, false,
        );
        return;
    }
    if args[0] == "original-managed-candidate-union-fine-scales" {
        diagnose_fresh_candidate_union(
            &args[1], &args[2], false, 2, true, false, false, false, false, false,
        );
        return;
    }
    if args[0] == "original-managed-candidate-union-smooth4" {
        diagnose_fresh_candidate_union(
            &args[1], &args[2], false, 4, false, false, false, false, false, false,
        );
        return;
    }
    if [
        "original-managed-candidate-union",
        "original-managed-candidate-union-collection",
    ]
    .contains(&args[0].as_str())
    {
        diagnose_fresh_candidate_union(
            &args[1],
            &args[2],
            args[0].ends_with("-collection"),
            2,
            false,
            false,
            false,
            false,
            false,
            false,
        );
        return;
    }
    if args[0] == "original-correspondence-union" {
        diagnose_correspondence_union(&args[1], &args[2]);
        return;
    }
    if args[0] == "original-smooth-candidates" {
        diagnose_smoothed_candidates(&args[1], &args[2]);
        return;
    }
    if args[0] == "original-union-collection-six" {
        diagnose_original_union_collection_six(&args[1]);
        return;
    }
    if args[0] == "original-area-collection-six" {
        diagnose_original_area_collection_six(&args[1]);
        return;
    }
    if [
        "original-managed-regions",
        "original-managed-collection",
        "original-managed-regions-blur7",
        "original-managed-regions-area",
        "original-managed-regions-area-blur7",
        "original-managed-regions-area-grid8-blur7",
        "original-managed-collection-area-grid8-blur7",
        "original-managed-regions-area-grid8-lowcontrast-blur7",
    ]
    .contains(&args[0].as_str())
    {
        diagnose_original_managed_regions(
            &args[1],
            &args[2],
            args[0].contains("-collection"),
            args[0].ends_with("-blur7"),
            args[0].contains("-area"),
            args[0].contains("-grid8"),
            args[0].contains("-lowcontrast"),
        );
        return;
    }
    if [
        "scale-ratio",
        "scale-distinct",
        "scale-distinct-wide",
        "scale-distinct-original",
    ]
    .contains(&args[0].as_str())
    {
        diagnose_scale_ratio(
            &args[1],
            &args[2],
            args[0].starts_with("scale-distinct"),
            args[0].ends_with("-wide") || args[0].ends_with("-original"),
            args[0].ends_with("-original"),
        );
        return;
    }
    if [
        "spatial-regions",
        "spatial-regions-refined",
        "spatial-regions-dense",
    ]
    .contains(&args[0].as_str())
    {
        diagnose_spatial_regions(
            &args[1],
            &args[2],
            args[0].ends_with("-refined"),
            args[0].ends_with("-dense"),
        );
        return;
    }
    if args[0] == "geometry" {
        diagnose_geometry(&args[1], &args[2]);
        return;
    }
    assert!(["direct", "collection", "spatial-direct", "spatial-collection"].contains(&args[0].as_str()));
    let spatial = args[0].starts_with("spatial-");
    let grid = rrrah_dedup::local_scan::SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 32,
    };
    let a = rrrah_decode::DecodeRequest::new(&args[1]);
    let b = rrrah_decode::DecodeRequest::new(&args[2]);
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let e = if args[0].ends_with("direct") {
        if spatial {
            rrrah_dedup::local_scan::compare_local_files_projective_spatial_gradient_scales_with_recipe(
                &a,
                &b,
                policy(),
                GradientCellRecipe::Interpolated,
                &factors,
                grid,
                &budget,
                || false,
            )
            .unwrap()
        } else {
            compare_local_files_projective_gradient_scales_with_recipe(
                &a,
                &b,
                policy(),
                GradientCellRecipe::Interpolated,
                &factors,
                &budget,
                || false,
            )
            .unwrap()
        }
    } else {
        use rrrah_dedup::{
            local_collection::{
                ProjectiveGradientCollectionPolicy, scan_projective_local_collection_gradient_scales,
            },
            local_index::FileFeatureBudgets,
        };
        let config = ProjectiveGradientCollectionPolicy {
            search: policy(),
            recipe: GradientCellRecipe::Interpolated,
            budgets: FileFeatureBudgets {
                max_files: 2,
                max_features: 4000,
                max_hits: 16000000,
                max_pair_counts: 1,
                max_pairs: 1,
            },
            max_retrieval_comparisons: 16000000,
        };
        let report = if spatial {
            rrrah_dedup::local_collection::scan_projective_local_collection_spatial_gradient_scales(
                [(1, a), (2, b)],
                config,
                &factors,
                grid,
                &budget,
                || false,
            )
        } else {
            scan_projective_local_collection_gradient_scales(
                [(1, a), (2, b)],
                config,
                &factors,
                &budget,
                || false,
            )
        }
        .unwrap();
        assert!(
            report.file_issues.is_empty()
                && report.local.issues.is_empty()
                && report.local.source_issues.is_empty()
        );
        if report.local.pairs.is_empty() {
            drop(report);
            assert_eq!(budget.used(), 0);
            println!(
                "{{\"status\":\"ok\",\"retrieved\":false,\"candidate\":false,\"managed_used\":0,\"managed_peak\":{}}}",
                budget.peak()
            );
            return;
        }
        assert_eq!(report.local.pairs.len(), 1);
        report.local.pairs.into_iter().next().unwrap().evidence
    };
    let geometry = e
        .geometry
        .as_ref()
        .map_or("null".to_string(), |v| format!("{:?}", v.transform.matrix));
    let pixels = e.pixels.as_ref().map_or("null".to_string(), |v| {
        format!(
            "[{:?},{:?}]",
            [
                v.fitted.forward.pixels.matched_pixels,
                v.fitted.forward.pixels.compared_pixels,
                v.fitted.forward.pixels.source_pixels
            ],
            [
                v.fitted.reverse.pixels.matched_pixels,
                v.fitted.reverse.pixels.compared_pixels,
                v.fitted.reverse.pixels.source_pixels
            ]
        )
    });
    let inliers = e.geometry.as_ref().map_or(0, |v| v.inliers.len());
    let count = e.correspondences.len();
    let candidate = e.candidate;
    drop(e);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"ok\",\"retrieved\":true,\"candidate\":{candidate},\"correspondences\":{count},\"inliers\":{inliers},\"geometry\":{geometry},\"pixels\":{pixels},\"managed_used\":0,\"managed_peak\":{}}}",
        budget.peak()
    );
}

fn diagnose_geometry(left: &str, right: &str) {
    let p = policy();
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let a = rrrah_dedup::decode::decode_selected_frame_bounded(
        &rrrah_decode::DecodeRequest::new(left),
        p.search.local.decode,
        &budget,
        || false,
    )
    .unwrap();
    let b = rrrah_dedup::decode::decode_selected_frame_bounded(
        &rrrah_decode::DecodeRequest::new(right),
        p.search.local.decode,
        &budget,
        || false,
    )
    .unwrap();
    let av = a.view(|| false).unwrap();
    let bv = b.view(|| false).unwrap();
    let pyramid = rrrah_dedup::pyramid::PyramidPolicy {
        local: p.search.local.extract,
        max_levels: 4,
        max_total_pixels: p.search.max_total_pixels,
        max_total_features: 2000,
    };
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let af = rrrah_dedup::gradient_scale::extract_gradient_scales_managed(
        &av,
        pyramid,
        &factors,
        1024000,
        GradientCellRecipe::Interpolated,
        &budget,
        || false,
    )
    .unwrap();
    let bf = rrrah_dedup::gradient_scale::extract_gradient_scales_managed(
        &bv,
        pyramid,
        &factors,
        1024000,
        GradientCellRecipe::Interpolated,
        &budget,
        || false,
    )
    .unwrap();
    let matches = rrrah_dedup::gradient::match_gradients(&af, &bf, p.matching, || false).unwrap();
    let model = rrrah_dedup::geometry::verify_projective_sampled(
        &matches,
        p.search.local.geometry,
        p.search.sampling,
        || false,
    )
    .unwrap()
    .unwrap();
    println!(
        "{{\"correspondences\":{},\"inliers\":{},\"geometry\":{:?}}}",
        matches.len(),
        model.inliers.len(),
        model.transform.matrix
    );
}

fn diagnose_spatial_regions(left: &str, right: &str, refined: bool, dense: bool) {
    use rrrah_dedup::{
        local_collection::{
            ProjectiveGradientCollectionPolicy, ProjectiveSpatialGradientRegionCollectionPolicy,
            scan_projective_local_collection_spatial_gradient_scale_regions,
        },
        local_index::FileFeatureBudgets,
    };
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let factors: Vec<f64> = if dense {
        (0..=6).map(|step| 2f64.powf(f64::from(step) / 4.)).collect()
    } else {
        vec![1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2]
    };
    let mut search = policy();
    if dense {
        // More extraction/matching work is explicitly admitted; acceptance is unchanged.
        search.search.max_levels = 7;
        search.search.max_total_features = 3500;
        search.search.local.geometry.max_points = 3500;
        search.max_total_gradient_samples = 1792000;
        search.matching.max_comparisons = 12250000;
    }
    let gradient = ProjectiveGradientCollectionPolicy {
        search,
        recipe: GradientCellRecipe::Interpolated,
        budgets: FileFeatureBudgets {
            max_files: 2,
            max_features: if dense { 7000 } else { 4000 },
            max_hits: if dense { 49000000 } else { 16000000 },
            max_pair_counts: 1,
            max_pairs: 1,
        },
        max_retrieval_comparisons: if dense { 49000000 } else { 16000000 },
    };
    let config = ProjectiveSpatialGradientRegionCollectionPolicy {
        gradient,
        spatial: rrrah_dedup::local_scan::SpatialFeaturePolicy {
            columns: 4,
            rows: 4,
            max_per_cell: 32,
        },
        grid: (4, 4),
        max_regions: 32,
        max_total_sample_pairs: 1088000000,
    };
    let report = scan_projective_local_collection_spatial_gradient_scale_regions(
        [
            (1, rrrah_decode::DecodeRequest::new(left)),
            (2, rrrah_decode::DecodeRequest::new(right)),
        ],
        config,
        &factors,
        &budget,
        || false,
    )
    .unwrap();
    assert!(
        report.file_issues.is_empty()
            && report.local.issues.is_empty()
            && report.local.source_issues.is_empty()
    );
    let mut rows = String::new();
    let mut support = 0;
    let mut whole = false;
    let mut geometry_diagnostic = "null".to_string();
    if let Some(pair) = report.local.pairs.first() {
        whole = pair.evidence.whole.candidate;
        let e = &pair.evidence.whole;
        let points: Vec<_> = e.correspondences.iter().map(|p| [p.source, p.target]).collect();
        let matrix = e
            .geometry
            .as_ref()
            .map_or("null".to_string(), |g| format!("{:?}", g.transform.matrix));
        let inliers = e
            .geometry
            .as_ref()
            .map_or("[]".to_string(), |g| format!("{:?}", g.inliers));
        geometry_diagnostic =
            format!("{{\"correspondences\":{points:?},\"matrix\":{matrix},\"inliers\":{inliers}}}");
        let refined_regions = if refined {
            if let Some(geometry) = &pair.evidence.whole.geometry {
                let p = policy();
                let a = rrrah_dedup::decode::decode_selected_frame_bounded(
                    &rrrah_decode::DecodeRequest::new(left),
                    p.search.local.decode,
                    &budget,
                    || false,
                )
                .unwrap();
                let b = rrrah_dedup::decode::decode_selected_frame_bounded(
                    &rrrah_decode::DecodeRequest::new(right),
                    p.search.local.decode,
                    &budget,
                    || false,
                )
                .unwrap();
                let av = a.view(|| false).unwrap();
                let bv = b.view(|| false).unwrap();
                let model = rrrah_dedup::warp::refine_projective_pixels_photometric(
                    &av,
                    &bv,
                    geometry.transform,
                    rrrah_dedup::warp::ProjectiveRegistrationPolicy {
                        radius: 3,
                        stride: 4,
                        rounds: 48,
                        max_sample_pairs: 512000000,
                    },
                    p.search.photometric,
                    || false,
                )
                .unwrap();
                Some(rrrah_dedup::local_scan::compare_local_files_projective_bidirectional_region_grid_transform(&rrrah_decode::DecodeRequest::new(left),&rrrah_decode::DecodeRequest::new(right),model,rrrah_dedup::local_scan::ProjectivePyramidRegionsFilePolicy{search:p.search,max_regions:32,max_total_sample_pairs:1056000000},(4,4),&budget,||false).unwrap())
            } else {
                None
            }
        } else {
            None
        };
        if let Some(regions) = refined_regions.as_ref().or(pair.evidence.regions.as_ref()) {
            support = regions.region_support_count();
            rows = regions
                .regions
                .iter()
                .map(|r| {
                    let counts = r.pixels.as_ref().map_or("null".to_string(), |v| {
                        format!(
                            "[{:?},{:?}]",
                            [
                                v.fitted.forward.pixels.matched_pixels,
                                v.fitted.forward.pixels.compared_pixels,
                                v.fitted.forward.pixels.source_pixels
                            ],
                            [
                                v.fitted.reverse.pixels.matched_pixels,
                                v.fitted.reverse.pixels.compared_pixels,
                                v.fitted.reverse.pixels.source_pixels
                            ]
                        )
                    });
                    format!("{{\"accepted\":{},\"counts\":{counts}}}", r.accepted_region)
                })
                .collect::<Vec<_>>()
                .join(",");
        }
    }
    drop(report);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"ok\",\"geometry_diagnostic\":{geometry_diagnostic},\"whole_candidate\":{whole},\"region_support_count\":{support},\"regions\":[{rows}],\"managed_used\":0,\"managed_peak\":{}}}",
        budget.peak()
    );
}

// Diagnostic only: same extraction and thresholds, no new acceptance path.
fn diagnose_scale_ratio(left: &str, right: &str, distinct: bool, wide: bool, original: bool) {
    use rrrah_dedup::gradient_scale::extract_spatial_gradient_scales_managed;
    let mut p = policy();
    let local_features = if wide { 2000usize } else { 500 };
    if wide {
        p.search.local.extract.max_features = local_features;
    }
    let quota = if wide { 128 } else { 32 };
    if original {
        p.search.local.decode.max_pixels = 6400000;
        p.search.local.decode.max_file_bytes = 5242880;
        p.search.local.extract.max_pixels = 6400000;
        p.search.local.extract.max_candidates = 6400000;
        p.search.local.pixels.max_source_pixels = 8000000;
        p.search.photometric.residual.max_source_pixels = 8000000;
        p.search.filter.filter.max_sample_pairs = 1200000000;
    }
    let budget = rrrah_core::MemoryBudget::new(if original {
        512 * 1024 * 1024
    } else {
        64 * 1024 * 1024
    });
    let a = rrrah_dedup::decode::decode_selected_frame_bounded(
        &rrrah_decode::DecodeRequest::new(left),
        p.search.local.decode,
        &budget,
        || false,
    )
    .unwrap();
    let b = rrrah_dedup::decode::decode_selected_frame_bounded(
        &rrrah_decode::DecodeRequest::new(right),
        p.search.local.decode,
        &budget,
        || false,
    )
    .unwrap();
    let av = a.view(|| false).unwrap();
    let bv = b.view(|| false).unwrap();
    let mut rows = Vec::new();
    for levels in [4usize, 7] {
        let factors: Vec<_> = if original {
            if levels == 4 {
                vec![1., 2., 4., 8.]
            } else {
                (0..=6).map(|n| 2f64.powf(f64::from(n) / 2.)).collect()
            }
        } else if levels == 4 {
            vec![1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2]
        } else {
            (0..=6).map(|n| 2f64.powf(f64::from(n) / 4.)).collect()
        };
        let pyramid = rrrah_dedup::pyramid::PyramidPolicy {
            local: p.search.local.extract,
            max_levels: levels,
            max_total_pixels: if original { 12800000 } else { 400000 },
            max_total_features: local_features * levels,
        };
        let af = extract_spatial_gradient_scales_managed(
            &av,
            pyramid,
            &factors,
            (local_features * levels * 512) as u64,
            GradientCellRecipe::Interpolated,
            (4, 4, quota),
            &budget,
            || false,
        )
        .unwrap();
        let bf = extract_spatial_gradient_scales_managed(
            &bv,
            pyramid,
            &factors,
            (local_features * levels * 512) as u64,
            GradientCellRecipe::Interpolated,
            (4, 4, quota),
            &budget,
            || false,
        )
        .unwrap();
        let mut stats = Vec::new();
        for (source, target) in [(&*af, &*bf), (&*bf, &*af)] {
            let mut radius_pass = 0;
            let mut ratio_pass = 0;
            let mut nearby_ratio_refusal = 0;
            for feature in source {
                let mut best = (f64::INFINITY, usize::MAX);
                let mut next = (f64::INFINITY, usize::MAX);
                for (j, t) in target.iter().enumerate() {
                    let distance: f64 = feature
                        .descriptor
                        .0
                        .iter()
                        .zip(t.descriptor.0)
                        .map(|(a, b)| (a - b) * (a - b))
                        .sum();
                    if distance < best.0 {
                        next = best;
                        best = (distance, j);
                    } else if distance < next.0 {
                        next = (distance, j);
                    }
                }
                if best.0 <= 0.5 {
                    radius_pass += 1;
                    if best.0 < 0.64 * next.0 {
                        ratio_pass += 1;
                    } else if next.1 != usize::MAX {
                        let x = target[best.1].position;
                        let y = target[next.1].position;
                        if (x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) <= 4. {
                            nearby_ratio_refusal += 1;
                        }
                    }
                }
            }
            stats.push(format!("{{\"radius_pass\":{radius_pass},\"ratio_pass\":{ratio_pass},\"nearby_ratio_refusal\":{nearby_ratio_refusal}}}"));
        }
        let mut distinct_evidence = "null".to_string();
        if distinct {
            let matches = rrrah_dedup::gradient::match_gradients_distinct_locations(
                &af,
                &bf,
                GradientMatchPolicy {
                    max_comparisons: if wide { 392000000 } else { 24500000 },
                    ..p.matching
                },
                2.,
                || false,
            )
            .unwrap();
            let geometry = rrrah_dedup::geometry::verify_projective_sampled_for_domains(
                &matches,
                GeometryPolicy {
                    max_points: local_features * levels,
                    ..p.search.local.geometry
                },
                p.search.sampling,
                av.dimensions(),
                bv.dimensions(),
                || false,
            )
            .unwrap();
            let matrix = geometry
                .as_ref()
                .map_or("null".to_string(), |g| format!("{:?}", g.transform.matrix));
            let inliers = geometry
                .as_ref()
                .map_or("[]".to_string(), |g| format!("{:?}", g.inliers));
            let points: Vec<_> = matches.iter().map(|p| [p.source, p.target]).collect();
            let mut projective_regions = "[]".to_string();
            let support = if let Some(g) = &geometry {
                let regions = rrrah_dedup::local_scan::compare_local_files_projective_bidirectional_region_grid_transform(
                    &rrrah_decode::DecodeRequest::new(left), &rrrah_decode::DecodeRequest::new(right), g.transform,
                    rrrah_dedup::local_scan::ProjectivePyramidRegionsFilePolicy {search:p.search,max_regions:32,max_total_sample_pairs:p.search.filter.filter.max_sample_pairs*33},
                    (4,4), &budget, ||false,
                ).unwrap();
                projective_regions = region_pixel_diagnostic(&regions);
                regions.region_support_count()
            } else {
                0
            };
            let mut similarity_rows = Vec::new();
            let mut similarity_error = "null".to_string();
            match rrrah_dedup::geometry::verify_similarity_candidates(
                &matches,
                GeometryPolicy {
                    max_points: local_features * levels,
                    ..p.search.local.geometry
                },
                || false,
            ) {
                Ok(candidates) => {
                    for g in candidates.into_iter().flatten() {
                        let t = g.transform;
                        let model = rrrah_dedup::geometry::ProjectiveTransform {
                            matrix: [
                                [t.a, -t.b, t.translation[0]],
                                [t.b, t.a, t.translation[1]],
                                [0., 0., 1.],
                            ],
                        };
                        let regions = rrrah_dedup::local_scan::compare_local_files_projective_bidirectional_region_grid_transform(
                        &rrrah_decode::DecodeRequest::new(left), &rrrah_decode::DecodeRequest::new(right), model,
                        rrrah_dedup::local_scan::ProjectivePyramidRegionsFilePolicy {search:p.search,max_regions:32,max_total_sample_pairs:p.search.filter.filter.max_sample_pairs*33},
                        (4,4), &budget, ||false,
                    ).unwrap();
                        similarity_rows.push(format!(
                            "{{\"matrix\":{:?},\"inliers\":{:?},\"region_support_count\":{},\"regions\":{}}}",
                            model.matrix,
                            g.inliers,
                            regions.region_support_count(),
                            region_pixel_diagnostic(&regions)
                        ));
                    }
                }
                Err(error) => similarity_error = format!("\"{error:?}\""),
            }
            distinct_evidence = format!(
                "{{\"correspondences\":{points:?},\"matrix\":{matrix},\"inliers\":{inliers},\"region_support_count\":{support},\"regions\":{projective_regions},\"similarity_error\":{similarity_error},\"similarity_candidates\":[{}]}}",
                similarity_rows.join(",")
            );
        }
        rows.push(format!("{{\"levels\":{levels},\"left_features\":{},\"right_features\":{},\"directions\":[{}],\"distinct\":{distinct_evidence}}}",af.len(),bf.len(),stats.join(",")));
    }
    drop(a);
    drop(b);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"ok\",\"stats\":[{}],\"managed_used\":0,\"managed_peak\":{}}}",
        rows.join(","),
        budget.peak()
    );
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

fn diagnose_original_managed_regions(
    left: &str,
    right: &str,
    collection: bool,
    blur7: bool,
    area: bool,
    grid8: bool,
    lowcontrast: bool,
) {
    let mut config = original_regions_config(blur7, grid8);
    if lowcontrast {
        config.gradient.search.local.extract.minimum_corner_score = 0.000001;
    }
    let factors: Vec<_> = (0..=6).map(|n| 2f64.powf(f64::from(n) / 2.)).collect();
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let result = if collection {
        let files = [
            (1, rrrah_decode::DecodeRequest::new(left)),
            (2, rrrah_decode::DecodeRequest::new(right)),
        ];
        let collection_policy =
            rrrah_dedup::local_collection::ProjectiveDistinctScaleRegionsCollectionPolicy {
                search: config,
                budgets: rrrah_dedup::local_index::FileFeatureBudgets {
                    max_files: 2,
                    max_features: 28000,
                    max_hits: 784000000,
                    max_pair_counts: 1,
                    max_pairs: 1,
                },
                max_retrieval_comparisons: 784000000,
            };
        let mut report = if area {
            rrrah_dedup::local_collection::scan_projective_local_collection_spatial_gradient_scale_regions_area_distinct_managed(
                files,collection_policy,&factors,128000000,&budget,||false,
            ).unwrap()
        } else {
            rrrah_dedup::local_collection::scan_projective_local_collection_spatial_gradient_scale_regions_distinct_managed(
                files,collection_policy,&factors,&budget,||false,
            ).unwrap()
        };
        assert!(
            report.file_issues.is_empty()
                && report.local.issues.is_empty()
                && report.local.source_issues.is_empty(),
            "{report:?}"
        );
        assert_eq!(
            report.local.pairs.len(),
            1,
            "retrieval must preserve the real crop"
        );
        let pair = report.local.pairs.pop().unwrap();
        assert_eq!((pair.left, pair.right), (1, 2));
        pair.evidence
    } else if area {
        rrrah_dedup::local_scan::compare_local_files_projective_spatial_gradient_scale_regions_area_distinct_managed(
            &rrrah_decode::DecodeRequest::new(left),&rrrah_decode::DecodeRequest::new(right),config,&factors,128000000,&budget,||false,
        ).unwrap()
    } else {
        rrrah_dedup::local_scan::compare_local_files_projective_spatial_gradient_scale_regions_distinct_managed(
            &rrrah_decode::DecodeRequest::new(left),&rrrah_decode::DecodeRequest::new(right),config,&factors,&budget,||false,
        ).unwrap()
    };
    let points: Vec<_> = result
        .whole
        .correspondences
        .iter()
        .map(|p| [p.source, p.target])
        .collect();
    let matrix = result
        .whole
        .geometry
        .as_ref()
        .map_or("null".to_string(), |g| format!("{:?}", g.transform.matrix));
    let inliers = result
        .whole
        .geometry
        .as_ref()
        .map_or("[]".to_string(), |g| format!("{:?}", &*g.inliers));
    let regions = result
        .regions
        .as_ref()
        .map_or("[]".to_string(), region_pixel_diagnostic);
    let support = result.regions.as_ref().map_or(0, |r| r.region_support_count());
    let whole = result.whole.candidate;
    let retained = budget.used();
    drop(result);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"ok\",\"whole_candidate\":{whole},\"correspondences\":{points:?},\"matrix\":{matrix},\"inliers\":{inliers},\"regions\":{regions},\"region_support_count\":{support},\"retained_before_drop\":{retained},\"managed_used\":0,\"managed_peak\":{}}}",
        budget.peak()
    );
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

fn diagnose_original_area_collection_six(folder: &str) {
    let names = [
        "200100.jpg",
        "200101.jpg",
        "200200.jpg",
        "200201.jpg",
        "200300.jpg",
        "200301.jpg",
    ];
    let files: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            (
                (i + 1) as u64,
                rrrah_decode::DecodeRequest::new(std::path::Path::new(folder).join(n)),
            )
        })
        .collect();
    let factors: Vec<_> = (0..=6).map(|n| 2f64.powf(f64::from(n) / 2.)).collect();
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let config = rrrah_dedup::local_collection::ProjectiveDistinctScaleRegionsCollectionPolicy {
        search: original_regions_config(true, true),
        budgets: rrrah_dedup::local_index::FileFeatureBudgets {
            max_files: 6,
            max_features: 84000,
            max_hits: 7056000000,
            max_pair_counts: 15,
            max_pairs: 15,
        },
        max_retrieval_comparisons: 7056000000,
    };
    let report=rrrah_dedup::local_collection::scan_projective_local_collection_spatial_gradient_scale_regions_area_distinct_managed(
        files,config,&factors,128000000,&budget,||false,
    ).unwrap();
    assert!(
        report.file_issues.is_empty()
            && report.local.issues.is_empty()
            && report.local.source_issues.is_empty(),
        "{report:?}"
    );
    let pairs:Vec<_>=report.local.pairs.iter().map(|pair| {
        let e=&pair.evidence;
        let points:Vec<_>=e.whole.correspondences.iter().map(|p|[p.source,p.target]).collect();
        let matrix=e.whole.geometry.as_ref().map_or("null".to_string(),|g|format!("{:?}",g.transform.matrix));
        let inliers=e.whole.geometry.as_ref().map_or("[]".to_string(),|g|format!("{:?}",&*g.inliers));
        let regions=e.regions.as_ref().map_or("[]".to_string(),region_pixel_diagnostic);
        let support=e.regions.as_ref().map_or(0,|r|r.region_support_count());
        format!("{{\"left\":{},\"right\":{},\"whole_candidate\":{},\"correspondences\":{points:?},\"matrix\":{matrix},\"inliers\":{inliers},\"regions\":{regions},\"region_support_count\":{support}}}",pair.left,pair.right,e.whole.candidate)
    }).collect();
    let indexed = report.indexed_features;
    let hits = report.descriptor_hits;
    let proposed = report.proposed_pairs;
    let verified = report.pixel_verification_pairs;
    let analysed = format!("{:?}", report.analysed);
    let insufficient = format!("{:?}", report.insufficient_features);
    let retained = budget.used();
    drop(report);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"ok\",\"analysed\":{analysed},\"insufficient_features\":{insufficient},\"indexed_features\":{indexed},\"descriptor_hits\":{hits},\"proposed_pairs\":{proposed},\"pixel_verification_pairs\":{verified},\"pairs\":[{}],\"retained_before_drop\":{retained},\"managed_used\":0,\"managed_peak\":{}}}",
        pairs.join(","),
        budget.peak()
    );
}

fn smooth_candidate_view(
    view: &rrrah_dedup::linear::LinearRgbaView<'_>,
    budget: &rrrah_core::MemoryBudget,
) -> rrrah_core::SharedBuffer<f32> {
    rrrah_dedup::gradient_scale::smooth_gradient_candidates_managed(view, 2, 64000000, budget, || false)
        .unwrap()
}

fn diagnose_smoothed_candidates(left: &str, right: &str) {
    let config = original_regions_config(true, true);
    let p = config.gradient;
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let a = rrrah_dedup::decode::decode_selected_frame_bounded(
        &rrrah_decode::DecodeRequest::new(left),
        p.search.local.decode,
        &budget,
        || false,
    )
    .unwrap();
    let b = rrrah_dedup::decode::decode_selected_frame_bounded(
        &rrrah_decode::DecodeRequest::new(right),
        p.search.local.decode,
        &budget,
        || false,
    )
    .unwrap();
    let av = a.view(|| false).unwrap();
    let bv = b.view(|| false).unwrap();
    let aa = smooth_candidate_view(&av, &budget);
    let bb = smooth_candidate_view(&bv, &budget);
    let (aw, ah) = av.dimensions();
    let (bw, bh) = bv.dimensions();
    let asmooth = rrrah_dedup::linear::LinearRgbaView::new(aw, ah, &aa, 6400000, || false).unwrap();
    let bsmooth = rrrah_dedup::linear::LinearRgbaView::new(bw, bh, &bb, 6400000, || false).unwrap();
    let factors: Vec<_> = (0..=6).map(|n| 2f64.powf(f64::from(n) / 2.)).collect();
    let pyramid = rrrah_dedup::pyramid::PyramidPolicy {
        local: p.search.local.extract,
        max_levels: 7,
        max_total_pixels: 12800000,
        max_total_features: 14000,
    };
    let extract = |v| {
        rrrah_dedup::gradient_scale::extract_spatial_gradient_scales_area_managed(
            v,
            pyramid,
            &factors,
            7168000,
            GradientCellRecipe::Interpolated,
            (4, 4, 128),
            128000000,
            &budget,
            || false,
        )
        .unwrap()
    };
    let af = extract(&asmooth);
    let bf = extract(&bsmooth);
    let matches =
        rrrah_dedup::gradient::match_gradients_distinct_locations(&af, &bf, p.matching, 2., || false)
            .unwrap();
    let geometry = rrrah_dedup::geometry::verify_projective_sampled_for_domains(
        &matches,
        p.search.local.geometry,
        p.search.sampling,
        av.dimensions(),
        bv.dimensions(),
        || false,
    )
    .unwrap();
    let matrix = geometry
        .as_ref()
        .map_or("null".to_string(), |g| format!("{:?}", g.transform.matrix));
    let inliers = geometry
        .as_ref()
        .map_or("[]".to_string(), |g| format!("{:?}", g.inliers));
    let mut regions = "[]".to_string();
    let mut support = 0;
    if let Some(g) = geometry {
        let evidence =
            rrrah_dedup::local_scan::compare_local_files_projective_bidirectional_region_grid_transform(
                &rrrah_decode::DecodeRequest::new(left),
                &rrrah_decode::DecodeRequest::new(right),
                g.transform,
                rrrah_dedup::local_scan::ProjectivePyramidRegionsFilePolicy {
                    search: p.search,
                    max_regions: 128,
                    max_total_sample_pairs: 130 * p.search.filter.filter.max_sample_pairs,
                },
                (8, 8),
                &budget,
                || false,
            )
            .unwrap();
        support = evidence.region_support_count();
        regions = region_pixel_diagnostic(&evidence);
    }
    let points: Vec<_> = matches.iter().map(|m| [m.source, m.target]).collect();
    let features = [af.len(), bf.len()];
    drop((af, bf, aa, bb, a, b));
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"ok\",\"candidate_smoothing_radius\":2,\"features\":{features:?},\"correspondences\":{points:?},\"matrix\":{matrix},\"inliers\":{inliers},\"regions\":{regions},\"region_support_count\":{support},\"managed_used\":0,\"managed_peak\":{}}}",
        budget.peak()
    );
}

fn diagnose_correspondence_union(points_file: &str, folder: &str) {
    let points = std::fs::read_to_string(points_file).unwrap();
    let mut matches = Vec::new();
    for line in points.lines() {
        let v: Vec<f64> = line.split_whitespace().map(|v| v.parse().unwrap()).collect();
        assert_eq!(v.len(), 4);
        assert!(v.iter().all(|v| v.is_finite()));
        matches.push(rrrah_dedup::geometry::Correspondence {
            source: [v[0], v[1]],
            target: [v[2], v[3]],
        });
    }
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let matches = rrrah_dedup::correspondence_union::union_correspondences_distinct_managed(
        &[&matches],
        2.,
        28000,
        392000000,
        &budget,
        || false,
    )
    .unwrap();
    let config = original_regions_config(true, true);
    let p = config.gradient;
    let geometry = rrrah_dedup::geometry::verify_projective_sampled_for_domains(
        &matches,
        p.search.local.geometry,
        p.search.sampling,
        (2048, 1536),
        (692, 349),
        || false,
    )
    .unwrap();
    let matrix = geometry
        .as_ref()
        .map_or("null".to_string(), |g| format!("{:?}", g.transform.matrix));
    let inliers = geometry
        .as_ref()
        .map_or("[]".to_string(), |g| format!("{:?}", g.inliers));
    let mut regions = "[]".to_string();
    let mut support = 0;
    if let Some(g) = geometry {
        let evidence =
            rrrah_dedup::local_scan::compare_local_files_projective_bidirectional_region_grid_transform(
                &rrrah_decode::DecodeRequest::new(std::path::Path::new(folder).join("200300.jpg")),
                &rrrah_decode::DecodeRequest::new(std::path::Path::new(folder).join("200301.jpg")),
                g.transform,
                rrrah_dedup::local_scan::ProjectivePyramidRegionsFilePolicy {
                    search: p.search,
                    max_regions: 128,
                    max_total_sample_pairs: 130 * p.search.filter.filter.max_sample_pairs,
                },
                (8, 8),
                &budget,
                || false,
            )
            .unwrap();
        support = evidence.region_support_count();
        regions = region_pixel_diagnostic(&evidence);
    }
    let points: Vec<_> = matches.iter().map(|m| [m.source, m.target]).collect();
    drop(matches);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"ok\",\"correspondences\":{points:?},\"matrix\":{matrix},\"inliers\":{inliers},\"regions\":{regions},\"region_support_count\":{support},\"managed_used\":0,\"managed_peak\":{}}}",
        budget.peak()
    );
}

#[cfg(test)]
mod candidate_smoothing_tests {
    #[test]
    fn separable_smoothing_matches_independent_full_footprint_and_releases_credit() {
        let pixels: Vec<f32> = (0..6)
            .flat_map(|i| {
                let value = i as f32 * 0.37 - 0.5;
                [value, value * 2., value + 1., 1.]
            })
            .collect();
        let view = rrrah_dedup::linear::LinearRgbaView::new(3, 2, &pixels, 6, || false).unwrap();
        let budget = rrrah_core::MemoryBudget::new(192);
        let output = super::smooth_candidate_view(&view, &budget);
        assert_eq!(output.len(), 24);
        for y in 0..2i64 {
            for x in 0..3i64 {
                for channel in 0..4 {
                    let mut expected = 0.;
                    for dy in -2..=2 {
                        for dx in -2..=2 {
                            let sx = (x + dx).clamp(0, 2) as usize;
                            let sy = (y + dy).clamp(0, 1) as usize;
                            expected += f64::from(pixels[(sy * 3 + sx) * 4 + channel]) / 25.;
                        }
                    }
                    let actual = f64::from(output[((y * 3 + x) * 4) as usize + channel]);
                    assert!(
                        (actual - expected).abs() < 0.000001,
                        "{x},{y},{channel}: {actual} != {expected}"
                    );
                }
            }
        }
        assert_eq!(budget.peak(), 192);
        assert_eq!(budget.used(), 96);
        let clone = output.clone();
        drop(output);
        assert_eq!(budget.used(), 96);
        drop(clone);
        assert_eq!(budget.used(), 0);
    }
}

fn diagnose_fresh_candidate_union(
    left: &str,
    right: &str,
    collection: bool,
    smoothing_radius: u32,
    fine_scales: bool,
    encoded: bool,
    domain_budget: bool,
    wider_filter: bool,
    asymmetric: bool,
    reverse_asymmetric: bool,
) {
    let mut config = rrrah_dedup::local_scan::ProjectiveCandidateUnionRegionsFilePolicy {
        search: original_regions_config(true, true),
        low_contrast_corner_score: 0.000001,
        smoothing_radius,
        max_smoothing_taps_per_image: if smoothing_radius == 2 {
            64000000
        } else {
            128000000
        },
        max_area_taps_per_extraction: 128000000,
        max_total_gradient_samples: 28672000,
        max_total_match_comparisons: 784000000,
        max_union_points: 28000,
        max_union_comparisons: 392000000,
    };
    if domain_budget {
        config.search.gradient.search.local.pixels.max_source_pixels = 12800000;
        config
            .search
            .gradient
            .search
            .photometric
            .residual
            .max_source_pixels = 12800000;
    }
    if encoded {
        config.search.gradient.search.filter.color_space = FilterColorSpace::EncodedSrgb;
    }
    if wider_filter {
        config.search.gradient.search.filter.filter.radius = 8;
    }
    let factors: Vec<_> = (0..=6)
        .map(|n| 2f64.powf(f64::from(n) / if fine_scales { 4. } else { 2. }))
        .collect();
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let e = if collection {
        let policy = rrrah_dedup::local_collection::ProjectiveCandidateUnionRegionsCollectionPolicy {
            search: config,
            budgets: rrrah_dedup::local_index::FileFeatureBudgets {
                max_files: 2,
                max_features: 56000,
                max_hits: 3136000000,
                max_pair_counts: 1,
                max_pairs: 1,
            },
            max_retrieval_comparisons: 3136000000,
        };
        let mut report =
            rrrah_dedup::local_collection::scan_projective_local_collection_candidate_union_regions_managed(
                [
                    (1, rrrah_decode::DecodeRequest::new(left)),
                    (2, rrrah_decode::DecodeRequest::new(right)),
                ],
                policy,
                &factors,
                &budget,
                || false,
            )
            .unwrap();
        assert!(
            report.file_issues.is_empty()
                && report.local.issues.is_empty()
                && report.local.source_issues.is_empty(),
            "{report:?}"
        );
        assert_eq!(report.local.pairs.len(), 1, "known recovery must be retrieved");
        let pair = report.local.pairs.pop().unwrap();
        assert_eq!((pair.left, pair.right), (1, 2));
        pair.evidence
    } else if asymmetric {
        rrrah_dedup::local_scan::compare_local_files_projective_candidate_union_regions_with_smoothing_managed(&rrrah_decode::DecodeRequest::new(left),&rrrah_decode::DecodeRequest::new(right),config,&factors,if reverse_asymmetric {[0,4]} else {[4,0]},&budget,||false).unwrap()
    } else {
        rrrah_dedup::local_scan::compare_local_files_projective_candidate_union_regions_managed(
            &rrrah_decode::DecodeRequest::new(left),
            &rrrah_decode::DecodeRequest::new(right),
            config,
            &factors,
            &budget,
            || false,
        )
        .unwrap()
    };
    let points: Vec<_> = e.correspondences.iter().map(|v| [v.source, v.target]).collect();
    let matrix = e
        .geometry
        .as_ref()
        .map_or("null".to_string(), |g| format!("{:?}", g.transform.matrix));
    let inliers = e
        .geometry
        .as_ref()
        .map_or("[]".to_string(), |g| format!("{:?}", &*g.inliers));
    let regions = e
        .regions
        .as_ref()
        .map_or("[]".to_string(), region_pixel_diagnostic);
    let support = e.regions.as_ref().map_or(0, |r| r.region_support_count());
    let retained = budget.used();
    drop(e);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"ok\",\"correspondences\":{points:?},\"matrix\":{matrix},\"inliers\":{inliers},\"regions\":{regions},\"region_support_count\":{support},\"retained_before_drop\":{retained},\"managed_used\":0,\"managed_peak\":{}}}",
        budget.peak()
    );
}

fn diagnose_original_union_collection_six(folder: &str) {
    let names = [
        "200100.jpg",
        "200101.jpg",
        "200200.jpg",
        "200201.jpg",
        "200300.jpg",
        "200301.jpg",
    ];
    let files: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            (
                (i + 1) as u64,
                rrrah_decode::DecodeRequest::new(std::path::Path::new(folder).join(n)),
            )
        })
        .collect();
    let factors: Vec<_> = (0..=6).map(|n| 2f64.powf(f64::from(n) / 2.)).collect();
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let search = rrrah_dedup::local_scan::ProjectiveCandidateUnionRegionsFilePolicy {
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
    let config = rrrah_dedup::local_collection::ProjectiveCandidateUnionRegionsCollectionPolicy {
        search,
        budgets: rrrah_dedup::local_index::FileFeatureBudgets {
            max_files: 6,
            max_features: 168000,
            max_hits: 28224000000,
            max_pair_counts: 15,
            max_pairs: 15,
        },
        max_retrieval_comparisons: 28224000000,
    };
    let report =
        rrrah_dedup::local_collection::scan_projective_local_collection_candidate_union_regions_managed(
            files,
            config,
            &factors,
            &budget,
            || false,
        )
        .unwrap();
    assert!(
        report.file_issues.is_empty()
            && report.local.issues.is_empty()
            && report.local.source_issues.is_empty(),
        "{report:?}"
    );
    let pairs:Vec<_>=report.local.pairs.iter().map(|pair| {
        let e=&pair.evidence;
        let points:Vec<_>=e.correspondences.iter().map(|p|[p.source,p.target]).collect();
        let matrix=e.geometry.as_ref().map_or("null".to_string(),|g|format!("{:?}",g.transform.matrix));
        let inliers=e.geometry.as_ref().map_or("[]".to_string(),|g|format!("{:?}",&*g.inliers));
        let regions=e.regions.as_ref().map_or("[]".to_string(),region_pixel_diagnostic);
        let support=e.regions.as_ref().map_or(0,|r|r.region_support_count());
        format!("{{\"left\":{},\"right\":{},\"correspondences\":{points:?},\"matrix\":{matrix},\"inliers\":{inliers},\"regions\":{regions},\"region_support_count\":{support}}}",pair.left,pair.right)
    }).collect();
    let indexed = report.indexed_features;
    let hits = report.descriptor_hits;
    let proposed = report.proposed_pairs;
    let verified = report.pixel_verification_pairs;
    let analysed = format!("{:?}", report.analysed);
    let insufficient = format!("{:?}", report.insufficient_features);
    let retained = budget.used();
    drop(report);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"ok\",\"analysed\":{analysed},\"insufficient_features\":{insufficient},\"indexed_features\":{indexed},\"descriptor_hits\":{hits},\"proposed_pairs\":{proposed},\"pixel_verification_pairs\":{verified},\"pairs\":[{}],\"retained_before_drop\":{retained},\"managed_used\":0,\"managed_peak\":{}}}",
        pairs.join(","),
        budget.peak()
    );
}

fn diagnose_candidate_union_fallback(left: &str, right: &str, collection: bool) {
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
    config.search.gradient.search.local.pixels.max_source_pixels = 12800000;
    config
        .search
        .gradient
        .search
        .photometric
        .residual
        .max_source_pixels = 12800000;
    let policy = rrrah_dedup::local_scan::ProjectiveCandidateUnionFallbackFilePolicy {
        search: config,
        asymmetric_radius: 4,
        max_total_gradient_samples: config.max_total_gradient_samples * 3,
        max_total_match_comparisons: config.max_total_match_comparisons * 3,
        max_total_union_comparisons: config.max_union_comparisons * 3,
    };
    let factors: Vec<_> = (0..=6).map(|n| 2f64.powf(f64::from(n) / 2.)).collect();
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let result = if collection {
        let policy = rrrah_dedup::local_collection::ProjectiveCandidateUnionFallbackCollectionPolicy {
            search: policy,
            budgets: rrrah_dedup::local_index::FileFeatureBudgets {
                max_files: 2,
                max_features: 112000,
                max_hits: 12544000000,
                max_pair_counts: 1,
                max_pairs: 1,
            },
            max_retrieval_comparisons: 12544000000,
        };
        let mut report=rrrah_dedup::local_collection::scan_projective_local_collection_candidate_union_fallback_regions_managed([(1,rrrah_decode::DecodeRequest::new(left)),(2,rrrah_decode::DecodeRequest::new(right))],policy,&factors,&budget,||false).unwrap();
        assert!(
            report.file_issues.is_empty()
                && report.local.issues.is_empty()
                && report.local.source_issues.is_empty(),
            "{report:?}"
        );
        assert_eq!(report.local.pairs.len(), 1, "known positive must be retrieved");
        let pair = report.local.pairs.pop().unwrap();
        assert_eq!((pair.left, pair.right), (1, 2));
        pair.evidence
    } else {
        rrrah_dedup::local_scan::compare_local_files_projective_candidate_union_fallback_regions_managed(
            &rrrah_decode::DecodeRequest::new(left),
            &rrrah_decode::DecodeRequest::new(right),
            policy,
            &factors,
            &budget,
            || false,
        )
        .unwrap()
    };
    let radii = result.smoothing_radii;
    let attempts = result.attempted_recipes;
    let e = result.evidence;
    let points: Vec<_> = e.correspondences.iter().map(|v| [v.source, v.target]).collect();
    let matrix = e
        .geometry
        .as_ref()
        .map_or("null".to_string(), |g| format!("{:?}", g.transform.matrix));
    let inliers = e
        .geometry
        .as_ref()
        .map_or("[]".to_string(), |g| format!("{:?}", &*g.inliers));
    let regions = e
        .regions
        .as_ref()
        .map_or("[]".to_string(), region_pixel_diagnostic);
    let support = e.regions.as_ref().map_or(0, |r| r.region_support_count());
    let retained = budget.used();
    drop(e);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"ok\",\"smoothing_radii\":{radii:?},\"attempted_recipes\":{attempts},\"correspondences\":{points:?},\"matrix\":{matrix},\"inliers\":{inliers},\"regions\":{regions},\"region_support_count\":{support},\"retained_before_drop\":{retained},\"managed_used\":0,\"managed_peak\":{}}}",
        budget.peak()
    );
}

fn diagnose_screen_translation_grid(left: &str, right: &str, matrix_path: &str) {
    let coefficients: Vec<f64> = std::fs::read_to_string(matrix_path)
        .unwrap()
        .split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect();
    assert_eq!(coefficients.len(), 9);
    assert!(coefficients.iter().all(|v| v.is_finite()));
    let original: [[f64; 3]; 3] = std::array::from_fn(|r| std::array::from_fn(|c| coefficients[r * 3 + c]));
    let mut p = original_regions_config(true, true);
    p.gradient.search.local.pixels.max_source_pixels = 12800000;
    p.gradient.search.photometric.residual.max_source_pixels = 12800000;
    p.gradient.search.filter.filter.radius = 8;
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let mut rows = Vec::new();
    for dy in [-1., 0., 1.] {
        for dx in [-1., 0., 1.] {
            let mut matrix = original;
            for c in 0..3 {
                matrix[0][c] += dx * original[2][c];
                matrix[1][c] += dy * original[2][c];
            }
            let e =
                rrrah_dedup::local_scan::compare_local_files_projective_bidirectional_region_grid_transform(
                    &rrrah_decode::DecodeRequest::new(left),
                    &rrrah_decode::DecodeRequest::new(right),
                    rrrah_dedup::geometry::ProjectiveTransform { matrix },
                    rrrah_dedup::local_scan::ProjectivePyramidRegionsFilePolicy {
                        search: p.gradient.search,
                        max_regions: 128,
                        max_total_sample_pairs: 130 * p.gradient.search.filter.filter.max_sample_pairs,
                    },
                    (8, 8),
                    &budget,
                    || false,
                )
                .unwrap();
            rows.push(format!(
                "{{\"offset\":[{dx},{dy}],\"matrix\":{matrix:?},\"regions\":{},\"region_support_count\":{}}}",
                region_pixel_diagnostic(&e),
                e.region_support_count()
            ));
            drop(e);
            assert_eq!(budget.used(), 0);
        }
    }
    println!(
        "{{\"status\":\"ok\",\"experiments\":[{}],\"managed_used\":0,\"managed_peak\":{}}}",
        rows.join(","),
        budget.peak()
    );
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
                    max_regions: 32,
                    max_total_sample_pairs: 33 * p.gradient.search.filter.filter.max_sample_pairs,
                },
                (4, 4),
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
