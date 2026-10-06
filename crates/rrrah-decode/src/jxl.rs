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
    let color = decoder
        .icc_profile()?
        .map_or(RasterColorSpace::Unspecified, RasterColorSpace::Icc);
    let image = DynamicImage::from_decoder(decoder)?;
    request.check_cancelled()?;
    let (width, height) = (image.width(), image.height());
    let pixels = match sample_bytes {
        2 => RasterPixels::Rgba16(Arc::new(image.into_rgba16().into_raw()).into()),
        4 => RasterPixels::Rgba32Float(Arc::new(image.into_rgba32f().into_raw()).into()),
        _ => RasterPixels::Rgba8(Arc::new(image.into_rgba8().into_raw()).into()),
    };
    Ok(DecodedRaster::new(width, height, pixels, color)?)
}

#[cfg(test)]
mod tests {
    use super::*;
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
