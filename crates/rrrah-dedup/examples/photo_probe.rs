use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::AnimationBudget,
    decode::decode_selected_frame_bounded,
    geometry::{GeometryPolicy, Transform},
    local::{LocalPolicy, MatchPolicy},
    local_scan::{
        LocalFilePolicy, compare_local_files, compare_local_files_photometric,
        compare_local_files_with_color_filter, compare_local_files_with_constrained_filter,
    },
    warp::{
        ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricPolicy, WarpPolicy,
        verify_photometric_bidirectional, verify_pixels,
    },
};
use std::path::PathBuf;
fn policy() -> LocalFilePolicy {
    LocalFilePolicy {
        decode: AnimationBudget {
            max_frames: 10,
            max_pixels: 200_000,
            max_file_bytes: 1024 * 1024,
        },
        extract: LocalPolicy {
            max_pixels: 200_000,
            max_candidates: 200_000,
            max_features: 500,
            minimum_corner_score: 0.0001,
        },
        matching: MatchPolicy {
            max_comparisons: 250_000,
            max_distance: 64,
        },
        geometry: GeometryPolicy {
            tolerance: 2.,
            min_inliers: 10,
            max_points: 500,
            max_hypotheses: 125_000,
        },
        pixels: WarpPolicy {
            tolerance: 0.03,
            max_source_pixels: 200_000,
        },
        minimum_compared_pixels: 1000,
        minimum_coverage_fraction: 0.3,
        minimum_matched_fraction: 0.9,
    }
}
fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments
        .get(1)
        .is_some_and(|a| a == "--gradient-distance-kernel-bench")
    {
        gradient_distance_kernel_bench(&arguments[2..]);
        return;
    }
    if arguments.get(1).is_some_and(|a| {
        [
            "--spatial-gradient-file-pair",
            "--spatial-gradient-interpolated-file-pair",
            "--spatial-gradient-collection-pair",
            "--spatial-gradient-interpolated-collection-pair",
            "--spatial-gradient-regional-collection-pair",
            "--spatial-gradient-interpolated-regional-collection-pair",
        ]
        .contains(&a.as_str())
    }) {
        gradient_file_pair(
            &arguments[2..],
            arguments[1].contains("interpolated"),
            arguments[1].contains("collection"),
        );
        return;
    }
    if arguments.get(1).is_some_and(|a| {
        [
            "--five-bidirectional-regions-collection-pair",
            "--six-regions-collection-pair",
        ]
        .contains(&a.as_str())
    }) {
        let paths = arguments[2..]
            .iter()
            .filter(|a| !a.starts_with("--managed-memory-bytes="))
            .cloned()
            .collect::<Vec<_>>();
        complementary_file_pair(&paths, true, true, true, true, true, true);
        return;
    }
    if arguments
        .get(1)
        .is_some_and(|a| a == "--bidirectional-region-transform")
    {
        bidirectional_region_transform_pair(&arguments[2..]);
        return;
    }
    if arguments
        .get(1)
        .is_some_and(|a| a == "--five-all-regions-collection-pair")
    {
        complementary_file_pair(&arguments[2..], true, true, true, true, true, true);
        return;
    }
    if arguments
        .get(1)
        .is_some_and(|a| a == "--five-regions-collection-pair")
    {
        complementary_file_pair(&arguments[2..], true, true, true, true, true, false);
        return;
    }
    if arguments.get(1).is_some_and(|a| a == "--five-collection-files") {
        complementary_file_pair(&arguments[2..], true, true, true, true, false, false);
        return;
    }
    if arguments.get(1).is_some_and(|a| {
        [
            "--gradient-file-pair",
            "--gradient-interpolated-file-pair",
            "--gradient-collection-pair",
            "--gradient-interpolated-collection-pair",
        ]
        .contains(&a.as_str())
    }) {
        gradient_file_pair(
            &arguments[2..],
            arguments[1].contains("interpolated"),
            arguments[1].contains("collection"),
        );
        return;
    }
    if arguments
        .get(1)
        .is_some_and(|a| a == "--gradient-pair" || a == "--gradient-pyramid-pair")
    {
        gradient_pair(&arguments[2..], arguments[1] == "--gradient-pyramid-pair");
        return;
    }
    if arguments.get(1).is_some_and(|a| {
        a == "--complementary-file-pair"
            || a == "--complementary-filter-portfolio-pair"
            || a == "--complementary-filter-collection-pair"
            || a == "--complementary-gradient-pair"
            || a == "--complementary-gradient-portfolio-pair"
            || a == "--five-collection-pair"
    }) {
        complementary_file_pair(
            &arguments[2..],
            arguments[1] != "--complementary-file-pair",
            arguments[1] == "--complementary-filter-collection-pair"
                || arguments[1] == "--five-collection-pair",
            arguments[1] == "--complementary-gradient-pair"
                || arguments[1] == "--complementary-gradient-portfolio-pair"
                || arguments[1] == "--five-collection-pair",
            arguments[1] == "--complementary-gradient-portfolio-pair"
                || arguments[1] == "--five-collection-pair",
            false,
            false,
        );
        return;
    }
    if arguments.get(1).is_some_and(|a| {
        a == "--pyramid-file-pair"
            || a == "--pyramid-collection-pair"
            || a == "--pyramid-blur-pair"
            || a == "--pyramid-encoded-blur-pair"
            || a == "--pyramid-region-grid-pair"
            || a == "--pyramid-region-grid-collection-pair"
            || a == "--pyramid-filter-portfolio-pair"
    }) {
        pyramid_file_pair(
            &arguments[2..],
            arguments[1] == "--pyramid-collection-pair"
                || arguments[1] == "--pyramid-region-grid-collection-pair",
            arguments[1] == "--pyramid-blur-pair"
                || arguments[1] == "--pyramid-encoded-blur-pair"
                || arguments[1] == "--pyramid-region-grid-pair"
                || arguments[1] == "--pyramid-region-grid-collection-pair",
            arguments[1] == "--pyramid-filter-portfolio-pair",
            arguments[1] == "--pyramid-encoded-blur-pair",
            arguments[1] == "--pyramid-region-grid-pair"
                || arguments[1] == "--pyramid-region-grid-collection-pair",
        );
        return;
    }
    if arguments.get(1).is_some_and(|a| {
        a == "--projective-pair"
            || a == "--projective-pair-spatial"
            || a == "--projective-pair-sampled"
            || a == "--projective-pair-anchored"
            || a == "--projective-pair-portfolio"
            || a == "--projective-pair-spatial-portfolio"
    }) {
        projective_pair(
            &arguments[2..],
            arguments[1] == "--projective-pair-spatial"
                || arguments[1] == "--projective-pair-spatial-portfolio",
            arguments[1] == "--projective-pair-sampled" || arguments[1] == "--projective-pair-anchored",
            arguments[1] == "--projective-pair-anchored",
            arguments[1] == "--projective-pair-portfolio"
                || arguments[1] == "--projective-pair-spatial-portfolio",
        );
        return;
    }
    if arguments
        .get(1)
        .is_some_and(|a| a == "--projective-correspondences")
    {
        projective_correspondences(&arguments[2..]);
        return;
    }
    if arguments.get(1).is_some_and(|a| a == "--pyramid-correspondences") {
        pyramid_correspondences(&arguments[2..]);
        return;
    }
    if arguments
        .get(1)
        .is_some_and(|a| a == "--registration-photo" || a == "--registration-photo-anchored")
    {
        registration_photo_probe(&arguments[2..], arguments[1] == "--registration-photo-anchored");
        return;
    }
    if arguments.get(1).is_some_and(|a| a == "--projective-phases") {
        projective_phases(&arguments[2..]);
        return;
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    if std::env::args().any(|a| a == "--jpeg-decoder-oracle") {
        jpeg_decoder_oracle(&root, &budget);
        return;
    }
    let oracle = std::env::args().any(|a| a == "--geometry-oracle");
    let encoded = std::env::args().any(|a| a == "--encoded-srgb");
    let constrained = std::env::args().any(|a| a == "--constrained-fit");
    let filtered = constrained || encoded || std::env::args().any(|a| a == "--filtered");
    let fitted = std::env::args().any(|a| a == "--photometric");
    let compare = |a: &DecodeRequest, b: &DecodeRequest| {
        if filtered {
            (if constrained {
                compare_local_files_with_constrained_filter
            } else {
                compare_local_files_with_color_filter
            })(
                a,
                b,
                policy(),
                photometric_policy(),
                selected_filter(encoded),
                &budget,
                || false,
            )
        } else if fitted {
            compare_local_files_photometric(a, b, policy(), photometric_policy(), &budget, || false)
        } else {
            compare_local_files(a, b, policy(), &budget, || false)
        }
    };
    if std::env::args().any(|a| a == "--partial-copies") {
        collage_probe(&root, &budget, compare);
        return;
    }
    for id in [830, 898, 1084, 1294] {
        for variant in ["resize", "crop", "rotate", "brightness", "jpeg"] {
            let ext = if variant == "jpeg" { "jpg" } else { "png" };
            let result = compare(
                &DecodeRequest::new(root.join(format!("{id}-base.png"))),
                &DecodeRequest::new(root.join(format!("{id}-{variant}.{ext}"))),
            );
            if oracle && matches!(variant, "rotate" | "jpeg") {
                photo_geometry_oracle(
                    &root,
                    &budget,
                    id,
                    variant,
                    result.as_ref().ok().and_then(|e| e.geometry.as_ref()),
                );
            }
            match result {
                Ok(e) => println!(
                    "{id} {variant}: candidate={} matches={} inliers={} pixels={:?} photometric={:?} filtered={:?}",
                    e.candidate,
                    e.correspondences.len(),
                    e.geometry.as_ref().map_or(0, |g| g.inliers.len()),
                    e.pixels,
                    e.photometric,
                    e.filtered
                ),
                Err(e) => println!("{id} {variant}: ERROR {e:?}"),
            }
            assert_eq!(budget.used(), 0);
        }
    }
    for (a, b) in [
        (830, 898),
        (830, 1084),
        (830, 1294),
        (898, 1084),
        (898, 1294),
        (1084, 1294),
    ] {
        let e = compare(
            &DecodeRequest::new(root.join(format!("{a}-base.png"))),
            &DecodeRequest::new(root.join(format!("{b}-base.png"))),
        )
        .unwrap();
        println!(
            "negative {a} {b}: candidate={} matches={} inliers={}",
            e.candidate,
            e.correspondences.len(),
            e.geometry.as_ref().map_or(0, |g| g.inliers.len())
        );
        assert!(!e.candidate);
        assert_eq!(budget.used(), 0);
    }
}

fn photometric_policy() -> PhotometricPolicy {
    let relaxed = std::env::args().any(|a| a == "--relaxed-fit");
    PhotometricPolicy {
        residual: policy().pixels,
        minimum_samples: 1000,
        minimum_variance: 1e-5,
        minimum_gain: if relaxed { 0.01 } else { 0.2 },
        maximum_gain: if relaxed { 100. } else { 5. },
        maximum_offset: if relaxed { 1. } else { 0.1 },
    }
}

// Pixel-center coordinates corresponding to the independently authored Pillow
// 17-degree expanded rotation; JPEG recompression has identity geometry.
fn authored_geometry(source: (u32, u32), target: (u32, u32), variant: &str) -> Transform {
    if variant == "jpeg" {
        return Transform {
            a: 1.,
            b: 0.,
            translation: [0.; 2],
        };
    }
    let angle = 17_f64.to_radians();
    let a = angle.cos();
    let b = -angle.sin();
    let sx = (f64::from(source.0) - 1.) / 2.;
    let sy = (f64::from(source.1) - 1.) / 2.;
    Transform {
        a,
        b,
        translation: [
            (f64::from(target.0) - 1.) / 2. - a * sx + b * sy,
            (f64::from(target.1) - 1.) / 2. - b * sx - a * sy,
        ],
    }
}

fn jpeg_decoder_oracle(root: &std::path::Path, budget: &MemoryBudget) {
    for id in [830, 898, 1084, 1294] {
        let a = decode_selected_frame_bounded(
            &DecodeRequest::new(root.join(format!("{id}-jpeg.jpg"))),
            policy().decode,
            budget,
            || false,
        )
        .unwrap();
        let b = decode_selected_frame_bounded(
            &DecodeRequest::new(root.join(format!("../photos-jpeg-oracle/{id}.png"))),
            policy().decode,
            budget,
            || false,
        )
        .unwrap();
        let evidence = verify_pixels(
            &a.view(|| false).unwrap(),
            &b.view(|| false).unwrap(),
            Transform {
                a: 1.,
                b: 0.,
                translation: [0.; 2],
            },
            policy().pixels,
            || false,
        )
        .unwrap();
        println!("jpeg-decoder-oracle {id}: {evidence:?}");
    }
    assert_eq!(budget.used(), 0);
}

fn selected_filter(encoded: bool) -> ColorFilterPolicy {
    let radius = std::env::args()
        .skip_while(|s| s != "--radius")
        .nth(1)
        .map_or(2, |s| s.parse().expect("--radius requires an integer"));
    ColorFilterPolicy {
        filter: FilterPolicy {
            radius,
            max_sample_pairs: 20_000_000,
        },
        color_space: if encoded {
            FilterColorSpace::EncodedSrgb
        } else {
            FilterColorSpace::LinearSrgb
        },
    }
}

