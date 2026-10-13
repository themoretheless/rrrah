//! HEVC still images. libheif applies item rotation, mirroring and cropping.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use libheif_rs::{
    Channel, ColorPrimaries, ColorSpace, DecodingOptions, HeifContext, LibHeif, MatrixCoefficients,
    RgbChroma, TransferCharacteristics,
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
    // This is the retained RGBA limit. Native working credit is admitted
    // separately below; applying its multiplier here rejected ordinary 24 MP
    // 10-bit camera images despite a 192 MB retained output.
    if width > 65536
        || height > 65536
        || u64::from(width) * u64::from(height) * bytes_per_pixel > MAX_RASTER_BYTES
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
    let bytes_per_pixel = if high { 8 } else { 4 };
    check_size(handle.width(), handle.height(), if high { 8 } else { 4 })?;
    if handle.ispe_width() > 0 && handle.ispe_height() > 0 {
        check_size(
            handle.ispe_width() as u32,
            handle.ispe_height() as u32,
            if high { 8 } else { 4 },
        )?;
    }
    let output_bytes = u64::from(handle.width()) * u64::from(handle.height()) * bytes_per_pixel;
    let encoded_bytes = if handle.ispe_width() > 0 && handle.ispe_height() > 0 {
        handle.ispe_width() as u64 * handle.ispe_height() as u64 * bytes_per_pixel
    } else {
        output_bytes
    };
    let admit = |bytes| {
        request
            .memory_budget
            .as_ref()
            .map(|budget| budget.try_reserve(bytes))
            .transpose()
            .map_err(crate::DecodeError::Memory)
            .map_err(RasterDecodeError::Source)
    };
    // Reserve before invoking the codec, including two conservative native
    // plane equivalents. This is estimated working credit, not measurement of
    // every HEVC plugin allocation. Retained output credit follows its pixels.
    let output_reservation = admit(output_bytes)?;
    let _native_working = admit(encoded_bytes.max(output_bytes) * 2)?;
    request.check_cancelled()?;
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
            (
                ColorPrimaries::ITU_R_BT_2020_2_and_2100_0,
                transfer @ (TransferCharacteristics::ITU_R_BT_2100_0_PQ
                | TransferCharacteristics::ITU_R_BT_2100_0_HLG),
            ) if profile.matrix_coefficients() != MatrixCoefficients::Unknown => {
                // libheif has reconstructed RGB, but has not applied the HDR
                // transfer function. Preserve the declaration for the shared
                // explicit PQ/HLG display policy in decode_raster_bytes.
                RasterColorSpace::Cicp {
                    primaries: 9,
                    transfer: transfer as u16,
                    matrix: profile.matrix_coefficients() as u16,
                    full_range: profile.full_range_flag() != 0,
                }
            }
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
        RasterPixels::Rgba16(if let Some(reservation) = output_reservation {
            reservation
                .try_adopt(out)
                .map_err(crate::DecodeError::Memory)?
                .into()
        } else {
            Arc::new(out).into()
        })
    } else {
        if plane.storage_bits_per_pixel != 32 || plane.bits_per_pixel != 8 {
            return Err(invalid("unexpected 8-bit RGBA storage"));
        }
        let mut out = Vec::with_capacity(width as usize * height as usize * 4);
        for row in plane.data.chunks_exact(plane.stride).take(height as usize) {
            request.check_cancelled()?;
            out.extend_from_slice(&row[..row_bytes]);
        }
        RasterPixels::Rgba8(if let Some(reservation) = output_reservation {
            reservation
                .try_adopt(out)
                .map_err(crate::DecodeError::Memory)?
                .into()
        } else {
            Arc::new(out).into()
        })
    };
    Ok(DecodedRaster::new(width, height, pixels, color)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_output_cap_and_native_working_admission_are_separate() {
        check_size(6000, 4000, 8).unwrap();
        check_size(8192, 8192, 8).unwrap();
        assert!(matches!(
            check_size(8192, 8193, 8),
            Err(RasterDecodeError::OutputTooLarge)
        ));
        let bytes = include_bytes!("../../../tests/fixtures/raster/precision12.heic");
        let retained = 64 * 48 * 8;
        // The producer codes a 64x64 image and crops sixteen rows.
        let working_peak = retained + 2 * 64 * 64 * 8;
        let mut request = DecodeRequest::new("precision12.heic");
        let insufficient = rrrah_core::MemoryBudget::new(working_peak - 1);
        request.memory_budget = Some(insufficient.clone());
        assert!(matches!(
            decode(bytes, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!(insufficient.used(), 0);
        let exact = rrrah_core::MemoryBudget::new(working_peak);
        request.memory_budget = Some(exact.clone());
        let raster = decode(bytes, &request).unwrap();
        assert_eq!(exact.peak(), working_peak);
        assert_eq!(exact.used(), retained);
        drop(raster);
        assert_eq!(exact.used(), 0);
    }
    #[test]
    fn hdr_hevc_color_codes_and_primary_conversion_match_independent_oracle() {
        for (name, bytes, hlg) in [
            (
                "heif-pq12-color.heic",
                include_bytes!("../../../tests/fixtures/raster/heif-pq12-color.heic").as_slice(),
                false,
            ),
            (
                "heif-hlg12-color.heic",
                include_bytes!("../../../tests/fixtures/raster/heif-hlg12-color.heic").as_slice(),
                true,
            ),
            (
                "heif-pq12-color-alpha.heic",
                include_bytes!("../../../tests/fixtures/raster/heif-pq12-color-alpha.heic").as_slice(),
                false,
            ),
            (
                "heif-hlg12-color-alpha.heic",
                include_bytes!("../../../tests/fixtures/raster/heif-hlg12-color-alpha.heic").as_slice(),
                true,
            ),
        ] {
            let mut request = DecodeRequest::new(name);
            let native = decode(bytes, &request).unwrap();
            let RasterPixels::Rgba16(samples) = native.pixels() else {
                panic!()
            };
            let codes = [[4095_u32, 0, 0], [0, 4095, 0], [0, 0, 4095], [2048, 3072, 1024]];
            for (i, pixel) in samples.chunks_exact(4).enumerate() {
                let expected = codes[i % 64 / 16].map(|v| ((v * 65535 + 2047) / 4095) as u16);
                assert_eq!(&pixel[..3], &expected, "{name}: pixel {i}");
                let alpha = if name.contains("alpha") {
                    [0_u32, 1024, 2048, 4095][i / 64 / 12]
                } else {
                    4095
                };
                assert_eq!(pixel[3], ((alpha * 65535 + 2047) / 4095) as u16);
            }
            if hlg {
                request.hlg_display = Some((100., 1000., 1.2));
            } else {
                request.pq_reference_white_nits = Some(100.);
            }
            let linear = crate::raster::decode_raster_bytes(bytes.to_vec(), &request).unwrap();
            let RasterPixels::Rgba32Float(values) = linear.pixels() else {
                panic!()
            };
            for (pixel, native) in values.chunks_exact(4).zip(samples.chunks_exact(4)) {
                assert_eq!(pixel[3], f32::from(native[3]) / 65535.);
            }
            let oracle = if hlg {
                include_str!("../../../tests/fixtures/raster/hlg-color-decimal-oracle.tsv")
            } else {
                include_str!("../../../tests/fixtures/raster/pq-color-decimal-oracle.tsv")
            };
            for band in 0..4 {
                let rgb = codes[band].map(|v| (v * 65535 + 2047) / 4095);
                let reference = oracle
                    .lines()
                    .find_map(|line| {
                        let fields: Vec<_> = line.split('\t').collect();
                        let input_at = if hlg { 3 } else { 1 };
                        if fields[0] != "100" || (hlg && (fields[1] != "1000" || fields[2] != "1.2")) {
                            return None;
                        }
                        if (0..3).any(|c| fields[input_at + c].parse::<u32>().unwrap() != rgb[c]) {
                            return None;
                        }
                        Some(std::array::from_fn::<_, 3, _>(|c| {
                            fields[input_at + 4 + c].parse::<f64>().unwrap()
                        }))
                    })
                    .expect("independent Decimal reference");
                for y in 0..48 {
                    for x in band * 16..(band + 1) * 16 {
                        for c in 0..3 {
                            let actual = f64::from(values[(y * 64 + x) * 4 + c]);
                            assert!(
                                (actual - reference[c]).abs() <= reference[c].abs().max(1.) * 2e-6,
                                "{name} {x},{y} channel {c}: {actual} != {}",
                                reference[c]
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn hdr_hevc_retains_twelve_bit_samples_and_requires_explicit_display_policy() {
        for (name, bytes, transfer) in [
            (
                "heif-pq12.heic",
                include_bytes!("../../../tests/fixtures/raster/heif-pq12.heic").as_slice(),
                16,
            ),
            (
                "heif-hlg12.heic",
                include_bytes!("../../../tests/fixtures/raster/heif-hlg12.heic").as_slice(),
                18,
            ),
        ] {
            let mut request = DecodeRequest::new(name);
            let native = decode(bytes, &request).unwrap();
            assert_eq!(
                native.color_space(),
                &RasterColorSpace::Cicp {
                    primaries: 9,
                    transfer,
                    matrix: 0,
                    full_range: true,
                }
            );
            assert!(crate::prepare_raster_for_display(&native).is_err());
            let RasterPixels::Rgba16(samples) = native.pixels() else {
                panic!("lost HEVC precision")
            };
            let codes = [0_u32, 1024, 2048, 2049, 3072, 4095, 512, 1536];
            for (index, pixel) in samples.chunks_exact(4).enumerate() {
                let value = ((codes[index % 64 / 8] * 65535 + 2047) / 4095) as u16;
                assert_eq!(pixel, &[value, value, value, 65535], "{name}: pixel {index}");
            }
            let budget = rrrah_core::MemoryBudget::new(128 * 1024);
            request.memory_budget = Some(budget.clone());
            if transfer == 16 {
                request.pq_reference_white_nits = Some(100.);
            } else {
                request.hlg_display = Some((100., 1000., 1.2));
            }
            let linear = crate::raster::decode_raster_bytes(bytes.to_vec(), &request).unwrap();
            assert_eq!(linear.color_space(), &RasterColorSpace::LinearSrgb);
            let RasterPixels::Rgba32Float(samples) = linear.pixels() else {
                panic!()
            };
            assert!(
                samples
                    .chunks_exact(4)
                    .any(|p| p[0] > if transfer == 16 { 99. } else { 9.9 })
            );
            assert!(samples.chunks_exact(4).all(|p| p[3] == 1.));
            drop(linear);
            assert_eq!(budget.used(), 0);
        }
    }
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
