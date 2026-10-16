//! Borrowed classic RGB palette tables; indexed source expands directly to output.
use super::{DecodeRequest, RasterDecodeError, bad, rgb, word};
use rrrah_core::{DecodedRaster, RasterPixels};
#[path = "dicom_segments.rs"]
mod segments;
type PaletteDescriptor<'a> = (Option<&'a [u8]>, &'a [u8]);
#[derive(Default)]
pub(super) struct Palette<'a> {
    descriptors: [Option<PaletteDescriptor<'a>>; 3],
    data: [Option<&'a [u8]>; 3],
    segmented: [Option<&'a [u8]>; 3],
}
pub(super) struct Input<'a> {
    pub width: u32,
    pub height: u32,
    pub frames: usize,
    pub pixel_frames: usize,
    pub pixel_index: usize,
    pub bits: u16,
    pub stored: u16,
    pub signed: u16,
    pub pixels: &'a [u8],
    pub little: bool,
    pub word_vr: bool,
    pub icc: Option<&'a [u8]>,
    pub color_label: Option<&'a str>,
    pub aspect: Option<f32>,
}
impl<'a> Palette<'a> {
    pub(super) fn present(&self) -> bool {
        self.descriptors.iter().any(Option::is_some)
            || self.data.iter().any(Option::is_some)
            || self.segmented.iter().any(Option::is_some)
    }
    pub(super) fn record(
        &mut self,
        tag: u32,
        vr: &'a [u8],
        value: &'a [u8],
        implicit: bool,
    ) -> Result<(), RasterDecodeError> {
        match tag {
            0x00281101..=0x00281103 => {
                self.descriptors[(tag - 0x00281101) as usize] =
                    Some((if implicit { None } else { Some(vr) }, value))
            }
            0x00281201..=0x00281203 => {
                if vr != b"OW" {
                    return Err(bad("palette data requires OW"));
                }
                self.data[(tag - 0x00281201) as usize] = Some(value);
            }
            0x00281221..=0x00281223 => {
                if vr != b"OW" {
                    return Err(bad("segmented palette requires OW"));
                }
                self.segmented[(tag - 0x00281221) as usize] = Some(value);
            }
            _ => return Err(bad("alpha palette not qualified")),
        }
        Ok(())
    }
    pub(super) fn output_bytes(&self, little: bool) -> Result<u64, RasterDecodeError> {
        let (_, value) = self.descriptors[0].ok_or_else(|| bad("missing palette descriptor"))?;
        if value.len() != 6 {
            return Err(bad("palette descriptor VR/value"));
        }
        match word(value[4..6].try_into().unwrap(), little) {
            8 => Ok(4),
            16 => Ok(8),
            _ => Err(bad("palette depth")),
        }
    }
    pub(super) fn decode(
        &self,
        input: Input<'_>,
        request: &DecodeRequest,
    ) -> Result<DecodedRaster, RasterDecodeError> {
        let expected_vr = if input.signed == 1 { b"SS" } else { b"US" };
        let mut descriptors = [[0i64; 3]; 3];
        for (out, desc) in descriptors.iter_mut().zip(self.descriptors) {
            let (vr, value) = desc.ok_or_else(|| bad("missing palette descriptor"))?;
            if value.len() != 6 || vr.is_some_and(|vr| vr != expected_vr) {
                return Err(bad("palette descriptor VR/value"));
            }
            let n = word(value[..2].try_into().unwrap(), input.little);
            let first = word(value[2..4].try_into().unwrap(), input.little);
            *out = [
                if n == 0 { 65536 } else { i64::from(n) },
                if input.signed == 1 {
                    i64::from(first as i16)
                } else {
                    i64::from(first)
                },
                i64::from(word(value[4..6].try_into().unwrap(), input.little)),
            ];
        }
        if descriptors[0] != descriptors[1]
            || descriptors[0] != descriptors[2]
            || !matches!(descriptors[0][2], 8 | 16)
        {
            return Err(bad("inconsistent palette descriptors"));
        }
        let [entries, first, depth] = descriptors[0];
        let entries = entries as usize;
        let expanded;
        let data = if self.segmented.iter().any(Option::is_some) {
            if depth != 16 {
                return Err(bad("8-bit segmented palette encoding not qualified"));
            }
            if self.data.iter().any(Option::is_some) {
                return Err(bad("mixed direct/segmented palettes"));
            }
            let [r, g, b] = self.segmented;
            expanded = [
                segments::expand(
                    r.ok_or_else(|| bad("missing segmented red palette"))?,
                    entries,
                    input.little,
                    request,
                )?,
                segments::expand(
                    g.ok_or_else(|| bad("missing segmented green palette"))?,
                    entries,
                    input.little,
                    request,
                )?,
                segments::expand(
                    b.ok_or_else(|| bad("missing segmented blue palette"))?,
                    entries,
                    input.little,
                    request,
                )?,
            ];
            expanded.each_ref().map(|v| Some(v.as_slice()))
        } else {
            self.data
        };
        let [red, green, blue] = data;
        let tables = [
            red.ok_or_else(|| bad("missing red palette"))?,
            green.ok_or_else(|| bad("missing green palette"))?,
            blue.ok_or_else(|| bad("missing blue palette"))?,
        ];
        let mut padded_words = [false; 3];
        for (c, table) in tables.iter().enumerate() {
            let packed = entries + entries % 2;
            if depth == 16 {
                if table.len() != entries * 2 {
                    return Err(bad("palette16 length"));
                }
            } else if table.len() == packed {
                if entries % 2 != 0 && table[if input.little { entries } else { entries ^ 1 }] != 0 {
                    return Err(bad("palette8 padding"));
                }
            } else if table.len() == entries * 2 {
                padded_words[c] = true;
                for (i, v) in table.chunks_exact(2).enumerate() {
                    if i % 4096 == 0 {
                        request.check_cancelled()?;
                    }
                    if word(v.try_into().unwrap(), input.little) > 255 {
                        return Err(bad("palette8 nonzero high bits"));
                    }
                }
            } else {
                return Err(bad("palette8 length"));
            }
        }
        let count = (input.width as usize)
            .checked_mul(input.height as usize)
            .ok_or_else(|| bad("palette dimensions overflow"))?;
        let frame_bytes = count
            .checked_mul(usize::from(input.bits / 8))
            .ok_or_else(|| bad("palette frame overflow"))?;
        let bytes = frame_bytes
            .checked_mul(input.pixel_frames)
            .ok_or_else(|| bad("palette source overflow"))?;
        if input.pixels.len() != bytes + bytes % 2 {
            return Err(bad("palette pixel length"));
        }
        if count as u64 * 4 * (depth as u64 / 8) > crate::raster::MAX_RASTER_BYTES {
            return Err(RasterDecodeError::OutputTooLarge);
        }
        let mask = (1u32 << input.stored) - 1;
        let offset = input.pixel_index * frame_bytes;
        let index = |i: usize| {
            let at = offset + i * usize::from(input.bits / 8);
            let code = (if input.bits == 8 {
                u32::from(
                    input.pixels[if !input.little && input.word_vr {
                        at ^ 1
                    } else {
                        at
                    }],
                )
            } else {
                u32::from(word(input.pixels[at..at + 2].try_into().unwrap(), input.little))
            }) & mask;
            let value = if input.signed == 1 && code & (1 << (input.stored - 1)) != 0 {
                i64::from(code) - (1i64 << input.stored)
            } else {
                i64::from(code)
            };
            (value - first).clamp(0, entries as i64 - 1) as usize
        };
        let (color, credit) = rgb::color(input.icc, input.color_label, request)?;
        let pixels = if depth == 8 {
            let mut out = rgb::buffer(count * 4, 255u8, request)?;
            for (i, p) in out.get_mut().unwrap().chunks_exact_mut(4).enumerate() {
                if i % 4096 == 0 {
                    request.check_cancelled()?;
                }
                let at = index(i);
                for c in 0..3 {
                    p[c] = if padded_words[c] {
                        word(tables[c][at * 2..at * 2 + 2].try_into().unwrap(), input.little) as u8
                    } else {
                        tables[c][if input.little { at } else { at ^ 1 }]
                    };
                }
            }
            RasterPixels::Rgba8(out)
        } else {
            let mut out = rgb::buffer(count * 4, u16::MAX, request)?;
            for (i, p) in out.get_mut().unwrap().chunks_exact_mut(4).enumerate() {
                if i % 4096 == 0 {
                    request.check_cancelled()?;
                }
                let at = index(i) * 2;
                for c in 0..3 {
                    p[c] = word(tables[c][at..at + 2].try_into().unwrap(), input.little);
                }
            }
            RasterPixels::Rgba16(out)
        };
        request.check_cancelled()?;
        let mut raster = DecodedRaster::new(input.width, input.height, pixels, color)?
            .with_image_selection(request.image_index, input.frames)?
            .with_pixel_aspect(input.aspect)?;
        if let Some(credit) = credit {
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
    #[test]
    fn mismatched_descriptors_alpha_supplemental_and_bad_legacy_words_refuse() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let original = std::fs::read(root.join("dicom-palette-explicit-legacy8.dcm")).unwrap();
        let budget = rrrah_core::MemoryBudget::new(4096);
        let mut request = DecodeRequest::new("palette.dcm");
        request.memory_budget = Some(budget.clone());
        let descriptor = original
            .windows(6)
            .position(|v| v == [0x28, 0, 1, 0x11, b'U', b'S'])
            .unwrap();
        for (offset, bytes) in [(8, 5u16.to_le_bytes()), (12, 9u16.to_le_bytes()), (4, *b"SS")] {
            let mut bad = original.clone();
            bad[descriptor + offset..descriptor + offset + 2].copy_from_slice(&bytes);
            assert!(super::super::decode(&bad, &request, None).is_err());
            assert_eq!(budget.used(), 0);
        }
        let table = original
            .windows(6)
            .position(|v| v == [0x28, 0, 1, 0x12, b'O', b'W'])
            .unwrap();
        let mut invalid = original.clone();
        invalid[table + 13] = 1;
        assert!(super::super::decode(&invalid, &request, None).is_err());
        assert_eq!(budget.used(), 0);
        let mut invalid = original.clone();
        invalid[table + 8..table + 12].copy_from_slice(&10u32.to_le_bytes());
        assert!(super::super::decode(&invalid, &request, None).is_err());
        assert_eq!(budget.used(), 0);
        let blue = original
            .windows(6)
            .position(|v| v == [0x28, 0, 3, 0x11, b'U', b'S'])
            .unwrap();
        let mut alpha = original[blue..blue + 14].to_vec();
        alpha[2] = 4;
        let mut invalid = original.clone();
        invalid.splice(blue + 14..blue + 14, alpha);
        assert!(matches!(
            super::super::decode(&invalid, &request, None),
            Err(RasterDecodeError::InvalidDicom("alpha palette not qualified"))
        ));
        assert_eq!(budget.used(), 0);
        let photo = original.windows(13).position(|v| v == b"PALETTE COLOR").unwrap();
        let mut invalid = original;
        invalid[photo..photo + 13].copy_from_slice(b"MONOCHROME2  ");
        assert!(matches!(
            super::super::decode(&invalid, &request, None),
            Err(RasterDecodeError::InvalidDicom(
                "supplemental palette requires qualified interpretation"
            ))
        ));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn malformed_segmented_count_is_format_error_under_full_source_pressure() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/dicom-segmented-explicit-linear.dcm");
        let mut bytes = std::fs::read(path).unwrap();
        for c in 1..=3 {
            let tag = [0x28, 0, c, 0x11, b'U', b'S'];
            let at = bytes.windows(6).position(|v| v == tag).unwrap();
            bytes[at + 8..at + 10].fill(0); // claims65536 entries, actual segmented stream has5
        }
        let budget = rrrah_core::MemoryBudget::new(1048576);
        let mut source = budget.try_buffer(bytes.len(), 0u8).unwrap();
        source.copy_from_slice(&bytes);
        let pressure = budget.try_reserve(budget.available_bytes() - 1).unwrap();
        let mut request = DecodeRequest::new("malformed.dcm");
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            super::super::decode(&source, &request, None),
            Err(RasterDecodeError::InvalidDicom(
                "segmented palette descriptor count mismatch"
            ))
        ));
        assert_eq!(budget.peak(), 1048576);
        assert_eq!(budget.used(), 1048575);
        drop(source);
        drop(pressure);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    #[ignore = "external packaged ultrasound palette source and independent pydicom LUT oracle"]
    fn packaged_palette_matches_all_samples_with_exact_managed_admission() {
        let source =
            std::path::PathBuf::from(std::env::var_os("RRRAH_DICOM_PALETTE_SOURCE").expect("source"));
        let oracle =
            std::path::PathBuf::from(std::env::var_os("RRRAH_DICOM_PALETTE_ORACLE").expect("oracle"));
        let input = std::fs::metadata(&source).unwrap().len();
        let output = 800 * 350 * 8;
        let budget = rrrah_core::MemoryBudget::new(input + output);
        let mut request = DecodeRequest::new(source);
        request.memory_budget = Some(budget.clone());
        let read = |request: &DecodeRequest| {
            let b = crate::bounded_io::read_bounded(request)?;
            super::super::decode(&b, request, None)
        };
        let frame: DecodedRaster = read(&request).unwrap();
        assert_eq!((frame.width(), frame.height()), (800, 350));
        let RasterPixels::Rgba16(values) = frame.pixels() else {
            panic!("palette precision lost")
        };
        let actual: Vec<_> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        assert_eq!(actual, std::fs::read(oracle).unwrap());
        assert_eq!(budget.used(), output);
        drop(frame);
        assert_eq!(budget.used(), 0);
        assert_eq!(budget.peak(), input + output);
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(input + output - 1));
        assert!(read(&request).is_err());
        assert_eq!(request.memory_budget.unwrap().used(), 0);
    }
}
