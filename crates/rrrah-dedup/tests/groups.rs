use rrrah_dedup::groups::{GroupBudget, GroupError, Pair, complete_link_groups};
const BUDGET: GroupBudget = GroupBudget {
    max_entries: 100,
    max_pairs: 1000,
    max_pair_checks: 10000,
};

#[test]
fn nontransitive_chain_preserves_edge_without_inventing_pair() {
    let result = complete_link_groups(
        [3, 1, 2],
        [Pair { left: 1, right: 2 }, Pair { left: 2, right: 3 }],
        BUDGET,
        || false,
    )
    .unwrap();
    assert_eq!(result.groups, [vec![1, 2], vec![3]]);
    assert_eq!(result.pairs.len(), 2);
    let reordered = complete_link_groups(
        [2, 3, 1],
        [
            Pair { left: 3, right: 2 },
            Pair { left: 2, right: 1 },
            Pair { left: 1, right: 2 },
        ],
        BUDGET,
        || false,
    )
    .unwrap();
    assert_eq!(reordered, result);
}

#[test]
fn every_emitted_group_has_all_pairs_for_all_small_graphs() {
    let edges = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
    for mask in 0..64 {
        let pairs: Vec<_> = edges
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, &(left, right))| Pair { left, right })
            .collect();
        let result = complete_link_groups(0..4, pairs.clone(), BUDGET, || false).unwrap();
        let reversed = pairs.iter().rev().map(|pair| Pair {
            left: pair.right,
            right: pair.left,
        });
        assert_eq!(
            complete_link_groups((0..4).rev(), reversed, BUDGET, || false).unwrap(),
            result
        );
        let checks = std::cell::Cell::new(0);
        complete_link_groups(0..4, pairs.clone(), BUDGET, || {
            checks.set(checks.get() + 1);
            false
        })
        .unwrap();
        for checkpoint in 1..=checks.get() {
            let current = std::cell::Cell::new(0);
            assert_eq!(
                complete_link_groups(0..4, pairs.clone(), BUDGET, || {
                    current.set(current.get() + 1);
                    current.get() == checkpoint
                }),
                Err(GroupError::Cancelled)
            );
        }
        assert_eq!(
            complete_link_groups(0..4, pairs.clone(), BUDGET, || false).unwrap(),
            result
        );
        let mut members: Vec<_> = result.groups.iter().flatten().copied().collect();
        members.sort_unstable();
        assert_eq!(members, [0, 1, 2, 3]);
        for group in result.groups {
            for (index, &left) in group.iter().enumerate() {
                for &right in &group[index + 1..] {
                    assert!(pairs.contains(&Pair { left, right }));
                }
            }
        }
        assert_eq!(result.pairs.len(), pairs.len());
    }
}

#[test]
fn invalid_input_budget_and_cancellation_are_explicit() {
    assert_eq!(
        complete_link_groups([1, 1], [], BUDGET, || false),
        Err(GroupError::DuplicateId)
    );
    for pair in [Pair { left: 1, right: 1 }, Pair { left: 1, right: 2 }] {
        assert_eq!(
            complete_link_groups([1], [pair], BUDGET, || false),
            Err(GroupError::InvalidPair)
        );
    }
    assert_eq!(
        complete_link_groups(
            [1],
            [],
            GroupBudget {
                max_entries: 0,
                ..BUDGET
            },
            || false
        ),
        Err(GroupError::Budget)
    );
    assert_eq!(
        complete_link_groups(
            [1, 2],
            [Pair { left: 1, right: 2 }],
            GroupBudget {
                max_pair_checks: 0,
                ..BUDGET
            },
            || false
        ),
        Err(GroupError::Budget)
    );
    assert_eq!(
        complete_link_groups([1], [], BUDGET, || true),
        Err(GroupError::Cancelled)
    );
}

fn exhaustive_reference(count: u64, pairs: &[Pair]) -> Vec<Vec<u64>> {
    let mut groups: Vec<Vec<u64>> = Vec::new();
    for id in 0..count {
        let destination = groups.iter().position(|group| {
            group.iter().all(|&member| {
                pairs.contains(&Pair {
                    left: member.min(id),
                    right: member.max(id),
                })
            })
        });
        if let Some(index) = destination {
            groups[index].push(id);
        } else {
            groups.push(vec![id]);
        }
    }
    groups
}

#[test]
fn sparse_selection_matches_independent_exhaustive_reference_for_all_six_node_graphs() {
    let possible = (0..6)
        .flat_map(|left| (left + 1..6).map(move |right| Pair { left, right }))
        .collect::<Vec<_>>();
    for mask in 0_u32..(1 << possible.len()) {
        let pairs = possible
            .iter()
            .enumerate()
            .filter_map(|(i, &pair)| (mask & (1 << i) != 0).then_some(pair))
            .collect::<Vec<_>>();
        let result = complete_link_groups(0..6, pairs.clone(), BUDGET, || false).unwrap();
        assert_eq!(
            result.groups,
            exhaustive_reference(6, &pairs),
            "graph mask={mask}"
        );
        assert_eq!(result.pairs, pairs, "accepted edges mask={mask}");
    }
}

#[test]
fn isolated_and_sparse_entries_avoid_unrelated_group_checks() {
    let count = 16_384_u64;
    let calls = std::cell::Cell::new(0);
    let result = complete_link_groups(
        0..count,
        [],
        GroupBudget {
            max_entries: usize::try_from(count).unwrap(),
            max_pairs: 0,
            max_pair_checks: 0,
        },
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    assert_eq!(result.groups.len(), usize::try_from(count).unwrap());
    assert!(
        result
            .groups
            .iter()
            .zip(0..count)
            .all(|(group, id)| group == &[id])
    );
    assert!(result.pairs.is_empty());
    assert!(calls.get() <= count * 3 + 10);
    let chain_count = 8192_u64;
    let pairs = (0..chain_count - 1)
        .map(|left| Pair {
            left,
            right: left + 1,
        })
        .collect::<Vec<_>>();
    let chain = complete_link_groups(
        0..chain_count,
        pairs.clone(),
        GroupBudget {
            max_entries: usize::try_from(chain_count).unwrap(),
            max_pairs: pairs.len(),
            max_pair_checks: chain_count,
        },
        || false,
    )
    .unwrap();
    assert_eq!(chain.groups.len(), usize::try_from(chain_count / 2).unwrap());
    assert!(
        chain
            .groups
            .iter()
            .enumerate()
            .all(|(i, group)| group == &[u64::try_from(i).unwrap() * 2, u64::try_from(i).unwrap() * 2 + 1])
    );
    assert_eq!(chain.pairs, pairs);
    eprintln!(
        "isolated entries={count} callback_checks={} member_check_budget=0; sparse chain entries={chain_count} member_check_budget={chain_count}",
        calls.get()
    );
}
