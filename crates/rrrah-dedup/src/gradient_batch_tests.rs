//! Production four-lane matcher against the retained scalar baseline.
use super::*;
fn scalar(
    left: &[GradientFeature],
    right: &[GradientFeature],
    policy: GradientMatchPolicy,
    distinct_radius_squared: Option<f64>,
    cancel: impl Fn() -> bool,
) -> Result<Vec<crate::geometry::Correspondence>, LocalError> {
    if !policy.max_squared_distance.is_finite()
        || !(0. ..=4.).contains(&policy.max_squared_distance)
        || !policy.squared_ratio.is_finite()
        || policy.squared_ratio <= 0.
        || policy.squared_ratio >= 1.
    {
        return Err(LocalError::Invalid);
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let work = u64::try_from(left.len())
        .ok()
        .and_then(|a| u64::try_from(right.len()).ok().and_then(|b| a.checked_mul(b)))
        .ok_or(LocalError::Budget)?;
    let work = work
        .checked_mul(if distinct_radius_squared.is_some() { 2 } else { 1 })
        .ok_or(LocalError::Budget)?;
    if work > policy.max_comparisons {
        return Err(LocalError::Budget);
    }
    for feature in left.iter().chain(right) {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        let d = &feature.descriptor.0;
        if feature.position.iter().any(|v| !v.is_finite())
            || d.iter().any(|v| !v.is_finite() || *v < 0.)
            || (d.iter().map(|v| v * v).sum::<f64>() - 1.).abs() > 1e-6
        {
            return Err(LocalError::Invalid);
        }
    }
    if left.is_empty() || right.is_empty() {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        return Ok(Vec::new());
    }
    let allocate = |len| {
        let mut v = Vec::new();
        v.try_reserve_exact(len).map_err(|_| LocalError::Budget)?;
        v.resize(len, (f64::INFINITY, f64::INFINITY, usize::MAX));
        Ok::<_, LocalError>(v)
    };
    let mut forward = allocate(left.len())?;
    let mut reverse = allocate(right.len())?;
    let update = |best: &mut (f64, f64, usize), distance, index| {
        if distance < best.0 {
            *best = (distance, best.0, index);
        } else {
            best.1 = best.1.min(distance);
        }
    };
    for (i, a) in left.iter().enumerate() {
        for (j, b) in right.iter().enumerate() {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let distance = a
                .descriptor
                .0
                .iter()
                .zip(b.descriptor.0.iter())
                .map(|(a, b)| (a - b) * (a - b))
                .sum();
            update(&mut forward[i], distance, j);
            update(&mut reverse[j], distance, i);
        }
    }
    if let Some(radius) = distinct_radius_squared {
        for best in forward.iter_mut().chain(reverse.iter_mut()) {
            best.1 = f64::INFINITY;
        }
        for (i, a) in left.iter().enumerate() {
            for (j, b) in right.iter().enumerate() {
                if cancel() {
                    return Err(LocalError::Cancelled);
                }
                let distance: f64 = a
                    .descriptor
                    .0
                    .iter()
                    .zip(b.descriptor.0.iter())
                    .map(|(x, y)| (x - y) * (x - y))
                    .sum();
                let nearest = right[forward[i].2].position;
                if (b.position[0] - nearest[0]).powi(2) + (b.position[1] - nearest[1]).powi(2) > radius {
                    forward[i].1 = forward[i].1.min(distance);
                }
                let nearest = left[reverse[j].2].position;
                if (a.position[0] - nearest[0]).powi(2) + (a.position[1] - nearest[1]).powi(2) > radius {
                    reverse[j].1 = reverse[j].1.min(distance);
                }
            }
        }
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(left.len().min(right.len()))
        .map_err(|_| LocalError::Budget)?;
    for (i, &(distance, second, j)) in forward.iter().enumerate() {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if (distinct_radius_squared.is_some() && !second.is_finite())
            || distance > policy.max_squared_distance
            || distance >= second * policy.squared_ratio
        {
            continue;
        }
        let (back, next, index) = reverse[j];
        if index == i
            && back < next * policy.squared_ratio
            && (distinct_radius_squared.is_none() || next.is_finite())
        {
            output.push(crate::geometry::Correspondence {
                source: left[i].position,
                target: right[j].position,
            });
        }
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(output)
}

fn batched(
    left: &[GradientFeature],
    right: &[GradientFeature],
    policy: GradientMatchPolicy,
    radius: Option<f64>,
    cancel: impl Fn() -> bool,
) -> Result<Vec<crate::geometry::Correspondence>, LocalError> {
    super::match_gradients_selected(left, right, policy, radius, cancel)
}

fn bank(count: usize, seed: u64, sparse: bool) -> Vec<GradientFeature> {
    let mut state = seed;
    (0..count)
        .map(|i| {
            let mut values = [0.; 128];
            for (k, v) in values.iter_mut().enumerate() {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *v = if sparse && k / 8 != i % 16 {
                    0.
                } else {
                    (state % 1000 + 1) as f64
                };
            }
            let norm = values.iter().map(|v| v * v).sum::<f64>().sqrt();
            for v in &mut values {
                *v /= norm;
            }
            GradientFeature {
                position: [(i % 7) as f64 * 4., (i / 7) as f64 * 3.],
                descriptor: GradientDescriptor(values),
            }
        })
        .collect()
}
fn policy() -> GradientMatchPolicy {
    GradientMatchPolicy {
        max_comparisons: 10_000_000,
        max_squared_distance: 2.,
        squared_ratio: 0.8,
    }
}
fn coordinates(
    result: Result<Vec<crate::geometry::Correspondence>, LocalError>,
) -> Result<Vec<([f64; 2], [f64; 2])>, String> {
    result
        .map(|v| v.into_iter().map(|p| (p.source, p.target)).collect())
        .map_err(|e| format!("{e:?}"))
}
#[test]
fn exact_matching_agrees_with_exhaustive_positive_negative_ties_and_spatial_chains() {
    let mut positive = 0;
    for seed in 1..=12 {
        for sparse in [false, true] {
            let left = bank(35, seed, sparse);
            let foreign = bank(29, seed + 100, sparse);
            let mut related = left.clone();
            for f in &mut related {
                f.position[0] += 10.;
            }
            // Duplicate descriptors at both identical and separated locations;
            // a 0/3/6 chain is not a transitive radius cluster.
            related.push(related[0]);
            let mut duplicate = related[0];
            duplicate.position[0] += 3.;
            related.push(duplicate);
            duplicate.position[0] += 3.;
            related.push(duplicate);
            let mut reverse = related.clone();
            reverse.reverse();
            for right in [&foreign, &related, &reverse] {
                for radius in [None, Some(0.), Some(16.), Some(10000.)] {
                    let expected = coordinates(scalar(&left, right, policy(), radius, &|| false));
                    positive += expected.as_ref().unwrap().len();
                    let actual = coordinates(batched(&left, right, policy(), radius, &|| false));
                    assert_eq!(actual, expected, "seed={seed} sparse={sparse} radius={radius:?}");
                }
            }
        }
    }
    assert!(
        positive > 1000,
        "must exercise accepted matches, not only negatives"
    );
}
#[test]
fn every_cancellation_checkpoint_and_work_refusal_matches_scalar() {
    use std::cell::Cell;
    let left = bank(4, 173, true);
    let right = left.clone();
    for radius in [None, Some(16.)] {
        let count = Cell::new(0);
        scalar(&left, &right, policy(), radius, &|| {
            count.set(count.get() + 1);
            false
        })
        .unwrap();
        for stop in 1..=count.get() + 1 {
            let a = Cell::new(0);
            let b = Cell::new(0);
            let expected = coordinates(scalar(&left, &right, policy(), radius, &|| {
                a.set(a.get() + 1);
                a.get() == stop
            }));
            let actual = coordinates(batched(&left, &right, policy(), radius, &|| {
                b.set(b.get() + 1);
                b.get() == stop
            }));
            assert_eq!(actual, expected);
            assert_eq!(a.get(), b.get());
        }
        let mut p = policy();
        p.max_comparisons = if radius.is_some() { 31 } else { 15 };
        assert_eq!(
            coordinates(batched(&left, &right, p, radius, &|| false)),
            coordinates(scalar(&left, &right, p, radius, &|| false))
        );
    }
}
#[test]
#[ignore = "manual alternating exact matcher timing control"]
fn alternating_matcher_timing() {
    use std::time::Instant;
    for sparse in [false, true] {
        let left = bank(500, 173, sparse);
        let right = bank(500, 591, sparse);
        let expected = coordinates(scalar(&left, &right, policy(), Some(16.), &|| false)).unwrap();
        for fast in [false, true, true, false] {
            let start = Instant::now();
            let got = if fast {
                batched(&left, &right, policy(), Some(16.), &|| false)
            } else {
                scalar(&left, &right, policy(), Some(16.), &|| false)
            };
            assert_eq!(coordinates(got).unwrap(), expected);
            eprintln!(
                "matcher_timing sparse={sparse} batched={fast} seconds={:.9}",
                start.elapsed().as_secs_f64()
            );
        }
    }
}

#[cfg(feature = "decode")]
#[test]
#[ignore = "manual real-photo exact matcher timing and parity"]
fn real_photo_matcher_timing() {
    use crate::{animated::AnimationBudget, decode::decode_selected_frame_bounded, pyramid::PyramidPolicy};
    use rrrah_core::MemoryBudget;
    use rrrah_decode::DecodeRequest;
    use std::time::Instant;
    let paths = std::env::var("RRRAH_GRADIENT_BENCH_FILES").expect("explicit colon-separated corpus paths");
    let budget = MemoryBudget::new(512 * 1024 * 1024);
    let mut banks = Vec::new();
    for path in paths.split(':') {
        let image = decode_selected_frame_bounded(
            &DecodeRequest::new(path),
            AnimationBudget {
                max_frames: 1,
                max_pixels: 6_400_000,
                max_file_bytes: 5_242_880,
            },
            &budget,
            || false,
        )
        .unwrap();
        let view = image.view(|| false).unwrap();
        let features = extract_gradient_pyramid_with_recipe(
            &view,
            PyramidPolicy {
                local: crate::local::LocalPolicy {
                    max_pixels: 6_400_000,
                    max_candidates: 6_400_000,
                    max_features: 500,
                    minimum_corner_score: 0.0001,
                },
                max_levels: 3,
                max_total_pixels: 12_800_000,
                max_total_features: 1500,
            },
            1_536_000,
            GradientCellRecipe::Interpolated,
            || false,
        )
        .unwrap();
        assert!(features.len() >= 100);
        banks.push(features);
    }
    assert_eq!(budget.used(), 0);
    for index in 0..banks.len() {
        let (left, right) = (&banks[0], &banks[index]);
        for radius in [None, Some(64.)] {
            let expected = coordinates(scalar(left, right, policy(), radius, &|| false)).unwrap();
            for fast in [false, true, true, false] {
                let start = Instant::now();
                let got = if fast {
                    batched(left, right, policy(), radius, &|| false)
                } else {
                    scalar(left, right, policy(), radius, &|| false)
                };
                assert_eq!(coordinates(got).unwrap(), expected);
                eprintln!(
                    "real_matcher pair=0:{index} features={}:{} radius={radius:?} batched={fast} matches={} seconds={:.9}",
                    left.len(),
                    right.len(),
                    expected.len(),
                    start.elapsed().as_secs_f64()
                );
            }
        }
    }
}

#[test]
fn four_lane_distances_are_bitwise_scalar_for_every_tail_length() {
    let mut left = bank(20, 173, false);
    let mut right = bank(20, 591, true);
    left.push(GradientFeature {
        position: [0., 0.],
        descriptor: GradientDescriptor(std::array::from_fn(|k| if k == 0 { 1. } else { 1e-300 })),
    });
    right.push(left[left.len() - 1]);
    for a in &left {
        for n in 1..=4 {
            for chunk in right.chunks(n) {
                let got = distances_four(&a.descriptor, chunk);
                for (lane, b) in chunk.iter().enumerate() {
                    let expected: f64 = a
                        .descriptor
                        .0
                        .iter()
                        .zip(&b.descriptor.0)
                        .map(|(a, b)| (a - b) * (a - b))
                        .sum();
                    assert_eq!(got[lane].to_bits(), expected.to_bits());
                }
            }
        }
    }
}
