//! Native QOI decode with strict framing in the same pass and admitted output.
use crate::{DecodeRequest, raster::RasterDecodeError};

pub(crate) fn decode(
    bytes: &[u8],
    request: &DecodeRequest,
) -> Result<rrrah_core::DecodedRaster, RasterDecodeError> {
    decode_with_check(bytes, request, &|| request.check_cancelled())
}

fn decode_with_check(
    bytes: &[u8],
    request: &DecodeRequest,
    check: &impl Fn() -> Result<(), crate::DecodeError>,
) -> Result<rrrah_core::DecodedRaster, RasterDecodeError> {
    use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
    let invalid = RasterDecodeError::InvalidQoi;
    check()?;
    if bytes.len() < 22 || !bytes.starts_with(b"qoif") {
        return Err(invalid("truncated header or end marker"));
    }
    let width = u32::from_be_bytes(bytes[4..8].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[8..12].try_into().unwrap());
    if width == 0
        || height == 0
        || width > 65536
        || height > 65536
        || !matches!(bytes[12], 3 | 4)
        || bytes[13] > 1
    {
        return Err(invalid("invalid dimensions, channels or color flag"));
    }
    let count = u64::from(width) * u64::from(height);
    if count > crate::raster::MAX_RASTER_BYTES / 4 {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let end = bytes.len() - 8;
    if bytes[end..] != [0, 0, 0, 0, 0, 0, 0, 1] {
        return Err(invalid("invalid end marker"));
    }
    // Each command consumes at least one byte and emits at most 62 pixels.
    // This necessary lower bound rejects impossible bodies before output admission.
    if count.div_ceil(62) > (end - 14) as u64 {
        return Err(invalid("pixel stream cannot contain declared pixel count"));
    }
    let reservation = request
        .memory_budget
        .as_ref()
        .map(|budget| budget.try_reserve(count * 4))
        .transpose()
        .map_err(crate::DecodeError::Memory)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(count as usize * 4)
        .map_err(|_| RasterDecodeError::OutputTooLarge)?;
    output.resize(count as usize * 4, 0);
    let mut written = 0usize;
    let mut pixel = [0u8, 0, 0, 255];
    let mut index = [[0u8; 4]; 64];
    let mut cursor = 14;
    while written < count as usize {
        check()?;
        if cursor >= end {
            return Err(invalid("truncated pixel stream"));
        }
        let opcode = bytes[cursor];
        cursor += 1;
        let mut run = 1;
        match opcode {
            0xfe => {
                if end - cursor < 3 {
                    return Err(invalid("truncated pixel command"));
                }
                pixel[..3].copy_from_slice(&bytes[cursor..cursor + 3]);
                cursor += 3;
            }
            0xff => {
                if end - cursor < 4 {
                    return Err(invalid("truncated pixel command"));
                }
                pixel.copy_from_slice(&bytes[cursor..cursor + 4]);
                cursor += 4;
            }
            _ => match opcode >> 6 {
                0 => pixel = index[opcode as usize],
                1 => {
                    pixel[0] = pixel[0].wrapping_add(((opcode >> 4) & 3).wrapping_sub(2));
                    pixel[1] = pixel[1].wrapping_add(((opcode >> 2) & 3).wrapping_sub(2));
                    pixel[2] = pixel[2].wrapping_add((opcode & 3).wrapping_sub(2));
                }
                2 => {
                    if cursor >= end {
                        return Err(invalid("truncated pixel command"));
                    }
                    let second = bytes[cursor];
                    cursor += 1;
                    let dg = (opcode & 63).wrapping_sub(32);
                    pixel[0] = pixel[0]
                        .wrapping_add(dg)
                        .wrapping_add((second >> 4).wrapping_sub(8));
                    pixel[1] = pixel[1].wrapping_add(dg);
                    pixel[2] = pixel[2]
                        .wrapping_add(dg)
                        .wrapping_add((second & 15).wrapping_sub(8));
                }
                _ => run = usize::from(opcode & 63) + 1,
            },
        }
        if run > count as usize - written {
            return Err(invalid("run exceeds declared pixel count"));
        }
        let hash = pixel[0]
            .wrapping_mul(3)
            .wrapping_add(pixel[1].wrapping_mul(5))
            .wrapping_add(pixel[2].wrapping_mul(7))
            .wrapping_add(pixel[3].wrapping_mul(11))
            & 63;
        index[hash as usize] = pixel;
        let (span, remainder) = output[written * 4..(written + run) * 4].as_chunks_mut::<4>();
        debug_assert!(remainder.is_empty());
        span.fill(pixel);
        written += run;
    }
    if cursor != end {
        return Err(invalid("trailing pixel commands or bytes"));
    }
    check()?;
    let pixels = crate::raster::adopt_raster_output(output, reservation)?;
    Ok(DecodedRaster::new(
        width,
        height,
        RasterPixels::Rgba8(pixels),
        if bytes[13] == 0 {
            RasterColorSpace::Srgb
        } else {
            RasterColorSpace::LinearSrgb
        },
    )?)
}

#[cfg(test)]
pub(crate) fn validate(bytes: &[u8], request: &DecodeRequest) -> Result<(), RasterDecodeError> {
    validate_with_check(bytes, &|| request.check_cancelled())
}

#[cfg(test)]
fn validate_with_check(
    bytes: &[u8],
    check: &impl Fn() -> Result<(), crate::DecodeError>,
) -> Result<(), RasterDecodeError> {
    let invalid = RasterDecodeError::InvalidQoi;
    check()?;
    if bytes.len() < 22 || !bytes.starts_with(b"qoif") {
        return Err(invalid("truncated header or end marker"));
    }
    let width = u32::from_be_bytes(bytes[4..8].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[8..12].try_into().unwrap());
    if width == 0 || height == 0 || !matches!(bytes[12], 3 | 4) || bytes[13] > 1 {
        return Err(invalid("invalid dimensions, channels or color flag"));
    }
    let pixels = u64::from(width) * u64::from(height);
    if pixels > crate::raster::MAX_RASTER_BYTES / 4 {
        return Err(invalid("image exceeds raster byte limit"));
    }
    let end = bytes.len() - 8;
    if bytes[end..] != [0, 0, 0, 0, 0, 0, 0, 1] {
        return Err(invalid("invalid end marker"));
    }
    let mut cursor = 14;
    let mut decoded = 0;
    while decoded < pixels {
        check()?;
        if cursor >= end {
            return Err(invalid("truncated pixel stream"));
        }
        let opcode = bytes[cursor];
        let (length, count) = match opcode {
            0xfe => (4, 1),
            0xff => (5, 1),
            _ => match opcode >> 6 {
                2 => (2, 1),
                3 => (1, u64::from(opcode & 63) + 1),
                _ => (1, 1),
            },
        };
        if count > pixels - decoded {
            return Err(invalid("run exceeds declared pixel count"));
        }
        if length > end - cursor {
            return Err(invalid("truncated pixel command"));
        }
        cursor += length;
        decoded += count;
    }
    if cursor != end {
        return Err(invalid("trailing pixel commands or bytes"));
    }
    check()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Warm memory timing of framing versus codec, not file loading or GPU presentation.
    #[test]
    #[ignore = "12 MP QOI timing control; run locally with --ignored --nocapture"]
    fn large_qoi_framing_cost() {
        use std::{hint::black_box, time::Instant};
        for dense in [false, true] {
            let pixels = 4000usize * 3000;
            let mut bytes = b"qoif".to_vec();
            bytes.extend_from_slice(&4000u32.to_be_bytes());
            bytes.extend_from_slice(&3000u32.to_be_bytes());
            bytes.extend_from_slice(&[4, 0]);
            if dense {
                for i in 0..pixels {
                    bytes.extend_from_slice(&[0xff, i as u8, (i >> 8) as u8, (i >> 16) as u8, 255]);
                }
            } else {
                let mut remaining = pixels;
                while remaining != 0 {
                    let run = remaining.min(62);
                    bytes.push(0xc0 | (run - 1) as u8);
                    remaining -= run;
                }
            }
            bytes.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
            let request = DecodeRequest::new("timing.qoi");
            let mut framing = Vec::new();
            let mut codec = Vec::new();
            let mut native = Vec::new();
            for iteration in 0..18 {
                let start = Instant::now();
                validate(black_box(&bytes), black_box(&request)).unwrap();
                let checked = Instant::now();
                let decoded = image::load_from_memory_with_format(black_box(&bytes), image::ImageFormat::Qoi)
                    .unwrap()
                    .into_rgba8();
                let finished = Instant::now();
                assert_eq!(decoded.dimensions(), (4000, 3000));
                for i in [0, 61, 62, pixels / 2, pixels - 1] {
                    let expected = if dense {
                        [i as u8, (i >> 8) as u8, (i >> 16) as u8, 255]
                    } else {
                        [0, 0, 0, 255]
                    };
                    assert_eq!(&decoded.as_raw()[i * 4..i * 4 + 4], &expected);
                }
                let native_start = Instant::now();
                let frame = decode(black_box(&bytes), black_box(&request)).unwrap();
                let native_end = Instant::now();
                let rrrah_core::RasterPixels::Rgba8(values) = frame.pixels() else {
                    panic!()
                };
                assert_eq!(&values[..], &decoded.as_raw()[..]);
                black_box(frame);
                black_box(decoded);
                if iteration >= 3 {
                    native.push((native_end - native_start).as_secs_f64() * 1000.0);
                    framing.push((checked - start).as_secs_f64() * 1000.0);
                    codec.push((finished - checked).as_secs_f64() * 1000.0);
                }
            }
            framing.sort_by(f64::total_cmp);
            codec.sort_by(f64::total_cmp);
            native.sort_by(f64::total_cmp);
            println!(
                "QOI_COST dense={dense} pixels={pixels} bytes={} samples=15 framing_p50_ms={:.6} framing_p95_ms={:.6} codec_p50_ms={:.6} codec_p95_ms={:.6} native_p50_ms={:.6} native_p95_ms={:.6}",
                bytes.len(),
                framing[7],
                framing[14],
                codec[7],
                codec[14],
                native[7],
                native[14]
            );
        }
    }

    #[test]
    fn impossible_qoi_pixel_counts_reject_before_output_admission() {
        for (width, height, body) in [
            (1u32, 1u32, &[][..]),
            (62, 1, &[][..]),
            (63, 1, &[0xfd][..]),
            (4000, 3000, &[0xfd][..]),
        ] {
            let mut bytes = b"qoif".to_vec();
            bytes.extend_from_slice(&width.to_be_bytes());
            bytes.extend_from_slice(&height.to_be_bytes());
            bytes.extend_from_slice(&[4, 0]);
            bytes.extend_from_slice(body);
            bytes.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
            let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
            let mut request = DecodeRequest::new("impossible.qoi");
            request.memory_budget = Some(budget.clone());
            assert!(matches!(
                decode(&bytes, &request),
                Err(RasterDecodeError::InvalidQoi(_))
            ));
            assert_eq!(budget.used(), 0);
            assert_eq!(budget.peak(), 0, "output admitted for {width}x{height}");
        }
        for count in [1u32, 61, 62, 63, 124] {
            let mut bytes = b"qoif".to_vec();
            bytes.extend_from_slice(&count.to_be_bytes());
            bytes.extend_from_slice(&1u32.to_be_bytes());
            bytes.extend_from_slice(&[4, 0]);
            let mut remaining = count;
            while remaining != 0 {
                let run = remaining.min(62);
                bytes.push(0xc0 | (run as u8 - 1));
                remaining -= run;
            }
            bytes.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
            let frame = decode(&bytes, &DecodeRequest::new("minimum.qoi")).unwrap();
            let rrrah_core::RasterPixels::Rgba8(pixels) = frame.pixels() else {
                panic!()
            };
            assert!(pixels.chunks_exact(4).all(|p| p == [0, 0, 0, 255]));
        }
    }

    #[test]
    fn qoi_channel_header_does_not_discard_encoded_alpha() {
        let source = include_bytes!("../../../tests/fixtures/qoi/all-opcodes-0.qoi");
        let expected = include_bytes!("../../../tests/fixtures/qoi/all-opcodes-0.rgba");
        for channels in [3, 4] {
            for flag in [0, 1] {
                let mut bytes = source.to_vec();
                bytes[12] = channels;
                bytes[13] = flag;
                let frame = decode(&bytes, &DecodeRequest::new("channels.qoi")).unwrap();
                let rrrah_core::RasterPixels::Rgba8(pixels) = frame.pixels() else {
                    panic!()
                };
                assert_eq!(&pixels[..], &expected[..]);
                assert_eq!(
                    frame.color_space(),
                    &if flag == 0 {
                        rrrah_core::RasterColorSpace::Srgb
                    } else {
                        rrrah_core::RasterColorSpace::LinearSrgb
                    }
                );
            }
        }
    }

    #[test]
    fn native_qoi_admits_output_and_releases_cancelled_partial_decode() {
        let bytes = include_bytes!("../../../tests/fixtures/qoi/all-opcodes-0.qoi");
        let budget = rrrah_core::MemoryBudget::new(272);
        let mut request = DecodeRequest::new("admitted.qoi");
        request.memory_budget = Some(budget.clone());
        let frame = decode(bytes, &request).unwrap();
        assert_eq!(budget.used(), 272);
        let clone = frame.clone();
        drop(frame);
        assert_eq!(budget.used(), 272);
        assert!(matches!(
            decode(bytes, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        drop(clone);
        assert_eq!(budget.used(), 0);
        for target in 1..=9 {
            let polls = std::cell::Cell::new(0);
            let result = decode_with_check(bytes, &request, &|| {
                polls.set(polls.get() + 1);
                if polls.get() == target {
                    Err(crate::DecodeError::Cancelled)
                } else {
                    Ok(())
                }
            });
            assert!(matches!(
                result,
                Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
            ));
            assert_eq!(polls.get(), target);
            assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn qoi_validation_cancels_at_every_command_checkpoint() {
        let bytes = include_bytes!("../../../tests/fixtures/qoi/all-opcodes-0.qoi");
        let polls = std::cell::Cell::new(0);
        validate_with_check(bytes, &|| {
            polls.set(polls.get() + 1);
            Ok(())
        })
        .unwrap();
        let total = polls.get();
        assert!(total >= 8);
        for target in 1..=total {
            polls.set(0);
            let result = validate_with_check(bytes, &|| {
                polls.set(polls.get() + 1);
                if polls.get() >= target {
                    Err(crate::DecodeError::Cancelled)
                } else {
                    Ok(())
                }
            });
            assert!(matches!(
                result,
                Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
            ));
            assert_eq!(polls.get(), target);
        }
    }
}
