//! Single-part OpenEXR color and physical-pixel declarations.
//! https://openexr.com/en/latest/StandardAttributes.html
use crate::RasterDecodeError;
use rrrah_core::RasterColorSpace;
const REC709: [f32; 8] = [0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290];
const REC2020: [f32; 8] = [0.708, 0.292, 0.170, 0.797, 0.131, 0.046, 0.3127, 0.3290];
const P3_D65: [f32; 8] = [0.680, 0.320, 0.265, 0.690, 0.150, 0.060, 0.3127, 0.3290];
const AP1: [f64; 8] = [0.713, 0.293, 0.165, 0.830, 0.128, 0.044, 0.32168, 0.33767];
const AP0: [f64; 8] = [0.7347, 0.2653, 0.0, 1.0, 0.0001, -0.0770, 0.32168, 0.33767];
fn same_xy(a: &[f32], b: &[f64]) -> bool {
    a.iter()
        .zip(b)
        .all(|(a, b)| (f64::from(*a) - b).abs() <= 0.000001)
}
fn explicit_color(xy: [f64; 8]) -> Result<RasterColorSpace, RasterDecodeError> {
    rrrah_core::RgbChromaticities::new(xy)
        .map(RasterColorSpace::LinearRgb)
        .ok_or_else(|| invalid("invalid or singular chromaticities"))
}
fn linear_color(primaries: u16) -> RasterColorSpace {
    if primaries == 1 {
        RasterColorSpace::LinearSrgb
    } else {
        RasterColorSpace::Cicp {
            primaries,
            transfer: 8,
            matrix: 0,
            full_range: true,
        }
    }
}
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
#[derive(Debug, Default)]
pub(crate) struct Declaration {
    pub(crate) color: Option<RasterColorSpace>,
    pub(crate) pixel_aspect: Option<f32>,
}

#[cfg(test)]
fn declaration(bytes: &[u8]) -> Result<Option<RasterColorSpace>, RasterDecodeError> {
    metadata(bytes).map(|value| value.color)
}

