//! Ordinal luminance candidates; original linear pixels remain verification evidence.
use crate::{
    linear::LinearRgbaView,
    local::{Feature, FeatureRecipe, LocalError, LocalPolicy, extract_multiscale_oriented},
};
/// Extract oriented multiscale BRIEF from global luminance ranks. A strictly
/// increasing neutral opaque transfer preserves ranks; RGB channel transforms,
/// changed crops and quantization are not promised invariant. The recipe is
/// distinct so raw-intensity descriptors cannot silently be mixed with ranks.
///
/// # Errors
/// Pixel/allocation/extraction budgets, invalid layout or cancellation.
/// Sorting checks cancellation before and after; its comparator is indivisible.
pub fn extract_rank_oriented(
    image: &LinearRgbaView<'_>,
    policy: LocalPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Vec<Feature>, LocalError> {
    if !policy.minimum_corner_score.is_finite() || policy.minimum_corner_score < 0.0 {
        return Err(LocalError::Invalid);
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let (width, height) = image.dimensions();
    let count = u64::from(width) * u64::from(height);
    if count > policy.max_pixels {
        return Err(LocalError::Budget);
    }
    let count = usize::try_from(count).map_err(|_| LocalError::Budget)?;
    let mut order = Vec::new();
    order.try_reserve_exact(count).map_err(|_| LocalError::Budget)?;
    let mut ranked = Vec::new();
    ranked
        .try_reserve_exact(count.checked_mul(4).ok_or(LocalError::Budget)?)
        .map_err(|_| LocalError::Budget)?;
    for y in 0..height {
        for x in 0..width {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let p = image.rgba(x, y).ok_or(LocalError::Invalid)?;
            let intensity = (f64::from(p[0]) * 0.2126 + f64::from(p[1]) * 0.7152 + f64::from(p[2]) * 0.0722)
                * f64::from(p[3]);
            order.push((intensity, ranked.len() / 4));
            ranked.extend_from_slice(&[0.0, 0.0, 0.0, p[3]]);
        }
    }
    order.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let mut begin = 0;
    while begin < count {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        let mut end = begin + 1;
        while end < count && order[end].0.total_cmp(&order[begin].0).is_eq() {
            end += 1;
        }
        #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
        let rank = (f64::midpoint(begin as f64, end as f64) / count as f64) as f32;
        for &(_, index) in &order[begin..end] {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            ranked[index * 4..index * 4 + 3].fill(rank);
        }
        begin = end;
    }
    drop(order);
    let view = LinearRgbaView::new(width, height, &ranked, policy.max_pixels, &cancel)
        .map_err(|_| LocalError::Invalid)?;
    let mut features = extract_multiscale_oriented(&view, policy, &cancel)?;
    for feature in &mut features {
        feature.recipe = FeatureRecipe::RankOrientedScaleBriefV1;
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(features)
}
