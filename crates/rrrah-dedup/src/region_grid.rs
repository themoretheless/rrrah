//! Bounded candidate domains. These rectangles require separate pixel verification.
use crate::{geometry::ProjectiveTransform, local::LocalError, warp::PixelRectangle};
use rrrah_core::{MemoryBudget, SharedBuffer};

/// Project a uniform source grid into clipped target bounding rectangles.
/// No region acceptance is inferred. Singular transforms or a horizon anywhere
/// in either whole image refuse; cancellation never returns partial domains.
///
/// # Errors
/// Invalid dimensions/grid/geometry, work or allocation budget, cancellation.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn projective_grid_domains(
    source: (u32, u32),
    target: (u32, u32),
    transform: ProjectiveTransform,
    grid: (u32, u32),
    max_cells: usize,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<SharedBuffer<[PixelRectangle; 2]>, LocalError> {
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    if source.0 == 0 || source.1 == 0 || target.0 == 0 || target.1 == 0 || grid.0 == 0 || grid.1 == 0 {
        return Err(LocalError::Invalid);
    }
    let cells = u64::from(grid.0)
        .checked_mul(u64::from(grid.1))
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    if cells > max_cells {
        return Err(LocalError::Budget);
    }
    let inverse = transform.inverse().map_err(|_| LocalError::Invalid)?;
    for (h, (w, ht)) in [(transform, source), (inverse, target)] {
        let mut sign = None;
        for [x, y] in [
            [0., 0.],
            [f64::from(w - 1), 0.],
            [0., f64::from(ht - 1)],
            [f64::from(w - 1), f64::from(ht - 1)],
        ] {
            let denominator = h.matrix[2][0] * x + h.matrix[2][1] * y + h.matrix[2][2];
            if !denominator.is_finite()
                || denominator == 0.
                || sign.is_some_and(|s| s != denominator.is_sign_positive())
            {
                return Err(LocalError::Invalid);
            }
            sign = Some(denominator.is_sign_positive());
        }
    }
    let bytes = cells
        .checked_mul(std::mem::size_of::<[PixelRectangle; 2]>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    let credit = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;
    let mut result = Vec::new();
    result.try_reserve_exact(cells).map_err(|_| LocalError::Budget)?;
    for row in 0..grid.1 {
        for col in 0..grid.0 {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let split = |i: u32, size: u32, parts: u32| {
                u32::try_from(u64::from(i) * u64::from(size) / u64::from(parts)).unwrap()
            };
            let x = split(col, source.0, grid.0);
            let y = split(row, source.1, grid.1);
            let width = split(col + 1, source.0, grid.0) - x;
            let height = split(row + 1, source.1, grid.1) - y;
            if width == 0 || height == 0 {
                continue;
            }
            let mut low = [f64::INFINITY; 2];
            let mut high = [f64::NEG_INFINITY; 2];
            for corner in [
                [x, y],
                [x + width - 1, y],
                [x, y + height - 1],
                [x + width - 1, y + height - 1],
            ] {
                let mapped = transform
                    .apply(corner.map(f64::from))
                    .ok_or(LocalError::Invalid)?;
                for axis in 0..2 {
                    low[axis] = low[axis].min(mapped[axis]);
                    high[axis] = high[axis].max(mapped[axis]);
                }
            }
            let bounds = [f64::from(target.0), f64::from(target.1)];
            let starts =
                std::array::from_fn::<_, 2, _>(|axis| low[axis].floor().clamp(0., bounds[axis]) as u32);
            let ends = std::array::from_fn::<_, 2, _>(|axis| {
                (high[axis].ceil() + 1.).clamp(0., bounds[axis]) as u32
            });
            if ends[0] <= starts[0] || ends[1] <= starts[1] {
                continue;
            }
            result.push([
                PixelRectangle { x, y, width, height },
                PixelRectangle {
                    x: starts[0],
                    y: starts[1],
                    width: ends[0] - starts[0],
                    height: ends[1] - starts[1],
                },
            ]);
        }
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    credit.try_adopt(result).map_err(|_| LocalError::Budget)
}

/// Candidate rectangles from grids on both images, returned in left/right order.
/// Both directions retain their independent grid partitions. Overlapping or
/// identical rectangles are candidates, not independent support or area unions.
/// No pixel acceptance is inferred. The declared cap admits both full grids
/// before allocating, including cells later clipped outside the other image.
///
/// # Errors
/// Invalid dimensions/geometry, insufficient cell/memory admission or cancellation.
pub fn projective_bidirectional_grid_domains(
    source: (u32, u32),
    target: (u32, u32),
    transform: ProjectiveTransform,
    grid: (u32, u32),
    max_cells: usize,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<SharedBuffer<[PixelRectangle; 2]>, LocalError> {
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    if source.0 == 0 || source.1 == 0 || target.0 == 0 || target.1 == 0 || grid.0 == 0 || grid.1 == 0 {
        return Err(LocalError::Invalid);
    }
    let cells = u64::from(grid.0)
        .checked_mul(u64::from(grid.1))
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    let total = cells.checked_mul(2).ok_or(LocalError::Budget)?;
    if total > max_cells {
        return Err(LocalError::Budget);
    }
    let inverse = transform.inverse().map_err(|_| LocalError::Invalid)?;
    let bytes = total
        .checked_mul(std::mem::size_of::<[PixelRectangle; 2]>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    let credit = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;
    let mut result = Vec::new();
    result.try_reserve_exact(total).map_err(|_| LocalError::Budget)?;
    for (dimensions, other, model, reverse) in [
        (source, target, transform, false),
        (target, source, inverse, true),
    ] {
        let domains = projective_grid_domains(dimensions, other, model, grid, cells, budget, &cancel)?;
        for pair in domains.iter() {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            result.push(if reverse { [pair[1], pair[0]] } else { *pair });
        }
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    credit.try_adopt(result).map_err(|_| LocalError::Budget)
}
