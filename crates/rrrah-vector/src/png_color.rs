//! PNG declared colors: cICP has precedence over ICC and sRGB.
//! <https://www.w3.org/TR/png-3/#11cICP>
use crate::RasterDecodeError;
use rrrah_core::RasterColorSpace;

#[derive(Debug, Default)]
pub struct Declaration {
    pub color: Option<RasterColorSpace>,
    pub override_icc: bool,
    pub pixel_aspect: Option<f32>,
}

fn invalid(message: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidPngColor(message)
}

fn gamma_chroma_color(
    gamma: Option<u32>,
    xy: Option<[f64; 8]>,
) -> Result<Option<RasterColorSpace>, RasterDecodeError> {
    let (Some(gamma), Some(xy)) = (gamma, xy) else {
        return Ok(None);
    };
    let chroma = rrrah_core::RgbChromaticities::new(xy)
        .ok_or_else(|| invalid("invalid or singular cHRM coordinates"))?;
    if gamma == 100000 {
        return Ok(Some(RasterColorSpace::LinearRgb(chroma)));
    }
    let exponent = 100000. / f64::from(gamma);
    // ICC parametric curves store signed 16.16 coefficients. Do not silently
    // clamp a valid PNG declaration outside that representation.
    if !(1. / 65536. ..=32767.).contains(&exponent) {
        return Err(invalid("gAMA exponent outside supported ICC representation"));
    }
    use moxcms::{ColorProfile, ToneReprCurve};
    let mut profile = ColorProfile::new_srgb();
    profile.description = None;
    profile.copyright = None;
    crate::icc_colorimetry::set_rgb_colorimetry(&mut profile, xy)
        .ok_or_else(|| invalid("invalid or singular cHRM coordinates"))?;
    let curve = ToneReprCurve::Parametric(vec![exponent as f32]);
    profile.red_trc = Some(curve.clone());
    profile.green_trc = Some(curve.clone());
    profile.blue_trc = Some(curve);
    let bytes = profile
        .encode()
        .map_err(|_| invalid("gAMA/cHRM profile encoding failed"))?;
    Ok(Some(RasterColorSpace::Icc(bytes)))
}

// Gray samples need a transfer curve but no RGB primary coordinates.
fn gray_gamma_color(gamma: u32) -> Result<Option<RasterColorSpace>, RasterDecodeError> {
    let exponent = 100000. / f64::from(gamma);
    if !(1. / 65536. ..=32767.).contains(&exponent) {
        return Err(invalid("gAMA exponent outside supported ICC representation"));
    }
    let mut profile = moxcms::ColorProfile::new_gray_with_gamma(exponent as f32);
    profile.gray_trc = Some(moxcms::ToneReprCurve::Parametric(vec![exponent as f32]));
    let bytes = profile
        .encode()
        .map_err(|_| invalid("gray gAMA profile encoding failed"))?;
    Ok(Some(RasterColorSpace::Icc(bytes)))
}

pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320_u32 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

