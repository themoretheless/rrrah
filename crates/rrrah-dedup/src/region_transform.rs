//! Pixel confirmation of candidate regional domains under supplied geometry.
//! Immutable views carry no file identity; file callers must retain source guards.
use crate::{
    geometry::ProjectiveTransform,
    linear::LinearRgbaView,
    local::LocalError,
    local_scan::{LocalFileError, ProjectivePyramidRegionsFilePolicy, ProjectiveRegionFileEvidence},
    warp::WarpError,
};
use rrrah_core::{MemoryBudget, SharedBuffer};

/// Verify automatic regions under a supplied candidate transform. The transform
/// is not trusted as copy evidence: every accepted region needs fitted pixels in
/// both directions. No whole-image candidate is returned. Singular/horizon,
/// work/memory and cancellation failures discard all partial regions.
///
/// # Errors
/// Invalid policy/geometry, cumulative phase limits or cancellation.
pub fn verify_projective_region_grid_views(
    left: &LinearRgbaView<'_>,
    right: &LinearRgbaView<'_>,
    transform: ProjectiveTransform,
    policy: ProjectivePyramidRegionsFilePolicy,
    grid: (u32, u32),
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<SharedBuffer<ProjectiveRegionFileEvidence>, LocalFileError> {
    verify_region_grid_views(left, right, transform, policy, grid, false, budget, cancel)
}

/// Verify candidate grids from both images under the same supplied transform.
/// Every region requires the unchanged pixel admission in both directions.
/// Supports can overlap and must not be summed as a coverage fraction.
/// Both full grids and their cumulative verification work are admitted first.
///
/// # Errors
/// Invalid policy/geometry, cumulative phase limits or cancellation.
pub fn verify_projective_bidirectional_region_grid_views(
    left: &LinearRgbaView<'_>,
    right: &LinearRgbaView<'_>,
    transform: ProjectiveTransform,
    policy: ProjectivePyramidRegionsFilePolicy,
    grid: (u32, u32),
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<SharedBuffer<ProjectiveRegionFileEvidence>, LocalFileError> {
    verify_region_grid_views(left, right, transform, policy, grid, true, budget, cancel)
}

#[allow(clippy::too_many_arguments)]
fn verify_region_grid_views(
    left: &LinearRgbaView<'_>,
    right: &LinearRgbaView<'_>,
    transform: ProjectiveTransform,
    policy: ProjectivePyramidRegionsFilePolicy,
    grid: (u32, u32),
    bidirectional: bool,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<SharedBuffer<ProjectiveRegionFileEvidence>, LocalFileError> {
    let latch = std::cell::Cell::new(false);
    let cancelled = || {
        let value = latch.get() || cancel();
        latch.set(value);
        value
    };
    let result = (|| {
        crate::local_scan::validate_projective_pyramid_file_policy(&policy.search)?;
        if grid.0 == 0 || grid.1 == 0 || policy.max_regions == 0 {
            return Err(LocalFileError::InvalidPolicy);
        }
        let cells = u64::from(grid.0)
            .checked_mul(u64::from(grid.1))
            .and_then(|n| n.checked_mul(if bidirectional { 2 } else { 1 }))
            .ok_or(WarpError::Budget)?;
        if cells > u64::try_from(policy.max_regions).map_err(|_| WarpError::Budget)? {
            return Err(WarpError::Budget.into());
        }
        let p = policy.search;
        let work = cells
            .checked_add(1)
            .and_then(|n| n.checked_mul(p.filter.filter.max_sample_pairs))
            .ok_or(WarpError::Budget)?;
        if work > policy.max_total_sample_pairs {
            return Err(WarpError::Budget.into());
        }
        let domains = if bidirectional {
            crate::region_grid::projective_bidirectional_grid_domains(
                left.dimensions(),
                right.dimensions(),
                transform,
                grid,
                policy.max_regions,
                budget,
                &cancelled,
            )?
        } else {
            crate::region_grid::projective_grid_domains(
                left.dimensions(),
                right.dimensions(),
                transform,
                grid,
                policy.max_regions,
                budget,
                &cancelled,
            )?
        };
        let bytes = domains
            .len()
            .checked_mul(std::mem::size_of::<ProjectiveRegionFileEvidence>())
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(LocalError::Budget)?;
        let credit = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;
        let shared = crate::warp::verify_projective_filtered(
            left,
            right,
            transform,
            p.photometric.residual,
            p.filter,
            &cancelled,
        )?;
        let mut regions = Vec::new();
        regions
            .try_reserve_exact(domains.len())
            .map_err(|_| LocalError::Budget)?;
        #[allow(clippy::cast_precision_loss)]
        let passes = |v: &crate::warp::WarpEvidence| {
            v.compared_pixels >= p.local.minimum_compared_pixels
                && v.compared_pixels as f64 >= v.source_pixels as f64 * p.local.minimum_coverage_fraction
                && v.matched_pixels as f64 >= v.compared_pixels as f64 * p.local.minimum_matched_fraction
        };
        for pair in domains.iter() {
            if cancelled() {
                return Err(LocalFileError::Cancelled);
            }
            let mut pixels = None;
            let mut fit_failure = None;
            match crate::warp::verify_projective_regions_with_unfitted(
                left,
                right,
                transform,
                *pair,
                p.photometric,
                p.filter,
                p.fit_mode,
                shared,
                &cancelled,
            ) {
                Ok(e) => pixels = Some(e),
                Err(WarpError::Fit(reason)) => fit_failure = Some(reason),
                Err(error) => return Err(error.into()),
            }
            let accepted_region = pixels
                .as_ref()
                .is_some_and(|e| passes(&e.fitted.forward.pixels) && passes(&e.fitted.reverse.pixels));
            regions.push(ProjectiveRegionFileEvidence {
                accepted_region,
                domains: *pair,
                pixels,
                fit_failure,
            });
        }
        if cancelled() {
            return Err(LocalFileError::Cancelled);
        }
        credit.try_adopt(regions).map_err(|_| LocalError::Budget.into())
    })();
    if cancelled() {
        Err(LocalFileError::Cancelled)
    } else {
        result
    }
}
