//! Guarded selected-frame bidirectional color evidence under supplied geometry.
use crate::{
    affine_region::{AffineRegionError, AffineRegionEvidence, AffineRegionPolicy, verify_affine_region},
    animated::AnimationBudget,
    decode::{CachedError, DecodeSourceSnapshot, decode_selected_frame_bounded},
    exact::SnapshotError,
    geometry::ProjectiveTransform,
};
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
#[derive(Debug, thiserror::Error)]
pub enum AffineRegionFileError {
    #[error("invalid file-region policy")]
    Invalid,
    #[error("file-region work budget exceeded")]
    Budget,
    #[error("file-region comparison cancelled")]
    Cancelled,
    #[error(transparent)]
    Source(#[from] SnapshotError),
    #[error(transparent)]
    Decode(#[from] CachedError),
    #[error(transparent)]
    Region(#[from] AffineRegionError),
    #[error(transparent)]
    Grid(#[from] crate::affine_region_grid::AffineGridError),
    #[error(transparent)]
    Search(#[from] crate::local_scan::LocalFileError),
}
#[derive(Debug, Clone, Copy)]
pub struct AffineRegionFilePolicy {
    pub decode: AnimationBudget,
    pub forward: AffineRegionPolicy,
    pub reverse: AffineRegionPolicy,
    pub maximum_total_pixel_reads: u64,
}
/// Source and external-dependency snapshots enclose both decodes and both
/// directional validations. Sticky cancellation observes callback and request
/// generation tokens. Returned evidence describes supplied regions only, and
/// is never exact-file/pixel identity or automatic geometric discovery.
pub fn verify_affine_region_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    model: ProjectiveTransform,
    left_region: [u32; 4],
    right_region: [u32; 4],
    policy: AffineRegionFilePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<[AffineRegionEvidence; 2], AffineRegionFileError> {
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
            return Err(AffineRegionFileError::Cancelled);
        }
        let work = |r: [u32; 4], p: AffineRegionPolicy| -> Option<u64> {
            if !(1..=16).contains(&p.radius) || r[2] == 0 || r[3] == 0 {
                return None;
            }
            u64::from(r[2])
                .checked_mul(u64::from(r[3]))?
                .checked_mul(u64::from(2 * p.radius + 1).pow(2))?
                .checked_mul(5)
        };
        let forward = work(right_region, policy.forward).ok_or(AffineRegionFileError::Invalid)?;
        let reverse = work(left_region, policy.reverse).ok_or(AffineRegionFileError::Invalid)?;
        if forward > policy.forward.maximum_pixel_reads
            || reverse > policy.reverse.maximum_pixel_reads
            || forward
                .checked_add(reverse)
                .ok_or(AffineRegionFileError::Budget)?
                > policy.maximum_total_pixel_reads
        {
            return Err(AffineRegionFileError::Budget);
        }
        let inverse = model.inverse().map_err(|_| AffineRegionFileError::Invalid)?;
        let first = DecodeSourceSnapshot::read(&left.path, policy.decode.max_file_bytes, &cancelled)?;
        let second = DecodeSourceSnapshot::read(&right.path, policy.decode.max_file_bytes, &cancelled)?;
        let a = decode_selected_frame_bounded(left, policy.decode, budget, &cancelled)?;
        let b = decode_selected_frame_bounded(right, policy.decode, budget, &cancelled)?;
        let av = a
            .view(&cancelled)
            .map_err(crate::decode::FileError::from)
            .map_err(CachedError::from)?;
        let bv = b
            .view(&cancelled)
            .map_err(crate::decode::FileError::from)
            .map_err(CachedError::from)?;
        let f = verify_affine_region(
            &av,
            &bv,
            model,
            left_region,
            right_region,
            policy.forward,
            budget,
            &cancelled,
        )?;
        let r = verify_affine_region(
            &bv,
            &av,
            inverse,
            right_region,
            left_region,
            policy.reverse,
            budget,
            &cancelled,
        )?;
        first.verify(&cancelled)?;
        second.verify(&cancelled)?;
        Ok([f, r])
    })();
    if cancelled() {
        Err(AffineRegionFileError::Cancelled)
    } else {
        result
    }
}

/// Automatic target-grid color evidence under supplied geometry, with one
/// guarded selected-frame lifecycle. The full declared per-region work cap is
/// admitted for both directions before IO; the view API also checks actual work.
/// Returned regions are local evidence and may overlap; no identity decision.
pub fn verify_affine_grid_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    model: ProjectiveTransform,
    decode: AnimationBudget,
    policy: crate::affine_region_grid::AffineGridPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<crate::affine_region_grid::AffineGridEvidence>, AffineRegionFileError> {
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
            return Err(AffineRegionFileError::Cancelled);
        }
        if policy.grid.0 == 0
            || policy.grid.1 == 0
            || !(1..=16).contains(&policy.region.radius)
            || !(1..=16).contains(&policy.maximum_source_radius)
        {
            return Err(AffineRegionFileError::Invalid);
        }
        let cells = u64::from(policy.grid.0)
            .checked_mul(u64::from(policy.grid.1))
            .ok_or(AffineRegionFileError::Budget)?;
        if cells > policy.maximum_regions as u64
            || cells
                .checked_mul(2)
                .and_then(|v| v.checked_mul(policy.region.maximum_pixel_reads))
                .ok_or(AffineRegionFileError::Budget)?
                > policy.maximum_total_pixel_reads
        {
            return Err(AffineRegionFileError::Budget);
        }
        model.inverse().map_err(|_| AffineRegionFileError::Invalid)?;
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
        let evidence = crate::affine_region_grid::verify_affine_region_grid(
            &av, &bv, model, policy, budget, &cancelled,
        )?;
        first.verify(&cancelled)?;
        second.verify(&cancelled)?;
        Ok(evidence)
    })();
    if cancelled() {
        Err(AffineRegionFileError::Cancelled)
    } else {
        result
    }
}