pub fn declaration(bytes: &[u8]) -> Result<Declaration, RasterDecodeError> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(Declaration::default());
    }
    let mut offset = 8;
    let mut srgb = false;
    let mut iccp = false;
    let mut cicp = None;
    let mut gamma = None;
    let mut chroma = None;
    let mut grayscale = false;
    let mut phys_seen = false;
    let mut pixel_aspect = None;
    for _ in 0..65536 {
        let header = bytes
            .get(offset..offset + 8)
            .ok_or_else(|| invalid("truncated chunk header"))?;
        let len = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as usize;
        let kind = &header[4..8];
        if kind == b"IDAT" || kind == b"IEND" {
            if srgb && iccp {
                return Err(invalid("sRGB and iCCP are mutually exclusive"));
            }
            return Ok(if let Some(color) = cicp {
                Declaration {
                    color: Some(color),
                    override_icc: true,
                    pixel_aspect,
                }
            } else {
                Declaration {
                    color: if srgb {
                        Some(RasterColorSpace::Srgb)
                    } else if !iccp && grayscale && gamma.is_some() && chroma.is_none() {
                        gray_gamma_color(gamma.unwrap())?
                    } else if !iccp && gamma.is_some() && chroma.is_some() {
                        gamma_chroma_color(gamma, chroma)?
                    } else if !iccp && gamma.is_none() && chroma.is_none() {
                        Some(RasterColorSpace::AssumedSrgb)
                    } else {
                        None
                    },
                    override_icc: false,
                    pixel_aspect,
                }
            });
        }
        let end = offset
            .checked_add(12)
            .and_then(|n| n.checked_add(len))
            .ok_or_else(|| invalid("chunk length overflow"))?;
        let chunk = bytes.get(offset..end).ok_or_else(|| invalid("truncated chunk"))?;
        let data = &chunk[8..8 + len];
        if matches!(kind, b"sRGB" | b"cICP" | b"iCCP" | b"gAMA" | b"cHRM") {
            let crc = &chunk[8 + len..];
            let expected = u32::from_be_bytes([crc[0], crc[1], crc[2], crc[3]]);
            if crc32(&chunk[4..8 + len]) != expected {
                return Err(invalid("color chunk CRC mismatch"));
            }
        }
        if kind == b"pHYs" {
            let crc = u32::from_be_bytes(chunk[8 + len..12 + len].try_into().unwrap());
            if phys_seen || len != 9 || crc32(&chunk[4..8 + len]) != crc {
                return Err(RasterDecodeError::InvalidPngPhysicalDimensions);
            }
            phys_seen = true;
            if data[8] > 1 {
                return Err(RasterDecodeError::InvalidPngPhysicalDimensions);
            }
            let x = u32::from_be_bytes(data[..4].try_into().unwrap());
            let y = u32::from_be_bytes(data[4..8].try_into().unwrap());
            // Width/height is the inverse of horizontal/vertical density.
            // Zero density conveys no usable aspect; keep square-pixel default.
            if x != 0 && y != 0 {
                pixel_aspect = Some((f64::from(y) / f64::from(x)) as f32);
            }
        }
        match kind {
            b"IHDR" => {
                if data.len() != 13 {
                    return Err(invalid("invalid IHDR length"));
                }
                grayscale = matches!(data[9], 0 | 4);
            }
            b"gAMA" => {
                if gamma.is_some() || data.len() != 4 {
                    return Err(invalid("invalid or duplicate gAMA"));
                }
                let value = u32::from_be_bytes(data.try_into().unwrap());
                if value == 0 {
                    return Err(invalid("zero gAMA"));
                }
                gamma = Some(value);
            }
            b"cHRM" => {
                if chroma.is_some() || data.len() != 32 {
                    return Err(invalid("invalid or duplicate cHRM"));
                }
                let mut coordinates = [0.; 8];
                for (i, value) in data.chunks_exact(4).enumerate() {
                    coordinates[(i + 6) % 8] =
                        f64::from(u32::from_be_bytes(value.try_into().unwrap())) / 100000.;
                }
                if coordinates
                    .chunks_exact(2)
                    .any(|v| v[0] > 1. || v[1] > 1. || v[0] + v[1] > 1.)
                {
                    return Err(invalid("cHRM coordinate outside chromaticity diagram"));
                }
                if rrrah_core::RgbChromaticities::new(coordinates).is_none() {
                    return Err(invalid("invalid or singular cHRM coordinates"));
                }
                chroma = Some(coordinates);
            }
            b"sRGB" => {
                if srgb || data.len() != 1 || data[0] > 3 {
                    return Err(invalid("invalid or duplicate sRGB chunk"));
                }
                srgb = true;
            }
            b"iCCP" => {
                if iccp {
                    return Err(invalid("duplicate iCCP chunk"));
                }
                iccp = true;
            }
            b"cICP" => {
                if cicp.is_some() || data.len() != 4 {
                    return Err(invalid("invalid or duplicate cICP chunk"));
                }
                // PNG requires RGB matrix coefficients; both full and narrow
                // range are valid. Preserve unsupported declarations so callers
                // can require the appropriate transform instead of assuming sRGB.
                if data[2] != 0 || data[3] > 1 {
                    return Err(invalid("PNG cICP must use RGB and a valid range flag"));
                }
                cicp = Some(match data {
                    [1, 13, 0, 1] => RasterColorSpace::Srgb,
                    [1, 8, 0, 1] => RasterColorSpace::LinearSrgb,
                    [1, 1, 0, 1] => RasterColorSpace::Bt709,
                    _ => RasterColorSpace::Cicp {
                        primaries: u16::from(data[0]),
                        transfer: u16::from(data[1]),
                        matrix: 0,
                        full_range: data[3] == 1,
                    },
                });
            }
            _ => {}
        }
        offset = end;
    }
    Err(invalid("too many metadata chunks"))
}

