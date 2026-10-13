//! Guarded selected-frame anchor support under caller-supplied geometry.
use crate::{
    anchor_rank::{AnchorRankEvidence, AnchorRankPolicy, compare_anchor_rank},
    animated::AnimationBudget,
    decode::{CachedError, DecodeSourceSnapshot, decode_selected_frame_bounded},
    exact::SnapshotError,
    geometry::{Correspondence, ProjectiveTransform},
    local_rank::LocalRankError,
};
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;

/// Cumulative admission for three proposal and original-pixel anchor attempts.
#[derive(Debug, Clone, Copy)]
pub struct AnchorGeometryFallbackPolicy {
    pub search: crate::local_scan::ProjectiveCandidateUnionFallbackFilePolicy,
    pub anchors: AnchorRankPolicy,
    pub maximum_total_point_checks: u64,
    pub maximum_total_selection_checks: u64,
    pub maximum_total_rank_sites: u64,
    pub maximum_total_pixel_reads: u64,
}

#[derive(Debug)]
pub struct AnchorGeometryFallbackEvidence {
    pub evidence: AnchorGeometrySearchEvidence,
    pub smoothing_radii: [u32; 2],
    pub attempted_recipes: u32,
}

/// Explicit six-attempt portfolio: three ordinary, then three reflected-source
/// recipes. All cumulative fields in `search` must cover six phase maxima.
/// Reflection bins cover both feature sets of each of three reflected phases.
#[derive(Debug, Clone, Copy)]
pub struct AnchorReflectionFallbackPolicy {
    pub search: AnchorGeometryFallbackPolicy,
    pub maximum_reflection_bins: u64,
}

#[derive(Debug)]
pub struct AnchorReflectionFallbackEvidence {
    pub evidence: AnchorGeometrySearchEvidence,
    pub smoothing_radii: [u32; 2],
    pub attempted_recipes: u32,
    pub reflected_source: bool,
}

pub(crate) fn validate_anchor_reflection_fallback_policy(
    policy: AnchorReflectionFallbackPolicy,
    factors: &[f64],
    cancel: &impl Fn() -> bool,
) -> Result<(), AnchorRankFileError> {
    validate_anchor_geometry_fallback_policy(policy.search, factors, cancel)?;
    let base = policy.search;
    let phase = base.search.search;
    for (per_attempt, total) in [
        (
            base.anchors.local.maximum_point_checks,
            base.maximum_total_point_checks,
        ),
        (
            base.anchors.maximum_selection_checks,
            base.maximum_total_selection_checks,
        ),
        (
            base.anchors.local.rank.maximum_sites,
            base.maximum_total_rank_sites,
        ),
        (
            base.anchors.local.rank.maximum_pixel_reads,
            base.maximum_total_pixel_reads,
        ),
        (
            phase.max_total_gradient_samples,
            base.search.max_total_gradient_samples,
        ),
        (
            phase.max_total_match_comparisons,
            base.search.max_total_match_comparisons,
        ),
        (
            phase.max_union_comparisons,
            base.search.max_total_union_comparisons,
        ),
    ] {
        if per_attempt.checked_mul(6).ok_or(LocalRankError::Budget)? > total {
            return Err(LocalRankError::Budget.into());
        }
    }
    let reflection = u64::try_from(phase.search.gradient.search.max_total_features)
        .map_err(|_| LocalRankError::Budget)?
        .checked_mul(768)
        .ok_or(LocalRankError::Budget)?;
    if reflection > policy.maximum_reflection_bins {
        return Err(LocalRankError::Budget.into());
    }
    Ok(())
}

