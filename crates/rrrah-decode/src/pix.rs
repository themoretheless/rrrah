//! Alias/Wavefront PIX row RLE (8-bit gray or 24-bit BGR).
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidPix(s)
}
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    let header = bytes.get(..10).ok_or_else(|| bad("truncated header"))?;
    let word = |i| u16::from_be_bytes([header[i], header[i + 1]]);
    let (width, height) = (u32::from(word(0)), u32::from(word(2)));
    let depth = word(8);
    if width == 0 || height == 0 || !matches!(depth, 8 | 24) {
        return Err(bad("dimensions or pixel depth"));
    }
    let count = u64::from(width) * u64::from(height);
    if count * 4 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let mut output = Vec::with_capacity(count as usize * 4);
    let mut at = 10;
    let mut x = 0u32;
    let packet = if depth == 24 { 4 } else { 2 };
    while output.len() < count as usize * 4 {
        request.check_cancelled()?;
        let run = bytes.get(at..at + packet).ok_or_else(|| bad("truncated run"))?;
        at += packet;
        let n = u32::from(run[0]);
        if n == 0 || x + n > width {
            return Err(bad("run exceeds row"));
        }
        let pixel = if depth == 24 {
            [run[3], run[2], run[1], 255]
        } else {
            [run[1], run[1], run[1], 255]
        };
        for _ in 0..n {
            output.extend_from_slice(&pixel);
        }
        x += n;
        if x == width {
            x = 0;
        }
    }
    if at != bytes.len() {
        return Err(bad("trailing pixels"));
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
    #[test]
    fn malformed_runs_are_rejected() {
        for run in [&[0, 17][..], &[3, 17], &[2][..], &[1, 17, 1][..]] {
            let mut bytes = vec![0, 2, 0, 1, 0, 0, 0, 0, 0, 8];
            bytes.extend(run);
            assert!(decode(&bytes, &DecodeRequest::new("x.pix")).is_err());
        }
    }
    #[test]
    fn maximum_run_and_row_reset() {
        let bytes = [1, 0, 0, 2, 0, 0, 0, 0, 0, 8, 255, 17, 1, 29, 255, 43, 1, 61];
        let frame = decode(&bytes, &DecodeRequest::new("x.pix")).unwrap();
        let RasterPixels::Rgba8(p) = frame.pixels() else {
            panic!()
        };
        assert_eq!(
            &p[254 * 4..257 * 4],
            &[17, 17, 17, 255, 29, 29, 29, 255, 43, 43, 43, 255]
        );
    }
}
