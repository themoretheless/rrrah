//! Bounded geometric aggregation across caller-owned, full-coordinate point banks.
//! Output witnesses require independent original-pixel confirmation.
use crate::geometry::{Correspondence, GeometryError, ProjectiveTransform, projective_domain_eligible};
use rrrah_core::{MemoryBudget, SharedBuffer};

#[derive(Debug, Clone, Copy)]
pub struct JointWitnessPolicy {
    pub source_tolerance: f64,
    pub target_tolerance: f64,
    pub minimum_separation: f64,
    pub maximum_points: usize,
    /// Two projections per supplied point, including points later excluded.
    pub maximum_projection_checks: u64,
    /// Worst-case greedy endpoint comparisons, N*(N-1)/2.
    pub maximum_separation_checks: u64,
}

/// Preserve caller order, filter by both residuals, then reject aliases at either
/// endpoint. Repeated cross-window points are expected and skipped. Malformed
/// points refuse the entire batch, including points after a usable prefix.
/// Work and retained capacity are admitted before scanning. Caller input storage
/// is external; output capacity is managed until its last clone drops. No pixels
/// are read and empty output means no witnesses, never image nonidentity.
#[allow(clippy::too_many_arguments)]
pub fn collect_joint_projective_witnesses(
    points: &[Correspondence],
    transform: ProjectiveTransform,
    source: (u32, u32),
    target: (u32, u32),
    policy: JointWitnessPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<SharedBuffer<Correspondence>, GeometryError> {
    if cancel() { return Err(GeometryError::Cancelled); }
    let positive = |v: f64| v.is_finite() && v > 0. && (v*v).is_finite() && v*v > 0.;
    if source.0 == 0 || source.1 == 0 || target.0 == 0 || target.1 == 0
        || !positive(policy.source_tolerance) || !positive(policy.target_tolerance)
        || !policy.minimum_separation.is_finite() || policy.minimum_separation <= 0.
        || !projective_domain_eligible(transform, source, target) {
        return Err(GeometryError::Invalid);
    }
    let n=u64::try_from(points.len()).map_err(|_|GeometryError::Budget)?;
    let projections=n.checked_mul(2).ok_or(GeometryError::Budget)?;
    let separation=n.checked_mul(n.saturating_sub(1)).and_then(|v|v.checked_div(2)).ok_or(GeometryError::Budget)?;
    if points.len()>policy.maximum_points || projections>policy.maximum_projection_checks
        || separation>policy.maximum_separation_checks { return Err(GeometryError::Budget); }
    let bytes=n.checked_mul(std::mem::size_of::<Correspondence>() as u64).ok_or(GeometryError::Budget)?;
    let credit=budget.try_reserve(bytes).map_err(|_|GeometryError::Budget)?;
    let mut selected: Vec<Correspondence>=Vec::new();
    selected.try_reserve_exact(points.len()).map_err(|_|GeometryError::Budget)?;
    let inverse=transform.inverse()?;
    let distance=|a:[f64;2],b:[f64;2]|(a[0]-b[0]).hypot(a[1]-b[1]);
    for point in points {
        if cancel() { return Err(GeometryError::Cancelled); }
        for (p,d) in [(point.source,source),(point.target,target)] {
            if p.iter().any(|v|!v.is_finite()) || p[0]<0. || p[1]<0.
                || p[0]>=f64::from(d.0) || p[1]>=f64::from(d.1) { return Err(GeometryError::Invalid); }
        }
        if !transform.apply(point.source).is_some_and(|p|distance(p,point.target)<=policy.target_tolerance)
            || !inverse.apply(point.target).is_some_and(|p|distance(p,point.source)<=policy.source_tolerance) { continue; }
        let mut alias=false;
        for prior in &selected {
            if cancel() { return Err(GeometryError::Cancelled); }
            if distance(prior.source,point.source)<=policy.minimum_separation
                || distance(prior.target,point.target)<=policy.minimum_separation {alias=true;break;}
        }
        if !alias { selected.push(*point); }
    }
    if cancel() { return Err(GeometryError::Cancelled); }
    let output=credit.try_adopt(selected).map_err(|_|GeometryError::Budget)?;
    if cancel() { return Err(GeometryError::Cancelled); }
    Ok(output)
}

/// Local hypothesis admission and full-bank witness admission are separate:
/// a four-point candidate may acquire ten strict witnesses across windows.
#[derive(Debug, Clone, Copy)]
pub struct JointBankPolicy {
    pub hypothesis: crate::geometry::JointProjectivePolicy,
    pub sampling: crate::geometry::ProjectiveSamplingPolicy,
    pub witnesses: JointWitnessPolicy,
    pub minimum_witnesses: usize,
    pub maximum_banks: usize,
    pub maximum_total_projection_checks: u64,
    pub maximum_total_separation_checks: u64,
}

#[derive(Debug, Clone)]
pub struct JointBankCandidate {
    pub transform: ProjectiveTransform,
    pub witnesses: SharedBuffer<Correspondence>,
    pub hypothesis_bank: usize,
}

/// Generate one sampled model per bank and rescore each on all banks. Choose
/// the largest alias-free witness set, retaining the first on ties. This is
/// geometry-only candidate ranking: the winner still requires pixel confirmation.
/// All input domains/counts and worst-case work are checked before any fitting;
/// malformed suffixes and cancellation discard the entire result. A flattened
/// temporary bank and returned witnesses are managed. Existing fitter scratch
/// remains fallible, bounded by the per-bank policy, outside payload accounting.
#[allow(clippy::too_many_arguments)]
pub fn search_joint_bank_candidate(
    banks: &[&[Correspondence]],
    source: (u32, u32),
    target: (u32, u32),
    policy: JointBankPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<Option<JointBankCandidate>, GeometryError> {
    search_joint_bank_candidate_with(banks,source,target,policy,budget,cancel,|_|Ok::<bool,GeometryError>(true))
}

/// Generate and qualify every strict bank candidate before geometric ranking.
/// Rejected candidates never prune later candidates. Callback errors are fatal;
/// cancellation immediately after the callback wins over its error. Input and
/// geometric work admission is identical to the geometry-only entry point.
/// Callers must prepay callback work separately. No partial result is returned.
#[allow(clippy::too_many_arguments)]
pub fn search_joint_bank_candidate_with<E: From<GeometryError>>(
    banks: &[&[Correspondence]], source:(u32,u32), target:(u32,u32),
    policy:JointBankPolicy,budget:&MemoryBudget,cancel:impl Fn()->bool,
    mut qualify:impl FnMut(&JointBankCandidate)->Result<bool,E>,
)->Result<Option<JointBankCandidate>,E> {
    use crate::geometry::verify_projective_sampled_joint_for_domains;
    if cancel() { return Err(GeometryError::Cancelled.into()); }
    let positive = |v:f64|v.is_finite() && v>0. && (v*v).is_finite() && v*v>0.;
    if source.0==0 || source.1==0 || target.0==0 || target.1==0
        || !positive(policy.witnesses.source_tolerance) || !positive(policy.witnesses.target_tolerance)
        || policy.hypothesis.source_tolerance != policy.witnesses.source_tolerance
        || policy.hypothesis.target.tolerance != policy.witnesses.target_tolerance
        || policy.minimum_witnesses < policy.hypothesis.target.min_inliers
        || policy.hypothesis.target.min_inliers < 4 || policy.sampling.trials == 0
        || policy.sampling.seed == 0 || policy.sampling.trials>policy.hypothesis.target.max_hypotheses
        || !policy.witnesses.minimum_separation.is_finite() || policy.witnesses.minimum_separation<=0. {
        return Err(GeometryError::Invalid.into());
    }
    if banks.len()>policy.maximum_banks { return Err(GeometryError::Budget.into()); }
    let mut count=0usize;
    for bank in banks {
        if cancel() { return Err(GeometryError::Cancelled.into()); }
        if bank.len()>policy.hypothesis.target.max_points { return Err(GeometryError::Budget.into()); }
        count=count.checked_add(bank.len()).ok_or(GeometryError::Budget)?;
    }
    let n=u64::try_from(count).map_err(|_|GeometryError::Budget)?;
    let b=u64::try_from(banks.len()).map_err(|_|GeometryError::Budget)?;
    let per_fit=u64::try_from(policy.hypothesis.target.max_points).map_err(|_|GeometryError::Budget)?
        .checked_mul(2).and_then(|v|policy.sampling.trials.checked_add(1).and_then(|t|v.checked_mul(t)))
        .ok_or(GeometryError::Budget)?;
    let per_collect=n.checked_mul(2).ok_or(GeometryError::Budget)?;
    let per_separation=n.checked_mul(n.saturating_sub(1)).and_then(|v|v.checked_div(2)).ok_or(GeometryError::Budget)?;
    let total=per_fit.checked_add(per_collect).and_then(|v|v.checked_mul(b)).ok_or(GeometryError::Budget)?;
    let separation=per_separation.checked_mul(b).ok_or(GeometryError::Budget)?;
    if count>policy.witnesses.maximum_points || per_fit>policy.hypothesis.maximum_projection_checks
        || per_collect>policy.witnesses.maximum_projection_checks || per_separation>policy.witnesses.maximum_separation_checks
        || total>policy.maximum_total_projection_checks || separation>policy.maximum_total_separation_checks {
        return Err(GeometryError::Budget.into());
    }
    let bytes=n.checked_mul(std::mem::size_of::<Correspondence>() as u64).ok_or(GeometryError::Budget)?;
    let credit=budget.try_reserve(bytes).map_err(|_|GeometryError::Budget)?;
    let mut flat=Vec::new();flat.try_reserve_exact(count).map_err(|_|GeometryError::Budget)?;
    for bank in banks {
        for point in *bank {
            if cancel() { return Err(GeometryError::Cancelled.into()); }
            for (p,d) in [(point.source,source),(point.target,target)] {
                if p.iter().any(|v|!v.is_finite()) || p[0]<0. || p[1]<0.
                    || p[0]>=f64::from(d.0) || p[1]>=f64::from(d.1) {return Err(GeometryError::Invalid.into());}
            }
            flat.push(*point);
        }
    }
    let flat=credit.try_adopt(flat).map_err(|_|GeometryError::Budget)?;
    let mut best:Option<JointBankCandidate>=None;
    for (index,bank) in banks.iter().enumerate() {
        if cancel() { return Err(GeometryError::Cancelled.into()); }
        let Some(model)=verify_projective_sampled_joint_for_domains(bank,policy.hypothesis,policy.sampling,source,target,&cancel)? else {continue};
        let witnesses=collect_joint_projective_witnesses(&flat,model.transform,source,target,policy.witnesses,budget,&cancel)?;
        if witnesses.len()>=policy.minimum_witnesses {
            let candidate=JointBankCandidate{transform:model.transform,witnesses,hypothesis_bank:index};
            let accepted=qualify(&candidate);
            if cancel() {return Err(GeometryError::Cancelled.into());}
            if accepted? && best.as_ref().is_none_or(|old|candidate.witnesses.len()>old.witnesses.len()) {
                best=Some(candidate);
            }
        }
    }
    if cancel() {return Err(GeometryError::Cancelled.into());}
    Ok(best)
}
