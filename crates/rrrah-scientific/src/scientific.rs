//! Explicit scalar-array display interpretation shared by scientific formats.
use crate::{DecodeRequest, bounded_io::read_bounded, raster::RasterDecodeError};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
// Only for a raster freshly decoded with this same request/budget. Borrowed
// public image methods continue to allocate separate output and retain raw units.
fn windowed_owned(
    raster: DecodedRaster,
    minimum: f32,
    maximum: f32,
    bad: fn(&'static str) -> RasterDecodeError,
    request: &DecodeRequest,
) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    let range = maximum - minimum;
    if !minimum.is_finite() || !maximum.is_finite() || !range.is_finite() || range <= 0. {
        return Err(bad("window must have finite increasing bounds"));
    }
    let info = (
        raster.width(),
        raster.height(),
        raster.image_index(),
        raster.image_count(),
        raster.pixel_aspect(),
    );
    let RasterPixels::Rgba32Float(mut samples) = raster.pixels().clone() else {
        return Err(bad("unexpected sample layout"));
    };
    drop(raster);
    let frame = |samples, color| {
        DecodedRaster::new(info.0, info.1, RasterPixels::Rgba32Float(samples), color)?
            .with_image_selection(info.2, info.3)?
            .with_pixel_aspect(info.4)
    };
    let Some(output) = samples.get_mut() else {
        let shared = frame(samples, RasterColorSpace::Unspecified)?;
        return windowed_with_request(&shared, minimum, maximum, bad, Some(request));
    };
    for (i, pixel) in output.chunks_exact_mut(4).enumerate() {
        if i % 4096 == 0 {
            request.check_cancelled()?;
        }
        if !pixel[0].is_finite() {
            return Err(bad("nonfinite sample requires an explicit missing-value policy"));
        }
        let value = ((pixel[0] - minimum) / range).clamp(0., 1.);
        pixel.copy_from_slice(&[value, value, value, 1.]);
    }
    request.check_cancelled()?;
    Ok(frame(samples, RasterColorSpace::LinearSrgb)?)
}
pub(crate) fn windowed_with_request(
    raster: &DecodedRaster,
    minimum: f32,
    maximum: f32,
    bad: fn(&'static str) -> RasterDecodeError,
    request: Option<&DecodeRequest>,
) -> Result<DecodedRaster, RasterDecodeError> {
    if let Some(request) = request {
        request.check_cancelled()?;
    }
    let range = maximum - minimum;
    if !minimum.is_finite() || !maximum.is_finite() || !range.is_finite() || range <= 0.0 {
        return Err(bad("window must have finite increasing bounds"));
    }
    let RasterPixels::Rgba32Float(source) = raster.pixels() else {
        return Err(bad("unexpected sample layout"));
    };
    let mut out: rrrah_core::PixelBuffer<f32> = match request.and_then(|r| r.memory_budget.as_ref()) {
        Some(budget) => budget
            .try_buffer(source.len(), 0f32)
            .map_err(crate::DecodeError::Memory)?
            .freeze()
            .into(),
        None => Arc::new(vec![0f32; source.len()]).into(),
    };
    let output = out.get_mut().unwrap();
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
        output[index * 4..index * 4 + 4].copy_from_slice(&[value, value, value, 1.0]);
    }
    Ok(DecodedRaster::new(
        raster.width(),
        raster.height(),
        RasterPixels::Rgba32Float(out),
        RasterColorSpace::LinearSrgb,
    )?
    .with_image_selection(raster.image_index(), raster.image_count())?
    .with_pixel_aspect(raster.pixel_aspect())?)
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
        let image = crate::fits::decode(&bytes, request)?;
        drop(bytes);
        let frame = image.windowed_request(
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
    // Selected pixels own their storage; source credit can fund the window.
    drop(bytes);
    let displayed = windowed_owned(raster, window.minimum, window.maximum, bad, request)?;
    request.check_cancelled()?;
    Ok(displayed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_window_uses_one_float_plane_under_low_file_budget() {
        let dir = tempfile::tempdir().unwrap();
        let mut nrrd = b"NRRD0005\ntype: uint8\ndimension: 2\nsizes: 32 32\nencoding: raw\n\n".to_vec();
        nrrd.resize(nrrd.len() + 1024, 7);
        let mut mrc = include_bytes!("../../../tests/fixtures/raster/mrc-i8-little.mrc")[..1056].to_vec();
        for (at, value) in [(0, 32u32), (4, 32), (8, 1)] {
            mrc[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        mrc.resize(1056 + 1024, 7);
        let nrrd_metadata = crate::nrrd::header_credit(&nrrd, &DecodeRequest::new("header")).unwrap();
        for (name, bytes, metadata) in [("test.nrrd", nrrd, nrrd_metadata), ("test.mrc", mrc, 32)] {
            let path = dir.path().join(name);
            std::fs::write(&path, &bytes).unwrap();
            let cap = bytes.len() as u64 + metadata + 16384;
            assert!(cap < 32768, "two float planes cannot fit");
            let budget = rrrah_core::MemoryBudget::new(cap);
            let mut request = DecodeRequest::new(path);
            request.memory_budget = Some(budget.clone());
            let result = decode_raster_with_window(&request, ScalarWindow::new(0., 10.).unwrap()).unwrap();
            assert_eq!((budget.used(), budget.peak()), (16384, cap));
            let RasterPixels::Rgba32Float(pixels) = result.pixels() else {
                panic!()
            };
            assert!(pixels.chunks_exact(4).all(|p| p == [0.7, 0.7, 0.7, 1.]));
            drop(result);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn owned_window_reuses_full_budget_but_never_mutates_shared_native_samples() {
        let budget = rrrah_core::MemoryBudget::new(48);
        let mut native = budget.try_buffer(12, 1f32).unwrap();
        native[..].copy_from_slice(&[-1., -1., -1., 1., 0., 0., 0., 1., 1., 1., 1., 1.]);
        let samples: rrrah_core::PixelBuffer<f32> = native.freeze().into();
        let address = samples.as_ptr();
        let frame = DecodedRaster::new(
            3,
            1,
            RasterPixels::Rgba32Float(samples),
            RasterColorSpace::Unspecified,
        )
        .unwrap()
        .with_image_selection(1, 2)
        .unwrap()
        .with_pixel_aspect(Some(2.))
        .unwrap();
        let mut request = DecodeRequest::new("already-decoded");
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            windowed_owned(frame.clone(), -1., 1., RasterDecodeError::InvalidNrrd, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        let RasterPixels::Rgba32Float(original) = frame.pixels() else {
            panic!()
        };
        assert_eq!(original[0], -1.);
        let displayed = windowed_owned(frame, -1., 1., RasterDecodeError::InvalidNrrd, &request).unwrap();
        let RasterPixels::Rgba32Float(pixels) = displayed.pixels() else {
            panic!()
        };
        assert_eq!(pixels.as_ptr(), address);
        assert_eq!(&pixels[..], &[0., 0., 0., 1., 0.5, 0.5, 0.5, 1., 1., 1., 1., 1.]);
        assert_eq!(displayed.pixel_aspect(), Some(2.));
        assert_eq!((displayed.image_index(), displayed.image_count()), (1, 2));
        assert_eq!(displayed.color_space(), &RasterColorSpace::LinearSrgb);
        assert_eq!((budget.used(), budget.peak()), (48, 48));
        drop(displayed);
        assert_eq!(budget.used(), 0);
    }
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
