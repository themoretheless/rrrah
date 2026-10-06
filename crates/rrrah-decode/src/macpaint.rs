//! MacPaint data fork: a fixed 576x720 bitmap, row-wise PackBits.
use crate::{DecodeRequest, raster::RasterDecodeError};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{path::Path, sync::Arc};
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidMacPaint(s)
}
pub(crate) fn is_path(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        ["mac", "macp", "pntg", "mpnt"]
            .iter()
            .any(|known| e.eq_ignore_ascii_case(known))
    })
}
/// A MacBinary PNTG wrapper has a recognizable type/name header, unlike
/// an unwrapped MacPaint data fork. Validation happens before extraction.
pub(crate) fn has_wrapper(b: &[u8]) -> bool {
    b.len() >= 128 && b[0] == 0 && (1..=63).contains(&b[1]) && &b[65..69] == b"PNTG"
}
fn crc16(b: &[u8]) -> u16 {
    let mut crc = 0u16;
    for value in b {
        crc ^= u16::from(*value) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}
fn data_fork(b: &[u8]) -> Result<&[u8], RasterDecodeError> {
    if !has_wrapper(b) {
        return Ok(b);
    }
    let h = &b[..128];
    if h[74] != 0 || h[82] != 0 || h[126..128] != [0, 0] {
        return Err(bad("MacBinary reserved fields"));
    }
    match h[122] {
        0 => {
            if h[99..128].iter().any(|v| *v != 0) {
                return Err(bad("MacBinary I extended fields"));
            }
        }
        129 | 130 => {
            if h[123] != 129 || u16::from_be_bytes(h[124..126].try_into().unwrap()) != crc16(&h[..124]) {
                return Err(bad("MacBinary version or header CRC"));
            }
            if h[122] == 130 && &h[102..106] != b"mBIN" {
                return Err(bad("MacBinary III signature"));
            }
        }
        _ => return Err(bad("MacBinary version unsupported")),
    }
    let secondary = usize::from(u16::from_be_bytes(h[120..122].try_into().unwrap()));
    let data_len = u32::from_be_bytes(h[83..87].try_into().unwrap()) as usize;
    let resource_len = u32::from_be_bytes(h[87..91].try_into().unwrap()) as usize;
    let comment_len = usize::from(u16::from_be_bytes(h[99..101].try_into().unwrap()));
    if data_len < 512 || data_len > 1024 * 1024 || resource_len > 1024 * 1024 {
        return Err(bad("MacBinary fork size"));
    }
    let padded = |n: usize| n.div_ceil(128) * 128;
    let data_at = 128 + padded(secondary);
    let resource_at = data_at + padded(data_len);
    let comment_at = resource_at + padded(resource_len);
    let end = comment_at + padded(comment_len);
    if b.len() != end {
        return Err(bad("MacBinary section size or trailing data"));
    }
    for (start, end) in [
        (128 + secondary, data_at),
        (data_at + data_len, resource_at),
        (resource_at + resource_len, comment_at),
        (comment_at + comment_len, end),
    ] {
        if b[start..end].iter().any(|v| *v != 0) {
            return Err(bad("MacBinary nonzero section padding"));
        }
    }
    Ok(&b[data_at..data_at + data_len])
}
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if b.len() > 3 * 1024 * 1024 {
        return Err(bad("source exceeds MacPaint input limit"));
    }
    let b = data_fork(b)?;
    if b.len() > 1024 * 1024 {
        return Err(bad("data fork exceeds MacPaint input limit"));
    }
    let header = b.get(..512).ok_or_else(|| bad("truncated header"))?;
    if !matches!(u32::from_be_bytes(header[..4].try_into().unwrap()), 0 | 2) {
        return Err(bad("header version or wrapper unsupported"));
    }
    let mut at = 512;
    let mut out = Vec::with_capacity(576 * 720 * 4);
    for _ in 0..720 {
        request.check_cancelled()?;
        let mut row = [0u8; 72];
        let mut produced = 0;
        while produced < 72 {
            let control = *b.get(at).ok_or_else(|| bad("truncated scanline"))? as i8;
            at += 1;
            match control {
                -128 => (),
                0..=127 => {
                    let count = control as usize + 1;
                    if produced + count > 72 {
                        return Err(bad("literal crosses scanline"));
                    }
                    let bytes = b.get(at..at + count).ok_or_else(|| bad("truncated literal"))?;
                    row[produced..produced + count].copy_from_slice(bytes);
                    at += count;
                    produced += count;
                }
                _ => {
                    let count = (1 - i16::from(control)) as usize;
                    if produced + count > 72 {
                        return Err(bad("repeat crosses scanline"));
                    }
                    let value = *b.get(at).ok_or_else(|| bad("truncated repeat"))?;
                    at += 1;
                    row[produced..produced + count].fill(value);
                    produced += count;
                }
            }
        }
        for byte in row {
            for bit in (0..8).rev() {
                let value = if byte & (1 << bit) == 0 { 255 } else { 0 };
                out.extend([value, value, value, 255]);
            }
        }
    }
    if b[at..].iter().any(|v| *v != 128) {
        return Err(bad("trailing bitmap data"));
    }
    Ok(DecodedRaster::new(
        576,
        720,
        RasterPixels::Rgba8(Arc::new(out).into()),
        RasterColorSpace::Srgb,
    )?)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn blank() -> Vec<u8> {
        let mut b = vec![0u8; 512];
        for _ in 0..720 {
            b.extend([185, 0]);
        }
        b
    }
    #[test]
    fn macbinary_crc_and_section_bounds_are_checked() {
        assert_eq!(crc16(b"123456789"), 0x31c3);
        let bytes = include_bytes!("../../../tests/fixtures/raster/wrapped-129-1.mac");
        for at in [69, 124] {
            let mut invalid = bytes.to_vec();
            invalid[at] ^= 1;
            assert!(decode(&invalid, &DecodeRequest::new("x.mac")).is_err());
        }
        let mut invalid = bytes.to_vec();
        let n = u32::from_be_bytes(invalid[83..87].try_into().unwrap()) as usize;
        invalid[128 + n] = 1;
        assert!(decode(&invalid, &DecodeRequest::new("x.mac")).is_err());
        let mut invalid = bytes.to_vec();
        invalid.pop();
        assert!(decode(&invalid, &DecodeRequest::new("x.mac")).is_err());
        let mut invalid = bytes.to_vec();
        invalid[87..91].copy_from_slice(&u32::MAX.to_be_bytes());
        let checksum = crc16(&invalid[..124]);
        invalid[124..126].copy_from_slice(&checksum.to_be_bytes());
        assert!(decode(&invalid, &DecodeRequest::new("x.mac")).is_err());
    }
    #[test]
    fn secondary_header_rounding_preserves_data_fork() {
        let original = include_bytes!("../../../tests/fixtures/raster/wrapped-129-0.mac");
        let mut bytes = original.to_vec();
        bytes[120..122].copy_from_slice(&129u16.to_be_bytes());
        let checksum = crc16(&bytes[..124]);
        bytes[124..126].copy_from_slice(&checksum.to_be_bytes());
        let mut extra = vec![0; 256];
        extra[..129].fill(7);
        bytes.splice(128..128, extra);
        assert_eq!(data_fork(&bytes).unwrap(), data_fork(original).unwrap());
        bytes[128 + 129] = 1;
        assert!(data_fork(&bytes).is_err());
    }
    #[test]
    fn repeat_runs_and_noops_preserve_polarity() {
        let mut b = blank();
        b.splice(512..512, [128]);
        b.extend([128, 128]);
        let frame = decode(&b, &DecodeRequest::new("x.mac")).unwrap();
        let RasterPixels::Rgba8(p) = frame.pixels() else {
            panic!()
        };
        assert!(p.iter().all(|v| *v == 255));
        b[514] = 255;
        let frame = decode(&b, &DecodeRequest::new("x.mac")).unwrap();
        let RasterPixels::Rgba8(p) = frame.pixels() else {
            panic!()
        };
        assert_eq!(&p[..4], &[0, 0, 0, 255]);
        assert_eq!(&p[576 * 4..576 * 4 + 4], &[255; 4]);
    }
    #[test]
    fn bad_runs_truncation_and_trailing_data_fail_typed() {
        let b = blank();
        for length in [8, 512, b.len() - 1] {
            assert!(decode(&b[..length], &DecodeRequest::new("x.mac")).is_err());
        }
        for control in [0, 127, 184, 255] {
            let mut invalid = b.clone();
            invalid[512] = control;
            assert!(decode(&invalid, &DecodeRequest::new("x.mac")).is_err());
        }
        let mut invalid = b;
        invalid.push(0);
        assert!(decode(&invalid, &DecodeRequest::new("x.mac")).is_err());
    }
}