pub(crate) fn metadata(bytes: &[u8]) -> Result<Declaration, RasterDecodeError> {
    if !bytes.starts_with(&[0x76, 0x2f, 0x31, 0x01]) {
        return Ok(Declaration::default());
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
    let mut pixel_aspect = None;
    for _ in 0..4096 {
        if at > 1024 * 1024 {
            return Err(invalid("header limit"));
        }
        let name = text(bytes, &mut at)?;
        if name.is_empty() {
            let chroma_color = chroma
                .as_ref()
                .map(|c| {
                    let xy = std::array::from_fn(|i| f64::from(c[i]));
                    let explicit = explicit_color(xy)?;
                    Ok::<_, RasterDecodeError>(
                        [(1, &REC709), (9, &REC2020), (12, &P3_D65)]
                            .into_iter()
                            .find_map(|(id, reference)| same(c, reference).then(|| linear_color(id)))
                            .unwrap_or(explicit),
                    )
                })
                .transpose()?;
            let declared_id = match interop {
                Some(b"lin_rec709_scene") => Some((
                    linear_color(1),
                    [0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290],
                )),
                Some(b"lin_rec2020_scene") => Some((
                    linear_color(9),
                    [0.708, 0.292, 0.170, 0.797, 0.131, 0.046, 0.3127, 0.3290],
                )),
                Some(b"lin_p3d65_scene") => Some((
                    linear_color(12),
                    [0.680, 0.320, 0.265, 0.690, 0.150, 0.060, 0.3127, 0.3290],
                )),
                Some(b"lin_ap1_scene") => Some((explicit_color(AP1)?, AP1)),
                Some(b"lin_ap0_scene") => Some((explicit_color(AP0)?, AP0)),
                _ => None,
            };
            if let (Some((_, reference)), Some(c)) = (&declared_id, &chroma) {
                if !same_xy(c, reference) {
                    return Err(invalid("conflicting colorInteropID and chromaticities"));
                }
            }
            if aces == Some(1)
                && (declared_id.as_ref().is_some_and(|(_, xy)| *xy != AP0)
                    || chroma.as_ref().is_some_and(|c| !same_xy(c, &AP0)))
            {
                return Err(invalid("ACES flag conflicts with declared non-AP0 RGB"));
            }
            let (known, xy) = if interop.is_some() {
                match declared_id {
                    Some((color, xy)) => (Some(color), Some(xy)),
                    None => (None, None),
                }
            } else {
                (
                    chroma_color,
                    chroma.as_ref().map(|c| std::array::from_fn(|i| f64::from(c[i]))),
                )
            };
            let color = Some(match (known, xy, neutral) {
                (Some(_), Some(xy), Some(n)) => RasterColorSpace::LinearRgb(
                    rrrah_core::RgbChromaticities::new(xy)
                        .and_then(|c| c.with_adopted_neutral([f64::from(n[0]), f64::from(n[1])]))
                        .ok_or_else(|| invalid("invalid adopted neutral"))?,
                ),
                (Some(color), _, _) => color,
                _ => RasterColorSpace::LinearRgbUnspecified,
            });
            return Ok(Declaration { color, pixel_aspect });
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
            b"pixelAspectRatio" => {
                if pixel_aspect.is_some() || kind != b"float" || len != 4 {
                    return Err(RasterDecodeError::InvalidExrGeometry(
                        "pixelAspectRatio type/length/duplicate",
                    ));
                }
                let aspect = f32::from_le_bytes(data.try_into().unwrap());
                if !aspect.is_finite() || aspect <= 0.0 {
                    return Err(RasterDecodeError::InvalidExrGeometry(
                        "pixelAspectRatio must be finite and positive",
                    ));
                }
                pixel_aspect = Some(aspect);
            }
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
                if values[0] <= 0.0 || values[1] <= 0.0 || f64::from(values[0]) + f64::from(values[1]) >= 1.0
                {
                    return Err(invalid("invalid adopted neutral"));
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
    fn pixel_aspect_is_independent_of_color_interpretation() {
        for aspect in [0.5f32, 1.0, 2.0] {
            for id in [b"lin_rec709_scene".as_slice(), b"unknown".as_slice()] {
                let result = metadata(&header(&[
                    ("pixelAspectRatio", "float", aspect.to_le_bytes().to_vec()),
                    ("colorInteropID", "string", id.to_vec()),
                ]))
                .unwrap();
                assert_eq!(result.pixel_aspect, Some(aspect));
                assert_eq!(
                    result.color,
                    Some(if id == b"unknown" {
                        RasterColorSpace::LinearRgbUnspecified
                    } else {
                        RasterColorSpace::LinearSrgb
                    })
                );
            }
        }
    }

    #[test]
    fn invalid_pixel_aspect_is_refused_before_sample_decoding() {
        for aspect in [0.0f32, -0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(matches!(
                metadata(&header(&[(
                    "pixelAspectRatio",
                    "float",
                    aspect.to_le_bytes().to_vec()
                ),])),
                Err(RasterDecodeError::InvalidExrGeometry(_))
            ));
        }
        for attrs in [
            vec![("pixelAspectRatio", "double", 2f64.to_le_bytes().to_vec())],
            vec![("pixelAspectRatio", "float", vec![0; 3])],
            vec![
                ("pixelAspectRatio", "float", 1f32.to_le_bytes().to_vec()),
                ("pixelAspectRatio", "float", 2f32.to_le_bytes().to_vec()),
            ],
        ] {
            assert!(matches!(
                metadata(&header(&attrs)),
                Err(RasterDecodeError::InvalidExrGeometry(_))
            ));
        }
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
        assert!(
            declaration(&header(&[
                ("chromaticities", "chromaticities", chroma()),
                ("adoptedNeutral", "v2f", vec![0; 8]),
            ]))
            .is_err()
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
