#![cfg(feature = "decode")]
use rrrah_dedup::{
    geometry::{Correspondence, ProjectiveEvidence, ProjectiveTransform},
    local::LocalError,
    local_scan::ProjectivePhotometricFileEvidence,
    managed_evidence::manage_projective_file_evidence,
};
fn evidence() -> ProjectivePhotometricFileEvidence {
    let mut points = Vec::with_capacity(7);
    points.push(Correspondence {
        source: [1., 2.],
        target: [3., 4.],
    });
    let mut inliers = Vec::with_capacity(5);
    inliers.push(0);
    ProjectivePhotometricFileEvidence {
        correspondences: points,
        geometry: Some(ProjectiveEvidence {
            transform: ProjectiveTransform {
                matrix: [[1., 0., 2.], [0., 1., 2.], [0., 0., 1.]],
            },
            inliers,
            squared_error: 0.,
            hypotheses: 1,
        }),
        registered_transform: None,
        pixels: None,
        fit_failure: None,
        unfitted: None,
        candidate: false,
    }
}
#[test]
fn evidence_adoption_preserves_allocations_capacity_and_independent_last_owners() {
    let source = evidence();
    let pp = source.correspondences.as_ptr();
    let ip = source.geometry.as_ref().unwrap().inliers.as_ptr();
    let expected = (source.correspondences.capacity() * std::mem::size_of::<Correspondence>()
        + source.geometry.as_ref().unwrap().inliers.capacity() * std::mem::size_of::<usize>())
        as u64;
    let budget = rrrah_core::MemoryBudget::new(expected);
    let result = manage_projective_file_evidence(source, &budget, || false).unwrap();
    assert_eq!(result.correspondences.as_ptr(), pp);
    assert_eq!(result.geometry.as_ref().unwrap().inliers.as_ptr(), ip);
    assert_eq!(budget.used(), expected);
    assert_eq!(budget.peak(), expected);
    let clone = result.clone();
    let points = clone.correspondences.clone();
    let inliers = clone.geometry.as_ref().unwrap().inliers.clone();
    drop(result);
    drop(clone);
    assert_eq!(budget.used(), expected);
    drop(points);
    assert_eq!(budget.used(), 5 * std::mem::size_of::<usize>() as u64);
    drop(inliers);
    assert_eq!(budget.used(), 0);
    let short = rrrah_core::MemoryBudget::new(expected - 1);
    assert!(matches!(
        manage_projective_file_evidence(evidence(), &short, || false),
        Err(LocalError::Budget)
    ));
    assert_eq!(short.used(), 0);
}
#[test]
fn evidence_adoption_cancellation_discards_all_stages_and_handles_absent_geometry() {
    let calls = std::cell::Cell::new(0);
    let budget = rrrah_core::MemoryBudget::new(4096);
    drop(
        manage_projective_file_evidence(evidence(), &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap(),
    );
    let total = calls.get();
    assert_eq!(budget.used(), 0);
    for stop in 1..=total {
        calls.set(0);
        assert!(matches!(
            manage_projective_file_evidence(evidence(), &budget, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(LocalError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    let mut e = evidence();
    e.geometry = None;
    let result = manage_projective_file_evidence(e, &budget, || false).unwrap();
    assert!(result.geometry.is_none());
    assert_eq!(result.correspondences.len(), 1);
    drop(result);
    assert_eq!(budget.used(), 0);
}
