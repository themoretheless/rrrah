//! HEVC still images. libheif applies item rotation, mirroring and cropping.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use libheif_rs::{
    Channel, ColorPrimaries, ColorSpace, DecodingOptions, HeifContext, LibHeif, RgbChroma,
    TransferCharacteristics,
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;

fn invalid(message: impl ToString) -> RasterDecodeError {
    RasterDecodeError::InvalidHeif(message.to_string())
}

pub(crate) fn decoder_identity() -> Result<[u8; 32], RasterDecodeError> {
    let library = LibHeif::new_checked().map_err(invalid)?;
    let descriptors = library.decoder_descriptors(256, Some(libheif_rs::CompressionFormat::Hevc));
    if descriptors.len() == 256 {
        return Err(invalid("HEVC decoder inventory exceeds identity limit"));
    }
    let mut hash = blake3::Hasher::new();
    hash.update(b"rrrah-hevc-decoder-inventory-v1");
    hash.update(&library.version());
    hash.update(&(descriptors.len() as u64).to_le_bytes());
    for descriptor in descriptors {
        for value in [descriptor.id().to_owned(), descriptor.name()] {
            hash.update(&(value.len() as u64).to_le_bytes());
            hash.update(value.as_bytes());
        }
    }
    Ok(*hash.finalize().as_bytes())
}

pub(crate) fn has_magic(bytes: &[u8]) -> bool {
    if bytes.len() < 16 || &bytes[4..8] != b"ftyp" {
        return false;
    }
    let size = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
    if size < 16 || size % 4 != 0 {
        return false;
    }
    let brands = std::iter::once(&bytes[8..12]).chain(bytes[16..size.min(bytes.len())].chunks_exact(4));
    // Generic mif1 is shared with AVIF; accept only HEVC-specific brands.
    brands
        .into_iter()
        .any(|brand| matches!(brand, b"heic" | b"heix" | b"hevc" | b"hevx"))
}