#[doc(hidden)]
pub mod tests {
    use super::*;
    pub fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        out.extend_from_slice(&crc32(&out[4..]).to_be_bytes());
        out
    }
    #[cfg(test)]
    fn png(chunks: &[Vec<u8>]) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        for chunk in chunks {
            bytes.extend_from_slice(chunk);
        }
        bytes.extend_from_slice(&chunk(b"IDAT", &[]));
        bytes
    }
    pub fn chrm(p3: bool) -> Vec<u8> {
        let xy: [u32; 8] = if p3 {
            [31270, 32900, 68000, 32000, 26500, 69000, 15000, 6000]
        } else {
            [31270, 32900, 64000, 33000, 30000, 60000, 15000, 6000]
        };
        chunk(
            b"cHRM",
            &xy.into_iter().flat_map(u32::to_be_bytes).collect::<Vec<_>>(),
        )
    }
    #[test]
    fn gamma_only_requires_gray_ihdr_and_respects_srgb() {
        let gamma = chunk(b"gAMA", &50000u32.to_be_bytes());
        for kind in [0, 4] {
            let mut ihdr = [0u8; 13];
            ihdr[9] = kind;
            let header = chunk(b"IHDR", &ihdr);
            assert!(matches!(
                declaration(&png(&[header.clone(), gamma.clone()])).unwrap().color,
                Some(RasterColorSpace::Icc(_))
            ));
            assert_eq!(
                declaration(&png(&[header.clone(), gamma.clone(), chunk(b"sRGB", &[0])]))
                    .unwrap()
                    .color,
                Some(RasterColorSpace::Srgb)
            );
            assert!(declaration(&png(&[header, chunk(b"gAMA", &1u32.to_be_bytes())])).is_err());
        }
        let mut rgb = [0u8; 13];
        rgb[9] = 2;
        assert!(
            declaration(&png(&[chunk(b"IHDR", &rgb), gamma]))
                .unwrap()
                .color
                .is_none()
        );
    }

    #[test]
    fn gamma_chroma_profiles_obey_precedence_and_validate_chunks() {
        let gamma = chunk(b"gAMA", &50000u32.to_be_bytes());
        assert!(matches!(
            declaration(&png(&[gamma.clone(), chrm(false)])).unwrap().color,
            Some(RasterColorSpace::Icc(_))
        ));
        assert!(matches!(
            declaration(&png(&[chunk(b"gAMA", &100000u32.to_be_bytes()), chrm(true)]))
                .unwrap()
                .color,
            Some(RasterColorSpace::LinearRgb(_))
        ));
        assert_eq!(
            declaration(&png(&[gamma.clone(), chrm(false), chunk(b"sRGB", &[0])]))
                .unwrap()
                .color,
            Some(RasterColorSpace::Srgb)
        );
        assert_eq!(
            declaration(&png(&[gamma.clone(), chrm(false), chunk(b"cICP", &[1, 8, 0, 1])]))
                .unwrap()
                .color,
            Some(RasterColorSpace::LinearSrgb)
        );
        assert!(declaration(&png(&[gamma.clone()])).unwrap().color.is_none());
        assert!(declaration(&png(&[chrm(false)])).unwrap().color.is_none());
        let mut corrupt = gamma.clone();
        *corrupt.last_mut().unwrap() ^= 1;
        for chunks in [
            vec![corrupt],
            vec![gamma.clone(), gamma],
            vec![chrm(false), chrm(false)],
            vec![chunk(b"gAMA", &[0; 4])],
            vec![chunk(b"gAMA", &[1; 3])],
            vec![chunk(b"gAMA", &1u32.to_be_bytes()), chrm(false)],
            vec![chunk(b"cHRM", &[0; 32])],
            vec![chunk(b"cHRM", &[0; 31])],
        ] {
            assert!(declaration(&png(&chunks)).is_err());
        }
    }
    #[test]
    fn cicp_overrides_srgb_and_unknown_primaries_do_not_become_srgb() {
        let result = declaration(&png(&[chunk(b"sRGB", &[0]), chunk(b"cICP", &[1, 8, 0, 1])])).unwrap();
        assert_eq!(result.color, Some(RasterColorSpace::LinearSrgb));
        assert!(result.override_icc);
        let result = declaration(&png(&[chunk(b"cICP", &[9, 16, 0, 1])])).unwrap();
        assert_eq!(
            result.color,
            Some(RasterColorSpace::Cicp {
                primaries: 9,
                transfer: 16,
                matrix: 0,
                full_range: true,
            })
        );
        assert_eq!(
            declaration(&png(&[chunk(b"sRGB", &[2])])).unwrap().color,
            Some(RasterColorSpace::Srgb)
        );
    }
    #[test]
    fn cicp_preserves_hdr_unknown_and_narrow_range_declarations() {
        for (primaries, transfer, range) in [
            (9, 16, 1),
            (9, 18, 1),
            (12, 8, 1),
            (9, 8, 1),
            (1, 1, 0),
            (255, 255, 1),
        ] {
            let result = declaration(&png(&[chunk(b"cICP", &[primaries, transfer, 0, range])])).unwrap();
            assert_eq!(
                result.color,
                Some(RasterColorSpace::Cicp {
                    primaries: u16::from(primaries),
                    transfer: u16::from(transfer),
                    matrix: 0,
                    full_range: range == 1,
                })
            );
            assert!(result.override_icc);
        }
        assert!(declaration(&png(&[chunk(b"cICP", &[1, 13, 0, 2])])).is_err());
    }
    #[test]
    fn corrupt_duplicate_and_invalid_color_chunks_are_rejected() {
        let mut corrupt = chunk(b"sRGB", &[0]);
        corrupt[8] = 1;
        for chunks in [
            vec![corrupt],
            vec![chunk(b"sRGB", &[4])],
            vec![chunk(b"sRGB", &[0]), chunk(b"sRGB", &[0])],
            vec![chunk(b"cICP", &[1, 13, 1, 1])],
        ] {
            assert!(declaration(&png(&chunks)).is_err());
        }
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }
}
