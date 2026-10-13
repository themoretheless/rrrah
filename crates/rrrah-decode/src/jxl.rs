//! JPEG XL first-frame decoding. The codec applies codestream orientation.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use image::{ColorType, DynamicImage, ImageDecoder, Limits};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{io::Cursor, sync::Arc};

pub(crate) fn has_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xff, 0x0a]) || bytes.starts_with(b"\0\0\0\x0cJXL \r\n\x87\n")
}

pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    let mut decoder = jxl_oxide::integration::JxlDecoder::new(Cursor::new(bytes))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(65536);
    limits.max_image_height = Some(65536);
    limits.max_alloc = Some(MAX_RASTER_BYTES);
    decoder.set_limits(limits)?;
    let (width, height) = decoder.dimensions();
    let sample_bytes = match decoder.color_type() {
        ColorType::L16 | ColorType::La16 | ColorType::Rgb16 | ColorType::Rgba16 => 2,
        ColorType::Rgb32F | ColorType::Rgba32F => 4,
        _ => 1,
    };
    if u64::from(width)
        .saturating_mul(u64::from(height))
        .saturating_mul(4 * sample_bytes)
        > MAX_RASTER_BYTES
    {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    // Admit the retained RGBA allocation before pixel decoding/conversion.
    let output_bytes = u64::from(width) * u64::from(height) * 4 * sample_bytes;
    let output_reservation = request
        .memory_budget
        .as_ref()
        .map(|budget| budget.try_reserve(output_bytes))
        .transpose()
        .map_err(crate::DecodeError::Memory)?;
    // Bound tracker-managed codec allocations independently of retained output.
    // Reserve an additional source-image equivalent for DynamicImage/RGBA overlap.
    // Header parsing precedes this admission; tracker limits are not an RSS bound.
    let codec_bytes = (65536 + u64::from(width) * u64::from(height) * 64).min(MAX_RASTER_BYTES);
    let _working = request
        .memory_budget
        .as_ref()
        .map(|budget| budget.try_reserve(codec_bytes + output_bytes))
        .transpose()
        .map_err(crate::DecodeError::Memory)?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(65536);
    limits.max_image_height = Some(65536);
    limits.max_alloc = Some(codec_bytes);
    decoder.set_limits(limits)?;
    let color = decoder
        .icc_profile()?
        .map_or(RasterColorSpace::Unspecified, RasterColorSpace::Icc);
    let image = DynamicImage::from_decoder(decoder)?;
    request.check_cancelled()?;
    let (width, height) = (image.width(), image.height());
    let pixels = match sample_bytes {
        2 => {
            let values = image.into_rgba16().into_raw();
            RasterPixels::Rgba16(if let Some(reservation) = output_reservation {
                reservation
                    .try_adopt(values)
                    .map_err(crate::DecodeError::Memory)?
                    .into()
            } else {
                Arc::new(values).into()
            })
        }
        4 => {
            let values = image.into_rgba32f().into_raw();
            RasterPixels::Rgba32Float(if let Some(reservation) = output_reservation {
                reservation
                    .try_adopt(values)
                    .map_err(crate::DecodeError::Memory)?
                    .into()
            } else {
                Arc::new(values).into()
            })
        }
        _ => {
            let values = image.into_rgba8().into_raw();
            RasterPixels::Rgba8(if let Some(reservation) = output_reservation {
                reservation
                    .try_adopt(values)
                    .map_err(crate::DecodeError::Memory)?
                    .into()
            } else {
                Arc::new(values).into()
            })
        }
    };
    Ok(DecodedRaster::new(width, height, pixels, color)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn truncated_stream_errors_release_admitted_output_and_working_credit() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/alpha-container-1.jxl");
        let mut admitted_failures = 0;
        for end in 1..bytes.len() {
            let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
            let mut request = DecodeRequest::new("truncated.jxl");
            request.memory_budget = Some(budget.clone());
            let result = decode(&bytes[..end], &request);
            if result.is_err() && budget.peak() > 65536 {
                admitted_failures += 1;
            }
            drop(result);
            assert_eq!(budget.used(), 0, "truncated at {end}");
        }
        assert!(
            admitted_failures > 0,
            "must exercise errors after working admission"
        );
    }
    #[test]
    fn output_admission_precedes_pixel_decode_and_tracks_shared_owners() {
        for bytes in [
            include_bytes!("../../../tests/fixtures/raster/precision16.jxl").as_slice(),
            include_bytes!("../../../tests/fixtures/raster/alpha-container-1.jxl").as_slice(),
        ] {
            let reference = decode(bytes, &DecodeRequest::new("image.jxl")).unwrap();
            let weight = reference.pixel_capacity_bytes();
            let budget = rrrah_core::MemoryBudget::new(weight - 1);
            let mut request = DecodeRequest::new("image.jxl");
            request.memory_budget = Some(budget.clone());
            assert!(matches!(
                decode(bytes, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(budget.peak(), 0);
            let working = 65536 + u64::from(reference.width()) * u64::from(reference.height()) * 64 + weight;
            let budget = rrrah_core::MemoryBudget::new(weight);
            request.memory_budget = Some(budget.clone());
            assert!(matches!(
                decode(bytes, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(budget.used(), 0);
            let budget = rrrah_core::MemoryBudget::new(weight + working);
            request.memory_budget = Some(budget.clone());
            let frame = decode(bytes, &request).unwrap();
            assert_eq!(budget.used(), weight);
            let retained = frame.clone();
            drop(frame);
            assert_eq!(budget.used(), weight);
            drop(retained);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn sixteen_bit_samples_remain_distinct() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/precision16.jxl");
        let frame = decode(bytes, &DecodeRequest::new("precision16.jxl")).unwrap();
        let RasterPixels::Rgba16(pixels) = frame.pixels() else {
            panic!("lost precision")
        };
        assert_eq!(
            pixels.as_slice(),
            &[32768, 32768, 32768, 65535, 32769, 32769, 32769, 65535]
        );
    }
    #[test]
    fn icc_profile_reaches_display_and_alpha_is_retained() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/alpha-container-1.jxl");
        let frame = decode(bytes, &DecodeRequest::new("renamed.cr3")).unwrap();
        assert!(matches!(frame.color_space(), RasterColorSpace::Icc(_)));
        let prepared = crate::prepare_raster_for_display(&frame).unwrap();
        let RasterPixels::Rgba32Float(pixels) = prepared.pixels() else {
            panic!()
        };
        assert!((pixels[7] - 128.0 / 255.0).abs() < 1e-7);
    }
}
