//! Native single-root DICOM Part 10, Explicit VR Little Endian scalar pixels.
//! No implicit color, VOI LUT, presentation-state or compression fallback.
use crate::{DecodeRequest, RasterDecodeError, ScalarWindow};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{collections::BTreeSet, sync::Arc};
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidDicom(s)
}
pub(crate) fn has_magic(b: &[u8]) -> bool {
    b.get(128..132) == Some(b"DICM")
}
fn text(b: &[u8]) -> Result<&str, RasterDecodeError> {
    std::str::from_utf8(b)
        .map(|s| s.trim_end_matches([' ', '\0']))
        .map_err(|_| bad("invalid text"))
}
fn us(vr: &[u8], b: &[u8]) -> Result<u16, RasterDecodeError> {
    if vr != b"US" || b.len() != 2 {
        return Err(bad("invalid US element"));
    }
    Ok(u16::from_le_bytes(b.try_into().unwrap()))
}
fn ds(vr: &[u8], b: &[u8]) -> Result<f64, RasterDecodeError> {
    if vr != b"DS" || b.len() > 16 {
        return Err(bad("invalid DS element"));
    }
    let x = text(b)?
        .trim()
        .parse::<f64>()
        .map_err(|_| bad("invalid decimal"))?;
    if !x.is_finite() {
        return Err(bad("nonfinite decimal"));
    }
    Ok(x)
}
pub(crate) fn decode(
    b: &[u8],
    request: &DecodeRequest,
    window: Option<ScalarWindow>,
) -> Result<DecodedRaster, RasterDecodeError> {
    if !has_magic(b) {
        return Err(bad("Part 10 preamble required"));
    }
    request.check_cancelled()?;
    let (mut at, mut count) = (132usize, 0usize);
    let mut seen = BTreeSet::new();
    let mut previous = 0u32;
    let mut meta_end = None;
    let mut dataset_started = false;
    let (mut rows, mut columns, mut samples, mut bits, mut stored, mut high, mut signed) =
        (None, None, None, None, None, None, None);
    let (mut photo, mut syntax, mut pixels) = (None, None, None);
    let (mut slope, mut intercept) = (None, None);
    let mut frames = 1usize;
    while at < b.len() {
        count += 1;
        if count > 65536 {
            return Err(bad("element count limit"));
        }
        if count % 32 == 0 {
            request.check_cancelled()?;
        }
        let element_start = at;
        let h = b.get(at..at + 8).ok_or_else(|| bad("truncated element"))?;
        let group = u16::from_le_bytes(h[..2].try_into().unwrap());
        let tag = (u32::from(group) << 16) | u32::from(u16::from_le_bytes(h[2..4].try_into().unwrap()));
        if tag < previous || !seen.insert(tag) {
            return Err(bad("unordered or duplicate tag"));
        }
        previous = tag;
        if group != 2 && !dataset_started {
            if meta_end != Some(element_start)
                || ![0x00020001, 0x00020002, 0x00020003, 0x00020012]
                    .iter()
                    .all(|tag| seen.contains(tag))
            {
                return Err(bad("incomplete/inconsistent Part 10 file meta information"));
            }
            dataset_started = true;
        }
        if group != 2 && syntax != Some("1.2.840.10008.1.2.1") {
            return Err(bad("requires Explicit VR Little Endian transfer syntax"));
        }
        let vr = &h[4..6];
        let long = matches!(
            vr,
            b"OB" | b"OD" | b"OF" | b"OL" | b"OV" | b"OW" | b"SQ" | b"UC" | b"UR" | b"UT" | b"UN"
        );
        let length = if long {
            if h[6..8] != [0, 0] {
                return Err(bad("reserved VR bytes"));
            }
            let n = u32::from_le_bytes(
                b.get(at + 8..at + 12)
                    .ok_or_else(|| bad("truncated length"))?
                    .try_into()
                    .unwrap(),
            );
            at += 12;
            n as usize
        } else {
            if !matches!(
                vr,
                b"AE"
                    | b"AS"
                    | b"AT"
                    | b"CS"
                    | b"DA"
                    | b"DS"
                    | b"DT"
                    | b"FD"
                    | b"FL"
                    | b"IS"
                    | b"LO"
                    | b"LT"
                    | b"PN"
                    | b"SH"
                    | b"SL"
                    | b"SS"
                    | b"ST"
                    | b"TM"
                    | b"UI"
                    | b"UL"
                    | b"US"
            ) {
                return Err(bad("unsupported VR"));
            }
            let n = u16::from_le_bytes(h[6..8].try_into().unwrap());
            at += 8;
            usize::from(n)
        };
        if length == u32::MAX as usize || vr == b"SQ" {
            return Err(bad("sequences/encapsulated data are not qualified"));
        }
        if length % 2 != 0 {
            return Err(bad("odd element length"));
        }
        let end = at.checked_add(length).ok_or_else(|| bad("element overflow"))?;
        let v = b.get(at..end).ok_or_else(|| bad("truncated value"))?;
        at = end;
        match tag {
            0x00020000 => {
                if count != 1 || vr != b"UL" || length != 4 {
                    return Err(bad("file meta group length"));
                }
                let size = u32::from_le_bytes(v.try_into().unwrap()) as usize;
                meta_end = Some(
                    at.checked_add(size)
                        .filter(|end| *end <= b.len())
                        .ok_or_else(|| bad("file meta range"))?,
                );
            }
            0x00020001 => {
                if vr != b"OB" || v != [0, 1] {
                    return Err(bad("file meta version"));
                }
            }
            0x00020002 | 0x00020003 | 0x00020012 => {
                if vr != b"UI"
                    || length > 64
                    || text(v)?.is_empty()
                    || !text(v)?.bytes().all(|c| c.is_ascii_digit() || c == b'.')
                {
                    return Err(bad("file meta UID"));
                }
            }
            0x00020010 => {
                if vr != b"UI" || length > 64 {
                    return Err(bad("invalid transfer syntax"));
                }
                syntax = Some(text(v)?);
            }
            0x00280010 => rows = Some(us(vr, v)?),
            0x00280011 => columns = Some(us(vr, v)?),
            0x00280002 => samples = Some(us(vr, v)?),
            0x00280100 => bits = Some(us(vr, v)?),
            0x00280101 => stored = Some(us(vr, v)?),
            0x00280102 => high = Some(us(vr, v)?),
            0x00280103 => signed = Some(us(vr, v)?),
            0x00280004 => {
                if vr != b"CS" || length > 16 {
                    return Err(bad("photometric VR"));
                }
                photo = Some(text(v)?);
            }
            0x00280008 => {
                if vr != b"IS" || length > 12 {
                    return Err(bad("frame count VR"));
                }
                frames = text(v)?.trim().parse().map_err(|_| bad("frame count"))?;
            }
            0x00281052 => intercept = Some(ds(vr, v)?),
            0x00281053 => slope = Some(ds(vr, v)?),
            0x00280120 | 0x00280121 => return Err(bad("pixel padding needs explicit missing-value policy")),
            0x7fe00010 => {
                if !matches!(vr, b"OB" | b"OW") {
                    return Err(bad("pixel VR"));
                }
                pixels = Some(v);
            }
            0x7fe00008 | 0x7fe00009 => return Err(bad("floating pixel data is not qualified")),
            _ => {}
        }
    }
    let width = u32::from(columns.ok_or_else(|| bad("missing columns"))?);
    let height = u32::from(rows.ok_or_else(|| bad("missing rows"))?);
    let bits = bits.ok_or_else(|| bad("missing bits allocated"))?;
    let stored = stored.ok_or_else(|| bad("missing bits stored"))?;
    let high = high.ok_or_else(|| bad("missing high bit"))?;
    let signed = signed.ok_or_else(|| bad("missing pixel representation"))?;
    let photo = photo.ok_or_else(|| bad("missing photometric interpretation"))?;
    if width == 0 || height == 0 || frames == 0 || frames > 65536 || request.image_index >= frames {
        return Err(bad("dimensions/frame selection"));
    }
    if samples != Some(1) || !matches!(photo, "MONOCHROME1" | "MONOCHROME2") {
        return Err(bad("non-scalar photometric interpretation"));
    }
    if !matches!(bits, 8 | 16) || stored == 0 || stored > bits || high != stored - 1 || signed > 1 {
        return Err(bad("sample bit layout"));
    }
    if slope.is_some() != intercept.is_some() {
        return Err(bad("incomplete rescale declaration"));
    }
    let (slope, intercept) = (slope.unwrap_or(1.), intercept.unwrap_or(0.));
    if slope == 0. {
        return Err(bad("zero rescale slope"));
    }
    let count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| bad("dimensions overflow"))?;
    let bytes = count
        .checked_mul(usize::from(bits / 8))
        .and_then(|n| n.checked_mul(frames))
        .ok_or_else(|| bad("pixel length overflow"))?;
    let pixels = pixels.ok_or_else(|| bad("missing pixel data"))?;
    if pixels.len()
        != bytes
            .checked_add(bytes % 2)
            .ok_or_else(|| bad("padded pixel length overflow"))?
    {
        return Err(bad("pixel data length"));
    }
    if (count as u64) * 16 > crate::raster::MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let mut output = match &request.memory_budget {
        Some(budget) => budget
            .try_buffer(count * 4, 0f32)
            .map(|b| b.freeze().into())
            .map_err(crate::DecodeError::Memory)?,
        None => rrrah_core::PixelBuffer::from(Arc::new(vec![0f32; count * 4])),
    };
    let out = output.get_mut().expect("exclusive scalar output");
    let mask = (1u32 << stored) - 1;
    let first = request.image_index * count * usize::from(bits / 8);
    for (i, p) in out.chunks_exact_mut(4).enumerate() {
        if i % 4096 == 0 {
            request.check_cancelled()?;
        }
        let at = first + i * usize::from(bits / 8);
        let code = if bits == 8 {
            u32::from(pixels[at])
        } else {
            u32::from(u16::from_le_bytes(pixels[at..at + 2].try_into().unwrap()))
        } & mask;
        let sample = if signed == 1 && code & (1 << (stored - 1)) != 0 {
            i64::from(code) - (1i64 << stored)
        } else {
            i64::from(code)
        };
        let physical = (sample as f64) * slope + intercept;
        if !physical.is_finite() || physical.abs() > f64::from(f32::MAX) {
            return Err(bad("rescale overflow"));
        }
        let value = if let Some(w) = window {
            let normalized =
                ((physical - f64::from(w.minimum())) / f64::from(w.maximum() - w.minimum())).clamp(0., 1.);
            (if photo == "MONOCHROME1" {
                1. - normalized
            } else {
                normalized
            }) as f32
        } else {
            physical as f32
        };
        p.copy_from_slice(&[value, value, value, 1.]);
    }
    request.check_cancelled()?;
    Ok(DecodedRaster::new(
        width,
        height,
        RasterPixels::Rgba32Float(output),
        if window.is_some() {
            RasterColorSpace::LinearSrgb
        } else {
            RasterColorSpace::Unspecified
        },
    )?
    .with_image_selection(request.image_index, frames)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster")
    }
    fn oracle(path: &std::path::Path) -> Vec<u32> {
        std::fs::read(path)
            .unwrap()
            .chunks_exact(4)
            .map(|v| u32::from_le_bytes(v.try_into().unwrap()))
            .collect()
    }
    fn assert_oracle(frame: &DecodedRaster, expected: Vec<u32>) {
        let RasterPixels::Rgba32Float(values) = frame.pixels() else {
            panic!()
        };
        assert_eq!(values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), expected);
    }
    #[test]
    fn pydicom_scalar_rescale_window_and_managed_frame_oracles() {
        for line in include_str!("../../../tests/fixtures/raster/dicom-manifest.tsv")
            .lines()
            .skip(1)
        {
            let fields: Vec<_> = line.split('\t').collect();
            let path = root().join(fields[0]);
            let width = fields[1].parse::<u32>().unwrap();
            let height = fields[2].parse::<u32>().unwrap();
            let frames = fields[3].parse::<usize>().unwrap();
            let window = ScalarWindow::new(fields[4].parse().unwrap(), fields[5].parse().unwrap()).unwrap();
            let input = std::fs::metadata(&path).unwrap().len();
            let output = u64::from(width) * u64::from(height) * 16;
            let budget = rrrah_core::MemoryBudget::new(input + output);
            let mut request = DecodeRequest::new(path);
            request.memory_budget = Some(budget.clone());
            assert_eq!(
                crate::image_source_kind(&request).unwrap(),
                crate::ImageSourceKind::Raster
            );
            for index in 0..frames {
                request.image_index = index;
                let frame = crate::decode_raster(&request).unwrap();
                assert_eq!((frame.width(), frame.height()), (width, height));
                assert_eq!((frame.image_index(), frame.image_count()), (index, frames));
                assert_eq!(frame.color_space(), &RasterColorSpace::Unspecified);
                assert!(crate::prepare_raster_for_display(&frame).is_err());
                assert_oracle(
                    &frame,
                    oracle(&root().join(format!("{}.{index}.raw-f32", fields[0]))),
                );
                assert_eq!(budget.used(), output);
                drop(frame);
                assert_eq!(budget.used(), 0);
                let frame = crate::decode_raster_with_window(&request, window).unwrap();
                assert_eq!(frame.color_space(), &RasterColorSpace::LinearSrgb);
                assert_oracle(
                    &frame,
                    oracle(&root().join(format!("{}.{index}.window-f32", fields[0]))),
                );
                assert_eq!(budget.used(), output);
                drop(frame);
                assert_eq!(budget.used(), 0);
            }
            assert_eq!(budget.peak(), input + output);
            request.image_index = frames;
            assert!(crate::decode_raster(&request).is_err());
            request.image_index = 0;
            request.memory_budget = Some(rrrah_core::MemoryBudget::new(input + output - 1));
            assert!(matches!(
                crate::decode_raster(&request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(request.memory_budget.as_ref().unwrap().used(), 0);
        }
    }
    #[test]
    fn malformed_native_elements_transfer_syntax_and_truncations_fail() {
        let bytes = std::fs::read(root().join("dicom-signed12-rescale.dcm")).unwrap();
        let request = DecodeRequest::new("test.dcm");
        for end in 0..bytes.len() {
            assert!(decode(&bytes[..end], &request, None).is_err(), "truncation {end}");
        }
        let syntax = bytes
            .windows(19)
            .position(|v| v == b"1.2.840.10008.1.2.1")
            .unwrap();
        let mut invalid = bytes.clone();
        invalid[syntax + 18] = b'2';
        assert!(decode(&invalid, &request, None).is_err());
        let stored = bytes
            .windows(8)
            .position(|v| v == [0x28, 0, 1, 1, b'U', b'S', 2, 0])
            .unwrap();
        let mut invalid = bytes.clone();
        invalid[stored + 8..stored + 10].copy_from_slice(&17u16.to_le_bytes());
        assert!(decode(&invalid, &request, None).is_err());
        let pixel = bytes
            .windows(6)
            .position(|v| v == [0xe0, 0x7f, 0x10, 0, b'O', b'W'])
            .unwrap();
        let mut invalid = bytes.clone();
        invalid[pixel + 8..pixel + 12].fill(255);
        assert!(decode(&invalid, &request, None).is_err());
        let mut invalid = bytes.clone();
        invalid[140] ^= 1;
        assert!(decode(&invalid, &request, None).is_err()); // meta group length
        let mut cancelled = request;
        cancelled.cancellation = Some(crate::GenerationToken::new(
            Arc::new(std::sync::atomic::AtomicU64::new(2)),
            1,
        ));
        assert!(matches!(
            decode(&bytes, &cancelled, None),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
    }
    #[test]
    fn rescale_overflow_and_unsupported_color_release_admitted_output() {
        let mut bytes = std::fs::read(root().join("dicom-signed12-rescale.dcm")).unwrap();
        let slope = bytes
            .windows(6)
            .position(|v| v == [0x28, 0, 0x53, 0x10, b'D', b'S'])
            .unwrap();
        let length = u16::from_le_bytes(bytes[slope + 6..slope + 8].try_into().unwrap()) as usize;
        bytes[slope + 6..slope + 8].copy_from_slice(&4u16.to_le_bytes());
        bytes.splice(slope + 8..slope + 8 + length, *b"1e39");
        let budget = rrrah_core::MemoryBudget::new(96);
        let mut request = DecodeRequest::new("scalar.dcm");
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            decode(&bytes, &request, None),
            Err(RasterDecodeError::InvalidDicom("rescale overflow"))
        ));
        assert_eq!(budget.peak(), 96);
        assert_eq!(budget.used(), 0);
        let mut color = std::fs::read(root().join("dicom-signed12-rescale.dcm")).unwrap();
        let samples = color
            .windows(8)
            .position(|v| v == [0x28, 0, 2, 0, b'U', b'S', 2, 0])
            .unwrap();
        color[samples + 8..samples + 10].copy_from_slice(&3u16.to_le_bytes());
        assert!(matches!(
            decode(&color, &request, None),
            Err(RasterDecodeError::InvalidDicom(
                "non-scalar photometric interpretation"
            ))
        ));
        assert_eq!(budget.used(), 0);
    }
}
