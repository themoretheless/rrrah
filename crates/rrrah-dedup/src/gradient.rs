//! Explicit opaque linear-light gradient histogram recipe. This is a descriptor
//! primitive, not a SIFT detector or independently qualified copy decision.
use crate::{linear::LinearRgbaView, local::LocalError};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientDescriptor(pub [f64; 128]);

/// Dominant orientation of an opaque integer-centered 16x16 gradient patch.
/// A weighted 36-bin circular histogram is smoothed six times and its strongest
/// peak is interpolated. Tied peaks within floating precision refuse; uniform
/// patches return None. This recipe assigns one orientation, not multiple peaks.
/// `max_samples` admits256 grid sites, each reading four neighboring pixels.
///
/// # Errors
/// Boundary/alpha refusal, insufficient work admission or cancellation.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn gradient_orientation(
    image: &LinearRgbaView<'_>,
    position: [u32; 2],
    max_samples: u64,
    cancel: impl Fn() -> bool,
) -> Result<Option<f64>, LocalError> {
    if max_samples < 256 {
        return Err(LocalError::Budget);
    }
    let (width, height) = image.dimensions();
    let [x, y] = position;
    if x < 9
        || y < 9
        || x.checked_add(9).is_none_or(|v| v >= width)
        || y.checked_add(9).is_none_or(|v| v >= height)
    {
        return Err(LocalError::Invalid);
    }
    let sample = |px: u32, py: u32| {
        let p = image
            .pixel(px, py)
            .ok_or(LocalError::Invalid)?
            .map(|v| f64::from(f32::from_bits(v)));
        if p[3] != 1. {
            return Err(LocalError::Invalid);
        }
        Ok(p[0] * 0.2126 + p[1] * 0.7152 + p[2] * 0.0722)
    };
    let mut histogram = [0.; 36];
    for row in 0..16_u32 {
        for col in 0..16_u32 {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let (px, py) = (x - 8 + col, y - 8 + row);
            let gx = sample(px + 1, py)? - sample(px - 1, py)?;
            let gy = sample(px, py + 1)? - sample(px, py - 1)?;
            let (u, v) = (f64::from(col) - 8., f64::from(row) - 8.);
            let magnitude = gx.hypot(gy) * (-(u * u + v * v) / 32.).exp();
            let direction = gy.atan2(gx).rem_euclid(std::f64::consts::TAU) * 36. / std::f64::consts::TAU;
            let bin = direction.floor() as usize;
            let fraction = direction - bin as f64;
            histogram[bin % 36] += magnitude * (1. - fraction);
            histogram[(bin + 1) % 36] += magnitude * fraction;
        }
    }
    for _ in 0..6 {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        histogram =
            std::array::from_fn(|i| (histogram[(i + 35) % 36] + histogram[i] + histogram[(i + 1) % 36]) / 3.);
    }
    let best = (0..36)
        .max_by(|&a, &b| histogram[a].total_cmp(&histogram[b]))
        .unwrap();
    let peak = histogram[best];
    if !peak.is_finite() {
        return Err(LocalError::Invalid);
    }
    let tied = histogram.iter().enumerate().any(|(i, &v)| {
        i != best
            && i != (best + 1) % 36
            && i != (best + 35) % 36
            && (peak - v).abs() <= peak * f64::EPSILON * 8.
    });
    let result = if peak == 0. || tied {
        None
    } else {
        let (previous, next) = (histogram[(best + 35) % 36], histogram[(best + 1) % 36]);
        let denominator = previous - 2. * peak + next;
        let offset = if denominator == 0. {
            0.
        } else {
            0.5 * (previous - next) / denominator
        };
        Some(
            ((best as f64 + offset.clamp(-0.5, 0.5)) * std::f64::consts::TAU / 36.)
                .rem_euclid(std::f64::consts::TAU),
        )
    };
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(result)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientFeature {
    pub position: [f64; 2],
    pub descriptor: GradientDescriptor,
}

/// Reflect a canonical descriptor across its dominant-orientation axis.
/// Original landmark coordinates are unchanged: this is only a proposal recipe.
/// Both fixed and interpolated cells reverse rows and negate angular bins.
/// Fixed-size stack storage; no heap allocation or image resampling is needed.
/// Refuses nonfinite, negative or nonunit inputs, with cancellation priority.
pub fn reflect_gradient_descriptor(
    descriptor: &GradientDescriptor,
    cancel: impl Fn() -> bool,
) -> Result<GradientDescriptor, LocalError> {
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let mut norm = 0.;
    for &value in &descriptor.0 {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if !value.is_finite() || value < 0. {
            return Err(LocalError::Invalid);
        }
        norm += value * value;
    }
    if !norm.is_finite() || (norm - 1.).abs() > 1e-6 {
        return Err(LocalError::Invalid);
    }
    let mut reflected = [0.; 128];
    for row in 0..4 {
        for column in 0..4 {
            for bin in 0..8 {
                if cancel() {
                    return Err(LocalError::Cancelled);
                }
                reflected[((3 - row) * 4 + column) * 8 + (8 - bin) % 8] =
                    descriptor.0[(row * 4 + column) * 8 + bin];
            }
        }
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(GradientDescriptor(reflected))
}

/// Managed reflected proposal features with unchanged original coordinates.
/// The explicit work cap counts transformed descriptor bins; input validation
/// and cancellation use fixed-size loops. Ownership retains allocation credit.
#[cfg(feature = "raster")]
pub fn reflect_gradient_features_managed(
    features: &[GradientFeature],
    maximum_bins: u64,
    budget: &rrrah_core::MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<GradientFeature>, LocalError> {
    if cancel() { return Err(LocalError::Cancelled); }
    let n = u64::try_from(features.len()).map_err(|_| LocalError::Budget)?;
    if n.checked_mul(128).ok_or(LocalError::Budget)? > maximum_bins {
        return Err(LocalError::Budget);
    }
    let bytes = n.checked_mul(std::mem::size_of::<GradientFeature>() as u64)
        .ok_or(LocalError::Budget)?;
    let credit = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;
    let mut output = Vec::new();
    output.try_reserve_exact(features.len()).map_err(|_| LocalError::Budget)?;
    for feature in features {
        if cancel() { return Err(LocalError::Cancelled); }
        if feature.position.iter().any(|v| !v.is_finite()) { return Err(LocalError::Invalid); }
        output.push(GradientFeature {
            position: feature.position,
            descriptor: reflect_gradient_descriptor(&feature.descriptor, &cancel)?,
        });
    }
    if cancel() { return Err(LocalError::Cancelled); }
    credit.try_adopt(output).map_err(|_| LocalError::Budget)
}

/// Extract strongest native corners, one dominant orientation and a unit-scale
/// gradient descriptor per interior point. A13-pixel margin admits every rotated
/// descriptor window. Uniform/ambiguous orientations produce no feature;
/// boundary points are excluded by the declared detector recipe. Alpha refusals
/// and resource errors remain errors, never silently discarded.
/// `max_total_samples` admits512 grid sites per maximum detector feature before
/// extraction, including orientations that later prove uninformative. Native
/// corner work remains bounded separately by `local` pixel/candidate limits.
/// This fallible-allocation primitive has no managed memory reservation.
///
/// # Errors
/// Detector policy/work/allocation excess, alpha refusal or cancellation.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn extract_gradients(
    image: &LinearRgbaView<'_>,
    local: crate::local::LocalPolicy,
    max_total_samples: u64,
    cancel: impl Fn() -> bool,
) -> Result<Vec<GradientFeature>, LocalError> {
    extract_gradients_with_recipe(image, local, max_total_samples, GradientCellRecipe::Fixed, cancel)
}

/// Spatial histogram recipe. Both sides of a match must use the same recipe;
/// cached descriptors must include this selection in their algorithm identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientCellRecipe {
    Fixed,
    Interpolated,
}

/// Explicit recipe selection with unchanged corner/orientation/sample admission.
/// Interpolation adds histogram operations, not sampled grid sites or heap arrays.
///
/// # Errors
/// Same detector, opacity, work, allocation and cancellation errors as `extract_gradients`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn extract_gradients_with_recipe(
    image: &LinearRgbaView<'_>,
    local: crate::local::LocalPolicy,
    max_total_samples: u64,
    recipe: GradientCellRecipe,
    cancel: impl Fn() -> bool,
) -> Result<Vec<GradientFeature>, LocalError> {
    extract_gradients_selected(image, local, max_total_samples, recipe, None, cancel)
}

