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
    if arguments.get(1).is_some_and(|a| {
        a == "--complementary-file-pair"
            || a == "--complementary-filter-portfolio-pair"
            || a == "--complementary-filter-collection-pair"
    }) {
        complementary_file_pair(
            &arguments[2..],
            arguments[1] != "--complementary-file-pair",
            arguments[1] == "--complementary-filter-collection-pair",
        );
        return;
    }
    if arguments.get(1).is_some_and(|a| {
        a == "--pyramid-file-pair"
            || a == "--pyramid-collection-pair"
            || a == "--pyramid-blur-pair"
            || a == "--pyramid-encoded-blur-pair"
            || a == "--pyramid-filter-portfolio-pair"
    }) {
        pyramid_file_pair(
            &arguments[2..],
            arguments[1] == "--pyramid-collection-pair",
            arguments[1] == "--pyramid-blur-pair" || arguments[1] == "--pyramid-encoded-blur-pair",
            arguments[1] == "--pyramid-filter-portfolio-pair",
            arguments[1] == "--pyramid-encoded-blur-pair",
        );
        return;
    }
    if arguments.get(1).is_some_and(|a| {
        a == "--projective-pair"
            || a == "--projective-pair-spatial"
            || a == "--projective-pair-sampled"
            || a == "--projective-pair-anchored"
            || a == "--projective-pair-portfolio"
    }) {
        projective_pair(
            &arguments[2..],
            arguments[1] == "--projective-pair-spatial",
            arguments[1] == "--projective-pair-sampled" || arguments[1] == "--projective-pair-anchored",
            arguments[1] == "--projective-pair-anchored",
            arguments[1] == "--projective-pair-portfolio",
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
                spatial:None,sampling:Some(rrrah_dedup::geometry::ProjectiveSamplingPolicy{trials:2048,seed:0x1234abcd})},&budget,||false).map(|e| {
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
        geometry::ProjectiveTransform,
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
            rows.push(format!("{{\"limit\":{limit},\"spatial\":{spatial},\"left_features\":{},\"right_features\":{},\"correspondences\":{},\"published_geometry_inliers\":{correct}}}",af.len(),bf.len(),matches.len()));
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

fn pyramid_file_pair(args: &[String], collection: bool, blur: bool, portfolio: bool, encoded: bool) {
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

fn pyramid_correspondences(args: &[String]) {
    use rrrah_dedup::{
        geometry::{ProjectiveSamplingPolicy, verify_projective_sampled},
        pyramid::{PyramidPolicy, extract_oriented_pyramid},
    };
    assert!(
        args.len() >= 2
            && args.len() <= 4
            && args[2..]
                .iter()
                .all(|v| v == "--registration" || v == "--low-contrast")
    );
    let registration_diagnostic = args[2..].iter().any(|v| v == "--registration");
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
        let af = extract_oriented_pyramid(&a, pyramid, || false).unwrap();
        let bf = extract_oriented_pyramid(&b, pyramid, || false).unwrap();
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
        rows.push(format!("{{\"levels\":{levels},\"left_features\":{},\"right_features\":{},\"correspondences\":{},\"inliers\":{},\"geometry\":{},\"pixel_verification\":{verification},\"registration_verification\":{refined},\"photometric_verification\":{photometric}}}",af.len(),bf.len(),matches.len(),model.as_ref().map_or(0,|m|m.inliers.len()),model.map_or_else(||"null".into(),|m|format!("{:?}",m.transform.matrix))));
    }
    drop(left);
    drop(right);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"ok\",\"minimum_corner_score\":{},\"registration_requested\":{registration_diagnostic},\"diagnostics\":[{}],\"scope\":\"Scale-pyramid verification with fixed pixel criteria and optional explicit registration; no known geometry or semantic accuracy qualification. Public feature allocations outside managed decode accounting.\"}}",
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

fn complementary_file_pair(args: &[String], portfolio: bool, collection: bool) {
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
    assert_eq!(args.len(), 2);
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
    let budget = MemoryBudget::new(64 * 1024 * 1024);
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
        let result = if collection {
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
            searches.candidate = e.candidate;
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