fn check_size(width: u32, height: u32, bytes_per_pixel: u64) -> Result<(), RasterDecodeError> {
    if width == 0 || height == 0 {
        return Err(invalid("empty image"));
    }
    // Include native planes and the copied output, not just the final RGBA.
    if width > 65536
        || height > 65536
        || u64::from(width) * u64::from(height) * bytes_per_pixel * 3 > MAX_RASTER_BYTES
    {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    Ok(())
}

pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    let lib = LibHeif::new_checked().map_err(invalid)?;
    let mut context = HeifContext::read_from_bytes(bytes).map_err(invalid)?;
    context.set_max_decoding_threads(2);
    let handle = context.primary_image_handle().map_err(invalid)?;
    if handle.is_premultiplied_alpha() {
        return Err(invalid("premultiplied alpha is not supported"));
    }
    let depth = handle.luma_bits_per_pixel().max(handle.chroma_bits_per_pixel());
    if !(8..=16).contains(&depth) {
        return Err(invalid("unsupported sample depth"));
    }
    let high = depth > 8;
    check_size(handle.width(), handle.height(), if high { 8 } else { 4 })?;
    if handle.ispe_width() > 0 && handle.ispe_height() > 0 {
        check_size(
            handle.ispe_width() as u32,
            handle.ispe_height() as u32,
            if high { 8 } else { 4 },
        )?;
    }
    let mut options = DecodingOptions::new().ok_or_else(|| invalid("cannot allocate decoding options"))?;
    options.set_convert_hdr_to_8bit(false);
    options.set_strict_decoding(true);
    let image = lib
        .decode(
            &handle,
            ColorSpace::Rgb(if high {
                RgbChroma::HdrRgbaBe
            } else {
                RgbChroma::Rgba
            }),
            Some(options),
        )
        .map_err(invalid)?;
    request.check_cancelled()?;
    let (width, height) = (image.width(), image.height());
    check_size(width, height, if high { 8 } else { 4 })?;
    let color = if let Some(profile) = image.color_profile_raw().or_else(|| handle.color_profile_raw()) {
        if profile.data.len() > 4 * 1024 * 1024 {
            return Err(invalid("ICC profile exceeds limit"));
        }
        RasterColorSpace::Icc(profile.data)
    } else if let Some(profile) = image.color_profile_nclx().or_else(|| handle.color_profile_nclx()) {
        match (profile.color_primaries(), profile.transfer_characteristics()) {
            (ColorPrimaries::ITU_R_BT_709_5, TransferCharacteristics::IEC_61966_2_1) => {
                RasterColorSpace::Srgb
            }
            (ColorPrimaries::ITU_R_BT_709_5, TransferCharacteristics::Linear) => RasterColorSpace::LinearSrgb,
            _ => RasterColorSpace::Unspecified,
        }
    } else {
        RasterColorSpace::Unspecified
    };
    let plane = image
        .planes()
        .interleaved
        .ok_or_else(|| invalid("missing RGBA plane"))?;
    let row_bytes = width as usize * if high { 8 } else { 4 };
    if plane.width != width || plane.height != height || plane.stride < row_bytes {
        return Err(invalid("invalid RGBA plane geometry"));
    }
    let pixels = if high {
        let depth = image
            .bits_per_pixel(Channel::Interleaved)
            .ok_or_else(|| invalid("missing sample depth"))?;
        if !(9..=16).contains(&depth) || plane.storage_bits_per_pixel != 64 {
            return Err(invalid("unexpected 16-bit RGBA storage"));
        }
        let maximum = (1u32 << depth) - 1;
        let mut out = Vec::with_capacity(width as usize * height as usize * 4);
        for row in plane.data.chunks_exact(plane.stride).take(height as usize) {
            request.check_cancelled()?;
            for value in row[..row_bytes].chunks_exact(2) {
                let value = u32::from(u16::from_be_bytes([value[0], value[1]]));
                if value > maximum {
                    return Err(invalid("sample exceeds declared depth"));
                }
                out.push(((value * 65535 + maximum / 2) / maximum) as u16);
            }
        }
        RasterPixels::Rgba16(Arc::new(out).into())
    } else {
        if plane.storage_bits_per_pixel != 32 || plane.bits_per_pixel != 8 {
            return Err(invalid("unexpected 8-bit RGBA storage"));
        }
        let mut out = Vec::with_capacity(width as usize * height as usize * 4);
        for row in plane.data.chunks_exact(plane.stride).take(height as usize) {
            request.check_cancelled()?;
            out.extend_from_slice(&row[..row_bytes]);
        }
        RasterPixels::Rgba8(Arc::new(out).into())
    };
    Ok(DecodedRaster::new(width, height, pixels, color)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn twelve_bit_samples_remain_distinct() {
        let frame = decode(
            include_bytes!("../../../tests/fixtures/raster/precision12.heic"),
            &DecodeRequest::new("precision12.heic"),
        )
        .unwrap();
        let RasterPixels::Rgba16(pixels) = frame.pixels() else {
            panic!("lost precision")
        };
        for (index, pixel) in pixels.chunks_exact(4).enumerate() {
            let value = if index % 2 == 0 { 32776 } else { 32792 };
            assert_eq!(pixel, &[value, value, value, 65535]);
        }
        assert_eq!(frame.color_space(), &RasterColorSpace::Srgb);
    }
    #[test]
    fn profiled_image_reaches_display() {
        let frame = decode(
            include_bytes!("../../../tests/fixtures/raster/profiled.heic"),
            &DecodeRequest::new("renamed.cr3"),
        )
        .unwrap();
        assert!(matches!(frame.color_space(), RasterColorSpace::Icc(_)));
        crate::prepare_raster_for_display(&frame).unwrap();
    }
    #[test]
    fn truncated_native_container_and_excessive_dimensions_are_rejected() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/alpha.heic");
        assert!(decode(&bytes[..bytes.len() / 2], &DecodeRequest::new("broken.heic")).is_err());
        assert!(matches!(
            check_size(65536, 65536, 8),
            Err(RasterDecodeError::OutputTooLarge)
        ));
    }
    #[test]
    fn hevc_brands_do_not_capture_avif() {
        assert!(has_magic(b"\0\0\0\x18ftypmif1\0\0\0\0heicmif1"));
        assert!(!has_magic(b"\0\0\0\x18ftypavif\0\0\0\0avifmif1"));
        assert!(!has_magic(b"\0\0\0\x08ftyp"));
    }
}
