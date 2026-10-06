//! Explicit single-part OpenEXR color declarations. Missing metadata is unknown.
//! https://openexr.com/en/latest/StandardAttributes.html
use crate::RasterDecodeError;
use rrrah_core::RasterColorSpace;
const REC709: [f32; 8] = [0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290];
fn invalid(reason: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidExrColor(reason)
}
fn same(a: &[f32], b: &[f32]) -> bool {
    a.iter().zip(b).all(|(a, b)| (*a - *b).abs() <= 0.000001)
}
fn text<'a>(bytes: &'a [u8], at: &mut usize) -> Result<&'a [u8], RasterDecodeError> {
    let tail = bytes.get(*at..).ok_or_else(|| invalid("truncated attribute"))?;
    let end = tail
        .iter()
        .take(256)
        .position(|b| *b == 0)
        .ok_or_else(|| invalid("attribute name/type limit"))?;
    *at += end + 1;
    Ok(&tail[..end])
}
pub(crate) fn declaration(bytes: &[u8]) -> Result<Option<RasterColorSpace>, RasterDecodeError> {
    if !bytes.starts_with(&[0x76, 0x2f, 0x31, 0x01]) {
        return Ok(None);
    }
    let version = u32::from_le_bytes(
        bytes
            .get(4..8)
            .ok_or_else(|| invalid("truncated version"))?
            .try_into()
            .unwrap(),
    );
    if version & 0x1800 != 0 {
        return Err(invalid("deep/multipart color selection is not qualified"));
    }
    let mut at = 8;
    let mut chroma: Option<Vec<f32>> = None;
    let mut interop: Option<&[u8]> = None;
    let mut neutral: Option<Vec<f32>> = None;
    let mut aces = None;
    for _ in 0..4096 {
        if at > 1024 * 1024 {
            return Err(invalid("header limit"));
        }
        let name = text(bytes, &mut at)?;
        if name.is_empty() {
            let rec709 = chroma.as_ref().is_some_and(|c| same(c, &REC709));
            let id709 = interop == Some(b"lin_rec709_scene".as_slice());
            if id709 && chroma.is_some() && !rec709 {
                return Err(invalid("conflicting colorInteropID and chromaticities"));
            }
            if aces == Some(1) && (id709 || rec709) {
                return Err(invalid("ACES flag conflicts with Rec.709"));
            }
            let neutral_ok = neutral.as_ref().is_none_or(|n| same(n, &REC709[6..]));
            let known = match interop {
                Some(_) => id709,
                None => rec709,
            };
            return Ok(Some(if known && neutral_ok && aces != Some(1) {
                RasterColorSpace::LinearSrgb
            } else {
                RasterColorSpace::LinearRgbUnspecified
            }));
        }
        let kind = text(bytes, &mut at)?;
        let len = u32::from_le_bytes(
            bytes
                .get(at..at + 4)
                .ok_or_else(|| invalid("truncated length"))?
                .try_into()
                .unwrap(),
        ) as usize;
        at += 4;
        let end = at
            .checked_add(len)
            .filter(|e| *e <= 1024 * 1024)
            .ok_or_else(|| invalid("header limit"))?;
        let data = bytes.get(at..end).ok_or_else(|| invalid("truncated value"))?;
        at = end;
        match name {
            b"chromaticities" => {
                if chroma.is_some() || kind != b"chromaticities" || len != 32 {
                    return Err(invalid("chromaticities type/length/duplicate"));
                }
                let values: Vec<f32> = data
                    .chunks_exact(4)
                    .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
                    .collect();
                if values.iter().any(|v| !v.is_finite()) {
                    return Err(invalid("non-finite chromaticities"));
                }
                chroma = Some(values);
            }
            b"colorInteropID" => {
                if interop.is_some()
                    || kind != b"string"
                    || len == 0
                    || len > 1024
                    || data.contains(&0)
                    || std::str::from_utf8(data).is_err()
                {
                    return Err(invalid("colorInteropID type/value/duplicate"));
                }
                interop = Some(data);
            }
            b"adoptedNeutral" => {
                if neutral.is_some() || kind != b"v2f" || len != 8 {
                    return Err(invalid("adoptedNeutral type/length/duplicate"));
                }
                let values: Vec<f32> = data
                    .chunks_exact(4)
                    .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
                    .collect();
                if values.iter().any(|v| !v.is_finite()) {
                    return Err(invalid("non-finite adoptedNeutral"));
                }
                neutral = Some(values);
            }
            b"acesImageContainerFlag" => {
                if aces.is_some() || kind != b"int" || len != 4 {
                    return Err(invalid("ACES flag type/length/duplicate"));
                }
                let value = i32::from_le_bytes(data.try_into().unwrap());
                if !(0..=1).contains(&value) {
                    return Err(invalid("ACES flag value"));
                }
                aces = Some(value);
            }
            _ => {}
        }
    }
    Err(invalid("attribute count limit"))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn header(attrs: &[(&str, &str, Vec<u8>)]) -> Vec<u8> {
        let mut b = vec![0x76, 0x2f, 0x31, 1, 2, 0, 0, 0];
        for (n, t, v) in attrs {
            b.extend(n.as_bytes());
            b.push(0);
            b.extend(t.as_bytes());
            b.push(0);
            b.extend((v.len() as u32).to_le_bytes());
            b.extend(v);
        }
        b.push(0);
        b
    }
    fn chroma() -> Vec<u8> {
        REC709.iter().flat_map(|v| v.to_le_bytes()).collect()
    }
    #[test]
    fn explicit_declarations_and_conflicts() {
        assert_eq!(
            declaration(&header(&[])).unwrap(),
            Some(RasterColorSpace::LinearRgbUnspecified)
        );
        for a in [
            vec![("chromaticities", "chromaticities", chroma())],
            vec![("colorInteropID", "string", b"lin_rec709_scene".to_vec())],
        ] {
            assert_eq!(
                declaration(&header(&a)).unwrap(),
                Some(RasterColorSpace::LinearSrgb)
            );
        }
        assert_eq!(
            declaration(&header(&[
                ("chromaticities", "chromaticities", chroma()),
                ("colorInteropID", "string", b"unknown".to_vec())
            ]))
            .unwrap(),
            Some(RasterColorSpace::LinearRgbUnspecified)
        );
        assert!(
            declaration(&header(&[
                ("chromaticities", "chromaticities", vec![0; 32]),
                ("colorInteropID", "string", b"lin_rec709_scene".to_vec())
            ]))
            .is_err()
        );
        assert!(
            declaration(&header(&[
                ("chromaticities", "chromaticities", chroma()),
                ("acesImageContainerFlag", "int", 1i32.to_le_bytes().to_vec())
            ]))
            .is_err()
        );
        assert_eq!(
            declaration(&header(&[
                ("chromaticities", "chromaticities", chroma()),
                ("adoptedNeutral", "v2f", vec![0; 8])
            ]))
            .unwrap(),
            Some(RasterColorSpace::LinearRgbUnspecified)
        );
    }
    #[test]
    fn truncation_invalid_types_duplicates_and_nonfinite_are_rejected() {
        let valid = header(&[("chromaticities", "chromaticities", chroma())]);
        for end in 4..valid.len() {
            assert!(declaration(&valid[..end]).is_err(), "{end}");
        }
        for attrs in [
            vec![("chromaticities", "float", chroma())],
            vec![("chromaticities", "chromaticities", vec![0; 31])],
            vec![("colorInteropID", "string", vec![0])],
            vec![
                ("chromaticities", "chromaticities", chroma()),
                ("chromaticities", "chromaticities", chroma()),
            ],
        ] {
            assert!(declaration(&header(&attrs)).is_err());
        }
        let mut nan = chroma();
        nan[..4].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(declaration(&header(&[("chromaticities", "chromaticities", nan)])).is_err());
    }
}

