//! Training-only projective hypotheses with one vote per endpoint component.
//! Geometry and coordinate separation do not establish copying or pixel independence.
use crate::{
    bank_partition::{BankPartitionEvidence, BankPartitionPolicy, partition_point_banks},
    geometry::{
        Correspondence, GeometryError, ProjectiveTransform, fit_projective_four, projective_domain_eligible,
    },
};
use rrrah_core::MemoryBudget;

#[derive(Debug, Clone, Copy)]
pub struct BankGeometryPolicy {
    pub trials: u64,
    pub seed: u64,
    pub minimum_fitting_components: usize,
    /// Two projections per fitting member per trial, plus every validation member once.
    pub maximum_projection_checks: u64,
    pub source_tolerance: f64,
    pub target_tolerance: f64,
}
#[derive(Debug, Clone, Copy)]
pub struct BankGeometryFit {
    pub transform: ProjectiveTransform,
    pub components: usize,
    /// Sum of the smallest jointly admitted normalized residual in each component.
    pub normalized_squared_error: f64,
    pub hypotheses: u64,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BankGeometryValidation {
    pub points: usize,
    pub components: usize,
    pub forward_components: usize,
    pub reverse_components: usize,
    /// A single member must satisfy both thresholds; separate members cannot combine votes.
    pub joint_components: usize,
    pub joint_points: usize,
    pub maximum_forward_error: f64,
    pub maximum_reverse_error: f64,
}
#[derive(Debug, Clone)]
pub struct BankGeometryEvidence {
    pub partition: BankPartitionEvidence,
    pub fitting: Option<BankGeometryFit>,
    pub validation: Option<BankGeometryValidation>,
    pub required_projection_checks: u64,
}

fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
fn errors(
    h: ProjectiveTransform,
    inverse: ProjectiveTransform,
    p: Correspondence,
) -> Result<(f64, f64), GeometryError> {
    let a = h.apply(p.source).ok_or(GeometryError::Invalid)?;
    let b = inverse.apply(p.target).ok_or(GeometryError::Invalid)?;
    let f = (a[0] - p.target[0]).hypot(a[1] - p.target[1]);
    let r = (b[0] - p.source[0]).hypot(b[1] - p.source[1]);
    if !f.is_finite() || !r.is_finite() {
        return Err(GeometryError::Invalid);
    }
    Ok((f, r))
}

/// Partition all alternatives into whole coordinate-alias components, sample
/// four different fitting components and one member from each, and rank by
/// fitting component support then error. Each fitting component contributes
/// at most one vote using its best jointly admitted member. No alternative is
/// discarded before hypothesis scoring. Deterministic sampling is approximate;
/// repeated subsets consume trials, and no confidence is certified.
///
/// The first winner of a count/error tie is retained. This API does not refine
/// the minimal four-point hypothesis. After all trials, its single selected
/// model is scored once on every validation member. Validation never enters
/// sampling, ranking or model replacement. No threshold-based duplicate verdict
/// occurs; absence of a model or validation support does not prove non-copying.
///
/// All input/domain checks and component construction use `partition_policy`.
/// Grouping scratch is managed and released before return; returned partition
/// credit lives with its last clone. Projection work is prepaid conservatively.
/// Domain-corner checks and four-point solves are additionally bounded by trials.
/// This does not bound total allocator metadata/RSS or prove neural/pixel context
/// independence. Caller must not tune repeated calls against validation results.
///
/// # Errors
/// Invalid policy/input, work or managed allocation refusal, cancellation or
/// numerical projection failure; no partial result is published.
#[allow(clippy::too_many_arguments)]
pub fn fit_and_validate_point_banks(
    banks: &[&[Correspondence]],
    source: (u32, u32),
    target: (u32, u32),
    partition_policy: BankPartitionPolicy,
    policy: BankGeometryPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<BankGeometryEvidence, GeometryError> {
    if cancel() {
        return Err(GeometryError::Cancelled);
    }
    if policy.trials == 0
        || policy.seed == 0
        || policy.minimum_fitting_components < 4
        || [policy.source_tolerance, policy.target_tolerance]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.)
    {
        return Err(GeometryError::Invalid);
    }
    let partition = partition_point_banks(banks, source, target, partition_policy, budget, &cancel)?;
    let required = u64::try_from(partition.fitting_points)
        .ok()
        .and_then(|n| n.checked_mul(policy.trials))
        .and_then(|n| {
            u64::try_from(partition.validation_points)
                .ok()
                .and_then(|v| n.checked_add(v))
        })
        .and_then(|n| n.checked_mul(2))
        .ok_or(GeometryError::Budget)?;
    if required > policy.maximum_projection_checks {
        return Err(GeometryError::Budget);
    }
    let count = partition.components;
    let n = partition.assignments.len();
    let fit_components = count.div_ceil(2);
    if fit_components < policy.minimum_fitting_components {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        return Ok(BankGeometryEvidence {
            partition,
            fitting: None,
            validation: None,
            required_projection_checks: required,
        });
    }
    let offsets_len = count.checked_add(1).ok_or(GeometryError::Budget)?;
    let bytes = |len: usize| {
        u64::try_from(len)
            .ok()
            .and_then(|n| n.checked_mul(std::mem::size_of::<usize>() as u64))
            .ok_or(GeometryError::Budget)
    };
    // Acquire every grouping credit before allocating any grouping buffer.
    let grouping_credit = budget.try_reserve(bytes(n)?).map_err(|_| GeometryError::Budget)?;
    let offsets_credit = budget
        .try_reserve(bytes(offsets_len)?)
        .map_err(|_| GeometryError::Budget)?;
    let cursors_credit = budget
        .try_reserve(bytes(count)?)
        .map_err(|_| GeometryError::Budget)?;
    let mut grouped = grouping_credit
        .try_buffer(n, 0usize)
        .map_err(|_| GeometryError::Budget)?;
    let mut offsets = offsets_credit
        .try_buffer(offsets_len, 0usize)
        .map_err(|_| GeometryError::Budget)?;
    let mut cursors = cursors_credit
        .try_buffer(count, 0usize)
        .map_err(|_| GeometryError::Budget)?;
    for a in partition.assignments.iter() {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        offsets[a.component + 1] += 1;
    }
    for i in 0..count {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        offsets[i + 1] += offsets[i];
        cursors[i] = offsets[i];
    }
    for (i, a) in partition.assignments.iter().enumerate() {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        grouped[cursors[a.component]] = i;
        cursors[a.component] += 1;
    }
    drop(cursors);
    let point = |group_index: usize| {
        let a = partition.assignments[grouped[group_index]];
        banks[a.bank][a.point]
    };
    let mut state = policy.seed;
    let mut best: Option<BankGeometryFit> = None;
    for _ in 0..policy.trials {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        let mut selected = [0usize; 4];
        for i in 0..4 {
            if cancel() {
                return Err(GeometryError::Cancelled);
            }
            let mut draw = (next(&mut state) % (fit_components - i) as u64) as usize;
            selected[..i].sort_unstable();
            for old in &selected[..i] {
                if draw >= *old {
                    draw += 1;
                }
            }
            selected[i] = draw;
        }
        let sample = selected.map(|c| {
            let c = c * 2;
            let member = (next(&mut state) % (offsets[c + 1] - offsets[c]) as u64) as usize;
            point(offsets[c] + member)
        });
        let h = match fit_projective_four(&sample, &cancel) {
            Ok(h) => h,
            Err(GeometryError::Invalid) => continue,
            Err(e) => return Err(e),
        };
        if !projective_domain_eligible(h, source, target) {
            continue;
        }
        let inverse = h.inverse()?;
        let mut components = 0usize;
        let mut total = 0.;
        for c in (0..count).step_by(2) {
            let mut minimum = f64::INFINITY;
            for i in offsets[c]..offsets[c + 1] {
                if cancel() {
                    return Err(GeometryError::Cancelled);
                }
                let (f, r) = errors(h, inverse, point(i))?;
                if f <= policy.target_tolerance && r <= policy.source_tolerance {
                    minimum = minimum
                        .min((f / policy.target_tolerance).powi(2) + (r / policy.source_tolerance).powi(2));
                }
            }
            if minimum.is_finite() {
                components += 1;
                total += minimum;
            }
        }
        if components >= policy.minimum_fitting_components
            && best.as_ref().is_none_or(|old| {
                components > old.components
                    || (components == old.components && total < old.normalized_squared_error)
            })
        {
            best = Some(BankGeometryFit {
                transform: h,
                components,
                normalized_squared_error: total,
                hypotheses: policy.trials,
            });
        }
    }
    let validation = if let Some(fit) = best {
        let inverse = fit.transform.inverse()?;
        let mut e = BankGeometryValidation {
            points: partition.validation_points,
            components: count / 2,
            forward_components: 0,
            reverse_components: 0,
            joint_components: 0,
            joint_points: 0,
            maximum_forward_error: 0.,
            maximum_reverse_error: 0.,
        };
        for c in (1..count).step_by(2) {
            let (mut forward, mut reverse, mut joint) = (false, false, false);
            for i in offsets[c]..offsets[c + 1] {
                if cancel() {
                    return Err(GeometryError::Cancelled);
                }
                let (f, r) = errors(fit.transform, inverse, point(i))?;
                e.maximum_forward_error = e.maximum_forward_error.max(f);
                e.maximum_reverse_error = e.maximum_reverse_error.max(r);
                let (a, b) = (f <= policy.target_tolerance, r <= policy.source_tolerance);
                forward |= a;
                reverse |= b;
                joint |= a && b;
                e.joint_points += usize::from(a && b);
            }
            e.forward_components += usize::from(forward);
            e.reverse_components += usize::from(reverse);
            e.joint_components += usize::from(joint);
        }
        Some(e)
    } else {
        None
    };
    drop(grouped);
    drop(offsets);
    if cancel() {
        return Err(GeometryError::Cancelled);
    }
    Ok(BankGeometryEvidence {
        partition,
        fitting: best,
        validation,
        required_projection_checks: required,
    })
}
