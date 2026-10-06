//! Selected TIFF directory alpha declaration and in-place straight-alpha conversion.
use crate::{DecodeRequest, raster::RasterDecodeError};
fn bad(reason: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidTiffAlpha(reason)
}
pub(super) fn associated(bytes: &[u8], request: &DecodeRequest) -> Result<bool, RasterDecodeError> {
    let little = bytes.starts_with(b"II");
    let number = |at: usize, count: usize| -> Result<u64, RasterDecodeError> {
        let data = bytes
            .get(at..at.checked_add(count).ok_or_else(|| bad("offset overflow"))?)
            .ok_or_else(|| bad("truncated directory"))?;
        Ok(if little {
            data.iter().rev().fold(0, |n, b| (n << 8) | u64::from(*b))
        } else {
            data.iter().fold(0, |n, b| (n << 8) | u64::from(*b))
        })
    };
    let (offset_at, pointer, count_size, entry_size, value_at) = match number(2, 2)? {
        42 => (4, 4, 2, 12, 8),
        43 => (8, 8, 8, 20, 12),
        _ => return Err(bad("TIFF version")),
    };
    let directory = usize::try_from(number(offset_at, pointer)?).map_err(|_| bad("directory offset"))?;
    let count = usize::try_from(number(directory, count_size)?).map_err(|_| bad("entry count"))?;
    if count > 4096 {
        return Err(bad("excessive directory entries"));
    }
    let mut result = None;
    for index in 0..count {
        request.check_cancelled()?;
        let at = index
            .checked_mul(entry_size)
            .and_then(|n| n.checked_add(count_size))
            .and_then(|n| n.checked_add(directory))
            .ok_or_else(|| bad("entry offset"))?;
        if number(at, 2)? != 338 {
            continue;
        }
        if result.is_some() || number(at + 2, 2)? != 3 || number(at + 4, pointer)? != 1 {
            return Err(bad("unsupported or duplicate ExtraSamples"));
        }
        result = Some(match number(at + value_at, 2)? {
            0 | 2 => false,
            1 => true,
            _ => return Err(bad("unknown ExtraSamples")),
        });
    }
    Ok(result.unwrap_or(false))
}
pub(super) fn u8_pixels(pixels: &mut [u8], request: &DecodeRequest) -> Result<(), RasterDecodeError> {
    let (pixels, remainder) = pixels.as_chunks_mut::<4>();
    if !remainder.is_empty() {
        return Err(bad("incomplete RGBA pixel"));
    }
    for (index, pixel) in pixels.iter_mut().enumerate() {
        if index % 4096 == 0 {
            request.check_cancelled()?;
        }
        let alpha = u32::from(pixel[3]);
        for channel in &mut pixel[..3] {
            if u32::from(*channel) > alpha {
                return Err(bad("associated RGB exceeds alpha"));
            }
            *channel = u8::try_from(
                (u32::from(*channel) * 255 + alpha / 2)
                    .checked_div(alpha)
                    .unwrap_or(0),
            )
            .map_err(|_| bad("unassociation overflow"))?;
        }
    }
    Ok(())
}
pub(super) fn u16_pixels(pixels: &mut [u16], request: &DecodeRequest) -> Result<(), RasterDecodeError> {
    let (pixels, remainder) = pixels.as_chunks_mut::<4>();
    if !remainder.is_empty() {
        return Err(bad("incomplete RGBA pixel"));
    }
    for (index, pixel) in pixels.iter_mut().enumerate() {
        if index % 4096 == 0 {
            request.check_cancelled()?;
        }
        let alpha = u64::from(pixel[3]);
        for channel in &mut pixel[..3] {
            if u64::from(*channel) > alpha {
                return Err(bad("associated RGB exceeds alpha"));
            }
            *channel = u16::try_from(
                (u64::from(*channel) * 65535 + alpha / 2)
                    .checked_div(alpha)
                    .unwrap_or(0),
            )
            .map_err(|_| bad("unassociation overflow"))?;
        }
    }
    Ok(())
}
pub(super) fn f32_pixels(pixels: &mut [f32], request: &DecodeRequest) -> Result<(), RasterDecodeError> {
    let (pixels, remainder) = pixels.as_chunks_mut::<4>();
    if !remainder.is_empty() {
        return Err(bad("incomplete RGBA pixel"));
    }
    for (index, pixel) in pixels.iter_mut().enumerate() {
        if index % 4096 == 0 {
            request.check_cancelled()?;
        }
        let alpha = pixel[3];
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(bad("invalid alpha"));
        }
        for channel in &mut pixel[..3] {
            if alpha == 0.0 {
                if *channel != 0.0 {
                    return Err(bad("zero-alpha emission"));
                }
                *channel = 0.0;
            } else {
                *channel /= alpha;
                if !channel.is_finite() {
                    return Err(bad("non-finite RGB"));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integer_and_hdr_unassociation_preserve_precision_and_reject_invalid_input() {
        let request = DecodeRequest::new("unused");
        let mut u16_values = [21845, 0, 43690, 43690];
        u16_pixels(&mut u16_values, &request).unwrap();
        assert_eq!(u16_values, [32768, 0, 65535, 43690]);
        let mut hdr = [2.0, -1.0, 0.0, 0.5];
        f32_pixels(&mut hdr, &request).unwrap();
        assert_eq!(hdr, [4.0, -2.0, 0.0, 0.5]);
        assert!(u8_pixels(&mut [86, 0, 0, 85], &request).is_err());
        assert!(u16_pixels(&mut [1, 0, 0, 0], &request).is_err());
        assert!(f32_pixels(&mut [1.0, 0.0, 0.0, 0.0], &request).is_err());
        assert!(f32_pixels(&mut [f32::INFINITY, 0.0, 0.0, 0.5], &request).is_err());
    }
}
