//! Native uncompressed full-width unsigned RGB, planar or interleaved.
use super::{DecodeRequest, RasterDecodeError, bad, word};
use rrrah_core::{DecodedRaster, PixelBuffer, RasterColorSpace, RasterPixels};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Encoding {
    Rgb,
    YbrFull,
    Ybr422,
}
// Exact BT.601 full-range inverse coefficients, round-half-up then clamp.
fn ybr_rgb([y, cb, cr]: [u8; 3]) -> [u8; 3] {
    let (y, cb, cr) = (i32::from(y), i32::from(cb) - 128, i32::from(cr) - 128);
    [
        ((500 * y + 701 * cr + 250) / 500).clamp(0, 255) as u8,
        ((293500 * y - 101004 * cb - 209599 * cr + 146750) / 293500).clamp(0, 255) as u8,
        ((250 * y + 443 * cb + 125) / 250).clamp(0, 255) as u8,
    ]
}
pub(super) struct Rgb<'a> {
    pub encoding: Encoding,
    pub width: u32,
    pub height: u32,
    pub frames: usize,
    pub pixel_frames: usize,
    pub pixel_index: usize,
    pub bits: u16,
    pub planar: u16,
    pub pixels: &'a [u8],
    pub word_vr: bool,
    pub little: bool,
    pub icc: Option<&'a [u8]>,
    pub color_label: Option<&'a str>,
}
pub(super) fn buffer<T: Copy + Send + Sync + 'static>(
    count: usize,
    value: T,
    request: &DecodeRequest,
) -> Result<PixelBuffer<T>, RasterDecodeError> {
    Ok(match &request.memory_budget {
        Some(budget) => budget
            .try_buffer(count, value)
            .map_err(crate::DecodeError::Memory)?
            .freeze()
            .into(),
        None => Arc::new(vec![value; count]).into(),
    })
}
pub(super) fn color(
    icc: Option<&[u8]>,
    color_label: Option<&str>,
    request: &DecodeRequest,
) -> Result<(RasterColorSpace, Option<rrrah_core::Reservation>), RasterDecodeError> {
    let profile = if let Some(icc) = icc {
        if icc.len() < 132
            || icc.len() > 16 * 1024 * 1024
            || &icc[36..40] != b"acsp"
            || &icc[16..20] != b"RGB "
        {
            return Err(bad("RGB ICC header"));
        }
        let length = u32::from_be_bytes(icc[..4].try_into().unwrap()) as usize;
        if length < 132
            || !(icc.len() == length || (length % 2 == 1 && icc.len() == length + 1 && icc[length] == 0))
        {
            return Err(bad("RGB ICC length"));
        }
        Some(&icc[..length])
    } else {
        None
    };
    let profile_credit = profile
        .and(request.memory_budget.as_ref())
        .map(|b| b.try_reserve(profile.unwrap().len() as u64))
        .transpose()
        .map_err(crate::DecodeError::Memory)?;
    let color = if let Some(icc) = profile {
        RasterColorSpace::Icc(icc.to_vec())
    } else if color_label == Some("SRGB") {
        RasterColorSpace::Srgb
    } else if color_label.is_some() {
        return Err(bad("RGB color label requires an ICC profile"));
    } else if request.assume_untagged_srgb {
        RasterColorSpace::AssumedSrgb
    } else {
        RasterColorSpace::Unspecified
    };
    Ok((color, profile_credit))
}
impl Rgb<'_> {
    pub(super) fn decode(self, request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
        let count = (self.width as usize)
            .checked_mul(self.height as usize)
            .ok_or_else(|| bad("RGB dimensions overflow"))?;
        let frame_samples = count
            .checked_mul(if self.encoding == Encoding::Ybr422 { 2 } else { 3 })
            .ok_or_else(|| bad("RGB sample length overflow"))?;
        let bytes = frame_samples
            .checked_mul(self.pixel_frames)
            .and_then(|n| n.checked_mul(usize::from(self.bits / 8)))
            .ok_or_else(|| bad("RGB source length overflow"))?;
        if self.pixels.len()
            != bytes
                .checked_add(bytes % 2)
                .ok_or_else(|| bad("RGB padding overflow"))?
        {
            return Err(bad("RGB pixel data length"));
        }
        let output_bytes = (count as u64) * 4 * u64::from(self.bits / 8);
        if output_bytes > crate::raster::MAX_RASTER_BYTES {
            return Err(RasterDecodeError::OutputTooLarge);
        }
        let (color, profile_credit) = color(self.icc, self.color_label, request)?;
        let first = self.pixel_index * frame_samples;
        let source = |pixel: usize, channel: usize| {
            first
                + if self.planar == 0 {
                    pixel * 3 + channel
                } else {
                    channel * count + pixel
                }
        };
        let pixels = if self.bits == 8 {
            let mut output = buffer(count * 4, 255u8, request)?;
            for (i, pixel) in output.get_mut().unwrap().chunks_exact_mut(4).enumerate() {
                if i % 4096 == 0 {
                    request.check_cancelled()?;
                }
                let byte = |at: usize| self.pixels[if !self.little && self.word_vr { at ^ 1 } else { at }];
                let values = if self.encoding == Encoding::Ybr422 {
                    let pair = first + (i / 2) * 4;
                    [byte(pair + i % 2), byte(pair + 2), byte(pair + 3)]
                } else {
                    [byte(source(i, 0)), byte(source(i, 1)), byte(source(i, 2))]
                };
                pixel[..3].copy_from_slice(&if self.encoding == Encoding::Rgb {
                    values
                } else {
                    ybr_rgb(values)
                });
            }
            RasterPixels::Rgba8(output)
        } else {
            let mut output = buffer(count * 4, u16::MAX, request)?;
            for (i, pixel) in output.get_mut().unwrap().chunks_exact_mut(4).enumerate() {
                if i % 4096 == 0 {
                    request.check_cancelled()?;
                }
                for (c, sample) in pixel[..3].iter_mut().enumerate() {
                    let at = source(i, c) * 2;
                    *sample = word(self.pixels[at..at + 2].try_into().unwrap(), self.little);
                }
            }
            RasterPixels::Rgba16(output)
        };
        request.check_cancelled()?;
        let mut raster = DecodedRaster::new(self.width, self.height, pixels, color)?
            .with_image_selection(request.image_index, self.frames)?;
        if let Some(credit) = profile_credit {
            raster = raster
                .with_color_profile_reservation(credit)
                .map_err(crate::DecodeError::Memory)?;
        }
        Ok(raster)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster")
    }
    fn bytes(frame: &DecodedRaster) -> Vec<u8> {
        match frame.pixels() {
            RasterPixels::Rgba8(v) => v.to_vec(),
            RasterPixels::Rgba16(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            _ => panic!("native integer precision lost"),
        }
    }
    fn read(request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
        let source = crate::bounded_io::read_bounded(request)?;
        super::super::decode(&source, request, None)
    }
    #[test]
    fn independent_planar_interleaved_rgb_words_frames_profiles_and_admission() {
        for line in include_str!("../../../tests/fixtures/raster/dicom-rgb-manifest.tsv")
            .lines()
            .skip(1)
            .chain(
                include_str!("../../../tests/fixtures/raster/dicom-ybr-manifest.tsv")
                    .lines()
                    .skip(1),
            )
            .chain(
                include_str!("../../../tests/fixtures/raster/dicom-geometry-color-manifest.tsv")
                    .lines()
                    .skip(1),
            )
            .chain(
                include_str!("../../../tests/fixtures/raster/dicom-palette-manifest.tsv")
                    .lines()
                    .skip(1),
            )
            .chain(
                include_str!("../../../tests/fixtures/raster/dicom-segmented-manifest.tsv")
                    .lines()
                    .skip(1),
            )
            .chain(
                include_str!("../../../tests/fixtures/raster/dicom-rle-color-manifest.tsv")
                    .lines()
                    .skip(1),
            )
        {
            let f: Vec<_> = line.split('\t').collect();
            let width = f[1].parse::<u32>().unwrap();
            let height = f[2].parse::<u32>().unwrap();
            let frames = f[3].parse::<usize>().unwrap();
            let bits = f[4].parse::<u64>().unwrap();
            let profile = f[5].parse::<u64>().unwrap();
            let path = root().join(f[0]);
            let input = std::fs::metadata(&path).unwrap().len();
            let output = u64::from(width) * u64::from(height) * 4 * (bits / 8);
            let scratch = f.get(8).map(|v| v.parse::<u64>().unwrap()).unwrap_or(0);
            let budget = rrrah_core::MemoryBudget::new(input + output + profile + scratch);
            let mut request = DecodeRequest::new(path);
            request.memory_budget = Some(budget.clone());
            for index in 0..frames {
                request.image_index = index;
                let frame = read(&request).unwrap();
                assert_eq!(
                    frame.pixel_aspect(),
                    f.get(7).filter(|v| **v != "-").map(|v| v.parse::<f32>().unwrap())
                );
                assert_eq!(
                    (
                        frame.width(),
                        frame.height(),
                        frame.image_index(),
                        frame.image_count()
                    ),
                    (width, height, index, frames)
                );
                assert_eq!(
                    bytes(&frame),
                    std::fs::read(root().join(format!("{}.{}.rgba{}", f[0], index, bits))).unwrap()
                );
                match frame.color_space() {
                    RasterColorSpace::Icc(p) => {
                        assert_eq!(p.len() as u64, profile);
                        assert_eq!(&p[36..40], b"acsp");
                        assert_eq!(p, &std::fs::read(root().join("dicom-rgb-profile.icc")).unwrap());
                    }
                    RasterColorSpace::Srgb => {
                        assert_eq!(f[6], "SRGB");
                        assert_eq!(profile, 0);
                    }
                    RasterColorSpace::Unspecified => {
                        assert_eq!(f[6], "-");
                        assert_eq!(profile, 0);
                    }
                    _ => panic!("unexpected RGB interpretation"),
                }
                assert_eq!(budget.used(), output + profile);
                let clone = frame.clone();
                drop(frame);
                assert_eq!(budget.used(), output + profile);
                drop(clone);
                assert_eq!(budget.used(), 0);
                assert!(
                    crate::decode_raster_with_window(&request, crate::ScalarWindow::new(0., 255.).unwrap())
                        .is_err()
                );
                assert_eq!(budget.used(), 0);
            }
            assert_eq!(budget.peak(), input + output + profile + scratch);
            request.image_index = frames;
            assert!(read(&request).is_err());
            assert_eq!(budget.used(), 0);
            request.image_index = 0;
            request.memory_budget = Some(rrrah_core::MemoryBudget::new(
                input + output + profile + scratch - 1,
            ));
            assert!(read(&request).is_err());
            assert_eq!(request.memory_budget.as_ref().unwrap().used(), 0);
            let source = std::fs::read(&request.path).unwrap();
            for end in 0..source.len() {
                if source.len() > 16384 && end >= 1024 && end + 32 < source.len() {
                    continue;
                }
                assert!(
                    super::super::decode(&source[..end], &request, None).is_err(),
                    "{} truncation{end}",
                    f[0]
                );
                assert_eq!(request.memory_budget.as_ref().unwrap().used(), 0);
            }
        }
    }
    #[test]
    fn malformed_planar_signed_transform_and_icc_declarations_refuse() {
        let original = std::fs::read(root().join("dicom-rgb-explicit-8-planar0-plain.dcm")).unwrap();
        let budget = rrrah_core::MemoryBudget::new(4096);
        let mut request = DecodeRequest::new("rgb.dcm");
        request.memory_budget = Some(budget.clone());
        for (tag, value) in [(0x0006u16, 2u16), (0x0103, 1), (0x0002, 1), (0x0101, 7)] {
            let pattern = [0x28, 0, tag as u8, (tag >> 8) as u8, b'U', b'S', 2, 0];
            let pos = original.windows(8).position(|v| v == pattern).unwrap();
            let mut invalid = original.clone();
            invalid[pos + 8..pos + 10].copy_from_slice(&value.to_le_bytes());
            assert!(super::super::decode(&invalid, &request, None).is_err());
            assert_eq!(budget.used(), 0);
        }
        let profiled = std::fs::read(root().join("dicom-rgb-explicit-8-planar1-icc.dcm")).unwrap();
        let tag = profiled
            .windows(6)
            .position(|v| v == [0x28, 0, 0, 0x20, b'O', b'B'])
            .unwrap();
        let start = tag + 12;
        for (offset, value) in [(0, 0xff), (16, b'C'), (36, b'x')] {
            let mut invalid = profiled.clone();
            invalid[start + offset] = value;
            assert!(super::super::decode(&invalid, &request, None).is_err());
            assert_eq!(budget.used(), 0);
        }
        let mut wrong_vr = profiled;
        wrong_vr[tag + 4..tag + 6].copy_from_slice(b"OW");
        assert!(super::super::decode(&wrong_vr, &request, None).is_err());
        assert_eq!(budget.used(), 0);
    }
    #[test]
    #[ignore = "external packaged uncompressed RGB sources and independent pydicom oracles"]
    fn packaged_rgb_sources_match_every_native_sample_and_release_memory() {
        let source =
            std::path::PathBuf::from(std::env::var_os("RRRAH_DICOM_RGB_SOURCES").expect("source directory"));
        let oracle =
            std::path::PathBuf::from(std::env::var_os("RRRAH_DICOM_RGB_ORACLES").expect("oracle directory"));
        for line in std::fs::read_to_string(oracle.join("manifest.tsv"))
            .unwrap()
            .lines()
        {
            let f: Vec<_> = line.split('\t').collect();
            let width = f[1].parse::<u32>().unwrap();
            let height = f[2].parse::<u32>().unwrap();
            let path = source.join(f[0]);
            let input = std::fs::metadata(&path).unwrap().len();
            let bits = f.get(3).map(|v| v.parse::<u64>().unwrap()).unwrap_or(8);
            let index = f.get(4).map(|v| v.parse::<usize>().unwrap()).unwrap_or(0);
            let scratch = f.get(5).map(|v| v.parse::<u64>().unwrap()).unwrap_or(0);
            let frames = f.get(7).map(|v| v.parse::<usize>().unwrap()).unwrap_or(1);
            let output = u64::from(width) * u64::from(height) * 4 * (bits / 8);
            let budget = rrrah_core::MemoryBudget::new(input + output + scratch);
            let mut request = DecodeRequest::new(path);
            request.memory_budget = Some(budget.clone());
            request.image_index = index;
            let frame = read(&request).unwrap();
            assert_eq!((frame.width(), frame.height()), (width, height));
            assert_eq!((frame.image_index(), frame.image_count()), (index, frames));
            assert_eq!(
                bytes(&frame),
                std::fs::read(
                    oracle.join(
                        f.get(6)
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| format!("{}.rgba{bits}", f[0]))
                    )
                )
                .unwrap()
            );
            assert_eq!(budget.used(), output);
            drop(frame);
            assert_eq!(budget.used(), 0);
            assert_eq!(budget.peak(), input + output + scratch);
            request.memory_budget = Some(rrrah_core::MemoryBudget::new(input + output + scratch - 1));
            assert!(read(&request).is_err());
            assert_eq!(request.memory_budget.unwrap().used(), 0);
        }
    }
    #[test]
    #[ignore = "full external Fraction oracle for all16M YBR triples"]
    fn exhaustive_ybr_rgb_matches_exact_external_rational_oracle() {
        let path = std::env::var_os("RRRAH_DICOM_YBR_EXHAUSTIVE").expect("oracle path");
        let oracle = std::fs::read(path).unwrap();
        assert_eq!(oracle.len(), 256 * 256 * 256 * 3);
        for (i, expected) in oracle.chunks_exact(3).enumerate() {
            let input = [(i >> 16) as u8, (i >> 8) as u8, i as u8];
            assert_eq!(ybr_rgb(input), expected, "YBR {input:?}");
        }
    }
    #[test]
    fn packed_ybr_rejects_planar_odd_width_and_wrong_storage() {
        let original = std::fs::read(root().join("dicom-ybr-explicit-422-planar0-plain.dcm")).unwrap();
        let budget = rrrah_core::MemoryBudget::new(4096);
        let mut request = DecodeRequest::new("ybr.dcm");
        request.memory_budget = Some(budget.clone());
        for (tag, value) in [(0x0006u16, 1u16), (0x0011, 3), (0x0100, 16)] {
            let pattern = [0x28, 0, tag as u8, (tag >> 8) as u8, b'U', b'S', 2, 0];
            let pos = original.windows(8).position(|v| v == pattern).unwrap();
            let mut invalid = original.clone();
            invalid[pos + 8..pos + 10].copy_from_slice(&value.to_le_bytes());
            assert!(super::super::decode(&invalid, &request, None).is_err());
            assert_eq!(budget.used(), 0);
        }
        assert_eq!(ybr_rgb([19, 78, 178]), [89, 1, 0]);
        assert_eq!(ybr_rgb([0, 0, 0]), [0, 135, 0]);
        assert_eq!(ybr_rgb([255, 128, 128]), [255, 255, 255]);
    }
}
