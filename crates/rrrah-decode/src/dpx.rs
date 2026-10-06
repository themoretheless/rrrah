//! DPX single-element unsigned RGB/ABGR/luma storage, without invented log transforms.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidDpx(s)
}
pub(crate) fn has_magic(b: &[u8]) -> bool {
    b.starts_with(b"SDPX") || b.starts_with(b"XPDS")
}
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if b.len() < 1664 || !has_magic(b) {
        return Err(bad("truncated generic header"));
    }
    let be = b.starts_with(b"SDPX");
    let u16at = |at| {
        let v = [b[at], b[at + 1]];
        if be {
            u16::from_be_bytes(v)
        } else {
            u16::from_le_bytes(v)
        }
    };
    let u32at = |at| {
        let v = b[at..at + 4].try_into().unwrap();
        if be {
            u32::from_be_bytes(v)
        } else {
            u32::from_le_bytes(v)
        }
    };
    if !matches!(&b[8..12], b"V1.0" | b"V2.0") {
        return Err(bad("unsupported version"));
    }
    if u32at(660) != u32::MAX || u16at(770) != 1 || u32at(780) != 0 || u16at(806) != 0 {
        return Err(bad("encrypted, signed, encoded or multiple image elements"));
    }
    let orient = u16at(768);
    if orient > 7 {
        return Err(bad("undefined orientation"));
    }
    let (w, h) = (u32at(772), u32at(776));
    if w == 0 || h == 0 || w > 65536 || h > 65536 {
        return Err(bad("invalid dimensions"));
    }
    let descriptor = b[800];
    let channels = match descriptor {
        6 => 1usize,
        50 => 3,
        51 | 52 => 4,
        _ => return Err(bad("unsupported component descriptor")),
    };
    let bits = b[803];
    let packing = u16at(804);
    if !matches!(bits, 8 | 10 | 12 | 16) || packing > 2 || (bits == 10 && (packing == 0 || descriptor == 6)) {
        return Err(bad("unsupported sample packing"));
    }
    if u64::from(w) * u64::from(h) * 8 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let samples = w as usize * channels;
    let row = match bits {
        8 => samples,
        10 => samples.div_ceil(3) * 4,
        12 if packing == 0 => (samples * 12).div_ceil(32) * 4,
        _ => samples * 2,
    };
    let line_padding = u32at(812) as usize;
    let image_padding = u32at(816) as usize;
    let stride = row
        .checked_add(line_padding)
        .ok_or_else(|| bad("row size overflow"))?;
    let start = u32at(808) as usize;
    let generic = u32at(24) as usize;
    let industry = u32at(28) as usize;
    let user = u32at(32) as usize;
    let header_end = generic
        .checked_add(industry)
        .and_then(|v| v.checked_add(user))
        .ok_or_else(|| bad("header size overflow"))?;
    if generic < 1664 || start < header_end || start != u32at(4) as usize {
        return Err(bad("invalid data offset or header sizes"));
    }
    let end = start
        .checked_add(
            stride
                .checked_mul(h as usize)
                .ok_or_else(|| bad("image size overflow"))?,
        )
        .and_then(|v| v.checked_add(image_padding))
        .ok_or_else(|| bad("image size overflow"))?;
    if end != b.len() || u32at(16) as usize != b.len() {
        return Err(bad("truncated payload or inconsistent declared length"));
    }
    let (ow, oh) = if orient >= 4 { (h, w) } else { (w, h) };
    let mut out = vec![0u16; w as usize * h as usize * 4];
    let max = (1u32 << bits) - 1;
    for y in 0..h as usize {
        request.check_cancelled()?;
        let r = &b[start + y * stride..start + y * stride + row];
        let word16 = |at| {
            let v = [r[at], r[at + 1]];
            if be {
                u16::from_be_bytes(v)
            } else {
                u16::from_le_bytes(v)
            }
        };
        let word32 = |at| {
            let v = r[at..at + 4].try_into().unwrap();
            if be {
                u32::from_be_bytes(v)
            } else {
                u32::from_le_bytes(v)
            }
        };
        let sample = |i: usize| -> u16 {
            let v = match bits {
                8 => u32::from(r[i]),
                10 => {
                    let shift = if packing == 1 { 22 } else { 20 };
                    (word32(i / 3 * 4) >> (shift - (i % 3) * 10)) & 1023
                }
                12 if packing == 0 => {
                    let bit = i * 12;
                    let at = bit / 32 * 4;
                    let shift = bit % 32;
                    let low = word32(at) >> shift;
                    let high = if shift > 20 {
                        word32(at + 4) << (32 - shift)
                    } else {
                        0
                    };
                    (low | high) & 4095
                }
                12 => u32::from(word16(i * 2)) >> if packing == 1 { 4 } else { 0 },
                _ => u32::from(word16(i * 2)),
            } & max;
            ((v * 65535 + max / 2) / max) as u16
        };
        for x in 0..w as usize {
            let mut rgba = [65535; 4];
            match descriptor {
                6 => {
                    rgba[..3].fill(sample(x));
                }
                52 => {
                    for (c, v) in rgba.iter_mut().enumerate() {
                        *v = sample(x * 4 + 3 - c);
                    }
                }
                _ => {
                    for (c, v) in rgba.iter_mut().take(channels).enumerate() {
                        *v = sample(x * channels + c);
                    }
                }
            }
            let (dx, dy) = match orient {
                0 => (x, y),
                1 => (w as usize - 1 - x, y),
                2 => (x, h as usize - 1 - y),
                3 => (w as usize - 1 - x, h as usize - 1 - y),
                4 => (y, x),
                5 => (h as usize - 1 - y, x),
                6 => (y, w as usize - 1 - x),
                _ => (h as usize - 1 - y, w as usize - 1 - x),
            };
            let at = (dy * ow as usize + dx) * 4;
            out[at..at + 4].copy_from_slice(&rgba);
        }
    }
    let normalized_reference = u32at(784) == 0
        && u32at(792) == max
        && f32::from_bits(u32at(788)) == 0.0
        && f32::from_bits(u32at(796)) == 1.0;
    let color = if normalized_reference && b[801] == 2 && b[802] == 6 {
        RasterColorSpace::LinearSrgb
    } else if normalized_reference && b[801] == 2 {
        RasterColorSpace::LinearRgbUnspecified
    } else {
        RasterColorSpace::Unspecified
    };
    let pixels = if bits == 8 {
        RasterPixels::Rgba8(Arc::new(out.into_iter().map(|v| (v / 257) as u8).collect::<Vec<_>>()).into())
    } else {
        RasterPixels::Rgba16(Arc::new(out).into())
    };
    Ok(DecodedRaster::new(ow, oh, pixels, color)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        include_bytes!("../../../tests/fixtures/raster/dpx-8-be-50-p0.dpx").to_vec()
    }
    #[test]
    fn all_orientations_color_and_bounds() {
        let request = DecodeRequest::new("synthetic.dpx");
        let bytes = fixture();
        let base = decode(&bytes, &request).unwrap();
        let RasterPixels::Rgba8(base) = base.pixels() else {
            panic!()
        };
        for orientation in 0u16..8 {
            let mut b = bytes.clone();
            b[768..770].copy_from_slice(&orientation.to_be_bytes());
            b[801] = 2;
            b[802] = 6;
            b[792..796].copy_from_slice(&255u32.to_be_bytes());
            b[796..800].copy_from_slice(&1.0f32.to_bits().to_be_bytes());
            let frame = decode(&b, &request).unwrap();
            assert_eq!(
                (frame.width(), frame.height()),
                if orientation < 4 { (5, 3) } else { (3, 5) }
            );
            let RasterPixels::Rgba8(p) = frame.pixels() else {
                panic!()
            };
            let orders = [
                [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14],
                [4, 3, 2, 1, 0, 9, 8, 7, 6, 5, 14, 13, 12, 11, 10],
                [10, 11, 12, 13, 14, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4],
                [14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
                [0, 5, 10, 1, 6, 11, 2, 7, 12, 3, 8, 13, 4, 9, 14],
                [10, 5, 0, 11, 6, 1, 12, 7, 2, 13, 8, 3, 14, 9, 4],
                [4, 9, 14, 3, 8, 13, 2, 7, 12, 1, 6, 11, 0, 5, 10],
                [14, 9, 4, 13, 8, 3, 12, 7, 2, 11, 6, 1, 10, 5, 0],
            ];
            for (pixel, source) in p.chunks_exact(4).zip(orders[orientation as usize]) {
                assert_eq!(pixel, &base[source * 4..source * 4 + 4]);
            }
            crate::prepare_raster_for_display(&frame).unwrap();
        }
        assert!(crate::prepare_raster_for_display(&decode(&bytes, &request).unwrap()).is_err());
        for at in [
            4, 16, 24, 660, 768, 770, 772, 776, 780, 803, 804, 806, 808, 812, 816,
        ] {
            let mut b = bytes.clone();
            b[at..at + 2].fill(if at == 660 { 0 } else { 255 });
            assert!(decode(&b, &request).is_err(), "field {at}");
        }
        assert!(decode(&bytes[..100], &request).is_err());
        assert!(decode(&bytes[..bytes.len() - 1], &request).is_err());
    }
    #[test]
    fn explicit_line_and_image_padding_and_abgr16() {
        let request = DecodeRequest::new("synthetic.dpx");
        let mut b = fixture();
        let payload = b.split_off(2048);
        for row in payload.chunks_exact(15) {
            b.extend_from_slice(row);
            b.extend([1, 2, 3]);
        }
        b.extend([4, 5]);
        b[812..816].copy_from_slice(&3u32.to_be_bytes());
        b[816..820].copy_from_slice(&2u32.to_be_bytes());
        let len = b.len() as u32;
        b[16..20].copy_from_slice(&len.to_be_bytes());
        let padded = decode(&b, &request).unwrap();
        let original = decode(&fixture(), &request).unwrap();
        let (RasterPixels::Rgba8(a), RasterPixels::Rgba8(c)) = (padded.pixels(), original.pixels()) else {
            panic!()
        };
        assert_eq!(a.as_slice(), c.as_slice());
        let mut b = include_bytes!("../../../tests/fixtures/raster/dpx-16-be-51-p0.dpx").to_vec();
        let expected = decode(&b, &request).unwrap();
        b[800] = 52;
        for pixel in b[2048..].chunks_exact_mut(8) {
            pixel.swap(0, 6);
            pixel.swap(1, 7);
            pixel.swap(2, 4);
            pixel.swap(3, 5);
        }
        let actual = decode(&b, &request).unwrap();
        let (RasterPixels::Rgba16(a), RasterPixels::Rgba16(c)) = (actual.pixels(), expected.pixels()) else {
            panic!()
        };
        assert_eq!(a.as_slice(), c.as_slice());
    }
}