fn collage_probe(
    root: &std::path::Path,
    budget: &MemoryBudget,
    compare: impl Fn(
        &DecodeRequest,
        &DecodeRequest,
    ) -> Result<
        rrrah_dedup::local_scan::LocalFileEvidence,
        rrrah_dedup::local_scan::LocalFileError,
    >,
) {
    for left in [830, 898, 1084, 1294] {
        for right in [830, 898, 1084, 1294] {
            if left == right {
                continue;
            }
            for percent in [40, 70] {
                let a = DecodeRequest::new(root.join(format!("{left}-base.png")));
                let b =
                    DecodeRequest::new(root.join(format!("../photo-collages/{left}-{right}-{percent}.png")));
                match compare(&a, &b) {
                    Ok(e) => println!(
                        "collage {left} {right} {percent}: candidate={} matches={} inliers={} auxiliary_failure={:?} filtered={:?}",
                        e.candidate,
                        e.correspondences.len(),
                        e.geometry.as_ref().map_or(0, |g| g.inliers.len()),
                        e.photometric_failure,
                        e.filtered
                    ),
                    Err(e) => println!("collage {left} {right} {percent}: INCONCLUSIVE {e:?}"),
                }
                assert_eq!(budget.used(), 0);
            }
        }
    }
}

fn photo_geometry_oracle(
    root: &std::path::Path,
    budget: &MemoryBudget,
    id: u32,
    variant: &str,
    fit: Option<&rrrah_dedup::geometry::GeometryEvidence>,
) {
    let ext = if variant == "jpeg" { "jpg" } else { "png" };
    let a = decode_selected_frame_bounded(
        &DecodeRequest::new(root.join(format!("{id}-base.png"))),
        policy().decode,
        budget,
        || false,
    )
    .unwrap();
    let b = decode_selected_frame_bounded(
        &DecodeRequest::new(root.join(format!("{id}-{variant}.{ext}"))),
        policy().decode,
        budget,
        || false,
    )
    .unwrap();
    let av = a.view(|| false).unwrap();
    let bv = b.view(|| false).unwrap();
    let truth = authored_geometry(av.dimensions(), bv.dimensions(), variant);
    let residual = verify_photometric_bidirectional(&av, &bv, truth, photometric_policy(), || false).unwrap();
    println!("oracle {id} {variant}: geometry={truth:?} feature_geometry={fit:?} residual={residual:?}");
}

fn projective_pair(args: &[String], spatial: bool, sampled: bool, anchored: bool, portfolio: bool) {
    use rrrah_dedup::{
        geometry::ProjectiveTransform,
        local_scan::compare_local_files_projective_registered,
        warp::{ProjectiveRegistrationPolicy, verify_projective_filtered},
    };
    assert_eq!(
        args.len(),
        11,
        "left right and nine published homography coefficients required"
    );
    let values: Vec<f64> = args[2..].iter().map(|v| v.parse().unwrap()).collect();
    let truth = ProjectiveTransform {
        matrix: [
            [values[0], values[1], values[2]],
            [values[3], values[4], values[5]],
            [values[6], values[7], values[8]],
        ],
    };
    let left = DecodeRequest::new(&args[0]);
    let right = DecodeRequest::new(&args[1]);
    let mut p = policy();
    p.extract.max_features = 32;
    p.geometry.max_points = 32;
    p.geometry.max_hypotheses = 40_000;
    p.matching.max_comparisons = 4096;
    p.pixels.max_source_pixels = 400_000;
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
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    if sampled || portfolio {
        p.extract.max_features = 500;
        p.geometry.max_points = 500;
        p.geometry.max_hypotheses = 2048;
        p.matching.max_comparisons = 250_000;
    }
    // Two lanes each visit both bounded 320x320 inputs: 2 * 2 * 320 * 320.
    if portfolio {
        p.pixels.max_source_pixels = 409_600;
    }
    let evidence_json = |e: &rrrah_dedup::warp::BidirectionalEvidence| {
        format!(
            "[{:?},{:?}]",
            [
                e.forward.matched_pixels,
                e.forward.compared_pixels,
                e.forward.source_pixels
            ],
            [
                e.reverse.matched_pixels,
                e.reverse.compared_pixels,
                e.reverse.source_pixels
            ]
        )
    };
    let mut portfolio_json = "null".to_string();
    let result = if portfolio {
        let mut fit = photometric_policy();
        fit.minimum_samples = 16;
        let trust = rrrah_dedup::warp::ProjectiveRegistrationTrustPolicy {
            registration,
            photometric: fit,
            maximum_corner_shift: 1.,
        };
        rrrah_dedup::local_scan::compare_local_files_projective_portfolio(&left,&right,
            rrrah_dedup::local_scan::ProjectivePortfolioFilePolicy{local:p,filter,
                registration:rrrah_dedup::warp::ProjectiveRegistrationPortfolioPolicy{anchored:trust,unanchored:registration,max_sample_pairs:128_000_000},
                spatial:spatial.then_some(rrrah_dedup::local_scan::SpatialFeaturePolicy{columns:4,rows:4,max_per_cell:32}),sampling:Some(rrrah_dedup::geometry::ProjectiveSamplingPolicy{trials:2048,seed:0x1234abcd})},&budget,||false).map(|e| {
            let chosen=usize::from(!e.accepted_lanes[0] && e.accepted_lanes[1]);
            let (registered_transform,pixels,filtered)=e.pixels.as_ref().map_or((None,None,None),|p| {
                portfolio_json=format!("{{\"accepted_lanes\":{:?},\"chosen_lane\":{},\"anchored\":{{\"registered\":{:?},\"strict_counts\":{},\"filtered_counts\":{}}},\"unanchored\":{{\"registered\":{:?},\"strict_counts\":{},\"filtered_counts\":{}}}}}",e.accepted_lanes,chosen,p.models.anchored.matrix,evidence_json(&p.anchored.strict),evidence_json(&p.anchored.filtered),p.models.unanchored.matrix,evidence_json(&p.unanchored.strict),evidence_json(&p.unanchored.filtered));
                let (model,residual)=if chosen==0 {(p.models.anchored,&p.anchored)}else{(p.models.unanchored,&p.unanchored)};
                (Some(model),Some(residual.strict.clone()),Some(residual.clone()))
            });
            rrrah_dedup::local_scan::ProjectiveFileEvidence{registered_transform,filtered,correspondences:e.correspondences,geometry:e.geometry,pixels,candidate:e.candidate}
        })
    } else if anchored {
        let mut fit = photometric_policy();
        fit.minimum_samples = 16;
        rrrah_dedup::local_scan::compare_local_files_projective_anchored(
            &left,
            &right,
            rrrah_dedup::local_scan::ProjectiveAnchoredFilePolicy {
                local: p,
                filter,
                registration: rrrah_dedup::warp::ProjectiveRegistrationTrustPolicy {
                    registration,
                    photometric: fit,
                    maximum_corner_shift: 1.,
                },
                spatial: None,
                sampling: Some(rrrah_dedup::geometry::ProjectiveSamplingPolicy {
                    trials: 2048,
                    seed: 0x1234abcd,
                }),
            },
            &budget,
            || false,
        )
    } else if sampled {
        rrrah_dedup::local_scan::compare_local_files_projective_sampled(
            &left,
            &right,
            rrrah_dedup::local_scan::ProjectiveSampledFilePolicy {
                local: p,
                filter,
                registration,
                spatial: None,
                sampling: rrrah_dedup::geometry::ProjectiveSamplingPolicy {
                    trials: 2048,
                    seed: 0x1234abcd,
                },
            },
            &budget,
            || false,
        )
    } else if spatial {
        rrrah_dedup::local_scan::compare_local_files_projective_registered_spatial(
            &left,
            &right,
            p,
            filter,
            registration,
            rrrah_dedup::local_scan::SpatialFeaturePolicy {
                columns: 4,
                rows: 4,
                max_per_cell: 2,
            },
            &budget,
            || false,
        )
    } else {
        compare_local_files_projective_registered(&left, &right, p, filter, registration, &budget, || false)
    };

    let (status, candidate, matches, inliers, geometry, registered, strict, filtered) = match result {
        Ok(e) => (
            "ok",
            e.candidate,
            e.correspondences.len(),
            e.geometry.as_ref().map_or(0, |g| g.inliers.len()),
            e.geometry
                .as_ref()
                .map_or_else(|| "null".into(), |g| format!("{:?}", g.transform.matrix)),
            e.registered_transform
                .map_or_else(|| "null".into(), |h| format!("{:?}", h.matrix)),
            e.pixels.as_ref().map_or_else(|| "null".into(), evidence_json),
            e.filtered
                .as_ref()
                .map_or_else(|| "null".into(), |f| evidence_json(&f.filtered)),
        ),
        Err(error) => {
            eprintln!("projective-pair error: {error:?}");
            (
                "error",
                false,
                0,
                0,
                "null".into(),
                "null".into(),
                "null".into(),
                "null".into(),
            )
        }
    };
    assert_eq!(budget.used(), 0);
    let a = decode_selected_frame_bounded(&left, p.decode, &budget, || false).unwrap();
    let b = decode_selected_frame_bounded(&right, p.decode, &budget, || false).unwrap();
    let oracle = verify_projective_filtered(
        &a.view(|| false).unwrap(),
        &b.view(|| false).unwrap(),
        truth,
        p.pixels,
        filter,
        || false,
    );
    let oracle = match oracle {
        Ok(e) => evidence_json(&e.filtered),
        Err(error) => {
            eprintln!("published geometry error: {error:?}");
            "null".into()
        }
    };
    drop(a);
    drop(b);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"{status}\",\"candidate\":{candidate},\"correspondences\":{matches},\"inliers\":{inliers},\"geometry\":{geometry},\"registered\":{registered},\"strict_counts\":{strict},\"filtered_counts\":{filtered},\"oracle_filtered_counts\":{oracle},\"portfolio\":{portfolio_json},\"managed_used\":{},\"managed_peak\":{}}}",
        budget.used(),
        budget.peak()
    );
}

fn projective_correspondences(args: &[String]) {
    use rrrah_dedup::{
        geometry::{ProjectiveSamplingPolicy, ProjectiveTransform, verify_projective_sampled},
        local::{extract_multiscale_oriented, extract_spatial_oriented, match_features},
    };
    assert_eq!(args.len(), 11);
    let values: Vec<f64> = args[2..].iter().map(|v| v.parse().unwrap()).collect();
    let truth = ProjectiveTransform {
        matrix: [
            [values[0], values[1], values[2]],
            [values[3], values[4], values[5]],
            [values[6], values[7], values[8]],
        ],
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let p = policy();
    let left =
        decode_selected_frame_bounded(&DecodeRequest::new(&args[0]), p.decode, &budget, || false).unwrap();
    let right =
        decode_selected_frame_bounded(&DecodeRequest::new(&args[1]), p.decode, &budget, || false).unwrap();
    let a = left.view(|| false).unwrap();
    let b = right.view(|| false).unwrap();
    let mut rows = Vec::new();
    for limit in [32usize, 128, 500] {
        for spatial in [false, true] {
            let mut extract = p.extract;
            extract.max_features = limit;
            let features = |view: &rrrah_dedup::linear::LinearRgbaView<'_>| {
                if spatial {
                    extract_spatial_oriented(view, extract, 4, 4, limit.div_ceil(16), || false)
                } else {
                    extract_multiscale_oriented(view, extract, || false)
                }
            };
            let af = features(&a).unwrap();
            let bf = features(&b).unwrap();
            let matches = match_features(
                &af,
                &bf,
                MatchPolicy {
                    max_comparisons: 250_000,
                    max_distance: 64,
                },
                || false,
            )
            .unwrap();
            let correct = matches
                .iter()
                .filter(|pair| {
                    truth.apply(pair.source).is_some_and(|target| {
                        (target[0] - pair.target[0]).hypot(target[1] - pair.target[1]) <= 2.0
                    })
                })
                .count();
            let mut geometry_policy = p.geometry;
            geometry_policy.max_hypotheses = 2048;
            let native = verify_projective_sampled(
                &matches,
                geometry_policy,
                ProjectiveSamplingPolicy {
                    trials: 2048,
                    seed: 0x1234abcd,
                },
                || false,
            )
            .unwrap();
            let model = native
                .as_ref()
                .map_or_else(|| "null".into(), |g| format!("{:?}", g.transform.matrix));
            let inliers = native.as_ref().map_or(0, |g| g.inliers.len());
            rows.push(format!("{{\"limit\":{limit},\"spatial\":{spatial},\"left_features\":{},\"right_features\":{},\"correspondences\":{},\"published_geometry_inliers\":{correct},\"native_geometry\":{model},\"native_inliers\":{inliers}}}",af.len(),bf.len(),matches.len()));
        }
    }
    drop(left);
    drop(right);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"ok\",\"diagnostics\":[{}],\"scope\":\"Descriptor correspondence diagnostics at two normalized pixels; public extraction allocations are outside decode managed accounting\"}}",
        rows.join(",")
    );
}

