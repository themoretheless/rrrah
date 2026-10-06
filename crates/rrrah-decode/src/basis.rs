//! Bounded LDR Basis Universal ETC1S/UASTC image/mip selection.
use crate::{DecodeRequest, RasterDecodeError};
use basis_universal::{TranscodeParameters, Transcoder, TranscoderTextureFormat};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
fn bad(reason: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidBasis(reason)
}
pub(crate) fn has_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(b"sB")
}
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if bytes.len() < 24 || bytes.len() > u32::MAX as usize || !has_magic(bytes) {
        return Err(bad("header"));
    }
    let header_size = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    let data_size = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if header_size.checked_add(data_size) != Some(bytes.len()) {
        return Err(bad("declared file length"));
    }
    // The wrapper converts codec enums with transmute: guard untrusted numeric values first.
    if bytes[20] > 1 || bytes[23] > 2 {
        return Err(bad("unsupported format/texture type (HDR, video or volume)"));
    }
    let mut transcoder = Transcoder::new();
    if !transcoder.validate_header(bytes) || !transcoder.validate_file_checksums(bytes, true) {
        return Err(bad("header/checksum"));
    }
    let info = transcoder.file_info(bytes).ok_or_else(|| bad("file info"))?;
    let images = transcoder.image_count(bytes);
    if !(1..=1024).contains(&images) || (bytes[23] == 2 && images % 6 != 0) {
        return Err(bad("image count"));
    }
    let mut total = 0usize;
    let mut selected = None;
    for image in 0..images {
        request.check_cancelled()?;
        let levels = transcoder.image_level_count(bytes, image);
        if !(1..=32).contains(&levels) {
            return Err(bad("mip count"));
        }
        for level in 0..levels {
            if total == request.image_index {
                selected = Some((image, level));
            }
            total += 1;
        }
    }
    let (image_index, level_index) = selected.ok_or(crate::DecodeError::UnsupportedImageIndex {
        index: request.image_index,
    })?;
    let dimensions = transcoder
        .image_level_description(bytes, image_index, level_index)
        .ok_or_else(|| bad("image level"))?;
    let (width, height) = (dimensions.original_width, dimensions.original_height);
    let output_bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|n| n.checked_mul(4))
        .ok_or(RasterDecodeError::OutputTooLarge)?;
    if width == 0
        || height == 0
        || width > 65536
        || height > 65536
        || output_bytes > crate::raster::MAX_RASTER_BYTES
    {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let budget = request
        .memory_budget
        .clone()
        .unwrap_or_else(|| rrrah_core::MemoryBudget::new(crate::raster::MAX_RASTER_BYTES));
    let reservation = budget
        .try_reserve(output_bytes)
        .map_err(crate::DecodeError::from)?;
    request.check_cancelled()?;
    transcoder
        .prepare_transcoding(bytes)
        .map_err(|_| bad("codebook preparation"))?;
    request.check_cancelled()?;
    let mut output = transcoder
        .transcode_image_level(
            bytes,
            TranscoderTextureFormat::RGBA32,
            TranscodeParameters {
                image_index,
                level_index,
                ..Default::default()
            },
        )
        .map_err(|_| bad("transcode"))?;
    request.check_cancelled()?;
    if output.len() as u64 != output_bytes {
        return Err(bad("output length"));
    }
    if info.m_y_flipped {
        let stride = width as usize * 4;
        for row in 0..height as usize / 2 {
            if row % 64 == 0 {
                request.check_cancelled()?;
            }
            let opposite = height as usize - 1 - row;
            let (before, after) = output.split_at_mut(opposite * stride);
            before[row * stride..(row + 1) * stride].swap_with_slice(&mut after[..stride]);
        }
    }
    let color = if u16::from_le_bytes([bytes[21], bytes[22]]) & 16 != 0 {
        RasterColorSpace::Srgb
    } else {
        RasterColorSpace::LinearSrgb
    };
    let pixels = reservation.try_adopt(output).map_err(crate::DecodeError::from)?;
    Ok(
        DecodedRaster::new(width, height, RasterPixels::Rgba8(pixels.into()), color)?
            .with_image_selection(request.image_index, total)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn encoded(format: basis_universal::BasisTextureFormat, linear: bool) -> Vec<u8> {
        let mut params = basis_universal::CompressorParams::new();
        params.set_basis_format(format);
        params.set_color_space(if linear {
            basis_universal::ColorSpace::Linear
        } else {
            basis_universal::ColorSpace::Srgb
        });
        params.set_generate_mipmaps(true);
        params.set_print_status_to_stdout(false);
        params.source_image_mut(0).init(&[255u8; 8 * 8 * 4], 8, 8, 4);
        let mut compressor = basis_universal::Compressor::default();
        // The compressor's documented unsafe init/process require params and source
        // ownership to outlive processing; both remain owned in this scope.
        #[allow(unsafe_code)]
        unsafe {
            compressor.init(&params);
            compressor.process().unwrap();
        }
        compressor.basis_file().to_vec()
    }
    #[test]
    fn codecs_mips_color_and_managed_output() {
        for format in [
            basis_universal::BasisTextureFormat::ETC1S,
            basis_universal::BasisTextureFormat::UASTC4x4,
        ] {
            for linear in [false, true] {
                let bytes = encoded(format, linear);
                let mut request = DecodeRequest::new("texture.basis");
                let budget = rrrah_core::MemoryBudget::new(256);
                request.memory_budget = Some(budget.clone());
                for index in 0..4 {
                    request.image_index = index;
                    let image = decode(&bytes, &request).unwrap();
                    assert_eq!(
                        (
                            image.width(),
                            image.height(),
                            image.image_index(),
                            image.image_count()
                        ),
                        (8 >> index, 8 >> index, index, 4)
                    );
                    assert_eq!(
                        image.color_space(),
                        &if linear && format == basis_universal::BasisTextureFormat::ETC1S {
                            RasterColorSpace::LinearSrgb
                        } else {
                            // This pinned encoder leaves UASTC marked sRGB even with linear metrics.
                            RasterColorSpace::Srgb
                        }
                    );
                    let RasterPixels::Rgba8(pixels) = image.pixels() else {
                        panic!()
                    };
                    assert!(pixels.is_managed());
                    assert!(pixels.iter().all(|&v| v == 255));
                    drop(image);
                    assert_eq!(budget.used(), 0);
                }
                request.image_index = 4;
                assert!(decode(&bytes, &request).is_err());
                request.image_index = 0;
                request.memory_budget = Some(rrrah_core::MemoryBudget::new(255));
                assert!(decode(&bytes, &request).is_err());
                assert_eq!(request.memory_budget.as_ref().unwrap().peak(), 0);
            }
        }
    }
    #[test]
    fn colored_alpha_blocks_preserve_channels_and_non_aligned_dimensions() {
        // Block-aligned constant colors isolate channel/alpha routing from lossy
        // spatial detail. The 7x5 extent also tests cropping padded codec blocks.
        let colors = [
            [255u8, 0, 0, 255],
            [0, 255, 0, 170],
            [0, 0, 255, 85],
            [255, 255, 0, 0],
        ];
        let mut source = Vec::new();
        for y in 0..5 {
            for x in 0..7 {
                source.extend_from_slice(&colors[(y / 4) * 2 + x / 4]);
            }
        }
        for format in [
            basis_universal::BasisTextureFormat::ETC1S,
            basis_universal::BasisTextureFormat::UASTC4x4,
        ] {
            let mut params = basis_universal::CompressorParams::new();
            params.set_basis_format(format);
            params.set_color_space(basis_universal::ColorSpace::Srgb);
            params.set_generate_mipmaps(false);
            params.set_print_status_to_stdout(false);
            params.source_image_mut(0).init(&source, 7, 5, 4);
            let mut compressor = basis_universal::Compressor::default();
            // Both params and source remain owned through native processing.
            #[allow(unsafe_code)]
            unsafe {
                compressor.init(&params);
                compressor.process().unwrap();
            }
            let budget = rrrah_core::MemoryBudget::new(7 * 5 * 4);
            let mut request = DecodeRequest::new("colored.basis");
            request.memory_budget = Some(budget.clone());
            let image = decode(compressor.basis_file(), &request).unwrap();
            assert_eq!((image.width(), image.height(), image.image_count()), (7, 5, 1));
            let RasterPixels::Rgba8(output) = image.pixels() else {
                panic!()
            };
            assert_eq!(output.len(), source.len());
            for (index, (&actual, &expected)) in output.iter().zip(&source).enumerate() {
                assert!(
                    actual.abs_diff(expected) <= 8,
                    "{format:?} channel {index}: {actual} vs {expected}"
                );
            }
            assert_eq!(budget.used(), 140);
            drop(image);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn multiple_images_flatten_mips_without_aliasing() {
        for format in [
            basis_universal::BasisTextureFormat::ETC1S,
            basis_universal::BasisTextureFormat::UASTC4x4,
        ] {
            let mut params = basis_universal::CompressorParams::new();
            params.set_basis_format(format);
            params.set_generate_mipmaps(true);
            params.set_mipmap_smallest_dimension(1);
            params.set_print_status_to_stdout(false);
            let red: Vec<u8> = [255, 0, 0, 255].repeat(64);
            let green: Vec<u8> = [0, 255, 0, 255].repeat(16);
            params.source_image_mut(0).init(&red, 8, 8, 4);
            params.source_image_mut(1).init(&green, 4, 4, 4);
            let mut compressor = basis_universal::Compressor::default();
            // Native processing finishes while source and params are owned.
            #[allow(unsafe_code)]
            unsafe {
                compressor.init(&params);
                compressor.process().unwrap();
            }
            let budget = rrrah_core::MemoryBudget::new(256);
            let mut request = DecodeRequest::new("multi.basis");
            request.memory_budget = Some(budget.clone());
            for (index, dimension, color) in [
                (0, 8, [255u8, 0, 0, 255]),
                (1, 4, [255, 0, 0, 255]),
                (2, 2, [255, 0, 0, 255]),
                (3, 1, [255, 0, 0, 255]),
                (4, 4, [0, 255, 0, 255]),
                (5, 2, [0, 255, 0, 255]),
                (6, 1, [0, 255, 0, 255]),
            ] {
                request.image_index = index;
                let image = decode(compressor.basis_file(), &request).unwrap();
                assert_eq!(
                    (
                        image.image_index(),
                        image.image_count(),
                        image.width(),
                        image.height()
                    ),
                    (index, 7, dimension, dimension)
                );
                let RasterPixels::Rgba8(values) = image.pixels() else {
                    panic!()
                };
                assert!(
                    values
                        .chunks_exact(4)
                        .all(|pixel| pixel.iter().zip(color).all(|(&v, c)| v.abs_diff(c) <= 8))
                );
                drop(image);
                assert_eq!(budget.used(), 0);
            }
            request.image_index = 7;
            assert!(matches!(
                decode(compressor.basis_file(), &request),
                Err(RasterDecodeError::Source(
                    crate::DecodeError::UnsupportedImageIndex { .. }
                ))
            ));
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn corruption_truncation_and_unrecognized_enums_refuse() {
        let bytes = encoded(basis_universal::BasisTextureFormat::UASTC4x4, false);
        let request = DecodeRequest::new("bad.basis");
        for n in 0..bytes.len() {
            assert!(decode(&bytes[..n], &request).is_err(), "{n}");
        }
        for offset in [6, 12, 20, 23, bytes.len() - 1] {
            let mut broken = bytes.clone();
            broken[offset] ^= 255;
            assert!(decode(&broken, &request).is_err());
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode(&trailing, &request).is_err());
    }
}
