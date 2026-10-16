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
fn buffer<T: Copy>(
    count: usize,
    initial: T,
    request: &DecodeRequest,
) -> Result<rrrah_core::PixelBuffer<T>, RasterDecodeError> {
    Ok(match &request.memory_budget {
        Some(budget) => budget
            .try_buffer(count, initial)
            .map_err(crate::DecodeError::Memory)?
            .freeze()
            .into(),
        None => {
            let mut values = Vec::new();
            values
                .try_reserve_exact(count)
                .map_err(|_| RasterDecodeError::OutputTooLarge)?;
            values.resize(count, initial);
            Arc::new(values).into()
        }
    })
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
fn row<const EMIT: bool>(
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
        if EMIT {
            for value in output {
                *value = sample(&mut bytes, bpc)?;
            }
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
            if EMIT {
                for value in &mut output[position..position + count] {
                    *value = sample(&mut bytes, bpc)?;
                }
            } else {
                bytes = bytes
                    .get(count * bpc..)
                    .ok_or_else(|| invalid("truncated sample"))?;
            }
        } else {
            let value = sample(&mut bytes, bpc)?;
            if EMIT {
                output[position..position + count].fill(value);
            }
        }
        position += count;
    }
}
pub fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
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
    let count = width
        .checked_mul(height)
        .and_then(|n| n.checked_mul(4))
        .ok_or(RasterDecodeError::OutputTooLarge)?;
    if count as u64 > MAX_RASTER_BYTES / bpc as u64 {
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
    let encoded_row = |index: usize| -> Result<&[u8], RasterDecodeError> {
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
        bytes.get(start..end).ok_or_else(|| invalid("row outside file"))
    };
    for index in 0..rows {
        request.check_cancelled()?;
        row::<false>(encoded_row(index)?, width, bpc, storage == 1, &mut [])?;
    }
    macro_rules! pixels {
        ($alpha:expr, $variant:ident, $convert:expr) => {{
            let mut pixels = buffer(count, $alpha, request)?;
            let mut values = buffer(width, 0u16, request)?;
            let output = pixels.get_mut().unwrap();
            for channel in 0..channels {
                for y in 0..height {
                    request.check_cancelled()?;
                    let index = channel * height + y;
                    row::<true>(
                        encoded_row(index)?,
                        width,
                        bpc,
                        storage == 1,
                        values.get_mut().unwrap(),
                    )?;
                    for (x, &value) in values.iter().enumerate() {
                        let dest = ((height - 1 - y) * width + x) * 4;
                        let value = ($convert)(value);
                        if channels == 1 {
                            output[dest..dest + 3].fill(value);
                        } else {
                            output[dest + channel] = value;
                        }
                    }
                }
            }

            RasterPixels::$variant(pixels)
        }};
    }
    let pixels = if bpc == 1 {
        pixels!(255u8, Rgba8, |v: u16| v as u8)
    } else {
        pixels!(65535u16, Rgba16, std::convert::identity::<u16>)
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
    #[test]
    fn malformed_rle_tables_and_packets_precede_output_admission() {
        let budget = rrrah_core::MemoryBudget::new(4096);
        let pressure = budget.try_reserve(4096).unwrap();
        let mut request = DecodeRequest::new("invalid.sgi");
        request.memory_budget = Some(budget.clone());
        for bpc in [1u8, 2] {
            let wrap = |words: &[u16]| {
                let payload: Vec<_> = words
                    .iter()
                    .flat_map(|&v| {
                        if bpc == 1 {
                            vec![v as u8]
                        } else {
                            v.to_be_bytes().to_vec()
                        }
                    })
                    .collect();
                let mut bytes = header(bpc, true);
                for v in [528u32, 528, payload.len() as u32, payload.len() as u32] {
                    bytes.extend(v.to_be_bytes());
                }
                bytes.extend(payload);
                bytes
            };
            let valid = wrap(&[2, 37, 0]);
            assert!(matches!(
                decode(&valid, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            for end in 0..valid.len() {
                assert!(
                    matches!(
                        decode(&valid[..end], &request),
                        Err(RasterDecodeError::InvalidSgi(_))
                    ),
                    "bpc={bpc}, end={end}"
                );
            }
            for words in [
                &[3, 37, 0][..],
                &[2, 37][..],
                &[1, 37, 0][..],
                &[130, 37][..],
                &[2, 37, 0, 0][..],
            ] {
                assert!(matches!(
                    decode(&wrap(words), &request),
                    Err(RasterDecodeError::InvalidSgi(_))
                ));
            }
            for offset in [511u32, u32::MAX] {
                let mut damaged = valid.clone();
                damaged[512..516].copy_from_slice(&offset.to_be_bytes());
                assert!(matches!(
                    decode(&damaged, &request),
                    Err(RasterDecodeError::InvalidSgi(_))
                ));
            }
            assert_eq!(budget.used(), 4096);
        }
        drop(pressure);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn pillow_rgb_and_rgba_oracles_match_with_exact_managed_capacity() {
        for (source, oracle) in [
            (
                include_bytes!("../../../tests/fixtures/raster/pattern.rgb").as_slice(),
                include_bytes!("../../../tests/fixtures/raster/pattern.rgb.rgba").as_slice(),
            ),
            (
                include_bytes!("../../../tests/fixtures/raster/pattern.rgba").as_slice(),
                include_bytes!("../../../tests/fixtures/raster/pattern.rgba.rgba").as_slice(),
            ),
        ] {
            let budget = rrrah_core::MemoryBudget::new(1056);
            let mut request = DecodeRequest::new("pillow.sgi");
            request.memory_budget = Some(budget.clone());
            let frame = decode(source, &request).unwrap();
            let RasterPixels::Rgba8(pixels) = frame.pixels() else {
                panic!()
            };
            assert_eq!(&pixels[..], oracle);
            assert_eq!((budget.used(), budget.peak()), (1024, 1056));
            drop(frame);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn managed_native_output_and_row_scratch_exact_short_and_shared_lifetime() {
        for bpc in [1u8, 2] {
            for rle in [false, true] {
                let mut bytes = header(bpc, rle);
                let encode = |v: u16| {
                    if bpc == 1 {
                        vec![v as u8]
                    } else {
                        v.to_be_bytes().to_vec()
                    }
                };
                if rle {
                    let first: Vec<_> = [130u16, 1, 2, 0].into_iter().flat_map(encode).collect();
                    let second: Vec<_> = [130u16, 3, 4, 0].into_iter().flat_map(encode).collect();
                    for v in [
                        528u32,
                        528 + first.len() as u32,
                        first.len() as u32,
                        second.len() as u32,
                    ] {
                        bytes.extend(v.to_be_bytes());
                    }
                    bytes.extend(first);
                    bytes.extend(second);
                } else {
                    bytes.extend([1u16, 2, 3, 4].into_iter().flat_map(encode));
                }
                let output = 16 * u64::from(bpc);
                let budget = rrrah_core::MemoryBudget::new(output + 4);
                let mut request = DecodeRequest::new("managed.sgi");
                request.memory_budget = Some(budget.clone());
                let frame = decode(&bytes, &request).unwrap();
                assert_eq!((budget.used(), budget.peak()), (output, output + 4));
                match frame.pixels() {
                    RasterPixels::Rgba8(p) => assert_eq!(&p[..4], &[3, 3, 3, 255]),
                    RasterPixels::Rgba16(p) => assert_eq!(&p[..4], &[3, 3, 3, 65535]),
                    _ => panic!(),
                }
                let shared = frame.clone();
                drop(frame);
                assert_eq!(budget.used(), output);
                drop(shared);
                assert_eq!(budget.used(), 0);
                let short = rrrah_core::MemoryBudget::new(output + 3);
                request.memory_budget = Some(short.clone());
                assert!(matches!(
                    decode(&bytes, &request),
                    Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
                ));
                assert_eq!(short.used(), 0);
            }
        }
    }
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
            assert!(row::<true>(bad, 2, 1, true, &mut out).is_err());
        }
        row::<true>(&[0, 130, 0, 1, 1, 1, 0, 0], 2, 2, true, &mut out).unwrap();
        assert_eq!(out, [1, 257]);
    }
}
