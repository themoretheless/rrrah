//! Exact normalized-pixel subimage evidence, without scale or geometric warps.
//! A region match never proves that whole images/files are identical.

use crate::linear::LinearRgbaView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CropEvidence {
    /// All matching placements, in row-major order.
    pub placements: Vec<Region>,
    pub compared_pixels: u64,
    /// The needle has at least two distinct canonical pixels. This information
    /// test does not establish uniqueness or semantic correspondence.
    pub informative: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CropBudget {
    pub max_pixel_comparisons: u64,
    pub max_matches: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CropError {
    #[error("crop search exceeded supplied work or result budget; result is inconclusive")]
    Budget,
    #[error("crop search cancelled; result is inconclusive")]
    Cancelled,
}

/// Find every placement of `needle` in `image`, retaining HDR and alpha semantics.
/// Empty result proves absence only for exact, unscaled, unrotated pixels.
/// Work includes the needle information check and every compared pixel. Budget
/// exhaustion never returns a partial list or a false negative.
///
/// # Errors
/// Returns cancellation or budget exhaustion as inconclusive evidence.
pub fn find_exact_crops(
    image: &LinearRgbaView<'_>,
    needle: &LinearRgbaView<'_>,
    budget: CropBudget,
    cancel: impl Fn() -> bool,
) -> Result<CropEvidence, CropError> {
    let (width, height) = image.dimensions();
    let (nw, nh) = needle.dimensions();
    let mut work = 0_u64;
    let mut tick = || {
        if cancel() {
            return Err(CropError::Cancelled);
        }
        if work >= budget.max_pixel_comparisons {
            return Err(CropError::Budget);
        }
        work += 1;
        Ok(())
    };
    if cancel() {
        return Err(CropError::Cancelled);
    }
    if nw > width || nh > height {
        return Ok(CropEvidence {
            placements: Vec::new(),
            compared_pixels: 0,
            informative: false,
        });
    }
    let anchor = needle.pixel(0, 0);
    let mut informative = false;
    'information: for y in 0..nh {
        for x in 0..nw {
            tick()?;
            if needle.pixel(x, y) != anchor {
                informative = true;
                break 'information;
            }
        }
    }
    let mut placements = Vec::new();
    for y in 0..=height - nh {
        'placement: for x in 0..=width - nw {
            for ny in 0..nh {
                for nx in 0..nw {
                    tick()?;
                    if image.pixel(x + nx, y + ny) != needle.pixel(nx, ny) {
                        continue 'placement;
                    }
                }
            }
            if placements.len() >= budget.max_matches {
                return Err(CropError::Budget);
            }
            crate::local::reserve_slot(&mut placements, budget.max_matches).map_err(|_| CropError::Budget)?;
            placements.push(Region {
                x,
                y,
                width: nw,
                height: nh,
            });
        }
    }
    if cancel() {
        return Err(CropError::Cancelled);
    }
    Ok(CropEvidence {
        placements,
        compared_pixels: work,
        informative,
    })
}