fn pyramid_file_pair(
    args: &[String],
    collection: bool,
    blur: bool,
    portfolio: bool,
    encoded: bool,
    region_grid: bool,
) {
    use rrrah_dedup::{
        geometry::ProjectiveSamplingPolicy,
        local_scan::{
            ProjectivePyramidPhotometricFilePolicy, compare_local_files_projective_pyramid_photometric,
        },
        warp::PhotometricFitMode,
    };
    assert_eq!(args.len(), 2, "left and right selected files required");
    let mut local = policy();
    local.matching.max_comparisons = 2_250_000;
    local.geometry.max_points = 1500;
    local.geometry.max_hypotheses = 2048;
    local.pixels.max_source_pixels = 409_600;
    let mut photometric = photometric_policy();
    photometric.residual = local.pixels;
    photometric.minimum_samples = 16;
    let p = ProjectivePyramidPhotometricFilePolicy {
        local,
        max_levels: 3,
        max_total_pixels: 400_000,
        max_total_features: 1500,
        sampling: ProjectiveSamplingPolicy {
            trials: 2048,
            seed: 0x1234abcd,
        },
        photometric,
        filter: ColorFilterPolicy {
            filter: FilterPolicy {
                radius: if blur { 3 } else { 1 },
                max_sample_pairs: if blur { 32_000_000 } else { 12_000_000 },
            },
            color_space: if encoded {
                FilterColorSpace::EncodedSrgb
            } else {
                FilterColorSpace::LinearSrgb
            },
        },
        fit_mode: PhotometricFitMode::ConstrainedLeastSquares,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    if region_grid {
        pyramid_region_grid_pair(args, p, &budget, collection);
        return;
    }
    let mut extra = String::new();
    let result = if portfolio {
        use rrrah_dedup::local_scan::{
            ProjectivePyramidFilterPortfolioPolicy, compare_local_files_projective_pyramid_filter_portfolio,
        };
        compare_local_files_projective_pyramid_filter_portfolio(
            &DecodeRequest::new(&args[0]), &DecodeRequest::new(&args[1]),
            ProjectivePyramidFilterPortfolioPolicy {
                primary: p,
                secondary: ColorFilterPolicy {filter: FilterPolicy {radius: 3, max_sample_pairs: 32_000_000}, color_space: FilterColorSpace::LinearSrgb},
                max_total_sample_pairs: 44_000_000,
            }, &budget, || false,
        ).map(|e| {
            let secondary_counts = e.secondary_pixels.as_ref().map_or_else(|| "null".into(), |p| format!("[{:?},{:?}]",
                [p.fitted.forward.pixels.matched_pixels, p.fitted.forward.pixels.compared_pixels, p.fitted.forward.pixels.source_pixels],
                [p.fitted.reverse.pixels.matched_pixels, p.fitted.reverse.pixels.compared_pixels, p.fitted.reverse.pixels.source_pixels]));
            extra = format!(",\"accepted_filters\":{:?},\"secondary_counts\":{},\"secondary_fit_failure\":{:?},\"primary_candidate\":{}",
                e.accepted_filters, secondary_counts, format!("{:?}", e.secondary_fit_failure), e.primary.candidate);
            let mut primary = e.primary;
            primary.candidate = e.candidate;
            primary
        })
    } else if collection {
        use rrrah_dedup::{
            local_collection::{ProjectivePyramidCollectionPolicy, scan_projective_local_collection_pyramid},
            local_index::FileFeatureBudgets,
        };
        // Retrieval includes same-file hits: at most 3000 queries * 3000 IDs.
        let config = ProjectivePyramidCollectionPolicy {
            search: p,
            budgets: FileFeatureBudgets {
                max_files: 2,
                max_features: 3000,
                max_hits: 9_000_000,
                max_pair_counts: 1,
                max_pairs: 1,
            },
        };
        match scan_projective_local_collection_pyramid(
            [
                (1, DecodeRequest::new(&args[0])),
                (2, DecodeRequest::new(&args[1])),
            ],
            config,
            &budget,
            || false,
        ) {
            Ok(report) => {
                if !report.file_issues.is_empty()
                    || !report.local.issues.is_empty()
                    || !report.local.source_issues.is_empty()
                {
                    println!(
                        "{{\"status\":\"error\",\"error\":{:?},\"managed_used\":{},\"managed_peak\":{}}}",
                        format!(
                            "file={:?}; pair={:?}; source={:?}",
                            report.file_issues, report.local.issues, report.local.source_issues
                        ),
                        budget.used(),
                        budget.peak()
                    );
                    return;
                }
                if let Some(pair) = report.local.pairs.into_iter().next() {
                    Ok(pair.evidence)
                } else {
                    println!(
                        "{{\"status\":\"ok\",\"candidate\":false,\"geometry\":null,\"fitted_counts\":null,\"retrieved_pairs\":0,\"managed_used\":{},\"managed_peak\":{}}}",
                        budget.used(),
                        budget.peak()
                    );
                    return;
                }
            }
            Err(error) => {
                println!(
                    "{{\"status\":\"error\",\"error\":{:?},\"managed_used\":{},\"managed_peak\":{}}}",
                    error.to_string(),
                    budget.used(),
                    budget.peak()
                );
                return;
            }
        }
    } else {
        compare_local_files_projective_pyramid_photometric(
            &DecodeRequest::new(&args[0]),
            &DecodeRequest::new(&args[1]),
            p,
            &budget,
            || false,
        )
    };
    match result {
        Ok(e) => {
            let counts = e.pixels.as_ref().map_or_else(
                || "null".into(),
                |p| {
                    format!(
                        "[{:?},{:?}]",
                        [
                            p.fitted.forward.pixels.matched_pixels,
                            p.fitted.forward.pixels.compared_pixels,
                            p.fitted.forward.pixels.source_pixels
                        ],
                        [
                            p.fitted.reverse.pixels.matched_pixels,
                            p.fitted.reverse.pixels.compared_pixels,
                            p.fitted.reverse.pixels.source_pixels
                        ]
                    )
                },
            );
            let fit_parameters = e.pixels.as_ref().map_or_else(|| "null".into(), |p| {
                format!("[{{\"gain\":{:?},\"offset\":{:?},\"constrained\":{:?},\"samples\":{}}},{{\"gain\":{:?},\"offset\":{:?},\"constrained\":{:?},\"samples\":{}}}]",
                    p.fitted.forward.gain,p.fitted.forward.offset,p.fitted.forward.constrained_channels,p.fitted.forward.fitted_samples,
                    p.fitted.reverse.gain,p.fitted.reverse.offset,p.fitted.reverse.constrained_channels,p.fitted.reverse.fitted_samples)
            });
            extra += &format!(",\"fit_parameters\":{}", fit_parameters);
            let geometry = e
                .geometry
                .as_ref()
                .map_or_else(|| "null".into(), |g| format!("{:?}", g.transform.matrix));
            println!(
                "{{\"status\":\"ok\",\"candidate\":{},\"correspondences\":{},\"inliers\":{},\"geometry\":{},\"fitted_counts\":{},\"fit_failure\":{:?},\"managed_used\":{},\"managed_peak\":{}{} }}",
                e.candidate,
                e.correspondences.len(),
                e.geometry.as_ref().map_or(0, |g| g.inliers.len()),
                geometry,
                counts,
                format!("{:?}", e.fit_failure),
                budget.used(),
                budget.peak(),
                extra
            );
        }
        Err(error) => println!(
            "{{\"status\":\"error\",\"error\":{:?},\"managed_used\":{},\"managed_peak\":{}}}",
            error.to_string(),
            budget.used(),
            budget.peak()
        ),
    }
}

fn gradient_file_pair(args: &[String], interpolated: bool, collection: bool) {
    use rrrah_dedup::local_scan::{
        ProjectiveGradientPyramidFilePolicy, ProjectivePyramidPhotometricFilePolicy,
        compare_local_files_projective_gradient_pyramid_with_recipe,
    };
    assert_eq!(args.len(), 2);
    let mut local = policy();
    local.geometry.max_points = 1500;
    local.geometry.max_hypotheses = 4096;
    let mut photometric = photometric_policy();
    photometric.residual.max_source_pixels = 409600;
    let search = ProjectivePyramidPhotometricFilePolicy {
        local,
        max_levels: 3,
        max_total_pixels: 400000,
        max_total_features: 1500,
        sampling: rrrah_dedup::geometry::ProjectiveSamplingPolicy {
            trials: 4096,
            seed: 0x1234abcd,
        },
        photometric,
        filter: ColorFilterPolicy {
            filter: FilterPolicy {
                radius: 3,
                max_sample_pairs: 32000000,
            },
            color_space: FilterColorSpace::LinearSrgb,
        },
        fit_mode: rrrah_dedup::warp::PhotometricFitMode::ConstrainedLeastSquares,
    };
    let p = ProjectiveGradientPyramidFilePolicy {
        search,
        matching: rrrah_dedup::gradient::GradientMatchPolicy {
            max_comparisons: 2250000,
            max_squared_distance: 0.5,
            squared_ratio: 0.64,
        },
        max_total_gradient_samples: 768000,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let recipe = if interpolated {
        rrrah_dedup::gradient::GradientCellRecipe::Interpolated
    } else {
        rrrah_dedup::gradient::GradientCellRecipe::Fixed
    };
    let mut metadata = String::new();
    let spatial_requested = std::env::args().any(|v| v.starts_with("--spatial-gradient-"));
    let e = if spatial_requested && std::env::args().any(|v| v.contains("regional-collection")) {
        use rrrah_dedup::{
            local_collection::{
                ProjectiveGradientCollectionPolicy, ProjectiveSpatialGradientRegionCollectionPolicy,
                scan_projective_local_collection_spatial_gradient_regions,
            },
            local_index::FileFeatureBudgets,
        };
        let config = ProjectiveSpatialGradientRegionCollectionPolicy {
            gradient: ProjectiveGradientCollectionPolicy {
                search: p,
                recipe,
                budgets: FileFeatureBudgets {
                    max_files: 2,
                    max_features: 3000,
                    max_hits: 9_000_000,
                    max_pair_counts: 1,
                    max_pairs: 1,
                },
                max_retrieval_comparisons: 9_000_000,
            },
            spatial: rrrah_dedup::local_scan::SpatialFeaturePolicy {
                columns: 4,
                rows: 4,
                max_per_cell: 32,
            },
            grid: (4, 4),
            max_regions: 32,
            max_total_sample_pairs: 34 * p.search.filter.filter.max_sample_pairs,
        };
        let report = scan_projective_local_collection_spatial_gradient_regions(
            [
                (1, DecodeRequest::new(&args[0])),
                (2, DecodeRequest::new(&args[1])),
            ],
            config,
            &budget,
            || false,
        )
        .unwrap();
        assert_eq!(report.analysed, vec![1, 2]);
        assert!(
            report.file_issues.is_empty()
                && report.local.issues.is_empty()
                && report.local.source_issues.is_empty()
        );
        let indexed = report.indexed_features;
        let hits = report.descriptor_hits;
        if report.local.pairs.is_empty() {
            drop(report);
            assert_eq!(budget.used(), 0);
            println!(
                "{{\"status\":\"ok\",\"retrieved\":false,\"candidate\":false,\"regions\":[],\"region_support_count\":0,\"managed_used_after_drop\":0,\"managed_peak\":{}}}",
                budget.peak()
            );
            return;
        }
        assert_eq!(report.local.pairs.len(), 1);
        let evidence = report.local.pairs.into_iter().next().unwrap().evidence;
        let (model, rows, support) = if let Some(regions) = &evidence.regions {
            (format!("{:?}",regions.transform.matrix),regions.regions.iter().map(|r|{
                let counts=r.pixels.as_ref().map_or_else(||"null".into(),|v|format!("[{:?},{:?}]",[v.fitted.forward.pixels.matched_pixels,v.fitted.forward.pixels.compared_pixels,v.fitted.forward.pixels.source_pixels],[v.fitted.reverse.pixels.matched_pixels,v.fitted.reverse.pixels.compared_pixels,v.fitted.reverse.pixels.source_pixels]));
                let [a,b]=r.domains;format!("{{\"domains\":[{:?},{:?}],\"accepted_region\":{},\"counts\":{},\"fit_failure\":{:?}}}",[a.x,a.y,a.width,a.height],[b.x,b.y,b.width,b.height],r.accepted_region,counts,format!("{:?}",r.fit_failure))
            }).collect::<Vec<_>>().join(","),regions.region_support_count())
        } else {
            ("null".into(), String::new(), 0)
        };
        metadata = format!(
            ",\"retrieved\":true,\"indexed_features\":{indexed},\"descriptor_hits\":{hits},\"spatial_grid\":[4,4,32],\"regional_geometry\":{model},\"regions\":[{rows}],\"region_support_count\":{support}"
        );
        evidence.whole
    } else if spatial_requested && !collection {
        metadata = ",\"spatial_grid\":[4,4,32]".into();
        rrrah_dedup::local_scan::compare_local_files_projective_spatial_gradient_pyramid_with_recipe(
            &DecodeRequest::new(&args[0]),
            &DecodeRequest::new(&args[1]),
            p,
            recipe,
            rrrah_dedup::local_scan::SpatialFeaturePolicy {
                columns: 4,
                rows: 4,
                max_per_cell: 32,
            },
            &budget,
            || false,
        )
        .unwrap()
    } else if collection {
        use rrrah_dedup::{
            local_collection::{
                ProjectiveGradientCollectionPolicy, scan_projective_local_collection_gradient,
            },
            local_index::FileFeatureBudgets,
        };
        let files = [
            (1, DecodeRequest::new(&args[0])),
            (2, DecodeRequest::new(&args[1])),
        ];
        let config = ProjectiveGradientCollectionPolicy {
            search: p,
            recipe,
            budgets: FileFeatureBudgets {
                max_files: 2,
                max_features: 3000,
                max_hits: 9_000_000,
                max_pair_counts: 1,
                max_pairs: 1,
            },
            max_retrieval_comparisons: 9_000_000,
        };
        let r = if spatial_requested {
            rrrah_dedup::local_collection::scan_projective_local_collection_spatial_gradient(
                files,
                config,
                rrrah_dedup::local_scan::SpatialFeaturePolicy {
                    columns: 4,
                    rows: 4,
                    max_per_cell: 32,
                },
                &budget,
                || false,
            )
        } else {
            scan_projective_local_collection_gradient(files, config, &budget, || false)
        }
        .unwrap();
        assert_eq!(r.analysed, vec![1, 2]);
        assert!(r.file_issues.is_empty() && r.local.issues.is_empty() && r.local.source_issues.is_empty());
        metadata = format!(
            ",\"retrieved\":{},\"indexed_features\":{},\"descriptor_hits\":{}",
            !r.local.pairs.is_empty(),
            r.indexed_features,
            r.descriptor_hits
        );
        if spatial_requested {
            metadata += ",\"spatial_grid\":[4,4,32]";
        }
        if r.local.pairs.is_empty() {
            drop(r);
            assert_eq!(budget.used(), 0);
            println!(
                "{{\"status\":\"ok\",\"candidate\":false,\"correspondences\":0,\"inliers\":0,\"geometry\":null,\"pixels\":null,\"managed_used_after_drop\":0,\"managed_peak\":{}{metadata}}}",
                budget.peak()
            );
            return;
        }
        assert_eq!(r.local.pairs.len(), 1);
        r.local.pairs.into_iter().next().unwrap().evidence
    } else {
        compare_local_files_projective_gradient_pyramid_with_recipe(
            &DecodeRequest::new(&args[0]),
            &DecodeRequest::new(&args[1]),
            p,
            recipe,
            &budget,
            || false,
        )
        .unwrap()
    };
    let pixels = if let Some(v) = &e.pixels {
        let f = v.fitted.forward.pixels;
        let r = v.fitted.reverse.pixels;
        format!(
            "{{\"status\":\"ok\",\"counts\":[{:?},{:?}]}}",
            [f.matched_pixels, f.compared_pixels, f.source_pixels],
            [r.matched_pixels, r.compared_pixels, r.source_pixels]
        )
    } else if let Some(reason) = e.fit_failure {
        format!("{{\"status\":\"error\",\"error\":\"Fit({reason:?})\"}}")
    } else {
        "null".into()
    };
    let geometry = e
        .geometry
        .as_ref()
        .map_or_else(|| "null".into(), |m| format!("{:?}", m.transform.matrix));
    let inliers = e.geometry.as_ref().map_or(0, |m| m.inliers.len());
    let candidate = e.candidate;
    let correspondences = e.correspondences.len();
    drop(e);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"ok\",\"correspondences\":{correspondences},\"inliers\":{inliers},\"geometry\":{geometry},\"pixels\":{pixels},\"candidate\":{candidate},\"managed_used_after_drop\":{},\"managed_peak\":{}{metadata},\"scope\":\"Public managed gradient file API with source/dependency/cancellation guards; fixed three-level geometry and radius3 confirmation.\"}}",
        budget.used(),
        budget.peak()
    );
}

fn gradient_pair(args: &[String], pyramid: bool) {
    use rrrah_dedup::{
        geometry::{ProjectiveSamplingPolicy, verify_projective_sampled},
        gradient::{GradientMatchPolicy, extract_gradients, match_gradients},
        warp::{PhotometricFitMode, verify_projective_photometric_filtered},
    };
    assert_eq!(args.len(), 2);
    let p = policy();
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let left =
        decode_selected_frame_bounded(&DecodeRequest::new(&args[0]), p.decode, &budget, || false).unwrap();
    let right =
        decode_selected_frame_bounded(&DecodeRequest::new(&args[1]), p.decode, &budget, || false).unwrap();
    let a = left.view(|| false).unwrap();
    let b = right.view(|| false).unwrap();
    let levels = if pyramid { 3 } else { 1 };
    let extract = |view: &rrrah_dedup::linear::LinearRgbaView<'_>| {
        if pyramid {
            rrrah_dedup::gradient::extract_gradient_pyramid(
                view,
                rrrah_dedup::pyramid::PyramidPolicy {
                    local: p.extract,
                    max_levels: levels,
                    max_total_pixels: 400000,
                    max_total_features: 1500,
                },
                768000,
                || false,
            )
        } else {
            extract_gradients(view, p.extract, 256000, || false)
        }
    };
    let af = extract(&a).unwrap();
    let bf = extract(&b).unwrap();
    let matches = match_gradients(
        &af,
        &bf,
        GradientMatchPolicy {
            max_comparisons: if pyramid { 2250000 } else { 250000 },
            max_squared_distance: 0.5,
            squared_ratio: 0.64,
        },
        || false,
    )
    .unwrap();
    let mut gp = p.geometry;
    gp.max_hypotheses = 4096;
    gp.max_points = if pyramid { 1500 } else { 500 };
    let model = verify_projective_sampled(
        &matches,
        gp,
        ProjectiveSamplingPolicy {
            trials: 4096,
            seed: 0x1234abcd,
        },
        || false,
    )
    .unwrap();
    let mut fit = photometric_policy();
    fit.residual.max_source_pixels = 409600;
    let filter = ColorFilterPolicy {
        filter: FilterPolicy {
            radius: 3,
            max_sample_pairs: 32000000,
        },
        color_space: FilterColorSpace::LinearSrgb,
    };
    let mut candidate = false;
    let pixels = if let Some(m) = &model {
        match verify_projective_photometric_filtered(
            &a,
            &b,
            m.transform,
            fit,
            filter,
            PhotometricFitMode::ConstrainedLeastSquares,
            || false,
        ) {
            Ok(e) => {
                let f = e.fitted.forward.pixels;
                let r = e.fitted.reverse.pixels;
                let pass = |v: rrrah_dedup::warp::WarpEvidence| {
                    v.compared_pixels >= 1000
                        && v.compared_pixels as f64 >= v.source_pixels as f64 * 0.3
                        && v.matched_pixels as f64 >= v.compared_pixels as f64 * 0.9
                };
                candidate = pass(f) && pass(r);
                format!(
                    "{{\"status\":\"ok\",\"counts\":[{:?},{:?}]}}",
                    [f.matched_pixels, f.compared_pixels, f.source_pixels],
                    [r.matched_pixels, r.compared_pixels, r.source_pixels]
                )
            }
            Err(error) => format!("{{\"status\":\"error\",\"error\":\"{error:?}\"}}"),
        }
    } else {
        "null".into()
    };
    let geometry = model
        .as_ref()
        .map_or_else(|| "null".into(), |m| format!("{:?}", m.transform.matrix));
    let inliers = model.as_ref().map_or(0, |m| m.inliers.len());
    drop(left);
    drop(right);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"ok\",\"left_features\":{},\"right_features\":{},\"correspondences\":{},\"inliers\":{inliers},\"geometry\":{geometry},\"pixels\":{pixels},\"candidate\":{candidate},\"managed_used_after_drop\":{},\"scope\":\"Gradient diagnostic with native geometry and fixed radius3 pixel confirmation; feature allocations unmanaged, no file lifecycle qualification.\"}}",
        af.len(),
        bf.len(),
        matches.len(),
        budget.used()
    );
}

