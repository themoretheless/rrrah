//! Local ordinal evidence under supplied geometry, never pixel/file equality.
//! No allocation. Neutral monotone transfers preserve comparisons at exact
//! mapped pixels; interpolation, colored light and quantization can change them.
use crate::{geometry::ProjectiveTransform, linear::LinearRgbaView};

#[derive(Debug, Clone, Copy)]
pub struct RankRegionPolicy {
    pub radius: u32,
    pub minimum_contrast: f64,
    pub minimum_pairs: u64,
    pub maximum_sites: u64,
    pub maximum_pixel_reads: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankRegionEvidence {
    pub sites: u64,
    pub valid_sites: u64,
    pub informative_pairs: u64,
    pub agreeing_pairs: u64,
    /// Conservative tap read accounting, including abandoned edge windows.
    pub pixel_reads: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RankRegionError {
    #[error("invalid rank-region input or unsupported pixels")]
    Invalid,
    #[error("rank-region work budget exceeded")]
    Budget,
    #[error("rank-region comparison cancelled")]
    Cancelled,
    #[error("insufficient informative local comparisons")]
    Uninformative,
}
fn rectangle(r: [u32; 4], image: &LinearRgbaView<'_>) -> bool {
    let (w, h) = image.dimensions();
    r[2] > 0
        && r[3] > 0
        && r[0].checked_add(r[2]).is_some_and(|x| x <= w)
        && r[1].checked_add(r[3]).is_some_and(|y| y <= h)
}
pub(crate) fn luminance(image: &LinearRgbaView<'_>, x: u32, y: u32) -> Result<f64, RankRegionError> {
    let p = image.rgba(x, y).ok_or(RankRegionError::Invalid)?;
    if p[3] != 1. || p[..3].iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)) {
        return Err(RankRegionError::Invalid);
    }
    Ok(f64::from(p[0]) * 0.2126 + f64::from(p[1]) * 0.7152 + f64::from(p[2]) * 0.0722)
}
pub(crate) fn interpolated(image: &LinearRgbaView<'_>, p: [f64; 2]) -> Result<Option<f64>, RankRegionError> {
    let (w, h) = image.dimensions();
    if p.iter().any(|v| !v.is_finite() || *v < 0.) || p[0] + 1. >= f64::from(w) || p[1] + 1. >= f64::from(h) {
        return Ok(None);
    }
    let x = p[0].floor() as u32;
    let y = p[1].floor() as u32;
    let tx = p[0] - f64::from(x);
    let ty = p[1] - f64::from(y);
    let a = luminance(image, x, y)?;
    let b = luminance(image, x + 1, y)?;
    let c = luminance(image, x, y + 1)?;
    let d = luminance(image, x + 1, y + 1)?;
    Ok(Some((a + (b - a) * tx) * (1. - ty) + (c + (d - c) * tx) * ty))
}
/// Eight neighbor/center comparisons per target center. Source centers are
/// clipped to `source_region`; neighbor support uses whole images. Both images
/// must contain opaque display-range linear RGB. Work is admitted before reads.
/// Evidence counts describe only this supplied region and do not classify copies.
pub fn compare_rank_region(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    model: ProjectiveTransform,
    source_region: [u32; 4],
    target_region: [u32; 4],
    policy: RankRegionPolicy,
    cancel: impl Fn() -> bool,
) -> Result<RankRegionEvidence, RankRegionError> {
    compare_filtered_rank_region(
        source,
        target,
        model,
        source_region,
        target_region,
        policy,
        0,
        cancel,
    )
}

