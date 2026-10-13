use super::*;
use rrrah_core::MemoryBudget;
use std::cell::Cell;

fn bank(count: usize, mut seed: u64) -> Vec<GradientFeature> {
    (0..count)
        .map(|i| {
            let mut values = [0.; 128];
            for v in &mut values {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                *v = (seed % 1000 + 1) as f64;
            }
            let norm = values.iter().map(|v| v * v).sum::<f64>().sqrt();
            for v in &mut values {
                *v /= norm;
            }
            GradientFeature {
                position: [(i % 10) as f64 * 8., (i / 10) as f64 * 8.],
                descriptor: GradientDescriptor(values),
            }
        })
        .collect()
}
fn policy() -> GradientMatchPolicy {
    GradientMatchPolicy {
        max_comparisons: 1_000_000,
        max_squared_distance: 2.,
        squared_ratio: 0.64,
    }
}
fn coordinates(points: &[crate::geometry::Correspondence]) -> Vec<([f64; 2], [f64; 2])> {
    points.iter().map(|p| (p.source, p.target)).collect()
}
fn baseline_bytes(left: usize, right: usize) -> u64 {
    ((left + right) * std::mem::size_of::<(f64, f64, usize)>()
        + left.min(right) * std::mem::size_of::<crate::geometry::Correspondence>()) as u64
}

#[test]
fn cached_matching_keeps_positive_negative_and_distinct_ties_under_both_budget_paths() {
    let left = bank(80, 173);
    let mut positive = left.clone();
    for f in &mut positive {
        f.position[0] += 10.;
    }
    let negative = bank(83, 991);
    let mut ties = positive.clone();
    for f in &positive[..10] {
        let mut duplicate = *f;
        duplicate.position[1] += 1000.;
        ties.push(duplicate);
    }
    for right in [&positive, &negative, &ties] {
        let expected = match_gradients_distinct_locations(&left, right, policy(), 2., || false).unwrap();
        if right.len() == 80 {
            assert_eq!(expected.len(), 80);
        }
        if right.len() == 90 {
            assert_eq!(expected.len(), 70);
        }
        let base = baseline_bytes(left.len(), right.len());
        for extra in [0, 1_000_000] {
            let b = MemoryBudget::new(base + extra);
            let credit = b.try_reserve(base).unwrap();
            let actual =
                match_gradients_distinct_locations_prepaid(&left, right, policy(), 2., &b, || false).unwrap();
            assert_eq!(coordinates(&actual), coordinates(&expected));
            assert_eq!(b.used(), base);
            if extra == 0 {
                assert_eq!(b.peak(), base);
            } else {
                assert!(b.peak() >= base + (left.len() * right.len() * 8) as u64);
            }
            drop(actual);
            drop(credit);
            assert_eq!(b.used(), 0);
        }
    }
}

#[test]
fn managed_cache_releases_before_output_and_respects_ancestor_pressure() {
    let left = bank(80, 173);
    let right = left.clone();
    let base = baseline_bytes(80, 80);
    for parent_limit in [base, 1_000_000] {
        let parent = MemoryBudget::new(parent_limit);
        let b = parent.child(1_000_000);
        let out =
            match_gradients_distinct_locations_managed(&left, &right, policy(), 2., &b, || false).unwrap();
        assert_eq!(out.len(), 80);
        let retained = 80 * std::mem::size_of::<crate::geometry::Correspondence>() as u64;
        assert_eq!(b.used(), retained);
        assert_eq!(parent.used(), retained);
        if parent_limit == base {
            assert_eq!(parent.peak(), base);
        } else {
            assert!(parent.peak() >= base + 80 * 80 * 8);
        }
        let copy = out.clone();
        drop(out);
        assert_eq!(parent.used(), retained);
        drop(copy);
        assert_eq!(b.used(), 0);
        assert_eq!(parent.used(), 0);
    }
}

#[test]
fn cached_checkpoints_and_cancellation_match_recomputation_without_credit_leaks() {
    let left = bank(80, 173);
    let right = left.clone();
    let count = Cell::new(0);
    let expected = match_gradients_distinct_locations(&left, &right, policy(), 2., || {
        count.set(count.get() + 1);
        false
    })
    .unwrap();
    let checkpoints = count.get();
    let base = baseline_bytes(80, 80);
    for stop in [0, 1, checkpoints / 4, checkpoints / 2, checkpoints] {
        let before = Cell::new(0);
        let baseline = match_gradients_distinct_locations(&left, &right, policy(), 2., || {
            before.set(before.get() + 1);
            stop != 0 && before.get() == stop
        });
        let b = MemoryBudget::new(1_000_000);
        let credit = b.try_reserve(base).unwrap();
        let after = Cell::new(0);
        let cached = match_gradients_distinct_locations_prepaid(&left, &right, policy(), 2., &b, || {
            after.set(after.get() + 1);
            stop != 0 && after.get() == stop
        });
        assert_eq!(before.get(), after.get());
        if stop == 0 {
            assert_eq!(coordinates(&cached.unwrap()), coordinates(&expected));
            assert!(baseline.is_ok());
        } else {
            assert!(matches!(baseline, Err(LocalError::Cancelled)));
            assert!(matches!(cached, Err(LocalError::Cancelled)));
        }
        assert_eq!(b.used(), base);
        drop(credit);
        assert_eq!(b.used(), 0);
    }
}

#[test]
fn malformed_suffix_and_work_refusal_do_not_allocate_optional_cache() {
    let left = bank(80, 173);
    let mut right = left.clone();
    right[79].descriptor.0[127] = f64::NAN;
    let b = MemoryBudget::new(1_000_000);
    let base = baseline_bytes(80, 80);
    assert!(matches!(
        match_gradients_distinct_locations_managed(&left, &right, policy(), 2., &b, || false),
        Err(LocalError::Invalid)
    ));
    assert_eq!(b.used(), 0);
    assert_eq!(b.peak(), base);
    let b = MemoryBudget::new(1_000_000);
    let mut p = policy();
    p.max_comparisons = 80 * 80 * 2 - 1;
    assert!(matches!(
        match_gradients_distinct_locations_managed(&left, &left, p, 2., &b, || false),
        Err(LocalError::Budget)
    ));
    assert_eq!((b.used(), b.peak()), (0, 0));
}

#[test]
fn cache_capacity_and_overflow_keep_exhaustive_fallback_available() {
    assert_eq!(distance_cache_pairs(64, 64, true), Some(4096));
    assert_eq!(distance_cache_pairs(1024, 2048, true), Some(2_097_152));
    assert_eq!(distance_cache_pairs(1025, 2048, true), None);
    assert_eq!(distance_cache_pairs(usize::MAX, 2, true), None);
    assert_eq!(distance_cache_pairs(64, 64, false), None);
    assert_eq!(distance_cache_pairs(63, 64, true), None);
}
