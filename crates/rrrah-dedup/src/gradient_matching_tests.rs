//! Rejected bounded-distance experiment: exact parity but real-photo regressions.
//! Candidate is test-only; production retains the full-distance matcher.
use super::*;

#[test]
#[allow(clippy::cast_precision_loss)] // The generated integers contain at most 53 bits.
fn unrolled_distance_blocks_preserve_scalar_bits_and_partial_lanes() {
    let mut state = 173_u64;
    let mut descriptor = || {
        let mut values = [0.; 128];
        for value in &mut values {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            *value = (state >> 11) as f64 / (1_u64 << 53) as f64;
        }
        let norm = values.iter().map(|v| v * v).sum::<f64>().sqrt();
        for value in &mut values {
            *value /= norm;
        }
        GradientDescriptor(values)
    };
    for _ in 0..256 {
        let a = descriptor();
        let b: [GradientFeature; 4] = std::array::from_fn(|_| GradientFeature {
            position: [0.; 2],
            descriptor: descriptor(),
        });
        for count in 0..=4 {
            let actual = distances_four(&a, &b[..count]);
            for (lane, distance) in actual.iter().enumerate() {
                let expected = if lane < count {
                    a.0.iter()
                        .zip(&b[lane].descriptor.0)
                        .map(|(x, y)| (x - y) * (x - y))
                        .sum::<f64>()
                } else {
                    0.
                };
                assert_eq!(distance.to_bits(), expected.to_bits());
            }
        }
    }
}

#[test]
#[allow(clippy::cast_precision_loss)] // The generated integers contain at most 53 bits.
fn sixteen_descriptor_lanes_preserve_scalar_bits_and_every_tail() {
    let mut state = 173_u64;
    let mut descriptor = || {
        let mut values = [0.; 128];
        for value in &mut values {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            *value = (state >> 11) as f64 / (1_u64 << 53) as f64;
        }
        let norm = values.iter().map(|v| v * v).sum::<f64>().sqrt();
        for value in &mut values {
            *value /= norm;
        }
        GradientDescriptor(values)
    };
    for _ in 0..256 {
        let a = descriptor();
        let b: [GradientFeature; 16] = std::array::from_fn(|_| GradientFeature {
            position: [0.; 2],
            descriptor: descriptor(),
        });
        for count in 0..=16 {
            let actual = distances_sixteen(&a, &b[..count]);
            for (lane, distance) in actual.iter().enumerate() {
                let expected = if lane < count {
                    a.0.iter()
                        .zip(&b[lane].descriptor.0)
                        .map(|(x, y)| (x - y) * (x - y))
                        .sum::<f64>()
                } else {
                    0.
                };
                assert_eq!(distance.to_bits(), expected.to_bits());
            }
        }
    }
}

