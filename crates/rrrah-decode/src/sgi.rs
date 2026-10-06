//! SGI normal-color planar images. Untagged RGB uses the explicit sRGB import convention.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;

fn invalid(reason: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidSgi(reason)
}
fn word(bytes: &[u8], pos: usize) -> u16 {
    u16::from_be_bytes([bytes[pos], bytes[pos + 1]])
}
fn long(bytes: &[u8], pos: usize) -> usize {
    u32::from_be_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize
}
fn sample(bytes: &mut &[u8], bpc: usize) -> Result<u16, RasterDecodeError> {
    if bytes.len() < bpc {
        return Err(invalid("truncated sample"));
    }
    let value = if bpc == 1 {
        u16::from(bytes[0])
    } else {
        word(bytes, 0)
    };
    *bytes = &bytes[bpc..];
    Ok(value)
}
fn row(
    mut bytes: &[u8],
    width: usize,
    bpc: usize,
    rle: bool,
    output: &mut [u16],
) -> Result<(), RasterDecodeError> {
    if !rle {
        if bytes.len() != width * bpc {
            return Err(invalid("invalid verbatim row length"));
        }
        for value in output {
            *value = sample(&mut bytes, bpc)?;
        }
        return Ok(());
    }
    let mut position = 0;
    loop {
        let token = sample(&mut bytes, bpc)?;
        let count = usize::from(token & 127);
        if count == 0 {
            if position != width || !bytes.is_empty() {
                return Err(invalid("invalid RLE terminator or row length"));
            }
            return Ok(());
        }
        if count > width - position {
            return Err(invalid("RLE crosses row boundary"));
        }
        if token & 128 != 0 {
            for value in &mut output[position..position + count] {
                *value = sample(&mut bytes, bpc)?;
            }
        } else {
            let value = sample(&mut bytes, bpc)?;
            output[position..position + count].fill(value);
        }
        position += count;
    }
}
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    if bytes.len() < 512 || word(bytes, 0) != 474 {
        return Err(invalid("truncated header or invalid magic"));
    }
    let storage = bytes[2];
    let bpc = usize::from(bytes[3]);
    if storage > 1 || !matches!(bpc, 1 | 2) {
        return Err(invalid("unsupported storage or sample precision"));
    }
    let dimension = word(bytes, 4);
    let width = usize::from(word(bytes, 6));
    let height = usize::from(word(bytes, 8));
    let channels = usize::from(word(bytes, 10));
    if width == 0
        || height == 0
        || !matches!(channels, 1 | 3 | 4)
        || !matches!(dimension, 1..=3)
        || (dimension == 1 && (height != 1 || channels != 1))
        || (dimension == 2 && channels != 1)
    {
        return Err(invalid("unsupported dimensions or channel layout"));
    }
    if long(bytes, 104) != 0 {
        return Err(invalid("unsupported colormap"));
    }
    let count = width * height * 4;
    // Intermediate u16 storage is bounded even for byte samples.
    if count as u64 > MAX_RASTER_BYTES / 2 {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let rows = height * channels;
    let table_end = 512 + rows * 8;
    if storage == 1 && bytes.len() < table_end {
        return Err(invalid("truncated row tables"));
    }
    if storage == 0 && bytes.len() != 512 + width * rows * bpc {
        return Err(invalid("invalid image length"));
    }
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(count)
        .map_err(|_| RasterDecodeError::OutputTooLarge)?;
    pixels.resize(count, if bpc == 1 { 255 } else { 65535 });
    let mut values = vec![0; width];
    for channel in 0..channels {
        for y in 0..height {
            request.check_cancelled()?;
            let index = channel * height + y;
            let (start, len) = if storage == 0 {
                (512 + index * width * bpc, width * bpc)
            } else {
                (
                    long(bytes, 512 + index * 4),
                    long(bytes, 512 + rows * 4 + index * 4),
                )
            };
            if storage == 1 && start < table_end {
                return Err(invalid("row overlaps header or tables"));
            }
            let end = start
                .checked_add(len)
                .ok_or_else(|| invalid("row offset overflow"))?;
            let encoded = bytes.get(start..end).ok_or_else(|| invalid("row outside file"))?;
            row(encoded, width, bpc, storage == 1, &mut values)?;
            for (x, &value) in values.iter().enumerate() {
                let dest = ((height - 1 - y) * width + x) * 4;
                if channels == 1 {
                    pixels[dest..dest + 3].fill(value);
                } else {
                    pixels[dest + channel] = value;
                }
            }
        }
    }
    let pixels = if bpc == 1 {
        RasterPixels::Rgba8(Arc::new(pixels.into_iter().map(|v| v as u8).collect::<Vec<_>>()).into())
    } else {
        RasterPixels::Rgba16(Arc::new(pixels).into())
    };
    Ok(DecodedRaster::new(
        width as u32,
        height as u32,
        pixels,
        RasterColorSpace::AssumedSrgb,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn header(bpc: u8, rle: bool) -> Vec<u8> {
        let mut b = vec![0; 512];
        b[..2].copy_from_slice(&474u16.to_be_bytes());
        b[2] = u8::from(rle);
        b[3] = bpc;
        for (offset, value) in [(4, 2u16), (6, 2), (8, 2), (10, 1)] {
            b[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        }
        b
    }
    #[test]
    fn verbatim_precision_and_bottom_up() {
        let mut b = header(2, false);
        for v in [1u16, 257, 32768, 65535] {
            b.extend(v.to_be_bytes());
        }
        let frame = decode(&b, &DecodeRequest::new("unused.sgi")).unwrap();
        let RasterPixels::Rgba16(p) = frame.pixels() else {
            panic!()
        };
        assert_eq!(&p[..8], &[32768, 32768, 32768, 65535, 65535, 65535, 65535, 65535]);
    }
    #[test]
    fn shared_rle_rows_and_malformed_tables() {
        let mut b = header(1, true);
        for v in [528u32, 528, 3, 3] {
            b.extend(v.to_be_bytes());
        }
        b.extend([2, 37, 0]);
        let frame = decode(&b, &DecodeRequest::new("unused.sgi")).unwrap();
        let RasterPixels::Rgba8(p) = frame.pixels() else {
            panic!()
        };
        assert_eq!(&p[..4], &[37, 37, 37, 255]);
        b[512..516].copy_from_slice(&511u32.to_be_bytes());
        assert!(decode(&b, &DecodeRequest::new("unused")).is_err());
    }
    #[test]
    fn rle_bounds_and_termination() {
        let mut out = [0; 2];
        for bad in [&[3, 7, 0][..], &[2, 7][..], &[1, 7, 0][..], &[2, 7, 0, 0][..]] {
            assert!(row(bad, 2, 1, true, &mut out).is_err());
        }
        row(&[0, 130, 0, 1, 1, 1, 0, 0], 2, 2, true, &mut out).unwrap();
        assert_eq!(out, [1, 257]);
    }
}
