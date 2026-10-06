//! Bounded dyadic local-feature pyramid. Positions map back to original pixel
//! centers. Exact scale matching is qualified first; continuous scales remain
//! approximate and require independent corpus calibration.

use crate::{
    linear::LinearRgbaView,
    local::{Feature, LocalError, LocalPolicy, extract},
};
#[derive(Debug, Clone, Copy)]
pub struct PyramidPolicy {
    pub local: LocalPolicy,
    pub max_levels: usize,
    pub max_total_pixels: u64,
    pub max_total_features: usize,
}

/// Area-downsample premultiplied linear RGBA by two at each level and extract
/// quarter-turn local descriptors. Odd final rows/columns are excluded from that
/// reduced level, while full-resolution extraction still covers their interior.
/// Each pixel of scale s maps to original center `(coordinate + .5) * s - .5`.
///
/// # Errors
/// Rejects zero levels, allocation/resource-budget excess or cancellation;
/// no partial multiscale feature list is returned.
pub fn extract_pyramid(
    image: &LinearRgbaView<'_>,
    policy: PyramidPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Vec<Feature>, LocalError> {
    extract_levels(image, policy, &cancel, false)
}

/// Area-downsampled oriented BRIEF pyramid, with positions in source pixels.
/// Shares the same bounded levels and premultiplied-alpha reduction as the
/// quarter-turn pyramid. Matching and geometry remain separate evidence.
///
/// # Errors
/// Invalid level policy, exceeded budgets or cancellation without partial output.
pub fn extract_oriented_pyramid(
    image: &LinearRgbaView<'_>,
    policy: PyramidPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Vec<Feature>, LocalError> {
    extract_levels(image, policy, &cancel, true)
}

/// Reserve conservative pyramid scratch and retained feature capacity before
/// extraction. The returned features retain their reservation until the last
/// owner drops them. This admission bounds managed work, not whole-process RSS.
///
/// # Errors
/// Invalid policy, resource/allocation excess, or cancellation returns no list.
#[cfg(feature = "raster")]
pub fn extract_oriented_pyramid_managed(
    image: &LinearRgbaView<'_>,
    policy: PyramidPolicy,
    budget: &rrrah_core::MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<Feature>, LocalError> {
    if policy.max_levels == 0
        || !policy.local.minimum_corner_score.is_finite()
        || policy.local.minimum_corner_score < 0.0
    {
        return Err(LocalError::Invalid);
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let (width, height) = image.dimensions();
    let pixels = u64::from(width) * u64::from(height);
    if pixels > policy.local.max_pixels || pixels > policy.max_total_pixels {
        return Err(LocalError::Budget);
    }
    // Grayscale detector/descriptor planes plus two overlapping RGBA pyramid
    // levels fit within 24 bytes per full-resolution pixel. Feature construction
    // may coexist with accumulated output and candidate corner storage.
    let scratch = pixels.checked_mul(24).ok_or(LocalError::Budget)?;
    let feature_bytes = policy
        .max_total_features
        .checked_mul(std::mem::size_of::<Feature>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    let local_features = policy
        .local
        .max_features
        .checked_mul(std::mem::size_of::<Feature>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    let corners = policy
        .local
        .max_candidates
        .checked_mul(std::mem::size_of::<(f64, usize, usize)>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    let work = scratch
        .checked_add(corners)
        .and_then(|n| n.checked_add(local_features.checked_mul(2)?))
        .and_then(|n| n.checked_add(feature_bytes))
        .ok_or(LocalError::Budget)?;
    let _work = budget.try_reserve(work).map_err(|_| LocalError::Budget)?;
    let retained = budget
        .try_reserve(feature_bytes)
        .map_err(|_| LocalError::Budget)?;
    let features = extract_oriented_pyramid(image, policy, &cancel)?;
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    retained.try_adopt(features).map_err(|_| LocalError::Budget)
}

fn extract_levels(
    image: &LinearRgbaView<'_>,
    policy: PyramidPolicy,
    cancel: &impl Fn() -> bool,
    oriented: bool,
) -> Result<Vec<Feature>, LocalError> {
    if policy.max_levels == 0 {
        return Err(LocalError::Invalid);
    }
    let (mut width, mut height) = image.dimensions();
    let mut storage: Option<Vec<f32>> = None;
    let mut scale = 1_u32;
    let mut visited = 0_u64;
    let mut result = Vec::new();
    for level in 0..policy.max_levels {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        visited = visited
            .checked_add(u64::from(width) * u64::from(height))
            .ok_or(LocalError::Budget)?;
        if visited > policy.max_total_pixels {
            return Err(LocalError::Budget);
        }
        let view = if let Some(samples) = &storage {
            LinearRgbaView::new(width, height, samples, policy.local.max_pixels, cancel)
                .map_err(|_| LocalError::Invalid)?
        } else {
            *image
        };
        let mut features = if oriented {
            crate::local::extract_multiscale_oriented(&view, policy.local, cancel)?
        } else {
            extract(&view, policy.local, cancel)?
        };
        if features.len() > policy.max_total_features.saturating_sub(result.len()) {
            return Err(LocalError::Budget);
        }
        // Admit storage before publishing this level; allocation failure must
        // remain an explicit resource error rather than aborting extraction.
        result
            .try_reserve_exact(features.len())
            .map_err(|_| LocalError::Budget)?;
        for feature in &mut features {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            feature.position = feature.position.map(|v| (v + 0.5) * f64::from(scale) - 0.5);
        }
        result.extend(features);
        if level + 1 == policy.max_levels || width < 2 || height < 2 {
            break;
        }
        let (next_width, next_height) = (width / 2, height / 2);
        let next_pixels = u64::from(next_width) * u64::from(next_height);
        if next_pixels > policy.local.max_pixels || next_pixels > policy.max_total_pixels - visited {
            return Err(LocalError::Budget);
        }
        let next = reduce(&view, next_width, next_height, &cancel)?;
        storage = Some(next);
        width = next_width;
        height = next_height;
        scale = scale.checked_mul(2).ok_or(LocalError::Budget)?;
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(result)
}

#[allow(clippy::cast_possible_truncation)] // Convex averages of finite f32 samples remain within f32 range.
fn reduce(
    image: &LinearRgbaView<'_>,
    width: u32,
    height: u32,
    cancel: &impl Fn() -> bool,
) -> Result<Vec<f32>, LocalError> {
    let count = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    let mut output = Vec::new();
    output.try_reserve_exact(count).map_err(|_| LocalError::Budget)?;
    for y in 0..height {
        for x in 0..width {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let mut premultiplied = [0.0; 4];
            for dy in 0..2 {
                for dx in 0..2 {
                    let p = image
                        .pixel(x * 2 + dx, y * 2 + dy)
                        .ok_or(LocalError::Invalid)?
                        .map(|v| f64::from(f32::from_bits(v)));
                    for c in 0..3 {
                        premultiplied[c] += p[c] * p[3] / 4.0;
                    }
                    premultiplied[3] += p[3] / 4.0;
                }
            }
            let alpha = premultiplied[3];
            for &channel in &premultiplied[..3] {
                output.push(if alpha == 0.0 {
                    0.0
                } else {
                    (channel / alpha) as f32
                });
            }
            output.push(alpha as f32);
        }
    }
    Ok(output)
}