fn pyramid_match_admission(
    left: &[rrrah_dedup::local::Feature],
    right: &[rrrah_dedup::local::Feature],
) -> String {
    // Diagnostic only: replay the fixed four-variant distance independently.
    // No relaxed matches enter model selection or pixel confirmation.
    let mut forward = vec![(u32::MAX, u32::MAX, usize::MAX); left.len()];
    let mut reverse = vec![(u32::MAX, u32::MAX, usize::MAX); right.len()];
    let update = |best: &mut (u32, u32, usize), distance, index| {
        if distance < best.0 {
            *best = (distance, best.0, index);
        } else {
            best.1 = best.1.min(distance);
        }
    };
    for (i, a) in left.iter().enumerate() {
        for (j, b) in right.iter().enumerate() {
            let mut distance = 256;
            for av in std::iter::once(&a.descriptor).chain(&a.quarter_turns) {
                for bv in std::iter::once(&b.descriptor).chain(&b.quarter_turns) {
                    distance = distance.min(av.iter().zip(bv).map(|(a, b)| (a ^ b).count_ones()).sum());
                }
            }
            update(&mut forward[i], distance, j);
            update(&mut reverse[j], distance, i);
        }
    }
    let mut admitted = [0usize; 4];
    for (i, &(distance, second, j)) in forward.iter().enumerate() {
        if distance > 64 {
            continue;
        }
        admitted[0] += 1;
        if u64::from(distance) * 4 >= u64::from(second) * 3 {
            continue;
        }
        admitted[1] += 1;
        let (back, next, index) = reverse[j];
        if index != i {
            continue;
        }
        admitted[2] += 1;
        if u64::from(back) * 4 < u64::from(next) * 3 {
            admitted[3] += 1;
        }
    }
    format!(
        "{{\"within_distance\":{},\"forward_ratio\":{},\"mutual\":{},\"both_ratios\":{}}}",
        admitted[0], admitted[1], admitted[2], admitted[3]
    )
}

fn pyramid_correspondences(args: &[String]) {
    use rrrah_dedup::{
        geometry::{ProjectiveSamplingPolicy, verify_projective_sampled},
        pyramid::{PyramidPolicy, extract_oriented_pyramid},
    };
    assert!(
        args.len() >= 2
            && args.len() <= 5
            && args[2..]
                .iter()
                .all(|v| v == "--registration" || v == "--low-contrast" || v == "--spatial")
    );
    let registration_diagnostic = args[2..].iter().any(|v| v == "--registration");
    let spatial_diagnostic = args[2..].iter().any(|v| v == "--spatial");
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut p = policy();
    if args[2..].iter().any(|v| v == "--low-contrast") {
        p.extract.minimum_corner_score = 0.;
    }
    let left =
        decode_selected_frame_bounded(&DecodeRequest::new(&args[0]), p.decode, &budget, || false).unwrap();
    let right =
        decode_selected_frame_bounded(&DecodeRequest::new(&args[1]), p.decode, &budget, || false).unwrap();
    let a = left.view(|| false).unwrap();
    let b = right.view(|| false).unwrap();
    let mut rows = Vec::new();
    for levels in [1usize, 2, 3] {
        let pyramid = PyramidPolicy {
            local: p.extract,
            max_levels: levels,
            max_total_pixels: 400_000,
            max_total_features: levels * 500,
        };
        let extract = |view: &rrrah_dedup::linear::LinearRgbaView<'_>| {
            if spatial_diagnostic {
                rrrah_dedup::pyramid::extract_spatial_oriented_pyramid(view, pyramid, 4, 4, 32, || false)
            } else {
                extract_oriented_pyramid(view, pyramid, || false)
            }
        };
        let af = extract(&a).unwrap();
        let bf = extract(&b).unwrap();
        let matches = rrrah_dedup::local::match_features(
            &af,
            &bf,
            MatchPolicy {
                max_comparisons: 2_250_000,
                max_distance: 64,
            },
            || false,
        )
        .unwrap();
        let mut geometry = p.geometry;
        geometry.max_points = 1500;
        geometry.max_hypotheses = 2048;
        let model = verify_projective_sampled(
            &matches,
            geometry,
            ProjectiveSamplingPolicy {
                trials: 2048,
                seed: 0x1234abcd,
            },
            || false,
        )
        .unwrap();
        let mut pixels = p.pixels;
        pixels.max_source_pixels = 409_600;
        let filter = ColorFilterPolicy {
            filter: FilterPolicy {
                radius: 1,
                max_sample_pairs: 4_000_000,
            },
            color_space: FilterColorSpace::LinearSrgb,
        };
        let counts = |e: &rrrah_dedup::warp::BidirectionalEvidence| {
            format!(
                "[{:?},{:?}]",
                [
                    e.forward.matched_pixels,
                    e.forward.compared_pixels,
                    e.forward.source_pixels
                ],
                [
                    e.reverse.matched_pixels,
                    e.reverse.compared_pixels,
                    e.reverse.source_pixels
                ]
            )
        };
        let accepted = |e: &rrrah_dedup::warp::WarpEvidence| {
            e.compared_pixels >= 1000
                && e.compared_pixels as f64 >= e.source_pixels as f64 * 0.3
                && e.matched_pixels as f64 >= e.compared_pixels as f64 * 0.9
        };
        let photometric = if let Some(m) = &model {
            use rrrah_dedup::warp::{PhotometricFitMode, verify_projective_photometric_filtered};
            let mut fit = photometric_policy();
            fit.residual = pixels;
            fit.minimum_samples = 16;
            // Three complete window passes need three times the single-pass cap.
            let mut fit_filter = filter;
            fit_filter.filter.max_sample_pairs = 12_000_000;
            let mut modes = Vec::new();
            for mode in [
                PhotometricFitMode::RejectOutsidePolicy,
                PhotometricFitMode::ConstrainedLeastSquares,
            ] {
                let outcome = match verify_projective_photometric_filtered(
                    &a,
                    &b,
                    m.transform,
                    fit,
                    fit_filter,
                    mode,
                    || false,
                ) {
                    Ok(e) => format!(
                        "{{\"mode\":\"{mode:?}\",\"status\":\"ok\",\"fitted_counts\":[{:?},{:?}],\"candidate\":{}}}",
                        [
                            e.fitted.forward.pixels.matched_pixels,
                            e.fitted.forward.pixels.compared_pixels,
                            e.fitted.forward.pixels.source_pixels
                        ],
                        [
                            e.fitted.reverse.pixels.matched_pixels,
                            e.fitted.reverse.pixels.compared_pixels,
                            e.fitted.reverse.pixels.source_pixels
                        ],
                        accepted(&e.fitted.forward.pixels) && accepted(&e.fitted.reverse.pixels)
                    ),
                    Err(error) => {
                        format!("{{\"mode\":\"{mode:?}\",\"status\":\"error\",\"error\":\"{error:?}\"}}")
                    }
                };
                modes.push(outcome);
            }
            format!("[{}]", modes.join(","))
        } else {
            "null".into()
        };
        let refined = if registration_diagnostic {
            if let Some(m) = &model {
                use rrrah_dedup::warp::{
                    ProjectiveRegistrationPolicy, ProjectiveRegistrationPortfolioPolicy,
                    ProjectiveRegistrationTrustPolicy, refine_projective_pixels_candidates,
                    verify_projective_candidates_filtered,
                };
                let registration = ProjectiveRegistrationPolicy {
                    radius: 1,
                    stride: 8,
                    rounds: 128,
                    max_sample_pairs: 64_000_000,
                };
                let mut fit = photometric_policy();
                fit.minimum_samples = 16;
                let portfolio = ProjectiveRegistrationPortfolioPolicy {
                    anchored: ProjectiveRegistrationTrustPolicy {
                        registration,
                        photometric: fit,
                        maximum_corner_shift: 1.,
                    },
                    unanchored: registration,
                    max_sample_pairs: 128_000_000,
                };
                match refine_projective_pixels_candidates(&a, &b, m.transform, portfolio, || false).and_then(
                    |models| verify_projective_candidates_filtered(&a, &b, models, pixels, filter, || false),
                ) {
                    Ok(e) => format!(
                        "{{\"status\":\"ok\",\"anchored\":{},\"unanchored\":{},\"accepted_lanes\":[{},{}]}}",
                        counts(&e.anchored.filtered),
                        counts(&e.unanchored.filtered),
                        accepted(&e.anchored.filtered.forward) && accepted(&e.anchored.filtered.reverse),
                        accepted(&e.unanchored.filtered.forward) && accepted(&e.unanchored.filtered.reverse)
                    ),
                    Err(error) => format!("{{\"status\":\"error\",\"error\":\"{error:?}\"}}"),
                }
            } else {
                "null".into()
            }
        } else {
            "null".into()
        };
        let verification = if let Some(m) = &model {
            match rrrah_dedup::warp::verify_projective_filtered(&a, &b, m.transform, pixels, filter, || false)
            {
                Ok(e) => format!(
                    "{{\"status\":\"ok\",\"strict_counts\":{},\"filtered_counts\":{},\"candidate\":{}}}",
                    counts(&e.strict),
                    counts(&e.filtered),
                    accepted(&e.filtered.forward) && accepted(&e.filtered.reverse)
                ),
                Err(error) => format!("{{\"status\":\"error\",\"error\":\"{error:?}\"}}"),
            }
        } else {
            "null".into()
        };
        let admission = pyramid_match_admission(&af, &bf);
        rows.push(format!("{{\"levels\":{levels},\"left_features\":{},\"right_features\":{},\"correspondences\":{},\"match_admission\":{admission},\"inliers\":{},\"geometry\":{},\"pixel_verification\":{verification},\"registration_verification\":{refined},\"photometric_verification\":{photometric}}}",af.len(),bf.len(),matches.len(),model.as_ref().map_or(0,|m|m.inliers.len()),model.map_or_else(||"null".into(),|m|format!("{:?}",m.transform.matrix))));
    }
    drop(left);
    drop(right);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"ok\",\"minimum_corner_score\":{},\"spatial_requested\":{spatial_diagnostic},\"registration_requested\":{registration_diagnostic},\"diagnostics\":[{}],\"scope\":\"Scale-pyramid verification with fixed pixel criteria and optional explicit registration; no known geometry or semantic accuracy qualification. Public feature allocations outside managed decode accounting.\"}}",
        p.extract.minimum_corner_score,
        rows.join(",")
    );
}

