//! Explicit exposure measurement for ordinal experiments, not copy evidence.
use crate::linear::LinearRgbaView;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RankExposure {
    /// Maximum original linear RGB component; no clipping or tone mapping.
    pub peak_rgb: f64,
    /// Positive uniform RGB gain that maps the peak to one.
    pub gain: f64,
    pub pixel_reads: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ExposureError {
    #[error("exposure destination has the wrong sample count")]
    Layout,
    #[error("exposure scan budget exceeded")]
    Budget,
    #[error("exposure scan cancelled")]
    Cancelled,
    #[error("exposure measurement requires opaque nonnegative finite RGB")]
    UnsupportedPixels,
    #[error("zero exposure has no positive calibration")]
    Uninformative,
}

/// Explicitly calibrate into caller-owned scratch; original samples are never
/// modified. Three complete image passes (measurement, write and validation) are prepaid. Each RGB component receives
/// the same positive gain, without clipping or channel-dependent adjustment.
/// Float conversion can round. On error scratch may be partially written and
/// must not be consumed as calibrated evidence. No view is returned on error.
pub fn calibrate_rank_exposure_into<'a>(
    image: &LinearRgbaView<'_>,
    scratch: &'a mut [f32],
    maximum_pixel_reads: u64,
    cancel: impl Fn() -> bool,
) -> Result<(LinearRgbaView<'a>, RankExposure), ExposureError> {
    if cancel() {
        return Err(ExposureError::Cancelled);
    }
    let (width, height) = image.dimensions();
    let pixels = u64::from(width) * u64::from(height);
    let samples = pixels
        .checked_mul(4)
        .and_then(|v| usize::try_from(v).ok())
        .ok_or(ExposureError::Layout)?;
    if scratch.len() != samples {
        return Err(ExposureError::Layout);
    }
    let reads = pixels.checked_mul(3).ok_or(ExposureError::Budget)?;
    if reads > maximum_pixel_reads {
        return Err(ExposureError::Budget);
    }
    let mut exposure = measure_rank_exposure(image, pixels, &cancel)?;
    for y in 0..height {
        for x in 0..width {
            if cancel() {
                return Err(ExposureError::Cancelled);
            }
            let p = image.rgba(x, y).ok_or(ExposureError::UnsupportedPixels)?;
            let offset = ((u64::from(y) * u64::from(width) + u64::from(x)) * 4) as usize;
            for channel in 0..3 {
                scratch[offset + channel] = (f64::from(p[channel]) * exposure.gain) as f32;
            }
            scratch[offset + 3] = 1.0;
        }
    }
    let view = LinearRgbaView::new(width, height, scratch, pixels, &cancel).map_err(|e| match e {
        crate::pixels::PixelError::Cancelled => ExposureError::Cancelled,
        _ => ExposureError::UnsupportedPixels,
    })?;
    exposure.pixel_reads = reads;
    Ok((view, exposure))
}

/// Scan the entire original image with constant storage. Work is admitted
/// before the first pixel read. Alpha and negative RGB are refused explicitly;
/// positive HDR is supported. The result supplies no texture/information or
/// duplicate verdict. Callers must retain source guards and separately qualify
/// any use of this gain. Existing rank policies are unaffected.
pub fn measure_rank_exposure(
    image: &LinearRgbaView<'_>,
    maximum_pixel_reads: u64,
    cancel: impl Fn() -> bool,
) -> Result<RankExposure, ExposureError> {
    if cancel() {
        return Err(ExposureError::Cancelled);
    }
    let (width, height) = image.dimensions();
    let reads = u64::from(width) * u64::from(height);
    if reads > maximum_pixel_reads {
        return Err(ExposureError::Budget);
    }
    let mut peak = 0.0_f64;
    for y in 0..height {
        for x in 0..width {
            if cancel() {
                return Err(ExposureError::Cancelled);
            }
            let p = image.rgba(x, y).ok_or(ExposureError::UnsupportedPixels)?;
            if p[3] != 1.0 || p[..3].iter().any(|v| !v.is_finite() || *v < 0.0) {
                return Err(ExposureError::UnsupportedPixels);
            }
            for value in &p[..3] {
                peak = peak.max(f64::from(*value));
            }
        }
    }
    if cancel() {
        return Err(ExposureError::Cancelled);
    }
    if peak == 0.0 {
        return Err(ExposureError::Uninformative);
    }
    Ok(RankExposure {
        peak_rgb: peak,
        gain: peak.recip(),
        pixel_reads: reads,
    })
}
