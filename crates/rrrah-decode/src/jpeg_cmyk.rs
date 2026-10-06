//! Preserve profiled JPEG CMYK channels until ICC conversion.
use crate::{DecodeRequest, RasterDecodeError};
use image::DynamicImage;
use moxcms::{ColorProfile, DataColorSpace, Layout, TransformOptions};
use zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions};
fn bad(message: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidJpegColor(message)
}
pub(crate) fn decode_profiled(
    bytes: &[u8],
    profile: Option<&[u8]>,
    width: u32,
    height: u32,
    request: &DecodeRequest,
) -> Result<Option<DynamicImage>, RasterDecodeError> {
    request.check_cancelled()?;
    let Some(profile) = profile else { return Ok(None) };
    let Ok(profile) = ColorProfile::new_from_slice(profile) else {
        return Ok(None);
    };
    if profile.color_space != DataColorSpace::Cmyk {
        return Ok(None);
    }
    // Parse only header segments, never search entropy-coded data for markers.
    let mut offset = 2;
    let mut adobe = None;
    while offset < bytes.len() {
        if bytes.get(offset) != Some(&255) {
            return Err(bad("JPEG marker"));
        }
        while bytes.get(offset) == Some(&255) {
            offset += 1;
        }
        let marker = *bytes.get(offset).ok_or(bad("truncated marker"))?;
        offset += 1;
        if marker == 0xda || marker == 0xd9 {
            break;
        }
        if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        let length = bytes.get(offset..offset + 2).ok_or(bad("truncated segment"))?;
        let length = usize::from(u16::from_be_bytes([length[0], length[1]]));
        if length < 2 {
            return Err(bad("segment length"));
        }
        let payload = bytes
            .get(offset + 2..offset + length)
            .ok_or(bad("segment extent"))?;
        if marker == 0xee && payload.starts_with(b"Adobe") {
            if payload.len() != 12 || !matches!(payload[11], 0 | 2) {
                return Err(bad("CMYK APP14 transform requires qualification"));
            }
            if adobe.is_some_and(|previous| previous != payload[11]) {
                return Err(bad("conflicting Adobe APP14 transforms"));
            }
            adobe = Some(payload[11]);
        }
        offset += length;
    }
    if adobe.is_none() {
        return Err(bad("CMYK sample convention requires APP14"));
    }
    let ycck = adobe == Some(2);
    let channels = if ycck { ColorSpace::YCCK } else { ColorSpace::CMYK };
    let options = DecoderOptions::default()
        .set_max_width(width as usize)
        .set_max_height(height as usize)
        .jpeg_set_out_colorspace(channels);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    decoder.decode_headers().map_err(|_| bad("CMYK JPEG headers"))?;
    if decoder.input_colorspace() != Some(channels)
        || decoder.dimensions() != Some((width as usize, height as usize))
    {
        return Err(bad("CMYK channel/extent mismatch"));
    }
    let count = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or(RasterDecodeError::OutputTooLarge)?;
    let mut reservation = request
        .memory_budget
        .as_ref()
        .map(|b| b.try_reserve(count as u64))
        .transpose()
        .map_err(crate::DecodeError::Memory)?;
    request.check_cancelled()?;
    let source = decoder.decode().map_err(|_| bad("CMYK JPEG samples"))?;
    if source.len() != count {
        return Err(bad("CMYK sample extent"));
    }
    if let Some(owner) = reservation.as_mut() {
        owner
            .ensure_bytes(source.capacity() as u64)
            .map_err(crate::DecodeError::Memory)?;
    }
    request.check_cancelled()?;
    let executor = profile
        .create_transform_f32(
            Layout::Rgba,
            &ColorProfile::new_srgb(),
            Layout::Rgb,
            TransformOptions {
                rrrah_cmyk_hybrid: true,
                ..Default::default()
            },
        )
        .map_err(|_| bad("CMYK ICC transform"))?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|error| crate::DecodeError::from(rrrah_core::BufferError::Allocate(error)))?;
    let mut input = [0.0; 1024];
    let mut rgb = [0.0; 768];
    for chunk in source.chunks(1024) {
        request.check_cancelled()?;
        for (values, codes) in input.chunks_exact_mut(4).zip(chunk.chunks_exact(4)) {
            if ycck {
                // JPEG full-range YCbCr, rounded in 16-bit fixed point before
                // normalization. Adobe YCCK stores inverted K separately.
                let y = i32::from(codes[0]);
                let cb = i32::from(codes[1]) - 128;
                let cr = i32::from(codes[2]) - 128;
                let cmy = [
                    y + ((91881 * cr + 32768) >> 16),
                    y + ((-22554 * cb - 46802 * cr + 32768) >> 16),
                    y + ((116130 * cb + 32768) >> 16),
                ];
                for (value, code) in values[..3].iter_mut().zip(cmy) {
                    *value = code.clamp(0, 255) as f32 / 255.0;
                }
                values[3] = f32::from(255 - codes[3]) / 255.0;
            } else {
                for (value, code) in values.iter_mut().zip(codes) {
                    *value = f32::from(255 - *code) / 255.0;
                }
            }
        }
        let n = chunk.len() / 4;
        executor
            .transform(&input[..chunk.len()], &mut rgb[..n * 3])
            .map_err(|_| bad("CMYK ICC samples"))?;
        for color in rgb[..n * 3].chunks_exact(3) {
            output.extend(color.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8));
            output.push(255);
        }
    }
    drop((source, reservation));
    Ok(Some(DynamicImage::ImageRgba8(
        image::RgbaImage::from_raw(width, height, output).ok_or(bad("RGB extent"))?,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rrrah_core::{MemoryBudget, RasterColorSpace, RasterPixels};

    #[test]
    fn conflicting_adobe_transforms_are_refused_before_pixel_allocation() {
        let source = include_bytes!("../../../tests/fixtures/raster/cmyk-app14-profiled.jpg");
        let profile = include_bytes!("../../../vendor/hayro-interpret/assets/CGATS001Compat-v2-micro.icc");
        let mut bytes = source[..2].to_vec();
        // A second Adobe header selects YCCK while the original selects CMYK.
        bytes.extend_from_slice(&[255, 238, 0, 14]);
        bytes.extend_from_slice(b"Adobe");
        bytes.extend_from_slice(&[0, 100, 0, 0, 0, 0, 2]);
        bytes.extend_from_slice(&source[2..]);
        let budget = MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new("unused.jpg");
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            decode_profiled(&bytes, Some(profile), 64, 8, &request),
            Err(RasterDecodeError::InvalidJpegColor(
                "conflicting Adobe APP14 transforms"
            ))
        ));
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn repeated_consistent_adobe_transform_preserves_samples() {
        let source = include_bytes!("../../../tests/fixtures/raster/cmyk-app14-profiled.jpg");
        let profile = include_bytes!("../../../vendor/hayro-interpret/assets/CGATS001Compat-v2-micro.icc");
        let mut bytes = source[..2].to_vec();
        bytes.extend_from_slice(&[255, 238, 0, 14]);
        bytes.extend_from_slice(b"Adobe");
        bytes.extend_from_slice(&[0, 100, 0, 0, 0, 0, 0]);
        bytes.extend_from_slice(&source[2..]);
        let request = DecodeRequest::new("unused.jpg");
        let image = decode_profiled(&bytes, Some(profile), 64, 8, &request)
            .unwrap()
            .unwrap();
        assert_eq!(
            image.as_rgba8().unwrap().as_raw(),
            include_bytes!("../../../tests/fixtures/pdf/cmyk-jpeg-app14-littlecms.rgba")
        );
    }

    #[test]
    fn asymmetric_profiled_jpegs_preserve_all_eight_exif_orientations() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for kind in ["cmyk", "ycck"] {
            for orientation in 1..=8 {
                let name = format!("{kind}-profiled-orientation-{orientation}");
                let budget = MemoryBudget::new(1024 * 1024);
                let mut request = DecodeRequest::new(root.join(format!("{name}.jpg")));
                request.memory_budget = Some(budget.clone());
                let image = crate::decode_raster(&request).unwrap();
                let extent = if orientation >= 5 { (13, 17) } else { (17, 13) };
                assert_eq!((image.width(), image.height()), extent, "{name}");
                let expected = std::fs::read(root.join(format!("{name}.rgba"))).unwrap();
                let RasterPixels::Rgba8(values) = image.pixels() else {
                    panic!("RGBA8 expected")
                };
                assert_eq!(values.len(), expected.len());
                let max_error = values
                    .iter()
                    .zip(&expected)
                    .map(|(a, b)| a.abs_diff(*b))
                    .max()
                    .unwrap();
                assert!(max_error <= 2, "{name}: max error {max_error}");
                drop(image);
                assert_eq!(budget.used(), 0);
            }
        }
    }

    #[test]
    fn cmyk_source_admission_refuses_before_decode_and_releases_memory() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/cmyk-app14-profiled.jpg");
        let profile = include_bytes!("../../../vendor/hayro-interpret/assets/CGATS001Compat-v2-micro.icc");
        let budget = MemoryBudget::new(64 * 8 * 4 - 1);
        let mut request = DecodeRequest::new("unused.jpg");
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            decode_profiled(bytes, Some(profile), 64, 8, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn profiled_cmyk_jpeg_matches_independent_oracle_and_releases_memory() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/cmyk-app14-profiled.jpg");
        let budget = MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        let image = crate::decode_raster(&request).unwrap();
        assert_eq!((image.width(), image.height()), (64, 8));
        assert_eq!(image.color_space(), &RasterColorSpace::Srgb);
        let RasterPixels::Rgba8(values) = image.pixels() else {
            panic!("RGBA8 expected")
        };
        let expected = include_bytes!("../../../tests/fixtures/pdf/cmyk-jpeg-app14-littlecms.rgba");
        assert_eq!(values.len(), expected.len());
        for (actual, reference) in values.chunks_exact(4).zip(expected.chunks_exact(4)) {
            for channel in 0..3 {
                assert!(actual[channel].abs_diff(reference[channel]) <= 1);
            }
            assert_eq!(actual[3], reference[3]);
        }
        let prepared = crate::prepare_raster_for_display_with_budget(&image, Some(&budget)).unwrap();
        assert!(matches!(prepared.pixels(), RasterPixels::Rgba32Float(_)));
        drop(prepared);
        drop(image);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn profiled_ycck_jpeg_matches_independent_oracle_and_releases_memory() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/ycck-app14-profiled.jpg");
        let budget = MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        let image = crate::decode_raster(&request).unwrap();
        assert_eq!((image.width(), image.height()), (64, 8));
        assert_eq!(image.color_space(), &RasterColorSpace::Srgb);
        let RasterPixels::Rgba8(values) = image.pixels() else {
            panic!("RGBA8 expected")
        };
        let expected = include_bytes!("../../../tests/fixtures/pdf/ycck-jpeg-app14-littlecms.rgba");
        assert_eq!(values.len(), expected.len());
        for (actual, reference) in values.chunks_exact(4).zip(expected.chunks_exact(4)) {
            for channel in 0..3 {
                assert!(actual[channel].abs_diff(reference[channel]) <= 1);
            }
            assert_eq!(actual[3], reference[3]);
        }
        let prepared = crate::prepare_raster_for_display_with_budget(&image, Some(&budget)).unwrap();
        assert!(matches!(prepared.pixels(), RasterPixels::Rgba32Float(_)));
        drop(prepared);
        drop(image);
        assert_eq!(budget.used(), 0);
    }
}
