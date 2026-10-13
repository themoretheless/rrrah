#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{local::LocalError, pair_schedule::CompletePairSchedule};
use std::{cell::Cell, collections::BTreeSet};
#[test]
fn every_proposal_subset_keeps_all_pairs_including_descriptorless_files() {
    let ids = [u64::MAX, 0, 11, 3];
    let sorted = [0, 3, 11, u64::MAX];
    let all: Vec<_> = (0..4)
        .flat_map(|i| (i + 1..4).map(move |j| (sorted[i], sorted[j])))
        .collect();
    for mask in 0..64 {
        let mut hints: Vec<_> = all
            .iter()
            .enumerate()
            .filter_map(|(i, p)| ((mask >> i) & 1 == 1).then_some(*p))
            .collect();
        hints.reverse();
        hints.extend(hints.clone());
        let budget = MemoryBudget::new(4096);
        let schedule = CompletePairSchedule::new(&ids, &hints, 4, 6, 12, &budget, || false).unwrap();
        let pairs = schedule.pairs(|| false).collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(pairs.len(), 6);
        assert_eq!(schedule.total_pairs(), 6);
        assert_eq!(
            pairs.iter().map(|p| (p.left, p.right)).collect::<BTreeSet<_>>(),
            all.iter().copied().collect()
        );
        let priority: Vec<_> = pairs
            .iter()
            .filter(|p| p.proposed)
            .map(|p| (p.left, p.right))
            .collect();
        let expected: BTreeSet<_> = hints.iter().copied().collect();
        assert_eq!(priority, expected.iter().copied().collect::<Vec<_>>());
        assert_eq!(schedule.proposed_pairs(), expected.len());
        assert!(pairs[..priority.len()].iter().all(|p| p.proposed));
        assert!(pairs[priority.len()..].iter().all(|p| !p.proposed));
        drop(schedule);
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn invalid_ids_and_hints_refuse_without_retaining_memory() {
    for (ids, hints) in [
        (vec![1, 1], vec![]),
        (vec![1, 2], vec![(2, 1)]),
        (vec![1, 2], vec![(1, 1)]),
        (vec![1, 2], vec![(1, 3)]),
        (vec![], vec![(1, 2)]),
    ] {
        let budget = MemoryBudget::new(4096);
        assert!(matches!(
            CompletePairSchedule::new(&ids, &hints, 3, 3, 3, &budget, || false),
            Err(LocalError::Invalid)
        ));
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn full_pair_count_and_input_limits_preflight_before_allocation() {
    for (files, pairs, hints) in [(2, 3, 1), (3, 2, 1), (3, 3, 0)] {
        let budget = MemoryBudget::new(4096);
        assert!(matches!(
            CompletePairSchedule::new(&[1, 2, 3], &[(1, 2)], files, pairs, hints, &budget, || false),
            Err(LocalError::Budget)
        ));
        assert_eq!((budget.used(), budget.peak()), (0, 0));
    }
    let budget = MemoryBudget::new(23);
    assert!(matches!(
        CompletePairSchedule::new(&[1, 2, 3], &[], 3, 3, 0, &budget, || false),
        Err(LocalError::Budget)
    ));
    assert_eq!(budget.used(), 0);
    for ids in [&[][..], &[u64::MAX][..]] {
        let budget = MemoryBudget::new(8);
        let schedule = CompletePairSchedule::new(ids, &[], 1, 0, 0, &budget, || false).unwrap();
        assert_eq!(schedule.total_pairs(), 0);
        assert!(schedule.pairs(|| false).next().is_none());
        drop(schedule);
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn constructor_and_enumeration_cancel_at_every_checkpoint_then_retry() {
    let budget = MemoryBudget::new(4096);
    let calls = Cell::new(0);
    fn build(budget: &MemoryBudget, cancel: &dyn Fn() -> bool) -> Result<CompletePairSchedule, LocalError> {
        CompletePairSchedule::new(&[3, 1, 2], &[(1, 2), (2, 3)], 3, 3, 2, budget, cancel)
    }
    let count = || {
        calls.set(calls.get() + 1);
        false
    };
    let schedule = build(&budget, &count as &dyn Fn() -> bool).unwrap();
    drop(schedule);
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let cancel = || {
            seen.set(seen.get() + 1);
            seen.get() == stop
        };
        assert!(matches!(
            build(&budget, &cancel as &dyn Fn() -> bool),
            Err(LocalError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    let schedule = build(&budget, &(|| false) as &dyn Fn() -> bool).unwrap();
    let baseline = budget.used();
    let calls = Cell::new(0);
    assert_eq!(
        schedule
            .pairs(|| {
                calls.set(calls.get() + 1);
                false
            })
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .len(),
        3
    );
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let mut iterator = schedule.pairs(|| {
            seen.set(seen.get() + 1);
            seen.get() == stop
        });
        let mut cancelled = false;
        while let Some(pair) = iterator.next() {
            if matches!(pair, Err(LocalError::Cancelled)) {
                cancelled = true;
                break;
            }
        }
        assert!(cancelled);
        assert!(iterator.next().is_none());
        assert!(iterator.next().is_none());
        assert_eq!(budget.used(), baseline);
    }
    assert_eq!(
        schedule
            .pairs(|| false)
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .len(),
        3
    );
    drop(schedule);
    assert_eq!(budget.used(), 0);
}
#[test]
fn late_memory_refusal_releases_ids_and_owned_inputs_cannot_change_schedule() {
    let budget = MemoryBudget::new(39);
    assert!(matches!(
        CompletePairSchedule::new(&[1, 2, 3], &[(1, 2)], 3, 3, 1, &budget, || false),
        Err(LocalError::Budget)
    ));
    assert_eq!(budget.used(), 0);
    assert_eq!(budget.peak(), 24);
    let budget = MemoryBudget::new(40);
    let mut ids = vec![3, 2, 1];
    let mut hints = vec![(1, 2)];
    let schedule = CompletePairSchedule::new(&ids, &hints, 3, 3, 1, &budget, || false).unwrap();
    assert_eq!(budget.used(), 40);
    ids.fill(99);
    hints.fill((99, 100));
    drop((ids, hints));
    let actual: Vec<_> = schedule
        .pairs(|| false)
        .map(|p| {
            let p = p.unwrap();
            (p.left, p.right, p.proposed)
        })
        .collect();
    assert_eq!(actual, vec![(1, 2, true), (1, 3, false), (2, 3, false)]);
    drop(schedule);
    assert_eq!(budget.used(), 0);
}
