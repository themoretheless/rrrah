use rrrah_dedup::{
    geometry::GeometryError,
    proposals::{ProposalDomain, ProposalPolicy, ScoredProposal, select_scored_proposals},
};
use std::cell::Cell;
const DOMAIN: ProposalDomain = ProposalDomain {
    input: [100, 80],
    original: [200, 240],
};
const POLICY: ProposalPolicy = ProposalPolicy {
    max_proposals: 100,
    max_ranking_checks: 10000,
    max_separation_checks: 10000,
    minimum_separation: 2.0,
};
fn sample() -> Vec<ScoredProposal> {
    (0..32)
        .map(|i| ScoredProposal {
            source: [f64::from(i % 8) * 8., f64::from(i / 8) * 9.],
            target: [f64::from(i % 8) * 7., f64::from(i / 8) * 10.],
            confidence: f64::from(i % 5) / 4.,
        })
        .collect()
}

#[test]
fn crop_center_offsets_use_full_domains_and_reject_points_outside_the_crop() {
    use rrrah_dedup::proposals::{RegionProposalDomain, select_scored_region_proposals};
    let left = RegionProposalDomain {
        domain: ProposalDomain {
            input: [8, 8],
            original: [11, 7],
        },
        origin: [3, 2],
        full: [17, 11],
    };
    let right = RegionProposalDomain {
        domain: ProposalDomain {
            input: [16, 8],
            original: [13, 9],
        },
        origin: [21, 17],
        full: [40, 30],
    };
    let proposals = [
        ScoredProposal {
            source: [1., 1.],
            target: [2., 1.],
            confidence: 1.,
        },
        ScoredProposal {
            source: [6., 6.],
            target: [13., 6.],
            confidence: 0.5,
        },
        // These map into the full image, but outside the declared inference crop.
        ScoredProposal {
            source: [-1., 2.],
            target: [8., 4.],
            confidence: 1.,
        },
        ScoredProposal {
            source: [3., 2.],
            target: [17., 4.],
            confidence: 1.,
        },
    ];
    let selected = select_scored_region_proposals(&proposals, left, right, POLICY, || false).unwrap();
    assert_eq!(selected.len(), 2);
    assert_eq!(
        selected[0].source,
        [3. + 1.5 * 11. / 8. - 0.5, 2. + 1.5 * 7. / 8. - 0.5]
    );
    assert_eq!(
        selected[0].target,
        [21. + 2.5 * 13. / 16. - 0.5, 17. + 1.5 * 9. / 8. - 0.5]
    );
    assert_eq!(
        selected[1].source,
        [3. + 6.5 * 11. / 8. - 0.5, 2. + 6.5 * 7. / 8. - 0.5]
    );
    for invalid in [
        RegionProposalDomain {
            origin: [u32::MAX, 0],
            ..left
        },
        RegionProposalDomain {
            full: [13, 11],
            ..left
        },
        RegionProposalDomain {
            domain: ProposalDomain {
                input: [0, 8],
                ..left.domain
            },
            ..left
        },
    ] {
        assert!(matches!(
            select_scored_region_proposals(&proposals, invalid, right, POLICY, || false),
            Err(GeometryError::Invalid)
        ));
    }
}

