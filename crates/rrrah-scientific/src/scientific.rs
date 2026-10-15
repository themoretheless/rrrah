//! Explicit scalar-array display interpretation shared by scientific formats.
use crate::{DecodeRequest, bounded_io::read_bounded, raster::RasterDecodeError};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
pub(crate) fn windowed(
    raster: &DecodedRaster,
    minimum: f32,
    maximum: f32,
    bad: fn(&'static str) -> RasterDecodeError,
) -> Result<DecodedRaster, RasterDecodeError> {
    windowed_with_request(raster, minimum, maximum, bad, None)
}
fn windowed_with_request(
    raster: &DecodedRaster,
    minimum: f32,
    maximum: f32,
    bad: fn(&'static str) -> RasterDecodeError,
    request: Option<&DecodeRequest>,
) -> Result<DecodedRaster, RasterDecodeError> {
    let range = maximum - minimum;
    if !minimum.is_finite() || !maximum.is_finite() || !range.is_finite() || range <= 0.0 {
        return Err(bad("window must have finite increasing bounds"));
    }
    let RasterPixels::Rgba32Float(source) = raster.pixels() else {
        return Err(bad("unexpected sample layout"));
    };
    let mut out = Vec::with_capacity(source.len());
    for (index, pixel) in source.chunks_exact(4).enumerate() {
        if index % 4096 == 0 {
            if let Some(request) = request {
                request.check_cancelled()?;
            }
        }
        if !pixel[0].is_finite() {
            return Err(bad("nonfinite sample requires an explicit missing-value policy"));
        }
        let value = ((pixel[0] - minimum) / range).clamp(0.0, 1.0);
        out.extend([value, value, value, 1.0]);
    }
    Ok(DecodedRaster::new(
        raster.width(),
        raster.height(),
        RasterPixels::Rgba32Float(Arc::new(out).into()),
        RasterColorSpace::LinearSrgb,
    )?
    .with_image_selection(raster.image_index(), raster.image_count())?)
}

/// Explicit scalar display bounds; raw units for NRRD/MRC, physical units for FITS.
/// Bounds must be finite and strictly increasing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScalarWindow {
    minimum: f32,
    maximum: f32,
}
impl ScalarWindow {
    pub fn new(minimum: f32, maximum: f32) -> Result<Self, RasterDecodeError> {
        let range = maximum - minimum;
        if !minimum.is_finite() || !maximum.is_finite() || !range.is_finite() || range <= 0.0 {
            return Err(RasterDecodeError::InvalidScalarWindow);
        }
        Ok(Self { minimum, maximum })
    }
    pub fn minimum(self) -> f32 {
        self.minimum
    }
    pub fn maximum(self) -> f32 {
        self.maximum
    }
}
/// Decode a supported scientific scalar array and explicitly window its selected plane.
/// Ordinary color images are rejected; this never overrides their color interpretation.
pub fn decode_raster_with_window(
    request: &DecodeRequest,
    window: ScalarWindow,
) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    let bytes = read_bounded(request)?;
    request.check_cancelled()?;
    if crate::dicom::has_magic(&bytes) {
        return crate::dicom::decode(&bytes, request, Some(window));
    }
    if crate::fits::has_magic(&bytes) {
        let frame = crate::fits::decode(&bytes, request)?.windowed_request(
            f64::from(window.minimum),
            f64::from(window.maximum),
            Some(request),
        )?;
        request.check_cancelled()?;
        return Ok(frame);
    }
    let (raster, bad): (_, fn(&'static str) -> RasterDecodeError) = if crate::nrrd::has_magic(&bytes) {
        (
            crate::nrrd::decode(&bytes, request)?.raster,
            RasterDecodeError::InvalidNrrd,
        )
    } else if crate::mrc::has_magic(&bytes) {
        (
            crate::mrc::decode(&bytes, request)?.raster,
            RasterDecodeError::InvalidMrc,
        )
    } else {
        return Err(RasterDecodeError::ScalarWindowRequiresScientificArray);
    };
    let displayed = windowed_with_request(&raster, window.minimum, window.maximum, bad, Some(request))?;
    request.check_cancelled()?;
    Ok(displayed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_window_api_selects_scientific_planes_only() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let window = ScalarWindow::new(-100., 1000.).unwrap();
        for name in ["nrrd-f32-gzip-little.nrrd", "mrc-f32-big.mrc"] {
            let mut request = DecodeRequest::new(root.join(name));
            request.image_index = 1;
            let frame = decode_raster_with_window(&request, window).unwrap();
            assert_eq!((frame.image_index(), frame.image_count()), (1, 2));
            assert_eq!(frame.color_space(), &RasterColorSpace::LinearSrgb);
            request.image_index = 2;
            assert!(decode_raster_with_window(&request, window).is_err());
        }
        let request = DecodeRequest::new(root.join("pattern.png"));
        assert!(matches!(
            decode_raster_with_window(&request, window),
            Err(RasterDecodeError::ScalarWindowRequiresScientificArray)
        ));
        for (lo, hi) in [
            (0., 0.),
            (1., 0.),
            (f32::NEG_INFINITY, 1.),
            (0., f32::NAN),
            (-f32::MAX, f32::MAX),
        ] {
            assert!(ScalarWindow::new(lo, hi).is_err());
        }
        let mut request = DecodeRequest::new("missing.mrc");
        request.cancellation = Some(crate::GenerationToken::new(
            Arc::new(std::sync::atomic::AtomicU64::new(2)),
            1,
        ));
        assert!(matches!(
            decode_raster_with_window(&request, window),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
    }
}
