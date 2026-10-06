//! Independent rendered-raster comparison; global evidence is not pixel equality.
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::decode::decode_selected_frame;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    validate_arguments(&args)?;
    let budget = MemoryBudget::new(2 * 1024 * 1024 * 1024);
    if args
        .get(2)
        .is_some_and(|mode| mode.starts_with("--verify-spatial"))
    {
        return verify_spatial_files(&args[..2], &budget, &args[2]);
    }
    if args.get(2).is_some_and(|mode| mode == "--verify-known-shift") {
        return verify_known_shift(&args[..2], &budget);
    }
    let mut results = Vec::new();
    let mut statistics = Vec::new();
    let mut features = Vec::new();
    for (i, path) in args[..2].iter().enumerate() {
        let mut request = DecodeRequest::new(path);
        request.assume_untagged_srgb = i == 1;
        request.memory_budget = Some(budget.clone());
        let image = if i == 0 && args.get(2).is_some_and(|mode| mode == "--no-highlight-recovery") {
            let rrrah_decode::DecodedImage::Sensor(sensor) = rrrah_decode::decode_image(&request)? else {
                return Err("diagnostic RAW routed as raster".into());
            };
            let lists = rrrah_decode::raw_development_opcodes(&request)?;
            let developed = rrrah_core::develop::develop_raw(
                &sensor.mosaic,
                &rrrah_core::develop::DevelopOptions {
                    recover_highlights: false,
                    ..Default::default()
                },
                &lists,
                Some(&budget),
                &|| false,
            )?;
            rrrah_dedup::raster::NormalizedRaster::new(&developed, 100_000_000, &budget, || false)?
        } else {
            decode_selected_frame(&request, 100_000_000, &budget, || false)?
        };
        let view = image.view(|| false)?;
        statistics.push(sample_statistics(&view, i)?);
        if std::env::var_os("RRRAH_FULL_RANGE").is_some() {
            report_full_range(&view, i)?;
        }
        if args
            .get(2)
            .is_some_and(|mode| ["--local", "--pyramid", "--rank", "--spatial"].contains(&mode.as_str()))
        {
            let policy = rrrah_dedup::local::LocalPolicy {
                max_pixels: 100_000_000,
                max_candidates: 25_000_000,
                max_features: 500,
                minimum_corner_score: 0.0001,
            };
            let extracted = if args[2] == "--pyramid" {
                rrrah_dedup::pyramid::extract_oriented_pyramid(
                    &view,
                    rrrah_dedup::pyramid::PyramidPolicy {
                        local: policy,
                        max_levels: 5,
                        max_total_pixels: 40_000_000,
                        max_total_features: 2500,
                    },
                    || false,
                )?
            } else if args[2] == "--spatial" {
                rrrah_dedup::local::extract_spatial_oriented(&view, policy, 16, 12, 3, || false)?
            } else if args[2] == "--rank" {
                rrrah_dedup::ordinal::extract_rank_oriented(&view, policy, || false)?
            } else {
                rrrah_dedup::local::extract_multiscale_oriented(&view, policy, || false)?
            };
            features.push(extracted);
        }
        results.push((view.dimensions(), view.fingerprint(|| false)?));
        drop(image);
        if budget.used() != 0 {
            return Err("managed memory retained".into());
        }
    }
    report_statistics(&statistics);
    report_local(&features)?;
    let evidence = results[0].1.compare(&results[1].1, true);
    println!(
        "{{\"raw_dimensions\":[{},{}],\"raster_dimensions\":[{},{}],\"global_distance\":{},\"right_transform\":{},\"informative\":{},\"managed_peak\":{}}}",
        results[0].0.0,
        results[0].0.1,
        results[1].0.0,
        results[1].0.1,
        evidence.distance,
        evidence.right_transform,
        evidence.informative,
        budget.peak()
    );
    Ok(())
}

fn report_local(features: &[Vec<rrrah_dedup::local::Feature>]) -> Result<(), Box<dyn std::error::Error>> {
    if features.len() == 2 {
        if let Ok(shift) = std::env::var("RRRAH_EXPECTED_SHIFT") {
            let values = shift
                .split(',')
                .map(str::parse::<f64>)
                .collect::<Result<Vec<_>, _>>()?;
            if values.len() != 2 {
                return Err("expected shift requires x,y".into());
            }
            report_known_shift(&features[0], &features[1], [values[0], values[1]]);
        }
        let matches = rrrah_dedup::local_index::match_features_indexed(
            &features[0],
            &features[1],
            rrrah_dedup::local_index::IndexedMatchPolicy {
                max_features_per_side: 2500,
                max_hits: 12_500_000,
                max_distance: 64,
            },
            || false,
        )?;
        let geometry = rrrah_dedup::geometry::verify_similarity(
            &matches,
            rrrah_dedup::geometry::GeometryPolicy {
                tolerance: 2.0,
                min_inliers: 10,
                max_points: 2500,
                max_hypotheses: 3_125_000,
            },
            || false,
        )?;
        eprintln!(
            "local: features={:?}, correspondences={}, geometry={geometry:?}",
            features.iter().map(Vec::len).collect::<Vec<_>>(),
            matches.len()
        );
    }
    Ok(())
}

