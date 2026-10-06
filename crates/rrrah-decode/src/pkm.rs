//! PKM ETC1/ETC2 color textures, cropped from padded 4x4 blocks.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidPkm(s)
}
pub(crate) fn has_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PKM 10") || bytes.starts_with(b"PKM 20")
}
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    let h = bytes.get(..16).ok_or_else(|| bad("truncated header"))?;
    if !has_magic(h) {
        return Err(bad("invalid signature"));
    }
    let word = |at| usize::from(u16::from_be_bytes([h[at], h[at + 1]]));
    let kind = word(6);
    let ew = word(8);
    let eh = word(10);
    let width = word(12);
    let height = word(14);
    if width == 0 || height == 0 || ew != width.div_ceil(4) * 4 || eh != height.div_ceil(4) * 4 {
        return Err(bad("invalid encoded or original dimensions"));
    }
    if &h[..6] == b"PKM 10" && kind != 0 {
        return Err(bad("PKM10 requires ETC1"));
    }
    let block_size = match kind {
        0 | 1 | 4 => 8,
        3 => 16,
        _ => return Err(bad("ETC/EAC storage type unsupported")),
    };
    let count = (ew / 4) * (eh / 4);
    if (width as u64) * (height as u64) * 4 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    if bytes.len() != 16 + count * block_size {
        return Err(bad("payload size mismatch"));
    }
    let mut pixels = vec![0u8; width * height * 4];
    for (i, block) in bytes[16..].chunks_exact(block_size).enumerate() {
        request.check_cancelled()?;
        let mut decoded = [0u32; 16];
        match kind {
            0 => {
                if block[3] & 2 != 0 {
                    for value in &block[..3] {
                        let base = i16::from(value >> 3);
                        let delta = i16::from(value & 7);
                        let delta = if delta >= 4 { delta - 8 } else { delta };
                        if !(0..32).contains(&(base + delta)) {
                            return Err(bad("invalid ETC1 differential color"));
                        }
                    }
                }
                texture2ddecoder::decode_etc1_block(block, &mut decoded);
            }
            1 => texture2ddecoder::decode_etc2_rgb_block(block, &mut decoded),
            3 => texture2ddecoder::decode_etc2_rgba8_block(block, &mut decoded),
            4 => texture2ddecoder::decode_etc2_rgba1_block(block, &mut decoded),
            _ => unreachable!(),
        }
        let (left, top) = ((i % (ew / 4)) * 4, (i / (ew / 4)) * 4);
        for y in 0..4 {
            for x in 0..4 {
                if left + x < width && top + y < height {
                    let [b, g, r, a] = decoded[y * 4 + x].to_le_bytes();
                    let at = ((top + y) * width + left + x) * 4;
                    pixels[at..at + 4].copy_from_slice(&[r, g, b, a]);
                }
            }
        }
    }
    Ok(DecodedRaster::new(
        width as u32,
        height as u32,
        RasterPixels::Rgba8(Arc::new(pixels).into()),
        RasterColorSpace::AssumedSrgb,
    )?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_dimensions_and_payload() {
        for b in [
            b"PKM 10\0\0\0\x04\0\x04\0\0\0\x03".as_slice(),
            b"PKM 10\0\0\0\x04\0\x04\0\x03\0\x03".as_slice(),
        ] {
            assert!(decode(b, &DecodeRequest::new("x.pkm")).is_err());
        }
    }
    #[test]
    fn invalid_etc1_differential_and_excessive_output_are_rejected() {
        let mut bytes = include_bytes!("../../../tests/fixtures/raster/individual.pkm").to_vec();
        bytes[16] = 249;
        bytes[19] |= 2;
        assert!(decode(&bytes, &DecodeRequest::new("x.pkm")).is_err());
        let mut bytes = b"PKM 10".to_vec();
        for value in [0u16, 65532, 65532, 65532, 65532] {
            bytes.extend(value.to_be_bytes());
        }
        assert!(matches!(
            decode(&bytes, &DecodeRequest::new("x.pkm")),
            Err(RasterDecodeError::OutputTooLarge)
        ));
    }
    #[test]
    fn constant_alpha_reaches_linear_display() {
        let frame = decode(
            include_bytes!("../../../tests/fixtures/raster/etc2-alpha-constant.pkm"),
            &DecodeRequest::new("x.pkm"),
        )
        .unwrap();
        let prepared = crate::prepare_raster_for_display(&frame).unwrap();
        let RasterPixels::Rgba32Float(p) = prepared.pixels() else {
            panic!()
        };
        assert!((p[3] - 17.0 / 255.0).abs() < 1e-7);
    }
}
