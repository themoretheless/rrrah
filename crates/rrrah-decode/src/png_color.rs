//! PNG declared colors: cICP has precedence over ICC and sRGB.
//! <https://www.w3.org/TR/png-3/#11cICP>
use crate::RasterDecodeError;
use rrrah_core::RasterColorSpace;

#[derive(Debug, Default)]
pub(crate) struct Declaration {
    pub(crate) color: Option<RasterColorSpace>,
    pub(crate) override_icc: bool,
}

fn invalid(message: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidPngColor(message)
}

pub(crate) fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320_u32 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

pub(crate) fn declaration(bytes: &[u8]) -> Result<Declaration, RasterDecodeError> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(Declaration::default());
    }
    let mut offset = 8;
    let mut srgb = false;
    let mut iccp = false;
    let mut cicp = None;
    let mut other_color = false;
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
                }
            } else {
                Declaration {
                    color: if srgb {
                        Some(RasterColorSpace::Srgb)
                    } else if !iccp && !other_color {
                        Some(RasterColorSpace::AssumedSrgb)
                    } else {
                        None
                    },
                    override_icc: false,
                }
            });
        }
        let end = offset
            .checked_add(12)
            .and_then(|n| n.checked_add(len))
            .ok_or_else(|| invalid("chunk length overflow"))?;
        let chunk = bytes.get(offset..end).ok_or_else(|| invalid("truncated chunk"))?;
        let data = &chunk[8..8 + len];
        if kind == b"sRGB" || kind == b"cICP" || kind == b"iCCP" {
            let crc = &chunk[8 + len..];
            let expected = u32::from_be_bytes([crc[0], crc[1], crc[2], crc[3]]);
            if crc32(&chunk[4..8 + len]) != expected {
                return Err(invalid("color chunk CRC mismatch"));
            }
        }
        match kind {
            b"gAMA" | b"cHRM" => other_color = true,
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
                // RGB matrix coefficients and full range are mandatory in PNG.
                if data[2] != 0 || data[3] != 1 {
                    return Err(invalid("PNG cICP must be full-range RGB"));
                }
                cicp = Some(match data {
                    [1, 13, 0, 1] => RasterColorSpace::Srgb,
                    [1, 8, 0, 1] => RasterColorSpace::LinearSrgb,
                    _ => RasterColorSpace::Unspecified,
                });
            }
            _ => {}
        }
        offset = end;
    }
    Err(invalid("too many metadata chunks"))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        out.extend_from_slice(&crc32(&out[4..]).to_be_bytes());
        out
    }
    fn png(chunks: &[Vec<u8>]) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        for chunk in chunks {
            bytes.extend_from_slice(chunk);
        }
        bytes.extend_from_slice(&chunk(b"IDAT", &[]));
        bytes
    }
    #[test]
    fn cicp_overrides_srgb_and_unknown_primaries_do_not_become_srgb() {
        let result = declaration(&png(&[chunk(b"sRGB", &[0]), chunk(b"cICP", &[1, 8, 0, 1])])).unwrap();
        assert_eq!(result.color, Some(RasterColorSpace::LinearSrgb));
        assert!(result.override_icc);
        let result = declaration(&png(&[chunk(b"cICP", &[9, 16, 0, 1])])).unwrap();
        assert_eq!(result.color, Some(RasterColorSpace::Unspecified));
        assert_eq!(
            declaration(&png(&[chunk(b"sRGB", &[2])])).unwrap().color,
            Some(RasterColorSpace::Srgb)
        );
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
