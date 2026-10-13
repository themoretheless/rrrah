//! Model-independent admission of scored correspondences. Proposals and geometry
//! are candidate evidence; original-pixel confirmation remains required.
use crate::geometry::{Correspondence, GeometryError};

#[derive(Debug, Clone, Copy)]
pub struct ScoredProposal {
    pub source: [f64; 2],
    pub target: [f64; 2],
    pub confidence: f64,
}

/// Resized inference raster and the original oriented raster, in width/height order.
#[derive(Debug, Clone, Copy)]
pub struct ProposalDomain {
    pub input: [u32; 2],
    pub original: [u32; 2],
}

/// A resized crop domain, its origin and the complete oriented image extent.
/// `domain.original` is the crop width/height. Bounds are checked before mapping;
/// returned correspondences always use complete original-image coordinates.
#[derive(Debug, Clone, Copy)]
pub struct RegionProposalDomain {
    pub domain: ProposalDomain,
    pub origin: [u32; 2],
    pub full: [u32; 2],
}
impl From<ProposalDomain> for RegionProposalDomain {
    fn from(domain: ProposalDomain) -> Self {
        Self {
            domain,
            origin: [0, 0],
            full: domain.original,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProposalPolicy {
    pub max_proposals: usize,
    pub max_ranking_checks: u64,
    pub max_separation_checks: u64,
    pub minimum_separation: f64,
}

/// Map pixel centers to original coordinates and greedily select distinct points
/// by descending confidence, retaining input order on ties. Out-of-domain mapped
/// points are discarded; malformed/nonfinite proposals refuse the entire batch.
/// Heap ranking and each separation comparison observe cancellation and explicit
/// work limits. Caller inputs, vector/allocator overhead and model allocations are
/// not memory-accounted here; vectors reserve at most the declared proposal count.
/// No model downloads, inference or pixel verdict occur in this function.
///
/// # Errors
/// Invalid dimensions, confidence outside [0,1], nonfinite coordinates/separation,
/// excessive count, allocation/work refusal or cancellation. No partial output.
pub fn select_scored_proposals(
    proposals: &[ScoredProposal],
    source: ProposalDomain,
    target: ProposalDomain,
    policy: ProposalPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Vec<Correspondence>, GeometryError> {
    select_scored_region_proposals(proposals, source.into(), target.into(), policy, cancel)
}

/// Select region proposals with the same confidence, distinctness and work
/// semantics as [`select_scored_proposals`], then apply original crop origins.
/// Out-of-crop mapped points are discarded even if they lie in the full image.
///
/// # Errors
/// Invalid/overflowing crop domains, malformed proposals, count/work/allocation
/// refusal or cancellation. No partial correspondence set is returned.
pub fn select_scored_region_proposals(
    proposals: &[ScoredProposal],
    source: RegionProposalDomain,
    target: RegionProposalDomain,
    policy: ProposalPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Vec<Correspondence>, GeometryError> {
    let checkpoint = || {
        if cancel() {
            Err(GeometryError::Cancelled)
        } else {
            Ok(())
        }
    };
    checkpoint()?;
    let valid = |r: RegionProposalDomain| {
        !r.domain.input.contains(&0)
            && !r.domain.original.contains(&0)
            && !r.full.contains(&0)
            && (0..2).all(|axis| {
                r.origin[axis]
                    .checked_add(r.domain.original[axis])
                    .is_some_and(|end| end <= r.full[axis])
            })
    };
    if !valid(source)
        || !valid(target)
        || !policy.minimum_separation.is_finite()
        || policy.minimum_separation < 0.0
        || !policy.minimum_separation.powi(2).is_finite()
    {
        return Err(GeometryError::Invalid);
    }
    if proposals.len() > policy.max_proposals {
        return Err(GeometryError::Budget);
    }
    for p in proposals {
        checkpoint()?;
        if p.source.iter().chain(p.target.iter()).any(|x| !x.is_finite())
            || !p.confidence.is_finite()
            || !(0.0..=1.0).contains(&p.confidence)
        {
            return Err(GeometryError::Invalid);
        }
    }
    let mut heap = Vec::new();
    let mut selected: Vec<Correspondence> = Vec::new();
    heap.try_reserve_exact(proposals.len())
        .map_err(|_| GeometryError::Budget)?;
    selected
        .try_reserve_exact(proposals.len())
        .map_err(|_| GeometryError::Budget)?;
    for i in 0..proposals.len() {
        checkpoint()?;
        heap.push(i);
    }
    let mut ranking = policy.max_ranking_checks;
    let mut better = |a: usize, b: usize| -> Result<bool, GeometryError> {
        checkpoint()?;
        ranking = ranking.checked_sub(1).ok_or(GeometryError::Budget)?;
        Ok(proposals[a].confidence > proposals[b].confidence
            || (proposals[a].confidence == proposals[b].confidence && a < b))
    };
    fn sift(
        heap: &mut [usize],
        mut parent: usize,
        better: &mut impl FnMut(usize, usize) -> Result<bool, GeometryError>,
    ) -> Result<(), GeometryError> {
        while let Some(left) = parent
            .checked_mul(2)
            .and_then(|v| v.checked_add(1))
            .filter(|v| *v < heap.len())
        {
            let right = left + 1;
            let child = if right < heap.len() && better(heap[right], heap[left])? {
                right
            } else {
                left
            };
            if !better(heap[child], heap[parent])? {
                break;
            }
            heap.swap(parent, child);
            parent = child;
        }
        Ok(())
    }
    for i in (0..heap.len() / 2).rev() {
        sift(&mut heap, i, &mut better)?;
    }
    let map = |p: [f64; 2], region: RegionProposalDomain| -> Option<[f64; 2]> {
        let domain = region.domain;
        let p: [f64; 2] = std::array::from_fn(|axis| {
            (p[axis] + 0.5) * f64::from(domain.original[axis]) / f64::from(domain.input[axis]) - 0.5
        });
        (0..2)
            .all(|axis| p[axis].is_finite() && p[axis] >= 0.0 && p[axis] < f64::from(domain.original[axis]))
            .then(|| std::array::from_fn(|axis| p[axis] + f64::from(region.origin[axis])))
    };
    let distance = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2);
    let threshold = policy.minimum_separation.powi(2);
    let mut separation = policy.max_separation_checks;
    while !heap.is_empty() {
        checkpoint()?;
        let index = heap.swap_remove(0);
        sift(&mut heap, 0, &mut better)?;
        let p = proposals[index];
        let (Some(source), Some(target)) = (map(p.source, source), map(p.target, target)) else {
            continue;
        };
        let mut distinct = true;
        for q in &selected {
            checkpoint()?;
            separation = separation.checked_sub(1).ok_or(GeometryError::Budget)?;
            if distance(source, q.source) <= threshold || distance(target, q.target) <= threshold {
                distinct = false;
                break;
            }
        }
        if distinct {
            selected.push(Correspondence { source, target });
        }
    }
    checkpoint()?;
    Ok(selected)
}

/// Admit ranking scratch and correspondence vector payload before selection.
/// Returned payload credit follows the last shared owner. Caller proposals,
/// model tensors, allocator and shared-owner overhead are excluded. This is not
/// an inference memory bound. Uses the same ordering and refusal semantics as
/// [`select_scored_proposals`].
///
/// # Errors
/// Invalid/count/work refusal, checked payload admission/allocation failure or
/// cancellation. All reservations are released on failure.
#[cfg(feature = "raster")]
pub fn select_scored_proposals_managed(
    proposals: &[ScoredProposal],
    source: ProposalDomain,
    target: ProposalDomain,
    policy: ProposalPolicy,
    budget: &rrrah_core::MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<Correspondence>, GeometryError> {
    select_scored_region_proposals_managed(proposals, source.into(), target.into(), policy, budget, cancel)
}

/// Managed counterpart of [`select_scored_region_proposals`]. Reservations span
/// ranking scratch and output payload; shared credit follows the last owner.
///
/// # Errors
/// Invalid input, count/work/memory refusal or cancellation, without partial output.
#[cfg(feature = "raster")]
pub fn select_scored_region_proposals_managed(
    proposals: &[ScoredProposal],
    source: RegionProposalDomain,
    target: RegionProposalDomain,
    policy: ProposalPolicy,
    budget: &rrrah_core::MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<Correspondence>, GeometryError> {
    if cancel() {
        return Err(GeometryError::Cancelled);
    }
    if proposals.len() > policy.max_proposals {
        return Err(GeometryError::Budget);
    }
    let bytes = |size: usize| {
        proposals
            .len()
            .checked_mul(size)
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(GeometryError::Budget)
    };
    let scratch = budget
        .try_reserve(bytes(std::mem::size_of::<usize>())?)
        .map_err(|_| GeometryError::Budget)?;
    let output = budget
        .try_reserve(bytes(std::mem::size_of::<Correspondence>())?)
        .map_err(|_| GeometryError::Budget)?;
    let selected = select_scored_region_proposals(proposals, source, target, policy, &cancel)?;
    drop(scratch);
    if cancel() {
        return Err(GeometryError::Cancelled);
    }
    let selected = output.try_adopt(selected).map_err(|_| GeometryError::Budget)?;
    if cancel() {
        return Err(GeometryError::Cancelled);
    }
    Ok(selected)
}
