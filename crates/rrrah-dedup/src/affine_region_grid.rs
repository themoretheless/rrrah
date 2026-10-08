//! Automatic target-grid regions with independent bidirectional color evidence.
use crate::{
    affine_region::{AffineRegionError, AffineRegionEvidence, AffineRegionPolicy, verify_affine_region},
    geometry::ProjectiveTransform,
    linear::LinearRgbaView,
    local::LocalError,
    region_footprint::{FootprintError, select_filter_footprint},
    warp::PixelRectangle,
};
use rrrah_core::{MemoryBudget, SharedBuffer};
#[derive(Debug, Clone, Copy)]
pub struct AffineGridPolicy {
    pub region: AffineRegionPolicy,
    pub grid: (u32, u32),
    pub maximum_regions: usize,
    pub maximum_source_radius: u32,
    pub maximum_total_pixel_reads: u64,
}
#[derive(Debug, Clone, Copy)]
pub struct AffineGridEvidence {
    pub domains: [PixelRectangle; 2],
    pub source_radius: u32,
    pub directions: [Result<AffineRegionEvidence, AffineRegionError>; 2],
}
#[derive(Debug, thiserror::Error)]
pub enum AffineGridError {
    #[error(transparent)]
    Geometry(#[from] LocalError),
    #[error(transparent)]
    Footprint(#[from] FootprintError),
    #[error(transparent)]
    Region(#[from] AffineRegionError),
}
fn rect(r: PixelRectangle) -> [u32; 4] {
    [r.x, r.y, r.width, r.height]
}
/// Enumerate a uniform target grid, map its rectangles to the source, and
/// validate both directions. Color-fit refusals remain in the result; work,
/// memory, unsupported input and cancellation abort without partial evidence.
/// No threshold decision or union coverage is inferred from these local results.
pub fn verify_affine_region_grid(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    model: ProjectiveTransform,
    policy: AffineGridPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<SharedBuffer<AffineGridEvidence>, AffineGridError> {
    let latch = std::cell::Cell::new(false);
    let cancelled = || {
        let value = latch.get() || cancel();
        latch.set(value);
        value
    };
    let result = (|| {
        if cancelled() {
            return Err(AffineRegionError::Cancelled.into());
        }
        if policy.region.radius == 0 || policy.maximum_source_radius == 0 {
            return Err(AffineRegionError::Invalid.into());
        }
        let inverse = model.inverse().map_err(|_| LocalError::Invalid)?;
        let domains = crate::region_grid::projective_grid_domains(
            target.dimensions(),
            source.dimensions(),
            inverse,
            policy.grid,
            policy.maximum_regions,
            budget,
            &cancelled,
        )?;
        let radius = |r: PixelRectangle| {
            select_filter_footprint(
                model,
                [
                    f64::from(r.x) + f64::from(r.width) / 2.,
                    f64::from(r.y) + f64::from(r.height) / 2.,
                ],
                policy.region.radius,
                policy.maximum_source_radius,
                &cancelled,
            )
            .map(|v| v.source_radius)
        };
        let mut total = 0u64;
        for pair in domains.iter() {
            let reverse_radius = radius(pair[1])?;
            for (r, rad) in [(pair[0], policy.region.radius), (pair[1], reverse_radius)] {
                let sites = u64::from(r.width) * u64::from(r.height);
                let reads = sites
                    .checked_mul(crate::affine_region::filter_taps(rad).ok_or(AffineRegionError::Budget)?)
                    .and_then(|v| v.checked_mul(5))
                    .ok_or(AffineRegionError::Budget)?;
                if sites > policy.region.maximum_sites || reads > policy.region.maximum_pixel_reads {
                    return Err(AffineRegionError::Budget.into());
                }
                total = total.checked_add(reads).ok_or(AffineRegionError::Budget)?;
            }
        }
        if total > policy.maximum_total_pixel_reads {
            return Err(AffineRegionError::Budget.into());
        }
        let bytes = domains
            .len()
            .checked_mul(std::mem::size_of::<AffineGridEvidence>())
            .and_then(|v| u64::try_from(v).ok())
            .ok_or(AffineRegionError::Budget)?;
        let credit = budget.try_reserve(bytes).map_err(|_| AffineRegionError::Budget)?;
        let mut results = Vec::new();
        results
            .try_reserve_exact(domains.len())
            .map_err(|_| AffineRegionError::Budget)?;
        for pair in domains.iter() {
            if cancelled() {
                return Err(AffineRegionError::Cancelled.into());
            }
            let source_radius = radius(pair[1])?;
            let mut reverse = policy.region;
            reverse.radius = source_radius;
            let forward = verify_affine_region(
                source,
                target,
                model,
                rect(pair[1]),
                rect(pair[0]),
                policy.region,
                budget,
                &cancelled,
            );
            let backward = verify_affine_region(
                target,
                source,
                inverse,
                rect(pair[0]),
                rect(pair[1]),
                reverse,
                budget,
                &cancelled,
            );
            for value in [&forward, &backward] {
                if let Err(error) = value {
                    if !matches!(
                        error,
                        AffineRegionError::Color(
                            crate::affine_color::AffineColorError::Uninformative
                                | crate::affine_color::AffineColorError::IllConditioned
                                | crate::affine_color::AffineColorError::Bounds
                        )
                    ) {
                        return Err((*error).into());
                    }
                }
            }
            results.push(AffineGridEvidence {
                domains: [pair[1], pair[0]],
                source_radius,
                directions: [forward, backward],
            });
        }
        if cancelled() {
            return Err(AffineRegionError::Cancelled.into());
        }
        credit
            .try_adopt(results)
            .map_err(|_| AffineRegionError::Budget.into())
    })();
    if cancelled() {
        Err(AffineRegionError::Cancelled.into())
    } else {
        result
    }
}