/// Box-average each center and neighbor in target coordinates before comparing
/// order. Radius zero preserves the unfiltered API. Declared work includes all
/// 9 windows and bilinear source reads; constant storage and cancellable taps.
/// Monotone pixel transforms need not commute with averaging: no such invariance
/// is promised for this filtered evidence. This function makes no copy decision.
pub fn compare_filtered_rank_region(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    model: ProjectiveTransform,
    source_region: [u32; 4],
    target_region: [u32; 4],
    policy: RankRegionPolicy,
    filter_radius: u32,
    cancel: impl Fn() -> bool,
) -> Result<RankRegionEvidence, RankRegionError> {
    if cancel() {
        return Err(RankRegionError::Cancelled);
    }
    if !rectangle(source_region, source)
        || !rectangle(target_region, target)
        || policy.radius == 0
        || !policy.minimum_contrast.is_finite()
        || !(0.0..=1.0).contains(&policy.minimum_contrast)
        || policy.minimum_contrast == 0.
        || policy.minimum_pairs == 0
    {
        return Err(RankRegionError::Invalid);
    }
    let inverse = model.inverse().map_err(|_| RankRegionError::Invalid)?;
    let [x, y, w, h] = target_region;
    let sites = u64::from(w) * u64::from(h);
    let side = u64::from(filter_radius) * 2 + 1;
    let taps = side.checked_mul(side).ok_or(RankRegionError::Budget)?;
    let reads = sites
        .checked_mul(45)
        .and_then(|v| v.checked_mul(taps))
        .ok_or(RankRegionError::Budget)?;
    let filter = i64::from(filter_radius);
    if sites > policy.maximum_sites || reads > policy.maximum_pixel_reads {
        return Err(RankRegionError::Budget);
    }
    let radius = i64::from(policy.radius);
    let offsets = [
        (0, 0),
        (-radius, -radius),
        (0, -radius),
        (radius, -radius),
        (-radius, 0),
        (radius, 0),
        (-radius, radius),
        (0, radius),
        (radius, radius),
    ];
    let (tw, th) = target.dimensions();
    let [sx, sy, sw, sh] = source_region;
    let mut evidence = RankRegionEvidence {
        sites,
        valid_sites: 0,
        informative_pairs: 0,
        agreeing_pairs: 0,
        pixel_reads: 0,
    };
    for py in y..y + h {
        for px in x..x + w {
            if cancel() {
                return Err(RankRegionError::Cancelled);
            }
            let center = inverse
                .apply([f64::from(px), f64::from(py)])
                .ok_or(RankRegionError::Invalid)?;
            if center[0] < f64::from(sx)
                || center[1] < f64::from(sy)
                || center[0] >= f64::from(sx + sw)
                || center[1] >= f64::from(sy + sh)
            {
                continue;
            }
            let mut values = [[0.; 2]; 9];
            let mut valid = true;
            for (index, (dx, dy)) in offsets.iter().enumerate() {
                if cancel() {
                    return Err(RankRegionError::Cancelled);
                }
                let mut sums = [0.; 2];
                'window: for fy in -filter..=filter {
                    for fx in -filter..=filter {
                        if filter_radius > 0 && cancel() {
                            return Err(RankRegionError::Cancelled);
                        }
                        let qx = i64::from(px) + dx + fx;
                        let qy = i64::from(py) + dy + fy;
                        if qx < 0 || qy < 0 || qx >= i64::from(tw) || qy >= i64::from(th) {
                            valid = false;
                            break 'window;
                        }
                        evidence.pixel_reads += 5;
                        let mapped = inverse
                            .apply([qx as f64, qy as f64])
                            .ok_or(RankRegionError::Invalid)?;
                        let Some(a) = interpolated(source, mapped)? else {
                            valid = false;
                            break 'window;
                        };
                        sums[0] += a;
                        sums[1] += luminance(target, qx as u32, qy as u32)?;
                    }
                }
                if !valid {
                    break;
                }
                values[index] = sums.map(|v| v / taps as f64);
            }
            if !valid {
                continue;
            }
            evidence.valid_sites += 1;
            for value in &values[1..] {
                let a = value[0] - values[0][0];
                let b = value[1] - values[0][1];
                if a.abs() < policy.minimum_contrast || b.abs() < policy.minimum_contrast {
                    continue;
                }
                evidence.informative_pairs += 1;
                evidence.agreeing_pairs += u64::from(a.is_sign_positive() == b.is_sign_positive());
            }
        }
    }
    if cancel() {
        return Err(RankRegionError::Cancelled);
    }
    if evidence.informative_pairs < policy.minimum_pairs {
        return Err(RankRegionError::Uninformative);
    }
    Ok(evidence)
}
