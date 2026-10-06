//! Deterministic complete-link grouping of explicitly accepted pair evidence.
//! No graph connectivity is promoted to unobserved pairwise correspondence.

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pair {
    pub left: u64,
    pub right: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grouping {
    /// Every pair in each group occurs in `pairs`. Singletons are retained.
    /// This deterministic greedy partition is not a maximum-clique solver.
    pub groups: Vec<Vec<u64>>,
    /// All accepted edges, including links crossing the partition boundaries.
    pub pairs: Vec<Pair>,
}

#[derive(Debug, Clone, Copy)]
pub struct GroupBudget {
    pub max_entries: usize,
    pub max_pairs: usize,
    /// Actual member-edge checks after indexed representative selection.
    pub max_pair_checks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GroupError {
    #[error("duplicate application id")]
    DuplicateId,
    #[error("pair references an unknown id or itself")]
    InvalidPair,
    #[error("grouping exceeded the supplied budget")]
    Budget,
    #[error("grouping cancelled")]
    Cancelled,
}

/// Partition sorted ids by admitting an id only when every member of a group
/// has an accepted edge to it. Input order and reversed/duplicate edges do not
/// change output. Caller retains the original evidence behind every pair.
///
/// # Errors
/// Rejects invalid ids/edges, work/storage budget excess and cancellation.
/// No partial grouping is returned.
#[allow(clippy::too_many_lines)] // Keep admission, indexed eligibility and pair-check budgeting together.
pub fn complete_link_groups(
    ids: impl IntoIterator<Item = u64>,
    pairs: impl IntoIterator<Item = Pair>,
    budget: GroupBudget,
    cancel: impl Fn() -> bool,
) -> Result<Grouping, GroupError> {
    let mut entries = HashSet::new();
    for id in ids {
        if cancel() {
            return Err(GroupError::Cancelled);
        }
        if entries.contains(&id) {
            return Err(GroupError::DuplicateId);
        }
        if entries.len() >= budget.max_entries {
            return Err(GroupError::Budget);
        }
        entries.try_reserve(1).map_err(|_| GroupError::Budget)?;
        entries.insert(id);
    }
    let mut edges = HashSet::new();
    for (consumed, pair) in pairs.into_iter().enumerate() {
        if cancel() {
            return Err(GroupError::Cancelled);
        }
        // Bound raw input as well as unique edge storage, including duplicates.
        if consumed >= budget.max_pairs {
            return Err(GroupError::Budget);
        }
        if pair.left == pair.right || !entries.contains(&pair.left) || !entries.contains(&pair.right) {
            return Err(GroupError::InvalidPair);
        }
        let edge = ordered(pair.left, pair.right);
        if !edges.contains(&edge) {
            edges.try_reserve(1).map_err(|_| GroupError::Budget)?;
            edges.insert(edge);
        }
    }
    let mut sorted_entries = Vec::new();
    sorted_entries
        .try_reserve_exact(entries.len())
        .map_err(|_| GroupError::Budget)?;
    sorted_entries.extend(entries);
    sorted_entries.sort_unstable();
    // Only a group whose first member has an accepted edge to this id can
    // possibly admit it. Keep all edges for evidence; index lower neighbours.
    let mut predecessors: HashMap<u64, Vec<u64>> = HashMap::new();
    for edge in &edges {
        if cancel() {
            return Err(GroupError::Cancelled);
        }
        predecessors.try_reserve(1).map_err(|_| GroupError::Budget)?;
        let row = predecessors.entry(edge.right).or_default();
        crate::local::reserve_slot(row, budget.max_pairs).map_err(|_| GroupError::Budget)?;
        row.push(edge.left);
    }
    let mut representatives: HashMap<u64, usize> = HashMap::new();
    let mut groups: Vec<Vec<u64>> = Vec::new();
    let mut checks = 0_u64;
    for id in sorted_entries {
        if cancel() {
            return Err(GroupError::Cancelled);
        }
        let mut candidates = Vec::new();
        if let Some(neighbours) = predecessors.get(&id) {
            for neighbour in neighbours {
                if cancel() {
                    return Err(GroupError::Cancelled);
                }
                if let Some(&index) = representatives.get(neighbour) {
                    crate::local::reserve_slot(&mut candidates, budget.max_entries)
                        .map_err(|_| GroupError::Budget)?;
                    candidates.push(index);
                }
            }
        }
        candidates.sort_unstable();
        let mut destination = None;
        for index in candidates {
            let group = &groups[index];
            let mut accepted = true;
            for &member in group {
                if cancel() {
                    return Err(GroupError::Cancelled);
                }
                if checks >= budget.max_pair_checks {
                    return Err(GroupError::Budget);
                }
                checks += 1;
                if !edges.contains(&ordered(member, id)) {
                    accepted = false;
                    break;
                }
            }
            if accepted {
                destination = Some(index);
                break;
            }
        }
        if let Some(index) = destination {
            crate::local::reserve_slot(&mut groups[index], budget.max_entries)
                .map_err(|_| GroupError::Budget)?;
            groups[index].push(id);
        } else {
            let mut group = Vec::new();
            group.try_reserve_exact(1).map_err(|_| GroupError::Budget)?;
            group.push(id);
            crate::local::reserve_slot(&mut groups, budget.max_entries).map_err(|_| GroupError::Budget)?;
            representatives.try_reserve(1).map_err(|_| GroupError::Budget)?;
            representatives.insert(id, groups.len());
            groups.push(group);
        }
    }
    let mut pairs = Vec::new();
    pairs
        .try_reserve_exact(edges.len())
        .map_err(|_| GroupError::Budget)?;
    pairs.extend(edges);
    pairs.sort_unstable();
    if cancel() {
        return Err(GroupError::Cancelled);
    }
    Ok(Grouping { groups, pairs })
}

fn ordered(a: u64, b: u64) -> Pair {
    Pair {
        left: a.min(b),
        right: a.max(b),
    }
}
