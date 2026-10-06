//! PSD/PSB stored RGB/grayscale composition, raw and PackBits.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{io::Read, sync::Arc};
pub(crate) fn has_magic(b: &[u8]) -> bool {
    b.starts_with(b"8BPS")
}
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidPsd(s)
}
struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], RasterDecodeError> {
        let end = self.at.checked_add(n).ok_or_else(|| bad("length overflow"))?;
        let b = self.b.get(self.at..end).ok_or_else(|| bad("truncated section"))?;
        self.at = end;
        Ok(b)
    }
    fn word(&mut self) -> Result<u16, RasterDecodeError> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn long(&mut self) -> Result<u32, RasterDecodeError> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn size(&mut self, large: bool) -> Result<usize, RasterDecodeError> {
        if large {
            usize::try_from(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))
                .map_err(|_| bad("section overflow"))
        } else {
            Ok(self.long()? as usize)
        }
    }
}
fn unpack(bytes: &[u8], out: &mut Vec<u8>, expected: usize) -> Result<(), RasterDecodeError> {
    let start = out.len();
    let mut r = Reader { b: bytes, at: 0 };
    while r.at < bytes.len() {
        let count = r.take(1)?[0] as i8;
        let n = if count >= 0 {
            count as usize + 1
        } else if count == -128 {
            0
        } else {
            (1 - i16::from(count)) as usize
        };
        if out.len() - start + n > expected {
            return Err(bad("RLE row overflow"));
        }
        if count >= 0 {
            out.extend_from_slice(r.take(n)?);
        } else if n > 0 {
            let v = r.take(1)?[0];
            out.resize(out.len() + n, v);
        }
    }
    if out.len() - start != expected {
        return Err(bad("RLE row underflow"));
    }
    Ok(())
}
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    let mut r = Reader { b: bytes, at: 0 };
    if r.take(4)? != b"8BPS" {
        return Err(bad("signature"));
    }
    let version = r.word()?;
    if !matches!(version, 1 | 2) || r.take(6)? != [0; 6] {
        return Err(bad("version or reserved bytes"));
    }
    let large = version == 2;
    let channels = r.word()? as usize;
    let height = r.long()?;
    let width = r.long()?;
    let depth = r.word()?;
    let mode = r.word()?;
    let colors = match mode {
        1 => 1,
        3 => 3,
        _ => return Err(bad("color mode unsupported")),
    };
    if channels < colors
        || channels > 56
        || !matches!(depth, 8 | 16 | 32)
        || width == 0
        || height == 0
        || width > 65536
        || height > 65536
    {
        return Err(bad("dimensions, channels or depth"));
    }
    let sample = usize::from(depth / 8);
    let pixels = width as usize * height as usize;
    let plane_bytes = pixels.checked_mul(sample).ok_or_else(|| bad("size overflow"))?;
    let total = plane_bytes
        .checked_mul(channels)
        .ok_or_else(|| bad("size overflow"))?;
    if (total as u64) + (pixels as u64) * 4 * (sample as u64) > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let color_len = r.long()? as usize;
    if color_len != 0 {
        return Err(bad("unexpected color mode data"));
    }
    let resource_len = r.long()? as usize;
    let mut resources = Reader {
        b: r.take(resource_len)?,
        at: 0,
    };
    let mut profile = None;
    let mut named_alpha = false;
    while resources.at < resources.b.len() {
        if resources.take(4)? != b"8BIM" {
            return Err(bad("resource signature"));
        }
        let id = resources.word()?;
        let name = resources.take(1)?[0] as usize;
        resources.take(name)?;
        if (name + 1) % 2 != 0 {
            resources.take(1)?;
        }
        let size = resources.long()? as usize;
        let data = resources.take(size)?;
        if size % 2 != 0 {
            resources.take(1)?;
        }
        if id == 1053 {
            named_alpha = true;
        }
        if id == 1039 {
            if profile.is_some() || size > 4 * 1024 * 1024 {
                return Err(bad("duplicate or excessive ICC"));
            }
            profile = Some(data.to_vec());
        }
    }
    let layer_len = r.size(large)?;
    let layer = r.take(layer_len)?;
    let mut alpha = channels == colors + 1 && !named_alpha;
    if !layer.is_empty() {
        let mut lr = Reader { b: layer, at: 0 };
        let info_len = lr.size(large)?;
        let info = lr.take(info_len)?;
        if !info.is_empty() {
            let mut ir = Reader { b: info, at: 0 };
            let count = ir.word()? as i16;
            if count != 0 {
                alpha = count < 0;
            }
        }
    }
    if alpha && channels <= colors {
        return Err(bad("missing merged alpha"));
    }
    let compression = r.word()?;
    let mut data = Vec::with_capacity(total);
    match compression {
        0 => data.extend_from_slice(r.take(total)?),
        1 => {
            let rows = channels * height as usize;
            let mut lengths = Vec::with_capacity(rows);
            for _ in 0..rows {
                lengths.push(if large {
                    r.long()? as usize
                } else {
                    r.word()? as usize
                });
            }
            for len in lengths {
                request.check_cancelled()?;
                unpack(r.take(len)?, &mut data, width as usize * sample)?;
            }
        }
        2 | 3 => {
            let compressed = r.take(bytes.len() - r.at)?;
            let mut decoder = flate2::read::ZlibDecoder::new(compressed);
            let mut chunk = [0u8; 65536];
            loop {
                request.check_cancelled()?;
                let n = decoder.read(&mut chunk).map_err(|_| bad("invalid ZIP stream"))?;
                if n == 0 {
                    break;
                }
                if data.len() + n > total {
                    return Err(bad("ZIP output exceeds canvas"));
                }
                data.extend_from_slice(&chunk[..n]);
            }
            if data.len() != total || decoder.total_in() != compressed.len() as u64 {
                return Err(bad("ZIP size or trailing bytes"));
            }
            if compression == 3 {
                for row in data.chunks_exact_mut(width as usize * sample) {
                    request.check_cancelled()?;
                    match depth {
                        8 => {
                            for i in 1..row.len() {
                                row[i] = row[i].wrapping_add(row[i - 1]);
                            }
                        }
                        16 => {
                            let mut previous = 0u16;
                            for bytes in row.chunks_exact_mut(2) {
                                previous =
                                    previous.wrapping_add(u16::from_be_bytes(bytes.try_into().unwrap()));
                                bytes.copy_from_slice(&previous.to_be_bytes());
                            }
                        }
                        32 => {
                            for i in 1..row.len() {
                                row[i] = row[i].wrapping_add(row[i - 1]);
                            }
                            let planes = row.to_vec();
                            for x in 0..width as usize {
                                for c in 0..4 {
                                    row[x * 4 + c] = planes[c * width as usize + x];
                                }
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
        _ => return Err(bad("compression unsupported")),
    }
    if r.at != bytes.len() {
        return Err(bad("trailing composition bytes"));
    }
    let color = profile.map_or(RasterColorSpace::Unspecified, RasterColorSpace::Icc);
    let offset = |c: usize, p: usize| c * plane_bytes + p * sample;
    macro_rules! interleave {
        ($ty:ty,$read:expr,$opaque:expr,$variant:ident) => {{
            let mut out = Vec::<$ty>::with_capacity(pixels * 4);
            for p in 0..pixels {
                if p % 65536 == 0 {
                    request.check_cancelled()?;
                }
                for c in 0..3 {
                    out.push($read(offset(if colors == 1 { 0 } else { c }, p)));
                }
                out.push(if alpha { $read(offset(colors, p)) } else { $opaque });
            }
            RasterPixels::$variant(Arc::new(out).into())
        }};
    }
    let mut pixels = match depth {
        8 => interleave!(u8, |i| data[i], 255, Rgba8),
        16 => interleave!(
            u16,
            |i| u16::from_be_bytes(data[i..i + 2].try_into().unwrap()),
            65535,
            Rgba16
        ),
        _ => interleave!(
            f32,
            |i| f32::from_be_bytes(data[i..i + 4].try_into().unwrap()),
            1.0,
            Rgba32Float
        ),
    };
    if alpha {
        if colors != 3 || depth != 8 {
            return Err(bad("high precision or grayscale merged transparency pending"));
        }
        let RasterPixels::Rgba8(p) = &mut pixels else {
            unreachable!()
        };
        for rgba in p
            .get_mut()
            .ok_or_else(|| bad("merged pixels unexpectedly shared"))?
            .chunks_exact_mut(4)
        {
            let a = u32::from(rgba[3]);
            if a > 0 {
                for c in &mut rgba[..3] {
                    *c = ((u32::from(*c) + a).saturating_sub(255) * 255 / a).min(255) as u8;
                }
            }
        }
    }
    Ok(DecodedRaster::new(width, height, pixels, color)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(depth: u16, samples: &[u8]) -> Vec<u8> {
        let mut b = b"8BPS".to_vec();
        b.extend(1u16.to_be_bytes());
        b.extend([0; 6]);
        b.extend(3u16.to_be_bytes());
        b.extend(1u32.to_be_bytes());
        b.extend(2u32.to_be_bytes());
        b.extend(depth.to_be_bytes());
        b.extend(3u16.to_be_bytes());
        b.extend([0; 12]);
        b.extend(0u16.to_be_bytes());
        b.extend(samples);
        b
    }
    #[test]
    fn retains_sixteen_bit_precision() {
        let samples = [32768u16, 32769, 1, 2, 65534, 65535];
        let bytes = fixture(
            16,
            &samples.into_iter().flat_map(u16::to_be_bytes).collect::<Vec<_>>(),
        );
        let f = decode(&bytes, &DecodeRequest::new("image.psd")).unwrap();
        let RasterPixels::Rgba16(p) = f.pixels() else {
            panic!()
        };
        assert_eq!(p.as_slice(), [32768, 1, 65534, 65535, 32769, 2, 65535, 65535]);
    }
    #[test]
    fn retains_float_values() {
        let samples = [0.5f32, 2.0, 0.1, 0.2, 0.3, 0.4];
        let bytes = fixture(
            32,
            &samples.into_iter().flat_map(f32::to_be_bytes).collect::<Vec<_>>(),
        );
        let f = decode(&bytes, &DecodeRequest::new("image.psd")).unwrap();
        let RasterPixels::Rgba32Float(p) = f.pixels() else {
            panic!()
        };
        assert_eq!(p.as_slice(), [0.5, 0.1, 0.3, 1.0, 2.0, 0.2, 0.4, 1.0]);
    }
    #[test]
    fn packbits_repeats_noop_and_row_limits() {
        let mut out = vec![];
        unpack(&[128, 254, 17], &mut out, 3).unwrap();
        assert_eq!(out, [17; 3]);
        assert!(unpack(&[254, 17], &mut vec![], 2).is_err());
        assert!(unpack(&[1, 17], &mut vec![], 2).is_err());
    }
    #[test]
    fn independent_prediction_retains_high_precision() {
        let request = DecodeRequest::new("image.psd");
        let f = decode(
            include_bytes!("../../../tests/fixtures/raster/prediction-16.psd"),
            &request,
        )
        .unwrap();
        let RasterPixels::Rgba16(p) = f.pixels() else {
            panic!()
        };
        assert_eq!(p.as_slice(), [32768, 1, 65534, 65535, 32769, 2, 65535, 65535]);
        let f = decode(
            include_bytes!("../../../tests/fixtures/raster/prediction-32.psd"),
            &request,
        )
        .unwrap();
        let RasterPixels::Rgba32Float(p) = f.pixels() else {
            panic!()
        };
        assert_eq!(p.as_slice(), [0.5, 0.1, 0.3, 1.0, 2.0, 0.2, 0.4, 1.0]);
    }
    #[test]
    fn zip_rejects_truncation_and_trailing_stream_data() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/composite-1-2.psd");
        let request = DecodeRequest::new("image.psd");
        assert!(decode(&bytes[..bytes.len() - 1], &request).is_err());
        let mut extra = bytes.to_vec();
        extra.push(0);
        assert!(decode(&extra, &request).is_err());
    }
}
