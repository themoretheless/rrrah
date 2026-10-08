#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    geometry::Correspondence,
    mesh_refine::{LandmarkAdmissionPolicy, SeedRefinement, TranslationEvidence, admit_mesh_landmarks},
    rank_region::RankRegionError,
};
use std::cell::Cell;
fn original() -> Vec<Correspondence> {
    [[0., 0.], [10., 0.], [0., 10.]]
        .into_iter()
        .map(|p| Correspondence { source: p, target: p })
        .collect()
}
fn evidence(p: [f64; 2], eligible: bool) -> SeedRefinement {
    SeedRefinement::Scored(TranslationEvidence {
        initial_source: p,
        source: p,
        offset: [0, 0],
        training_pairs: 100,
        training_agreeing: 100,
        validation_pairs: 100,
        validation_agreeing: 100,
        eligible_proposal: eligible,
    })
}
fn policy() -> LandmarkAdmissionPolicy {
    LandmarkAdmissionPolicy {
        maximum_points: 5,
        maximum_proposals: 3,
        maximum_distance_tests: 10,
        minimum_separation: 2.,
    }
}
#[test]
fn original_points_are_preserved_and_duplicate_proposals_are_explicit() {
    let original = original();
    let seeds = [[4, 4], [1, 1], [8, 8]];
    let proposals = [
        evidence([4., 4.], true),
        evidence([1., 1.], true),
        evidence([8., 8.], false),
    ];
    let budget = MemoryBudget::new(1000);
    let result = admit_mesh_landmarks(
        &original,
        &seeds,
        &proposals,
        (64, 64),
        (64, 64),
        policy(),
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(result.eligible_proposals, 2);
    assert_eq!(result.duplicate_proposals, 1);
    assert_eq!(result.points.len(), 4);
    for (a, b) in result.points[..3].iter().zip(&original) {
        assert_eq!(a.source, b.source);
        assert_eq!(a.target, b.target);
    }
    assert_eq!(result.points[3].target, [4., 4.]);
    let bytes = budget.used();
    assert!(bytes > 0);
    let clone = result.clone();
    drop(result);
    assert_eq!(budget.used(), bytes);
    drop(clone);
    assert_eq!(budget.used(), 0);
}
#[test]
fn every_cancel_and_budget_refusal_return_no_partial_landmarks() {
    let original = original();
    let seeds = [[4, 4], [1, 1], [8, 8]];
    let proposals = [
        evidence([4., 4.], true),
        evidence([1., 1.], true),
        evidence([8., 8.], false),
    ];
    let budget = MemoryBudget::new(1000);
    let calls = Cell::new(0);
    let result = admit_mesh_landmarks(
        &original,
        &seeds,
        &proposals,
        (64, 64),
        (64, 64),
        policy(),
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    drop(result);
    let total = calls.get();
    for checkpoint in 1..=total {
        let c = Cell::new(0);
        assert!(matches!(
            admit_mesh_landmarks(
                &original,
                &seeds,
                &proposals,
                (64, 64),
                (64, 64),
                policy(),
                &budget,
                || {
                    c.set(c.get() + 1);
                    c.get() == checkpoint
                }
            ),
            Err(RankRegionError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    let mut p = policy();
    p.maximum_distance_tests = 9;
    assert!(matches!(
        admit_mesh_landmarks(
            &original,
            &seeds,
            &proposals,
            (64, 64),
            (64, 64),
            p,
            &budget,
            || false
        ),
        Err(RankRegionError::Budget)
    ));
    assert!(matches!(
        admit_mesh_landmarks(
            &original,
            &seeds,
            &proposals,
            (64, 64),
            (64, 64),
            policy(),
            &MemoryBudget::new(1),
            || false
        ),
        Err(RankRegionError::Budget)
    ));
}
#[test]
fn aliases_and_malformed_scored_evidence_are_invalid() {
    let mut p = original();
    p[1] = p[0];
    let budget = MemoryBudget::new(1000);
    assert!(matches!(
        admit_mesh_landmarks(&p, &[], &[], (64, 64), (64, 64), policy(), &budget, || false),
        Err(RankRegionError::Invalid)
    ));
    let mut bad = TranslationEvidence {
        initial_source: [4., 4.],
        source: [5., 4.],
        offset: [0, 0],
        training_pairs: 100,
        training_agreeing: 100,
        validation_pairs: 100,
        validation_agreeing: 100,
        eligible_proposal: true,
    };
    assert!(matches!(
        admit_mesh_landmarks(
            &original(),
            &[[4, 4]],
            &[SeedRefinement::Scored(bad)],
            (64, 64),
            (64, 64),
            policy(),
            &budget,
            || false
        ),
        Err(RankRegionError::Invalid)
    ));
    bad.source = [4., 4.];
    bad.training_agreeing = 101;
    assert!(matches!(
        admit_mesh_landmarks(
            &original(),
            &[[4, 4]],
            &[SeedRefinement::Scored(bad)],
            (64, 64),
            (64, 64),
            policy(),
            &budget,
            || false
        ),
        Err(RankRegionError::Invalid)
    ));
    assert_eq!(budget.used(), 0);
}
