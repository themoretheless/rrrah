//! Preserve bank provenance while separating whole endpoint-alias components.
//! This partitions coordinates, not image/descriptor contexts or copy evidence.
use crate::geometry::{Correspondence, GeometryError};
use rrrah_core::{MemoryBudget, SharedBuffer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BankPointPartition {
    Fitting,
    Validation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BankPointAssignment {
    pub bank: usize,
    pub point: usize,
    /// Compact component ID in first-member input order.
    pub component: usize,
    pub partition: BankPointPartition,
}

#[derive(Debug, Clone, Copy)]
pub struct BankPartitionPolicy {
    pub maximum_banks: usize,
    pub maximum_points: usize,
    pub maximum_separation_checks: u64,
    pub maximum_parent_steps: u64,
    pub minimum_endpoint_separation: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BankPartitionWork {
    pub separation_checks: u64,
    /// Root reads with union-by-rank depth bound, plus one per union attempt.
    pub parent_steps: u64,
    /// Assignment storage plus managed parent/rank/component-label scratch.
    pub managed_payload_bytes: u64,
}

/// Conservative work and payload admission without inspecting point values.
/// # Errors
/// Count, pair, parent-step or payload arithmetic overflow.
pub fn required_bank_partition_work(points: usize) -> Result<BankPartitionWork, GeometryError> {
    let n = u64::try_from(points).map_err(|_| GeometryError::Budget)?;
    let pairs = n
        .checked_mul(n.saturating_sub(1))
        .and_then(|x| x.checked_div(2))
        .ok_or(GeometryError::Budget)?;
    let depth = u64::from(u64::BITS - n.leading_zeros());
    let steps = pairs
        .checked_mul(2)
        .and_then(|x| x.checked_add(n))
        .and_then(|x| x.checked_mul(depth))
        .and_then(|x| x.checked_add(pairs))
        .ok_or(GeometryError::Budget)?;
    let bytes = n
        .checked_mul(
            (std::mem::size_of::<BankPointAssignment>()
                + 2 * std::mem::size_of::<usize>()
                + std::mem::size_of::<u8>()) as u64,
        )
        .ok_or(GeometryError::Budget)?;
    Ok(BankPartitionWork {
        separation_checks: pairs,
        parent_steps: steps,
        managed_payload_bytes: bytes,
    })
}

#[derive(Debug, Clone)]
pub struct BankPartitionEvidence {
    pub assignments: SharedBuffer<BankPointAssignment>,
    pub banks: usize,
    pub components: usize,
    pub fitting_points: usize,
    pub validation_points: usize,
    pub work: BankPartitionWork,
    pub parent_steps: u64,
}

fn root(
    parents: &[usize],
    mut i: usize,
    remaining: &mut u64,
    cancel: &impl Fn() -> bool,
) -> Result<usize, GeometryError> {
    loop {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        *remaining = remaining.checked_sub(1).ok_or(GeometryError::Budget)?;
        let next = parents[i];
        if next == i {
            return Ok(i);
        }
        i = next;
    }
}

/// Connect points if either endpoint is within the caller's separation distance,
/// including transitive chains across banks. Alternate whole components between
/// fitting and validation in first-member input order. Every original point and
/// alternative correspondence is retained; no residual, score, model or pixel
/// influences grouping, component order or assignment. Different partitions
/// therefore have strictly separated endpoints in both coordinate domains.
///
/// Assignments refer to the caller's arrays; retain those arrays unchanged when
/// using them. A component can contain conflicting alternatives or most of the
/// entire pool. Such a component must not be split just to obtain enough heldout
/// points. Count components, not their duplicate members, as separate geometric
/// witnesses. Within-partition aliases are intentional, unlike the flat-set
/// contract of `geometry_validation::validate_projective_points`.
/// Coordinate separation does not imply disjoint pixel/descriptor contexts or
/// statistical independence from a provider that saw the complete images.
///
/// All unordered pairs and worst-case union-by-rank parent traversal work are
/// admitted before allocation/traversal. Assignment and forest payloads are
/// managed; scratch is released before return, output credit follows its last
/// shared owner. Allocator/shared-owner overhead is outside payload admission.
/// Empty banks are valid. No partial assignments survive failure/cancellation.
/// # Errors
/// Invalid policy/domains/endpoints, count/work/payload refusal, allocation
/// failure, arithmetic overflow or cancellation.
#[allow(clippy::too_many_arguments)]
pub fn partition_point_banks(
    banks: &[&[Correspondence]],
    source: (u32, u32),
    target: (u32, u32),
    policy: BankPartitionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<BankPartitionEvidence, GeometryError> {
    if cancel() {
        return Err(GeometryError::Cancelled);
    }
    if source.0 == 0
        || source.1 == 0
        || target.0 == 0
        || target.1 == 0
        || !policy.minimum_endpoint_separation.is_finite()
        || policy.minimum_endpoint_separation <= 0.
    {
        return Err(GeometryError::Invalid);
    }
    let n = banks
        .iter()
        .try_fold(0usize, |n, b| n.checked_add(b.len()))
        .ok_or(GeometryError::Budget)?;
    let work = required_bank_partition_work(n)?;
    if banks.len() > policy.maximum_banks
        || n > policy.maximum_points
        || work.separation_checks > policy.maximum_separation_checks
        || work.parent_steps > policy.maximum_parent_steps
    {
        return Err(GeometryError::Budget);
    }
    // Admit all four payloads before allocating any of them.
    let reserve = |size: usize| {
        budget
            .try_reserve((n as u64).checked_mul(size as u64).ok_or(GeometryError::Budget)?)
            .map_err(|_| GeometryError::Budget)
    };
    let output_credit = reserve(std::mem::size_of::<BankPointAssignment>())?;
    let parent_credit = reserve(std::mem::size_of::<usize>())?;
    let rank_credit = reserve(std::mem::size_of::<u8>())?;
    let label_credit = reserve(std::mem::size_of::<usize>())?;
    let dummy = BankPointAssignment {
        bank: 0,
        point: 0,
        component: 0,
        partition: BankPointPartition::Fitting,
    };
    let mut assignments = output_credit
        .try_buffer(n, dummy)
        .map_err(|_| GeometryError::Budget)?;
    let mut parents = parent_credit
        .try_buffer(n, 0usize)
        .map_err(|_| GeometryError::Budget)?;
    let mut ranks = rank_credit
        .try_buffer(n, 0u8)
        .map_err(|_| GeometryError::Budget)?;
    let mut labels = label_credit
        .try_buffer(n, usize::MAX)
        .map_err(|_| GeometryError::Budget)?;
    let mut flat = 0;
    for (bank, points) in banks.iter().enumerate() {
        for (point, p) in points.iter().enumerate() {
            if cancel() {
                return Err(GeometryError::Cancelled);
            }
            for (xy, d) in [(p.source, source), (p.target, target)] {
                if xy.iter().any(|v| !v.is_finite() || *v < 0.)
                    || xy[0] >= f64::from(d.0)
                    || xy[1] >= f64::from(d.1)
                {
                    return Err(GeometryError::Invalid);
                }
            }
            assignments[flat] = BankPointAssignment { bank, point, ..dummy };
            parents[flat] = flat;
            flat += 1;
        }
    }
    let mut remaining = work.parent_steps;
    for i in 0..n {
        for j in 0..i {
            if cancel() {
                return Err(GeometryError::Cancelled);
            }
            let a = assignments[i];
            let b = assignments[j];
            let a = banks[a.bank][a.point];
            let b = banks[b.bank][b.point];
            if ![(a.source, b.source), (a.target, b.target)]
                .iter()
                .any(|(a, b)| (a[0] - b[0]).hypot(a[1] - b[1]) <= policy.minimum_endpoint_separation)
            {
                continue;
            }
            let mut a = root(&parents, i, &mut remaining, &cancel)?;
            let mut b = root(&parents, j, &mut remaining, &cancel)?;
            remaining = remaining.checked_sub(1).ok_or(GeometryError::Budget)?;
            if a == b {
                continue;
            }
            if ranks[a] < ranks[b] {
                std::mem::swap(&mut a, &mut b);
            }
            parents[b] = a;
            if ranks[a] == ranks[b] {
                ranks[a] = ranks[a].checked_add(1).ok_or(GeometryError::Budget)?;
            }
        }
    }
    let mut components = 0;
    let mut fitting_points = 0;
    for (i, assignment) in assignments.iter_mut().enumerate() {
        let r = root(&parents, i, &mut remaining, &cancel)?;
        if labels[r] == usize::MAX {
            labels[r] = components;
            components += 1;
        }
        assignment.component = labels[r];
        assignment.partition = if labels[r] % 2 == 0 {
            fitting_points += 1;
            BankPointPartition::Fitting
        } else {
            BankPointPartition::Validation
        };
    }
    if cancel() {
        return Err(GeometryError::Cancelled);
    }
    let parent_steps = work.parent_steps - remaining;
    drop(parents);
    drop(ranks);
    drop(labels);
    let assignments = assignments.freeze();
    if cancel() {
        return Err(GeometryError::Cancelled);
    }
    Ok(BankPartitionEvidence {
        assignments,
        banks: banks.len(),
        components,
        fitting_points,
        validation_points: n - fitting_points,
        work,
        parent_steps,
    })
}
