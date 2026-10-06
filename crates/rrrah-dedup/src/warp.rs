//! Pixel residuals under supplied geometry, with explicit overlap evidence.
//! Bilinear interpolation operates on premultiplied linear RGB and alpha.
//! Tolerance is caller policy, not a calibrated probability of duplication.

use crate::{geometry::Transform, linear::LinearRgbaView};
#[derive(Debug, Clone, Copy)]
pub struct WarpPolicy {
    pub tolerance: f64,
    pub max_source_pixels: u64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct WarpEvidence {
    pub source_pixels: u64,
    pub compared_pixels: u64,
    pub matched_pixels: u64,
    pub maximum_channel_error: f64,
    pub squared_error: f64,
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WarpError {
    #[error("invalid geometry, pixels or verification policy")]
    Invalid,
    #[error("pixel verification exceeds the supplied work budget; inconclusive")]
    Budget,
    #[error("pixel verification cancelled; inconclusive")]
    Cancelled,
    #[error(transparent)]
    Fit(#[from] PhotometricFitFailure),
}

/// Verify every source pixel center falling inside the transformed target.
/// Exposes source coverage and residuals rather than declaring whole images
/// equal. Caller must require enough overlap and qualify matching thresholds.
/// Crop/scale geometry can cover only a region; zero overlap proves nothing.
///
/// # Errors
/// Rejects invalid policies/transforms, work-budget excess and cancellation.
pub fn verify_pixels(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    transform: Transform,
    policy: WarpPolicy,
    cancel: impl Fn() -> bool,
) -> Result<WarpEvidence, WarpError> {
    verify_pixels_mapped(source, target, transform, policy, cancel, false)
}

/// Verify a source x reflection followed by the supplied similarity transform.
/// Coordinates and residuals use the original source image, including linear
/// HDR values and premultiplied alpha. Evidence does not declare identity.
///
/// # Errors
/// Invalid geometry/policy, pixel-work budget exhaustion or cancellation.
pub fn verify_reflected_pixels(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    transform: Transform,
    policy: WarpPolicy,
    cancel: impl Fn() -> bool,
) -> Result<WarpEvidence, WarpError> {
    verify_pixels_mapped(source, target, transform, policy, cancel, true)
}

fn verify_pixels_mapped(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    transform: Transform,
    policy: WarpPolicy,
    cancel: impl Fn() -> bool,
    reflected: bool,
) -> Result<WarpEvidence, WarpError> {
    if !policy.tolerance.is_finite()
        || policy.tolerance < 0.0
        || [
            transform.a,
            transform.b,
            transform.translation[0],
            transform.translation[1],
        ]
        .iter()
        .any(|v| !v.is_finite())
        || transform.a.hypot(transform.b) == 0.0
    {
        return Err(WarpError::Invalid);
    }
    verify_mapped_coordinates(source, target, policy, cancel, |[x,y]| {
        Some(transform.apply([if reflected { -x } else { x }, y]))
    })
}

/// Verify premultiplied linear pixel residuals under planar perspective geometry.
/// Returns explicit overlap counts, never a whole-image equality claim.
/// A horizon crossing the source rectangle or a singular matrix is invalid.
///
/// # Errors
/// Invalid model/policy, bounded pixel work exhaustion or cancellation.
pub fn verify_projective_pixels(
    source: &LinearRgbaView<'_>, target: &LinearRgbaView<'_>,
    transform: crate::geometry::ProjectiveTransform, policy: WarpPolicy,
    cancel: impl Fn() -> bool,
) -> Result<WarpEvidence, WarpError> {
    let m = transform.matrix;
    if m.iter().flatten().any(|v| !v.is_finite()) { return Err(WarpError::Invalid); }
    let scale = m.iter().flatten().fold(0.0_f64, |a,b| a.max(b.abs()));
    if scale == 0.0 { return Err(WarpError::Invalid); }
    let h = m.map(|row| row.map(|v| v/scale));
    let det = h[0][0]*(h[1][1]*h[2][2]-h[1][2]*h[2][1])
        -h[0][1]*(h[1][0]*h[2][2]-h[1][2]*h[2][0])
        +h[0][2]*(h[1][0]*h[2][1]-h[1][1]*h[2][0]);
    if !det.is_finite() || det == 0.0 { return Err(WarpError::Invalid); }
    let (w,h) = source.dimensions();
    let denominators = [[0.0,0.0],[f64::from(w-1),0.0],[0.0,f64::from(h-1)],[f64::from(w-1),f64::from(h-1)]]
        .map(|[x,y]| m[2][0]*x+m[2][1]*y+m[2][2]);
    if denominators.iter().any(|d| !d.is_finite() || *d == 0.0 || d.is_sign_positive()!=denominators[0].is_sign_positive()) {
        return Err(WarpError::Invalid);
    }
    let (tw,th) = target.dimensions();
    verify_mapped_coordinates(source,target,policy,cancel,|p| {
        let mut q = transform.apply(p)?;
        for (coordinate,last) in q.iter_mut().zip([f64::from(tw-1),f64::from(th-1)]) {
            let epsilon = 16.0*f64::EPSILON*last.max(1.0);
            if *coordinate < 0.0 && *coordinate >= -epsilon { *coordinate = 0.0; }
            if *coordinate > last && *coordinate <= last+epsilon { *coordinate = last; }
        }
        Some(q)
    })
}

/// Verify both source and target grids under inverse projective mappings.
/// Shared `max_source_pixels` admits the sum of both grids before either pass.
///
/// # Errors
/// Invalid/singular/horizon-crossing geometry, shared work refusal or cancellation.
pub fn verify_projective_bidirectional(
    source: &LinearRgbaView<'_>, target: &LinearRgbaView<'_>,
    transform: crate::geometry::ProjectiveTransform, policy: WarpPolicy,
    cancel: impl Fn() -> bool,
) -> Result<BidirectionalEvidence, WarpError> {
    let (sw,sh) = source.dimensions(); let (tw,th) = target.dimensions();
    let work = (u64::from(sw)*u64::from(sh)).checked_add(u64::from(tw)*u64::from(th)).ok_or(WarpError::Budget)?;
    if work > policy.max_source_pixels { return Err(WarpError::Budget); }
    let inverse = transform.inverse().map_err(|_| WarpError::Invalid)?;
    let forward = verify_projective_pixels(source,target,transform,policy,&cancel)?;
    let reverse = verify_projective_pixels(target,source,inverse,policy,&cancel)?;
    if cancel() { return Err(WarpError::Cancelled); }
    Ok(BidirectionalEvidence { forward, reverse })
}

fn verify_mapped_coordinates(
    source: &LinearRgbaView<'_>, target: &LinearRgbaView<'_>, policy: WarpPolicy,
    cancel: impl Fn() -> bool, mapping: impl Fn([f64;2]) -> Option<[f64;2]>,
) -> Result<WarpEvidence, WarpError> {
    if !policy.tolerance.is_finite() || policy.tolerance < 0.0 { return Err(WarpError::Invalid); }
    if cancel() { return Err(WarpError::Cancelled); }
    let (width, height) = source.dimensions();
    let source_pixels = u64::from(width) * u64::from(height);
    if source_pixels > policy.max_source_pixels {
        return Err(WarpError::Budget);
    }
    let mut result = WarpEvidence {
        source_pixels,
        compared_pixels: 0,
        matched_pixels: 0,
        maximum_channel_error: 0.0,
        squared_error: 0.0,
    };
    for y in 0..height {
        for x in 0..width {
            if cancel() {
                return Err(WarpError::Cancelled);
            }
            let position = mapping([f64::from(x), f64::from(y)]).ok_or(WarpError::Invalid)?;
            if position.iter().any(|v| !v.is_finite()) { return Err(WarpError::Invalid); }
            let Some(actual) = interpolate(target, position) else {
                continue;
            };
            let expected = premultiplied(source, x, y).ok_or(WarpError::Invalid)?;
            let mut maximum = 0.0_f64;
            for (a, b) in actual.into_iter().zip(expected) {
                let error = (a - b).abs();
                maximum = maximum.max(error);
                result.squared_error += error * error;
            }
            result.compared_pixels += 1;
            result.matched_pixels += u64::from(maximum <= policy.tolerance);
            result.maximum_channel_error = result.maximum_channel_error.max(maximum);
        }
    }
    if cancel() {
        return Err(WarpError::Cancelled);
    }
    Ok(result)
}

fn premultiplied(image: &LinearRgbaView<'_>, x: u32, y: u32) -> Option<[f64; 4]> {
    premultiplied_in_space(image, x, y, FilterColorSpace::LinearSrgb)
}
fn premultiplied_in_space(
    image: &LinearRgbaView<'_>,
    x: u32,
    y: u32,
    space: FilterColorSpace,
) -> Option<[f64; 4]> {
    let mut p = image.pixel(x, y)?.map(|bits| f64::from(f32::from_bits(bits)));
    if space == FilterColorSpace::EncodedSrgb {
        for c in &mut p[..3] {
            *c = encode_srgb(*c);
        }
    }
    Some([p[0] * p[3], p[1] * p[3], p[2] * p[3], p[3]])
}

fn interpolate(image: &LinearRgbaView<'_>, p: [f64; 2]) -> Option<[f64; 4]> {
    interpolate_in_space(image, p, FilterColorSpace::LinearSrgb)
}
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Validated finite image coordinates.
fn interpolate_in_space(
    image: &LinearRgbaView<'_>,
    p: [f64; 2],
    space: FilterColorSpace,
) -> Option<[f64; 4]> {
    let (width, height) = image.dimensions();
    if p[0] < 0.0 || p[1] < 0.0 || p[0] > f64::from(width - 1) || p[1] > f64::from(height - 1) {
        return None;
    }
    let x = p[0].floor() as u32;
    let y = p[1].floor() as u32;
    let dx = p[0] - f64::from(x);
    let dy = p[1] - f64::from(y);
    let mut result = [0.0; 4];
    for (sx, sy, weight) in [
        (x, y, (1.0 - dx) * (1.0 - dy)),
        ((x + 1).min(width - 1), y, dx * (1.0 - dy)),
        (x, (y + 1).min(height - 1), (1.0 - dx) * dy),
        ((x + 1).min(width - 1), (y + 1).min(height - 1), dx * dy),
    ] {
        for (out, value) in result
            .iter_mut()
            .zip(premultiplied_in_space(image, sx, sy, space)?)
        {
            *out += value * weight;
        }
    }
    Some(result)
}

#[derive(Debug, Clone, PartialEq)]
pub struct BidirectionalEvidence {
    pub forward: WarpEvidence,
    pub reverse: WarpEvidence,
}

/// Verify both pixel grids. Reverse verification catches target edits between
/// projected source centers when an image is enlarged. Budget applies to each
/// source grid separately; total work is bounded by twice the supplied cap.
/// Resampling kernels differ, so residual thresholds require corpus calibration.
///
/// # Errors
/// Returns invalid inverse geometry, work excess or cancellation without partial
/// evidence from either direction.
pub fn verify_bidirectional(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    transform: Transform,
    policy: WarpPolicy,
    cancel: impl Fn() -> bool,
) -> Result<BidirectionalEvidence, WarpError> {
    let den = transform.a * transform.a + transform.b * transform.b;
    if !den.is_finite() || den <= 0.0 {
        return Err(WarpError::Invalid);
    }
    let inverse = Transform {
        a: transform.a / den,
        b: -transform.b / den,
        translation: [
            (-transform.a * transform.translation[0] - transform.b * transform.translation[1]) / den,
            (transform.b * transform.translation[0] - transform.a * transform.translation[1]) / den,
        ],
    };
    let forward = verify_pixels(source, target, transform, policy, &cancel)?;
    let reverse = verify_pixels(target, source, inverse, policy, cancel)?;
    Ok(BidirectionalEvidence { forward, reverse })
}

/// Valid pixel data can still lack a qualified global photometric model.
/// These reasons are inconclusive fitting outcomes, not source/decode failures
/// and not evidence of unrelated images.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PhotometricFitFailure {
    #[error("only {observed} opaque samples; fitting requires {required}")]
    InsufficientSamples { observed: u64, required: u64 },
    #[error("insufficient variance in RGB channel {channel}")]
    LowVariance { channel: usize },
    #[error("fitted change outside caller policy in RGB channel {channel}")]
    OutsidePolicy { channel: usize },
}

/// Caller-bounded per-channel affine change, in linear RGB by default. Explicit
/// color filtering interprets gains/offsets/tolerance in its selected space.
/// Alpha is never fitted.
/// This is visual evidence only, not normalized-pixel equality.
#[derive(Debug, Clone, Copy)]
pub struct PhotometricPolicy {
    pub residual: WarpPolicy,
    pub minimum_samples: u64,
    pub minimum_variance: f64,
    pub minimum_gain: f64,
    pub maximum_gain: f64,
    pub maximum_offset: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct PhotometricEvidence {
    pub gain: [f64; 3],
    pub offset: [f64; 3],
    /// A bounded optimizer changed the otherwise out-of-policy unconstrained fit.
    pub constrained_channels: [bool; 3],
    pub fitted_samples: u64,
    pub pixels: WarpEvidence,
}

pub(crate) fn validate_photometric_policy(policy: PhotometricPolicy) -> Result<(), WarpError> {
    if policy.minimum_samples < 2
        || !policy.minimum_variance.is_finite()
        || policy.minimum_variance <= 0.0
        || !policy.minimum_gain.is_finite()
        || policy.minimum_gain <= 0.0
        || !policy.maximum_gain.is_finite()
        || policy.maximum_gain < policy.minimum_gain
        || !policy.maximum_offset.is_finite()
        || policy.maximum_offset < 0.0
        || !policy.residual.tolerance.is_finite()
        || policy.residual.tolerance < 0.0
    {
        return Err(WarpError::Invalid);
    }
    Ok(())
}

/// Fit one global affine color change on opaque overlap, then inspect every
/// overlapping pixel under that model. Stable online covariance needs constant
/// memory and two bounded grid passes. Transparent pixels do not train the fit;
/// residuals compare alpha unchanged and offsets multiplied by source alpha.
/// Low variance, insufficient opaque overlap or excessive fitted changes are
/// inconclusive errors. No tone map, clipping or spatially varying fit is used.
///
/// # Errors
/// Invalid/degenerate policy, geometry or fit, pixel budget, or cancellation.
pub fn verify_photometric(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    transform: Transform,
    policy: PhotometricPolicy,
    cancel: impl Fn() -> bool,
) -> Result<PhotometricEvidence, WarpError> {
    verify_photometric_grid(
        source,
        target,
        transform,
        GridPolicy {
            reflected: false,
            photometric: policy,
            radius: 0,
            space: FilterColorSpace::LinearSrgb,
            fit: PhotometricFitMode::RejectOutsidePolicy,
            fit_range: None,
            projection: None,
        },
        cancel,
    )
}
/// Inclusive straight-RGB bounds for fitting only. Residual verification still
/// covers every overlapping pixel, including samples outside this range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitSampleRange {
    pub minimum: f64,
    pub maximum: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct RangePhotometricEvidence {
    pub range: FitSampleRange,
    pub evidence: BidirectionalPhotometricEvidence,
}
pub(crate) fn validate_fit_range(range: FitSampleRange) -> Result<(), WarpError> {
    if !range.minimum.is_finite() || !range.maximum.is_finite() || range.minimum >= range.maximum {
        return Err(WarpError::Invalid);
    }
    Ok(())
}
/// Fit on explicitly bounded opaque RGB samples and verify all overlap in both directions.
/// No clipping, tone mapping or removal of residual samples is performed.
///
/// # Errors
/// Invalid range/policy/transform, insufficient fit samples or variance, resource limits or cancellation.
pub fn verify_photometric_bidirectional_in_range(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    transform: Transform,
    policy: PhotometricPolicy,
    range: FitSampleRange,
    cancel: impl Fn() -> bool,
) -> Result<RangePhotometricEvidence, WarpError> {
    verify_ranged(source, target, (transform, false), policy, range, None, cancel)
}

/// Explicit display-range projection, not normalized-pixel identity. Fits only
/// unclipped opaque samples, then clamps predicted and observed straight RGB
/// in the selected range for residual comparison. Alpha is never projected.
/// Strict original bidirectional pixel evidence is retained independently.
///
/// # Errors
/// Invalid range/geometry, insufficient informative samples, budgets or cancellation.
pub fn verify_photometric_display_projection(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    transform: Transform,
    policy: PhotometricPolicy,
    range: FitSampleRange,
    cancel: impl Fn() -> bool,
) -> Result<DisplayProjectionEvidence, WarpError> {
    let projected = verify_ranged(
        source,
        target,
        (transform, false),
        policy,
        range,
        Some(range),
        &cancel,
    )?;
    let strict = verify_bidirectional(source, target, transform, policy.residual, &cancel)?;
    if cancel() {
        return Err(WarpError::Cancelled);
    }
    Ok(DisplayProjectionEvidence { projected, strict })
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayProjectionEvidence {
    pub projected: RangePhotometricEvidence,
    pub strict: BidirectionalEvidence,
}

#[allow(clippy::too_many_arguments)] // Share the same bounded ranged fitting pipeline.
fn verify_ranged(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    (transform, reflected): (Transform, bool),
    policy: PhotometricPolicy,
    range: FitSampleRange,
    projection: Option<FitSampleRange>,
    cancel: impl Fn() -> bool,
) -> Result<RangePhotometricEvidence, WarpError> {
    validate_fit_range(range)?;
    let den = transform.a * transform.a + transform.b * transform.b;
    if !den.is_finite() || den <= 0.0 {
        return Err(WarpError::Invalid);
    }
    let inverse = if reflected {
        Transform {
            a: transform.a / den,
            b: transform.b / den,
            translation: [
                (transform.a * transform.translation[0] + transform.b * transform.translation[1]) / den,
                (transform.b * transform.translation[0] - transform.a * transform.translation[1]) / den,
            ],
        }
    } else {
        Transform {
            a: transform.a / den,
            b: -transform.b / den,
            translation: [
                (-transform.a * transform.translation[0] - transform.b * transform.translation[1]) / den,
                (transform.b * transform.translation[0] - transform.a * transform.translation[1]) / den,
            ],
        }
    };
    let grid = GridPolicy {
        reflected,
        photometric: policy,
        radius: 0,
        space: FilterColorSpace::LinearSrgb,
        fit: PhotometricFitMode::RejectOutsidePolicy,
        fit_range: Some(range),
        projection,
    };
    let forward = verify_photometric_grid(source, target, transform, grid, &cancel)?;
    let reverse = verify_photometric_grid(target, source, inverse, grid, &cancel)?;
    if cancel() {
        return Err(WarpError::Cancelled);
    }
    Ok(RangePhotometricEvidence {
        range,
        evidence: BidirectionalPhotometricEvidence { forward, reverse },
    })
}

#[derive(Clone, Copy)]
struct GridPolicy {
    reflected: bool,
    photometric: PhotometricPolicy,
    radius: u32,
    space: FilterColorSpace,
    fit: PhotometricFitMode,
    fit_range: Option<FitSampleRange>,
    projection: Option<FitSampleRange>,
}
fn verify_photometric_grid(
    source: &LinearRgbaView<'_>, target: &LinearRgbaView<'_>,
    transform: Transform, grid: GridPolicy, cancel: impl Fn() -> bool,
) -> Result<PhotometricEvidence, WarpError> {
    if [
        transform.a,
        transform.b,
        transform.translation[0],
        transform.translation[1],
    ]
    .iter()
    .any(|v| !v.is_finite())
        || transform.a.hypot(transform.b) == 0.0
    {
        return Err(WarpError::Invalid);
    }
    verify_photometric_grid_mapped(source, target,
        &|[x,y]| Some(transform.apply([if grid.reflected { -x } else { x },y])),
        grid, cancel)
}

#[allow(clippy::cast_precision_loss, clippy::too_many_lines)] // Two bounded passes share one fitted model.
fn verify_photometric_grid_mapped(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    mapping: &impl Fn([f64;2]) -> Option<[f64;2]>,
    grid: GridPolicy,
    cancel: impl Fn() -> bool,
) -> Result<PhotometricEvidence, WarpError> {
    let policy = grid.photometric;
    let radius = grid.radius;
    let space = grid.space;
    validate_photometric_policy(policy)?;
    let (width, height) = source.dimensions();
    let total = u64::from(width) * u64::from(height);
    if total > policy.residual.max_source_pixels {
        return Err(WarpError::Budget);
    }
    let mut n = 0_u64;
    let mut mean_source = [0.0; 3];
    let mut mean_target = [0.0; 3];
    let mut variance = [0.0; 3];
    let mut covariance = [0.0; 3];
    for y in 0..height {
        for x in 0..width {
            if cancel() {
                return Err(WarpError::Cancelled);
            }
            let Some((expected, actual)) = sample_pair_mapped(
                source,
                target,
                mapping,
                [x, y],
                radius,
                space,
                &cancel,
            )?
            else {
                continue;
            };
            if actual[3] < 0.999 || expected[3] < 0.999 {
                continue;
            }
            if grid.fit_range.is_some_and(|range| {
                (0..3).any(|c| {
                    let a = expected[c] / expected[3];
                    let b = actual[c] / actual[3];
                    a < range.minimum || a > range.maximum || b < range.minimum || b > range.maximum
                })
            }) {
                continue;
            }
            n += 1;
            for c in 0..3 {
                let a = expected[c] / expected[3];
                let b = actual[c] / actual[3];
                let delta_a = a - mean_source[c];
                let delta_b = b - mean_target[c];
                mean_source[c] += delta_a / n as f64;
                mean_target[c] += delta_b / n as f64;
                variance[c] += delta_a * (a - mean_source[c]);
                covariance[c] += delta_a * (b - mean_target[c]);
            }
        }
    }
    if n < policy.minimum_samples {
        return Err(PhotometricFitFailure::InsufficientSamples {
            observed: n,
            required: policy.minimum_samples,
        }
        .into());
    }
    let mut gain = [0.0; 3];
    let mut offset = [0.0; 3];
    let mut constrained_channels = [false; 3];
    for c in 0..3 {
        if variance[c] / (n as f64) < policy.minimum_variance {
            return Err(PhotometricFitFailure::LowVariance { channel: c }.into());
        }
        let stats = ChannelStats {
            source: mean_source[c],
            target: mean_target[c],
            variance: variance[c],
            covariance: covariance[c],
            samples: n as f64,
        };
        let fitted = fit_channel(stats, policy, grid.fit)
            .ok_or(PhotometricFitFailure::OutsidePolicy { channel: c })?;
        gain[c] = fitted.gain;
        offset[c] = fitted.offset;
        constrained_channels[c] = fitted.constrained;
    }
    let mut pixels = WarpEvidence {
        source_pixels: total,
        compared_pixels: 0,
        matched_pixels: 0,
        maximum_channel_error: 0.0,
        squared_error: 0.0,
    };
    for y in 0..height {
        for x in 0..width {
            if cancel() {
                return Err(WarpError::Cancelled);
            }
            let Some((mut expected, mut actual)) = sample_pair_mapped(
                source,
                target,
                mapping,
                [x, y],
                radius,
                space,
                &cancel,
            )?
            else {
                continue;
            };
            for c in 0..3 {
                expected[c] = gain[c] * expected[c] + offset[c] * expected[3];
            }
            if let Some(range) = grid.projection {
                if expected.iter().chain(&actual).any(|c| !c.is_finite()) {
                    return Err(WarpError::Invalid);
                }
                for c in 0..3 {
                    expected[c] = expected[c].clamp(range.minimum * expected[3], range.maximum * expected[3]);
                    actual[c] = actual[c].clamp(range.minimum * actual[3], range.maximum * actual[3]);
                }
            }
            let mut maximum = 0.0_f64;
            for (a, b) in actual.into_iter().zip(expected) {
                let error = (a - b).abs();
                if !error.is_finite() {
                    return Err(WarpError::Invalid);
                }
                maximum = maximum.max(error);
                pixels.squared_error += error * error;
            }
            pixels.compared_pixels += 1;
            pixels.matched_pixels += u64::from(maximum <= policy.residual.tolerance);
            pixels.maximum_channel_error = pixels.maximum_channel_error.max(maximum);
        }
    }
    if cancel() {
        return Err(WarpError::Cancelled);
    }
    Ok(PhotometricEvidence {
        gain,
        offset,
        constrained_channels,
        fitted_samples: n,
        pixels,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct BidirectionalPhotometricEvidence {
    pub forward: PhotometricEvidence,
    pub reverse: PhotometricEvidence,
}
/// Fit and verify each grid independently. Both fitted models are exposed;
/// caller overlap/residual gates must hold in both directions. Four bounded
/// grid passes in total, with no fitted-image allocation.
///
/// # Errors
/// Invalid inverse/fit, work budget, or cancellation; no partial result.
pub fn verify_photometric_bidirectional(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    transform: Transform,
    policy: PhotometricPolicy,
    cancel: impl Fn() -> bool,
) -> Result<BidirectionalPhotometricEvidence, WarpError> {
    let den = transform.a * transform.a + transform.b * transform.b;
    if !den.is_finite() || den <= 0.0 {
        return Err(WarpError::Invalid);
    }
    let inverse = Transform {
        a: transform.a / den,
        b: -transform.b / den,
        translation: [
            (-transform.a * transform.translation[0] - transform.b * transform.translation[1]) / den,
            (transform.b * transform.translation[0] - transform.a * transform.translation[1]) / den,
        ],
    };
    let forward = verify_photometric(source, target, transform, policy, &cancel)?;
    let reverse = verify_photometric(target, source, inverse, policy, cancel)?;
    Ok(BidirectionalPhotometricEvidence { forward, reverse })
}

/// Explicit low-pass visual verification policy. A box window is sampled on
/// each source grid and mapped into the target; no edge padding is invented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilterPolicy {
    /// Radius in source-grid pixels, independently applied to both grids: 1..=8.
    pub radius: u32,
    /// Aggregate source/target sample-pair upper bound, including both fit and
    /// residual passes. Each pair reads one source pixel and bilinear target.
    pub max_sample_pairs: u64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct FilteredPhotometricEvidence {
    pub filter: FilterPolicy,
    /// Residual tolerance, gains and offsets are expressed in this selected space.
    pub color_space: FilterColorSpace,
    pub fit_mode: PhotometricFitMode,
    pub evidence: BidirectionalPhotometricEvidence,
}
pub(crate) fn validate_filter_policy(policy: FilterPolicy) -> Result<(), WarpError> {
    if !(1..=8).contains(&policy.radius) {
        return Err(WarpError::Invalid);
    }
    Ok(())
}

/// Average corresponding premultiplied samples in a fixed box, fit global RGB
/// changes on opaque averaged overlap, then verify every complete window in both
/// directions. Excluded border windows reduce the reported coverage. Alpha is
/// averaged but never color-fitted. This can suppress real high-frequency edits:
/// caller must retain strict residuals and qualify visual candidate thresholds.
/// This evidence never establishes exact pixels or containment of every detail.
///
/// # Errors
/// Invalid/degenerate fitting, invalid window/inverse, resource cap, cancellation.
pub fn verify_filtered_photometric_bidirectional(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    transform: Transform,
    photometric: PhotometricPolicy,
    filter: FilterPolicy,
    cancel: impl Fn() -> bool,
) -> Result<FilteredPhotometricEvidence, WarpError> {
    verify_filtered_photometric_with_color(
        source,
        target,
        transform,
        photometric,
        ColorFilterPolicy {
            filter,
            color_space: FilterColorSpace::LinearSrgb,
        },
        cancel,
    )
}
/// Explicit choice of transfer before premultiplication, interpolation and window
/// averaging. Numeric tolerances are in the selected space and are not equivalent
/// between spaces. The original filtered API retains linear-light behavior.
///
/// # Errors
/// Same invalid fitting, resource or cancellation errors as linear filtering.
pub fn verify_filtered_photometric_with_color(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    transform: Transform,
    photometric: PhotometricPolicy,
    policy: ColorFilterPolicy,
    cancel: impl Fn() -> bool,
) -> Result<FilteredPhotometricEvidence, WarpError> {
    verify_filtered_impl(
        source,
        target,
        (transform, false),
        photometric,
        policy,
        PhotometricFitMode::RejectOutsidePolicy,
        cancel,
    )
}
/// Minimize squared RGB error inside the supplied gain/offset rectangle, then
/// verify residuals. An unconstrained optimum outside the rectangle does not
/// abort this explicitly selected mode. Each affected channel is recorded.
///
/// # Errors
/// Invalid policy/geometry, insufficient opaque support/variance, work or cancellation.
pub fn verify_filtered_photometric_constrained(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    transform: Transform,
    photometric: PhotometricPolicy,
    policy: ColorFilterPolicy,
    cancel: impl Fn() -> bool,
) -> Result<FilteredPhotometricEvidence, WarpError> {
    verify_filtered_impl(
        source,
        target,
        (transform, false),
        photometric,
        policy,
        PhotometricFitMode::ConstrainedLeastSquares,
        cancel,
    )
}
fn verify_filtered_impl(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    mapping: (Transform, bool),
    photometric: PhotometricPolicy,
    policy: ColorFilterPolicy,
    fit: PhotometricFitMode,
    cancel: impl Fn() -> bool,
) -> Result<FilteredPhotometricEvidence, WarpError> {
    let (transform, reflected) = mapping;
    let filter = policy.filter;
    let space = policy.color_space;
    validate_photometric_policy(photometric)?;
    validate_filter_policy(filter)?;
    let count = |view: &LinearRgbaView<'_>| {
        let (w, h) = view.dimensions();
        u64::from(w) * u64::from(h)
    };
    let side = u64::from(filter.radius) * 2 + 1;
    let work = count(source)
        .checked_add(count(target))
        .and_then(|n| n.checked_mul(side * side))
        .and_then(|n| n.checked_mul(2))
        .ok_or(WarpError::Budget)?;
    if count(source) > photometric.residual.max_source_pixels
        || count(target) > photometric.residual.max_source_pixels
        || work > filter.max_sample_pairs
    {
        return Err(WarpError::Budget);
    }
    let den = transform.a * transform.a + transform.b * transform.b;
    if !den.is_finite() || den <= 0. {
        return Err(WarpError::Invalid);
    }
    let inverse = if reflected {
        Transform {
            a: transform.a / den,
            b: transform.b / den,
            translation: [
                (transform.a * transform.translation[0] + transform.b * transform.translation[1]) / den,
                (transform.b * transform.translation[0] - transform.a * transform.translation[1]) / den,
            ],
        }
    } else {
        Transform {
            a: transform.a / den,
            b: -transform.b / den,
            translation: [
                (-transform.a * transform.translation[0] - transform.b * transform.translation[1]) / den,
                (transform.b * transform.translation[0] - transform.a * transform.translation[1]) / den,
            ],
        }
    };
    let grid = GridPolicy {
        reflected,
        photometric,
        radius: filter.radius,
        space,
        fit,
        fit_range: None,
        projection: None,
    };
    let forward = verify_photometric_grid(source, target, transform, grid, &cancel)?;
    let reverse = verify_photometric_grid(target, source, inverse, grid, cancel)?;
    Ok(FilteredPhotometricEvidence {
        filter,
        color_space: space,
        fit_mode: fit,
        evidence: BidirectionalPhotometricEvidence { forward, reverse },
    })
}

type SamplePair = ([f64; 4], [f64; 4]);
/// Filtered planar evidence retains strict residuals separately. Averages can
/// suppress small edits, so these are visual candidates rather than pixel identity.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectiveFilteredEvidence {
    pub filter: ColorFilterPolicy,
    pub strict: BidirectionalEvidence,
    pub filtered: BidirectionalEvidence,
}

/// Verify full box windows on both grids under inverse planar mappings.
/// Source/target sampling remains premultiplied; no color gain is fitted.
///
/// # Errors
/// Invalid model/filter, shared pixel/sample-work refusal or cancellation.
pub fn verify_projective_filtered(
    source: &LinearRgbaView<'_>, target: &LinearRgbaView<'_>,
    transform: crate::geometry::ProjectiveTransform, pixels: WarpPolicy,
    filter: ColorFilterPolicy, cancel: impl Fn() -> bool,
) -> Result<ProjectiveFilteredEvidence,WarpError> {
    validate_filter_policy(filter.filter)?;
    let (sw,sh)=source.dimensions(); let (tw,th)=target.dimensions();
    let count=(u64::from(sw)*u64::from(sh)).checked_add(u64::from(tw)*u64::from(th)).ok_or(WarpError::Budget)?;
    let side=u64::from(filter.filter.radius)*2+1;
    let work=count.checked_mul(side*side).ok_or(WarpError::Budget)?;
    if work>filter.filter.max_sample_pairs { return Err(WarpError::Budget); }
    let strict=verify_projective_bidirectional(source,target,transform,pixels,&cancel)?;
    let inverse=transform.inverse().map_err(|_|WarpError::Invalid)?;
    let forward=projective_filtered_grid(source,target,transform,pixels,filter,&cancel)?;
    let reverse=projective_filtered_grid(target,source,inverse,pixels,filter,&cancel)?;
    if cancel() { return Err(WarpError::Cancelled); }
    Ok(ProjectiveFilteredEvidence {filter,strict,filtered:BidirectionalEvidence {forward,reverse}})
}
/// Perspective evidence keeps unfitted strict/window residuals beside a bounded
/// per-channel affine color fit. Fitted evidence is a visual candidate score,
/// never pixel identity. Alpha is compared without a fitted gain or offset.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectivePhotometricEvidence {
    pub unfitted: ProjectiveFilteredEvidence,
    pub fit_mode: PhotometricFitMode,
    pub fitted: BidirectionalPhotometricEvidence,
}

/// Fit color under fixed, bidirectionally verified perspective geometry.
/// `max_sample_pairs` admits all three window passes on both grids before work:
/// unfitted verification, fit statistics, and fitted residuals. Strict residuals
/// are retained separately. No geometry refinement or implicit projection occurs.
///
/// # Errors
/// Invalid geometry/policy, insufficient or low-variance fit samples, an
/// out-of-policy fit, shared work exhaustion, or cancellation discards the result.
pub fn verify_projective_photometric_filtered(
    source: &LinearRgbaView<'_>, target: &LinearRgbaView<'_>,
    transform: crate::geometry::ProjectiveTransform,
    photometric: PhotometricPolicy, filter: ColorFilterPolicy,
    fit_mode: PhotometricFitMode, cancel: impl Fn() -> bool,
) -> Result<ProjectivePhotometricEvidence, WarpError> {
    validate_photometric_policy(photometric)?;
    validate_filter_policy(filter.filter)?;
    let (sw,sh) = source.dimensions(); let (tw,th) = target.dimensions();
    let count = (u64::from(sw)*u64::from(sh)).checked_add(u64::from(tw)*u64::from(th))
        .ok_or(WarpError::Budget)?;
    let side = u64::from(filter.filter.radius)*2+1;
    let work = count.checked_mul(side*side).and_then(|n| n.checked_mul(3))
        .ok_or(WarpError::Budget)?;
    if count > photometric.residual.max_source_pixels || work > filter.filter.max_sample_pairs {
        return Err(WarpError::Budget);
    }
    // This first pass enforces singularity and horizon guards on both rectangles.
    let unfitted = verify_projective_filtered(source,target,transform,photometric.residual,filter,&cancel)?;
    let inverse = transform.inverse().map_err(|_| WarpError::Invalid)?;
    let grid = GridPolicy {
        reflected: false, photometric, radius: filter.filter.radius,
        space: filter.color_space, fit: fit_mode, fit_range: None, projection: None,
    };
    let forward = verify_photometric_grid_mapped(source,target,&|p| transform.apply(p),grid,&cancel)?;
    let reverse = verify_photometric_grid_mapped(target,source,&|p| inverse.apply(p),grid,&cancel)?;
    if cancel() { return Err(WarpError::Cancelled); }
    Ok(ProjectivePhotometricEvidence { unfitted, fit_mode,
        fitted: BidirectionalPhotometricEvidence { forward, reverse } })
}

fn projective_filtered_grid(
    source:&LinearRgbaView<'_>,target:&LinearRgbaView<'_>,
    transform:crate::geometry::ProjectiveTransform,pixels:WarpPolicy,
    filter:ColorFilterPolicy,cancel:&impl Fn()->bool,
)->Result<WarpEvidence,WarpError>{
    let (width,height)=source.dimensions();
    let mut evidence=WarpEvidence{source_pixels:u64::from(width)*u64::from(height),compared_pixels:0,matched_pixels:0,maximum_channel_error:0.0,squared_error:0.0};
    for y in 0..height {for x in 0..width {
        if cancel(){return Err(WarpError::Cancelled);}
        let Some((expected,actual))=sample_pair_mapped(source,target,&|p|transform.apply(p),[x,y],filter.filter.radius,filter.color_space,cancel)? else {continue;};
        let mut maximum=0.0_f64;
        for (a,b) in expected.into_iter().zip(actual){let error=(a-b).abs();maximum=maximum.max(error);evidence.squared_error+=error*error;}
        evidence.compared_pixels+=1;evidence.matched_pixels+=u64::from(maximum<=pixels.tolerance);evidence.maximum_channel_error=evidence.maximum_channel_error.max(maximum);
    }}
    if cancel(){return Err(WarpError::Cancelled);}
    Ok(evidence)
}

fn sample_pair_mapped(
    source: &LinearRgbaView<'_>, target: &LinearRgbaView<'_>,
    mapping: &impl Fn([f64;2]) -> Option<[f64;2]>, point: [u32;2],
    radius: u32, space: FilterColorSpace, cancel: &impl Fn() -> bool,
) -> Result<Option<SamplePair>,WarpError> {
    let (width, height) = source.dimensions();
    let [x, y] = point;
    if x < radius
        || y < radius
        || x.checked_add(radius).is_none_or(|v| v >= width)
        || y.checked_add(radius).is_none_or(|v| v >= height)
    {
        return Ok(None);
    }
    let side = radius * 2 + 1;
    let mut expected = [0.; 4];
    let mut actual = [0.; 4];
    for dy in 0..side {
        for dx in 0..side {
            if cancel() {
                return Err(WarpError::Cancelled);
            }
            let sx = x - radius + dx;
            let sy = y - radius + dy;
            let p = mapping([f64::from(sx),f64::from(sy)]).ok_or(WarpError::Invalid)?;
            if p.iter().any(|v| !v.is_finite()) {
                return Err(WarpError::Invalid);
            }
            let Some(sample) = interpolate_in_space(target, p, space) else {
                return Ok(None);
            };
            let original = premultiplied_in_space(source, sx, sy, space).ok_or(WarpError::Invalid)?;
            for c in 0..4 {
                expected[c] += original[c];
                actual[c] += sample[c];
            }
        }
    }
    let weight = f64::from(side * side);
    for c in 0..4 {
        expected[c] /= weight;
        actual[c] /= weight;
    }
    Ok(Some((expected, actual)))
}

/// Signal space used only for explicitly requested filtered visual comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterColorSpace {
    LinearSrgb,
    /// Extended sRGB transfer, sign-preserving for negative values and without
    /// clipping HDR values. Not a tone map or a perceptually uniform distance.
    EncodedSrgb,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorFilterPolicy {
    pub filter: FilterPolicy,
    pub color_space: FilterColorSpace,
}
impl From<FilterPolicy> for ColorFilterPolicy {
    fn from(filter: FilterPolicy) -> Self {
        Self {
            filter,
            color_space: FilterColorSpace::LinearSrgb,
        }
    }
}
// Independently expressed extended sRGB transfer; reference formula:
// https://www.w3.org/TR/css-color-4/#color-conversion-code
fn encode_srgb(linear: f64) -> f64 {
    if linear.abs() <= 0.003_130_8 {
        return linear * 12.92;
    }
    (1.055 * linear.abs().powf(1. / 2.4) - 0.055).copysign(linear)
}

#[cfg(test)]
mod encoded_signal_tests {
    use super::{FilterColorSpace, encode_srgb, interpolate_in_space, premultiplied_in_space};
    use crate::linear::LinearRgbaView;
    #[test]
    fn extended_srgb_landmarks_match_independent_decimal_oracle_without_clipping() {
        for line in include_str!("../tests/fixtures/encoded-srgb/transfer.tsv").lines() {
            let mut values = line.split_whitespace().map(|s| s.parse::<f64>().unwrap());
            let input = values.next().unwrap();
            let expected = values.next().unwrap();
            assert!((encode_srgb(input) - expected).abs() < 1e-12);
        }
    }
    #[test]
    fn transfer_precedes_premultiplication_and_bilinear_sampling() {
        let rgba = [0.18, 0.18, 0.18, 0.25];
        let image = LinearRgbaView::new(1, 1, &rgba, 1, || false).unwrap();
        let p = premultiplied_in_space(&image, 0, 0, FilterColorSpace::EncodedSrgb).unwrap();
        assert!((p[0] - 0.461_356_129_500_441_6 * 0.25).abs() < 1e-7);
        assert!((p[3] - 0.25).abs() < 1e-12);
        let rgba = [0., 0., 0., 1., 1., 1., 1., 1.];
        let image = LinearRgbaView::new(2, 1, &rgba, 2, || false).unwrap();
        let p = interpolate_in_space(&image, [0.5, 0.], FilterColorSpace::EncodedSrgb).unwrap();
        assert!((p[0] - 0.5).abs() < 1e-12);
        let rgba = [f32::NAN, 999., -999., 0.];
        let image = LinearRgbaView::new(1, 1, &rgba, 1, || false).unwrap();
        let p = premultiplied_in_space(&image, 0, 0, FilterColorSpace::EncodedSrgb).unwrap();
        assert!(p.iter().all(|v| v.abs() < 1e-12));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotometricFitMode {
    /// Existing APIs require their unconstrained optimum to be within bounds.
    RejectOutsidePolicy,
    /// Select the least-squares optimum inside the declared parameter rectangle.
    ConstrainedLeastSquares,
}
#[derive(Clone, Copy)]
struct ChannelStats {
    source: f64,
    target: f64,
    variance: f64,
    covariance: f64,
    samples: f64,
}
struct ChannelFit {
    gain: f64,
    offset: f64,
    constrained: bool,
}
fn fit_channel(s: ChannelStats, p: PhotometricPolicy, mode: PhotometricFitMode) -> Option<ChannelFit> {
    let unconstrained = s.covariance / s.variance;
    let intercept = s.target - unconstrained * s.source;
    if !unconstrained.is_finite() || !intercept.is_finite() {
        return None;
    }
    if (p.minimum_gain..=p.maximum_gain).contains(&unconstrained) && intercept.abs() <= p.maximum_offset {
        return Some(ChannelFit {
            gain: unconstrained,
            offset: intercept,
            constrained: false,
        });
    }
    if mode == PhotometricFitMode::RejectOutsidePolicy {
        return None;
    }
    // A strictly convex quadratic on a rectangle has an interior optimum or an
    // optimum on one of four edges. Each edge is minimized analytically; simple
    // independent clipping of the two unconstrained coefficients is not optimal.
    let mut best = None;
    let mut cost = f64::INFINITY;
    for (a, b) in [
        (
            p.minimum_gain,
            (s.target - p.minimum_gain * s.source).clamp(-p.maximum_offset, p.maximum_offset),
        ),
        (
            p.maximum_gain,
            (s.target - p.maximum_gain * s.source).clamp(-p.maximum_offset, p.maximum_offset),
        ),
        (offset_edge_gain(s, -p.maximum_offset, p), -p.maximum_offset),
        (offset_edge_gain(s, p.maximum_offset, p), p.maximum_offset),
    ] {
        let objective =
            s.variance * (a - unconstrained).powi(2) + s.samples * (s.target - a * s.source - b).powi(2);
        if a.is_finite() && b.is_finite() && objective.is_finite() && objective < cost {
            cost = objective;
            best = Some(ChannelFit {
                gain: a,
                offset: b,
                constrained: true,
            });
        }
    }
    best
}
fn offset_edge_gain(s: ChannelStats, b: f64, p: PhotometricPolicy) -> f64 {
    ((s.covariance + s.samples * s.source * (s.target - b)) / (s.variance + s.samples * s.source * s.source))
        .clamp(p.minimum_gain, p.maximum_gain)
}

#[cfg(test)]
mod bounded_fit_tests {
    use super::*;

    #[test]
    fn boundary_fit_matches_exhaustive_rectangle_and_beats_independent_clipping() {
        let policy = PhotometricPolicy {
            minimum_gain: 0.2,
            maximum_gain: 5.0,
            maximum_offset: 0.1,
            residual: WarpPolicy {
                tolerance: 0.03,
                max_source_pixels: 1000,
            },
            minimum_samples: 8,
            minimum_variance: 1e-5,
        };
        for (mean_x, mean_y, variance, covariance) in [
            (0.5, 0.8, 0.2, 0.08),
            (0.7, 0.1, 0.4, -0.2),
            (0.4, 3.0, 0.3, 2.4),
            (-0.8, 0.6, 0.7, 0.5),
        ] {
            let s = ChannelStats {
                source: mean_x,
                target: mean_y,
                variance,
                covariance,
                samples: 10.0,
            };
            let fit = fit_channel(s, policy, PhotometricFitMode::ConstrainedLeastSquares).unwrap();
            assert!(fit.constrained);
            assert!(fit_channel(s, policy, PhotometricFitMode::RejectOutsidePolicy).is_none());
            // Independent objective from raw second moments, evaluated over a grid.
            let cost = |a: f64, b: f64| {
                a * a * (variance + 10.0 * mean_x * mean_x) - 2.0 * a * (covariance + 10.0 * mean_x * mean_y)
                    + 20.0 * a * b * mean_x
                    - 20.0 * b * mean_y
                    + 10.0 * b * b
            };
            let optimum = cost(fit.gain, fit.offset);
            for gain_step in 0..=480 {
                for offset_step in 0..=200 {
                    let a = 0.2 + f64::from(gain_step) * 0.01;
                    let b = -0.1 + f64::from(offset_step) * 0.001;
                    assert!(optimum <= cost(a, b) + 1e-10);
                }
            }
            if (mean_x - 0.5).abs() < 1e-12 {
                let clipped = cost(0.4, 0.1);
                assert!(optimum < clipped - 0.1);
                assert!((fit.offset - 0.1).abs() < 1e-12);
                assert!((fit.gain - (3.58 / 2.7)).abs() < 1e-12);
            }
        }
    }
}

/// Verify both original pixel grids under an orientation-reversing similarity.
/// Uses the exact reflected inverse, without allocating mirrored image buffers.
/// Each grid is bounded independently by `max_source_pixels`.
///
/// # Errors
/// Invalid inverse geometry, work excess or cancellation without partial evidence.
pub fn verify_reflected_bidirectional(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    transform: Transform,
    policy: WarpPolicy,
    cancel: impl Fn() -> bool,
) -> Result<BidirectionalEvidence, WarpError> {
    let den = transform.a * transform.a + transform.b * transform.b;
    if !den.is_finite() || den <= 0.0 {
        return Err(WarpError::Invalid);
    }
    let inverse = Transform {
        a: transform.a / den,
        b: transform.b / den,
        translation: [
            (transform.a * transform.translation[0] + transform.b * transform.translation[1]) / den,
            (transform.b * transform.translation[0] - transform.a * transform.translation[1]) / den,
        ],
    };
    let forward = verify_reflected_pixels(source, target, transform, policy, &cancel)?;
    let reverse = verify_reflected_pixels(target, source, inverse, policy, cancel)?;
    Ok(BidirectionalEvidence { forward, reverse })
}

/// Fit per-channel gain/offset and verify both reflected original pixel grids.
/// Alpha is not fitted. Strict residuals must be retained separately by callers.
/// Fitting uses the existing bounded linear-sRGB policy without display clipping.
///
/// # Errors
/// Invalid geometry/policy, insufficient/degenerate fit, resource excess or
/// cancellation without partial evidence from either direction.
pub fn verify_reflected_photometric_bidirectional(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    transform: Transform,
    policy: PhotometricPolicy,
    cancel: impl Fn() -> bool,
) -> Result<BidirectionalPhotometricEvidence, WarpError> {
    let den = transform.a * transform.a + transform.b * transform.b;
    if !den.is_finite() || den <= 0.0 {
        return Err(WarpError::Invalid);
    }
    let inverse = Transform {
        a: transform.a / den,
        b: transform.b / den,
        translation: [
            (transform.a * transform.translation[0] + transform.b * transform.translation[1]) / den,
            (transform.b * transform.translation[0] - transform.a * transform.translation[1]) / den,
        ],
    };
    let grid = GridPolicy {
        reflected: true,
        photometric: policy,
        radius: 0,
        space: FilterColorSpace::LinearSrgb,
        fit: PhotometricFitMode::RejectOutsidePolicy,
        fit_range: None,
        projection: None,
    };
    let forward = verify_photometric_grid(source, target, transform, grid, &cancel)?;
    let reverse = verify_photometric_grid(target, source, inverse, grid, &cancel)?;
    if cancel() {
        return Err(WarpError::Cancelled);
    }
    Ok(BidirectionalPhotometricEvidence { forward, reverse })
}

/// Filtered reflected visual evidence in an explicitly selected color space.
/// Strict pixel and unfiltered fitted evidence remain separate caller obligations.
/// Window sampling uses original pixel grids, premultiplied alpha and reflected
/// geometry; no mirrored image or implicit clipping is introduced.
///
/// # Errors
/// Invalid policy/fit/geometry, work exhaustion or cancellation without evidence.
pub fn verify_reflected_filtered_photometric(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    transform: Transform,
    photometric: PhotometricPolicy,
    policy: ColorFilterPolicy,
    cancel: impl Fn() -> bool,
) -> Result<FilteredPhotometricEvidence, WarpError> {
    verify_filtered_impl(
        source,
        target,
        (transform, true),
        photometric,
        policy,
        PhotometricFitMode::RejectOutsidePolicy,
        cancel,
    )
}

/// Reflected filtered evidence with explicitly constrained least-squares fitting.
///
/// # Errors
/// Invalid policy/fit/geometry, work exhaustion or cancellation without evidence.
pub fn verify_reflected_filtered_photometric_constrained(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    transform: Transform,
    photometric: PhotometricPolicy,
    policy: ColorFilterPolicy,
    cancel: impl Fn() -> bool,
) -> Result<FilteredPhotometricEvidence, WarpError> {
    verify_filtered_impl(
        source,
        target,
        (transform, true),
        photometric,
        policy,
        PhotometricFitMode::ConstrainedLeastSquares,
        cancel,
    )
}

/// Fit reflected original pixels on an explicit range, retaining all overlap residuals.
/// Alpha is never fitted and no display projection is applied.
///
/// # Errors
/// Invalid policy/range/geometry, insufficient fit, work excess or cancellation.
pub fn verify_reflected_photometric_bidirectional_in_range(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    transform: Transform,
    policy: PhotometricPolicy,
    range: FitSampleRange,
    cancel: impl Fn() -> bool,
) -> Result<RangePhotometricEvidence, WarpError> {
    verify_ranged(source, target, (transform, true), policy, range, None, cancel)
}

/// Explicit reflected display projection retaining strict original pixel evidence.
/// Only informative unclipped samples fit RGB; alpha is never projected.
///
/// # Errors
/// Invalid policy/range/geometry, insufficient fit, work excess or cancellation.
pub fn verify_reflected_photometric_display_projection(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    transform: Transform,
    policy: PhotometricPolicy,
    range: FitSampleRange,
    cancel: impl Fn() -> bool,
) -> Result<DisplayProjectionEvidence, WarpError> {
    let projected = verify_ranged(
        source,
        target,
        (transform, true),
        policy,
        range,
        Some(range),
        &cancel,
    )?;
    let strict = verify_reflected_bidirectional(source, target, transform, policy.residual, &cancel)?;
    if cancel() {
        return Err(WarpError::Cancelled);
    }
    Ok(DisplayProjectionEvidence { projected, strict })
}

/// Explicit bound for pixel registration. Final evidence must still be verified
/// on both complete pixel grids; this sampled objective cannot establish equality.
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveRegistrationPolicy {
    /// Box radius for the registration objective, 0..=8; zero samples a single pixel.
    pub radius: u32,
    pub stride: u32,
    pub rounds: u32,
    pub max_sample_pairs: u64,
}
pub(crate) fn validate_registration_policy(policy: ProjectiveRegistrationPolicy) -> Result<(),WarpError> {
    if policy.radius > 8 || policy.stride == 0 || policy.rounds == 0 || policy.rounds > 256 {return Err(WarpError::Invalid);}
    Ok(())
}
/// Refine an initial planar model against a fixed target-grid sample domain.
/// Trials cannot improve their score by dropping initially admitted pixels.
/// No fitted color correction or image-dependent tolerance is introduced.
///
/// # Errors
/// Invalid model/policy, insufficient overlap, cancellation or exhausted work.
pub fn refine_projective_pixels(
    source: &LinearRgbaView<'_>, target: &LinearRgbaView<'_>,
    initial: crate::geometry::ProjectiveTransform,
    policy: ProjectiveRegistrationPolicy, cancel: impl Fn() -> bool,
) -> Result<crate::geometry::ProjectiveTransform, WarpError> {
    refine_projective_pixels_selected(source,target,initial,policy,None,None,cancel)
}

/// Refine geometry with bounded encoded-sRGB gains/offsets as nuisance parameters.
/// Alpha remains absolute. The fitted objective never establishes a candidate;
/// callers must perform independent final residual verification.
///
/// # Errors
/// Invalid policies/model, insufficient informative samples, work or cancellation.
pub fn refine_projective_pixels_photometric(
    source:&LinearRgbaView<'_>,target:&LinearRgbaView<'_>,
    initial:crate::geometry::ProjectiveTransform,policy:ProjectiveRegistrationPolicy,
    photometric:PhotometricPolicy,cancel:impl Fn()->bool,
)->Result<crate::geometry::ProjectiveTransform,WarpError>{
    validate_photometric_policy(photometric)?;
    refine_projective_pixels_selected(source,target,initial,policy,Some(photometric),None,cancel)
}

#[allow(clippy::cast_precision_loss)] // Admitted fixed-grid sample count.
fn refine_projective_pixels_selected(
    source:&LinearRgbaView<'_>,target:&LinearRgbaView<'_>,
    initial:crate::geometry::ProjectiveTransform,policy:ProjectiveRegistrationPolicy,
    photometric:Option<PhotometricPolicy>,maximum_corner_shift:Option<f64>,cancel:impl Fn()->bool,
)->Result<crate::geometry::ProjectiveTransform,WarpError>{
    validate_registration_policy(policy)?;
    let inverse=initial.inverse().map_err(|_|WarpError::Invalid)?;
    let mut matrix=inverse.matrix;
    let scale=matrix[2][2];
    if scale==0.0 || !scale.is_finite() {return Err(WarpError::Invalid);}
    for row in &mut matrix {for value in row {*value/=scale;}}
    let (sw,sh)=source.dimensions();let (tw,th)=target.dimensions();
    let corners=[[0.,0.],[f64::from(tw-1),0.],[0.,f64::from(th-1)],[f64::from(tw-1),f64::from(th-1)]];
    let mut controls=corners.map(|source|crate::geometry::Correspondence{source,target:[0.,0.]});
    for control in &mut controls {control.target=inverse.apply(control.source).ok_or(WarpError::Invalid)?;}
    let original_controls=controls;
    let mut step=1.0;
    let work=std::cell::Cell::new(0_u64);
    let score=|trial: [[f64;3];3]| -> Result<f64,WarpError> {
        let model=crate::geometry::ProjectiveTransform{matrix:trial};
        let mut sum=0.;let mut count=0_u64;
        let mut n=0_u64;let mut mean_a=[0.;3];let mut mean_b=[0.;3];
        let mut variance_a=[0.;3];let mut variance_b=[0.;3];let mut covariance=[0.;3];let mut alpha_error=0.;
        for y in (0..th).step_by(policy.stride as usize) {for x in (0..tw).step_by(policy.stride as usize) {
            if cancel(){return Err(WarpError::Cancelled);}
            let next=work.get().checked_add(1).ok_or(WarpError::Budget)?;
            if next>policy.max_sample_pairs {return Err(WarpError::Budget);}work.set(next);
            let point=[f64::from(x),f64::from(y)];
            let Some(original)=inverse.apply(point) else {return Err(WarpError::Invalid);};
            if original[0]<8. || original[1]<8. || original[0]>f64::from(sw)-9. || original[1]>f64::from(sh)-9. {continue;}
            let (expected,actual)=if policy.radius==0 {
                let Some(mapped)=model.apply(point) else {return Ok(f64::INFINITY);};
                let Some(actual)=interpolate_in_space(source,mapped,FilterColorSpace::EncodedSrgb) else {return Ok(f64::INFINITY);};
                (premultiplied_in_space(target,x,y,FilterColorSpace::EncodedSrgb).ok_or(WarpError::Invalid)?,actual)
            } else {
                let side=u64::from(policy.radius)*2+1;
                let next=work.get().checked_add(2*side*side).ok_or(WarpError::Budget)?;
                if next>policy.max_sample_pairs {return Err(WarpError::Budget);}work.set(next);
                // Admission depends only on the initial model. Dropped trial
                // windows fail the trial instead of shrinking its objective.
                if sample_pair_mapped(target,source,&|p|inverse.apply(p),[x,y],policy.radius,FilterColorSpace::EncodedSrgb,&cancel)?.is_none(){continue;}
                let Some(pair)=sample_pair_mapped(target,source,&|p|model.apply(p),[x,y],policy.radius,FilterColorSpace::EncodedSrgb,&cancel)? else {return Ok(f64::INFINITY);};
                pair
            };
            for (a,b) in actual.into_iter().zip(expected){sum+=(a-b)*(a-b);}
            if photometric.is_some(){
                alpha_error+=(actual[3]-expected[3]).powi(2);
                // Restrict nuisance fitting to opaque samples. Other RGB samples
                // retain absolute error and cannot disappear from the objective.
                if actual[3]>=0.999 && expected[3]>=0.999 {
                    n+=1;
                    for c in 0..3 {
                        let da=actual[c]-mean_a[c];let db=expected[c]-mean_b[c];
                        mean_a[c]+=da/n as f64;mean_b[c]+=db/n as f64;
                        variance_a[c]+=da*(actual[c]-mean_a[c]);variance_b[c]+=db*(expected[c]-mean_b[c]);covariance[c]+=da*(expected[c]-mean_b[c]);
                    }
                }else{for c in 0..3 {alpha_error+=(actual[c]-expected[c]).powi(2);}}
            }
            count+=1;
        }}
        if count<16{return Err(WarpError::Invalid);}
        if let Some(fit)=photometric {
            if n<fit.minimum_samples {return Ok(f64::INFINITY);}
            sum=alpha_error;
            for c in 0..3 {
                if variance_a[c]/(n as f64)<fit.minimum_variance {return Ok(f64::INFINITY);}
                let stats=ChannelStats{source:mean_a[c],target:mean_b[c],variance:variance_a[c],covariance:covariance[c],samples:n as f64};
                let Some(model)=fit_channel(stats,fit,PhotometricFitMode::ConstrainedLeastSquares) else {return Ok(f64::INFINITY);};
                let residual=variance_b[c]+model.gain*model.gain*variance_a[c]-2.*model.gain*covariance[c]+n as f64*(mean_b[c]-model.gain*mean_a[c]-model.offset).powi(2);
                sum+=residual.max(0.);
            }
        }
        if !sum.is_finite(){return Ok(f64::INFINITY);}Ok(sum)
    };
    let mut best=score(matrix)?;
    if !best.is_finite(){return Err(WarpError::Invalid);}
    for _ in 0..policy.rounds {
        let mut improved=false;
        for corner in 0..4 {for axis in 0..2 {
            for sign in [-1.,1.] {
                let mut trial_controls=controls;trial_controls[corner].target[axis]+=sign*step;
                if maximum_corner_shift.is_some_and(|limit|(trial_controls[corner].target[0]-original_controls[corner].target[0]).hypot(trial_controls[corner].target[1]-original_controls[corner].target[1])>limit){continue;}
                let trial=match crate::geometry::fit_projective_four(&trial_controls,&cancel) {
                    Ok(model)=>model.matrix,
                    Err(crate::geometry::GeometryError::Cancelled)=>return Err(WarpError::Cancelled),
                    Err(crate::geometry::GeometryError::Invalid)=>continue,
                    Err(crate::geometry::GeometryError::Budget)=>return Err(WarpError::Budget),
                };
                let error=score(trial)?;
                if error<best {matrix=trial;controls=trial_controls;best=error;improved=true;}
            }
        }}
        if !improved {step*=0.5;}
    }
    if cancel(){return Err(WarpError::Cancelled);}
    crate::geometry::ProjectiveTransform{matrix}.inverse().map_err(|_|WarpError::Invalid)
}


/// Caller-declared trust region around the initial inverse corner coordinates.
#[derive(Debug,Clone,Copy)]
pub struct ProjectiveRegistrationTrustPolicy {
    pub registration:ProjectiveRegistrationPolicy,
    pub photometric:PhotometricPolicy,
    /// Maximum Euclidean displacement of each target corner mapped into source
    /// coordinates. This is not a continuous image-wide displacement certificate.
    pub maximum_corner_shift:f64,
}

/// Bounded photometric registration anchored to the caller's initial geometry.
/// Zero shift freezes controls while still validating sample/work/cancel limits.
/// Final pixel confirmation remains mandatory and separate.
///
/// # Errors
/// Invalid policies/model, insufficient samples, work refusal or cancellation.
pub fn refine_projective_pixels_anchored(
    source:&LinearRgbaView<'_>,target:&LinearRgbaView<'_>,
    initial:crate::geometry::ProjectiveTransform,policy:ProjectiveRegistrationTrustPolicy,
    cancel:impl Fn()->bool,
)->Result<crate::geometry::ProjectiveTransform,WarpError>{
    validate_registration_trust_policy(policy)?;
    refine_projective_pixels_selected(source,target,initial,policy.registration,Some(policy.photometric),Some(policy.maximum_corner_shift),cancel)
}

pub(crate) fn validate_registration_trust_policy(policy:ProjectiveRegistrationTrustPolicy)->Result<(),WarpError>{
    if !policy.maximum_corner_shift.is_finite() || policy.maximum_corner_shift<0. {return Err(WarpError::Invalid);}
    validate_photometric_policy(policy.photometric)?;
    validate_registration_policy(policy.registration)
}

/// Independent registration hypotheses with pre-admitted cumulative work caps.
/// Neither lane may silently consume the other's reserved work allowance.
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveRegistrationPortfolioPolicy {
    pub anchored: ProjectiveRegistrationTrustPolicy,
    pub unanchored: ProjectiveRegistrationPolicy,
    pub max_sample_pairs: u64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectiveRegistrationCandidates {
    pub anchored: crate::geometry::ProjectiveTransform,
    pub unanchored: crate::geometry::ProjectiveTransform,
}
/// Retain both bounded-color anchored and absolute-color unanchored hypotheses.
/// The sum of lane work caps must fit the portfolio cap before either lane runs.
/// Final full-grid verification is still required separately for each model.
/// Failure or cancellation in either required lane discards the entire result.
///
/// # Errors
/// Invalid policies/model, cumulative or lane work exhaustion, cancellation or
/// insufficient informative registration samples.
pub fn refine_projective_pixels_candidates(
    source: &LinearRgbaView<'_>, target: &LinearRgbaView<'_>,
    initial: crate::geometry::ProjectiveTransform,
    policy: ProjectiveRegistrationPortfolioPolicy, cancel: impl Fn()->bool,
) -> Result<ProjectiveRegistrationCandidates,WarpError> {
    validate_registration_portfolio_policy(policy)?;
    let latched=std::cell::Cell::new(false);
    let cancelled=||{let value=latched.get() || cancel();latched.set(value);value};
    let anchored=refine_projective_pixels_anchored(source,target,initial,policy.anchored,cancelled)?;
    let unanchored=refine_projective_pixels(source,target,initial,policy.unanchored,cancelled)?;
    if cancelled(){return Err(WarpError::Cancelled);}
    Ok(ProjectiveRegistrationCandidates{anchored,unanchored})
}

/// Separate complete-grid residuals for both registration hypotheses.
/// No model is selected by descriptor count or sampled registration score.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectiveCandidatePixelEvidence {
    pub models: ProjectiveRegistrationCandidates,
    pub anchored: ProjectiveFilteredEvidence,
    pub unanchored: ProjectiveFilteredEvidence,
}
/// Verify both candidate mappings with unchanged strict/window tolerances.
/// Pixel and window-work budgets admit the sum of both complete bidirectional
/// verifications before either pass. No color fit grants candidate acceptance.
///
/// # Errors
/// Invalid model/policy, cumulative work refusal, or cancellation discards all
/// evidence, including a successfully verified first hypothesis.
pub fn verify_projective_candidates_filtered(
    source: &LinearRgbaView<'_>, target: &LinearRgbaView<'_>,
    models: ProjectiveRegistrationCandidates, pixels: WarpPolicy,
    filter: ColorFilterPolicy, cancel: impl Fn()->bool,
) -> Result<ProjectiveCandidatePixelEvidence,WarpError> {
    validate_filter_policy(filter.filter)?;
    if !pixels.tolerance.is_finite() || pixels.tolerance<0.0 {return Err(WarpError::Invalid);}
    let (sw,sh)=source.dimensions();let (tw,th)=target.dimensions();
    let visits=(u64::from(sw)*u64::from(sh)).checked_add(u64::from(tw)*u64::from(th))
        .and_then(|v|v.checked_mul(2)).ok_or(WarpError::Budget)?;
    let side=u64::from(filter.filter.radius)*2+1;
    let work=visits.checked_mul(side*side).ok_or(WarpError::Budget)?;
    if visits>pixels.max_source_pixels || work>filter.filter.max_sample_pairs {return Err(WarpError::Budget);}
    let latched=std::cell::Cell::new(false);
    let cancelled=||{let value=latched.get() || cancel();latched.set(value);value};
    let anchored=verify_projective_filtered(source,target,models.anchored,pixels,filter,cancelled)?;
    let unanchored=verify_projective_filtered(source,target,models.unanchored,pixels,filter,cancelled)?;
    if cancelled(){return Err(WarpError::Cancelled);}
    Ok(ProjectiveCandidatePixelEvidence{models,anchored,unanchored})
}

pub(crate) fn validate_registration_portfolio_policy(policy: ProjectiveRegistrationPortfolioPolicy) -> Result<(),WarpError> {
    validate_registration_trust_policy(policy.anchored)?;
    validate_registration_policy(policy.unanchored)?;
    let admitted=policy.anchored.registration.max_sample_pairs
        .checked_add(policy.unanchored.max_sample_pairs).ok_or(WarpError::Budget)?;
    if admitted>policy.max_sample_pairs {return Err(WarpError::Budget);}
    Ok(())
}
