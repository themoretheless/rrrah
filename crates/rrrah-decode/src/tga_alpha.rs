//! Bounded TGA2 footer/extension interpretation and encoded-byte alpha semantics.
use crate::{DecodeRequest, raster::RasterDecodeError};
fn bad(reason: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidTgaAlpha(reason)
}
#[derive(Clone, Copy)]
enum Alpha {
    Keep,
    Opaque,
    Associated,
}
#[derive(Clone, Copy)]
pub(super) struct Metadata {
    pub(super) pixel_end: usize,
    pub(super) pixel_aspect: Option<f32>,
    alpha: Alpha,
    useful: bool,
}
impl Metadata {
    pub(super) fn parse(bytes: &[u8], request: &DecodeRequest) -> Result<Self, RasterDecodeError> {
        request.check_cancelled()?;
        let mut result = Self {
            pixel_end: bytes.len(),
            pixel_aspect: None,
            alpha: Alpha::Keep,
            useful: false,
        };
        if !bytes.ends_with(b"TRUEVISION-XFILE.\0") {
            return Ok(result);
        }
        let footer = bytes
            .len()
            .checked_sub(26)
            .ok_or_else(|| bad("truncated footer"))?;
        if footer < 18 {
            return Err(bad("footer overlaps image header"));
        }
        result.pixel_end = footer;
        let extension = u32::from_le_bytes(bytes[footer..footer + 4].try_into().unwrap()) as usize;
        let developer = u32::from_le_bytes(bytes[footer + 4..footer + 8].try_into().unwrap()) as usize;
        if developer != 0 {
            if developer < 18 || developer >= footer {
                return Err(bad("developer directory offset"));
            }
            result.pixel_end = result.pixel_end.min(developer);
        }
        if extension != 0 {
            if extension < 18 || extension.checked_add(2).is_none_or(|end| end > footer) {
                return Err(bad("extension offset"));
            }
            let size = u16::from_le_bytes(bytes[extension..extension + 2].try_into().unwrap()) as usize;
            if size < 495 || extension.checked_add(size).is_none_or(|end| end > footer) {
                return Err(bad("extension extent"));
            }
            if developer != 0 && developer >= extension && developer < extension + size {
                return Err(bad("overlapping extension/developer areas"));
            }
            let numerator = u16::from_le_bytes(bytes[extension + 474..extension + 476].try_into().unwrap());
            let denominator = u16::from_le_bytes(bytes[extension + 476..extension + 478].try_into().unwrap());
            if denominator != 0 {
                if numerator == 0 {
                    return Err(bad("zero pixel aspect numerator"));
                }
                result.pixel_aspect = Some(f32::from(numerator) / f32::from(denominator));
            }
            let mode = bytes[extension + 494];
            result.alpha = match mode {
                0 | 1 => Alpha::Opaque,
                2 | 3 => Alpha::Keep,
                4 => Alpha::Associated,
                _ => return Err(bad("reserved or unassigned Attributes Type")),
            };
            result.useful = mode == 3 || mode == 4;
            result.pixel_end = result.pixel_end.min(extension);
        }
        let map = if bytes[1] == 1 {
            usize::from(u16::from_le_bytes([bytes[5], bytes[6]])) * usize::from(bytes[7]).div_ceil(8)
        } else {
            0
        };
        let minimum = 18usize
            .checked_add(usize::from(bytes[0]))
            .and_then(|v| v.checked_add(map))
            .ok_or_else(|| bad("pixel prefix overflow"))?;
        if result.pixel_end < minimum {
            return Err(bad("metadata overlaps ID or color map"));
        }
        if matches!(bytes[2], 1 | 2 | 3 | 9 | 10 | 11) {
            let count = u64::from(u16::from_le_bytes([bytes[12], bytes[13]]))
                * u64::from(u16::from_le_bytes([bytes[14], bytes[15]]));
            let sample = u64::from(bytes[16]).div_ceil(8);
            let required = if bytes[2] >= 9 {
                count.div_ceil(128) * (sample + 1)
            } else {
                count * sample
            };
            if required > (result.pixel_end - minimum) as u64 {
                return Err(bad("pixel payload overlaps metadata"));
            }
        }
        Ok(result)
    }
    pub(super) fn validate_color(self, color: image::ColorType) -> Result<(), RasterDecodeError> {
        if self.useful && !matches!(color, image::ColorType::Rgba8 | image::ColorType::La8) {
            return Err(bad("declared useful alpha is not exposed by pixel layout"));
        }
        Ok(())
    }
    pub(super) fn apply(self, pixels: &mut [u8], request: &DecodeRequest) -> Result<(), RasterDecodeError> {
        request.check_cancelled()?;
        if matches!(self.alpha, Alpha::Keep) {
            return Ok(());
        }
        for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
            if index % 4096 == 0 {
                request.check_cancelled()?;
            }
            match self.alpha {
                Alpha::Opaque => pixel[3] = 255,
                Alpha::Associated => {
                    let alpha = u32::from(pixel[3]);
                    for channel in &mut pixel[..3] {
                        if u32::from(*channel) > alpha {
                            return Err(bad("associated RGB exceeds alpha"));
                        }
                        *channel = if alpha == 0 {
                            0
                        } else {
                            ((u32::from(*channel) * 255 + alpha / 2) / alpha) as u8
                        };
                    }
                }
                Alpha::Keep => unreachable!(),
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use crate::{DecodeRequest, decode_raster};
    use rrrah_core::{MemoryBudget, RasterPixels};
    #[test]
    fn authored_tga2_alpha_types_preserve_samples_and_apply_defined_semantics() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/raster/tga2-alpha-reference.json"
        ))
        .unwrap();
        let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for row in reference["rows"].as_array().unwrap() {
            let name = row["name"].as_str().unwrap();
            let root = MemoryBudget::new(1024 * 1024);
            let mut request = DecodeRequest::new(fixtures.join(name));
            request.memory_budget = Some(root.clone());
            let frame = decode_raster(&request).unwrap();
            let RasterPixels::Rgba8(p) = frame.pixels() else {
                panic!()
            };
            assert_eq!(
                &p[..],
                std::fs::read(fixtures.join(row["rgba8_file"].as_str().unwrap())).unwrap(),
                "{name}"
            );
            drop(frame);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn footer_bounds_refuse_corrupt_fields_before_any_pixel_reservation() {
        use super::*;
        let source = include_bytes!("../../../tests/fixtures/raster/tga2-alpha-mode4-origin0-raw.tga");
        let footer = source.len() - 26;
        let ext = u32::from_le_bytes(source[footer..footer + 4].try_into().unwrap()) as usize;
        let mut request = DecodeRequest::new("control.tga");
        let zero = MemoryBudget::new(0);
        request.memory_budget = Some(zero.clone());
        for offset in [1u32, 17, u32::MAX] {
            let mut b = source.to_vec();
            b[footer..footer + 4].copy_from_slice(&offset.to_le_bytes());
            assert!(Metadata::parse(&b, &request).is_err());
        }
        for size in [0u16, 494, 496, u16::MAX] {
            let mut b = source.to_vec();
            b[ext..ext + 2].copy_from_slice(&size.to_le_bytes());
            assert!(Metadata::parse(&b, &request).is_err());
        }
        for mode in [5, 127, 128, 255] {
            let mut b = source.to_vec();
            b[ext + 494] = mode;
            assert!(Metadata::parse(&b, &request).is_err());
        }
        let mut b = source.to_vec();
        b[footer + 4..footer + 8].copy_from_slice(&((ext + 2) as u32).to_le_bytes());
        assert!(Metadata::parse(&b, &request).is_err());
        let mut b = source.to_vec();
        b.drain(ext - 4..ext);
        let footer = b.len() - 26;
        b[footer..footer + 4].copy_from_slice(&((ext - 4) as u32).to_le_bytes());
        assert!(matches!(
            Metadata::parse(&b, &request),
            Err(RasterDecodeError::InvalidTgaAlpha(
                "pixel payload overlaps metadata"
            ))
        ));
        let mut future = source.to_vec();
        future.insert(source.len() - 26, 0);
        future[ext..ext + 2].copy_from_slice(&496u16.to_le_bytes());
        assert!(Metadata::parse(&future, &request).is_ok());
        assert_eq!(zero.peak(), 0);
    }
    #[test]
    fn alpha_arithmetic_refuses_invalid_association_and_cancellation_without_allocating() {
        use super::*;
        let metadata = Metadata {
            pixel_end: 0,
            pixel_aspect: None,
            alpha: Alpha::Associated,
            useful: true,
        };
        let zero = MemoryBudget::new(0);
        let mut request = DecodeRequest::new("control.tga");
        request.memory_budget = Some(zero.clone());
        for mut pixel in [[1, 0, 0, 0], [128, 0, 0, 127]] {
            assert!(metadata.apply(&mut pixel, &request).is_err());
        }
        assert!(metadata.validate_color(image::ColorType::Rgb8).is_err());
        assert!(metadata.validate_color(image::ColorType::Rgba8).is_ok());
        request.cancellation = Some(crate::GenerationToken::new(
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
            0,
        ));
        let mut pixel = [16, 32, 64, 128];
        assert!(matches!(
            metadata.apply(&mut pixel, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
        assert_eq!(pixel, [16, 32, 64, 128]);
        assert_eq!(zero.peak(), 0);
    }
    #[test]
    fn tga2_pixel_aspect_matches_independent_ratios_without_changing_samples() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/raster/tga2-aspect-reference.json"
        ))
        .unwrap();
        let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for row in reference["rows"].as_array().unwrap() {
            let name = row["name"].as_str().unwrap();
            let root = MemoryBudget::new(1024 * 1024);
            let mut request = DecodeRequest::new(fixtures.join(name));
            request.memory_budget = Some(root.clone());
            request.assume_untagged_srgb = true;
            let expected = row["pixel_aspect"].as_f64().map(|v| v as f32);
            let frame = decode_raster(&request).unwrap();
            assert_eq!(frame.pixel_aspect(), expected, "{name}");
            let RasterPixels::Rgba8(p) = frame.pixels() else {
                panic!()
            };
            assert_eq!(
                &p[..],
                std::fs::read(fixtures.join(row["rgba8_file"].as_str().unwrap())).unwrap()
            );
            let prepared = crate::prepare_raster_for_display_with_budget(&frame, Some(&root)).unwrap();
            assert_eq!(prepared.pixel_aspect(), expected);
            drop((prepared, frame));
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn aspect_rational_boundaries_are_finite_and_denominator_zero_is_unspecified() {
        use super::*;
        let source = include_bytes!("../../../tests/fixtures/raster/tga2-alpha-mode3-origin0-raw.tga");
        let footer = source.len() - 26;
        let extension = u32::from_le_bytes(source[footer..footer + 4].try_into().unwrap()) as usize;
        let zero = MemoryBudget::new(0);
        let mut request = DecodeRequest::new("aspect.tga");
        request.memory_budget = Some(zero.clone());
        for (n, d) in [(1u16, 65535u16), (65535, 1), (65535, 65535), (0, 0), (65535, 0)] {
            let mut bytes = source.to_vec();
            bytes[extension + 474..extension + 476].copy_from_slice(&n.to_le_bytes());
            bytes[extension + 476..extension + 478].copy_from_slice(&d.to_le_bytes());
            let actual = Metadata::parse(&bytes, &request).unwrap().pixel_aspect;
            let expected = (d != 0).then(|| (f64::from(n) / f64::from(d)) as f32);
            assert_eq!(actual.map(f32::to_bits), expected.map(f32::to_bits));
        }
        for d in [1u16, 65535] {
            let mut bytes = source.to_vec();
            bytes[extension + 474..extension + 476].copy_from_slice(&0u16.to_le_bytes());
            bytes[extension + 476..extension + 478].copy_from_slice(&d.to_le_bytes());
            assert!(matches!(
                Metadata::parse(&bytes, &request),
                Err(RasterDecodeError::InvalidTgaAlpha("zero pixel aspect numerator"))
            ));
        }
        request.cancellation = Some(crate::GenerationToken::new(
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
            0,
        ));
        assert!(matches!(
            Metadata::parse(source, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
        assert_eq!(zero.peak(), 0);
    }
}
