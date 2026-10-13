#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    gradient::{GradientCellRecipe, GradientDescriptor},
    gradient_index::GradientDescriptorIndex,
    local::LocalError,
};
fn descriptor(seed: u64) -> GradientDescriptor {
    let mut state = seed;
    let mut d = [0.; 128];
    for v in &mut d {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        *v = ((state >> 32) as f64) + 1.;
    }
    let norm = d.iter().map(|v| v * v).sum::<f64>().sqrt();
    for v in &mut d {
        *v /= norm;
    }
    GradientDescriptor(d)
}
#[test]
fn retrieval_equals_exhaustive_including_boundaries_and_collisions() {
    let mut entries = (1..90).map(|i| (i, descriptor(i))).collect::<Vec<_>>();
    entries.push((u64::MAX, entries[0].1));
    for recipe in [GradientCellRecipe::Fixed, GradientCellRecipe::Interpolated] {
        let budget = MemoryBudget::new(1_000_000);
        let index = GradientDescriptorIndex::new(&entries, recipe, 90, &budget, || false).unwrap();
        for query in [entries[0].1, descriptor(901), descriptor(12345)] {
            let mut radii = vec![0., 0.001, 0.1, 0.5, 4.];
            for (_, d) in &entries {
                let boundary = (0..128).fold(0., |sum, i| {
                    let delta = query.0[i] - d.0[i];
                    sum + delta * delta
                });
                // Exact inclusive cutoff and its immediately adjacent floats.
                radii.extend([boundary.next_down().max(0.), boundary, boundary.next_up()]);
            }
            for radius in radii {
                let actual = index.search(&query, recipe, radius, 90, 90, || false).unwrap();
                let mut expected = entries
                    .iter()
                    .filter_map(|(id, d)| {
                        let distance = (0..128).fold(0., |sum, i| {
                            let delta = query.0[i] - d.0[i];
                            sum + delta * delta
                        });
                        (distance <= radius).then_some((*id, distance))
                    })
                    .collect::<Vec<_>>();
                expected.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
                assert_eq!(
                    actual
                        .iter()
                        .map(|v| (v.id, v.squared_distance))
                        .collect::<Vec<_>>(),
                    expected
                );
            }
        }
        assert!(budget.used() > 0);
        drop(index);
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn admission_cancel_recipe_and_owned_reservation() {
    let entries = vec![(1, descriptor(1)), (2, descriptor(2))];
    let recipe = GradientCellRecipe::Fixed;
    let budget = MemoryBudget::new(1000);
    assert!(matches!(
        GradientDescriptorIndex::new(&entries, recipe, 1, &budget, || false),
        Err(LocalError::Budget)
    ));
    assert_eq!(budget.peak(), 0);
    let index = GradientDescriptorIndex::new(&entries, recipe, 2, &budget, || false).unwrap();
    assert!(matches!(
        index.search(&entries[0].1, recipe, 4., 1, 2, || false),
        Err(LocalError::Budget)
    ));
    assert!(matches!(
        index.search(&entries[0].1, recipe, 4., 2, 1, || false),
        Err(LocalError::Budget)
    ));
    assert!(matches!(
        index.search(&entries[0].1, GradientCellRecipe::Interpolated, 4., 2, 2, || {
            false
        }),
        Err(LocalError::Invalid)
    ));
    let calls = std::cell::Cell::new(0);
    index
        .search(&entries[0].1, recipe, 4., 2, 2, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    let total = calls.get();
    for stop in [1, total / 2, total] {
        calls.set(0);
        assert!(matches!(
            index.search(&entries[0].1, recipe, 4., 2, 2, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(LocalError::Cancelled)
        ));
    }
    drop(index);
    assert_eq!(budget.used(), 0);
    let duplicate = [entries[0], entries[0]];
    assert!(matches!(
        GradientDescriptorIndex::new(&duplicate, recipe, 2, &budget, || false),
        Err(LocalError::Invalid)
    ));
    assert_eq!(budget.used(), 0);
    let empty = GradientDescriptorIndex::new(&[], recipe, 0, &budget, || false).unwrap();
    calls.set(0);
    assert!(matches!(
        empty.search(&entries[0].1, recipe, 0., 0, 0, || {
            calls.set(calls.get() + 1);
            calls.get() == 2
        }),
        Err(LocalError::Cancelled)
    ));
}

#[test]
fn invalid_vectors_exact_memory_and_build_cancellation() {
    let recipe = GradientCellRecipe::Fixed;
    let entries = [(7, descriptor(7)), (9, descriptor(9))];
    let bytes = (entries.len() * std::mem::size_of::<usize>()) as u64;
    let short = MemoryBudget::new(bytes - 1);
    assert!(matches!(
        GradientDescriptorIndex::new(&entries, recipe, 2, &short, || false),
        Err(LocalError::Budget)
    ));
    assert_eq!(short.used(), 0);
    let budget = MemoryBudget::new(bytes);
    let calls = std::cell::Cell::new(0);
    let index = GradientDescriptorIndex::new(&entries, recipe, 2, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let total = calls.get();
    assert_eq!(budget.used(), bytes);
    drop(index);
    assert_eq!(budget.used(), 0);
    for stop in [1, total / 2, total] {
        calls.set(0);
        assert!(matches!(
            GradientDescriptorIndex::new(&entries, recipe, 2, &budget, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(LocalError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    for value in [f64::NAN, f64::INFINITY, -0.1, 0.] {
        let mut bad = entries;
        bad[0].1.0 = [value; 128];
        assert!(matches!(
            GradientDescriptorIndex::new(&bad, recipe, 2, &budget, || false),
            Err(LocalError::Invalid)
        ));
        assert_eq!(budget.used(), 0);
    }
    let retry = GradientDescriptorIndex::new(&entries, recipe, 2, &budget, || false).unwrap();
    assert_eq!(
        retry
            .search(&entries[0].1, recipe, 0., 2, 2, || false)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn file_proposals_equal_exhaustive_without_ambiguity_loss() {
    use rrrah_dedup::gradient_index::gradient_file_pairs;
    let mut entries = (0..24).map(|i| (i, descriptor(i + 1))).collect::<Vec<_>>();
    entries[21].1 = entries[0].1;
    entries[22].1 = entries[0].1;
    let owners = (0..24).map(|i| i / 3).collect::<Vec<_>>();
    let budget = MemoryBudget::new(10000);
    for recipe in [GradientCellRecipe::Fixed, GradientCellRecipe::Interpolated] {
        for radius in [0., 0.1, 0.5, 4.] {
            let actual =
                gradient_file_pairs(&entries, &owners, recipe, radius, 576, 576, 28, &budget, || false)
                    .unwrap();
            let mut expected = std::collections::BTreeSet::new();
            for (i, a) in entries.iter().enumerate() {
                for (j, b) in entries.iter().enumerate() {
                    let d = (0..128).map(|k| (a.1.0[k] - b.1.0[k]).powi(2)).sum::<f64>();
                    if owners[i] != owners[j] && d <= radius {
                        expected.insert((owners[i].min(owners[j]), owners[i].max(owners[j])));
                    }
                }
            }
            assert_eq!(actual, expected.into_iter().collect::<Vec<_>>());
            assert_eq!(budget.used(), 0);
        }
    }
    assert!(matches!(
        gradient_file_pairs(
            &entries,
            &owners,
            GradientCellRecipe::Fixed,
            4.,
            575,
            576,
            28,
            &budget,
            || false
        ),
        Err(LocalError::Budget)
    ));
    assert!(matches!(
        gradient_file_pairs(
            &entries,
            &owners,
            GradientCellRecipe::Fixed,
            4.,
            576,
            575,
            28,
            &budget,
            || false
        ),
        Err(LocalError::Budget)
    ));
    assert!(matches!(
        gradient_file_pairs(
            &entries,
            &owners,
            GradientCellRecipe::Fixed,
            4.,
            576,
            576,
            27,
            &budget,
            || false
        ),
        Err(LocalError::Budget)
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn file_proposal_cancellation_never_returns_partial_pairs() {
    use rrrah_dedup::gradient_index::gradient_file_pairs;
    let entries = [(1, descriptor(1)), (2, descriptor(1)), (3, descriptor(2))];
    let owners = [7, 8, 9];
    let recipe = GradientCellRecipe::Fixed;
    let budget = MemoryBudget::new(1000);
    let calls = std::cell::Cell::new(0);
    let baseline = gradient_file_pairs(&entries, &owners, recipe, 4., 9, 9, 3, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(baseline, vec![(7, 8), (7, 9), (8, 9)]);
    let checkpoints = calls.get();
    for stop in 1..=checkpoints {
        calls.set(0);
        assert!(matches!(
            gradient_file_pairs(&entries, &owners, recipe, 4., 9, 9, 3, &budget, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(LocalError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
        assert_eq!(
            gradient_file_pairs(&entries, &owners, recipe, 4., 9, 9, 3, &budget, || false).unwrap(),
            baseline
        );
    }
    assert!(matches!(
        gradient_file_pairs(&entries, &owners[..2], recipe, 4., 9, 9, 3, &budget, || { false }),
        Err(LocalError::Invalid)
    ));
    calls.set(0);
    assert!(
        gradient_file_pairs(&[], &[], recipe, 0., 0, 0, 0, &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap()
        .is_empty()
    );
    let empty_checkpoints = calls.get();
    calls.set(0);
    assert!(matches!(
        gradient_file_pairs(&[], &[], recipe, 0., 0, 0, 0, &budget, || {
            calls.set(calls.get() + 1);
            calls.get() == empty_checkpoints
        }),
        Err(LocalError::Cancelled)
    ));
}

#[test]
fn proposal_scratch_and_retained_report_admit_exact_peak() {
    use rrrah_dedup::gradient_index::gradient_file_pair_report;
    let entries = [(1, descriptor(1)), (2, descriptor(1)), (3, descriptor(2))];
    let owners = [7, 8, 9];
    let recipe = GradientCellRecipe::Fixed;
    let budget = MemoryBudget::new(10000);
    let report =
        gradient_file_pair_report(&entries, &owners, recipe, 4., 9, 9, 3, &budget, || false).unwrap();
    assert_eq!(report.pairs, vec![(7, 8), (7, 9), (8, 9)]);
    assert!(budget.used() >= (report.pairs.capacity() * std::mem::size_of::<(u64, u64)>()) as u64);
    let peak = budget.peak();
    drop(report);
    assert_eq!(budget.used(), 0);
    let exact = MemoryBudget::new(peak);
    let retry = gradient_file_pair_report(&entries, &owners, recipe, 4., 9, 9, 3, &exact, || false).unwrap();
    assert_eq!(retry.pairs.len(), 3);
    drop(retry);
    assert_eq!(exact.used(), 0);
    let short = MemoryBudget::new(peak - 1);
    assert!(matches!(
        gradient_file_pair_report(&entries, &owners, recipe, 4., 9, 9, 3, &short, || false),
        Err(LocalError::Budget)
    ));
    assert_eq!(short.used(), 0);
}

#[test]
fn repeated_descriptor_hits_retain_one_pair_with_bounded_memory() {
    use rrrah_dedup::gradient_index::gradient_file_pair_report;
    use std::cell::Cell;
    let entries = (0..32).map(|id| (id, descriptor(77))).collect::<Vec<_>>();
    let owners = (0..32)
        .map(|i| if i % 2 == 0 { 0 } else { u64::MAX })
        .collect::<Vec<_>>();
    for recipe in [GradientCellRecipe::Fixed, GradientCellRecipe::Interpolated] {
        let budget = MemoryBudget::new(2048);
        let calls = Cell::new(0);
        let report = gradient_file_pair_report(&entries, &owners, recipe, 0., 1024, 1024, 1, &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
        assert_eq!(report.pairs, [(0, u64::MAX)]);
        assert_eq!(report.descriptor_hits, 1024);
        let peak = budget.peak();
        assert!(peak <= 2048);
        drop(report);
        assert_eq!(budget.used(), 0);
        for limit in [peak, peak - 1] {
            let budget = MemoryBudget::new(limit);
            let result =
                gradient_file_pair_report(&entries, &owners, recipe, 0., 1024, 1024, 1, &budget, || false);
            assert_eq!(result.is_ok(), limit == peak);
            drop(result);
            assert_eq!(budget.used(), 0);
        }
        for stop in [1, calls.get() / 2, calls.get()] {
            let seen = Cell::new(0);
            let budget = MemoryBudget::new(2048);
            assert!(matches!(
                gradient_file_pair_report(&entries, &owners, recipe, 0., 1024, 1024, 1, &budget, || {
                    seen.set(seen.get() + 1);
                    seen.get() == stop
                }),
                Err(LocalError::Cancelled)
            ));
            assert_eq!(budget.used(), 0);
        }
        let budget = MemoryBudget::new(2048);
        assert!(matches!(
            gradient_file_pair_report(&entries, &owners, recipe, 0., 1024, 1024, 0, &budget, || false),
            Err(LocalError::Budget)
        ));
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn actual_work_admits_sparse_pairs_without_quadratic_prepayment() {
    use rrrah_dedup::gradient_index::{
        gradient_file_pair_report, gradient_file_pair_report_with_actual_work,
    };
    use std::cell::Cell;
    let mut entries = (0..32).map(|id| (id, descriptor(id + 173))).collect::<Vec<_>>();
    entries.push((32, entries[0].1));
    entries.push((33, entries[3].1));
    let owners = (0..34).collect::<Vec<_>>();
    let recipe = GradientCellRecipe::Fixed;
    let denied = MemoryBudget::new(65536);
    assert!(matches!(
        gradient_file_pair_report(&entries, &owners, recipe, 0., 38, 38, 2, &denied, || false),
        Err(LocalError::Budget)
    ));
    assert_eq!(denied.peak(), 0);
    let a = MemoryBudget::new(65536);
    let b = MemoryBudget::new(65536);
    let ca = Cell::new(0);
    let cb = Cell::new(0);
    let old = gradient_file_pair_report(&entries, &owners, recipe, 0., 34 * 34, 38, 2, &a, || {
        ca.set(ca.get() + 1);
        false
    })
    .unwrap();
    let new = gradient_file_pair_report_with_actual_work(
        &entries,
        &owners,
        recipe,
        0.,
        38,
        38,
        2,
        &b,
        None,
        || {
            cb.set(cb.get() + 1);
            false
        },
    )
    .unwrap();
    assert_eq!(new.pairs, vec![(0, 32), (3, 33)]);
    assert_eq!(old.pairs, new.pairs);
    assert_eq!(old.descriptor_hits, 38);
    assert_eq!(new.descriptor_hits, 38);
    assert_eq!(old.descriptor_comparisons, 38);
    assert_eq!(new.descriptor_comparisons, 38);
    assert_eq!(ca.get(), cb.get());
    assert_eq!(a.peak(), b.peak());
    drop(old);
    drop(new);
    assert_eq!(a.used(), 0);
    assert_eq!(b.used(), 0);
}
#[test]
fn actual_work_counts_rejected_coordinate_candidates_and_never_returns_partial_pairs() {
    use rrrah_dedup::gradient_index::gradient_file_pair_report_with_actual_work as query;
    let entries = (1..=3)
        .map(|dim| {
            let mut d = [0.; 128];
            d[dim] = 1.;
            (dim as u64, GradientDescriptor(d))
        })
        .collect::<Vec<_>>();
    for (comparisons, hits, success) in [(5, 3, true), (4, 3, false), (5, 2, false)] {
        let budget = MemoryBudget::new(4096);
        let result = query(
            &entries,
            &[1, 2, 3],
            GradientCellRecipe::Fixed,
            0.,
            comparisons,
            hits,
            0,
            &budget,
            None,
            || false,
        );
        if success {
            let report = result.unwrap();
            assert_eq!(report.descriptor_comparisons, 5);
            assert_eq!(report.descriptor_hits, 3);
            assert!(report.pairs.is_empty());
            drop(report);
        } else {
            assert!(matches!(result, Err(LocalError::Budget)));
        }
        assert_eq!(budget.used(), 0);
    }
    let budget = MemoryBudget::new(4096);
    let empty = query(
        &[],
        &[],
        GradientCellRecipe::Fixed,
        0.,
        0,
        0,
        0,
        &budget,
        None,
        || false,
    )
    .unwrap();
    assert_eq!(empty.descriptor_comparisons, 0);
    assert_eq!(empty.descriptor_hits, 0);
    drop(empty);
    assert_eq!(budget.used(), 0);
}
#[test]
fn actual_work_preserves_reflected_hits_and_transform_admission() {
    use rrrah_dedup::gradient_index::gradient_file_pair_report_with_actual_work as query;
    let d = descriptor(173);
    let r = rrrah_dedup::gradient::reflect_gradient_descriptor(&d, || false).unwrap();
    let entries = [(1, d), (2, r)];
    for (comparisons, bins, success) in [(4, 256, true), (3, 256, false), (4, 255, false)] {
        let budget = MemoryBudget::new(4096);
        let result = query(
            &entries,
            &[10, 99],
            GradientCellRecipe::Interpolated,
            0.,
            comparisons,
            4,
            1,
            &budget,
            Some(bins),
            || false,
        );
        if success {
            let report = result.unwrap();
            assert_eq!(report.pairs, vec![(10, 99)]);
            assert_eq!(report.descriptor_hits, 4);
            assert_eq!(report.descriptor_comparisons, 4);
            drop(report);
        } else {
            assert!(matches!(result, Err(LocalError::Budget)));
            if bins == 255 {
                assert_eq!(budget.peak(), 0);
            }
        }
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn actual_work_every_cancel_checkpoint_discards_even_previous_pairs() {
    use rrrah_dedup::gradient_index::gradient_file_pair_report_with_actual_work as query;
    use std::cell::Cell;
    let d = descriptor(173);
    let entries = [(1, d), (2, d), (3, descriptor(211))];
    let owners = [7, 8, 9];
    let budget = MemoryBudget::new(4096);
    let count = Cell::new(0);
    let report = query(
        &entries,
        &owners,
        GradientCellRecipe::Fixed,
        0.,
        5,
        5,
        1,
        &budget,
        None,
        || {
            count.set(count.get() + 1);
            false
        },
    )
    .unwrap();
    assert_eq!(report.pairs, vec![(7, 8)]);
    drop(report);
    for stop in 1..=count.get() {
        let seen = Cell::new(0);
        assert!(matches!(
            query(
                &entries,
                &owners,
                GradientCellRecipe::Fixed,
                0.,
                5,
                5,
                1,
                &budget,
                None,
                || {
                    seen.set(seen.get() + 1);
                    seen.get() == stop
                }
            ),
            Err(LocalError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn actual_work_minimum_self_work_and_late_refusal() {
    use rrrah_dedup::gradient_index::gradient_file_pair_report_with_actual_work as query;
    let entries = [(1, descriptor(173)), (2, descriptor(211))];
    for (comparisons, hits) in [(1, 2), (2, 1)] {
        let budget = MemoryBudget::new(4096);
        assert!(matches!(
            query(
                &entries,
                &[7, 8],
                GradientCellRecipe::Fixed,
                0.,
                comparisons,
                hits,
                1,
                &budget,
                None,
                || false
            ),
            Err(LocalError::Budget)
        ));
        assert_eq!(budget.peak(), 0);
        assert_eq!(budget.used(), 0);
    }
    let budget = MemoryBudget::new(4096);
    let report = query(
        &entries,
        &[7, 8],
        GradientCellRecipe::Fixed,
        0.,
        2,
        2,
        1,
        &budget,
        None,
        || false,
    )
    .unwrap();
    assert!(report.pairs.is_empty());
    assert_eq!((report.descriptor_comparisons, report.descriptor_hits), (2, 2));
    drop(report);
    assert_eq!(budget.used(), 0);
    let entries = [(1, descriptor(173)), (2, descriptor(173)), (3, descriptor(211))];
    let budget = MemoryBudget::new(4096);
    assert!(matches!(
        query(
            &entries,
            &[7, 8, 9],
            GradientCellRecipe::Fixed,
            0.,
            4,
            5,
            1,
            &budget,
            None,
            || false
        ),
        Err(LocalError::Budget)
    ));
    assert!(budget.peak() > 0);
    assert_eq!(budget.used(), 0);
}

#[test]
fn actual_work_pair_union_matches_independent_exhaustive_oracle() {
    use rrrah_dedup::gradient_index::gradient_file_pair_report_with_actual_work as query;
    use std::collections::BTreeSet;
    fn mirror(d: &GradientDescriptor) -> GradientDescriptor {
        let mut result = [0.; 128];
        for (i, value) in d.0.iter().enumerate() {
            let cell = i / 8;
            let bin = i % 8;
            result[((3 - cell / 4) * 4 + cell % 4) * 8 + (8 - bin) % 8] = *value;
        }
        GradientDescriptor(result)
    }
    fn distance(a: &GradientDescriptor, b: &GradientDescriptor) -> f64 {
        a.0.iter().zip(b.0.iter()).fold(0., |sum, (a, b)| {
            let delta = a - b;
            sum + delta * delta
        })
    }
    for n in [0, 1, 2, 7, 32] {
        let mut entries = Vec::new();
        let mut owners = Vec::new();
        for i in 0..n {
            let base = descriptor(173 + (i / 3) as u64);
            let d = if i % 3 == 1 { mirror(&base) } else { base };
            entries.push((u64::MAX - i as u64, d));
            owners.push(if i % 4 == 0 { u64::MAX } else { (i % 5) as u64 });
        }
        let mut radii = vec![0., f64::from_bits(1), 0.01, 0.1, 0.5, 1., 4.];
        if n > 1 {
            let boundary = distance(&entries[0].1, &entries[1].1);
            radii.extend([boundary.next_down().max(0.), boundary, boundary.next_up()]);
        }
        for recipe in [GradientCellRecipe::Fixed, GradientCellRecipe::Interpolated] {
            for reflected in [false, true] {
                for radius in radii.iter().copied() {
                    let mut pairs = BTreeSet::new();
                    let mut hits = 0;
                    for (i, (_, d)) in entries.iter().enumerate() {
                        for phase in 0..if reflected { 2 } else { 1 } {
                            let q = if phase == 0 { *d } else { mirror(d) };
                            for (j, (_, target)) in entries.iter().enumerate() {
                                if distance(&q, target) <= radius {
                                    hits += 1;
                                    if owners[i] != owners[j] {
                                        pairs.insert((owners[i].min(owners[j]), owners[i].max(owners[j])));
                                    }
                                }
                            }
                        }
                    }
                    let cap = n * n * if reflected { 2 } else { 1 };
                    let budget = MemoryBudget::new(1024 * 1024);
                    let report = query(
                        &entries,
                        &owners,
                        recipe,
                        radius,
                        cap as u64,
                        cap,
                        n * n,
                        &budget,
                        reflected.then_some(n as u64 * 128),
                        || false,
                    )
                    .unwrap();
                    assert_eq!(
                        report.pairs,
                        pairs.into_iter().collect::<Vec<_>>(),
                        "n={n} reflected={reflected} radius={radius}"
                    );
                    assert_eq!(report.descriptor_hits, hits);
                    assert!(
                        (hits as u64) <= report.descriptor_comparisons
                            && report.descriptor_comparisons <= cap as u64
                    );
                    let exact_comparisons = report.descriptor_comparisons;
                    let expected_pairs = report.pairs.clone();
                    drop(report);
                    assert_eq!(budget.used(), 0);
                    // Re-admit at the precise measured distance/hit/pair caps.
                    let report = query(
                        &entries,
                        &owners,
                        recipe,
                        radius,
                        exact_comparisons,
                        hits,
                        expected_pairs.len(),
                        &budget,
                        reflected.then_some(n as u64 * 128),
                        || false,
                    )
                    .unwrap();
                    assert_eq!(report.pairs, expected_pairs);
                    assert_eq!(report.descriptor_hits, hits);
                    drop(report);
                    assert_eq!(budget.used(), 0);
                }
            }
        }
    }
}
