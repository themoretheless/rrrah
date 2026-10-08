//! Explicit known-region color evidence; geometric discovery and identity are caller policy.
use crate::{
    affine_color::{
        AffineColorError, AffineColorModel, AffineColorPolicy, ColorValidation, RgbPair, fit_affine_color,
        validate_affine_color,
    },
    geometry::ProjectiveTransform,
    linear::LinearRgbaView,
};
use rrrah_core::MemoryBudget;
#[derive(Debug, Clone, Copy)]
pub struct AffineRegionPolicy {
    pub radius: u32,
    pub maximum_sites: u64,
    pub maximum_pixel_reads: u64,
    pub color: AffineColorPolicy,
    pub tolerance: f64,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AffineRegionEvidence {
    pub model: AffineColorModel,
    pub heldout: ColorValidation,
    pub training_samples: usize,
    pub pixel_reads: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AffineRegionError {
    #[error("invalid or unsupported region/pixel input")]
    Invalid,
    #[error("region memory or work budget exceeded")]
    Budget,
    #[error("region comparison cancelled")]
    Cancelled,
    #[error(transparent)]
    Color(#[from] AffineColorError),
}
fn rgb(view: &LinearRgbaView<'_>, x: u32, y: u32) -> Result<[f64; 3], AffineRegionError> {
    let p = view.rgba(x, y).ok_or(AffineRegionError::Invalid)?;
    if p[3] != 1. || p[..3].iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)) {
        return Err(AffineRegionError::Invalid);
    }
    Ok([p[0] as f64, p[1] as f64, p[2] as f64])
}
fn interpolate(view: &LinearRgbaView<'_>, p: [f64; 2]) -> Result<Option<[f64; 3]>, AffineRegionError> {
    let (w, h) = view.dimensions();
    if p.iter().any(|v| !v.is_finite() || *v < 0.) || p[0] + 1. >= f64::from(w) || p[1] + 1. >= f64::from(h) {
        return Ok(None);
    }
    let (x, y) = (p[0].floor() as u32, p[1].floor() as u32);
    let (tx, ty) = (p[0] - f64::from(x), p[1] - f64::from(y));
    let a = rgb(view, x, y)?;
    let b = rgb(view, x + 1, y)?;
    let c = rgb(view, x, y + 1)?;
    let d = rgb(view, x + 1, y + 1)?;
    Ok(Some(std::array::from_fn(|i| {
        let top = a[i] + (b[i] - a[i]) * tx;
        let bottom = c[i] + (d[i] - c[i]) * tx;
        top + (bottom - top) * ty
    })))
}
fn rectangle_valid(r: [u32; 4], dims: (u32, u32)) -> bool {
    r[2] > 0
        && r[3] > 0
        && r[0].checked_add(r[2]).is_some_and(|x| x <= dims.0)
        && r[1].checked_add(r[3]).is_some_and(|y| y <= dims.1)
}
/// Map target centers to source, clip centers to the supplied source region,
/// and filter with whole-image support. Checker cells8x8 split fitting/heldout.
/// Both sample vector payloads are reserved before allocation and released on
/// every exit. Allocator overhead, borrowed images and caller metadata excluded.
/// No decision or partially fitted evidence is returned on any error.
pub fn verify_affine_region(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    model: ProjectiveTransform,
    source_region: [u32; 4],
    target_region: [u32; 4],
    policy: AffineRegionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AffineRegionEvidence, AffineRegionError> {
    if cancel() {
        return Err(AffineRegionError::Cancelled);
    }
    if !rectangle_valid(source_region, source.dimensions())
        || !rectangle_valid(target_region, target.dimensions())
        || !(1..=16).contains(&policy.radius)
        || !policy.tolerance.is_finite()
        || !(0.0..=1.0).contains(&policy.tolerance)
    {
        return Err(AffineRegionError::Invalid);
    }
    let inverse = model.inverse().map_err(|_| AffineRegionError::Invalid)?;
    let [x, y, w, h] = target_region;
    let sites = u64::from(w) * u64::from(h);
    let taps = u64::from(2 * policy.radius + 1).pow(2);
    let maximum = sites
        .checked_mul(taps)
        .and_then(|v| v.checked_mul(5))
        .ok_or(AffineRegionError::Budget)?;
    if sites > policy.maximum_sites
        || sites > policy.color.maximum_samples as u64
        || maximum > policy.maximum_pixel_reads
    {
        return Err(AffineRegionError::Budget);
    }
    let count = usize::try_from(sites).map_err(|_| AffineRegionError::Budget)?;
    let bytes = sites
        .checked_mul(std::mem::size_of::<RgbPair>() as u64)
        .and_then(|v| v.checked_mul(2))
        .ok_or(AffineRegionError::Budget)?;
    let _credit = budget.try_reserve(bytes).map_err(|_| AffineRegionError::Budget)?;
    let mut training = Vec::new();
    let mut held = Vec::new();
    training
        .try_reserve_exact(count)
        .map_err(|_| AffineRegionError::Budget)?;
    held.try_reserve_exact(count)
        .map_err(|_| AffineRegionError::Budget)?;
    if training.capacity() > count || held.capacity() > count {
        return Err(AffineRegionError::Budget);
    }
    let (tw, th) = target.dimensions();
    let mut work = 0;
    let radius = i64::from(policy.radius);
    for py in y..y + h {
        for px in x..x + w {
            if cancel() {
                return Err(AffineRegionError::Cancelled);
            }
            let center = inverse
                .apply([f64::from(px), f64::from(py)])
                .ok_or(AffineRegionError::Invalid)?;
            let [ox, oy, ow, oh] = source_region;
            if center[0] < f64::from(ox)
                || center[1] < f64::from(oy)
                || center[0] >= f64::from(ox + ow)
                || center[1] >= f64::from(oy + oh)
            {
                continue;
            }
            let mut aa = [0.; 3];
            let mut bb = [0.; 3];
            let mut valid = true;
            'window: for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if cancel() {
                        return Err(AffineRegionError::Cancelled);
                    }
                    work += 5;
                    let qx = i64::from(px) + dx;
                    let qy = i64::from(py) + dy;
                    if qx < 0 || qy < 0 || qx >= i64::from(tw) || qy >= i64::from(th) {
                        valid = false;
                        break 'window;
                    }
                    let Some(mapped) = inverse.apply([qx as f64, qy as f64]) else {
                        valid = false;
                        break 'window;
                    };
                    let Some(a) = interpolate(source, mapped)? else {
                        valid = false;
                        break 'window;
                    };
                    let b = rgb(target, qx as u32, qy as u32)?;
                    for i in 0..3 {
                        aa[i] += a[i] / taps as f64;
                        bb[i] += b[i] / taps as f64;
                    }
                }
            }
            if valid {
                let pair = RgbPair {
                    source: aa,
                    target: bb,
                };
                if ((px - x) / 8 + (py - y) / 8) % 2 == 0 {
                    training.push(pair);
                } else {
                    held.push(pair);
                }
            }
        }
    }
    let fitted = fit_affine_color(&training, policy.color, &cancel)?;
    let heldout = validate_affine_color(
        fitted,
        &held,
        policy.tolerance,
        policy.color.maximum_samples,
        &cancel,
    )?;
    if cancel() {
        return Err(AffineRegionError::Cancelled);
    }
    Ok(AffineRegionEvidence {
        model: fitted,
        heldout,
        training_samples: training.len(),
        pixel_reads: work,
    })
}
