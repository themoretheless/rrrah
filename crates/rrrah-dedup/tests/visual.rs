use rrrah_dedup::{
    pixels::Fingerprint,
    visual::{VisualCandidate, VisualError, VisualIndex},
};

#[test]
fn full_radius_direct_search_cancellation_never_returns_partial_matches() {
    let query = Fingerprint {
        variants: [[0; 4]; 8],
        mean_linear_rgb: [0.5; 3],
        luminance_stddev: 0.2,
    };
    let entries = [0_u64, 1, 3, u64::MAX].map(|word| {
        (
            word,
            Fingerprint {
                variants: [[word; 4]; 8],
                ..query.clone()
            },
        )
    });
    let index = VisualIndex::new(entries.clone(), 4, || false).unwrap();
    for allow in [false, true] {
        let mut expected = entries
            .iter()
            .map(|(id, other)| VisualCandidate {
                id: *id,
                evidence: query.compare(other, allow),
            })
            .collect::<Vec<_>>();
        expected.sort_unstable_by_key(|c| (c.evidence.distance, c.id));
        for checkpoint in 1..=5 {
            let calls = std::cell::Cell::new(0);
            assert_eq!(
                index.search(&query, 256, allow, || {
                    calls.set(calls.get() + 1);
                    calls.get() == checkpoint
                }),
                Err(VisualError::Cancelled)
            );
        }
        assert_eq!(index.search(&query, u32::MAX, allow, || false).unwrap(), expected);
    }
}

#[test]
fn fallible_search_result_caps_are_inconclusive_and_leave_index_unchanged() {
    use rrrah_dedup::{Candidate, HammingIndex, IndexError};
    let values = [0_u64, 0, 1, 3, u64::MAX];
    let mut index = HammingIndex::new();
    for (id, hash) in values.into_iter().enumerate() {
        index
            .try_insert_with_cancel(u64::try_from(id).unwrap(), hash, 5, || false)
            .unwrap();
    }
    for radius in [0, 1, 32, 64, u32::MAX] {
        let mut expected = values
            .into_iter()
            .enumerate()
            .filter_map(|(id, hash)| {
                let distance = hash.count_ones();
                (distance <= radius).then_some(Candidate {
                    id: u64::try_from(id).unwrap(),
                    distance,
                })
            })
            .collect::<Vec<_>>();
        expected.sort_unstable_by_key(|c| (c.distance, c.id));
        assert_eq!(
            index.try_search_with_cancel(0, radius, expected.len() - 1, || false),
            Err(IndexError::Budget)
        );
        assert_eq!(
            index
                .try_search_with_cancel(0, radius, expected.len(), || false)
                .unwrap(),
            expected
        );
    }
    assert!(
        index
            .try_search_with_cancel(2, 0, 0, || false)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        index.try_search_with_cancel(0, 64, 5, || true),
        Err(IndexError::Cancelled)
    );
    assert_eq!(index.try_search_with_cancel(0, 64, 5, || false).unwrap().len(), 5);
}

#[test]
fn fallible_insert_enforces_total_ids_without_partial_mutation() {
    use rrrah_dedup::{HammingIndex, IndexError};
    let mut index = HammingIndex::new();
    assert_eq!(
        index.try_insert_with_cancel(1, 0, 0, || false),
        Err(IndexError::Budget)
    );
    assert!(index.search(0, 64).is_empty());
    index.try_insert_with_cancel(1, 0, 2, || false).unwrap();
    index.try_insert_with_cancel(2, 0, 2, || false).unwrap();
    let before = index.search(0, 64);
    for hash in [0, 1, u64::MAX] {
        assert_eq!(
            index.try_insert_with_cancel(3, hash, 2, || false),
            Err(IndexError::Budget)
        );
        assert_eq!(index.search(0, 64), before);
    }
    index.try_insert_with_cancel(2, 0, 2, || false).unwrap();
    assert_eq!(index.search(0, 64), before);
    assert_eq!(
        index.try_insert_with_cancel(3, 1, 3, || true),
        Err(IndexError::Cancelled)
    );
    assert_eq!(index.search(0, 64), before);
    index.try_insert_with_cancel(3, 1, 3, || false).unwrap();
    assert_eq!(index.search(0, 64).len(), 3);
    // Fail after traversal and reservation, before publishing a new child.
    let checks = std::cell::Cell::new(0);
    let before = index.search(0, 64);
    assert_eq!(
        index.try_insert_with_cancel(4, 2, 4, || {
            checks.set(checks.get() + 1);
            checks.get() >= 4
        }),
        Err(IndexError::Cancelled)
    );
    assert_eq!(index.search(0, 64), before);
    index.try_insert_with_cancel(4, 2, 4, || false).unwrap();
    assert_eq!(index.search(0, 64).len(), 4);
}