#[derive(Debug)]
pub struct AffineCandidateSearchEvidence {
    pub search: crate::local_scan::ManagedProjectiveCandidateUnionRegionsFileEvidence,
    pub color_regions: Option<rrrah_core::SharedBuffer<crate::affine_region_grid::AffineGridEvidence>>,
}
/// Fresh native candidate union, geometric verification, then automatic affine
/// color regions. Outer source/dependency snapshots cover both phases, so a new
/// content revision cannot be confirmed using geometry from an older revision.
/// Each phase retains its explicit work cap and shared memory budget. No default
/// copy decision is made; callers must inspect both directions and coverage.
pub fn search_affine_color_regions(
    left: &DecodeRequest,
    right: &DecodeRequest,
    search: crate::local_scan::ProjectiveCandidateUnionRegionsFilePolicy,
    factors: &[f64],
    grid: crate::affine_region_grid::AffineGridPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AffineCandidateSearchEvidence, AffineRegionFileError> {
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
            return Err(AffineRegionFileError::Cancelled);
        }
        let cells = u64::from(grid.grid.0)
            .checked_mul(u64::from(grid.grid.1))
            .ok_or(AffineRegionFileError::Budget)?;
        if cells == 0 {
            return Err(AffineRegionFileError::Invalid);
        }
        if cells > grid.maximum_regions as u64
            || cells
                .checked_mul(2)
                .and_then(|v| v.checked_mul(grid.region.maximum_pixel_reads))
                .ok_or(AffineRegionFileError::Budget)?
                > grid.maximum_total_pixel_reads
        {
            return Err(AffineRegionFileError::Budget);
        }
        let decode = search.search.gradient.search.local.decode;
        let first = DecodeSourceSnapshot::read(&left.path, decode.max_file_bytes, &cancelled)?;
        let second = DecodeSourceSnapshot::read(&right.path, decode.max_file_bytes, &cancelled)?;
        let evidence = crate::local_scan::compare_local_files_projective_candidate_union_regions_managed(
            left, right, search, factors, budget, &cancelled,
        )?;
        let color_regions = if let Some(geometry) = &evidence.geometry {
            Some(verify_affine_grid_files(
                left,
                right,
                geometry.transform,
                decode,
                grid,
                budget,
                &cancelled,
            )?)
        } else {
            None
        };
        first.verify(&cancelled)?;
        second.verify(&cancelled)?;
        Ok(AffineCandidateSearchEvidence {
            search: evidence,
            color_regions,
        })
    })();
    if cancelled() {
        Err(AffineRegionFileError::Cancelled)
    } else {
        result
    }
}
