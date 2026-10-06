//! Softimage PIC packet streams, with each run bounded to its scanline.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidSoftimage(s)
}
pub(crate) fn has_magic(b: &[u8]) -> bool {
    b.starts_with(&[0x53, 0x80, 0xf6, 0x34])
}
struct Input<'a> {
    b: &'a [u8],
    at: usize,
}
impl Input<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], RasterDecodeError> {
        let end = self.at.checked_add(n).ok_or_else(|| bad("offset overflow"))?;
        let v = self.b.get(self.at..end).ok_or_else(|| bad("truncated packet"))?;
        self.at = end;
        Ok(v)
    }
    fn byte(&mut self) -> Result<u8, RasterDecodeError> {
        Ok(self.take(1)?[0])
    }
}
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if b.len() < 104 || !has_magic(b) || &b[88..92] != b"PICT" {
        return Err(bad("invalid header"));
    }
    let w = u16::from_be_bytes([b[92], b[93]]) as usize;
    let h = u16::from_be_bytes([b[94], b[95]]) as usize;
    let ratio = f32::from_be_bytes(b[96..100].try_into().unwrap());
    if w == 0
        || h == 0
        || f32::from_be_bytes(b[4..8].try_into().unwrap()) != 1.0
        || ratio != 1.0
        || u16::from_be_bytes([b[100], b[101]]) != 3
    {
        return Err(bad("unsupported version, aspect ratio, fields or dimensions"));
    }
    if w as u64 * h as u64 * 8 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let mut input = Input { b, at: 104 };
    let mut packets = Vec::new();
    let mut seen = 0u8;
    let mut depth = 8;
    loop {
        let p = input.take(4)?;
        let (chain, bits, kind, mask) = (p[0], p[1], p[2], p[3]);
        if chain > 1
            || !matches!(bits, 8 | 16)
            || kind > 2
            || mask == 0
            || mask & 15 != 0
            || mask & seen != 0
            || packets.len() == 4
        {
            return Err(bad("invalid channel packet"));
        }
        seen |= mask;
        depth = depth.max(bits);
        packets.push((bits, kind, mask));
        if chain == 0 {
            break;
        }
    }
    if seen & 0xe0 != 0xe0 {
        return Err(bad("RGB channels are required"));
    }
    let mut out = vec![0u16; w * h * 4];
    for p in out.chunks_exact_mut(4) {
        p[3] = 65535;
    }
    for y in 0..h {
        request.check_cancelled()?;
        for &(bits, kind, mask) in &packets {
            let channels: Vec<_> = (0..4).filter(|c| mask & (128 >> c) != 0).collect();
            let mut x = 0;
            while x < w {
                let (n, repeated) = match kind {
                    0 => (w - x, false),
                    1 => (input.byte()? as usize, true),
                    _ => {
                        let count = input.byte()?;
                        if count < 128 {
                            (count as usize + 1, false)
                        } else if count == 128 {
                            let v = input.take(2)?;
                            (u16::from_be_bytes([v[0], v[1]]) as usize, true)
                        } else {
                            (count as usize - 127, true)
                        }
                    }
                };
                if n == 0 || n > w - x {
                    return Err(bad("zero or cross-row run"));
                }
                let mut pixel = [0u16; 4];
                for i in 0..n {
                    if !repeated || i == 0 {
                        for &c in &channels {
                            pixel[c] = if bits == 8 {
                                u16::from(input.byte()?) * 257
                            } else {
                                let v = input.take(2)?;
                                u16::from_be_bytes([v[0], v[1]])
                            };
                        }
                    }
                    for &c in &channels {
                        out[((y * w + x + i) * 4) + c] = pixel[c];
                    }
                }
                x += n;
            }
        }
    }
    if input.at != b.len() {
        return Err(bad("trailing payload"));
    }
    let pixels = if depth == 8 {
        RasterPixels::Rgba8(Arc::new(out.into_iter().map(|v| (v / 257) as u8).collect::<Vec<_>>()).into())
    } else {
        RasterPixels::Rgba16(Arc::new(out).into())
    };
    Ok(DecodedRaster::new(
        w as u32,
        h as u32,
        pixels,
        RasterColorSpace::AssumedSrgb,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_packets_and_runs() {
        let b = include_bytes!("../../../tests/fixtures/raster/softimage-8-a1-rle2.pic");
        let request = DecodeRequest::new("synthetic.pic");
        for end in [0, 103, 105, 111, b.len() - 1] {
            assert!(decode(&b[..end], &request).is_err());
        }
        for at in [4, 88, 92, 96, 100, 104, 105, 106, 107, 108, 109, 110, 111] {
            let mut v = b.to_vec();
            v[at] = 255;
            assert!(decode(&v, &request).is_err(), "field {at}");
        }
        let mut v = b.to_vec();
        v[113..115].copy_from_slice(&0u16.to_be_bytes());
        assert!(decode(&v, &request).is_err());
        let mut v = b.to_vec();
        v[113..115].copy_from_slice(&141u16.to_be_bytes());
        assert!(decode(&v, &request).is_err());
        let mut v = b.to_vec();
        v.push(0);
        assert!(decode(&v, &request).is_err());
    }
}
