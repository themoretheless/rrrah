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

fn buffer(count: usize, request: &DecodeRequest) -> Result<rrrah_core::PixelBuffer<u8>, RasterDecodeError> {
    Ok(match &request.memory_budget {
        Some(budget) => budget
            .try_buffer(count, 0u8)
            .map_err(crate::DecodeError::Memory)?
            .freeze()
            .into(),
        None => {
            let mut values = Vec::new();
            values
                .try_reserve_exact(count)
                .map_err(|_| RasterDecodeError::OutputTooLarge)?;
            values.resize(count, 0);
            Arc::new(values).into()
        }
    })
}
fn rle(
    mut input: &[u8],
    size: usize,
    request: &DecodeRequest,
    mut emit: impl FnMut(usize, usize, u8) -> Result<(), RasterDecodeError>,
) -> Result<(), RasterDecodeError> {
    let (mut position, mut packets) = (0usize, 0usize);
    while !input.is_empty() {
        if packets % 4096 == 0 {
            request.check_cancelled()?;
        }
        packets += 1;
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
        if count > size - position {
            return Err(invalid("RLE exceeds image size"));
        }
        emit(position, count, value)?;
        position += count;
    }
    if position != size {
        return Err(invalid("short RLE image"));
    }
    Ok(())
}
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
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
    if kind == 2 {
        rle(encoded, raw_size, request, |at, count, value| {
            if !palette.is_empty()
                && usize::from(value) >= map_len / 3
                && (at..at + count).any(|i| i % stride < width as usize)
            {
                return Err(invalid("palette index outside map"));
            }
            Ok(())
        })?;
    } else {
        if encoded.len() != raw_size {
            return Err(invalid("invalid row storage length"));
        }
        if !palette.is_empty() {
            for row in encoded.chunks_exact(stride) {
                for chunk in row[..width as usize].chunks(4096) {
                    request.check_cancelled()?;
                    if chunk.iter().any(|&v| usize::from(v) >= map_len / 3) {
                        return Err(invalid("palette index outside map"));
                    }
                }
            }
        }
    }
    let mut output = buffer(pixel_count as usize * 4, request)?;
    let pixels = output.get_mut().unwrap();
    let convert = |row: &[u8], pixels: &mut [u8]| -> Result<(), RasterDecodeError> {
        request.check_cancelled()?;
        for x in 0..width as usize {
            if x % 4096 == 0 {
                request.check_cancelled()?;
            }
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
            pixels[x * 4..x * 4 + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }

        Ok(())
    };
    let output_stride = width as usize * 4;
    if kind == 2 {
        let mut scratch = buffer(stride, request)?;
        let row = scratch.get_mut().unwrap();
        rle(encoded, raw_size, request, |mut at, mut count, value| {
            while count > 0 {
                let column = at % stride;
                let take = count.min(stride - column);
                row[column..column + take].fill(value);
                if column + take == stride {
                    let begin = (at / stride) * output_stride;
                    convert(row, &mut pixels[begin..begin + output_stride])?;
                }
                at += take;
                count -= take;
            }
            Ok(())
        })?;
    } else {
        for (row, output) in encoded
            .chunks_exact(stride)
            .zip(pixels.chunks_exact_mut(output_stride))
        {
            convert(row, output)?;
        }
    }
    Ok(DecodedRaster::new(
        width,
        height,
        RasterPixels::Rgba8(output),
        RasterColorSpace::AssumedSrgb,
    )?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "manual large Sun RLE row-scratch and complete pixel verification"]
    fn large_rle_image_uses_source_output_and_one_row_budget() {
        let (width, height) = (4097u32, 2048u32);
        let stride = (u64::from(width) * 24).div_ceil(16) * 2;
        let raw_bytes = stride * u64::from(height);
        let output_bytes = u64::from(width) * u64::from(height) * 4;
        let mut encoded = Vec::new();
        let mut remaining = raw_bytes;
        while remaining > 0 {
            let take = remaining.min(256);
            if take == 1 {
                encoded.push(77);
            } else {
                encoded.extend_from_slice(&[128, (take - 1) as u8, 77]);
            }
            remaining -= take;
        }
        let mut bytes = fixture(width, 24, 2, &encoded);
        bytes[8..12].copy_from_slice(&height.to_be_bytes());
        drop(encoded);
        let source_bytes = bytes.len() as u64;
        let cap = source_bytes + output_bytes + stride;
        let budget = rrrah_core::MemoryBudget::new(cap);
        let mut source = budget.try_buffer(bytes.len(), 0u8).unwrap();
        source[..].copy_from_slice(&bytes);
        drop(bytes);
        let source = source.freeze();
        let mut request = DecodeRequest::new("large.ras");
        request.memory_budget = Some(budget.clone());
        for run in 0..3 {
            let started = std::time::Instant::now();
            let frame = decode(&source, &request).unwrap();
            let elapsed = started.elapsed().as_nanos();
            let RasterPixels::Rgba8(pixels) = frame.pixels() else {
                panic!()
            };
            assert!(pixels.chunks_exact(4).all(|v| v == [77, 77, 77, 255]));
            assert_eq!((budget.used(), budget.peak()), (source_bytes + output_bytes, cap));
            drop(frame);
            assert_eq!(budget.used(), source_bytes);
            eprintln!(
                "SUN_LARGE run={run} width={width} height={height} source_bytes={source_bytes} raw_bytes={raw_bytes} output_bytes={output_bytes} scratch={stride} cap={cap} decode_ns={elapsed}"
            );
        }
        let pressure = budget.try_reserve(1).unwrap();
        assert!(matches!(
            decode(&source, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!(budget.used(), source_bytes + 1);
        drop(pressure);
        drop(source);
        assert_eq!(budget.used(), 0);
        eprintln!("SUN_LARGE final=0 short_budget_refused=true");
    }
    #[test]
    fn long_rle_runs_cross_many_padded_rows_with_one_row_scratch() {
        let mut bytes = fixture(3, 8, 2, &[128, 255, 77, 128, 255, 77, 128, 3, 77]);
        bytes[8..12].copy_from_slice(&129u32.to_be_bytes());
        let budget = rrrah_core::MemoryBudget::new(1548 + 4);
        let mut request = DecodeRequest::new("many-rows.ras");
        request.memory_budget = Some(budget.clone());
        let frame = decode(&bytes, &request).unwrap();
        let RasterPixels::Rgba8(p) = frame.pixels() else {
            panic!()
        };
        assert_eq!(p.len(), 1548);
        assert!(p.chunks_exact(4).all(|v| v == [77, 77, 77, 255]));
        assert_eq!((budget.used(), budget.peak()), (1548, 1552));
        drop(frame);
        assert_eq!(budget.used(), 0);
        let short = rrrah_core::MemoryBudget::new(1551);
        request.memory_budget = Some(short.clone());
        assert!(matches!(
            decode(&bytes, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!(short.used(), 0);
    }
    #[test]
    fn palette_indices_validate_before_admission_but_padding_is_not_a_pixel() {
        fn mapped(kind: u32, height: u32, data: &[u8]) -> Vec<u8> {
            let mut b = fixture(1, 8, kind, data);
            b[8..12].copy_from_slice(&height.to_be_bytes());
            b[24..28].copy_from_slice(&1u32.to_be_bytes());
            b[28..32].copy_from_slice(&3u32.to_be_bytes());
            b.splice(32..32, [4, 5, 6]);
            b
        }
        let budget = rrrah_core::MemoryBudget::new(0);
        let mut request = DecodeRequest::new("palette.ras");
        request.memory_budget = Some(budget.clone());
        for kind in [1, 2] {
            assert!(matches!(
                decode(&mapped(kind, 1, &[1, 0]), &request),
                Err(RasterDecodeError::InvalidSun("palette index outside map"))
            ));
            // Padding99 is outside the one-entry palette but is not displayed.
            let valid = mapped(kind, 1, &[0, 99]);
            assert!(matches!(
                decode(&valid, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            let mut admitted = request.clone();
            admitted.memory_budget = Some(rrrah_core::MemoryBudget::new(if kind == 2 { 6 } else { 4 }));
            let frame = decode(&valid, &admitted).unwrap();
            let RasterPixels::Rgba8(p) = frame.pixels() else {
                panic!()
            };
            assert_eq!(&p[..], &[4, 5, 6, 255]);
        }
        assert!(matches!(
            decode(&mapped(2, 2, &[0, 128, 1, 99, 0]), &request),
            Err(RasterDecodeError::InvalidSun("palette index outside map"))
        ));
        assert_eq!((budget.used(), budget.peak()), (0, 0));
    }
    #[test]
    fn independent_oracles_admit_exact_rle_and_output_buffers() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let mut cases = 0;
        for line in include_str!("../../../tests/fixtures/raster/manifest.tsv").lines() {
            let f: Vec<_> = line.split('\t').collect();
            if !f[0].ends_with(".ras") {
                continue;
            }
            let source = std::fs::read(root.join(f[0])).unwrap();
            let oracle = std::fs::read(root.join(f[4])).unwrap();
            let scratch = if long(&source, 20) == 2 {
                (u64::from(long(&source, 4)) * u64::from(long(&source, 12))).div_ceil(16) * 2
            } else {
                0
            };
            let output = oracle.len() as u64;
            let budget = rrrah_core::MemoryBudget::new(output + scratch);
            let mut request = DecodeRequest::new(f[0]);
            request.memory_budget = Some(budget.clone());
            let frame = decode(&source, &request).unwrap();
            let RasterPixels::Rgba8(pixels) = frame.pixels() else {
                panic!()
            };
            assert_eq!(&pixels[..], &oracle);
            assert_eq!((budget.used(), budget.peak()), (output, output + scratch));
            let shared = frame.clone();
            drop(frame);
            assert_eq!(budget.used(), output);
            drop(shared);
            assert_eq!(budget.used(), 0);
            let short = rrrah_core::MemoryBudget::new(output + scratch - 1);
            request.memory_budget = Some(short.clone());
            assert!(matches!(
                decode(&source, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(short.used(), 0);
            cases += 1;
        }
        assert_eq!(cases, 8);
    }
    #[test]
    fn malformed_rle_precedes_managed_admission() {
        let mut request = DecodeRequest::new("invalid.ras");
        let budget = rrrah_core::MemoryBudget::new(0);
        request.memory_budget = Some(budget.clone());
        for encoded in [&[128][..], &[128, 2, 7][..], &[1][..]] {
            assert!(matches!(
                decode(&fixture(2, 8, 2, encoded), &request),
                Err(RasterDecodeError::InvalidSun(_))
            ));
        }
        assert_eq!(budget.peak(), 0);
    }
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
