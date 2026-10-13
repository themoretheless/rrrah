#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    bank_partition::*,
    geometry::{Correspondence, GeometryError},
};
use std::cell::Cell;
fn point(source: [f64; 2], target: [f64; 2]) -> Correspondence {
    Correspondence { source, target }
}
fn policy(banks: usize, n: usize) -> BankPartitionPolicy {
    let w = required_bank_partition_work(n).unwrap();
    BankPartitionPolicy {
        maximum_banks: banks,
        maximum_points: n,
        maximum_separation_checks: w.separation_checks,
        maximum_parent_steps: w.parent_steps,
        minimum_endpoint_separation: 1.1,
    }
}
#[test]
fn cross_bank_transitive_chains_keep_all_conflicting_alternatives_and_origins() {
    let a = [point([10., 10.], [10., 10.]), point([50., 50.], [50., 50.])];
    let b = [point([11., 10.], [70., 70.]), point([80., 80.], [71., 70.])];
    let c = [a[0]];
    let budget = MemoryBudget::new(4096);
    let out = partition_point_banks(
        &[&a, &b, &c],
        (100, 100),
        (100, 100),
        policy(3, 5),
        &budget,
        || false,
    )
    .unwrap();
    let ids: Vec<_> = out
        .assignments
        .iter()
        .map(|p| (p.bank, p.point, p.component, p.partition))
        .collect();
    assert_eq!(
        ids,
        vec![
            (0, 0, 0, BankPointPartition::Fitting),
            (0, 1, 1, BankPointPartition::Validation),
            (1, 0, 0, BankPointPartition::Fitting),
            (1, 1, 0, BankPointPartition::Fitting),
            (2, 0, 0, BankPointPartition::Fitting)
        ]
    );
    assert_eq!(
        (out.components, out.fitting_points, out.validation_points),
        (2, 4, 1)
    );
    assert!(out.parent_steps <= out.work.parent_steps);
    drop(out);
    assert_eq!(budget.used(), 0);
}
#[test]
fn a_single_component_is_not_split_to_manufacture_validation_and_empty_ids_survive() {
    let a = [point([10., 10.], [20., 20.]); 5];
    let budget = MemoryBudget::new(4096);
    let out = partition_point_banks(
        &[&[], &a, &[]],
        (100, 100),
        (100, 100),
        policy(3, 5),
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(
        (
            out.banks,
            out.components,
            out.fitting_points,
            out.validation_points
        ),
        (3, 1, 5, 0)
    );
    assert!(out.assignments.iter().enumerate().all(|(i, p)| p.bank == 1
        && p.point == i
        && p.component == 0
        && p.partition == BankPointPartition::Fitting));
    drop(out);
    assert_eq!(budget.used(), 0);
    let zero = MemoryBudget::new(0);
    let empty =
        partition_point_banks(&[&[], &[]], (100, 100), (100, 100), policy(2, 0), &zero, || false).unwrap();
    assert_eq!(
        (empty.banks, empty.components, empty.assignments.len()),
        (2, 0, 0)
    );
    assert_eq!(zero.used(), 0);
    assert_eq!(
        required_bank_partition_work(0).unwrap(),
        BankPartitionWork {
            separation_checks: 0,
            parent_steps: 0,
            managed_payload_bytes: 0
        }
    );
    assert!(matches!(
        required_bank_partition_work(usize::MAX),
        Err(GeometryError::Budget)
    ));
}
#[test]
fn connected_components_match_independent_graph_flood_fill_on_seeded_banks() {
    let budget = MemoryBudget::new(16384);
    let mut state = 173u64;
    for _ in 0..300 {
        let mut points = Vec::new();
        for _ in 0..9 {
            let mut xy = [0.; 4];
            for v in &mut xy {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                *v = ((state >> 32) % 8) as f64;
            }
            points.push(point([xy[0], xy[1]], [xy[2], xy[3]]));
        }
        let n = points.len();
        let mut labels = vec![usize::MAX; n];
        let mut component = 0;
        let adjacent = |a: Correspondence, b: Correspondence| {
            [(a.source, b.source), (a.target, b.target)]
                .iter()
                .any(|(a, b)| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) <= 1.1f64.powi(2))
        };
        for i in 0..n {
            if labels[i] != usize::MAX {
                continue;
            }
            labels[i] = component;
            let mut frontier = vec![i];
            while let Some(j) = frontier.pop() {
                for k in 0..n {
                    if labels[k] == usize::MAX && adjacent(points[j], points[k]) {
                        labels[k] = component;
                        frontier.push(k);
                    }
                }
            }
            component += 1;
        }
        let out = partition_point_banks(
            &[&points[..3], &points[3..6], &points[6..]],
            (100, 100),
            (100, 100),
            policy(3, n),
            &budget,
            || false,
        )
        .unwrap();
        assert_eq!(out.components, component);
        for (i, a) in out.assignments.iter().enumerate() {
            assert_eq!((a.bank, a.point, a.component), (i / 3, i % 3, labels[i]));
            assert_eq!(
                a.partition,
                if labels[i] % 2 == 0 {
                    BankPointPartition::Fitting
                } else {
                    BankPointPartition::Validation
                }
            );
        }
        for i in 0..n {
            for j in 0..i {
                if out.assignments[i].partition != out.assignments[j].partition {
                    assert!(!adjacent(points[i], points[j]));
                }
            }
        }
        assert!(out.parent_steps <= out.work.parent_steps);
        drop(out);
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn all_work_and_payload_are_prepaid_and_output_credit_follows_last_owner() {
    let a = [point([10., 10.], [10., 10.]), point([30., 30.], [30., 30.])];
    let banks = [&a[..], &a[..]];
    let p = policy(2, 4);
    let work = required_bank_partition_work(4).unwrap();
    let exact = MemoryBudget::new(work.managed_payload_bytes);
    let out = partition_point_banks(&banks, (100, 100), (100, 100), p, &exact, || false).unwrap();
    assert_eq!(exact.peak(), work.managed_payload_bytes);
    assert_eq!(
        exact.used(),
        (4 * std::mem::size_of::<BankPointAssignment>()) as u64
    );
    let clone = out.clone();
    drop(out);
    assert!(exact.used() > 0);
    drop(clone);
    assert_eq!(exact.used(), 0);
    let short = MemoryBudget::new(work.managed_payload_bytes - 1);
    assert!(matches!(
        partition_point_banks(&banks, (100, 100), (100, 100), p, &short, || false),
        Err(GeometryError::Budget)
    ));
    assert_eq!(short.used(), 0);
    for field in 0..4 {
        let mut p = p;
        match field {
            0 => p.maximum_banks -= 1,
            1 => p.maximum_points -= 1,
            2 => p.maximum_separation_checks -= 1,
            _ => p.maximum_parent_steps -= 1,
        };
        assert!(matches!(
            partition_point_banks(&banks, (100, 100), (100, 100), p, &exact, || false),
            Err(GeometryError::Budget)
        ));
        assert_eq!(exact.used(), 0);
    }
    assert!(partition_point_banks(&banks, (100, 100), (100, 100), p, &exact, || false).is_ok());
    assert_eq!(exact.used(), 0);
}
#[test]
fn malformed_suffixes_and_policy_release_every_reservation() {
    let budget = MemoryBudget::new(4096);
    let base = [point([10., 10.], [10., 10.]); 5];
    for side in [false, true] {
        for axis in 0..2 {
            for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1., 100.] {
                let mut a = base;
                if side {
                    a[4].source[axis] = invalid
                } else {
                    a[4].target[axis] = invalid
                };
                assert!(matches!(
                    partition_point_banks(&[&a], (100, 100), (100, 100), policy(1, 5), &budget, || false),
                    Err(GeometryError::Invalid)
                ));
                assert_eq!(budget.used(), 0);
            }
        }
    }
    for invalid in [0., -1., f64::NAN, f64::INFINITY] {
        let mut p = policy(1, 5);
        p.minimum_endpoint_separation = invalid;
        assert!(matches!(
            partition_point_banks(&[&base], (100, 100), (100, 100), p, &budget, || false),
            Err(GeometryError::Invalid)
        ));
        assert_eq!(budget.used(), 0);
    }
    assert!(matches!(
        partition_point_banks(&[&base], (0, 100), (100, 100), policy(1, 5), &budget, || false),
        Err(GeometryError::Invalid)
    ));
}
#[test]
fn every_one_shot_cancel_checkpoint_discards_assignments_and_retry_is_identical() {
    let a = [point([10., 10.], [10., 10.]); 8];
    let budget = MemoryBudget::new(16384);
    let checks = Cell::new(0);
    let baseline = partition_point_banks(&[&a], (100, 100), (100, 100), policy(1, 8), &budget, || {
        checks.set(checks.get() + 1);
        false
    })
    .unwrap();
    let retained = budget.used();
    assert!(checks.get() > 100);
    for checkpoint in 1..=checks.get() {
        let calls = Cell::new(0);
        assert!(matches!(
            partition_point_banks(&[&a], (100, 100), (100, 100), policy(1, 8), &budget, || {
                calls.set(calls.get() + 1);
                calls.get() == checkpoint
            }),
            Err(GeometryError::Cancelled)
        ));
        assert_eq!(budget.used(), retained);
    }
    let retry =
        partition_point_banks(&[&a], (100, 100), (100, 100), policy(1, 8), &budget, || false).unwrap();
    assert_eq!(&*retry.assignments, &*baseline.assignments);
    assert_eq!(retry.parent_steps, baseline.parent_steps);
    drop(retry);
    drop(baseline);
    assert_eq!(budget.used(), 0);
}