/// Select the first original-pixel-supported recipe, ordinary before reflected.
/// Shared source/dependency snapshots span every attempt. Work is prepaid for
/// all six phases. Insufficient geometry permits the next recipe; other errors stop.
/// Unsupported buffers release before the next phase. Local support is not identity.
pub fn search_anchor_rank_reflection_fallback_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: AnchorReflectionFallbackPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AnchorReflectionFallbackEvidence, AnchorRankFileError> {
    let latch = std::cell::Cell::new(false);
    let cancelled = || {
        let value = latch.get()
            || cancel()
            || [left, right].iter().any(|r| {
                r.cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(value);
        value
    };
    let result = (|| {
        validate_anchor_reflection_fallback_policy(policy, factors, &cancelled)?;
        let base = policy.search;
        let phase = base.search.search;
        let decode = phase.search.gradient.search.local.decode;
        let first = DecodeSourceSnapshot::read(&left.path, decode.max_file_bytes, &cancelled)?;
        let second = DecodeSourceSnapshot::read(&right.path, decode.max_file_bytes, &cancelled)?;
        let radii = [
            [phase.smoothing_radius; 2],
            [base.search.asymmetric_radius, 0],
            [0, base.search.asymmetric_radius],
        ];
        let mut inconclusive = false;
        for index in 0..6 {
            let reflected_source = index >= 3;
            let smoothing_radii = radii[index % 3];
            let evidence = search_anchor_rank_geometry_files_selected(
                left,
                right,
                phase,
                factors,
                smoothing_radii,
                base.anchors,
                reflected_source.then_some(policy.maximum_reflection_bins / 3),
                budget,
                &cancelled,
            );
            let evidence = match evidence {
                Ok(evidence) => evidence,
                Err(AnchorRankFileError::Rank(LocalRankError::InsufficientGeometry)) => {
                    first.verify(&cancelled)?;
                    second.verify(&cancelled)?;
                    inconclusive = true;
                    if index == 5 {
                        return Ok((Err(LocalRankError::InsufficientGeometry.into()), first, second));
                    }
                    continue;
                }
                Err(error) => return Err(error),
            };
            first.verify(&cancelled)?;
            second.verify(&cancelled)?;
            if index == 5 || evidence.anchors.is_some_and(|a| a.supported) {
                let supported = evidence.anchors.is_some_and(|a| a.supported);
                let outcome = if !supported && inconclusive {
                    Err(LocalRankError::InsufficientGeometry.into())
                } else {
                    Ok(AnchorReflectionFallbackEvidence {
                        evidence,
                        smoothing_radii,
                        attempted_recipes: (index + 1) as u32,
                        reflected_source,
                    })
                };
                return Ok((outcome, first, second));
            }
            drop(evidence);
        }
        unreachable!("six recipes produce a terminal outcome")
    })();
    finish_source_guard(result, left, right, &latch, cancelled())?
}

pub(crate) fn validate_anchor_geometry_fallback_policy(
    policy: AnchorGeometryFallbackPolicy,
    factors: &[f64],
    cancel: &impl Fn() -> bool,
) -> Result<(), AnchorRankFileError> {
    if cancel() {
        return Err(AnchorRankFileError::Cancelled);
    }
    crate::anchor_rank::validate_anchor_rank_policy(policy.anchors)?;
    crate::local_scan::validate_candidate_union_fallback_file_policy(policy.search, factors, cancel)?;
    // Every attempt enforces these per-phase maxima; admitting all three
    // maxima before IO therefore bounds their cumulative work conservatively.
    for (per_attempt, total) in [
        (
            policy.anchors.local.maximum_point_checks,
            policy.maximum_total_point_checks,
        ),
        (
            policy.anchors.maximum_selection_checks,
            policy.maximum_total_selection_checks,
        ),
        (
            policy.anchors.local.rank.maximum_sites,
            policy.maximum_total_rank_sites,
        ),
        (
            policy.anchors.local.rank.maximum_pixel_reads,
            policy.maximum_total_pixel_reads,
        ),
    ] {
        if per_attempt.checked_mul(3).ok_or(LocalRankError::Budget)? > total {
            return Err(LocalRankError::Budget.into());
        }
    }
    Ok(())
}

/// Try symmetric, source-smoothed and target-smoothed proposals in that order.
/// Original-pixel anchor support selects the first successful attempt. Insufficient
/// geometry permits the next recipe and remains an error if none supports. Other
/// errors stop the operation. Unsuccessful attempt buffers
/// are released before the next attempt, and source guards span every attempt.
/// Without support, returns the final negative evidence only if all recipes
/// completed conclusively; otherwise returns insufficient geometry.
/// Returned local support never establishes whole-image duplicate identity.
pub fn search_anchor_rank_geometry_fallback_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: AnchorGeometryFallbackPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AnchorGeometryFallbackEvidence, AnchorRankFileError> {
    let latch = std::cell::Cell::new(false);
    let cancelled = || {
        let value = latch.get()
            || cancel()
            || [left, right].iter().any(|r| {
                r.cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(value);
        value
    };
    let result = (|| {
        validate_anchor_geometry_fallback_policy(policy, factors, &cancelled)?;
        let search = policy.search;
        let decode = search.search.search.gradient.search.local.decode;
        let first = DecodeSourceSnapshot::read(&left.path, decode.max_file_bytes, &cancelled)?;
        let second = DecodeSourceSnapshot::read(&right.path, decode.max_file_bytes, &cancelled)?;
        let mut inconclusive = false;
        for (index, radii) in [
            [search.search.smoothing_radius; 2],
            [search.asymmetric_radius, 0],
            [0, search.asymmetric_radius],
        ]
        .into_iter()
        .enumerate()
        {
            let evidence = search_anchor_rank_geometry_files_with_smoothing(
                left,
                right,
                search.search,
                factors,
                radii,
                policy.anchors,
                budget,
                &cancelled,
            );
            let evidence = match evidence {
                Ok(evidence) => evidence,
                Err(AnchorRankFileError::Rank(LocalRankError::InsufficientGeometry)) => {
                    first.verify(&cancelled)?;
                    second.verify(&cancelled)?;
                    inconclusive = true;
                    if index == 2 {
                        return Ok((Err(LocalRankError::InsufficientGeometry.into()), first, second));
                    }
                    continue;
                }
                Err(error) => return Err(error),
            };
            first.verify(&cancelled)?;
            second.verify(&cancelled)?;
            if index == 2 || evidence.anchors.is_some_and(|a| a.supported) {
                let supported = evidence.anchors.is_some_and(|a| a.supported);
                let outcome = if !supported && inconclusive {
                    Err(LocalRankError::InsufficientGeometry.into())
                } else {
                    Ok(AnchorGeometryFallbackEvidence {
                        evidence,
                        smoothing_radii: radii,
                        attempted_recipes: (index + 1) as u32,
                    })
                };
                return Ok((outcome, first, second));
            }
            drop(evidence);
        }
        unreachable!("three recipes always produce a terminal outcome")
    })();
    finish_source_guard(result, left, right, &latch, cancelled())?
}

#[derive(Debug, thiserror::Error)]
pub enum AnchorRankFileError {
    #[error("anchor file comparison cancelled")]
    Cancelled,
    #[error(transparent)]
    Source(#[from] SnapshotError),
    #[error(transparent)]
    Decode(#[from] CachedError),
    #[error(transparent)]
    Rank(#[from] LocalRankError),
    #[error(transparent)]
    Search(#[from] crate::local_scan::LocalFileError),
}

/// Exact capability refusal retained for an explicitly configured alternate
/// verifier. This is neither a normal negative result nor malformed input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePixelRefusal {
    GradientOpacity,
    RankPixelRange,
}
impl AnchorRankFileError {
    /// Identify only the native recipe's typed pixel-domain refusals. Policy,
    /// geometry, work, source, decode and cancellation errors remain terminal.
    /// Insufficient geometric witnesses are a separate inconclusive reason.
    pub fn pixel_refusal(&self) -> Option<NativePixelRefusal> {
        use crate::{local::LocalError, local_scan::LocalFileError, rank_region::RankRegionError};
        match self {
            Self::Search(LocalFileError::Features(LocalError::UnsupportedPixels)) => {
                Some(NativePixelRefusal::GradientOpacity)
            }
            Self::Rank(LocalRankError::Rank(RankRegionError::UnsupportedPixels))
            | Self::Search(LocalFileError::Rank(LocalRankError::Rank(RankRegionError::UnsupportedPixels))) => {
                Some(NativePixelRefusal::RankPixelRange)
            }
            _ => None,
        }
    }
}

/// Invoke this only after the last caller checkpoint. Source observations must
/// never be followed by more caller code; owner cancellation still has priority.
fn finish_source_guard<T>(
    result: Result<(T, DecodeSourceSnapshot, DecodeSourceSnapshot), AnchorRankFileError>,
    left: &DecodeRequest,
    right: &DecodeRequest,
    latch: &std::cell::Cell<bool>,
    caller_cancelled: bool,
) -> Result<T, AnchorRankFileError> {
    if caller_cancelled {
        return Err(AnchorRankFileError::Cancelled);
    }
    let (outcome, first, second) = result?;
    let owners = || {
        let stopped = latch.get()
            || [left, right].iter().any(|request| {
                request
                    .cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(stopped);
        stopped
    };
    let verified = (|| {
        first.verify(&owners)?;
        second.verify(&owners)?;
        Ok(outcome)
    })();
    if owners() {
        Err(AnchorRankFileError::Cancelled)
    } else {
        verified
    }
}

/// Snapshots include external decoder dependencies and enclose both decodes and
/// pixel comparison. Cancellation is sticky, including request generation tokens.
/// All supplied points are preserved; evidence is local support, never identity.
#[allow(clippy::too_many_arguments)]
pub fn compare_anchor_rank_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    model: ProjectiveTransform,
    points: &[Correspondence],
    decode: AnimationBudget,
    policy: AnchorRankPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AnchorRankEvidence, AnchorRankFileError> {
    compare_anchor_rank_files_impl(left, right, model, points, decode, policy, None, budget, cancel)
        .map(|(anchors, _)| anchors)
}

/// Local ordinal evidence after explicitly recorded uniform exposure calibration.
/// This can accept unrelated smooth regions; geometry and negative precision must
/// be separately qualified. No default search path selects this verifier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExposureAnchorRankEvidence {
    pub anchors: AnchorRankEvidence,
    pub exposures: [crate::exposure::RankExposure; 2],
}

/// Guard both original sources and decoder dependencies across decode,
/// calibration and pixel verification. Scratch has managed admission and fallible
/// allocation. `maximum_calibration_pixel_reads` must cover six times the decode
/// pixel cap before I/O (three passes per image). All existing rank work caps and
/// thresholds still apply. Calibration refuses alpha/negative/zero RGB explicitly.
#[allow(clippy::too_many_arguments)]
pub fn compare_exposure_anchor_rank_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    model: ProjectiveTransform,
    points: &[Correspondence],
    decode: AnimationBudget,
    policy: AnchorRankPolicy,
    maximum_calibration_pixel_reads: u64,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ExposureAnchorRankEvidence, AnchorRankFileError> {
    compare_anchor_rank_files_impl(
        left,
        right,
        model,
        points,
        decode,
        policy,
        Some(maximum_calibration_pixel_reads),
        budget,
        cancel,
    )
    .map(|(anchors, exposures)| ExposureAnchorRankEvidence {
        anchors,
        exposures: exposures.expect("explicit calibration returns exposure metadata"),
    })
}

#[allow(clippy::too_many_arguments)]
fn compare_anchor_rank_files_impl(
    left: &DecodeRequest,
    right: &DecodeRequest,
    model: ProjectiveTransform,
    points: &[Correspondence],
    decode: AnimationBudget,
    policy: AnchorRankPolicy,
    maximum_calibration_pixel_reads: Option<u64>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<(AnchorRankEvidence, Option<[crate::exposure::RankExposure; 2]>), AnchorRankFileError> {
    let latch = std::cell::Cell::new(false);
    let cancelled = || {
        let value = latch.get()
            || cancel()
            || [left, right].iter().any(|r| {
                r.cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(value);
        value
    };
    let result = (|| {
        if cancelled() {
            return Err(AnchorRankFileError::Cancelled);
        }
        crate::anchor_rank::validate_anchor_rank_policy(policy)?;
        if let Some(limit) = maximum_calibration_pixel_reads {
            if decode.max_pixels.checked_mul(6).ok_or(LocalRankError::Budget)? > limit {
                return Err(LocalRankError::Budget.into());
            }
        }
        let n = u64::try_from(points.len()).map_err(|_| LocalRankError::Budget)?;
        let pairs = n
            .checked_mul(n.saturating_sub(1))
            .and_then(|v| v.checked_div(2))
            .ok_or(LocalRankError::Budget)?;
        let size = policy
            .window_radius
            .checked_mul(2)
            .and_then(|v| v.checked_add(1))
            .ok_or(LocalRankError::Budget)?;
        let pad = policy
            .local
            .rank
            .radius
            .checked_add(policy.local.filter_radius)
            .ok_or(LocalRankError::Budget)?;
        let extent = u64::from(size)
            .checked_add(2 * u64::from(pad))
            .ok_or(LocalRankError::Budget)?;
        let sites = n
            .checked_mul(2)
            .and_then(|v| v.checked_mul(extent))
            .and_then(|v| v.checked_mul(extent))
            .ok_or(LocalRankError::Budget)?;
        if points.len() > policy.local.maximum_points
            || pairs.checked_add(n).ok_or(LocalRankError::Budget)? > policy.local.maximum_point_checks
            || pairs
                .checked_mul(3)
                .and_then(|v| v.checked_add(n))
                .ok_or(LocalRankError::Budget)?
                > policy.maximum_selection_checks
            || sites > policy.local.rank.maximum_sites
            || sites.checked_mul(5).ok_or(LocalRankError::Budget)? > policy.local.rank.maximum_pixel_reads
        {
            return Err(LocalRankError::Budget.into());
        }
        let first = DecodeSourceSnapshot::read(&left.path, decode.max_file_bytes, &cancelled)?;
        let second = DecodeSourceSnapshot::read(&right.path, decode.max_file_bytes, &cancelled)?;
        let a = decode_selected_frame_bounded(left, decode, budget, &cancelled)?;
        let b = decode_selected_frame_bounded(right, decode, budget, &cancelled)?;
        let av = a
            .view(&cancelled)
            .map_err(crate::decode::FileError::from)
            .map_err(CachedError::from)?;
        let bv = b
            .view(&cancelled)
            .map_err(crate::decode::FileError::from)
            .map_err(CachedError::from)?;
        let (evidence, exposures) = if maximum_calibration_pixel_reads.is_some() {
            let samples = |view: &crate::linear::LinearRgbaView<'_>| {
                let (w, h) = view.dimensions();
                (u64::from(w) * u64::from(h))
                    .checked_mul(4)
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or(LocalRankError::Budget)
            };
            let an = samples(&av)?;
            let bn = samples(&bv)?;
            let _ac = budget
                .try_reserve((an as u64).checked_mul(4).ok_or(LocalRankError::Budget)?)
                .map_err(|_| LocalRankError::Budget)?;
            let _bc = budget
                .try_reserve((bn as u64).checked_mul(4).ok_or(LocalRankError::Budget)?)
                .map_err(|_| LocalRankError::Budget)?;
            let mut ab = Vec::new();
            let mut bb = Vec::new();
            for (buffer, count) in [(&mut ab, an), (&mut bb, bn)] {
                buffer
                    .try_reserve_exact(count)
                    .map_err(|_| LocalRankError::Budget)?;
                while buffer.len() < count {
                    if cancelled() {
                        return Err(AnchorRankFileError::Cancelled);
                    }
                    buffer.resize(buffer.len().saturating_add(4096).min(count), 0.0);
                }
            }
            let map_error = |e| {
                use crate::{exposure::ExposureError as E, rank_region::RankRegionError as R};
                match e {
                    E::Cancelled => LocalRankError::Cancelled,
                    E::Budget => LocalRankError::Budget,
                    E::Layout => LocalRankError::Invalid,
                    E::UnsupportedPixels => LocalRankError::Rank(R::UnsupportedPixels),
                    E::Uninformative => LocalRankError::Rank(R::Uninformative),
                }
            };
            let (a, ae) =
                crate::exposure::calibrate_rank_exposure_into(&av, &mut ab, (an as u64 / 4) * 3, &cancelled)
                    .map_err(map_error)?;
            let (b, be) =
                crate::exposure::calibrate_rank_exposure_into(&bv, &mut bb, (bn as u64 / 4) * 3, &cancelled)
                    .map_err(map_error)?;
            (
                compare_anchor_rank(&a, &b, model, points, policy, budget, &cancelled)?,
                Some([ae, be]),
            )
        } else {
            (
                compare_anchor_rank(&av, &bv, model, points, policy, budget, &cancelled)?,
                None,
            )
        };
        first.verify(&cancelled)?;
        second.verify(&cancelled)?;
        Ok(((evidence, exposures), first, second))
    })();
    finish_source_guard(result, left, right, &latch, cancelled())
}

#[derive(Debug)]
pub struct AnchorCandidateSearchEvidence {
    pub search: crate::local_scan::ManagedProjectiveCandidateUnionRegionsFileEvidence,
    pub anchors: Option<AnchorRankEvidence>,
}

/// Fresh candidate extraction and geometry followed by original-pixel anchors.
/// Outer source/dependency snapshots enclose both phases; geometry cannot cross
/// source revisions. Every original union correspondence is supplied unchanged.
/// Returned local support is not a whole-file duplicate decision.
#[allow(clippy::too_many_arguments)]
pub fn search_anchor_rank_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    search: crate::local_scan::ProjectiveCandidateUnionRegionsFilePolicy,
    factors: &[f64],
    policy: AnchorRankPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AnchorCandidateSearchEvidence, AnchorRankFileError> {
    let latch = std::cell::Cell::new(false);
    let cancelled = || {
        let value = latch.get()
            || cancel()
            || [left, right].iter().any(|r| {
                r.cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(value);
        value
    };
    let result = (|| {
        if cancelled() {
            return Err(AnchorRankFileError::Cancelled);
        }
        crate::anchor_rank::validate_anchor_rank_policy(policy)?;
        let decode = search.search.gradient.search.local.decode;
        let first = DecodeSourceSnapshot::read(&left.path, decode.max_file_bytes, &cancelled)?;
        let second = DecodeSourceSnapshot::read(&right.path, decode.max_file_bytes, &cancelled)?;
        let evidence = crate::local_scan::compare_local_files_projective_candidate_union_regions_managed(
            left, right, search, factors, budget, &cancelled,
        )?;
        let anchors = if let Some(geometry) = &evidence.geometry {
            Some(compare_anchor_rank_files(
                left,
                right,
                geometry.transform,
                &evidence.correspondences,
                decode,
                policy,
                budget,
                &cancelled,
            )?)
        } else {
            None
        };
        first.verify(&cancelled)?;
        second.verify(&cancelled)?;
        Ok((
            AnchorCandidateSearchEvidence {
                search: evidence,
                anchors,
            },
            first,
            second,
        ))
    })();
    finish_source_guard(result, left, right, &latch, cancelled())
}

#[derive(Debug)]
pub struct AnchorFallbackSearchEvidence {
    pub search: crate::local_scan::ManagedProjectiveCandidateUnionFallbackFileEvidence,
    pub anchors: Option<AnchorRankEvidence>,
}
/// Existing color-confirmed symmetric/asymmetric fallback chooses geometry;
/// original-pixel anchor comparison then evaluates its complete point buffer.
/// Outer snapshots span all candidate attempts and anchor confirmation. Recipe
/// metadata and original color evidence are retained; no identity decision.
#[allow(clippy::too_many_arguments)]
pub fn search_anchor_rank_fallback_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    search: crate::local_scan::ProjectiveCandidateUnionFallbackFilePolicy,
    factors: &[f64],
    policy: AnchorRankPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AnchorFallbackSearchEvidence, AnchorRankFileError> {
    let latch = std::cell::Cell::new(false);
    let cancelled = || {
        let value = latch.get()
            || cancel()
            || [left, right].iter().any(|r| {
                r.cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(value);
        value
    };
    let result = (|| {
        if cancelled() {
            return Err(AnchorRankFileError::Cancelled);
        }
        crate::anchor_rank::validate_anchor_rank_policy(policy)?;
        let decode = search.search.search.gradient.search.local.decode;
        let first = DecodeSourceSnapshot::read(&left.path, decode.max_file_bytes, &cancelled)?;
        let second = DecodeSourceSnapshot::read(&right.path, decode.max_file_bytes, &cancelled)?;
        let evidence =
            crate::local_scan::compare_local_files_projective_candidate_union_fallback_regions_managed(
                left, right, search, factors, budget, &cancelled,
            )?;
        let anchors = if let Some(geometry) = &evidence.evidence.geometry {
            Some(compare_anchor_rank_files(
                left,
                right,
                geometry.transform,
                &evidence.evidence.correspondences,
                decode,
                policy,
                budget,
                &cancelled,
            )?)
        } else {
            None
        };
        first.verify(&cancelled)?;
        second.verify(&cancelled)?;
        Ok((
            AnchorFallbackSearchEvidence {
                search: evidence,
                anchors,
            },
            first,
            second,
        ))
    })();
    finish_source_guard(result, left, right, &latch, cancelled())
}

#[derive(Debug)]
pub struct AnchorGeometrySearchEvidence {
    pub search: crate::local_scan::ManagedCandidateUnionGeometryFileEvidence,
    pub anchors: Option<AnchorRankEvidence>,
}
/// Same native proposals and geometry, with anchor pixels as the only local
/// confirmation stage. Legacy regional color fitting is not run or reported.
/// Outer source/dependency guards span both phases; this remains local support.
#[allow(clippy::too_many_arguments)]
pub fn search_anchor_rank_geometry_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    search: crate::local_scan::ProjectiveCandidateUnionRegionsFilePolicy,
    factors: &[f64],
    policy: AnchorRankPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AnchorGeometrySearchEvidence, AnchorRankFileError> {
    search_anchor_rank_geometry_files_with_smoothing(
        left,
        right,
        search,
        factors,
        [search.smoothing_radius; 2],
        policy,
        budget,
        cancel,
    )
}

/// Explicit ordered candidate smoothing; original-pixel anchor policy is unchanged.
/// Source guards span proposal and confirmation. No whole-image identity is implied.
#[allow(clippy::too_many_arguments)]
pub fn search_anchor_rank_geometry_files_with_smoothing(
    left: &DecodeRequest,
    right: &DecodeRequest,
    search: crate::local_scan::ProjectiveCandidateUnionRegionsFilePolicy,
    factors: &[f64],
    smoothing_radii: [u32; 2],
    policy: AnchorRankPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AnchorGeometrySearchEvidence, AnchorRankFileError> {
    search_anchor_rank_geometry_files_selected(
        left,
        right,
        search,
        factors,
        smoothing_radii,
        policy,
        None,
        budget,
        cancel,
    )
}

/// Automatic reflected-source proposals and original-pixel anchor confirmation.
/// Shared source/dependency snapshots and sticky cancellation span all phases.
/// Reflection work is admitted before IO; typed refusals never become misses.
/// This explicit recipe proves finite local support, not whole-image identity.
#[allow(clippy::too_many_arguments)]
pub fn search_anchor_rank_reflected_geometry_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    search: crate::local_scan::ProjectiveCandidateUnionRegionsFilePolicy,
    factors: &[f64],
    smoothing_radii: [u32; 2],
    policy: AnchorRankPolicy,
    maximum_reflection_bins: u64,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AnchorGeometrySearchEvidence, AnchorRankFileError> {
    search_anchor_rank_geometry_files_selected(
        left,
        right,
        search,
        factors,
        smoothing_radii,
        policy,
        Some(maximum_reflection_bins),
        budget,
        cancel,
    )
}

#[allow(clippy::too_many_arguments)]
fn search_anchor_rank_geometry_files_selected(
    left: &DecodeRequest,
    right: &DecodeRequest,
    search: crate::local_scan::ProjectiveCandidateUnionRegionsFilePolicy,
    factors: &[f64],
    smoothing_radii: [u32; 2],
    policy: AnchorRankPolicy,
    reflection_bins: Option<u64>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AnchorGeometrySearchEvidence, AnchorRankFileError> {
    let latch = std::cell::Cell::new(false);
    let cancelled = || {
        let value = latch.get()
            || cancel()
            || [left, right].iter().any(|r| {
                r.cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(value);
        value
    };
    let result = (|| {
        if cancelled() {
            return Err(AnchorRankFileError::Cancelled);
        }
        crate::anchor_rank::validate_anchor_rank_policy(policy)?;
        if let Some(limit) = reflection_bins {
            crate::local_scan::validate_candidate_union_regions_file_policy(search, factors, &cancelled)?;
            crate::local_scan::validate_candidate_reflection_bins(search, limit)?;
        }
        let decode = search.search.gradient.search.local.decode;
        let first = DecodeSourceSnapshot::read(&left.path, decode.max_file_bytes, &cancelled)?;
        let second = DecodeSourceSnapshot::read(&right.path, decode.max_file_bytes, &cancelled)?;
        let evidence = if let Some(limit) = reflection_bins {
            crate::local_scan::find_projective_candidate_union_reflected_geometry_files_managed(
                left,
                right,
                search,
                factors,
                smoothing_radii,
                limit,
                budget,
                &cancelled,
            )?
        } else {
            crate::local_scan::find_projective_candidate_union_geometry_files_managed(
                left,
                right,
                search,
                factors,
                smoothing_radii,
                budget,
                &cancelled,
            )?
        };
        let anchors = if let Some(geometry) = &evidence.geometry {
            Some(compare_anchor_rank_files(
                left,
                right,
                geometry.transform,
                &evidence.correspondences,
                decode,
                policy,
                budget,
                &cancelled,
            )?)
        } else {
            None
        };
        first.verify(&cancelled)?;
        second.verify(&cancelled)?;
        Ok((
            AnchorGeometrySearchEvidence {
                search: evidence,
                anchors,
            },
            first,
            second,
        ))
    })();
    finish_source_guard(result, left, right, &latch, cancelled())
}

/// Explicit six-recipe recovery from proposals extracted with the inputs reversed.
/// These limits are additional to any ordinary six-recipe search the caller runs.
#[derive(Debug, Clone, Copy)]
pub struct OppositeAnchorSearchPolicy {
    pub search: AnchorReflectionFallbackPolicy,
    /// Must admit the native recipe's complete maximum correspondence payload.
    pub maximum_swapped_points: usize,
}

/// Caller-direction points and independently verified original pixels.
/// Opposite-direction fit inliers are deliberately not relabelled as caller inliers.
#[derive(Debug)]
pub struct OppositeAnchorSearchEvidence {
    pub correspondences: rrrah_core::SharedBuffer<Correspondence>,
    pub transform: Option<ProjectiveTransform>,
    pub anchors: Option<AnchorRankEvidence>,
    pub attempted_recipes: u32,
    /// Reflection was applied to the proposal source, i.e. the caller's right input.
    pub reflected_proposal_source: bool,
}

/// Search the opposite input direction and independently confirm inverted proposals.
/// Caller-direction anchor thresholds remain unchanged. All six recipes, reflected
/// descriptor bins and maximum swapped-point storage are admitted before file IO.
/// No opposite-direction pixel decision or fit-inlier metadata is reused.
///
/// # Errors
/// Policy/work/memory limits, cancellation, source changes, decode errors, or
/// insufficient anchor geometry in any unresolved recipe. No partial result escapes.
pub fn search_anchor_rank_opposite_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: OppositeAnchorSearchPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<OppositeAnchorSearchEvidence, AnchorRankFileError> {
    search_anchor_rank_opposite_files_prepaid(left, right, policy, factors, budget, None, cancel)
}

pub(crate) fn validate_opposite_policy(
    policy: OppositeAnchorSearchPolicy,
    factors: &[f64],
    cancel: &impl Fn() -> bool,
) -> Result<u64, AnchorRankFileError> {
    validate_anchor_reflection_fallback_policy(policy.search, factors, cancel)?;
    if policy.maximum_swapped_points < policy.search.search.search.search.max_union_points {
        return Err(LocalRankError::Budget.into());
    }
    policy
        .maximum_swapped_points
        .checked_mul(std::mem::size_of::<Correspondence>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(|| LocalRankError::Budget.into())
}

/// Joint fitting in caller coordinates, with six native extraction recipes.
/// Projection admission is cumulative and additional to native extraction limits.
/// No target-only fit is run in this mode. Original pixels still decide support.
#[derive(Debug, Clone, Copy)]
pub struct JointOppositeAnchorSearchPolicy {
    pub opposite: OppositeAnchorSearchPolicy,
    pub maximum_total_projection_checks: u64,
}

pub fn search_anchor_rank_joint_opposite_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: JointOppositeAnchorSearchPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<OppositeAnchorSearchEvidence, AnchorRankFileError> {
    search_anchor_rank_opposite_files_selected(
        left,
        right,
        policy.opposite,
        factors,
        budget,
        None,
        Some(policy.maximum_total_projection_checks),
        cancel,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn search_anchor_rank_opposite_files_prepaid(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: OppositeAnchorSearchPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    prepaid: Option<rrrah_core::Reservation>,
    cancel: impl Fn() -> bool,
) -> Result<OppositeAnchorSearchEvidence, AnchorRankFileError> {
    search_anchor_rank_opposite_files_selected(left, right, policy, factors, budget, prepaid, None, cancel)
}

pub(crate) fn validate_joint_opposite_projection_policy(
    policy: OppositeAnchorSearchPolicy,
    total: u64,
    cancel: &impl Fn() -> bool,
) -> Result<crate::geometry::JointProjectivePolicy, AnchorRankFileError> {
    let phase = policy.search.search.search.search;
    let mut target = phase.search.gradient.search.local.geometry;
    target.tolerance = policy.search.search.anchors.local.target_tolerance;
    let policy = crate::geometry::JointProjectivePolicy {
        target,
        source_tolerance: policy.search.search.anchors.local.source_tolerance,
        maximum_projection_checks: total / 6,
    };
    // Validate all geometry and cumulative work before source IO.
    crate::geometry::verify_projective_sampled_joint_for_domains(
        &[],
        policy,
        phase.search.gradient.search.sampling,
        (1, 1),
        (1, 1),
        cancel,
    )
    .map_err(crate::local_scan::LocalFileError::from)?;
    Ok(policy)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn search_anchor_rank_opposite_files_selected(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: OppositeAnchorSearchPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    prepaid: Option<rrrah_core::Reservation>,
    joint_projection_checks: Option<u64>,
    cancel: impl Fn() -> bool,
) -> Result<OppositeAnchorSearchEvidence, AnchorRankFileError> {
    let latch = std::cell::Cell::new(false);
    let cancelled = || {
        let stopped = latch.get()
            || cancel()
            || [left, right].iter().any(|r| {
                r.cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(stopped);
        stopped
    };
    let result = (|| {
        let bytes = validate_opposite_policy(policy, factors, &cancelled)?;
        let base = policy.search.search;
        let phase = base.search.search;
        let joint = joint_projection_checks
            .map(|total| validate_joint_opposite_projection_policy(policy, total, &cancelled))
            .transpose()?;
        let credit = match prepaid {
            Some(credit) if credit.bytes() >= bytes => credit,
            Some(_) => return Err(LocalRankError::Budget.into()),
            None => budget.try_reserve(bytes).map_err(|_| LocalRankError::Budget)?,
        };
        let mut initial_credit = Some(credit);
        let decode = phase.search.gradient.search.local.decode;
        let first = DecodeSourceSnapshot::read(&left.path, decode.max_file_bytes, &cancelled)?;
        let second = DecodeSourceSnapshot::read(&right.path, decode.max_file_bytes, &cancelled)?;
        let radii = [
            [phase.smoothing_radius; 2],
            [base.search.asymmetric_radius, 0],
            [0, base.search.asymmetric_radius],
        ];
        let mut inconclusive = false;
        for index in 0..6 {
            if cancelled() {
                return Err(AnchorRankFileError::Cancelled);
            }
            let credit = match initial_credit.take() {
                Some(credit) => credit,
                None => budget.try_reserve(bytes).map_err(|_| LocalRankError::Budget)?,
            };
            first.verify(&cancelled)?;
            second.verify(&cancelled)?;
            let (proposal_points, legacy_transform, dimensions) = if joint.is_some() {
                let (points, source, target) =
                    crate::local_scan::extract_candidate_union_points_files_managed(
                        right,
                        left,
                        phase,
                        factors,
                        radii[index % 3],
                        (index >= 3).then_some(policy.search.maximum_reflection_bins / 3),
                        budget,
                        &cancelled,
                    )?;
                (points, None, Some((target, source)))
            } else {
                let proposal = if index < 3 {
                    crate::local_scan::find_projective_candidate_union_geometry_files_managed(
                        right,
                        left,
                        phase,
                        factors,
                        radii[index % 3],
                        budget,
                        &cancelled,
                    )?
                } else {
                    crate::local_scan::find_projective_candidate_union_reflected_geometry_files_managed(
                        right,
                        left,
                        phase,
                        factors,
                        radii[index % 3],
                        policy.search.maximum_reflection_bins / 3,
                        budget,
                        &cancelled,
                    )?
                };
                let transform = proposal
                    .geometry
                    .as_ref()
                    .map(|g| g.transform.inverse())
                    .transpose()
                    .map_err(|_| LocalRankError::Invalid)?;
                (proposal.correspondences, transform, None)
            };
            if proposal_points.len() > policy.maximum_swapped_points {
                return Err(LocalRankError::Budget.into());
            }
            let mut swapped = Vec::new();
            swapped
                .try_reserve_exact(proposal_points.len())
                .map_err(|_| LocalRankError::Budget)?;
            for point in proposal_points.iter() {
                if cancelled() {
                    return Err(AnchorRankFileError::Cancelled);
                }
                swapped.push(Correspondence {
                    source: point.target,
                    target: point.source,
                });
            }
            if swapped.capacity() > policy.maximum_swapped_points {
                return Err(LocalRankError::Budget.into());
            }
            let points = credit.try_adopt(swapped).map_err(|_| LocalRankError::Budget)?;
            drop(proposal_points);
            let transform = if let Some(joint) = joint {
                let (source, target) = dimensions.expect("joint extraction dimensions");
                crate::geometry::verify_projective_sampled_joint_for_domains(
                    &points,
                    joint,
                    phase.search.gradient.search.sampling,
                    source,
                    target,
                    &cancelled,
                )
                .map_err(crate::local_scan::LocalFileError::from)?
                .map(|g| g.transform)
            } else {
                legacy_transform
            };
            if joint.is_some() && transform.is_none() {
                inconclusive = true;
            }
            let anchors = if let Some(model) = transform {
                match compare_anchor_rank_files(
                    left,
                    right,
                    model,
                    &points,
                    decode,
                    base.anchors,
                    budget,
                    &cancelled,
                ) {
                    Ok(evidence) => Some(evidence),
                    Err(AnchorRankFileError::Rank(LocalRankError::InsufficientGeometry)) => {
                        inconclusive = true;
                        None
                    }
                    Err(error) => return Err(error),
                }
            } else {
                None
            };
            first.verify(&cancelled)?;
            second.verify(&cancelled)?;
            let supported = anchors.is_some_and(|e| e.supported);
            if supported || index == 5 {
                let outcome = if !supported && inconclusive {
                    Err(AnchorRankFileError::Rank(LocalRankError::InsufficientGeometry))
                } else {
                    Ok(OppositeAnchorSearchEvidence {
                        correspondences: points,
                        transform,
                        anchors,
                        attempted_recipes: (index + 1) as u32,
                        reflected_proposal_source: index >= 3,
                    })
                };
                return Ok((outcome, first, second));
            }
        }
        unreachable!("six opposite recipes always produce a terminal outcome")
    })();
    finish_source_guard(result, left, right, &latch, cancelled())?
}
