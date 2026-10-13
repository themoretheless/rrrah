//! Explicit intermediate-scale gradient sampling. This is a candidate recipe;
//! geometry and pixel confirmation remain independent downstream requirements.
use crate::{
    gradient::{GradientCellRecipe, GradientFeature, extract_gradients_with_recipe},
    linear::LinearRgbaView,
    local::LocalError,
    pyramid::PyramidPolicy,
};

/// Sample caller-declared reduction factors, each relative to the original.
/// Bilinear linear-RGBA resampling uses pixel-center coordinates. Factors must
/// be finite, at least one, and strictly increasing. The scale list belongs in
/// cache/descriptor recipe identity. This primitive uses fallible allocations;
/// it does not promise a managed-memory or process-memory bound.
///
/// # Errors
/// Invalid factors, checked pixel/feature/sample admission, allocation refusal,
/// opacity refusal from extraction, or cancellation without partial results.
pub fn extract_gradient_scales(
    image: &LinearRgbaView<'_>,
    policy: PyramidPolicy,
    factors: &[f64],
    max_total_samples: u64,
    recipe: GradientCellRecipe,
    cancel: impl Fn() -> bool,
) -> Result<Vec<GradientFeature>, LocalError> {
    extract_scales_selected(
        image,
        policy,
        factors,
        max_total_samples,
        recipe,
        None,
        None,
        cancel,
    )
}

/// Intermediate scales with independent spatial detector quotas at each level.
/// Histogram recipes and matching thresholds are unchanged.
/// # Errors
/// Invalid grid/factors, work limits, alpha refusal or cancellation.
pub fn extract_spatial_gradient_scales(
    image: &LinearRgbaView<'_>,
    policy: PyramidPolicy,
    factors: &[f64],
    max_total_samples: u64,
    recipe: GradientCellRecipe,
    grid: (usize, usize, usize),
    cancel: impl Fn() -> bool,
) -> Result<Vec<GradientFeature>, LocalError> {
    extract_scales_selected(
        image,
        policy,
        factors,
        max_total_samples,
        recipe,
        Some(grid),
        None,
        cancel,
    )
}

fn grid_cells(policy: PyramidPolicy, grid: Option<(usize, usize, usize)>) -> Result<usize, LocalError> {
    let Some((columns, rows, quota)) = grid else {
        return Ok(0);
    };
    if columns == 0 || rows == 0 || quota == 0 {
        return Err(LocalError::Invalid);
    }
    let cells = columns.checked_mul(rows).ok_or(LocalError::Budget)?;
    if cells > policy.local.max_features {
        return Err(LocalError::Budget);
    }
    Ok(cells)
}

