//! `ZSoft` PCX: row-planar pixels and byte RLE, bounded before allocation.
//! Reference: <https://techheap.packetizer.com/compression/graphics/pcxfmt.html>

use std::sync::Arc;

use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};

use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};

fn invalid(message: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidPcx(message)
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
fn scanline<const EMIT: bool>(
    encoded: &[u8],
    offset: &mut usize,
    row_len: usize,
    packed: bool,
    output: &mut [u8],
    request: &DecodeRequest,
) -> Result<(), RasterDecodeError> {
    if !packed {
        let end = (*offset)
            .checked_add(row_len)
            .ok_or_else(|| invalid("scanline offset overflow"))?;
        let row = encoded
            .get(*offset..end)
            .ok_or_else(|| invalid("truncated raster"))?;
        if EMIT {
            output.copy_from_slice(row);
        }
        *offset = end;
        return Ok(());
    }
    let (mut filled, mut packets) = (0usize, 0usize);
    while filled < row_len {
        if packets % 4096 == 0 {
            request.check_cancelled()?;
        }
        packets += 1;
        let marker = *encoded.get(*offset).ok_or_else(|| invalid("truncated raster"))?;
        *offset += 1;
        let (count, value) = if packed && marker & 0xc0 == 0xc0 {
            let count = usize::from(marker & 0x3f);
            if count == 0 {
                return Err(invalid("zero-length RLE run"));
            }
            let value = *encoded.get(*offset).ok_or_else(|| invalid("truncated RLE run"))?;
            *offset += 1;
            (count, value)
        } else {
            (1, marker)
        };
        if count > row_len - filled {
            return Err(invalid("RLE run crosses a scanline"));
        }
        if EMIT {
            output[filled..filled + count].fill(value);
        }
        filled += count;
    }
    Ok(())
}
pub fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    decode_internal(bytes, request, false)?.ok_or_else(|| invalid("missing decoded raster"))
}

/// Validate the complete external indexed palette source without allocating its image.
pub(crate) fn validate_game_palette(bytes: &[u8], request: &DecodeRequest) -> Result<(), RasterDecodeError> {
    decode_internal(bytes, request, true).map(|_| ())
}