fn registration_photo_probe(args: &[String], anchored: bool) {
    use rrrah_dedup::{
        geometry::ProjectiveTransform,
        warp::{ProjectiveRegistrationPolicy, refine_projective_pixels_photometric},
    };
    assert_eq!(args.len(), 11);
    let values: Vec<f64> = args[2..].iter().map(|v| v.parse().unwrap()).collect();
    let initial = ProjectiveTransform {
        matrix: [
            [values[0], values[1], values[2]],
            [values[3], values[4], values[5]],
            [values[6], values[7], values[8]],
        ],
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let p = policy();
    let a =
        decode_selected_frame_bounded(&DecodeRequest::new(&args[0]), p.decode, &budget, || false).unwrap();
    let b =
        decode_selected_frame_bounded(&DecodeRequest::new(&args[1]), p.decode, &budget, || false).unwrap();
    let mut fit = photometric_policy();
    fit.minimum_samples = 16;
    let registration = ProjectiveRegistrationPolicy {
        radius: 1,
        stride: 8,
        rounds: 128,
        max_sample_pairs: 64_000_000,
    };
    let result = if anchored {
        rrrah_dedup::warp::refine_projective_pixels_anchored(
            &a.view(|| false).unwrap(),
            &b.view(|| false).unwrap(),
            initial,
            rrrah_dedup::warp::ProjectiveRegistrationTrustPolicy {
                registration,
                photometric: fit,
                maximum_corner_shift: 1.,
            },
            || false,
        )
    } else {
        refine_projective_pixels_photometric(
            &a.view(|| false).unwrap(),
            &b.view(|| false).unwrap(),
            initial,
            registration,
            fit,
            || false,
        )
    };
    drop(a);
    drop(b);
    assert_eq!(budget.used(), 0);
    match result {
        Ok(h) => println!(
            "{{\"status\":\"ok\",\"registered\":{:?},\"managed_used\":{},\"managed_peak\":{}}}",
            h.matrix,
            budget.used(),
            budget.peak()
        ),
        Err(error) => {
            eprintln!("{error:?}");
            println!("{{\"status\":\"error\"}}");
        }
    }
}

fn projective_phases(args: &[String]) {
    use rrrah_dedup::{
        geometry::{ProjectiveSamplingPolicy, verify_projective_sampled},
        local::{extract_multiscale_oriented, match_features},
        warp::{
            ProjectiveRegistrationPolicy, ProjectiveRegistrationTrustPolicy, refine_projective_pixels,
            refine_projective_pixels_anchored, refine_projective_pixels_photometric,
            verify_projective_filtered,
        },
    };
    assert_eq!(args.len(), 2);
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut p = policy();
    p.extract.max_features = 500;
    p.geometry.max_points = 500;
    p.geometry.max_hypotheses = 2048;
    p.matching.max_comparisons = 250_000;
    p.pixels.max_source_pixels = 400_000;
    let a =
        decode_selected_frame_bounded(&DecodeRequest::new(&args[0]), p.decode, &budget, || false).unwrap();
    let b =
        decode_selected_frame_bounded(&DecodeRequest::new(&args[1]), p.decode, &budget, || false).unwrap();
    let av = a.view(|| false).unwrap();
    let bv = b.view(|| false).unwrap();
    let af = extract_multiscale_oriented(&av, p.extract, || false).unwrap();
    let bf = extract_multiscale_oriented(&bv, p.extract, || false).unwrap();
    let matches = match_features(&af, &bf, p.matching, || false).unwrap();
    let geometry = verify_projective_sampled(
        &matches,
        p.geometry,
        ProjectiveSamplingPolicy {
            trials: 2048,
            seed: 0x1234abcd,
        },
        || false,
    )
    .unwrap();
    let Some(g) = geometry else {
        println!(
            "{{\"stage\":\"no_geometry\",\"correspondences\":{}}}",
            matches.len()
        );
        return;
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
    let mut fit = photometric_policy();
    fit.minimum_samples = 16;
    let original = verify_projective_filtered(&av, &bv, g.transform, p.pixels, filter, || false);
    let mut stages = Vec::new();
    stages.push(format!("\"initial_pixels\":{:?}", format!("{original:?}")));
    for mode in ["raw", "photometric", "anchored"] {
        let result = match mode {
            "raw" => refine_projective_pixels(&av, &bv, g.transform, registration, || false),
            "photometric" => {
                refine_projective_pixels_photometric(&av, &bv, g.transform, registration, fit, || false)
            }
            _ => refine_projective_pixels_anchored(
                &av,
                &bv,
                g.transform,
                ProjectiveRegistrationTrustPolicy {
                    registration,
                    photometric: fit,
                    maximum_corner_shift: 1.,
                },
                || false,
            ),
        };
        let phase = match result {
            Ok(h) => format!(
                "registered={:?}; pixels={:?}",
                h.matrix,
                verify_projective_filtered(&av, &bv, h, p.pixels, filter, || false)
            ),
            Err(error) => format!("registration_error={error:?}"),
        };
        stages.push(format!("\"{mode}\":{phase:?}"));
    }
    println!(
        "{{\"correspondences\":{},\"inliers\":{},\"geometry\":{:?},{}}}",
        matches.len(),
        g.inliers.len(),
        g.transform.matrix,
        stages.join(",")
    );
}

fn complementary_file_pair(
    args: &[String],
    portfolio: bool,
    collection: bool,
    four: bool,
    five: bool,
    regions: bool,
    all_regions: bool,
) {
    use rrrah_dedup::{
        geometry::ProjectiveSamplingPolicy,
        local_scan::{
            ProjectiveComplementaryFilePolicy, ProjectivePortfolioFilePolicy,
            ProjectivePyramidPhotometricFilePolicy, compare_local_files_projective_complementary,
        },
        warp::{
            PhotometricFitMode, ProjectiveRegistrationPolicy, ProjectiveRegistrationPortfolioPolicy,
            ProjectiveRegistrationTrustPolicy,
        },
    };
    assert!(args.len() >= 2);
    let mut local = policy();
    local.geometry.max_hypotheses = 2048;
    local.pixels.max_source_pixels = 409_600;
    let sampling = ProjectiveSamplingPolicy {
        trials: 2048,
        seed: 0x1234abcd,
    };
    let filter = ColorFilterPolicy {
        filter: FilterPolicy {
            radius: 1,
            max_sample_pairs: 4_000_000,
        },
        color_space: FilterColorSpace::LinearSrgb,
    };
    let mut photometric = photometric_policy();
    photometric.residual = local.pixels;
    photometric.minimum_samples = 16;
    let registration = ProjectiveRegistrationPolicy {
        radius: 1,
        stride: 8,
        rounds: 128,
        max_sample_pairs: 64_000_000,
    };
    let mut registration_photometric = photometric_policy();
    registration_photometric.minimum_samples = 16;
    let r = ProjectivePortfolioFilePolicy {
        local,
        filter,
        registration: ProjectiveRegistrationPortfolioPolicy {
            anchored: ProjectiveRegistrationTrustPolicy {
                registration,
                photometric: registration_photometric,
                maximum_corner_shift: 1.,
            },
            unanchored: registration,
            max_sample_pairs: 128_000_000,
        },
        spatial: None,
        sampling: Some(sampling),
    };
    let mut multiscale = local;
    multiscale.matching.max_comparisons = 2_250_000;
    multiscale.geometry.max_points = 1500;
    let p = ProjectivePyramidPhotometricFilePolicy {
        local: multiscale,
        max_levels: 3,
        max_total_pixels: 400_000,
        max_total_features: 1500,
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
        max_total_comparisons: 2_500_000,
        max_total_hypotheses: 4096,
        max_total_sample_pairs: 144_000_000,
    };
    let managed_limit = if std::env::args().any(|a| a == "--six-regions-collection-pair") {
        std::env::args()
            .find_map(|a| {
                a.strip_prefix("--managed-memory-bytes=")
                    .map(|v| v.parse::<u64>().expect("invalid managed memory limit"))
            })
            .unwrap_or(64 * 1024 * 1024)
    } else {
        64 * 1024 * 1024
    };
    let budget = MemoryBudget::new(managed_limit);
    let counts = |e: &rrrah_dedup::warp::BidirectionalEvidence| {
        format!(
            "[{:?},{:?}]",
            [
                e.forward.matched_pixels,
                e.forward.compared_pixels,
                e.forward.source_pixels
            ],
            [
                e.reverse.matched_pixels,
                e.reverse.compared_pixels,
                e.reverse.source_pixels
            ]
        )
    };
    let mut extra = String::new();
    let mut four_candidate = None;
    let result = if portfolio {
        use rrrah_dedup::local_scan::{
            ProjectiveComplementaryFilterPortfolioPolicy,
            compare_local_files_projective_complementary_filter_portfolio,
        };
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
        let result = if four {
            use rrrah_dedup::local_scan::{
                ProjectiveComplementaryGradientPolicy, ProjectiveGradientPyramidFilePolicy,
                compare_local_files_projective_complementary_gradient,
            };
            let mut gradient_search = p;
            gradient_search.local.geometry.max_hypotheses = 4096;
            gradient_search.sampling.trials = 4096;
            gradient_search.photometric.minimum_samples = 1000;
            gradient_search.filter = q.secondary;
            let gradient = ProjectiveGradientPyramidFilePolicy {
                search: gradient_search,
                matching: rrrah_dedup::gradient::GradientMatchPolicy {
                    max_comparisons: 2250000,
                    max_squared_distance: 0.5,
                    squared_ratio: 0.64,
                },
                max_total_gradient_samples: 768000,
            };
            let joined = ProjectiveComplementaryGradientPolicy {
                base: q,
                gradient,
                max_total_comparisons: 4750000,
                max_total_hypotheses: 8192,
                max_total_sample_pairs: 208000000,
                max_total_gradient_samples: 1536000,
            };
            let mut five_candidate = None;
            let combined = if five {
                use rrrah_dedup::local_scan::{
                    ProjectiveComplementaryGradientPortfolioPolicy,
                    compare_local_files_projective_complementary_gradient_portfolio,
                };
                let policy = ProjectiveComplementaryGradientPortfolioPolicy {
                    primary: joined,
                    interpolated: gradient,
                    max_total_comparisons: 7000000,
                    max_total_hypotheses: 12288,
                    max_total_sample_pairs: 240000000,
                    max_total_gradient_samples: 3072000,
                };
                if collection && args.len() > 2 {
                    use rrrah_dedup::{
                        local_collection::{
                            ProjectiveFiveSearchCollectionPolicy,
                            scan_projective_local_collection_five_searches,
                        },
                        local_index::FileFeatureBudgets,
                    };
                    let n = args.len();
                    let pair_cap = n.checked_mul(n - 1).unwrap() / 2;
                    let gradient_n = u64::try_from(n.checked_mul(1500).unwrap()).unwrap();
                    let work = gradient_n
                        .checked_mul(gradient_n)
                        .unwrap()
                        .checked_mul(2)
                        .unwrap();
                    if std::env::args().any(|a| a == "--six-regions-collection-pair") {
                        use rrrah_dedup::local_collection::{
                            ProjectiveSixRegionCollectionPolicy, ProjectiveSixSearchCollectionPolicy,
                            scan_projective_local_collection_six_regions,
                        };
                        let six_work = gradient_n
                            .checked_mul(gradient_n)
                            .unwrap()
                            .checked_mul(5)
                            .unwrap();
                        let six = ProjectiveSixSearchCollectionPolicy {
                            five: ProjectiveFiveSearchCollectionPolicy {
                                search: policy,
                                budgets: FileFeatureBudgets {
                                    max_files: n,
                                    max_features: n.checked_mul(6500).unwrap(),
                                    max_hits: usize::try_from(six_work.checked_mul(2).unwrap()).unwrap(),
                                    max_pair_counts: pair_cap,
                                    max_pairs: pair_cap,
                                },
                                max_gradient_retrieval_comparisons: six_work,
                            },
                            spatial: rrrah_dedup::local_scan::SpatialFeaturePolicy {
                                columns: 4,
                                rows: 4,
                                max_per_cell: 32,
                            },
                            max_total_comparisons: policy.max_total_comparisons
                                + gradient.matching.max_comparisons,
                            max_total_hypotheses: policy.max_total_hypotheses
                                + gradient.search.sampling.trials,
                            max_total_sample_pairs: policy.max_total_sample_pairs
                                + gradient.search.filter.filter.max_sample_pairs,
                        };
                        let joined = ProjectiveSixRegionCollectionPolicy {
                            six,
                            grid: (4, 4),
                            max_regions: 32,
                            max_total_comparisons: six.max_total_comparisons
                                + p.local.matching.max_comparisons,
                            max_total_hypotheses: six.max_total_hypotheses + p.sampling.trials,
                            max_total_sample_pairs: six.max_total_sample_pairs
                                + 17 * q.secondary.filter.max_sample_pairs
                                + 33 * policy.primary.gradient.search.filter.filter.max_sample_pairs
                                + 66 * policy.interpolated.search.filter.filter.max_sample_pairs,
                        };
                        let report = match scan_projective_local_collection_six_regions(
                            args.iter()
                                .enumerate()
                                .map(|(i, path)| (u64::try_from(i + 1).unwrap(), DecodeRequest::new(path))),
                            joined,
                            &budget,
                            || false,
                        ) {
                            Ok(report) => report,
                            Err(error) => {
                                println!(
                                    "{{\"status\":\"error\",\"error\":{:?},\"files\":{n},\"managed_limit\":{managed_limit},\"managed_used\":{},\"managed_peak\":{},\"max_gradient_retrieval_comparisons\":{six_work},\"max_features\":{},\"max_hits\":{}}}",
                                    format!("{error:?}"),
                                    budget.used(),
                                    budget.peak(),
                                    six.five.budgets.max_features,
                                    six.five.budgets.max_hits
                                );
                                std::process::exit(2);
                            }
                        };
                        assert_eq!(report.analysed.len(), n);
                        assert!(
                            report.file_issues.is_empty()
                                && report.local.issues.is_empty()
                                && report.local.source_issues.is_empty()
                        );
                        let edges=report.local.pairs.iter().map(|pair|{
                            let e=&pair.evidence;
                            let binary=e.binary_regions.whole.registered_transform.or_else(||e.binary_regions.whole.geometry.as_ref().map(|v|v.transform))
                                .map(|transform|rrrah_dedup::local_scan::ProjectiveTransformedRegionsFileEvidence{transform,regions:e.binary_regions.regions.clone()});
                            let regions=e.gradient_regions.iter().map(|v|six_regions_json(v.as_ref())).collect::<Vec<_>>().join(",");
                            format!("{{\"left\":{},\"right\":{},\"legacy_candidate\":{},\"legacy_searches\":{:?},\"spatial_gradient\":{},\"binary_regions\":{},\"gradient_regions\":[{regions}]}}",pair.left,pair.right,e.whole.base.candidate,e.whole.base.accepted_searches,six_whole_json(&e.whole.spatial),six_regions_json(binary.as_ref()))
                        }).collect::<Vec<_>>().join(",");
                        let indexed = report.indexed_features;
                        let hits = report.descriptor_hits;
                        let proposals = report.proposed_pairs;
                        drop(report);
                        assert_eq!(budget.used(), 0);
                        println!(
                            "{{\"status\":\"ok\",\"files\":{n},\"managed_limit\":{managed_limit},\"indexed_features\":{indexed},\"descriptor_hits\":{hits},\"proposed_pairs\":{proposals},\"max_gradient_retrieval_comparisons\":{six_work},\"edges\":[{edges}],\"managed_used\":0,\"managed_peak\":{}}}",
                            budget.peak()
                        );
                        return;
                    }
                    let report = scan_projective_local_collection_five_searches(
                        args.iter()
                            .enumerate()
                            .map(|(i, path)| (u64::try_from(i + 1).unwrap(), DecodeRequest::new(path))),
                        ProjectiveFiveSearchCollectionPolicy {
                            search: policy,
                            budgets: FileFeatureBudgets {
                                max_files: n,
                                max_features: n.checked_mul(5000).unwrap(),
                                max_hits: usize::try_from(work.checked_mul(2).unwrap()).unwrap(),
                                max_pair_counts: pair_cap,
                                max_pairs: pair_cap,
                            },
                            max_gradient_retrieval_comparisons: work,
                        },
                        &budget,
                        || false,
                    )
                    .unwrap();
                    assert_eq!(report.analysed.len(), n);
                    assert!(
                        report.file_issues.is_empty()
                            && report.local.issues.is_empty()
                            && report.local.source_issues.is_empty()
                    );
                    let edges = report
                        .local
                        .pairs
                        .iter()
                        .map(|pair| {
                            format!(
                                "[{}, {}, {}, {:?}]",
                                pair.left,
                                pair.right,
                                pair.evidence.candidate,
                                pair.evidence.accepted_searches
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    let indexed = report.indexed_features;
                    let hits = report.descriptor_hits;
                    let proposals = report.proposed_pairs;
                    drop(report);
                    assert_eq!(budget.used(), 0);
                    println!(
                        "{{\"status\":\"ok\",\"files\":{n},\"indexed_features\":{indexed},\"descriptor_hits\":{hits},\"proposed_pairs\":{proposals},\"edges\":[{edges}],\"managed_used\":0,\"managed_peak\":{}}}",
                        budget.peak()
                    );
                    return;
                }
                let measured = if regions {
                    use rrrah_dedup::{
                        local_collection::{
                            ProjectiveFiveRegionCollectionPolicy, ProjectiveFiveSearchCollectionPolicy,
                            scan_projective_local_collection_five_regions,
                        },
                        local_index::FileFeatureBudgets,
                    };
                    let regional_work = 17 * q.secondary.filter.max_sample_pairs;
                    let config = ProjectiveFiveRegionCollectionPolicy {
                        five: ProjectiveFiveSearchCollectionPolicy {
                            search: policy,
                            budgets: FileFeatureBudgets {
                                max_files: 2,
                                max_features: 10000,
                                max_hits: 34_000_000,
                                max_pair_counts: 1,
                                max_pairs: 1,
                            },
                            max_gradient_retrieval_comparisons: 18_000_000,
                        },
                        grid: (4, 4),
                        max_regions: 16,
                        max_total_comparisons: policy.max_total_comparisons
                            + p.local.matching.max_comparisons,
                        max_total_hypotheses: policy.max_total_hypotheses + p.sampling.trials,
                        max_total_sample_pairs: policy.max_total_sample_pairs + regional_work,
                    };
                    let (combined, indexed, hits) = if all_regions {
                        let mut all = config;
                        let sixth = std::env::args().any(|a| a == "--six-regions-collection-pair");
                        let bidirectional = sixth
                            || std::env::args().any(|a| a == "--five-bidirectional-regions-collection-pair");
                        let phases = if bidirectional {
                            all.max_regions = 32;
                            33
                        } else {
                            17
                        };
                        all.max_total_sample_pairs += phases
                            * policy.primary.gradient.search.filter.filter.max_sample_pairs
                            + phases * policy.interpolated.search.filter.filter.max_sample_pairs;
                        let files = [
                            (1, DecodeRequest::new(&args[0])),
                            (2, DecodeRequest::new(&args[1])),
                        ];
                        let report=if sixth {
                            use rrrah_dedup::local_collection::{ProjectiveSixSearchCollectionPolicy,ProjectiveSixRegionCollectionPolicy,LocalCollectionReport,ProjectiveFiveAllRegionsEvidence,ProjectiveFiveRegionEvidence};
                            let mut retrieval=all.five;retrieval.budgets.max_features=13000;retrieval.budgets.max_hits=45_000_000;retrieval.max_gradient_retrieval_comparisons=45_000_000;
                            let six=ProjectiveSixSearchCollectionPolicy{five:retrieval,spatial:rrrah_dedup::local_scan::SpatialFeaturePolicy{columns:4,rows:4,max_per_cell:32},
                                max_total_comparisons:policy.max_total_comparisons+gradient.matching.max_comparisons,
                                max_total_hypotheses:policy.max_total_hypotheses+gradient.search.sampling.trials,
                                max_total_sample_pairs:policy.max_total_sample_pairs+gradient.search.filter.filter.max_sample_pairs};
                            let joined=ProjectiveSixRegionCollectionPolicy{six,grid:all.grid,max_regions:all.max_regions,
                                max_total_comparisons:all.max_total_comparisons+gradient.matching.max_comparisons,
                                max_total_hypotheses:all.max_total_hypotheses+gradient.search.sampling.trials,
                                max_total_sample_pairs:all.max_total_sample_pairs+34*gradient.search.filter.filter.max_sample_pairs};
                            let r=rrrah_dedup::local_collection::scan_projective_local_collection_six_regions(files,joined,&budget,||false).unwrap();
                            let pairs=r.local.pairs.into_iter().map(|pair|{
                                let e=pair.evidence;let [fixed,interpolated,spatial_regions]=e.gradient_regions;
                                extra+=&format!(",\"spatial_gradient\":{},\"spatial_gradient_regions\":{},\"spatial_grid\":[4,4,32]",six_whole_json(&e.whole.spatial),six_regions_json(spatial_regions.as_ref()));
                                rrrah_dedup::local_scan::LocalPair{left:pair.left,right:pair.right,evidence:ProjectiveFiveAllRegionsEvidence{
                                    base:ProjectiveFiveRegionEvidence{whole:e.whole.base,regional:e.binary_regions},gradient_regions:[fixed,interpolated]}}
                            }).collect();
                            Ok(LocalCollectionReport{analysed:r.analysed,insufficient_features:r.insufficient_features,file_issues:r.file_issues,
                                local:rrrah_dedup::local_scan::LocalFileReport{pairs,issues:r.local.issues,source_issues:r.local.source_issues},
                                indexed_features:r.indexed_features,descriptor_hits:r.descriptor_hits,proposed_pairs:r.proposed_pairs,pixel_verification_pairs:r.pixel_verification_pairs})
                        } else if bidirectional {
                            rrrah_dedup::local_collection::scan_projective_local_collection_five_bidirectional_gradient_regions(files,all,&budget,||false)
                        } else {
                            rrrah_dedup::local_collection::scan_projective_local_collection_five_all_regions(files,all,&budget,||false)
                        }.unwrap();
                        assert_eq!(report.analysed, vec![1, 2]);
                        assert!(
                            report.file_issues.is_empty()
                                && report.local.issues.is_empty()
                                && report.local.source_issues.is_empty()
                        );
                        let indexed = report.indexed_features;
                        let hits = report.descriptor_hits;
                        assert!(report.local.pairs.len() <= 1);
                        let combined=report.local.pairs.into_iter().next().map(|pair|{
                            let e=pair.evidence;
                            let lanes=e.gradient_regions.iter().map(|lane|lane.as_ref().map_or_else(||"null".to_string(),|lane|{
                                let rows=lane.regions.iter().map(|r|{
                                    let counts=r.pixels.as_ref().map_or_else(||"null".into(),|e|format!("[{:?},{:?}]",[e.fitted.forward.pixels.matched_pixels,e.fitted.forward.pixels.compared_pixels,e.fitted.forward.pixels.source_pixels],[e.fitted.reverse.pixels.matched_pixels,e.fitted.reverse.pixels.compared_pixels,e.fitted.reverse.pixels.source_pixels]));
                                    let [a,b]=r.domains;format!("{{\"domains\":[{:?},{:?}],\"accepted_region\":{},\"counts\":{counts},\"fit_failure\":{:?}}}",[a.x,a.y,a.width,a.height],[b.x,b.y,b.width,b.height],r.accepted_region,format!("{:?}",r.fit_failure))
                                }).collect::<Vec<_>>().join(",");
                                format!("{{\"geometry\":{:?},\"regions\":[{rows}],\"region_support_count\":{}}}",lane.transform.matrix,lane.region_support_count())
                            })).collect::<Vec<_>>().join(",");
                            extra+=&format!(",\"gradient_regions\":[{lanes}]");e.base
                        });
                        (combined, indexed, hits)
                    } else {
                        let report = scan_projective_local_collection_five_regions(
                            [
                                (1, DecodeRequest::new(&args[0])),
                                (2, DecodeRequest::new(&args[1])),
                            ],
                            config,
                            &budget,
                            || false,
                        )
                        .unwrap();
                        assert_eq!(report.analysed, vec![1, 2]);
                        assert!(
                            report.file_issues.is_empty()
                                && report.local.issues.is_empty()
                                && report.local.source_issues.is_empty()
                        );
                        assert!(report.local.pairs.len() <= 1);
                        (
                            report.local.pairs.into_iter().next().map(|pair| pair.evidence),
                            report.indexed_features,
                            report.descriptor_hits,
                        )
                    };
                    extra += &format!(
                        ",\"retrieved\":{},\"indexed_features\":{indexed},\"descriptor_hits\":{hits}",
                        combined.is_some()
                    );
                    let Some(combined) = combined else {
                        assert_eq!(budget.used(), 0);
                        println!(
                            "{{\"status\":\"ok\",\"candidate\":false,\"regions\":[],\"region_support_count\":0,\"managed_used\":0,\"managed_peak\":{}{extra}}}",
                            budget.peak()
                        );
                        return;
                    };
                    let pass = |v: &rrrah_dedup::warp::WarpEvidence| {
                        v.compared_pixels >= p.local.minimum_compared_pixels
                            && v.compared_pixels as f64
                                >= v.source_pixels as f64 * p.local.minimum_coverage_fraction
                            && v.matched_pixels as f64
                                >= v.compared_pixels as f64 * p.local.minimum_matched_fraction
                    };
                    let mut supports = 0;
                    let rows=combined.regional.regions.iter().map(|r|{
                        let accepted=r.accepted_region;assert_eq!(accepted,r.pixels.as_ref().is_some_and(|e|pass(&e.fitted.forward.pixels)&&pass(&e.fitted.reverse.pixels)));supports+=usize::from(accepted);
                        let counts=r.pixels.as_ref().map_or_else(||"null".into(),|e|format!("[{:?},{:?}]",[e.fitted.forward.pixels.matched_pixels,e.fitted.forward.pixels.compared_pixels,e.fitted.forward.pixels.source_pixels],[e.fitted.reverse.pixels.matched_pixels,e.fitted.reverse.pixels.compared_pixels,e.fitted.reverse.pixels.source_pixels]));
                        let [a,b]=r.domains;format!("{{\"domains\":[{:?},{:?}],\"accepted_region\":{accepted},\"counts\":{counts},\"fit_failure\":{:?}}}",[a.x,a.y,a.width,a.height],[b.x,b.y,b.width,b.height],format!("{:?}",r.fit_failure))
                    }).collect::<Vec<_>>().join(",");
                    let geometry = combined
                        .regional
                        .whole
                        .geometry
                        .as_ref()
                        .map_or_else(|| "null".into(), |g| format!("{:?}", g.transform.matrix));
                    extra += &format!(
                        ",\"regional_geometry\":{geometry},\"regional_whole_candidate\":{},\"regions\":[{rows}],\"region_support_count\":{supports}",
                        combined.regional.whole.candidate
                    );
                    Ok(combined.whole)
                } else if collection {
                    use rrrah_dedup::{
                        local_collection::{
                            ProjectiveFiveSearchCollectionPolicy,
                            scan_projective_local_collection_five_searches,
                        },
                        local_index::FileFeatureBudgets,
                    };
                    let report = scan_projective_local_collection_five_searches(
                        [
                            (1, DecodeRequest::new(&args[0])),
                            (2, DecodeRequest::new(&args[1])),
                        ],
                        ProjectiveFiveSearchCollectionPolicy {
                            search: policy,
                            budgets: FileFeatureBudgets {
                                max_files: 2,
                                max_features: 10000,
                                max_hits: 34_000_000,
                                max_pair_counts: 1,
                                max_pairs: 1,
                            },
                            max_gradient_retrieval_comparisons: 18_000_000,
                        },
                        &budget,
                        || false,
                    )
                    .unwrap();
                    assert_eq!(report.analysed, vec![1, 2]);
                    assert!(
                        report.file_issues.is_empty()
                            && report.local.issues.is_empty()
                            && report.local.source_issues.is_empty()
                    );
                    extra += &format!(
                        ",\"retrieved\":{},\"indexed_features\":{},\"descriptor_hits\":{}",
                        !report.local.pairs.is_empty(),
                        report.indexed_features,
                        report.descriptor_hits
                    );
                    if report.local.pairs.is_empty() {
                        drop(report);
                        assert_eq!(budget.used(), 0);
                        println!(
                            "{{\"status\":\"ok\",\"candidate\":false,\"managed_used\":0,\"managed_peak\":{}{extra}}}",
                            budget.peak()
                        );
                        return;
                    }
                    assert_eq!(report.local.pairs.len(), 1);
                    Ok(report.local.pairs.into_iter().next().unwrap().evidence)
                } else {
                    compare_local_files_projective_complementary_gradient_portfolio(
                        &DecodeRequest::new(&args[0]),
                        &DecodeRequest::new(&args[1]),
                        policy,
                        &budget,
                        || false,
                    )
                };
                measured.map(|e| {
                    let g = &e.interpolated;
                    let model = g.geometry.as_ref().map_or_else(|| "null".into(), |m| format!("{:?}", m.transform.matrix));
                    let counts = g.pixels.as_ref().map_or_else(|| "null".into(), |v| format!("[{:?},{:?}]", [v.fitted.forward.pixels.matched_pixels,v.fitted.forward.pixels.compared_pixels,v.fitted.forward.pixels.source_pixels],[v.fitted.reverse.pixels.matched_pixels,v.fitted.reverse.pixels.compared_pixels,v.fitted.reverse.pixels.source_pixels]));
                    extra += &format!(",\"five_accepted_searches\":{:?},\"four_candidate\":{},\"interpolated_candidate\":{},\"interpolated_correspondences\":{},\"interpolated_inliers\":{},\"interpolated_geometry\":{model},\"interpolated_counts\":{counts},\"interpolated_fit_failure\":{:?}", e.accepted_searches,e.primary.candidate,g.candidate,g.correspondences.len(),g.geometry.as_ref().map_or(0,|m|m.inliers.len()),format!("{:?}",g.fit_failure));
                    five_candidate = Some(e.candidate);
                    e.primary
                })
            } else {
                compare_local_files_projective_complementary_gradient(
                    &DecodeRequest::new(&args[0]),
                    &DecodeRequest::new(&args[1]),
                    joined,
                    &budget,
                    || false,
                )
            };
            combined.map(|e| {
                let g=&e.gradient;
                let model=g.geometry.as_ref().map_or_else(||"null".into(),|m|format!("{:?}",m.transform.matrix));
                let pixel_counts=g.pixels.as_ref().map_or_else(||"null".into(),|v|format!("[{:?},{:?}]",[v.fitted.forward.pixels.matched_pixels,v.fitted.forward.pixels.compared_pixels,v.fitted.forward.pixels.source_pixels],[v.fitted.reverse.pixels.matched_pixels,v.fitted.reverse.pixels.compared_pixels,v.fitted.reverse.pixels.source_pixels]));
                extra+=&format!(",\"four_accepted_searches\":{:?},\"three_candidate\":{},\"gradient_candidate\":{},\"gradient_correspondences\":{},\"gradient_inliers\":{},\"gradient_geometry\":{model},\"gradient_counts\":{pixel_counts},\"gradient_fit_failure\":{:?}",e.accepted_searches,e.base.candidate,g.candidate,g.correspondences.len(),g.geometry.as_ref().map_or(0,|m|m.inliers.len()),format!("{:?}",g.fit_failure));
                four_candidate=Some(five_candidate.unwrap_or(e.candidate));
                e.base
            })
        } else if collection {
            use rrrah_dedup::{
                local_collection::{
                    ProjectiveComplementaryFilterPortfolioCollectionPolicy,
                    scan_projective_local_collection_complementary_filter_portfolio,
                },
                local_index::FileFeatureBudgets,
            };
            let config = ProjectiveComplementaryFilterPortfolioCollectionPolicy {
                search: q,
                // Both feature families, both files; retrieval also visits same-file hits.
                budgets: FileFeatureBudgets {
                    max_files: 2,
                    max_features: 4000,
                    max_hits: 16_000_000,
                    max_pair_counts: 1,
                    max_pairs: 1,
                },
            };
            match scan_projective_local_collection_complementary_filter_portfolio(
                [
                    (1, DecodeRequest::new(&args[0])),
                    (2, DecodeRequest::new(&args[1])),
                ],
                config,
                &budget,
                || false,
            ) {
                Ok(report) => {
                    if !report.file_issues.is_empty()
                        || !report.local.issues.is_empty()
                        || !report.local.source_issues.is_empty()
                    {
                        println!(
                            "{{\"status\":\"error\",\"error\":{:?},\"managed_used\":{},\"managed_peak\":{}}}",
                            format!(
                                "file={:?};pair={:?};source={:?}",
                                report.file_issues, report.local.issues, report.local.source_issues
                            ),
                            budget.used(),
                            budget.peak()
                        );
                        return;
                    }
                    if let Some(pair) = report.local.pairs.into_iter().next() {
                        extra = ",\"retrieved_pairs\":1".into();
                        Ok(pair.evidence)
                    } else {
                        println!(
                            "{{\"status\":\"ok\",\"candidate\":false,\"retrieved_pairs\":0,\"managed_used\":{},\"managed_peak\":{}}}",
                            budget.used(),
                            budget.peak()
                        );
                        return;
                    }
                }
                Err(error) => {
                    println!(
                        "{{\"status\":\"error\",\"error\":{:?},\"managed_used\":{},\"managed_peak\":{}}}",
                        error.to_string(),
                        budget.used(),
                        budget.peak()
                    );
                    return;
                }
            }
        } else {
            compare_local_files_projective_complementary_filter_portfolio(
                &DecodeRequest::new(&args[0]),
                &DecodeRequest::new(&args[1]),
                q,
                &budget,
                || false,
            )
        };
        result.map(|e| {
            let secondary_counts = e.secondary_pixels.as_ref().map_or_else(|| "null".into(), |p| format!("[{:?},{:?}]",
                [p.fitted.forward.pixels.matched_pixels, p.fitted.forward.pixels.compared_pixels, p.fitted.forward.pixels.source_pixels],
                [p.fitted.reverse.pixels.matched_pixels, p.fitted.reverse.pixels.compared_pixels, p.fitted.reverse.pixels.source_pixels]));
            extra += &format!(",\"joined_accepted_searches\":{:?},\"secondary_counts\":{},\"secondary_fit_failure\":{:?},\"base_candidate\":{}",
                e.accepted_searches, secondary_counts, format!("{:?}", e.secondary_fit_failure), e.searches.candidate);
            let mut searches = e.searches;
            searches.candidate = four_candidate.unwrap_or(e.candidate);
            searches
        })
    } else {
        compare_local_files_projective_complementary(
            &DecodeRequest::new(&args[0]),
            &DecodeRequest::new(&args[1]),
            policy,
            &budget,
            || false,
        )
    };
    match result {
        Ok(e) => {
            let registration_counts = e.registration.pixels.as_ref().map_or_else(
                || "null".into(),
                |p| {
                    format!(
                        "[{},{}]",
                        counts(&p.anchored.filtered),
                        counts(&p.unanchored.filtered)
                    )
                },
            );
            let pyramid_counts = e.pyramid.pixels.as_ref().map_or_else(
                || "null".into(),
                |p| {
                    format!(
                        "[{:?},{:?}]",
                        [
                            p.fitted.forward.pixels.matched_pixels,
                            p.fitted.forward.pixels.compared_pixels,
                            p.fitted.forward.pixels.source_pixels
                        ],
                        [
                            p.fitted.reverse.pixels.matched_pixels,
                            p.fitted.reverse.pixels.compared_pixels,
                            p.fitted.reverse.pixels.source_pixels
                        ]
                    )
                },
            );
            let rg = e
                .registration
                .geometry
                .as_ref()
                .map_or_else(|| "null".into(), |g| format!("{:?}", g.transform.matrix));
            let pg = e
                .pyramid
                .geometry
                .as_ref()
                .map_or_else(|| "null".into(), |g| format!("{:?}", g.transform.matrix));
            println!(
                "{{\"status\":\"ok\",\"candidate\":{},\"accepted_searches\":{:?},\"registration_accepted_lanes\":{:?},\"registration_counts\":{},\"pyramid_counts\":{},\"registration_geometry\":{},\"pyramid_geometry\":{},\"correspondence_counts\":{:?},\"inlier_counts\":{:?},\"fit_failure\":{:?},\"managed_used\":{},\"managed_peak\":{}{} }}",
                e.candidate,
                e.accepted_searches,
                e.registration.accepted_lanes,
                registration_counts,
                pyramid_counts,
                rg,
                pg,
                [
                    e.registration.correspondences.len(),
                    e.pyramid.correspondences.len()
                ],
                [
                    e.registration.geometry.as_ref().map_or(0, |g| g.inliers.len()),
                    e.pyramid.geometry.as_ref().map_or(0, |g| g.inliers.len())
                ],
                format!("{:?}", e.pyramid.fit_failure),
                budget.used(),
                budget.peak(),
                extra
            );
        }
        Err(error) => println!(
            "{{\"status\":\"error\",\"error\":{:?},\"managed_used\":{},\"managed_peak\":{}}}",
            error.to_string(),
            budget.used(),
            budget.peak()
        ),
    }
}

/// Diagnostic geometry-derived 4x4 regions; no origin labels select domains.
fn pyramid_region_grid_pair(
    args: &[String],
    p: rrrah_dedup::local_scan::ProjectivePyramidPhotometricFilePolicy,
    budget: &MemoryBudget,
    collection: bool,
) {
    use rrrah_dedup::local_scan::{
        ProjectivePyramidRegionsFilePolicy, compare_local_files_projective_pyramid_region_grid,
    };
    let left = DecodeRequest::new(&args[0]);
    let right = DecodeRequest::new(&args[1]);
    let q = ProjectivePyramidRegionsFilePolicy {
        search: p,
        max_regions: 16,
        max_total_sample_pairs: p.filter.filter.max_sample_pairs * 17,
    };
    let mut metadata = String::new();
    let e = if collection {
        use rrrah_dedup::{
            local_collection::{
                ProjectiveRegionGridCollectionPolicy, scan_projective_local_collection_region_grid,
            },
            local_index::FileFeatureBudgets,
        };
        let report = scan_projective_local_collection_region_grid(
            [(1, left.clone()), (2, right.clone())],
            ProjectiveRegionGridCollectionPolicy {
                search: q,
                grid: (4, 4),
                budgets: FileFeatureBudgets {
                    max_files: 2,
                    max_features: 3000,
                    max_hits: 9_000_000,
                    max_pair_counts: 1,
                    max_pairs: 1,
                },
            },
            budget,
            || false,
        )
        .unwrap();
        assert_eq!(report.analysed, vec![1, 2]);
        assert!(
            report.file_issues.is_empty()
                && report.local.issues.is_empty()
                && report.local.source_issues.is_empty()
        );
        metadata = format!(
            ",\"retrieved\":{},\"indexed_features\":{},\"descriptor_hits\":{}",
            !report.local.pairs.is_empty(),
            report.indexed_features,
            report.descriptor_hits
        );
        if report.local.pairs.is_empty() {
            drop(report);
            assert_eq!(budget.used(), 0);
            println!(
                "{{\"status\":\"ok\",\"candidate\":false,\"geometry\":null,\"regions\":[],\"region_support_count\":0,\"managed_used_after_drop\":0,\"managed_peak\":{}{metadata}}}",
                budget.peak()
            );
            return;
        }
        assert_eq!(report.local.pairs.len(), 1);
        report.local.pairs.into_iter().next().unwrap().evidence
    } else {
        compare_local_files_projective_pyramid_region_grid(&left, &right, q, (4, 4), budget, || false)
            .unwrap()
    };
    let pass = |v: &rrrah_dedup::warp::WarpEvidence| {
        v.compared_pixels >= p.local.minimum_compared_pixels
            && v.compared_pixels as f64 >= v.source_pixels as f64 * p.local.minimum_coverage_fraction
            && v.matched_pixels as f64 >= v.compared_pixels as f64 * p.local.minimum_matched_fraction
    };
    let mut support = 0;
    let rows = e
        .regions
        .iter()
        .map(|r| {
            let accepted = r
                .pixels
                .as_ref()
                .is_some_and(|v| pass(&v.fitted.forward.pixels) && pass(&v.fitted.reverse.pixels));
            assert_eq!(accepted, r.accepted_region);
            support += usize::from(accepted);
            let counts = r.pixels.as_ref().map_or_else(
                || "null".into(),
                |v| {
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
                },
            );
            let [a, b] = r.domains;
            format!(
                "{{\"domains\":[{:?},{:?}],\"accepted_region\":{},\"counts\":{},\"fit_failure\":{:?}}}",
                [a.x, a.y, a.width, a.height],
                [b.x, b.y, b.width, b.height],
                accepted,
                counts,
                format!("{:?}", r.fit_failure)
            )
        })
        .collect::<Vec<_>>();
    let retained = budget.used();
    let candidate = e.whole.candidate;
    let geometry = e
        .whole
        .geometry
        .as_ref()
        .map_or_else(|| "null".to_string(), |g| format!("{:?}", g.transform.matrix));
    drop(e);
    println!(
        "{{\"status\":\"ok\",\"candidate\":{},\"geometry\":{},\"regions\":[{}],\"region_support_count\":{},\"managed_retained_bytes\":{},\"managed_used_after_drop\":{},\"managed_peak\":{},\"scope\":\"Geometry-derived 4x4 domains with original pixel thresholds; regional support does not promote a whole-image candidate.\"{metadata}}}",
        candidate,
        geometry,
        rows.join(","),
        support,
        retained,
        budget.used(),
        budget.peak()
    );
}

/// Supplied model isolates domain-generation changes from feature/model search.
fn bidirectional_region_transform_pair(args: &[String]) {
    use rrrah_dedup::{
        geometry::{ProjectiveSamplingPolicy, ProjectiveTransform},
        local_scan::{
            ProjectivePyramidPhotometricFilePolicy, ProjectivePyramidRegionsFilePolicy,
            compare_local_files_projective_bidirectional_region_grid_transform,
        },
        warp::PhotometricFitMode,
    };
    assert_eq!(
        args.len(),
        11,
        "two files and nine row-major model coefficients required"
    );
    let matrix =
        std::array::from_fn(|row| std::array::from_fn(|col| args[2 + row * 3 + col].parse::<f64>().unwrap()));
    let model = ProjectiveTransform { matrix };
    let mut local = policy();
    local.matching.max_comparisons = 2_250_000;
    local.geometry.max_points = 1500;
    local.geometry.max_hypotheses = 2048;
    local.pixels.max_source_pixels = 409_600;
    let mut photometric = photometric_policy();
    photometric.residual = local.pixels;
    photometric.minimum_samples = 1000;
    let search = ProjectivePyramidPhotometricFilePolicy {
        local,
        max_levels: 3,
        max_total_pixels: 400_000,
        max_total_features: 1500,
        sampling: ProjectiveSamplingPolicy {
            trials: 2048,
            seed: 0x1234abcd,
        },
        photometric,
        filter: ColorFilterPolicy {
            filter: FilterPolicy {
                radius: 3,
                max_sample_pairs: 32_000_000,
            },
            color_space: FilterColorSpace::LinearSrgb,
        },
        fit_mode: PhotometricFitMode::ConstrainedLeastSquares,
    };
    let q = ProjectivePyramidRegionsFilePolicy {
        search,
        max_regions: 32,
        max_total_sample_pairs: 1_056_000_000,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let evidence = compare_local_files_projective_bidirectional_region_grid_transform(
        &DecodeRequest::new(&args[0]),
        &DecodeRequest::new(&args[1]),
        model,
        q,
        (4, 4),
        &budget,
        || false,
    )
    .unwrap();
    let support = evidence.region_support_count();
    let rows = evidence
        .regions
        .iter()
        .map(|r| {
            let counts = r.pixels.as_ref().map_or_else(
                || "null".into(),
                |v| {
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
                },
            );
            let [a, b] = r.domains;
            format!(
                "{{\"domains\":[{:?},{:?}],\"accepted_region\":{},\"counts\":{},\"fit_failure\":{:?}}}",
                [a.x, a.y, a.width, a.height],
                [b.x, b.y, b.width, b.height],
                r.accepted_region,
                counts,
                format!("{:?}", r.fit_failure)
            )
        })
        .collect::<Vec<_>>();
    drop(evidence);
    println!(
        "{{\"status\":\"ok\",\"geometry\":{:?},\"regions\":[{}],\"region_support_count\":{},\"managed_used\":{},\"managed_peak\":{}}}",
        matrix,
        rows.join(","),
        support,
        budget.used(),
        budget.peak()
    );
}

fn six_whole_json(e: &rrrah_dedup::local_scan::ProjectivePhotometricFileEvidence) -> String {
    let geometry = e
        .geometry
        .as_ref()
        .map_or_else(|| "null".into(), |v| format!("{:?}", v.transform.matrix));
    let pixels = e.pixels.as_ref().map_or_else(
        || {
            e.fit_failure.map_or_else(
                || "null".into(),
                |v| format!("{{\"status\":\"error\",\"error\":\"Fit({v:?})\"}}"),
            )
        },
        |v| {
            let f = v.fitted.forward.pixels;
            let r = v.fitted.reverse.pixels;
            format!(
                "{{\"status\":\"ok\",\"counts\":[{:?},{:?}]}}",
                [f.matched_pixels, f.compared_pixels, f.source_pixels],
                [r.matched_pixels, r.compared_pixels, r.source_pixels]
            )
        },
    );
    format!(
        "{{\"candidate\":{},\"correspondences\":{},\"inliers\":{},\"geometry\":{geometry},\"pixels\":{pixels}}}",
        e.candidate,
        e.correspondences.len(),
        e.geometry.as_ref().map_or(0, |v| v.inliers.len())
    )
}
fn six_regions_json(e: Option<&rrrah_dedup::local_scan::ProjectiveTransformedRegionsFileEvidence>) -> String {
    let Some(e) = e else {
        return "null".into();
    };
    let rows = e
        .regions
        .iter()
        .map(|r| {
            let counts = r.pixels.as_ref().map_or_else(
                || "null".into(),
                |v| {
                    let f = v.fitted.forward.pixels;
                    let b = v.fitted.reverse.pixels;
                    format!(
                        "[{:?},{:?}]",
                        [f.matched_pixels, f.compared_pixels, f.source_pixels],
                        [b.matched_pixels, b.compared_pixels, b.source_pixels]
                    )
                },
            );
            let [a, b] = r.domains;
            format!(
                "{{\"domains\":[{:?},{:?}],\"accepted_region\":{},\"counts\":{counts},\"fit_failure\":{:?}}}",
                [a.x, a.y, a.width, a.height],
                [b.x, b.y, b.width, b.height],
                r.accepted_region,
                format!("{:?}", r.fit_failure)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"geometry\":{:?},\"regions\":[{rows}],\"region_support_count\":{}}}",
        e.transform.matrix,
        e.region_support_count()
    )
}

// Isolated experiment; neither kernel changes the production index here.
fn gradient_distance_kernel_bench(args: &[String]) {
    use rrrah_dedup::gradient::{GradientCellRecipe, extract_gradient_pyramid_with_recipe_managed};
    use std::{hint::black_box, time::Instant};
    assert!(!args.is_empty() && args.len() <= 12);
    fn full(a: &[f64; 128], b: &[f64; 128]) -> f64 {
        a.iter().zip(b).map(|(a, b)| (a - b) * (a - b)).sum()
    }
    fn bounded(a: &[f64; 128], b: &[f64; 128], radius: f64) -> f64 {
        let mut sum = 0.;
        for (a, b) in a.chunks_exact(16).zip(b.chunks_exact(16)) {
            for (a, b) in a.iter().zip(b) {
                let d = a - b;
                sum += d * d;
            }
            if sum > radius {
                break;
            }
        }
        sum
    }
    let p = policy();
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let pyramid = rrrah_dedup::pyramid::PyramidPolicy {
        local: p.extract,
        max_levels: 3,
        max_total_pixels: 600000,
        max_total_features: 1500,
    };
    let mut records = Vec::new();
    for recipe in [GradientCellRecipe::Fixed, GradientCellRecipe::Interpolated] {
        let mut descriptors = Vec::new();
        for path in args {
            let image = decode_selected_frame_bounded(&DecodeRequest::new(path), p.decode, &budget, || false)
                .unwrap();
            let view = image.view(|| false).unwrap();
            let features = extract_gradient_pyramid_with_recipe_managed(
                &view,
                pyramid,
                1536000,
                recipe,
                &budget,
                || false,
            )
            .unwrap();
            descriptors.extend(features.iter().map(|f| f.descriptor.0));
        }
        assert!(descriptors.len() >= 128);
        assert_eq!(budget.used(), 0);
        let queries = (0..128)
            .map(|i| descriptors[i * descriptors.len() / 128])
            .collect::<Vec<_>>();
        for radius in [0., 0.1, 0.5, 4.] {
            let mut accepted = 0usize;
            for q in &queries {
                for d in &descriptors {
                    let a = full(q, d);
                    let b = bounded(q, d, radius);
                    assert_eq!(a <= radius, b <= radius);
                    if a <= radius {
                        assert_eq!(a.to_bits(), b.to_bits());
                        accepted += 1;
                    }
                }
            }
            let mut old = Vec::new();
            let mut new = Vec::new();
            for round in 0..5 {
                for early in if round % 2 == 0 {
                    [false, true]
                } else {
                    [true, false]
                } {
                    let start = Instant::now();
                    let mut count = 0usize;
                    for q in &queries {
                        for d in &descriptors {
                            let value = if early {
                                bounded(black_box(q), black_box(d), black_box(radius))
                            } else {
                                full(black_box(q), black_box(d))
                            };
                            count += usize::from(value <= radius);
                        }
                    }
                    let elapsed = start.elapsed().as_nanos();
                    assert_eq!(count, accepted);
                    black_box(count);
                    if early {
                        new.push(elapsed);
                    } else {
                        old.push(elapsed);
                    }
                }
            }
            records.push(format!("{{\"recipe\":{:?},\"radius\":{radius},\"descriptors\":{},\"queries\":128,\"accepted\":{accepted},\"full_ns\":{:?},\"bounded_ns\":{:?}}}",format!("{recipe:?}"),descriptors.len(),old,new));
        }
    }
    println!(
        "{{\"status\":\"verified_kernel_experiment\",\"records\":[{}],\"scope\":\"Real native descriptor arrays, five alternating timing samples; exact accepted distances and decisions. Isolated kernel experiment under current host load; no production change or end-to-end speed claim.\"}}",
        records.join(",")
    );
}
