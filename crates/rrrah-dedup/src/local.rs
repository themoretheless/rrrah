//! Native corner/BRIEF primitives with separate quarter-turn and oriented
//! recipes. Geometric correspondences remain candidates; continuous scale
//! normalization and broad real-image qualification remain separate work.
use crate::{geometry::Correspondence, linear::LinearRgbaView};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureRecipe {
    QuarterTurnBriefV1,
    OrientedBriefV1,
    OrientedScaleBriefV1,
    RankOrientedScaleBriefV1,
}
impl FeatureRecipe {
    pub(crate) fn has_scale_variants(self) -> bool {
        matches!(self, Self::OrientedScaleBriefV1 | Self::RankOrientedScaleBriefV1)
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Feature {
    pub recipe: FeatureRecipe,
    pub position: [f64; 2],
    pub descriptor: [u64; 4],
    /// Additional recipe-specific variants: quarter turns for legacy BRIEF,
    /// repeated orientation for `OrientedBriefV1`, patch scales for scale BRIEF.
    pub quarter_turns: [[u64; 4]; 3],
}
#[derive(Debug, Clone, Copy)]
pub struct LocalPolicy {
    pub max_pixels: u64,
    pub max_candidates: usize,
    pub max_features: usize,
    pub minimum_corner_score: f64,
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LocalError {
    #[error("invalid feature policy or coordinates")]
    Invalid,
    #[error("local feature extraction/matching exceeds supplied budget")]
    Budget,
    #[error("local feature extraction/matching cancelled")]
    Cancelled,
}

/// Extract deterministic strongest nonmax-suppressed corners and 256-bit BRIEF.
/// Quarter-turn patch variants are evaluated during matching.
/// Gray values are premultiplied linear-light luminance, with no HDR clipping.
/// Feature limit is an explicit top-response sampling policy, not exhaustive
/// detection. Arbitrary-angle and scale compensation are not yet implemented.
///
/// # Errors
/// Returns invalid policy, allocation/pixel/candidate budgets or cancellation.
pub fn extract(
    image: &LinearRgbaView<'_>,
    policy: LocalPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Vec<Feature>, LocalError> {
    extract_selected(image, policy, None, &cancel, false)
}

/// Spatial quotas bound how many strong corners one region can contribute.
/// Cells are normalized to image dimensions; source coordinates and BRIEF
/// semantics are unchanged. Geometry/pixels must still confirm candidates.
///
/// # Errors
/// Invalid grid/threshold, pixel/candidate/feature budgets or cancellation.
pub fn extract_spatial_oriented(
    image: &LinearRgbaView<'_>,
    policy: LocalPolicy,
    columns: usize,
    rows: usize,
    max_per_cell: usize,
    cancel: impl Fn() -> bool,
) -> Result<Vec<Feature>, LocalError> {
    if columns == 0 || rows == 0 || max_per_cell == 0 {
        return Err(LocalError::Invalid);
    }
    let cells = columns.checked_mul(rows).ok_or(LocalError::Budget)?;
    if cells > policy.max_features {
        return Err(LocalError::Budget);
    }
    let features = extract_selected(image, policy, Some((columns, rows, max_per_cell)), &cancel, false)?;
    oriented_from_features(
        image,
        features,
        &[0.75, 1.0, 1.25, 1.5],
        FeatureRecipe::OrientedScaleBriefV1,
        &cancel,
        false,
    )
}

// Grow detector buffers fallibly, requesting no more elements than the
// admitted count. An allocation failure discards the extraction result.
pub(crate) fn reserve_slot<T>(items: &mut Vec<T>, limit: usize) -> Result<(), LocalError> {
    if items.len() >= limit {
        return Err(LocalError::Budget);
    }
    if items.len() == items.capacity() {
        let next = items.capacity().saturating_mul(2).max(4).min(limit);
        items
            .try_reserve_exact(next - items.len())
            .map_err(|_| LocalError::Budget)?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // Keep detector scores, suppression and spatial admission together.
fn extract_selected(
    image: &LinearRgbaView<'_>,
    policy: LocalPolicy,
    grid: Option<(usize, usize, usize)>,
    cancel: &impl Fn() -> bool,
    reflected: bool,
) -> Result<Vec<Feature>, LocalError> {
    if !policy.minimum_corner_score.is_finite() || policy.minimum_corner_score < 0.0 {
        return Err(LocalError::Invalid);
    }
    let (width, height) = image.dimensions();
    let pixels = u64::from(width) * u64::from(height);
    if pixels > policy.max_pixels {
        return Err(LocalError::Budget);
    }
    let length = usize::try_from(pixels).map_err(|_| LocalError::Budget)?;
    let mut gray = Vec::new();
    gray.try_reserve_exact(length).map_err(|_| LocalError::Budget)?;
    for y in 0..height {
        for x in 0..width {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let p = image
                .pixel(if reflected { width - 1 - x } else { x }, y)
                .ok_or(LocalError::Invalid)?
                .map(|v| f64::from(f32::from_bits(v)));
            gray.push((p[0] * 0.2126 + p[1] * 0.7152 + p[2] * 0.0722) * p[3]);
        }
    }
    let w = usize::try_from(width).map_err(|_| LocalError::Budget)?;
    let h = usize::try_from(height).map_err(|_| LocalError::Budget)?;
    let mut corners = Vec::new();
    for y in 10..h.saturating_sub(10) {
        for x in 10..w.saturating_sub(10) {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let mut tensor = [0.0; 3];
            for sy in y - 1..=y + 1 {
                for sx in x - 1..=x + 1 {
                    let gx = (gray[sy * w + sx + 1] - gray[sy * w + sx - 1]) / 2.0;
                    let gy = (gray[(sy + 1) * w + sx] - gray[(sy - 1) * w + sx]) / 2.0;
                    tensor[0] += gx * gx;
                    tensor[1] += gy * gy;
                    tensor[2] += gx * gy;
                }
            }
            let score = (tensor[0] + tensor[1] - (tensor[0] - tensor[1]).hypot(2.0 * tensor[2])) / 2.0;
            if score <= policy.minimum_corner_score {
                continue;
            }
            if corners.len() >= policy.max_candidates {
                return Err(LocalError::Budget);
            }
            reserve_slot(&mut corners, policy.max_candidates)?;
            corners.push((score, x, y));
        }
    }
    corners.sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then_with(|| (a.2, a.1).cmp(&(b.2, b.1))));
    let mut selected: Vec<(usize, usize)> = Vec::new();
    let mut occupancy = if let Some((columns, rows, _)) = grid {
        let mut counts = Vec::new();
        counts
            .try_reserve_exact(columns * rows)
            .map_err(|_| LocalError::Budget)?;
        counts.resize(columns * rows, 0usize);
        Some(counts)
    } else {
        None
    };
    let mut features = Vec::new();
    for (_, x, y) in corners {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if features.len() >= policy.max_features {
            break;
        }
        let cell = if let Some((columns, rows, _)) = grid {
            let cy = y.checked_mul(rows).ok_or(LocalError::Budget)? / h;
            let cx = x.checked_mul(columns).ok_or(LocalError::Budget)? / w;
            Some(
                cy.checked_mul(columns)
                    .and_then(|v| v.checked_add(cx))
                    .ok_or(LocalError::Budget)?,
            )
        } else {
            None
        };
        if let (Some(index), Some((_, _, quota)), Some(counts)) = (cell, grid, &occupancy)
            && counts[index] >= quota
        {
            continue;
        }
        if selected
            .iter()
            .any(|&(sx, sy)| sx.abs_diff(x) <= 3 && sy.abs_diff(y) <= 3)
        {
            continue;
        }
        let variants = descriptors(&gray, w, x, y, &cancel)?;
        reserve_slot(&mut selected, policy.max_features)?;
        reserve_slot(&mut features, policy.max_features)?;
        selected.push((x, y));
        if let (Some(index), Some(counts)) = (cell, &mut occupancy) {
            counts[index] += 1;
        }
        features.push(Feature {
            recipe: FeatureRecipe::QuarterTurnBriefV1,
            position: [
                f64::from(u32::try_from(x).map_err(|_| LocalError::Invalid)?),
                f64::from(u32::try_from(y).map_err(|_| LocalError::Invalid)?),
            ],
            descriptor: variants[0],
            quarter_turns: [variants[1], variants[2], variants[3]],
        });
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(features)
}
fn offset(seed: &mut u64) -> (i32, i32) {
    let mut next = || {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        i32::try_from(*seed % 17).unwrap_or(0) - 8
    };
    (next(), next())
}

#[derive(Debug, Clone, Copy)]
pub struct MatchPolicy {
    /// Maximum feature pairs; each pair evaluates at most 16 descriptor variants.
    pub max_comparisons: u64,
    pub max_distance: u32,
}
/// Mutual unique nearest descriptors with a strict 3/4 ambiguity ratio on both
/// sides. Equal nearest distances never produce a correspondence. Distance is
/// recipe-specific evidence and needs calibration on a real image corpus.
/// Both inputs must carry the same recipe; mixed recipes are rejected.
///
/// # Errors
/// Returns comparison-budget excess or cancellation without partial matches.
pub fn match_features(
    left: &[Feature],
    right: &[Feature],
    policy: MatchPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Vec<Correspondence>, LocalError> {
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    if left.is_empty() || right.is_empty() {
        return Ok(Vec::new());
    }

    let comparisons = u64::try_from(left.len())
        .ok()
        .and_then(|a| u64::try_from(right.len()).ok().and_then(|b| a.checked_mul(b)))
        .ok_or(LocalError::Budget)?;
    if comparisons > policy.max_comparisons {
        return Err(LocalError::Budget);
    }
    let recipe = left[0].recipe;
    for feature in left.iter().chain(right) {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if feature.recipe != recipe {
            return Err(LocalError::Invalid);
        }
    }
    let mut l = Vec::new();
    l.try_reserve_exact(left.len()).map_err(|_| LocalError::Budget)?;
    l.resize(left.len(), (u32::MAX, u32::MAX, 0));
    let mut r = Vec::new();
    r.try_reserve_exact(right.len()).map_err(|_| LocalError::Budget)?;
    r.resize(right.len(), (u32::MAX, u32::MAX, 0));
    let mut work = 0;
    for (i, a) in left.iter().enumerate() {
        for (j, b) in right.iter().enumerate() {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            if work >= policy.max_comparisons {
                return Err(LocalError::Budget);
            }
            work += 1;
            let mut distance = 256;
            let left_variants = std::iter::once(&a.descriptor).chain(
                a.quarter_turns
                    .iter()
                    .take(if recipe.has_scale_variants() { 3 } else { 0 }),
            );
            for av in left_variants {
                for bv in std::iter::once(&b.descriptor).chain(&b.quarter_turns) {
                    distance = distance.min(
                        av.iter()
                            .zip(bv)
                            .map(|(&a, &b)| (a ^ b).count_ones())
                            .sum::<u32>(),
                    );
                }
            }
            update(&mut l[i], distance, j);
            update(&mut r[j], distance, i);
        }
    }
    let mut matches = Vec::new();
    for (i, &(distance, second, j)) in l.iter().enumerate() {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if distance > policy.max_distance || u64::from(distance) * 4 >= u64::from(second) * 3 {
            continue;
        }
        let (reverse, next, index) = r[j];
        if index == i && u64::from(reverse) * 4 < u64::from(next) * 3 {
            reserve_slot(&mut matches, left.len().min(right.len()))?;
            matches.push(Correspondence {
                source: left[i].position,
                target: right[j].position,
            });
        }
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(matches)
}
fn update(best: &mut (u32, u32, usize), distance: u32, index: usize) {
    if distance < best.0 {
        *best = (distance, best.0, index);
    } else {
        best.1 = best.1.min(distance);
    }
}

fn rotate(x: i32, y: i32, turn: usize) -> (i32, i32) {
    match turn {
        0 => (x, y),
        1 => (-y, x),
        2 => (-x, -y),
        _ => (y, -x),
    }
}

fn descriptors(
    gray: &[f64],
    w: usize,
    x: usize,
    y: usize,
    cancel: &impl Fn() -> bool,
) -> Result<[[u64; 4]; 4], LocalError> {
    let mut variants = [[0; 4]; 4];
    let mut seed = 0x4d59_5df4_d0f3_3173_u64;
    for bit in 0..256 {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        let (ax, ay) = offset(&mut seed);
        let (bx, by) = offset(&mut seed);
        let sample = |dx: i32, dy: i32| -> Result<f64, LocalError> {
            let sx = x.checked_add_signed(dx as isize).ok_or(LocalError::Invalid)?;
            let sy = y.checked_add_signed(dy as isize).ok_or(LocalError::Invalid)?;
            gray.get(sy * w + sx).copied().ok_or(LocalError::Invalid)
        };
        for (turn, descriptor) in variants.iter_mut().enumerate() {
            let (ax, ay) = rotate(ax, ay, turn);
            let (bx, by) = rotate(bx, by, turn);
            if sample(ax, ay)? < sample(bx, by)? {
                descriptor[bit / 64] |= 1 << (bit % 64);
            }
        }
    }
    Ok(variants)
}

/// Extract local-intensity-moment oriented BRIEF descriptors. Arbitrary patch
/// angles use bilinear samples, retaining native linear-light/alpha semantics.
/// This recipe is separate from quarter-turn BRIEF; matching mixed recipes is
/// invalid. Continuous scale invariance and real-corpus thresholds remain
/// caller qualification work. Orientation-ambiguous/edge patches are omitted.
///
/// # Errors
/// Returns pixel/corner/feature budgets, invalid samples or cancellation.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)] // Admitted u32 pixel coordinates are exact in f64.
pub fn extract_oriented(
    image: &LinearRgbaView<'_>,
    policy: LocalPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Vec<Feature>, LocalError> {
    oriented_variants(image, policy, &[1.0], FeatureRecipe::OrientedBriefV1, cancel)
}

/// Four fixed patch scales (0.75, 1, 1.25, 1.5) with independent orientation
/// normalization. Feature positions stay in original coordinates; matching tries
/// all 16 variant pairs and geometry estimates the actual continuous transform.
/// These finite scale hypotheses do not guarantee arbitrary-scale invariance.
///
/// # Errors
/// Returns pixel/corner/feature budgets, invalid samples or cancellation.
pub fn extract_multiscale_oriented(
    image: &LinearRgbaView<'_>,
    policy: LocalPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Vec<Feature>, LocalError> {
    oriented_variants(
        image,
        policy,
        &[0.75, 1.0, 1.25, 1.5],
        FeatureRecipe::OrientedScaleBriefV1,
        cancel,
    )
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)] // Admitted u32 coordinates and bounded patch scales.
fn oriented_variants(
    image: &LinearRgbaView<'_>,
    policy: LocalPolicy,
    scales: &[f64],
    recipe: FeatureRecipe,
    cancel: impl Fn() -> bool,
) -> Result<Vec<Feature>, LocalError> {
    let features = extract(image, policy, &cancel)?;
    oriented_from_features(image, features, scales, recipe, &cancel, false)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn oriented_from_features(
    image: &LinearRgbaView<'_>,
    features: Vec<Feature>,
    scales: &[f64],
    recipe: FeatureRecipe,
    cancel: &impl Fn() -> bool,
    reflected: bool,
) -> Result<Vec<Feature>, LocalError> {
    let (width, height) = image.dimensions();
    let (w, h) = (width as usize, height as usize);
    let margin = (scales.iter().copied().fold(1.0_f64, f64::max) * 12.0).ceil() as usize + 1;
    let mut gray = Vec::new();
    gray.try_reserve_exact(w * h).map_err(|_| LocalError::Budget)?;
    for y in 0..height {
        for x in 0..width {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let p = image
                .pixel(if reflected { width - 1 - x } else { x }, y)
                .ok_or(LocalError::Invalid)?
                .map(|v| f64::from(f32::from_bits(v)));
            gray.push((p[0] * 0.2126 + p[1] * 0.7152 + p[2] * 0.0722) * p[3]);
        }
    }
    let sample = |px: f64, py: f64| {
        let (ix, iy) = (px.floor() as usize, py.floor() as usize);
        let (tx, ty) = (px - ix as f64, py - iy as f64);
        let row = |yy| gray[yy * w + ix] * (1.0 - tx) + gray[yy * w + ix + 1] * tx;
        row(iy) * (1.0 - ty) + row(iy + 1) * ty
    };
    let mut output = Vec::new();
    output
        .try_reserve_exact(features.len())
        .map_err(|_| LocalError::Budget)?;
    for mut feature in features {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        let (x, y) = (feature.position[0] as usize, feature.position[1] as usize);
        if x < margin || y < margin || x + margin >= w || y + margin >= h {
            continue;
        }
        let mut variants = [[0_u64; 4]; 4];
        let mut valid = true;
        for (index, &scale) in scales.iter().enumerate() {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let mut moment = [0.0_f64; 2];
            for dy in -9_i32..=9 {
                for dx in -9_i32..=9 {
                    if dx * dx + dy * dy > 81 {
                        continue;
                    }
                    let intensity =
                        sample(x as f64 + f64::from(dx) * scale, y as f64 + f64::from(dy) * scale);
                    moment[0] += f64::from(dx) * intensity;
                    moment[1] += f64::from(dy) * intensity;
                }
            }
            let norm = moment[0].hypot(moment[1]);
            if !norm.is_finite() || norm <= f64::EPSILON {
                valid = false;
                break;
            }
            let (cos, sin) = (moment[0] / norm, moment[1] / norm);
            let steered = |dx: i32, dy: i32| {
                sample(
                    x as f64 + scale * (f64::from(dx) * cos - f64::from(dy) * sin),
                    y as f64 + scale * (f64::from(dx) * sin + f64::from(dy) * cos),
                )
            };
            let mut seed = 0x4d59_5df4_d0f3_3173_u64;
            for bit in 0..256 {
                if cancel() {
                    return Err(LocalError::Cancelled);
                }
                let (ax, ay) = offset(&mut seed);
                let (bx, by) = offset(&mut seed);
                if steered(ax, ay) < steered(bx, by) {
                    variants[index][bit / 64] |= 1 << (bit % 64);
                }
            }
        }
        if !valid {
            continue;
        }
        if scales.len() == 1 {
            variants = [variants[0]; 4];
        }
        feature.recipe = recipe;
        feature.descriptor = variants[0];
        feature.quarter_turns = [variants[1], variants[2], variants[3]];
        output.push(feature);
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(output)
}

/// Extract oriented BRIEF on a horizontal reflection, returning positions in
/// the original image coordinates. Samples are read in reverse without an RGBA
/// copy; descriptors retain the ordinary oriented recipe for cross-image matching.
/// Match these to ordinary features and require reflected geometry/pixel evidence.
///
/// # Errors
/// Invalid policy, pixel/corner/feature/allocation budget or cancellation;
/// no partial feature list is returned.
pub fn extract_reflected_oriented(
    image: &LinearRgbaView<'_>,
    policy: LocalPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Vec<Feature>, LocalError> {
    extract_reflected_variants(
        image,
        policy,
        &[1.0],
        FeatureRecipe::OrientedBriefV1,
        None,
        cancel,
    )
}

/// Reflected four-scale oriented BRIEF, with original source coordinates.
/// Uses the same finite patch scales and recipe as ordinary multiscale extraction.
///
/// # Errors
/// Invalid policy, exceeded resource/allocation budgets or cancellation.
pub fn extract_reflected_multiscale_oriented(
    image: &LinearRgbaView<'_>,
    policy: LocalPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Vec<Feature>, LocalError> {
    extract_reflected_variants(
        image,
        policy,
        &[0.75, 1.0, 1.25, 1.5],
        FeatureRecipe::OrientedScaleBriefV1,
        None,
        cancel,
    )
}

fn extract_reflected_variants(
    image: &LinearRgbaView<'_>,
    policy: LocalPolicy,
    scales: &[f64],
    recipe: FeatureRecipe,
    grid: Option<(usize, usize, usize)>,
    cancel: impl Fn() -> bool,
) -> Result<Vec<Feature>, LocalError> {
    let features = extract_selected(image, policy, grid, &cancel, true)?;
    let mut features = oriented_from_features(image, features, scales, recipe, &cancel, true)?;
    let last_x = f64::from(image.dimensions().0 - 1);
    for feature in &mut features {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        feature.position[0] = last_x - feature.position[0];
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(features)
}

/// Spatially capped reflected multiscale features in original coordinates.
/// Quotas apply to cells in the mirrored extraction grid; original positions
/// are returned for reflected geometry and pixel verification.
///
/// # Errors
/// Invalid grid/policy, exhausted resources or cancellation without partial features.
pub fn extract_reflected_spatial_oriented(
    image: &LinearRgbaView<'_>,
    policy: LocalPolicy,
    columns: usize,
    rows: usize,
    max_per_cell: usize,
    cancel: impl Fn() -> bool,
) -> Result<Vec<Feature>, LocalError> {
    if columns == 0 || rows == 0 || max_per_cell == 0 {
        return Err(LocalError::Invalid);
    }
    let cells = columns.checked_mul(rows).ok_or(LocalError::Budget)?;
    if cells > policy.max_features {
        return Err(LocalError::Budget);
    }
    extract_reflected_variants(
        image,
        policy,
        &[0.75, 1.0, 1.25, 1.5],
        FeatureRecipe::OrientedScaleBriefV1,
        Some((columns, rows, max_per_cell)),
        cancel,
    )
}