fn extract_scales_selected(
    image: &LinearRgbaView<'_>,
    policy: PyramidPolicy,
    factors: &[f64],
    max_total_samples: u64,
    recipe: GradientCellRecipe,
    grid: Option<(usize, usize, usize)>,
    area_taps: Option<u64>,
    cancel: impl Fn() -> bool,
) -> Result<Vec<GradientFeature>, LocalError> {
    grid_cells(policy, grid)?;
    let features = validate_scales(image, policy, factors, max_total_samples, &cancel)?;
    if let Some(limit) = area_taps {
        validate_area_work(image, factors, limit, &cancel)?;
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let (width, height) = image.dimensions();
    let extract = |view: &LinearRgbaView<'_>| {
        if let Some(grid) = grid {
            crate::gradient::extract_spatial_gradients_with_recipe(
                view,
                policy.local,
                policy.local.max_features as u64 * 512,
                recipe,
                grid,
                &cancel,
            )
        } else {
            extract_gradients_with_recipe(
                view,
                policy.local,
                policy.local.max_features as u64 * 512,
                recipe,
                &cancel,
            )
        }
    };
    let mut output = Vec::new();
    output
        .try_reserve_exact(features)
        .map_err(|_| LocalError::Budget)?;
    for &factor in factors {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        let w = (f64::from(width) / factor).floor() as u32;
        let h = (f64::from(height) / factor).floor() as u32;
        let mut level = if factor == 1. {
            extract(image)?
        } else {
            let count = usize::try_from(u64::from(w) * u64::from(h))
                .map_err(|_| LocalError::Budget)?
                .checked_mul(4)
                .ok_or(LocalError::Budget)?;
            let mut pixels = Vec::new();
            pixels.try_reserve_exact(count).map_err(|_| LocalError::Budget)?;
            for y in 0..h {
                for x in 0..w {
                    if cancel() {
                        return Err(LocalError::Cancelled);
                    }
                    if area_taps.is_some() {
                        pixels.extend(sample_area(image, x, y, factor, &cancel)?);
                        continue;
                    }
                    let sx = ((f64::from(x) + 0.5) * factor - 0.5).clamp(0., f64::from(width - 1));
                    let sy = ((f64::from(y) + 0.5) * factor - 0.5).clamp(0., f64::from(height - 1));
                    let ix = sx.floor() as u32;
                    let iy = sy.floor() as u32;
                    let dx = sx - f64::from(ix);
                    let dy = sy - f64::from(iy);
                    let taps = [
                        (ix, iy, (1. - dx) * (1. - dy)),
                        ((ix + 1).min(width - 1), iy, dx * (1. - dy)),
                        (ix, (iy + 1).min(height - 1), (1. - dx) * dy),
                        ((ix + 1).min(width - 1), (iy + 1).min(height - 1), dx * dy),
                    ];
                    let mut p = [0.; 4];
                    for (tx, ty, weight) in taps {
                        let source = image.pixel(tx, ty).ok_or(LocalError::Invalid)?;
                        for c in 0..4 {
                            p[c] += f64::from(f32::from_bits(source[c])) * weight;
                        }
                    }
                    pixels.extend(p.map(|v| v as f32));
                }
            }
            let view =
                LinearRgbaView::new(w, h, &pixels, u64::from(w) * u64::from(h), &cancel).map_err(|e| {
                    if e == crate::pixels::PixelError::Cancelled {
                        LocalError::Cancelled
                    } else {
                        LocalError::Invalid
                    }
                })?;
            extract(&view)?
        };
        for feature in &mut level {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            feature.position = feature.position.map(|v| (v + 0.5) * factor - 0.5);
        }
        output.extend(level);
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(output)
}

fn validate_scales(
    image: &LinearRgbaView<'_>,
    policy: PyramidPolicy,
    factors: &[f64],
    max_total_samples: u64,
    cancel: &impl Fn() -> bool,
) -> Result<usize, LocalError> {
    if !policy.local.minimum_corner_score.is_finite() || policy.local.minimum_corner_score < 0. {
        return Err(LocalError::Invalid);
    }
    if factors.is_empty() || factors.len() > policy.max_levels {
        return Err(LocalError::Budget);
    }
    let (width, height) = image.dimensions();
    let mut total_pixels = 0_u64;
    let mut previous = 0.;
    for &factor in factors {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if !factor.is_finite() || factor < 1. || factor <= previous {
            return Err(LocalError::Invalid);
        }
        previous = factor;
        let w = (f64::from(width) / factor).floor() as u32;
        let h = (f64::from(height) / factor).floor() as u32;
        if u64::from(w) * u64::from(h) > policy.local.max_pixels {
            return Err(LocalError::Budget);
        }
        if w == 0 || h == 0 {
            return Err(LocalError::Invalid);
        }
        total_pixels = total_pixels
            .checked_add(u64::from(w) * u64::from(h))
            .ok_or(LocalError::Budget)?;
    }
    let features = policy
        .local
        .max_features
        .checked_mul(factors.len())
        .ok_or(LocalError::Budget)?;
    let samples = u64::try_from(features)
        .ok()
        .and_then(|n| n.checked_mul(512))
        .ok_or(LocalError::Budget)?;
    if total_pixels > policy.max_total_pixels
        || features > policy.max_total_features
        || samples > max_total_samples
    {
        return Err(LocalError::Budget);
    }
    Ok(features)
}

/// Admit conservative resampling/detector scratch and retained feature capacity
/// before extraction. The returned shared buffer retains its credit until its
/// last owner drops. Input pixels are caller-owned and not charged here.
///
/// # Errors
/// Same work/policy/cancellation errors as the unowned primitive, plus managed
/// memory admission or actual retained-capacity refusal.
#[cfg(feature = "raster")]
pub fn extract_gradient_scales_managed(
    image: &LinearRgbaView<'_>,
    policy: PyramidPolicy,
    factors: &[f64],
    max_total_samples: u64,
    recipe: GradientCellRecipe,
    budget: &rrrah_core::MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<GradientFeature>, LocalError> {
    extract_scales_managed_selected(
        image,
        policy,
        factors,
        max_total_samples,
        recipe,
        None,
        None,
        budget,
        cancel,
    )
}

/// Managed intermediate scales with per-level spatial quotas.
/// # Errors
/// Invalid grid/recipe/work limits, memory refusal or cancellation.
#[cfg(feature = "raster")]
pub fn extract_spatial_gradient_scales_managed(
    image: &LinearRgbaView<'_>,
    policy: PyramidPolicy,
    factors: &[f64],
    max_total_samples: u64,
    recipe: GradientCellRecipe,
    grid: (usize, usize, usize),
    budget: &rrrah_core::MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<GradientFeature>, LocalError> {
    extract_scales_managed_selected(
        image,
        policy,
        factors,
        max_total_samples,
        recipe,
        Some(grid),
        None,
        budget,
        cancel,
    )
}

#[cfg(feature = "raster")]
fn extract_scales_managed_selected(
    image: &LinearRgbaView<'_>,
    policy: PyramidPolicy,
    factors: &[f64],
    max_total_samples: u64,
    recipe: GradientCellRecipe,
    grid: Option<(usize, usize, usize)>,
    area_taps: Option<u64>,
    budget: &rrrah_core::MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<GradientFeature>, LocalError> {
    let cells = grid_cells(policy, grid)?;
    let features = validate_scales(image, policy, factors, max_total_samples, &cancel)?;
    if let Some(limit) = area_taps {
        validate_area_work(image, factors, limit, &cancel)?;
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let bytes = |count: usize, size: usize| {
        count
            .checked_mul(size)
            .and_then(|v| u64::try_from(v).ok())
            .ok_or(LocalError::Budget)
    };
    let retained_bytes = bytes(features, std::mem::size_of::<GradientFeature>())?;
    let (width, height) = image.dimensions();
    // Factor one borrows the caller image: only detector grayscale is temporary.
    // Reduced levels overlap their RGBA32 resample with detector grayscale.
    let maximum_pixel_bytes = factors
        .iter()
        .map(|&f| {
            let pixels = u64::from((f64::from(width) / f).floor() as u32)
                * u64::from((f64::from(height) / f).floor() as u32);
            pixels
                .checked_mul(if f == 1. { 8 } else { 24 })
                .ok_or(LocalError::Budget)
        })
        .try_fold(0u64, |maximum, bytes| Ok::<_, LocalError>(maximum.max(bytes?)))?;
    let corners = bytes(
        policy.local.max_candidates,
        std::mem::size_of::<(f64, usize, usize)>(),
    )?;
    let detector = bytes(
        policy.local.max_features,
        std::mem::size_of::<crate::local::Feature>(),
    )?;
    let descriptors = bytes(policy.local.max_features, std::mem::size_of::<GradientFeature>())?;
    let scratch_bytes = maximum_pixel_bytes
        .checked_add(corners)
        .and_then(|v| v.checked_add(detector.checked_mul(2)?))
        .and_then(|v| v.checked_add(descriptors))
        .and_then(|v| v.checked_add(bytes(cells, std::mem::size_of::<usize>()).ok()?))
        .ok_or(LocalError::Budget)?;
    let _scratch = budget
        .try_reserve(scratch_bytes)
        .map_err(|_| LocalError::Budget)?;
    let retained = budget
        .try_reserve(retained_bytes)
        .map_err(|_| LocalError::Budget)?;
    let output = extract_scales_selected(
        image,
        policy,
        factors,
        max_total_samples,
        recipe,
        grid,
        area_taps,
        &cancel,
    )?;
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    retained.try_adopt(output).map_err(|_| LocalError::Budget)
}

/// Area-average each declared reduction footprint in linear RGBA before local
/// gradient extraction. This explicit candidate recipe suppresses point-sampling
/// aliasing; original pixel confirmation remains a separate requirement.
/// Scale factors and sampling mode belong in persistent descriptor identity.
/// Input pixels are caller owned; no complete decoder/RSS bound is promised.
/// # Errors
/// Invalid factors/grid, checked pixel/feature/area-tap limits, opacity refusal,
/// memory admission or cancellation without retained partial output.
#[cfg(feature = "raster")]
pub fn extract_spatial_gradient_scales_area_managed(
    image: &LinearRgbaView<'_>,
    policy: PyramidPolicy,
    factors: &[f64],
    max_total_samples: u64,
    recipe: GradientCellRecipe,
    grid: (usize, usize, usize),
    max_resample_taps: u64,
    budget: &rrrah_core::MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<GradientFeature>, LocalError> {
    extract_scales_managed_selected(
        image,
        policy,
        factors,
        max_total_samples,
        recipe,
        Some(grid),
        Some(max_resample_taps),
        budget,
        cancel,
    )
}

fn validate_area_work(
    image: &LinearRgbaView<'_>,
    factors: &[f64],
    limit: u64,
    cancel: &impl Fn() -> bool,
) -> Result<(), LocalError> {
    let (width, height) = image.dimensions();
    let mut total = 0_u64;
    for &factor in factors {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if !factor.is_finite() || factor < 1. {
            return Err(LocalError::Invalid);
        }
        if factor == 1. {
            continue;
        }
        let axis = |length: u32| -> Result<u64, LocalError> {
            let count = (f64::from(length) / factor).floor() as u32;
            let mut taps = 0_u64;
            for n in 0..count {
                if cancel() {
                    return Err(LocalError::Cancelled);
                }
                let start = (f64::from(n) * factor).floor() as u32;
                let end = ((f64::from(n) + 1.) * factor).ceil().min(f64::from(length)) as u32;
                taps = taps
                    .checked_add(u64::from(end - start))
                    .ok_or(LocalError::Budget)?;
            }
            Ok(taps)
        };
        total = total
            .checked_add(
                axis(width)?
                    .checked_mul(axis(height)?)
                    .ok_or(LocalError::Budget)?,
            )
            .ok_or(LocalError::Budget)?;
        if total > limit {
            return Err(LocalError::Budget);
        }
    }
    Ok(())
}

fn sample_area(
    image: &LinearRgbaView<'_>,
    x: u32,
    y: u32,
    factor: f64,
    cancel: &impl Fn() -> bool,
) -> Result<[f32; 4], LocalError> {
    let (width, height) = image.dimensions();
    let left = f64::from(x) * factor;
    let top = f64::from(y) * factor;
    let right = ((f64::from(x) + 1.) * factor).min(f64::from(width));
    let bottom = ((f64::from(y) + 1.) * factor).min(f64::from(height));
    let area = (right - left) * (bottom - top);
    if !area.is_finite() || area <= 0. {
        return Err(LocalError::Invalid);
    }
    let mut value = [0.; 4];
    for sy in top.floor() as u32..bottom.ceil() as u32 {
        for sx in left.floor() as u32..right.ceil() as u32 {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let weight = (right.min(f64::from(sx) + 1.) - left.max(f64::from(sx)))
                * (bottom.min(f64::from(sy) + 1.) - top.max(f64::from(sy)))
                / area;
            let source = image.pixel(sx, sy).ok_or(LocalError::Invalid)?;
            for c in 0..4 {
                value[c] += f64::from(f32::from_bits(source[c])) * weight;
            }
        }
    }
    Ok(value.map(|v| v as f32))
}

#[cfg(test)]
mod area_tests {
    use super::*;
    #[test]
    fn footprint_average_removes_stripe_alias_and_matches_fractional_oracle() {
        let pixels: Vec<_> = (0..6)
            .flat_map(|_| {
                (0..6).flat_map(|x| {
                    let v = if x % 3 == 0 { 1. } else { 0. };
                    [v, v, v, 1.]
                })
            })
            .collect();
        let view = LinearRgbaView::new(6, 6, &pixels, 36, || false).unwrap();
        // Point samples at x=1 and x=4 see zero; a 3x3 footprint retains mean1/3.
        for x in 0..2 {
            assert_eq!(
                sample_area(&view, x, 0, 3., &|| false).unwrap(),
                [1. / 3., 1. / 3., 1. / 3., 1.]
            );
        }
        // [2.5,5) intersects white column3 with width1, so its mean is1/2.5.
        let p = sample_area(&view, 1, 0, 2.5, &|| false).unwrap();
        assert_eq!(p, [0.4, 0.4, 0.4, 1.]);
        assert!(matches!(
            sample_area(&view, 0, 0, 3., &|| true),
            Err(LocalError::Cancelled)
        ));
        assert!(validate_area_work(&view, &[1., 3.], 36, &|| false).is_ok());
        assert!(matches!(
            validate_area_work(&view, &[1., 3.], 35, &|| false),
            Err(LocalError::Budget)
        ));
        assert!(validate_area_work(&view, &[2.5], 36, &|| false).is_ok());
        assert!(matches!(
            validate_area_work(&view, &[2.5], 35, &|| false),
            Err(LocalError::Budget)
        ));
    }
}

// Validate the identity smoothing recipe without allocating a duplicate frame.
// Its samples are read through LinearRgbaView, which already canonicalizes zero.
#[cfg(feature = "raster")]
pub(crate) fn validate_unsmoothed_gradient_candidates(
    image: &LinearRgbaView<'_>,
    max_sample_taps: u64,
    cancel: impl Fn() -> bool,
) -> Result<(), LocalError> {
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let (width, height) = image.dimensions();
    let work = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(2))
        .ok_or(LocalError::Budget)?;
    if work > max_sample_taps {
        return Err(LocalError::Budget);
    }
    for y in 0..height {
        for x in 0..width {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            if image.rgba(x, y).ok_or(LocalError::Invalid)?[3] != 1. {
                return Err(LocalError::UnsupportedPixels);
            }
        }
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(())
}

/// Explicit candidate-only separable box smoothing of opaque linear RGBA.
/// Clamped borders, f64 accumulation and f32 intermediate samples define this
/// recipe. Radius and sampling mode belong in persistent descriptor identity.
/// Final duplicate confirmation must continue to read the original pixels.
/// Input samples are caller-owned. The full output and at most `2 * radius + 1`
/// horizontally smoothed rows are managed; completed rows are reused as a ring.
/// `max_sample_taps` counts RGBA sites across both passes, including border repeats.
///
/// # Errors
/// Checked work/memory admission, nonopaque source or cancellation without a
/// retained partial result. HDR and negative RGB values are preserved.
#[cfg(feature = "raster")]
pub fn smooth_gradient_candidates_managed(
    image: &LinearRgbaView<'_>,
    radius: u32,
    max_sample_taps: u64,
    budget: &rrrah_core::MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<f32>, LocalError> {
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let (width, height) = image.dimensions();
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(LocalError::Budget)?;
    let side = u64::from(radius)
        .checked_mul(2)
        .and_then(|v| v.checked_add(1))
        .ok_or(LocalError::Budget)?;
    let work = pixels
        .checked_mul(side)
        .and_then(|v| v.checked_mul(2))
        .ok_or(LocalError::Budget)?;
    if work > max_sample_taps {
        return Err(LocalError::Budget);
    }
    let count = usize::try_from(pixels)
        .ok()
        .and_then(|v| v.checked_mul(4))
        .ok_or(LocalError::Budget)?;
    let bytes = count
        .checked_mul(std::mem::size_of::<f32>())
        .and_then(|v| u64::try_from(v).ok())
        .ok_or(LocalError::Budget)?;
    // Radius zero has no neighboring rows: retain only its output buffer.
    // Keep opacity validation and the old +0 arithmetic (canonical signed zero).
    if radius == 0 {
        let credit = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;
        let mut output = Vec::new();
        output.try_reserve_exact(count).map_err(|_| LocalError::Budget)?;
        for y in 0..height {
            for x in 0..width {
                if cancel() {
                    return Err(LocalError::Cancelled);
                }
                let sample = image.rgba(x, y).ok_or(LocalError::Invalid)?;
                if sample[3] != 1. {
                    return Err(LocalError::UnsupportedPixels);
                }
                output.extend(sample.map(|v| (0.0_f64 + f64::from(v)) as f32));
            }
        }
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        return credit.try_adopt(output).map_err(|_| LocalError::Budget);
    }
    // Each horizontal row is independent. Retain only the vertical window,
    // preserving the original f64 summation and f32 intermediate rounding.
    let ring_rows = side.min(u64::from(height));
    let horizontal_count = u64::from(width)
        .checked_mul(ring_rows)
        .and_then(|v| v.checked_mul(4))
        .and_then(|v| usize::try_from(v).ok())
        .ok_or(LocalError::Budget)?;
    let horizontal_bytes = horizontal_count
        .checked_mul(std::mem::size_of::<f32>())
        .and_then(|v| u64::try_from(v).ok())
        .ok_or(LocalError::Budget)?;
    // Admit scratch and retained output before allocating either one.
    let _horizontal_credit = budget
        .try_reserve(horizontal_bytes)
        .map_err(|_| LocalError::Budget)?;
    let output_credit = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;
    let mut horizontal = Vec::new();
    horizontal
        .try_reserve_exact(horizontal_count)
        .map_err(|_| LocalError::Budget)?;
    if horizontal.capacity() > horizontal_count {
        return Err(LocalError::Budget);
    }
    let mut output = Vec::new();
    output.try_reserve_exact(count).map_err(|_| LocalError::Budget)?;
    let mut next_row = 0;
    for y in 0..height {
        let required_row = y.saturating_add(radius).min(height - 1);
        while next_row <= required_row {
            let row_slot = u64::from(next_row) % ring_rows;
            for x in 0..width {
                let mut sum = [0.; 4];
                for offset in -i64::from(radius)..=i64::from(radius) {
                    if cancel() {
                        return Err(LocalError::Cancelled);
                    }
                    let sx = (i64::from(x) + offset).clamp(0, i64::from(width) - 1) as u32;
                    let sample = image.rgba(sx, next_row).ok_or(LocalError::Invalid)?;
                    if sample[3] != 1. {
                        return Err(LocalError::UnsupportedPixels);
                    }
                    for channel in 0..4 {
                        sum[channel] += f64::from(sample[channel]) / side as f64;
                    }
                }
                let index = ((row_slot * u64::from(width) + u64::from(x)) * 4) as usize;
                let pixel = sum.map(|v| v as f32);
                if index == horizontal.len() {
                    horizontal.extend(pixel);
                } else {
                    horizontal[index..index + 4].copy_from_slice(&pixel);
                }
            }
            next_row += 1;
        }
        for x in 0..width {
            let mut sum = [0.; 4];
            for offset in -i64::from(radius)..=i64::from(radius) {
                if cancel() {
                    return Err(LocalError::Cancelled);
                }
                let sy = (i64::from(y) + offset).clamp(0, i64::from(height) - 1) as u64;
                let index = (((sy % ring_rows) * u64::from(width) + u64::from(x)) * 4) as usize;
                for channel in 0..4 {
                    sum[channel] += f64::from(horizontal[index + channel]) / side as f64;
                }
            }
            output.extend(sum.map(|v| v as f32));
        }
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    output_credit.try_adopt(output).map_err(|_| LocalError::Budget)
}

#[cfg(all(test, feature = "raster"))]
mod zero_candidate_tests {
    use super::*;
    use std::cell::Cell;
    #[test]
    fn identity_validation_is_bitwise_owned_zero_radius_for_hdr_and_shapes() {
        let values = [
            -0.0,
            0.0,
            f32::from_bits(1),
            -f32::from_bits(1),
            -100.0,
            100.0,
            f32::MAX / 2.0,
        ];
        for (w, h) in [(1, 1), (2, 7), (7, 2), (1, 19), (19, 1)] {
            let mut pixels = Vec::new();
            for n in 0..w * h {
                pixels.extend([
                    values[n as usize % values.len()],
                    values[(n as usize + 1) % values.len()],
                    values[(n as usize + 3) % values.len()],
                    1.0,
                ]);
            }
            let view = LinearRgbaView::new(w, h, &pixels, u64::MAX, || false).unwrap();
            validate_unsmoothed_gradient_candidates(&view, u64::from(w * h) * 2, || false).unwrap();
            let budget = rrrah_core::MemoryBudget::new(u64::from(w * h) * 16);
            let owned = smooth_gradient_candidates_managed(&view, 0, u64::from(w * h) * 2, &budget, || false)
                .unwrap();
            let converted = LinearRgbaView::new(w, h, &owned, u64::MAX, || false).unwrap();
            for y in 0..h {
                for x in 0..w {
                    assert_eq!(
                        view.rgba(x, y).unwrap().map(f32::to_bits),
                        converted.rgba(x, y).unwrap().map(f32::to_bits)
                    );
                }
            }
            drop(owned);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn identity_validation_work_alpha_and_every_cancel_checkpoint_are_explicit() {
        let mut pixels = vec![0.25; 3 * 9 * 4];
        for p in pixels.chunks_exact_mut(4) {
            p[3] = 1.0;
        }
        let view = LinearRgbaView::new(3, 9, &pixels, 27, || false).unwrap();
        assert!(matches!(
            validate_unsmoothed_gradient_candidates(&view, 53, || false),
            Err(LocalError::Budget)
        ));
        assert!(matches!(
            validate_unsmoothed_gradient_candidates(&view, 0, || true),
            Err(LocalError::Cancelled)
        ));
        let total = Cell::new(0);
        validate_unsmoothed_gradient_candidates(&view, 54, || {
            total.set(total.get() + 1);
            false
        })
        .unwrap();
        assert_eq!(total.get(), 29);
        for stop in 1..=total.get() {
            let calls = Cell::new(0);
            let result = validate_unsmoothed_gradient_candidates(&view, 54, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            });
            assert!(matches!(result, Err(LocalError::Cancelled)), "stop={stop}");
        }
        pixels[3 * 9 * 4 - 1] = 0.5;
        let view = LinearRgbaView::new(3, 9, &pixels, 27, || false).unwrap();
        assert!(matches!(
            validate_unsmoothed_gradient_candidates(&view, 54, || false),
            Err(LocalError::UnsupportedPixels)
        ));
    }
}