fn exhaustive(
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
fn bounded_distance_preserves_order_and_strict_ties() {
    let a = bank(30, 173, false);
    let b = bank(30, 591, true);
    for x in &a {
        for y in &b {
            let distance: f64 = x
                .descriptor
                .0
                .iter()
                .zip(&y.descriptor.0)
                .map(|(a, b)| (a - b) * (a - b))
                .sum();
            for bound in [
                0.,
                distance / 2.,
                distance,
                f64::from_bits(distance.to_bits() + 1),
                f64::INFINITY,
            ] {
                let got = bounded_descriptor_distance(&x.descriptor, &y.descriptor, bound);
                if distance <= bound {
                    assert_eq!(got.unwrap().to_bits(), distance.to_bits());
                } else {
                    assert!(got.is_none());
                }
            }
        }
    }
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
                    let expected = coordinates(exhaustive(&left, right, policy(), radius, &|| false));
                    positive += expected.as_ref().unwrap().len();
                    let actual = coordinates(candidate(&left, right, policy(), radius, &|| false));
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
fn every_cancellation_checkpoint_and_work_refusal_matches_exhaustive() {
    use std::cell::Cell;
    let left = bank(4, 173, true);
    let right = left.clone();
    for radius in [None, Some(16.)] {
        let count = Cell::new(0);
        exhaustive(&left, &right, policy(), radius, &|| {
            count.set(count.get() + 1);
            false
        })
        .unwrap();
        for stop in 1..=count.get() + 1 {
            let a = Cell::new(0);
            let b = Cell::new(0);
            let expected = coordinates(exhaustive(&left, &right, policy(), radius, &|| {
                a.set(a.get() + 1);
                a.get() == stop
            }));
            let actual = coordinates(candidate(&left, &right, policy(), radius, &|| {
                b.set(b.get() + 1);
                b.get() == stop
            }));
            assert_eq!(actual, expected);
            assert_eq!(a.get(), b.get());
        }
        let mut p = policy();
        p.max_comparisons = if radius.is_some() { 31 } else { 15 };
        assert_eq!(
            coordinates(candidate(&left, &right, p, radius, &|| false)),
            coordinates(exhaustive(&left, &right, p, radius, &|| false))
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
        let expected = coordinates(exhaustive(&left, &right, policy(), Some(16.), &|| false)).unwrap();
        for fast in [false, true, true, false] {
            let start = Instant::now();
            let got = if fast {
                candidate(&left, &right, policy(), Some(16.), &|| false)
            } else {
                exhaustive(&left, &right, policy(), Some(16.), &|| false)
            };
            assert_eq!(coordinates(got).unwrap(), expected);
            eprintln!(
                "matcher_timing sparse={sparse} bounded={fast} seconds={:.9}",
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
            let expected = coordinates(exhaustive(left, right, policy(), radius, &|| false)).unwrap();
            for fast in [false, true, true, false] {
                let start = Instant::now();
                let got = if fast {
                    candidate(left, right, policy(), radius, &|| false)
                } else {
                    exhaustive(left, right, policy(), radius, &|| false)
                };
                assert_eq!(coordinates(got).unwrap(), expected);
                eprintln!(
                    "real_matcher pair=0:{index} features={}:{} radius={radius:?} bounded={fast} matches={} seconds={:.9}",
                    left.len(),
                    right.len(),
                    expected.len(),
                    start.elapsed().as_secs_f64()
                );
            }
        }
    }
}

// Validated finite descriptors yield nonnegative terms. Strictly exceeding
// both relevant best/runner-up distances cannot affect either table. Keep the
// original left-to-right summation order, including equality/tie behavior.
fn bounded_descriptor_distance(a: &GradientDescriptor, b: &GradientDescriptor, bound: f64) -> Option<f64> {
    let mut sum = 0.;
    for (left, right) in a.0.chunks_exact(8).zip(b.0.chunks_exact(8)) {
        for (&x, &y) in left.iter().zip(right) {
            sum += (x - y) * (x - y);
        }
        if sum > bound {
            return None;
        }
    }
    Some(sum)
}

fn candidate(
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
            // Distinct-location matching discards first-pass runner-ups.
            let bound = if distinct_radius_squared.is_some() {
                forward[i].0.max(reverse[j].0)
            } else {
                forward[i].1.max(reverse[j].1)
            };
            let Some(distance) = bounded_descriptor_distance(&a.descriptor, &b.descriptor, bound) else {
                continue;
            };
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
                let nearest = right[forward[i].2].position;
                let eligible_forward =
                    (b.position[0] - nearest[0]).powi(2) + (b.position[1] - nearest[1]).powi(2) > radius;
                let nearest = left[reverse[j].2].position;
                let eligible_reverse =
                    (a.position[0] - nearest[0]).powi(2) + (a.position[1] - nearest[1]).powi(2) > radius;
                if !eligible_forward && !eligible_reverse {
                    continue;
                }
                let bound = match (eligible_forward, eligible_reverse) {
                    (true, true) => forward[i].1.max(reverse[j].1),
                    (true, false) => forward[i].1,
                    (false, true) => reverse[j].1,
                    (false, false) => unreachable!(),
                };
                let Some(distance) = bounded_descriptor_distance(&a.descriptor, &b.descriptor, bound) else {
                    continue;
                };
                if eligible_forward {
                    forward[i].1 = forward[i].1.min(distance);
                }
                if eligible_reverse {
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

#[test]
fn production_mutual_competitors_match_exhaustive_at_every_checkpoint() {
    use std::cell::Cell;
    for sparse in [false, true] {
        for sizes in [(0, 4), (1, 1), (3, 7), (8, 5), (17, 19)] {
            let left = bank(sizes.0, 173, sparse);
            let mut right = bank(sizes.1, 591, sparse);
            if !left.is_empty() && right.len() > 1 {
                right[0] = left[0].clone();
                right[1] = left[0].clone();
                right[1].position[0] += 3.;
            }
            for radius in [None, Some(0.), Some(4.), Some(64.)] {
                for threshold in [0., 0.05, 0.5, 4.] {
                    let mut p = policy();
                    p.max_squared_distance = threshold;
                    let calls = Cell::new(0usize);
                    let expected = coordinates(exhaustive(&left, &right, p, radius, || {
                        calls.set(calls.get() + 1);
                        false
                    }));
                    let count = calls.get();
                    for stop in 0..=count {
                        let a = Cell::new(0usize);
                        let b = Cell::new(0usize);
                        let before = coordinates(exhaustive(&left, &right, p, radius, || {
                            a.set(a.get() + 1);
                            stop != 0 && a.get() == stop
                        }));
                        let after = coordinates(match_gradients_selected(&left, &right, p, radius, || {
                            b.set(b.get() + 1);
                            stop != 0 && b.get() == stop
                        }));
                        assert_eq!(
                            before, after,
                            "sizes={sizes:?} sparse={sparse} radius={radius:?} threshold={threshold} stop={stop}"
                        );
                        assert_eq!(a.get(), b.get());
                        if stop == 0 {
                            assert_eq!(after, expected);
                        }
                    }
                }
            }
        }
    }
}
