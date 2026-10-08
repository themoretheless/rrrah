//! Debevec/Netpbm bottom-up PFM. Scale describes sample units and is
//! preserved as metadata. The indistinguishable Adobe top-down dialect
//! requires an explicit future import option, rather than a heuristic flip.

use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};

fn line<'a>(bytes: &mut &'a [u8]) -> Result<&'a str, RasterDecodeError> {
    let end = bytes
        .iter()
        .take(256)
        .position(|b| *b == b'\n')
        .ok_or(RasterDecodeError::InvalidPfm("missing or oversized header line"))?;
    let text =
        std::str::from_utf8(&bytes[..end]).map_err(|_| RasterDecodeError::InvalidPfm("non-ASCII header"))?;
    *bytes = &bytes[end + 1..];
    Ok(text.trim_end_matches('\r'))
}

pub(crate) fn decode(mut bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    let channels = match line(&mut bytes)? {
        "PF" => 3_usize,
        "Pf" => 1,
        _ => return Err(RasterDecodeError::InvalidPfm("invalid magic")),
    };
    let mut dimensions = line(&mut bytes)?.split_ascii_whitespace();
    let mut dimension = || {
        dimensions
            .next()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v != 0)
            .ok_or(RasterDecodeError::InvalidPfm("invalid dimensions"))
    };
    let width = dimension()?;
    let height = dimension()?;
    if dimensions.next().is_some() {
        return Err(RasterDecodeError::InvalidPfm("extra dimensions"));
    }
    let scale = line(&mut bytes)?
        .parse::<f32>()
        .map_err(|_| RasterDecodeError::InvalidPfm("invalid scale"))?;
    if !scale.is_finite() || scale == 0.0 {
        return Err(RasterDecodeError::InvalidPfm("invalid scale"));
    }
    let pixel_count = u64::from(width) * u64::from(height);
    if pixel_count > MAX_RASTER_BYTES / 16 {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let pixel_count = usize::try_from(pixel_count).map_err(|_| RasterDecodeError::OutputTooLarge)?;
    if bytes.len() != pixel_count * channels * 4 {
        return Err(RasterDecodeError::InvalidPfm(
            "raster length does not match dimensions",
        ));
    }
    let reservation = request
        .memory_budget
        .as_ref()
        .map(|budget| budget.try_reserve(pixel_count as u64 * 16))
        .transpose()
        .map_err(|e| RasterDecodeError::Source(crate::DecodeError::Memory(e)))?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(pixel_count * 4)
        .map_err(|_| RasterDecodeError::OutputTooLarge)?;
    let row_bytes = width as usize * channels * 4;
    for row in bytes.chunks_exact(row_bytes).rev() {
        request.check_cancelled()?;
        for (index, pixel) in row.chunks_exact(channels * 4).enumerate() {
            if index.is_multiple_of(4096) {
                request.check_cancelled()?;
            }
            let sample = |channel: usize| {
                let start = channel * 4;
                let raw = [pixel[start], pixel[start + 1], pixel[start + 2], pixel[start + 3]];
                if scale < 0.0 {
                    f32::from_le_bytes(raw)
                } else {
                    f32::from_be_bytes(raw)
                }
            };
            if channels == 1 {
                let gray = sample(0);
                pixels.extend_from_slice(&[gray, gray, gray, 1.0]);
            } else {
                pixels.extend_from_slice(&[sample(0), sample(1), sample(2), 1.0]);
            }
        }
    }
    Ok(DecodedRaster::new(
        width,
        height,
        RasterPixels::Rgba32Float(crate::raster::adopt_raster_output(pixels, reservation)?),
        request.qualify_linear_color(RasterColorSpace::LinearRgbUnspecified),
    )?
    .with_sample_scale(scale.abs())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_is_admitted_before_allocation_and_released_with_last_owner() {
        let mut bytes = b"PF\n1 1\n-1\n".to_vec();
        for v in [8.0f32, -0.0, -2.0] {
            bytes.extend(v.to_le_bytes());
        }
        let mut request = DecodeRequest::new("unused.pfm");
        let short = rrrah_core::MemoryBudget::new(15);
        request.memory_budget = Some(short.clone());
        assert!(matches!(
            decode(&bytes, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!(short.used(), 0);
        assert_eq!(short.peak(), 0);
        let exact = rrrah_core::MemoryBudget::new(16);
        request.memory_budget = Some(exact.clone());
        let frame = decode(&bytes, &request).unwrap();
        let RasterPixels::Rgba32Float(pixels) = frame.pixels() else {
            panic!()
        };
        assert!(pixels.is_managed());
        assert_eq!(pixels[0].to_bits(), 8.0f32.to_bits());
        assert_eq!(pixels[1].to_bits(), (-0.0f32).to_bits());
        assert_eq!(exact.used(), 16);
        let retained = frame.clone();
        drop(frame);
        assert_eq!(exact.used(), 16);
        drop(retained);
        assert_eq!(exact.used(), 0);
    }

    #[test]
    fn both_endians_reverse_rows_and_preserve_hdr_and_scale() {
        for little in [false, true] {
            let mut bytes = format!("PF\n1 2\n{}2.5\n", if little { "-" } else { "" }).into_bytes();
            for value in [4.0_f32, -1.0, 0.5, 0.25, 2.0, 8.0] {
                bytes.extend_from_slice(&if little {
                    value.to_le_bytes()
                } else {
                    value.to_be_bytes()
                });
            }
            let frame = decode(&bytes, &DecodeRequest::new("unused.pfm")).unwrap();
            let RasterPixels::Rgba32Float(p) = frame.pixels() else {
                panic!("precision lost")
            };
            assert_eq!(p.as_slice(), [0.25, 2.0, 8.0, 1.0, 4.0, -1.0, 0.5, 1.0]);
            assert_eq!(frame.sample_scale(), 2.5);
        }
    }

    #[test]
    fn grayscale_preserves_binary_whitespace_and_special_values() {
        let mut bytes = b"Pf\r\n2 1\r\n-1\r\n".to_vec();
        // First payload byte is whitespace; it must not be skipped.
        let value = f32::from_bits(0x3f80_0020);
        bytes.extend_from_slice(&value.to_le_bytes());
        bytes.extend_from_slice(&f32::INFINITY.to_le_bytes());
        let frame = decode(&bytes, &DecodeRequest::new("unused")).unwrap();
        let RasterPixels::Rgba32Float(p) = frame.pixels() else {
            panic!("precision lost")
        };
        assert_eq!(&p[..4], &[value, value, value, 1.0]);
        assert!(p[4].is_infinite());
    }

    #[test]
    fn malformed_inputs_never_allocate_large_output() {
        for bytes in [
            b"PF\n0 1\n-1\n".as_slice(),
            b"PF\n1 1\n0\n",
            b"PF\n1 1\nNaN\n",
            b"PF\n1 1\n-1\n",
            b"PF\n4294967295 4294967295\n1\n",
        ] {
            assert!(decode(bytes, &DecodeRequest::new("unused")).is_err());
        }
    }
}