fn report_statistics(statistics: &[([f64; 3], usize, usize)]) {
    for (i, (mean, above, below)) in statistics.iter().enumerate() {
        eprintln!(
            "input {i}: linear channel means {mean:?}, above-one channels {above}/3072, negative channels {below}/3072"
        );
    }
}

type SampleStatistics = ([f64; 3], usize, usize);
fn sample_statistics(
    view: &rrrah_dedup::linear::LinearRgbaView<'_>,
    i: usize,
) -> Result<SampleStatistics, Box<dyn std::error::Error>> {
    let (width, height) = view.dimensions();
    let mut mean = [0.0f64; 3];
    let mut grid = Vec::with_capacity(32 * 32 * 16);
    let mut above_one = 0usize;
    let mut below_zero = 0usize;
    for y in 0..32 {
        for x in 0..32 {
            let pixel = view
                .rgba((2 * x + 1) * width / 64, (2 * y + 1) * height / 64)
                .ok_or("sample outside image")?;
            for value in pixel {
                grid.extend_from_slice(&value.to_le_bytes());
            }
            for c in 0..3 {
                mean[c] += f64::from(pixel[c]) / 1024.0;
                above_one += usize::from(pixel[c] > 1.0);
                below_zero += usize::from(pixel[c] < 0.0);
            }
        }
    }
    if let Some(prefix) = std::env::var_os("RRRAH_SAMPLE_GRID_PREFIX") {
        let mut path = prefix;
        path.push(format!("-{i}.f32le"));
        std::fs::write(std::path::PathBuf::from(path), &grid)?;
    }
    Ok((mean, above_one, below_zero))
}

fn report_known_shift(
    left: &[rrrah_dedup::local::Feature],
    right: &[rrrah_dedup::local::Feature],
    shift: [f64; 2],
) {
    let mut distances = Vec::new();
    for a in left {
        if let Some(b) = right
            .iter()
            .filter(|b| {
                (b.position[0] - a.position[0] - shift[0]).hypot(b.position[1] - a.position[1] - shift[1])
                    <= 2.0
            })
            .min_by(|a, b| a.position[0].total_cmp(&b.position[0]))
        {
            let distance = std::iter::once(&a.descriptor)
                .chain(&a.quarter_turns)
                .flat_map(|av| {
                    std::iter::once(&b.descriptor)
                        .chain(&b.quarter_turns)
                        .map(move |bv| av.iter().zip(bv).map(|(x, y)| (x ^ y).count_ones()).sum::<u32>())
                })
                .min()
                .unwrap();
            distances.push(distance);
        }
    }
    distances.sort_unstable();
    eprintln!(
        "known shift {shift:?}: source_features={}, positions_with_target_within_2px={}, descriptor_distances={distances:?}",
        left.len(),
        distances.len()
    );
}