#[cfg(test)]
mod real_fixture_tests {
    use super::*;
    #[test]
    fn declared_rec709_exr_prepares_hdr_without_changing_samples() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/float-none.exr");
        let original = std::fs::read(&path).unwrap();
        let mut at = 8;
        loop {
            if text(&original, &mut at).unwrap().is_empty() {
                break;
            }
            text(&original, &mut at).unwrap();
            let n = u32::from_le_bytes(original[at..at + 4].try_into().unwrap()) as usize;
            at += 4 + n;
        }
        let mut attribute = Vec::new();
        attribute.extend(b"chromaticities\0chromaticities\0");
        attribute.extend(32u32.to_le_bytes());
        attribute.extend(REC709.iter().flat_map(|v| v.to_le_bytes()));
        // Scanline chunk offsets are absolute file positions. Extend the header
        // and relocate every offset without touching compressed channel samples.
        let first_chunk = u64::from_le_bytes(original[at..at + 8].try_into().unwrap()) as usize;
        let table_bytes = first_chunk - at;
        assert_eq!(table_bytes % 8, 0);
        let mut tagged = original[..at - 1].to_vec();
        tagged.extend(&attribute);
        tagged.push(0);
        for offset in original[at..first_chunk].chunks_exact(8) {
            tagged.extend(
                (u64::from_le_bytes(offset.try_into().unwrap()) + attribute.len() as u64).to_le_bytes(),
            );
        }
        tagged.extend(&original[first_chunk..]);
        let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = crate::DecodeRequest::new(path.clone());
        request.memory_budget = Some(budget.clone());
        let frame = crate::raster::decode_raster_bytes(tagged, &request).unwrap();
        assert_eq!(frame.color_space(), &RasterColorSpace::LinearSrgb);
        let prepared = crate::prepare_raster_for_display_with_budget(&frame, Some(&budget)).unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(a) = frame.pixels() else {
            panic!()
        };
        let rrrah_core::RasterPixels::Rgba32Float(b) = prepared.pixels() else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        assert!(a.iter().any(|v| *v > 1.0));
        let reference = crate::decode_raster(&crate::DecodeRequest::new(path)).unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(c) = reference.pixels() else {
            panic!()
        };
        assert_eq!(
            a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            c.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
        drop(frame);
        drop(prepared);
        assert_eq!(budget.used(), 0);
    }
}