fn decode_internal(
    bytes: &[u8],
    request: &DecodeRequest,
    palette_only: bool,
) -> Result<Option<DecodedRaster>, RasterDecodeError> {
    crate::check_single_image(request)?;
    let header = bytes.get(..128).ok_or_else(|| invalid("truncated header"))?;
    if header[0] != 10 || !matches!(header[1], 0 | 2 | 3 | 4 | 5) || header[2] > 1 {
        return Err(invalid("manufacturer, version or encoding"));
    }
    let word = |offset| u16::from_le_bytes([header[offset], header[offset + 1]]);
    let (xmin, ymin, xmax, ymax) = (word(4), word(6), word(8), word(10));
    if xmax < xmin || ymax < ymin {
        return Err(invalid("inverted image bounds"));
    }
    let width = u32::from(xmax) - u32::from(xmin) + 1;
    let height = u32::from(ymax) - u32::from(ymin) + 1;
    let bits = usize::from(header[3]);
    let planes = usize::from(header[65]);
    let rgb = bits == 8 && planes == 3;
    let indexed8 = bits == 8 && planes == 1;
    if palette_only && !indexed8 {
        return Err(invalid("game palette requires indexed 8-bit pixels"));
    }
    let low_color = matches!(bits, 1 | 2 | 4) && planes > 0 && bits * planes <= 4;
    if !(rgb || indexed8 || low_color) {
        return Err(invalid("unsupported bit depth or plane count"));
    }
    // Versions 0 and 3 do not define a usable color palette. Monochrome is
    // unambiguous; colored variants need an explicit palette import policy.
    let monochrome = bits == 1 && planes == 1;
    if low_color && !monochrome && matches!(header[1], 0 | 3) {
        return Err(invalid("legacy image requires an external palette"));
    }
    let stride = usize::from(word(66));
    if stride == 0 || stride % 2 != 0 || stride < (width as usize * bits).div_ceil(8) {
        return Err(invalid("invalid bytes-per-plane scanline stride"));
    }
    let output_bytes = u64::from(width) * u64::from(height) * 4;
    if output_bytes > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let output_bytes = usize::try_from(output_bytes).map_err(|_| RasterDecodeError::OutputTooLarge)?;
    let (encoded, palette) = if indexed8 {
        let palette_start = bytes
            .len()
            .checked_sub(769)
            .filter(|offset| *offset >= 128)
            .ok_or_else(|| invalid("missing 256-color palette"))?;
        if bytes[palette_start] != 12 {
            return Err(invalid("missing palette marker"));
        }
        (&bytes[128..palette_start], &bytes[palette_start + 1..])
    } else {
        (&bytes[128..], &header[16..64])
    };
    let row_len = stride * planes;
    let mut offset = 0;
    for _ in 0..height {
        request.check_cancelled()?;
        scanline::<false>(encoded, &mut offset, row_len, header[2] == 1, &mut [], request)?;
    }
    if offset != encoded.len() {
        return Err(invalid("unexpected data after raster"));
    }
    request.check_cancelled()?;
    if palette_only {
        return Ok(None);
    }
    let mut output = buffer(output_bytes, request)?;
    let pixels = output.get_mut().unwrap();
    let mut row = buffer(row_len, request)?;
    let mut offset = 0;
    let mut pixel_index = 0;
    for _ in 0..height {
        request.check_cancelled()?;
        scanline::<true>(
            encoded,
            &mut offset,
            row_len,
            header[2] == 1,
            row.get_mut().unwrap(),
            request,
        )?;
        for x in 0..width as usize {
            if x % 4096 == 0 {
                request.check_cancelled()?;
            }
            let color = if rgb {
                [row[x], row[stride + x], row[2 * stride + x]]
            } else {
                let index = if indexed8 {
                    usize::from(row[x])
                } else {
                    let mut index = 0;
                    let bit_offset = x * bits;
                    let shift = 8 - bits - bit_offset % 8;
                    for plane in 0..planes {
                        let value = (row[plane * stride + bit_offset / 8] >> shift) & ((1 << bits) - 1);
                        index |= usize::from(value) << (plane * bits);
                    }
                    index
                };
                if monochrome && matches!(header[1], 0 | 3) {
                    let gray = if index == 0 { 0 } else { 255 };
                    [gray; 3]
                } else {
                    let start = index * 3;
                    [palette[start], palette[start + 1], palette[start + 2]]
                }
            };
            pixels[pixel_index * 4..pixel_index * 4 + 4]
                .copy_from_slice(&[color[0], color[1], color[2], 255]);
            pixel_index += 1;
        }
    }
    if offset != encoded.len() {
        return Err(invalid("unexpected data after raster"));
    }
    Ok(Some(DecodedRaster::new(
        width,
        height,
        RasterPixels::Rgba8(output),
        RasterColorSpace::AssumedSrgb,
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn external_palette_validation_has_zero_pixel_allocation_and_keeps_strict_errors() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/wal-colormap.pcx");
        let budget = rrrah_core::MemoryBudget::new(0);
        let mut request = DecodeRequest::new("palette.pcx");
        request.memory_budget = Some(budget.clone());
        validate_game_palette(bytes, &request).unwrap();
        assert_eq!((budget.used(), budget.peak()), (0, 0));
        // Every truncated prefix remains invalid, as in ordinary full decoding.
        for length in 0..bytes.len() {
            assert!(validate_game_palette(&bytes[..length], &request).is_err());
        }
        let ordinary = DecodeRequest::new("palette.pcx");
        for (offset, value) in [(0, 0), (2, 2), (3, 4), (65, 3), (66, 0), (67, 0)] {
            let mut malformed = bytes.to_vec();
            malformed[offset] = value;
            if decode(&malformed, &ordinary).is_err() {
                assert!(validate_game_palette(&malformed, &request).is_err());
            }
        }
        let mut trailing = bytes.to_vec();
        trailing.insert(trailing.len() - 769, 0);
        assert!(validate_game_palette(&trailing, &request).is_err());
        request.cancellation = Some(crate::GenerationToken::new(
            Arc::new(std::sync::atomic::AtomicU64::new(2)),
            1,
        ));
        assert!(matches!(
            validate_game_palette(bytes, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
        assert_eq!((budget.used(), budget.peak()), (0, 0));
    }

    #[test]
    fn managed_pillow_rgb_and_indexed_exact_short_and_shared_ownership() {
        for data in [
            include_bytes!("../../../tests/fixtures/pcx/rgb.pcx").as_slice(),
            include_bytes!("../../../tests/fixtures/pcx/indexed.pcx").as_slice(),
        ] {
            let row = u64::from(u16::from_le_bytes(data[66..68].try_into().unwrap())) * u64::from(data[65]);
            let budget = rrrah_core::MemoryBudget::new(24 + row);
            let mut request = DecodeRequest::new("managed.pcx");
            request.memory_budget = Some(budget.clone());
            let frame = decode(data, &request).unwrap();
            let RasterPixels::Rgba8(p) = frame.pixels() else {
                panic!()
            };
            assert_eq!(
                &p[..],
                [
                    11, 22, 33, 255, 192, 193, 255, 255, 1, 2, 3, 255, 44, 55, 66, 255, 77, 88, 99, 255, 0,
                    255, 127, 255
                ]
            );
            assert_eq!((budget.used(), budget.peak()), (24, 24 + row));
            let shared = frame.clone();
            drop(frame);
            assert_eq!(budget.used(), 24);
            drop(shared);
            assert_eq!(budget.used(), 0);
            let short = rrrah_core::MemoryBudget::new(23 + row);
            request.memory_budget = Some(short.clone());
            assert!(matches!(
                decode(data, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(short.used(), 0);
        }
    }
    #[test]
    fn invalid_packets_and_trailing_data_precede_output_admission() {
        let budget = rrrah_core::MemoryBudget::new(0);
        let mut request = DecodeRequest::new("bad.pcx");
        request.memory_budget = Some(budget.clone());
        for data in [
            &[0xc0, 1][..],
            &[0xc7, 1][..],
            &[0xc2][..],
            &[1, 2][..],
            &[0xc6, 7, 99][..],
        ] {
            let mut bytes = header(8, 3, 1, 1, 2);
            bytes.extend(data);
            assert!(matches!(
                decode(&bytes, &request),
                Err(RasterDecodeError::InvalidPcx(_))
            ));
        }
        let mut valid = header(8, 3, 1, 1, 2);
        valid.extend([0xc6, 7]);
        assert!(matches!(
            decode(&valid, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!((budget.used(), budget.peak()), (0, 0));
    }

    #[test]
    fn independently_encoded_pillow_corpus_matches_pixel_oracle() {
        for data in [
            include_bytes!("../../../tests/fixtures/pcx/rgb.pcx").as_slice(),
            include_bytes!("../../../tests/fixtures/pcx/indexed.pcx").as_slice(),
        ] {
            assert_eq!(
                pixels(data),
                [
                    11, 22, 33, 255, 192, 193, 255, 255, 1, 2, 3, 255, 44, 55, 66, 255, 77, 88, 99, 255, 0,
                    255, 127, 255
                ]
            );
        }
    }

    fn header(bits: u8, planes: u8, width: u16, height: u16, stride: u16) -> Vec<u8> {
        let mut h = vec![0; 128];
        h[0..4].copy_from_slice(&[10, 5, 1, bits]);
        h[8..10].copy_from_slice(&(width - 1).to_le_bytes());
        h[10..12].copy_from_slice(&(height - 1).to_le_bytes());
        h[65] = planes;
        h[66..68].copy_from_slice(&stride.to_le_bytes());
        h
    }

    fn pixels(bytes: &[u8]) -> Vec<u8> {
        let frame = decode(bytes, &DecodeRequest::new("unused")).unwrap();
        let RasterPixels::Rgba8(p) = frame.pixels() else {
            panic!("unexpected precision")
        };
        p.as_ref().to_vec()
    }

    #[test]
    fn rgb_planes_keep_padding_out_of_pixels_and_decode_escaped_literals() {
        let mut h = header(8, 3, 3, 1, 4);
        h.extend_from_slice(&[1, 2, 3, 99, 4, 5, 6, 98, 0xc1, 255, 8, 9, 97]);
        assert_eq!(pixels(&h), [1, 4, 255, 255, 2, 5, 8, 255, 3, 6, 9, 255]);
    }

    #[test]
    fn indexed_rle_uses_trailing_palette_and_allows_cross_plane_runs() {
        let mut h = header(8, 1, 2, 1, 2);
        h.extend_from_slice(&[0xc2, 7, 12]);
        let mut palette = [0_u8; 768];
        palette[21..24].copy_from_slice(&[11, 22, 33]);
        h.extend_from_slice(&palette);
        assert_eq!(pixels(&h), [11, 22, 33, 255, 11, 22, 33, 255]);
        let mut h = header(1, 4, 1, 1, 2);
        h[16 + 15 * 3..16 + 16 * 3].copy_from_slice(&[17, 29, 43]);
        h.extend_from_slice(&[0xc8, 128]);
        assert_eq!(pixels(&h), [17, 29, 43, 255]);
    }

    #[test]
    fn planar_bits_are_msb_first_and_plane_zero_is_least_significant() {
        let mut h = header(1, 2, 2, 1, 2);
        h[19..22].copy_from_slice(&[10, 20, 30]);
        h[22..25].copy_from_slice(&[40, 50, 60]);
        h.extend_from_slice(&[128, 0, 64, 0]);
        assert_eq!(pixels(&h), [10, 20, 30, 255, 40, 50, 60, 255]);
    }

    #[test]
    fn raw_encoding_does_not_interpret_rle_markers() {
        let mut h = header(8, 3, 1, 1, 2);
        h[2] = 0;
        h.extend_from_slice(&[192, 0, 193, 0, 194, 0]);
        assert_eq!(pixels(&h), [192, 193, 194, 255]);
    }

    #[test]
    fn malformed_runs_and_header_shapes_fail() {
        for data in [&[0xc0, 1][..], &[0xc7, 1], &[0xc2], &[1, 2]] {
            let mut h = header(8, 3, 1, 1, 2);
            h.extend_from_slice(data);
            assert!(decode(&h, &DecodeRequest::new("unused")).is_err());
        }
        let h = header(8, 3, 65535, 65535, 65534);
        assert!(decode(&h, &DecodeRequest::new("unused")).is_err());
        for size in 0..128 {
            assert!(decode(&vec![10; size], &DecodeRequest::new("unused")).is_err());
        }
    }
    #[test]
    fn pcx_import_convention_reaches_linear_display() {
        for data in [
            include_bytes!("../../../tests/fixtures/pcx/rgb.pcx").as_slice(),
            include_bytes!("../../../tests/fixtures/pcx/indexed.pcx").as_slice(),
        ] {
            let f = decode(data, &DecodeRequest::new("unused")).unwrap();
            assert_eq!(f.color_space(), &RasterColorSpace::AssumedSrgb);
            let RasterPixels::Rgba8(p) = f.pixels() else {
                panic!()
            };
            assert_eq!(&p[..4], &[11, 22, 33, 255]);
        }
    }
    #[test]
    fn windows_version_four_uses_header_palette() {
        let mut b = header(4, 1, 2, 1, 2);
        b[1] = 4;
        b[16..22].copy_from_slice(&[12, 34, 56, 78, 90, 123]);
        b.extend([0x01, 0]);
        assert_eq!(pixels(&b), [12, 34, 56, 255, 78, 90, 123, 255]);
    }
}