fn verify_spatial_files(
    paths: &[String],
    budget: &MemoryBudget,
    mode: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use rrrah_dedup::{
        animated::AnimationBudget,
        geometry::GeometryPolicy,
        local::{LocalPolicy, MatchPolicy},
        local_scan::{LocalFilePolicy, SpatialFeaturePolicy, compare_local_files_spatial_with_policy},
        warp::WarpPolicy,
    };
    let left = DecodeRequest::new(&paths[0]);
    let mut right = DecodeRequest::new(&paths[1]);
    right.assume_untagged_srgb = true;
    let policy = LocalFilePolicy {
        decode: AnimationBudget {
            max_frames: 10,
            max_pixels: 100_000_000,
            max_file_bytes: 256 * 1024 * 1024,
        },
        extract: LocalPolicy {
            max_pixels: 100_000_000,
            max_candidates: 25_000_000,
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
            max_source_pixels: 100_000_000,
        },
        minimum_compared_pixels: 1000,
        minimum_coverage_fraction: 0.3,
        minimum_matched_fraction: 0.9,
    };
    let photometric = rrrah_dedup::warp::PhotometricPolicy {
        residual: policy.pixels,
        minimum_samples: 1000,
        minimum_variance: 1e-5,
        minimum_gain: 0.2,
        maximum_gain: 5.0,
        maximum_offset: 0.1,
    };
    let search = rrrah_dedup::local_scan::LocalSearchPolicy {
        local: policy,
        comparison: match mode {
            "--verify-spatial-display" => rrrah_dedup::local_scan::LocalComparisonMode::DisplayProjection {
                photometric,
                range: rrrah_dedup::warp::FitSampleRange {
                    minimum: 0.0,
                    maximum: 1.0,
                },
            },
            "--verify-spatial-range" => rrrah_dedup::local_scan::LocalComparisonMode::RangePhotometric {
                photometric,
                range: rrrah_dedup::warp::FitSampleRange {
                    minimum: 0.0,
                    maximum: 1.0,
                },
            },
            "--verify-spatial-photometric" => {
                rrrah_dedup::local_scan::LocalComparisonMode::Photometric(photometric)
            }
            "--verify-spatial-filtered" => rrrah_dedup::local_scan::LocalComparisonMode::Filtered {
                photometric,
                filter: rrrah_dedup::warp::ColorFilterPolicy {
                    filter: rrrah_dedup::warp::FilterPolicy {
                        radius: 1,
                        max_sample_pairs: 900_000_000,
                    },
                    color_space: rrrah_dedup::warp::FilterColorSpace::EncodedSrgb,
                },
                fit: rrrah_dedup::warp::PhotometricFitMode::ConstrainedLeastSquares,
            },
            _ => rrrah_dedup::local_scan::LocalComparisonMode::Strict,
        },
    };
    let spatial = SpatialFeaturePolicy {
        columns: 16,
        rows: 12,
        max_per_cell: 3,
    };
    if std::env::var_os("RRRAH_VERIFY_COLLECTION").is_some() {
        let files = collection_inputs(left, right)?;
        return verify_collection(files, search, spatial, budget);
    }
    let result = compare_local_files_spatial_with_policy(&left, &right, search, spatial, budget, || false);
    println!("{result:#?}");
    println!("managed_used={} managed_peak={}", budget.used(), budget.peak());
    if budget.used() != 0 {
        return Err("managed memory retained".into());
    }
    result?;
    Ok(())
}

fn validate_arguments(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() == 2
        || (args.len() == 3
            && [
                "--no-highlight-recovery",
                "--local",
                "--pyramid",
                "--rank",
                "--spatial",
                "--verify-spatial",
                "--verify-spatial-photometric",
                "--verify-spatial-range",
                "--verify-spatial-display",
                "--verify-spatial-filtered",
                "--verify-known-shift",
            ]
            .contains(&args[2].as_str()))
    {
        return Ok(());
    }
    Err("usage: raw_raster_probe RAW RENDERED_RASTER [--no-highlight-recovery|--local|--pyramid|--rank|--spatial|--verify-spatial|--verify-spatial-photometric|--verify-spatial-range|--verify-spatial-display|--verify-spatial-filtered|--verify-known-shift]".into())
}

fn report_full_range(
    view: &rrrah_dedup::linear::LinearRgbaView<'_>,
    input: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let (w, h) = view.dimensions();
    let mut minimum = [f32::INFINITY; 3];
    let mut maximum = [f32::NEG_INFINITY; 3];
    let mut locations = [(0, 0); 3];
    let mut minimum_locations = [(0, 0); 3];
    let mut sums = [0.0f64; 3];
    let mut above = [[0u64; 4]; 3];
    for y in 0..h {
        for x in 0..w {
            let pixel = view.rgba(x, y).ok_or("range pixel unavailable")?;
            for c in 0..3 {
                let v = pixel[c];
                if v < minimum[c] {
                    minimum[c] = v;
                    minimum_locations[c] = (x, y);
                }
                if v > maximum[c] {
                    maximum[c] = v;
                    locations[c] = (x, y);
                }
                sums[c] += f64::from(v);
                for (i, t) in [1.0, 2.0, 10.0, 100.0].iter().enumerate() {
                    above[c][i] += u64::from(v > *t);
                }
            }
        }
    }
    let mean = sums.map(|v| v / (f64::from(w) * f64::from(h)));
    eprintln!(
        "FULL_RANGE input={input} dimensions=({w},{h}) minimum={minimum:?} minimum_locations={minimum_locations:?} maximum={maximum:?} maximum_locations={locations:?} mean={mean:?} above_1_2_10_100={above:?}"
    );
    Ok(())
}

