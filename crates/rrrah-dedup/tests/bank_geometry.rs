#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    bank_geometry::{BankGeometryPolicy, fit_and_validate_point_banks},
    bank_partition::{BankPartitionPolicy, required_bank_partition_work},
    geometry::{Correspondence, GeometryError},
};
use std::cell::Cell;
fn points() -> Vec<Correspondence> {
    [
        [10., 10.],
        [20., 20.],
        [80., 10.],
        [70., 20.],
        [10., 80.],
        [20., 70.],
        [80., 80.],
        [70., 70.],
        [40., 15.],
        [40., 30.],
        [15., 40.],
        [30., 40.],
    ]
    .map(|source| Correspondence {
        source,
        target: source,
    })
    .to_vec()
}
fn partition(n: usize) -> BankPartitionPolicy {
    let w = required_bank_partition_work(n).unwrap();
    BankPartitionPolicy {
        maximum_banks: 3,
        maximum_points: n,
        maximum_separation_checks: w.separation_checks,
        maximum_parent_steps: w.parent_steps,
        minimum_endpoint_separation: 1.,
    }
}
fn policy() -> BankGeometryPolicy {
    BankGeometryPolicy {
        trials: 8,
        seed: 173,
        minimum_fitting_components: 4,
        maximum_projection_checks: 100_000,
        source_tolerance: 0.1,
        target_tolerance: 0.1,
    }
}
#[test]
fn alternatives_remain_but_duplicate_votes_cannot_outweigh_more_components() {
    let base = points();
    let mut alternatives = Vec::new();
    for p in base.iter().step_by(2).take(4) {
        for _ in 0..8 {
            let mut q = *p;
            q.target[0] += 5.;
            alternatives.push(q);
        }
    }
    let n = base.len() + alternatives.len();
    let budget = MemoryBudget::new(1_000_000);
    let mut p = policy();
    p.trials = 4096;
    p.maximum_projection_checks = 1_000_000;
    let e = fit_and_validate_point_banks(
        &[&base, &alternatives],
        (100, 100),
        (100, 100),
        partition(n),
        p,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(e.partition.assignments.len(), n);
    assert_eq!(e.partition.components, 12);
    assert_eq!(e.fitting.unwrap().components, 6);
    let v = e.validation.unwrap();
    assert_eq!((v.joint_components, v.joint_points), (6, 6));
    let credit = budget.used();
    assert!(credit > 0);
    let clone = e.clone();
    drop(e);
    assert_eq!(budget.used(), credit);
    drop(clone);
    assert_eq!(budget.used(), 0);
}
#[test]
fn validation_cannot_select_or_replace_training_model() {
    let base = points();
    let mut changed = base.clone();
    for p in changed.iter_mut().skip(1).step_by(2) {
        p.target[1] += 3.;
    }
    let budget = MemoryBudget::new(100_000);
    let a = fit_and_validate_point_banks(
        &[&base],
        (100, 100),
        (100, 100),
        partition(12),
        policy(),
        &budget,
        || false,
    )
    .unwrap();
    let b = fit_and_validate_point_banks(
        &[&changed],
        (100, 100),
        (100, 100),
        partition(12),
        policy(),
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(a.partition.assignments[..], b.partition.assignments[..]);
    assert_eq!(
        a.fitting.unwrap().transform.matrix.map(|r| r.map(f64::to_bits)),
        b.fitting.unwrap().transform.matrix.map(|r| r.map(f64::to_bits))
    );
    assert_eq!(a.validation.unwrap().joint_components, 6);
    assert_eq!(b.validation.unwrap().joint_components, 0);
    drop(a);
    drop(b);
    assert_eq!(budget.used(), 0);
}
#[test]
fn repeated_heldout_alternatives_are_points_but_only_one_component_vote() {
    let base = points();
    let mut aliases = Vec::new();
    for p in base.iter().skip(1).step_by(2) {
        for _ in 0..5 {
            aliases.push(*p);
        }
    }
    let budget = MemoryBudget::new(100_000);
    let e = fit_and_validate_point_banks(
        &[&base, &aliases],
        (100, 100),
        (100, 100),
        partition(42),
        policy(),
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(e.fitting.unwrap().components, 6);
    let v = e.validation.unwrap();
    assert_eq!(
        (v.points, v.components, v.joint_points, v.joint_components),
        (36, 6, 36, 6)
    );
    drop(e);
    assert_eq!(budget.used(), 0);
}
#[test]
fn budget_and_malformed_suffix_refuse_atomically_and_recover() {
    let base = points();
    let budget = MemoryBudget::new(100_000);
    let e = fit_and_validate_point_banks(
        &[&base],
        (100, 100),
        (100, 100),
        partition(12),
        policy(),
        &budget,
        || false,
    )
    .unwrap();
    let required = e.required_projection_checks;
    let peak = budget.peak();
    drop(e);
    let mut p = policy();
    p.maximum_projection_checks = required - 1;
    assert!(matches!(
        fit_and_validate_point_banks(
            &[&base],
            (100, 100),
            (100, 100),
            partition(12),
            p,
            &budget,
            || false
        ),
        Err(GeometryError::Budget)
    ));
    assert_eq!(budget.used(), 0);
    let short = MemoryBudget::new(peak - 1);
    assert!(matches!(
        fit_and_validate_point_banks(
            &[&base],
            (100, 100),
            (100, 100),
            partition(12),
            policy(),
            &short,
            || false
        ),
        Err(GeometryError::Budget)
    ));
    assert_eq!(short.used(), 0);
    let exact = MemoryBudget::new(peak);
    drop(
        fit_and_validate_point_banks(
            &[&base],
            (100, 100),
            (100, 100),
            partition(12),
            policy(),
            &exact,
            || false,
        )
        .unwrap(),
    );
    assert_eq!(exact.used(), 0);
    let mut bad = base.clone();
    bad[11].target[0] = f64::NAN;
    assert!(matches!(
        fit_and_validate_point_banks(
            &[&bad],
            (100, 100),
            (100, 100),
            partition(12),
            policy(),
            &budget,
            || false
        ),
        Err(GeometryError::Invalid)
    ));
    assert_eq!(budget.used(), 0);
    for field in 0..4 {
        let mut p = policy();
        match field {
            0 => p.seed = 0,
            1 => p.trials = 0,
            2 => p.minimum_fitting_components = 3,
            _ => p.source_tolerance = f64::INFINITY,
        };
        assert!(matches!(
            fit_and_validate_point_banks(
                &[&base],
                (100, 100),
                (100, 100),
                partition(12),
                p,
                &budget,
                || false
            ),
            Err(GeometryError::Invalid)
        ));
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn insufficient_components_do_not_invent_a_model_or_validation() {
    let p = Correspondence {
        source: [1., 1.],
        target: [1., 1.],
    };
    let bank = [p; 20];
    let budget = MemoryBudget::new(100_000);
    let e = fit_and_validate_point_banks(
        &[&bank],
        (100, 100),
        (100, 100),
        partition(20),
        policy(),
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(e.partition.components, 1);
    assert!(e.fitting.is_none() && e.validation.is_none());
    drop(e);
    assert_eq!(budget.used(), 0);
}
#[test]
fn every_one_shot_cancellation_releases_all_credit_and_identical_retry_succeeds() {
    let base = points();
    let budget = MemoryBudget::new(100_000);
    let calls = Cell::new(0);
    let baseline = fit_and_validate_point_banks(
        &[&base],
        (100, 100),
        (100, 100),
        partition(12),
        policy(),
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    let matrix = baseline.fitting.unwrap().transform.matrix;
    drop(baseline);
    let total = calls.get();
    assert!(total > 100);
    for stop in 1..=total {
        calls.set(0);
        let e = fit_and_validate_point_banks(
            &[&base],
            (100, 100),
            (100, 100),
            partition(12),
            policy(),
            &budget,
            || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            },
        );
        assert!(
            matches!(e, Err(GeometryError::Cancelled)),
            "stop={stop}/{total}: {e:?}"
        );
        assert_eq!(budget.used(), 0);
    }
    let retry = fit_and_validate_point_banks(
        &[&base],
        (100, 100),
        (100, 100),
        partition(12),
        policy(),
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(retry.fitting.unwrap().transform.matrix, matrix);
    drop(retry);
    assert_eq!(budget.used(), 0);
}

#[test]
fn different_members_cannot_combine_forward_and_reverse_component_support() {
    let mut base = points();
    for p in &mut base {
        p.target = [p.source[0] * 10., p.source[1] * 0.1];
    }
    let mut reverse_only = base[1];
    base[1].target[1] += 1.; // Forward error1 <=2, reverse error10 >0.5.
    reverse_only.target[0] += 3.; // Forward error3 >2, reverse error0.3 <=0.5.
    let aliases = [reverse_only];
    let mut p = policy();
    p.source_tolerance = 0.5;
    p.target_tolerance = 2.;
    let budget = MemoryBudget::new(100_000);
    let e = fit_and_validate_point_banks(
        &[&base, &aliases],
        (100, 100),
        (1000, 100),
        partition(13),
        p,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(e.fitting.unwrap().components, 6);
    let v = e.validation.unwrap();
    assert_eq!(
        (
            v.forward_components,
            v.reverse_components,
            v.joint_components,
            v.joint_points
        ),
        (6, 6, 5, 5)
    );
    drop(e);
    assert_eq!(budget.used(), 0);
}