/// Gradient extraction with spatial quotas on the native detector corners.
/// Descriptor/orientation recipes and downstream matching thresholds retain
/// their definitions. Quotas alter feature selection and require calibration.
/// This primitive uses fallible allocation, without managed reservations.
///
/// # Errors
/// Invalid grid/policy, detector/descriptor work or allocation limits, cancellation.
pub fn extract_spatial_gradients_with_recipe(
    image: &LinearRgbaView<'_>,
    local: crate::local::LocalPolicy,
    max_total_samples: u64,
    recipe: GradientCellRecipe,
    grid: (usize, usize, usize),
    cancel: impl Fn() -> bool,
) -> Result<Vec<GradientFeature>, LocalError> {
    extract_gradients_selected(image, local, max_total_samples, recipe, Some(grid), cancel)
}

fn extract_gradients_selected(
    image: &LinearRgbaView<'_>,
    local: crate::local::LocalPolicy,
    max_total_samples: u64,
    recipe: GradientCellRecipe,
    grid: Option<(usize, usize, usize)>,
    cancel: impl Fn() -> bool,
) -> Result<Vec<GradientFeature>, LocalError> {
    let required = u64::try_from(local.max_features)
        .ok()
        .and_then(|n| n.checked_mul(512))
        .ok_or(LocalError::Budget)?;
    if required > max_total_samples {
        return Err(LocalError::Budget);
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let corners = if let Some((columns, rows, max_per_cell)) = grid {
        crate::local::extract_spatial(image, local, columns, rows, max_per_cell, &cancel)?
    } else {
        crate::local::extract(image, local, &cancel)?
    };
    let (width, height) = image.dimensions();
    let mut output = Vec::new();
    output
        .try_reserve_exact(corners.len())
        .map_err(|_| LocalError::Budget)?;
    for corner in corners {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        let [x, y] = corner.position.map(|v| v as u32);
        if x < 13
            || y < 13
            || x.checked_add(13).is_none_or(|v| v >= width)
            || y.checked_add(13).is_none_or(|v| v >= height)
        {
            continue;
        }
        let Some(angle) = gradient_orientation(image, [x, y], 256, &cancel)? else {
            continue;
        };
        if let Some(descriptor) = describe_gradient_cells(
            image,
            corner.position,
            1.,
            angle,
            256,
            recipe == GradientCellRecipe::Interpolated,
            &cancel,
        )? {
            output.push(GradientFeature {
                position: corner.position,
                descriptor,
            });
        }
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(output)
}

#[derive(Debug, Clone, Copy)]
pub struct GradientMatchPolicy {
    /// Full descriptor pairs; each evaluates 128 squared differences.
    pub max_comparisons: u64,
    pub max_squared_distance: f64,
    /// Squared-distance ratio, applied in both directions. Strict inequality.
    pub squared_ratio: f64,
}

/// Native area-reduced gradient pyramid with original pixel-center coordinates.
/// Work admits512 grid sites per maximum local feature per declared level;
/// detector/total-pixel/output-feature limits retain their independent meaning.
///
/// # Errors
/// Invalid levels, checked work/pixel/feature/allocation excess or cancellation.
pub fn extract_gradient_pyramid(
    image: &LinearRgbaView<'_>,
    policy: crate::pyramid::PyramidPolicy,
    max_total_samples: u64,
    cancel: impl Fn() -> bool,
) -> Result<Vec<GradientFeature>, LocalError> {
    extract_gradient_pyramid_with_recipe(
        image,
        policy,
        max_total_samples,
        GradientCellRecipe::Fixed,
        cancel,
    )
}

/// Original-coordinate gradient pyramid with explicit spatial histogram recipe.
///
/// # Errors
/// Same level, cumulative pixel/feature/sample, allocation and cancellation errors
/// as `extract_gradient_pyramid`.
pub fn extract_gradient_pyramid_with_recipe(
    image: &LinearRgbaView<'_>,
    policy: crate::pyramid::PyramidPolicy,
    max_total_samples: u64,
    recipe: GradientCellRecipe,
    cancel: impl Fn() -> bool,
) -> Result<Vec<GradientFeature>, LocalError> {
    extract_gradient_pyramid_selected(image, policy, max_total_samples, recipe, None, cancel)
}

/// Spatial gradient quotas are applied independently at every pyramid level.
/// Positions retain the original pixel-center mapping and descriptor recipe.
/// This primitive uses fallible allocation without managed reservations.
///
/// # Errors
/// Invalid grid/policy, cumulative pixel/feature/sample limits or cancellation.
pub fn extract_spatial_gradient_pyramid_with_recipe(
    image: &LinearRgbaView<'_>,
    policy: crate::pyramid::PyramidPolicy,
    max_total_samples: u64,
    recipe: GradientCellRecipe,
    grid: (usize, usize, usize),
    cancel: impl Fn() -> bool,
) -> Result<Vec<GradientFeature>, LocalError> {
    extract_gradient_pyramid_selected(image, policy, max_total_samples, recipe, Some(grid), cancel)
}

fn extract_gradient_pyramid_selected(
    image: &LinearRgbaView<'_>,
    policy: crate::pyramid::PyramidPolicy,
    max_total_samples: u64,
    recipe: GradientCellRecipe,
    grid: Option<(usize, usize, usize)>,
    cancel: impl Fn() -> bool,
) -> Result<Vec<GradientFeature>, LocalError> {
    if let Some((columns, rows, quota)) = grid {
        if columns == 0 || rows == 0 || quota == 0 {
            return Err(LocalError::Invalid);
        }
        if columns.checked_mul(rows).ok_or(LocalError::Budget)? > policy.local.max_features {
            return Err(LocalError::Budget);
        }
    }
    if policy.max_levels == 0 {
        return Err(LocalError::Invalid);
    }
    let per_level = u64::try_from(policy.local.max_features)
        .ok()
        .and_then(|n| n.checked_mul(512))
        .ok_or(LocalError::Budget)?;
    let required = u64::try_from(policy.max_levels)
        .ok()
        .and_then(|n| n.checked_mul(per_level))
        .ok_or(LocalError::Budget)?;
    if required > max_total_samples {
        return Err(LocalError::Budget);
    }
    crate::pyramid::extract_custom_levels(
        image,
        policy,
        &cancel,
        |view| extract_gradients_selected(view, policy.local, per_level, recipe, grid, &cancel),
        |feature, scale| feature.position = feature.position.map(|v| (v + 0.5) * f64::from(scale) - 0.5),
    )
}

/// Managed admission for gradient pyramid scratch and retained descriptors.
/// Conservative heap reservations precede extraction; returned ownership keeps
/// feature memory charged until its last clone drops. Not a process RSS bound.
///
/// # Errors
/// Invalid policy, checked work/memory/pixel/feature limits or cancellation.
#[cfg(feature = "raster")]
pub fn extract_gradient_pyramid_managed(
    image: &LinearRgbaView<'_>,
    policy: crate::pyramid::PyramidPolicy,
    max_total_samples: u64,
    budget: &rrrah_core::MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<GradientFeature>, LocalError> {
    extract_gradient_pyramid_with_recipe_managed(
        image,
        policy,
        max_total_samples,
        GradientCellRecipe::Fixed,
        budget,
        cancel,
    )
}

/// Managed explicit-recipe pyramid; retains the same conservative scratch and
/// last-owner output reservations. This does not bound whole-process RSS.
///
/// # Errors
/// Same checked work, memory, pixel, feature and cancellation errors as the fixed recipe.
#[cfg(feature = "raster")]
pub fn extract_gradient_pyramid_with_recipe_managed(
    image: &LinearRgbaView<'_>,
    policy: crate::pyramid::PyramidPolicy,
    max_total_samples: u64,
    recipe: GradientCellRecipe,
    budget: &rrrah_core::MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<GradientFeature>, LocalError> {
    extract_gradient_pyramid_selected_managed(image, policy, max_total_samples, recipe, None, budget, cancel)
}

/// Managed spatial-gradient pyramid. Spatial occupancy scratch is admitted
/// alongside detector/descriptor scratch, and retained output stays credited
/// until its final owner drops. No whole-process RSS guarantee is implied.
///
/// # Errors
/// Invalid grid/policy, work/memory/pixel/feature limits or cancellation.
#[cfg(feature = "raster")]
pub fn extract_spatial_gradient_pyramid_with_recipe_managed(
    image: &LinearRgbaView<'_>,
    policy: crate::pyramid::PyramidPolicy,
    max_total_samples: u64,
    recipe: GradientCellRecipe,
    grid: (usize, usize, usize),
    budget: &rrrah_core::MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<GradientFeature>, LocalError> {
    extract_gradient_pyramid_selected_managed(
        image,
        policy,
        max_total_samples,
        recipe,
        Some(grid),
        budget,
        cancel,
    )
}

#[cfg(feature = "raster")]
fn extract_gradient_pyramid_selected_managed(
    image: &LinearRgbaView<'_>,
    policy: crate::pyramid::PyramidPolicy,
    max_total_samples: u64,
    recipe: GradientCellRecipe,
    grid: Option<(usize, usize, usize)>,
    budget: &rrrah_core::MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<GradientFeature>, LocalError> {
    let grid_cells = if let Some((columns, rows, quota)) = grid {
        if columns == 0 || rows == 0 || quota == 0 {
            return Err(LocalError::Invalid);
        }
        let cells = columns.checked_mul(rows).ok_or(LocalError::Budget)?;
        if cells > policy.local.max_features {
            return Err(LocalError::Budget);
        }
        cells
    } else {
        0
    };
    if policy.max_levels == 0
        || !policy.local.minimum_corner_score.is_finite()
        || policy.local.minimum_corner_score < 0.
    {
        return Err(LocalError::Invalid);
    }
    let required = u64::try_from(policy.local.max_features)
        .ok()
        .and_then(|n| n.checked_mul(512))
        .and_then(|n| {
            u64::try_from(policy.max_levels)
                .ok()
                .and_then(|levels| n.checked_mul(levels))
        })
        .ok_or(LocalError::Budget)?;
    if required > max_total_samples {
        return Err(LocalError::Budget);
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let (width, height) = image.dimensions();
    let pixels = u64::from(width) * u64::from(height);
    if pixels > policy.local.max_pixels || pixels > policy.max_total_pixels {
        return Err(LocalError::Budget);
    }
    let bytes = |count: usize, size: usize| {
        count
            .checked_mul(size)
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(LocalError::Budget)
    };
    let retained_bytes = bytes(policy.max_total_features, std::mem::size_of::<GradientFeature>())?;
    let corners = bytes(
        policy.local.max_candidates,
        std::mem::size_of::<(f64, usize, usize)>(),
    )?;
    let local = bytes(policy.local.max_features, std::mem::size_of::<GradientFeature>())?;
    let detector = bytes(
        policy.local.max_features,
        std::mem::size_of::<crate::local::Feature>(),
    )?;
    let occupancy = bytes(grid_cells, std::mem::size_of::<usize>())?;
    let work = pixels
        .checked_mul(24)
        .and_then(|n| n.checked_add(corners))
        .and_then(|n| n.checked_add(occupancy))
        .and_then(|n| n.checked_add(local))
        .and_then(|n| n.checked_add(detector.checked_mul(2)?))
        .and_then(|n| n.checked_add(retained_bytes))
        .ok_or(LocalError::Budget)?;
    let _scratch = budget.try_reserve(work).map_err(|_| LocalError::Budget)?;
    let retained = budget
        .try_reserve(retained_bytes)
        .map_err(|_| LocalError::Budget)?;
    let features =
        extract_gradient_pyramid_selected(image, policy, max_total_samples, recipe, grid, &cancel)?;
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    retained.try_adopt(features).map_err(|_| LocalError::Budget)
}

/// Mutual unique nearest descriptors with explicit squared-distance evidence.
/// Equal-distance ties reject. Returned positions need independent geometry and
/// pixel confirmation. All allocations are fallible; no managed reservation.
///
/// # Errors
/// Invalid/nonunit/nonfinite descriptors, invalid policy, work/allocation limits
/// or cancellation; no partial correspondences are returned.
pub fn match_gradients(
    left: &[GradientFeature],
    right: &[GradientFeature],
    policy: GradientMatchPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Vec<crate::geometry::Correspondence>, LocalError> {
    match_gradients_selected(left, right, policy, None, cancel)
}

/// Explicit ratio alternative for multiscale features: the competing descriptor
/// must lie outside the supplied radius around the best feature in each image.
/// Mutual nearest indices still enforce one-to-one correspondence. Both directions
/// require a finite distinct-location competitor; spatial proximity does not prove
/// landmark identity. Two full descriptor passes are admitted before allocation.
/// # Errors
/// Invalid radius/features/policy, insufficient comparisons, or cancellation.
pub fn match_gradients_distinct_locations(
    left: &[GradientFeature],
    right: &[GradientFeature],
    policy: GradientMatchPolicy,
    competitor_radius: f64,
    cancel: impl Fn() -> bool,
) -> Result<Vec<crate::geometry::Correspondence>, LocalError> {
    if !competitor_radius.is_finite() || competitor_radius < 0. || !competitor_radius.powi(2).is_finite() {
        return Err(LocalError::Invalid);
    }
    match_gradients_selected(left, right, policy, Some(competitor_radius.powi(2)), cancel)
}

/// Managed variant of distinct-location matching. Input features are caller-owned.
/// Scratch tables and worst-case retained correspondences are admitted before
/// allocation; retained credit follows the result's last shared owner. This is
/// managed allocation admission, not a process RSS bound.
/// # Errors
/// Same validation/work/cancellation errors plus memory-budget refusal.
#[cfg(feature = "raster")]
pub fn match_gradients_distinct_locations_managed(
    left: &[GradientFeature],
    right: &[GradientFeature],
    policy: GradientMatchPolicy,
    competitor_radius: f64,
    budget: &rrrah_core::MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<crate::geometry::Correspondence>, LocalError> {
    if !competitor_radius.is_finite()
        || competitor_radius < 0.
        || !competitor_radius.powi(2).is_finite()
        || !policy.max_squared_distance.is_finite()
        || !(0. ..=4.).contains(&policy.max_squared_distance)
        || !policy.squared_ratio.is_finite()
        || policy.squared_ratio <= 0.
        || policy.squared_ratio >= 1.
    {
        return Err(LocalError::Invalid);
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let comparisons = left
        .len()
        .checked_mul(right.len())
        .and_then(|v| v.checked_mul(2))
        .and_then(|v| u64::try_from(v).ok())
        .ok_or(LocalError::Budget)?;
    if comparisons > policy.max_comparisons {
        return Err(LocalError::Budget);
    }
    let scratch = left
        .len()
        .checked_add(right.len())
        .and_then(|v| v.checked_mul(std::mem::size_of::<(f64, f64, usize)>()))
        .and_then(|v| u64::try_from(v).ok())
        .ok_or(LocalError::Budget)?;
    let retained = left
        .len()
        .min(right.len())
        .checked_mul(std::mem::size_of::<crate::geometry::Correspondence>())
        .and_then(|v| u64::try_from(v).ok())
        .ok_or(LocalError::Budget)?;
    let _scratch = budget.try_reserve(scratch).map_err(|_| LocalError::Budget)?;
    let retained = budget.try_reserve(retained).map_err(|_| LocalError::Budget)?;
    let output = match_gradients_distinct_locations(left, right, policy, competitor_radius, &cancel)?;
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    retained.try_adopt(output).map_err(|_| LocalError::Budget)
}

fn match_gradients_selected(
    left: &[GradientFeature],
    right: &[GradientFeature],
    policy: GradientMatchPolicy,
    distinct_radius_squared: Option<f64>,
    cancel: impl Fn() -> bool,
) -> Result<Vec<crate::geometry::Correspondence>, LocalError> {
    if !policy.max_squared_distance.is_finite()
        || !(0. ..=4.).contains(&policy.max_squared_distance)
        || !policy.squared_ratio.is_finite()
        || policy.squared_ratio <= 0.
        || policy.squared_ratio >= 1.
    {
        return Err(LocalError::Invalid);
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let work = u64::try_from(left.len())
        .ok()
        .and_then(|a| u64::try_from(right.len()).ok().and_then(|b| a.checked_mul(b)))
        .ok_or(LocalError::Budget)?;
    let work = work
        .checked_mul(if distinct_radius_squared.is_some() { 2 } else { 1 })
        .ok_or(LocalError::Budget)?;
    if work > policy.max_comparisons {
        return Err(LocalError::Budget);
    }
    for feature in left.iter().chain(right) {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        let d = &feature.descriptor.0;
        if feature.position.iter().any(|v| !v.is_finite())
            || d.iter().any(|v| !v.is_finite() || *v < 0.)
            || (d.iter().map(|v| v * v).sum::<f64>() - 1.).abs() > 1e-6
        {
            return Err(LocalError::Invalid);
        }
    }
    if left.is_empty() || right.is_empty() {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        return Ok(Vec::new());
    }
    let allocate = |len| {
        let mut v = Vec::new();
        v.try_reserve_exact(len).map_err(|_| LocalError::Budget)?;
        v.resize(len, (f64::INFINITY, f64::INFINITY, usize::MAX));
        Ok::<_, LocalError>(v)
    };
    let mut forward = allocate(left.len())?;
    let mut reverse = allocate(right.len())?;
    let update = |best: &mut (f64, f64, usize), distance, index| {
        if distance < best.0 {
            *best = (distance, best.0, index);
        } else {
            best.1 = best.1.min(distance);
        }
    };
    for (i, a) in left.iter().enumerate() {
        for (j, b) in right.iter().enumerate() {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let distance = a
                .descriptor
                .0
                .iter()
                .zip(b.descriptor.0.iter())
                .map(|(a, b)| (a - b) * (a - b))
                .sum();
            update(&mut forward[i], distance, j);
            update(&mut reverse[j], distance, i);
        }
    }
    if let Some(radius) = distinct_radius_squared {
        for best in forward.iter_mut().chain(reverse.iter_mut()) {
            best.1 = f64::INFINITY;
        }
        for (i, a) in left.iter().enumerate() {
            for (j, b) in right.iter().enumerate() {
                if cancel() {
                    return Err(LocalError::Cancelled);
                }
                let distance: f64 = a
                    .descriptor
                    .0
                    .iter()
                    .zip(b.descriptor.0.iter())
                    .map(|(x, y)| (x - y) * (x - y))
                    .sum();
                let nearest = right[forward[i].2].position;
                if (b.position[0] - nearest[0]).powi(2) + (b.position[1] - nearest[1]).powi(2) > radius {
                    forward[i].1 = forward[i].1.min(distance);
                }
                let nearest = left[reverse[j].2].position;
                if (a.position[0] - nearest[0]).powi(2) + (a.position[1] - nearest[1]).powi(2) > radius {
                    reverse[j].1 = reverse[j].1.min(distance);
                }
            }
        }
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(left.len().min(right.len()))
        .map_err(|_| LocalError::Budget)?;
    for (i, &(distance, second, j)) in forward.iter().enumerate() {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if (distinct_radius_squared.is_some() && !second.is_finite())
            || distance > policy.max_squared_distance
            || distance >= second * policy.squared_ratio
        {
            continue;
        }
        let (back, next, index) = reverse[j];
        if index == i
            && back < next * policy.squared_ratio
            && (distinct_radius_squared.is_none() || next.is_finite())
        {
            output.push(crate::geometry::Correspondence {
                source: left[i].position,
                target: right[j].position,
            });
        }
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(output)
}

/// Describe a rotated 16x16 sampling grid with 4x4 cells and eight direction
/// bins. Directions interpolate circularly; cells have fixed boundaries.
/// Gaussian weighting, unit normalization, clipping at .2 and renormalization
/// define this recipe. Position/orientation/scale are caller supplied.
/// `max_samples` counts grid sites (256); each site reads four bilinear samples.
/// Uniform patches return None. Only fully opaque sampled pixels are admitted.
/// No heap allocations or image-wide scratch are used.
///
/// # Errors
/// Invalid coordinates/scale/orientation, boundary/alpha refusal, insufficient
/// work admission, or cancellation. No partial descriptor is returned.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn describe_gradient(
    image: &LinearRgbaView<'_>,
    position: [f64; 2],
    scale: f64,
    angle: f64,
    max_samples: u64,
    cancel: impl Fn() -> bool,
) -> Result<Option<GradientDescriptor>, LocalError> {
    describe_gradient_cells(image, position, scale, angle, max_samples, false, cancel)
}

/// Explicit alternative to fixed spatial cells: each grid sample contributes to
/// its adjacent cell centers in x/y and adjacent circular direction bins.
/// Contributions beyond the 4x4 window are discarded. Sampling, Gaussian weight,
/// clipping and normalization retain the original recipe. This is not a SIFT
/// detector or scale-selection implementation. No upstream source is copied.
///
/// # Errors
/// Same coordinate, opacity, work and cancellation refusals as `describe_gradient`.
pub fn describe_gradient_interpolated(
    image: &LinearRgbaView<'_>,
    position: [f64; 2],
    scale: f64,
    angle: f64,
    max_samples: u64,
    cancel: impl Fn() -> bool,
) -> Result<Option<GradientDescriptor>, LocalError> {
    describe_gradient_cells(image, position, scale, angle, max_samples, true, cancel)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_arguments
)]
fn describe_gradient_cells(
    image: &LinearRgbaView<'_>,
    position: [f64; 2],
    scale: f64,
    angle: f64,
    max_samples: u64,
    interpolate: bool,
    cancel: impl Fn() -> bool,
) -> Result<Option<GradientDescriptor>, LocalError> {
    if !scale.is_finite() || scale <= 0. || !angle.is_finite() || position.iter().any(|v| !v.is_finite()) {
        return Err(LocalError::Invalid);
    }
    if max_samples < 256 {
        return Err(LocalError::Budget);
    }
    let (width, height) = image.dimensions();
    let sample = |x: f64, y: f64| -> Result<f64, LocalError> {
        if !x.is_finite()
            || !y.is_finite()
            || x < 0.
            || y < 0.
            || x + 1. >= f64::from(width)
            || y + 1. >= f64::from(height)
        {
            return Err(LocalError::Invalid);
        }
        let (ix, iy) = (x.floor() as u32, y.floor() as u32);
        let (tx, ty) = (x - f64::from(ix), y - f64::from(iy));
        let mut values = [0.; 4];
        for dy in 0..2 {
            for dx in 0..2 {
                let p = image
                    .pixel(ix + dx, iy + dy)
                    .ok_or(LocalError::Invalid)?
                    .map(|v| f64::from(f32::from_bits(v)));
                if p[3] != 1. {
                    return Err(LocalError::Invalid);
                }
                values[(dy * 2 + dx) as usize] = p[0] * 0.2126 + p[1] * 0.7152 + p[2] * 0.0722;
            }
        }
        // Difference-form interpolation preserves a constant exactly, including
        // rotated fractional sampling. Weighted sums can invent tiny gradients.
        let top = values[0] + (values[1] - values[0]) * tx;
        let bottom = values[2] + (values[3] - values[2]) * tx;
        Ok(top + (bottom - top) * ty)
    };
    let (sin, cos) = angle.sin_cos();
    let mut descriptor = [0.; 128];
    for row in 0..16 {
        for col in 0..16 {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let (u, v) = (f64::from(col) - 7.5, f64::from(row) - 7.5);
            let x = position[0] + scale * (u * cos - v * sin);
            let y = position[1] + scale * (u * sin + v * cos);
            let gx = sample(x + scale * cos, y + scale * sin)? - sample(x - scale * cos, y - scale * sin)?;
            let gy = sample(x - scale * sin, y + scale * cos)? - sample(x + scale * sin, y - scale * cos)?;
            let magnitude = gx.hypot(gy) * (-(u * u + v * v) / 128.).exp();
            let direction = gy.atan2(gx).rem_euclid(std::f64::consts::TAU) * 8. / std::f64::consts::TAU;
            let bin = direction.floor() as usize;
            let fraction = direction - bin as f64;
            if interpolate {
                let cx = (f64::from(col) + 0.5) / 4. - 0.5;
                let cy = (f64::from(row) + 0.5) / 4. - 0.5;
                let ix = cx.floor() as i32;
                let iy = cy.floor() as i32;
                let fx = cx - f64::from(ix);
                let fy = cy - f64::from(iy);
                for dy in 0..2 {
                    for dx in 0..2 {
                        let xcell = ix + dx;
                        let ycell = iy + dy;
                        if !(0..4).contains(&xcell) || !(0..4).contains(&ycell) {
                            continue;
                        }
                        let weight = magnitude
                            * if dx == 0 { 1. - fx } else { fx }
                            * if dy == 0 { 1. - fy } else { fy };
                        let cell = (ycell * 4 + xcell) as usize * 8;
                        descriptor[cell + bin % 8] += weight * (1. - fraction);
                        descriptor[cell + (bin + 1) % 8] += weight * fraction;
                    }
                }
            } else {
                let cell = ((row / 4) * 4 + col / 4) as usize * 8;
                descriptor[cell + bin % 8] += magnitude * (1. - fraction);
                descriptor[cell + (bin + 1) % 8] += magnitude * fraction;
            }
        }
    }
    let normalize = |values: &mut [f64; 128]| {
        let norm = values.iter().map(|v| v * v).sum::<f64>().sqrt();
        if norm == 0. || !norm.is_finite() {
            return false;
        }
        for v in values {
            *v /= norm;
        }
        true
    };
    if !normalize(&mut descriptor) {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        return Ok(None);
    }
    for v in &mut descriptor {
        *v = v.min(0.2);
    }
    if !normalize(&mut descriptor) {
        return Err(LocalError::Invalid);
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(Some(GradientDescriptor(descriptor)))
}