#[cfg(feature = "raster")]
#[test]
fn managed_crop_selection_cancellation_and_last_owner_keep_credit() {
    use rrrah_core::MemoryBudget;
    use rrrah_dedup::proposals::{RegionProposalDomain, select_scored_region_proposals_managed};
    let domain = RegionProposalDomain {
        domain: DOMAIN,
        origin: [30, 40],
        full: [300, 300],
    };
    let points = sample();
    let budget = MemoryBudget::new(8192);
    let calls = Cell::new(0);
    let selected = select_scored_region_proposals_managed(&points, domain, domain, POLICY, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let clone = selected.clone();
    drop(selected);
    assert_eq!(budget.used(), clone.capacity_bytes());
    drop(clone);
    assert_eq!(budget.used(), 0);
    for at in 1..=calls.get() {
        let current = Cell::new(0);
        assert!(matches!(
            select_scored_region_proposals_managed(&points, domain, domain, POLICY, &budget, || {
                current.set(current.get() + 1);
                current.get() == at
            }),
            Err(GeometryError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn heap_selection_matches_stable_reference_with_ties_boundaries_and_both_side_collisions() {
    let mut points = sample();
    points.extend([
        ScoredProposal {
            source: [0., 0.],
            target: [90., 70.],
            confidence: 1.,
        },
        ScoredProposal {
            source: [90., 70.],
            target: [0., 0.],
            confidence: 1.,
        },
        ScoredProposal {
            source: [-1., 10.],
            target: [10., 10.],
            confidence: 1.,
        },
        ScoredProposal {
            source: [10., 10.],
            target: [100., 10.],
            confidence: 1.,
        },
    ]);
    let mut order: Vec<_> = (0..points.len()).collect();
    order.sort_by(|a, b| points[*b].confidence.total_cmp(&points[*a].confidence));
    let mut expected: Vec<[[f64; 2]; 2]> = Vec::new();
    for i in order {
        let p = points[i];
        let pair = [p.source, p.target].map(|p| [(p[0] + 0.5) * 2. - 0.5, (p[1] + 0.5) * 3. - 0.5]);
        if pair
            .iter()
            .any(|p| p[0] < 0. || p[0] >= 200. || p[1] < 0. || p[1] >= 240.)
        {
            continue;
        }
        if expected.iter().all(|q| {
            (0..2)
                .all(|side| (pair[side][0] - q[side][0]).powi(2) + (pair[side][1] - q[side][1]).powi(2) > 4.)
        }) {
            expected.push(pair);
        }
    }
    let actual = select_scored_proposals(&points, DOMAIN, DOMAIN, POLICY, || false).unwrap();
    assert_eq!(
        actual.iter().map(|p| [p.source, p.target]).collect::<Vec<_>>(),
        expected
    );
}
#[test]
fn every_checkpoint_including_heap_and_final_observes_one_shot_cancellation() {
    let points = sample();
    let calls = Cell::new(0);
    let baseline = select_scored_proposals(&points, DOMAIN, DOMAIN, POLICY, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(baseline.len(), 32);
    for at in 1..=calls.get() {
        let current = Cell::new(0);
        assert!(
            matches!(
                select_scored_proposals(&points, DOMAIN, DOMAIN, POLICY, || {
                    current.set(current.get() + 1);
                    current.get() == at
                }),
                Err(GeometryError::Cancelled)
            ),
            "checkpoint {at}"
        );
        assert_eq!(current.get(), at);
    }
}
#[test]
fn malformed_inputs_and_each_work_limit_refuse_without_a_partial_result() {
    let points = sample();
    for policy in [
        ProposalPolicy {
            max_proposals: 31,
            ..POLICY
        },
        ProposalPolicy {
            max_ranking_checks: 0,
            ..POLICY
        },
        ProposalPolicy {
            max_separation_checks: 0,
            ..POLICY
        },
    ] {
        assert!(matches!(
            select_scored_proposals(&points, DOMAIN, DOMAIN, policy, || false),
            Err(GeometryError::Budget)
        ));
    }
    for bad in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        let mut points = points.clone();
        points[31].confidence = bad;
        assert!(matches!(
            select_scored_proposals(&points, DOMAIN, DOMAIN, POLICY, || false),
            Err(GeometryError::Invalid)
        ));
    }
    for bad in [f64::NAN, f64::INFINITY] {
        let mut points = points.clone();
        points[31].source[0] = bad;
        assert!(matches!(
            select_scored_proposals(&points, DOMAIN, DOMAIN, POLICY, || false),
            Err(GeometryError::Invalid)
        ));
    }
    assert!(matches!(
        select_scored_proposals(
            &points,
            ProposalDomain {
                input: [0, 1],
                ..DOMAIN
            },
            DOMAIN,
            POLICY,
            || false
        ),
        Err(GeometryError::Invalid)
    ));
    assert!(matches!(
        select_scored_proposals(
            &points,
            DOMAIN,
            DOMAIN,
            ProposalPolicy {
                minimum_separation: f64::MAX,
                ..POLICY
            },
            || false
        ),
        Err(GeometryError::Invalid)
    ));
    assert!(
        select_scored_proposals(
            &[],
            DOMAIN,
            DOMAIN,
            ProposalPolicy {
                max_proposals: 0,
                max_ranking_checks: 0,
                max_separation_checks: 0,
                ..POLICY
            },
            || false
        )
        .unwrap()
        .is_empty()
    );
}

#[cfg(feature = "raster")]
#[test]
fn managed_payload_credit_survives_clones_and_all_cancellation_paths_release_it() {
    use rrrah_dedup::{geometry::Correspondence, proposals::select_scored_proposals_managed};
    let points = sample();
    let output = (points.len() * std::mem::size_of::<Correspondence>()) as u64;
    let scratch = (points.len() * std::mem::size_of::<usize>()) as u64;
    let insufficient = rrrah_core::MemoryBudget::new(output + scratch - 1);
    assert!(matches!(
        select_scored_proposals_managed(&points, DOMAIN, DOMAIN, POLICY, &insufficient, || false),
        Err(GeometryError::Budget)
    ));
    assert_eq!(insufficient.used(), 0);
    let budget = rrrah_core::MemoryBudget::new(output + scratch);
    let calls = Cell::new(0);
    let selected = select_scored_proposals_managed(&points, DOMAIN, DOMAIN, POLICY, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(budget.used(), output);
    assert_eq!(budget.peak(), output + scratch);
    let clone = selected.clone();
    drop(selected);
    assert_eq!(budget.used(), output);
    drop(clone);
    assert_eq!(budget.used(), 0);
    for at in 1..=calls.get() {
        let current = Cell::new(0);
        assert!(matches!(
            select_scored_proposals_managed(&points, DOMAIN, DOMAIN, POLICY, &budget, || {
                current.set(current.get() + 1);
                current.get() == at
            }),
            Err(GeometryError::Cancelled)
        ));
        assert_eq!(current.get(), at);
        assert_eq!(budget.used(), 0);
    }
}
