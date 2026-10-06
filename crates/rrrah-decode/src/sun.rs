//! Sun Raster: big-endian header, 16-bit row alignment and byte RLE.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
fn invalid(message: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidSun(message)
}
fn long(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    if bytes.len() < 32 || long(bytes, 0) != 0x59a66a95 {
        return Err(invalid("invalid or truncated header"));
    }
    let width = long(bytes, 4);
    let height = long(bytes, 8);
    let depth = long(bytes, 12);
    let length = long(bytes, 16) as usize;
    let kind = long(bytes, 20);
    let map = long(bytes, 24);
    let map_len = long(bytes, 28) as usize;
    if width == 0 || height == 0 || !matches!(depth, 1 | 8 | 24 | 32) || kind > 3 {
        return Err(invalid("unsupported dimensions, depth or storage"));
    }
    let pixel_count = u64::from(width) * u64::from(height);
    if pixel_count > MAX_RASTER_BYTES / 4 {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    if !(map == 0 && map_len == 0
        || map == 1 && map_len > 0 && map_len % 3 == 0 && map_len <= 768 && depth == 8)
    {
        return Err(invalid("unsupported or invalid colormap"));
    }
    let start = 32 + map_len;
    let palette = bytes.get(32..start).ok_or_else(|| invalid("truncated palette"))?;
    let encoded = bytes.get(start..).ok_or_else(|| invalid("truncated image"))?;
    let stride = ((u64::from(width) * u64::from(depth) + 15) / 16) * 2;
    let raw_size = stride * u64::from(height);
    if raw_size > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    if !(length == encoded.len() || kind == 0 && length == 0) {
        return Err(invalid("image length mismatch"));
    }
    let raw_size = raw_size as usize;
    let stride = stride as usize;
    let mut expanded = Vec::new();
    let raw = if kind == 2 {
        expanded
            .try_reserve_exact(raw_size)
            .map_err(|_| RasterDecodeError::OutputTooLarge)?;
        let mut input = encoded;
        let mut packets = 0_u32;
        while !input.is_empty() {
            if packets % 4096 == 0 {
                request.check_cancelled()?;
            }
            packets = packets.wrapping_add(1);
            let first = input[0];
            input = &input[1..];
            let (count, value) = if first != 128 {
                (1, first)
            } else {
                let (&run, rest) = input
                    .split_first()
                    .ok_or_else(|| invalid("truncated RLE escape"))?;
                input = rest;
                if run == 0 {
                    (1, 128)
                } else {
                    let (&value, rest) = input.split_first().ok_or_else(|| invalid("truncated RLE run"))?;
                    input = rest;
                    (usize::from(run) + 1, value)
                }
            };
            if count > raw_size - expanded.len() {
                return Err(invalid("RLE exceeds image size"));
            }
            expanded.resize(expanded.len() + count, value);
        }
        if expanded.len() != raw_size {
            return Err(invalid("short RLE image"));
        }
        expanded.as_slice()
    } else {
        if encoded.len() != raw_size {
            return Err(invalid("invalid row storage length"));
        }
        encoded
    };
    let mut output = Vec::new();
    output
        .try_reserve_exact(pixel_count as usize * 4)
        .map_err(|_| RasterDecodeError::OutputTooLarge)?;
    for row in raw.chunks_exact(stride) {
        request.check_cancelled()?;
        for x in 0..width as usize {
            let rgb = match depth {
                1 => {
                    let v = if row[x / 8] & (128 >> (x % 8)) == 0 {
                        255
                    } else {
                        0
                    };
                    [v, v, v]
                }
                8 => {
                    let index = usize::from(row[x]);
                    if palette.is_empty() {
                        [row[x]; 3]
                    } else {
                        let n = map_len / 3;
                        if index >= n {
                            return Err(invalid("palette index outside map"));
                        }
                        [palette[index], palette[n + index], palette[2 * n + index]]
                    }
                }
                24 | 32 => {
                    // 32-bit Sun pixels are XRGB/XBGR: X is padding, not alpha.
                    let size = (depth / 8) as usize;
                    let start = x * size + usize::from(depth == 32);
                    let p = &row[start..start + 3];
                    if kind == 3 {
                        [p[0], p[1], p[2]]
                    } else {
                        [p[2], p[1], p[0]]
                    }
                }
                _ => unreachable!(),
            };
            output.extend([rgb[0], rgb[1], rgb[2], 255]);
        }
    }
    Ok(DecodedRaster::new(
        width,
        height,
        RasterPixels::Rgba8(Arc::new(output).into()),
        RasterColorSpace::AssumedSrgb,
    )?)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(width: u32, depth: u32, kind: u32, data: &[u8]) -> Vec<u8> {
        let mut b = Vec::new();
        for v in [0x59a66a95, width, 1, depth, data.len() as u32, kind, 0, 0] {
            b.extend(v.to_be_bytes());
        }
        b.extend(data);
        b
    }
    #[test]
    fn rle_escape_repeat_and_padding() {
        let b = fixture(3, 8, 2, &[128, 0, 128, 1, 44, 0]);
        let f = decode(&b, &DecodeRequest::new("unused")).unwrap();
        let RasterPixels::Rgba8(p) = f.pixels() else {
            panic!()
        };
        assert_eq!(&p[..], [128, 128, 128, 255, 44, 44, 44, 255, 44, 44, 44, 255]);
    }
    #[test]
    fn rejects_truncation_overrun_and_unknown_layout() {
        for b in [
            fixture(2, 8, 2, &[128]),
            fixture(2, 8, 2, &[128, 2, 7]),
            fixture(2, 8, 2, &[1]),
            fixture(1, 16, 1, &[0; 2]),
        ] {
            assert!(decode(&b, &DecodeRequest::new("unused")).is_err());
        }
    }
    #[test]
    fn excessive_dimensions_are_rejected_before_allocation() {
        let mut b = fixture(1, 8, 1, &[0, 0]);
        b[4..8].copy_from_slice(&u32::MAX.to_be_bytes());
        b[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(matches!(
            decode(&b, &DecodeRequest::new("unused")),
            Err(RasterDecodeError::OutputTooLarge)
        ));
    }

    #[test]
    fn bit_order_and_odd_width_alignment() {
        let f = decode(&fixture(3, 1, 1, &[0b10100000, 0]), &DecodeRequest::new("unused")).unwrap();
        let RasterPixels::Rgba8(p) = f.pixels() else {
            panic!()
        };
        assert_eq!(&p[..], [0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255]);
    }
}