fn verify_collection(
    files: Vec<(u64, DecodeRequest)>,
    search: rrrah_dedup::local_scan::LocalSearchPolicy,
    spatial: rrrah_dedup::local_scan::SpatialFeaturePolicy,
    budget: &MemoryBudget,
) -> Result<(), Box<dyn std::error::Error>> {
    use rrrah_dedup::{
        local_collection::{LocalCollectionPolicy, scan_local_collection_spatial},
        local_index::FileFeatureBudgets,
    };
    let count = files.len();
    let pairs = count * count.saturating_sub(1) / 2;
    let report = scan_local_collection_spatial(
        files,
        LocalCollectionPolicy {
            search,
            budgets: FileFeatureBudgets {
                max_files: count,
                max_features: count * 500,
                max_hits: count * count * 1_000_000,
                max_pair_counts: pairs,
                max_pairs: pairs,
            },
        },
        spatial,
        budget,
        || false,
    )?;
    println!(
        "indexed_features={} descriptor_hits={} proposed_pairs={} pixel_verification_pairs={} analysed={:?} file_issues={:?} pair_issues={:?} source_issues={:?}",
        report.indexed_features,
        report.descriptor_hits,
        report.proposed_pairs,
        report.pixel_verification_pairs,
        report.analysed,
        report.file_issues,
        report.local.issues,
        report.local.source_issues
    );
    for pair in report.local.pairs {
        println!(
            "pair=({}, {}) candidate={} geometry={} pixels={:?}",
            pair.left,
            pair.right,
            pair.evidence.candidate,
            pair.evidence.geometry.is_some(),
            pair.evidence.pixels
        );
    }
    println!("managed_used={} managed_peak={}", budget.used(), budget.peak());
    if budget.used() != 0 {
        return Err("managed memory retained".into());
    }
    Ok(())
}

fn collection_inputs(
    left: DecodeRequest,
    right: DecodeRequest,
) -> Result<Vec<(u64, DecodeRequest)>, Box<dyn std::error::Error>> {
    let Some(paths) = std::env::var_os("RRRAH_COLLECTION_PATHS") else {
        return Ok(vec![(1, left), (2, right)]);
    };
    let paths = paths
        .to_str()
        .ok_or("collection path list must be UTF-8")?
        .lines()
        .collect::<Vec<_>>();
    if paths.len() < 2 || paths.len() > 32 {
        return Err("diagnostic collection requires 2..32 paths".into());
    }
    Ok(paths
        .iter()
        .zip(1u64..)
        .map(|(path, id)| {
            let mut request = DecodeRequest::new(path);
            request.assume_untagged_srgb = true;
            (id, request)
        })
        .collect())
}

fn verify_known_shift(paths: &[String], budget: &MemoryBudget) -> Result<(), Box<dyn std::error::Error>> {
    let value = std::env::var("RRRAH_EXPECTED_SHIFT")?;
    let shift = value
        .split(',')
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()?;
    if shift.len() != 2 || shift.iter().any(|v| !v.is_finite()) {
        return Err("expected finite x,y shift".into());
    }
    let left = decode_selected_frame(&DecodeRequest::new(&paths[0]), 100_000_000, budget, || false)?;
    let mut request = DecodeRequest::new(&paths[1]);
    request.assume_untagged_srgb = true;
    let right = decode_selected_frame(&request, 100_000_000, budget, || false)?;
    let evidence = rrrah_dedup::warp::verify_bidirectional(
        &left.view(|| false)?,
        &right.view(|| false)?,
        rrrah_dedup::geometry::Transform {
            a: 1.0,
            b: 0.0,
            translation: [shift[0], shift[1]],
        },
        rrrah_dedup::warp::WarpPolicy {
            tolerance: 0.03,
            max_source_pixels: 100_000_000,
        },
        || false,
    )?;
    if std::env::var_os("RRRAH_RESIDUAL_BINS").is_some() {
        report_residual_bins(
            &left.view(|| false)?,
            &right.view(|| false)?,
            [shift[0], shift[1]],
        )?;
    }
    if std::env::var_os("RRRAH_FIT_RANGE").is_some() {
        report_known_range_fit(
            &left.view(|| false)?,
            &right.view(|| false)?,
            [shift[0], shift[1]],
        )?;
    }
    println!("diagnostic_known_shift={shift:?} pixels={evidence:#?}");
    drop((left, right));
    println!("managed_used={} managed_peak={}", budget.used(), budget.peak());
    if budget.used() != 0 {
        return Err("managed memory retained".into());
    }
    Ok(())
}