#[test]
fn complete_recipe_index_matches_exhaustive_oracle() {
    let mut seed = 42_u64;
    let mut entries = Vec::new();
    for id in 0..200_u64 {
        let mut variants = [[0; 4]; 8];
        for variant in &mut variants {
            for hash in variant {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                *hash = seed;
            }
        }
        entries.push((
            id,
            Fingerprint {
                variants,
                mean_linear_rgb: [0.0; 3],
                luminance_stddev: 0.5,
            },
        ));
    }
    entries.push((u64::MAX, entries[0].1.clone()));
    let index = VisualIndex::new(entries.clone(), entries.len(), || false).unwrap();
    for (_, query) in entries.iter().step_by(19) {
        for allow in [false, true] {
            for radius in [0, 3, 4, 32, 64, 127, 128, 200, 255, 256, u32::MAX] {
                let mut expected: Vec<_> = entries
                    .iter()
                    .filter_map(|(id, other)| {
                        let evidence = query.compare(other, allow);
                        (evidence.distance <= radius).then_some(VisualCandidate { id: *id, evidence })
                    })
                    .collect();
                expected.sort_unstable_by_key(|v| (v.evidence.distance, v.id));
                assert_eq!(index.search(query, radius, allow, || false).unwrap(), expected);
            }
        }
    }
}

#[test]
fn duplicate_ids_budgets_and_cancel_do_not_return_partial_indexes() {
    let fp = Fingerprint {
        variants: [[0; 4]; 8],
        mean_linear_rgb: [0.0; 3],
        luminance_stddev: 0.0,
    };
    assert_eq!(
        VisualIndex::new([(1, fp.clone()), (1, fp.clone())], 2, || false).unwrap_err(),
        VisualError::DuplicateId
    );
    assert_eq!(
        VisualIndex::new([(1, fp.clone())], 0, || false).unwrap_err(),
        VisualError::Budget
    );
    assert_eq!(
        VisualIndex::new([(1, fp.clone())], 1, || true).unwrap_err(),
        VisualError::Cancelled
    );
    let index = VisualIndex::new([(1, fp.clone())], 1, || false).unwrap();
    assert_eq!(index.search(&fp, 0, true, || true), Err(VisualError::Cancelled));
    assert!(
        !index.search(&fp, 0, true, || false).unwrap()[0]
            .evidence
            .informative
    );
}

#[test]
fn cancellation_inside_large_collision_bucket_is_observed() {
    use rrrah_dedup::{HammingIndex, SearchCancelled};
    use std::cell::Cell;
    let mut index = HammingIndex::new();
    for id in 0..1000 {
        index.insert(id, 0);
    }
    let checks = Cell::new(0);
    let result = index.search_with_cancel(0, 0, || {
        checks.set(checks.get() + 1);
        checks.get() >= 20
    });
    assert_eq!(result, Err(SearchCancelled));
    assert_eq!(checks.get(), 20);
}

#[test]
fn cancelled_insertions_preserve_collision_buckets_and_deep_trees() {
    use rrrah_dedup::{HammingIndex, SearchCancelled};
    use std::cell::Cell;
    let mut collisions = HammingIndex::new();
    for id in 0..1000 {
        collisions.insert(id, 0);
    }
    let mut chain = HammingIndex::new();
    chain.insert(100, 0);
    for bit in 0..64 {
        chain.insert(bit, 1_u64 << bit);
    }
    for (index, hash) in [(&mut collisions, 0), (&mut chain, 1_u64 << 63)] {
        let observed = Cell::new(0);
        index
            .insert_with_cancel(9_999, hash, || {
                observed.set(observed.get() + 1);
                false
            })
            .unwrap();
        let before = index.search(hash, 64);
        for checkpoint in 1..=observed.get() {
            let checks = Cell::new(0);
            assert_eq!(
                index.insert_with_cancel(10_000, hash, || {
                    checks.set(checks.get() + 1);
                    checks.get() == checkpoint
                }),
                Err(SearchCancelled)
            );
            assert_eq!(index.search(hash, 64), before);
        }
        index.insert_with_cancel(10_000, hash, || false).unwrap();
        assert_eq!(index.search(hash, 64).len(), before.len() + 1);
    }
}

#[test]
fn dense_adaptive_search_keeps_oracle_and_cancellation() {
    let query = Fingerprint {
        variants: [[0; 4]; 8],
        mean_linear_rgb: [0.5; 3],
        luminance_stddev: 0.2,
    };
    let entries = (0..24)
        .map(|id| {
            (
                id,
                Fingerprint {
                    variants: [[if id % 2 == 0 { 0 } else { u64::MAX }; 4]; 8],
                    ..query.clone()
                },
            )
        })
        .collect::<Vec<_>>();
    let index = VisualIndex::new(entries.clone(), entries.len(), || false).unwrap();
    for allow in [false, true] {
        for radius in [0, 32, 128, 255] {
            let mut expected = entries
                .iter()
                .filter_map(|(id, other)| {
                    let evidence = query.compare(other, allow);
                    (evidence.distance <= radius).then_some(VisualCandidate { id: *id, evidence })
                })
                .collect::<Vec<_>>();
            expected.sort_unstable_by_key(|c| (c.evidence.distance, c.id));
            let calls = std::cell::Cell::new(0);
            assert_eq!(
                index
                    .search(&query, radius, allow, || {
                        calls.set(calls.get() + 1);
                        false
                    })
                    .unwrap(),
                expected
            );
            for checkpoint in 1..=calls.get() {
                let current = std::cell::Cell::new(0);
                assert_eq!(
                    index.search(&query, radius, allow, || {
                        current.set(current.get() + 1);
                        current.get() == checkpoint
                    }),
                    Err(VisualError::Cancelled)
                );
            }
            assert_eq!(index.search(&query, radius, allow, || false).unwrap(), expected);
        }
    }
}
