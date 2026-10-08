#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    correspondence_union::union_correspondences_distinct_managed as union, geometry::Correspondence,
    local::LocalError,
};
fn point(x: f64, y: f64) -> Correspondence {
    Correspondence {
        source: [x, 0.],
        target: [y, 0.],
    }
}
fn rows(points: &[Correspondence]) -> Vec<[f64; 4]> {
    points
        .iter()
        .map(|p| [p.source[0], p.source[1], p.target[0], p.target[1]])
        .collect()
}
#[test]
fn union_matches_independent_greedy_oracle_including_boundaries_duplicates_and_order() {
    let mut inputs = (0..40)
        .map(|i| point(f64::from((i * 7) % 19), f64::from((i * 11) % 23)))
        .collect::<Vec<_>>();
    inputs.extend([point(-0., 0.), point(0., -0.), point(2., 2.)]);
    for radius in [0., 1., 2., 3.] {
        let mut sorted = rows(&inputs);
        for row in &mut sorted {
            for v in row {
                if *v == 0. {
                    *v = 0.;
                }
            }
        }
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut expected: Vec<[f64; 4]> = Vec::new();
        for row in sorted {
            if expected.iter().all(|p| {
                (row[0] - p[0]).hypot(row[1] - p[1]) > radius && (row[2] - p[2]).hypot(row[3] - p[3]) > radius
            }) {
                expected.push(row);
            }
        }
        for reversed in [false, true] {
            let mut order = inputs.clone();
            if reversed {
                order.reverse();
            }
            let budget = MemoryBudget::new(43 * 32);
            let output = union(&[&order[..20], &order[20..]], radius, 43, 903, &budget, || false).unwrap();
            assert_eq!(rows(&output), expected);
            assert_eq!(budget.peak(), 43 * 32);
            let clone = output.clone();
            drop(output);
            assert_eq!(budget.used(), 43 * 32);
            drop(clone);
            assert_eq!(budget.used(), 0);
        }
    }
}
#[test]
fn checked_admission_invalid_inputs_every_cancellation_and_retry() {
    let input = [point(0., 0.), point(2., 2.), point(5., 5.)];
    let b = MemoryBudget::new(96);
    assert!(matches!(
        union(&[&input], 2., 2, 3, &b, || false),
        Err(LocalError::Budget)
    ));
    assert_eq!(b.peak(), 0);
    assert!(matches!(
        union(&[&input], 2., 3, 2, &b, || false),
        Err(LocalError::Budget)
    ));
    assert_eq!(b.peak(), 0);
    let denied = MemoryBudget::new(95);
    assert!(matches!(
        union(&[&input], 2., 3, 3, &denied, || false),
        Err(LocalError::Budget)
    ));
    assert_eq!(denied.used(), 0);
    for radius in [-1., f64::NAN, f64::INFINITY, f64::MAX] {
        assert!(matches!(
            union(&[&input], radius, 3, 3, &b, || false),
            Err(LocalError::Invalid)
        ));
    }
    assert!(matches!(
        union(&[&[point(f64::NAN, 0.)]], 2., 1, 0, &b, || false),
        Err(LocalError::Invalid)
    ));
    let calls = std::cell::Cell::new(0);
    let output = union(&[&input], 2., 3, 3, &b, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(rows(&output), vec![[0., 0., 0., 0.], [5., 0., 5., 0.]]);
    drop(output);
    for stop in 1..=calls.get() {
        let count = std::cell::Cell::new(0);
        assert!(matches!(
            union(&[&input], 2., 3, 3, &b, || {
                count.set(count.get() + 1);
                count.get() == stop
            }),
            Err(LocalError::Cancelled)
        ));
        assert_eq!(b.used(), 0);
    }
    drop(union(&[&input], 2., 3, 3, &b, || false).unwrap());
    assert_eq!(b.used(), 0);
    let zero = MemoryBudget::new(0);
    assert!(union(&[], 0., 0, 0, &zero, || false).unwrap().is_empty());
    assert_eq!(zero.used(), 0);
}