fn report_residual_bins(
    left: &rrrah_dedup::linear::LinearRgbaView<'_>,
    right: &rrrah_dedup::linear::LinearRgbaView<'_>,
    shift: [f64; 2],
) -> Result<(), Box<dyn std::error::Error>> {
    if shift
        .iter()
        .any(|v| v.fract() != 0.0 || *v < 0.0 || *v > f64::from(u32::MAX))
    {
        return Err("bins require nonnegative integer shift".into());
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let offset = [shift[0] as u32, shift[1] as u32];
    let (w, h) = left.dimensions();
    // Buckets: both samples within display range; native RAW outside range;
    // remaining pixels (target out of range). No samples are excluded.
    let mut counts = [0u64; 3];
    let mut mean_source = [[0.0f64; 3]; 3];
    let mut mean_target = [[0.0f64; 3]; 3];
    let mut variance = [[0.0f64; 3]; 3];
    let mut covariance = [[0.0f64; 3]; 3];
    let mut rejected = [0u64; 3];
    let mut squared = [[0.0f64; 3]; 3];
    for y in 0..h {
        for x in 0..w {
            let a = left.rgba(x, y).ok_or("source sample absent")?;
            let Some(b) = x
                .checked_add(offset[0])
                .zip(y.checked_add(offset[1]))
                .and_then(|(x, y)| right.rgba(x, y))
            else {
                continue;
            };
            let raw_out = a[..3].iter().any(|v| !(0.0..=1.0).contains(v));
            let target_out = b[..3].iter().any(|v| !(0.0..=1.0).contains(v));
            let bucket = if raw_out {
                1
            } else if target_out {
                2
            } else {
                0
            };
            counts[bucket] += 1;
            let count = f64::from(u32::try_from(counts[bucket])?);
            let mut fails = false;
            for c in 0..3 {
                let delta = f64::from(a[c]) - f64::from(b[c]);
                squared[bucket][c] += delta * delta;
                let da = f64::from(a[c]) - mean_source[bucket][c];
                let db = f64::from(b[c]) - mean_target[bucket][c];
                mean_source[bucket][c] += da / count;
                mean_target[bucket][c] += db / count;
                variance[bucket][c] += da * (f64::from(a[c]) - mean_source[bucket][c]);
                covariance[bucket][c] += da * (f64::from(b[c]) - mean_target[bucket][c]);
                fails |= delta.abs() > 0.03;
            }
            rejected[bucket] += u64::from(fails);
        }
    }
    println!("residual_bins counts={counts:?} rejected={rejected:?} squared_rgb={squared:?}");
    for bucket in 0..3 {
        if counts[bucket] < 2 || variance[bucket].iter().any(|v| *v <= 0.0) {
            println!("diagnostic_bin_fit bucket={bucket} insufficient_variance");
            continue;
        }
        let gain = std::array::from_fn::<_, 3, _>(|c| covariance[bucket][c] / variance[bucket][c]);
        let offset =
            std::array::from_fn::<_, 3, _>(|c| mean_target[bucket][c] - gain[c] * mean_source[bucket][c]);
        println!(
            "diagnostic_bin_fit bucket={bucket} gain={gain:?} offset={offset:?} samples={}",
            counts[bucket]
        );
    }
    Ok(())
}

fn report_known_range_fit(
    left: &rrrah_dedup::linear::LinearRgbaView<'_>,
    right: &rrrah_dedup::linear::LinearRgbaView<'_>,
    shift: [f64; 2],
) -> Result<(), Box<dyn std::error::Error>> {
    use rrrah_dedup::warp::{
        FitSampleRange, PhotometricPolicy, WarpPolicy, verify_photometric_bidirectional_in_range,
    };
    let result = verify_photometric_bidirectional_in_range(
        left,
        right,
        rrrah_dedup::geometry::Transform {
            a: 1.0,
            b: 0.0,
            translation: shift,
        },
        PhotometricPolicy {
            residual: WarpPolicy {
                tolerance: 0.03,
                max_source_pixels: 100_000_000,
            },
            minimum_samples: 1000,
            minimum_variance: 1e-5,
            minimum_gain: 0.2,
            maximum_gain: 5.0,
            maximum_offset: 0.1,
        },
        FitSampleRange {
            minimum: 0.0,
            maximum: 1.0,
        },
        || false,
    )?;
    println!("diagnostic_known_range_fit={result:#?}");
    Ok(())
}
