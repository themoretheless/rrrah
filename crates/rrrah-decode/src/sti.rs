//! Sir-Tech STCI indexed planes and ETRLE subimages. Offsets are sprite placement,
//! not part of the returned standalone subimage. Color is conventionally sRGB.
use crate::{DecodeError, DecodeRequest, RasterDecodeError};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidSti(s)
}
pub(crate) fn has_magic(b: &[u8]) -> bool {
    b.starts_with(b"STCI")
}
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if b.len() < 64 || !has_magic(b) {
        return Err(bad("header"));
    }
    let u16at = |n| u16::from_le_bytes(b[n..n + 2].try_into().unwrap()) as usize;
    let u32at = |n| u32::from_le_bytes(b[n..n + 4].try_into().unwrap()) as usize;
    let flags = u32at(16);
    if flags & 16 != 0 {
        return decode_zlib(b, request);
    }
    if flags & 4 != 0 {
        return decode_rgb(b, request);
    }
    if flags & !0x29 != 0 || flags & 8 == 0 || b[44] != 8 || b[30..33] != [8, 8, 8] {
        return Err(bad("unsupported layout/flags"));
    }
    let colors = u32at(24);
    if colors == 0 || colors > 256 {
        return Err(bad("palette count"));
    }
    let palette_end = 64 + colors * 3;
    let palette = b.get(64..palette_end).ok_or_else(|| bad("palette bounds"))?;
    let compressed = flags & 32 != 0;
    let count = if compressed { u16at(28) } else { 1 };
    if count == 0 || (!compressed && u16at(28) != 0) {
        return Err(bad("subimage count"));
    }
    if request.image_index >= count {
        return Err(DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    let data_start = palette_end + if compressed { count * 16 } else { 0 };
    let data_end = data_start
        .checked_add(u32at(8))
        .ok_or_else(|| bad("stored size"))?;
    let end = data_end
        .checked_add(u32at(48))
        .ok_or_else(|| bad("app data size"))?;
    if end != b.len() {
        return Err(bad("stored/app data bounds"));
    }
    let mut selected = None;
    for index in 0..count {
        let (width, height, offset, length) = if compressed {
            let at = palette_end + index * 16;
            if at + 16 > data_start || at + 16 > b.len() {
                return Err(bad("subimage directory"));
            }
            let width = u16at(at + 14);
            let height = u16at(at + 12);
            (width, height, u32at(at), u32at(at + 4))
        } else {
            (u16at(22), u16at(20), 0, u32at(8))
        };
        if width == 0
            || height == 0
            || width as u64 * height as u64 * 4 > crate::raster::MAX_RASTER_BYTES
            || offset.checked_add(length).is_none_or(|n| n > u32at(8))
        {
            return Err(bad("subimage dimensions/range"));
        }
        if index == request.image_index {
            selected = Some((width, height, offset, length));
        }
    }
    let (width, height, offset, length) = selected.unwrap();
    let source = &b[data_start + offset..data_start + offset + length];
    let samples = width * height * 4;
    let credit = request
        .memory_budget
        .as_ref()
        .map(|m| m.try_reserve(samples as u64))
        .transpose()
        .map_err(DecodeError::Memory)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(samples)
        .map_err(|e| DecodeError::Memory(rrrah_core::BufferError::Allocate(e)))?;
    output.resize(samples, 0u8);
    let write = |out: &mut [u8], index: u8| -> Result<(), RasterDecodeError> {
        let color = palette
            .get(index as usize * 3..index as usize * 3 + 3)
            .ok_or_else(|| bad("palette index"))?;
        out[..3].copy_from_slice(color);
        out[3] = if flags & 1 != 0 && index as usize == u32at(12) {
            0
        } else {
            255
        };
        Ok(())
    };
    if compressed {
        let mut at = 0;
        for y in 0..height {
            request.check_cancelled()?;
            let mut x = 0;
            loop {
                let control = *source.get(at).ok_or_else(|| bad("truncated ETRLE"))?;
                at += 1;
                if control == 0 {
                    if x != width {
                        return Err(bad("short ETRLE row"));
                    }
                    break;
                }
                let run = (control & 127) as usize;
                if run == 0 || run > width - x {
                    return Err(bad("cross-row ETRLE run"));
                }
                if control & 128 == 0 {
                    let values = source
                        .get(at..at + run)
                        .ok_or_else(|| bad("ETRLE literal bounds"))?;
                    for (i, &v) in values.iter().enumerate() {
                        write(
                            &mut output[(y * width + x + i) * 4..(y * width + x + i + 1) * 4],
                            v,
                        )?;
                    }
                    at += run;
                }
                x += run;
            }
        }
        if at != source.len() {
            return Err(bad("trailing ETRLE bytes"));
        }
    } else {
        if source.len() != width * height || u32at(4) != source.len() {
            return Err(bad("uncompressed length"));
        }
        for (index, &v) in source.iter().enumerate() {
            if index % 4096 == 0 {
                request.check_cancelled()?;
            }
            write(&mut output[index * 4..index * 4 + 4], v)?;
        }
    }
    let pixels = match credit {
        Some(c) => c.try_adopt(output).map_err(DecodeError::Memory)?.into(),
        None => std::sync::Arc::new(output).into(),
    };
    Ok(DecodedRaster::new(
        width as u32,
        height as u32,
        RasterPixels::Rgba8(pixels),
        RasterColorSpace::AssumedSrgb,
    )?
    .with_image_selection(request.image_index, count)?)
}

fn decode_zlib(b: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    use std::io::Read;
    let u32at = |n| u32::from_le_bytes(b[n..n + 4].try_into().unwrap()) as usize;
    let flags = u32at(16);
    if flags & 32 != 0 || !matches!(flags & 12, 4 | 8) {
        return Err(bad("unsupported zlib layout"));
    }
    let width = u16::from_le_bytes(b[22..24].try_into().unwrap()) as usize;
    let height = u16::from_le_bytes(b[20..22].try_into().unwrap()) as usize;
    let sample_bytes = if flags & 8 != 0 {
        if flags & !0x19 != 0 || b[44] != 8 || b[28..30] != [0, 0] {
            return Err(bad("zlib indexed layout"));
        }
        1
    } else {
        if flags & !0x17 != 0 || !matches!(b[44], 16 | 24 | 32) {
            return Err(bad("zlib RGB layout"));
        }
        b[44] as usize / 8
    };
    if width == 0
        || height == 0
        || width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(sample_bytes))
            != Some(u32at(4))
    {
        return Err(bad("zlib original dimensions/size"));
    }
    let palette = if flags & 8 != 0 {
        let count = u32at(24);
        if count == 0 || count > 256 {
            return Err(bad("palette count"));
        }
        count * 3
    } else {
        0
    };
    let start = 64 + palette;
    let stored = u32at(8);
    let expanded = u32at(4);
    let app = u32at(48);
    if expanded as u64 > crate::raster::MAX_RASTER_BYTES
        || start.checked_add(stored).and_then(|n| n.checked_add(app)) != Some(b.len())
    {
        return Err(bad("zlib size/bounds"));
    }
    let length = start
        .checked_add(expanded)
        .and_then(|n| n.checked_add(app))
        .ok_or_else(|| bad("zlib expanded size"))?;
    let _credit = request
        .memory_budget
        .as_ref()
        .map(|m| m.try_reserve(length as u64))
        .transpose()
        .map_err(DecodeError::Memory)?;
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(length)
        .map_err(|e| DecodeError::Memory(rrrah_core::BufferError::Allocate(e)))?;
    decoded.resize(length, 0u8);
    decoded[..start].copy_from_slice(&b[..start]);
    decoded[16..20].copy_from_slice(&((flags as u32) & !16).to_le_bytes());
    decoded[8..12].copy_from_slice(&(expanded as u32).to_le_bytes());
    let mut z = flate2::read::ZlibDecoder::new(&b[start..start + stored]);
    for chunk in decoded[start..start + expanded].chunks_mut(65536) {
        request.check_cancelled()?;
        z.read_exact(chunk)
            .map_err(|_| bad("truncated/corrupt zlib stream"))?;
    }
    request.check_cancelled()?;
    let mut extra = [0u8; 1];
    if z.read(&mut extra).map_err(|_| bad("corrupt zlib checksum"))? != 0 || z.total_in() != stored as u64 {
        return Err(bad("zlib length/trailing data"));
    }
    decoded[start + expanded..].copy_from_slice(&b[start + stored..]);
    decode(&decoded, request)
}

fn decode_rgb(b: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    let u16at = |n| u16::from_le_bytes(b[n..n + 2].try_into().unwrap()) as usize;
    let u32at = |n| u32::from_le_bytes(b[n..n + 4].try_into().unwrap());
    let flags = u32at(16);
    let bits = b[44];
    if flags & !7 != 0 || !matches!(bits, 16 | 24 | 32) {
        return Err(bad("unsupported RGB layout/flags"));
    }
    if request.image_index != 0 {
        return Err(DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    let width = u16at(22);
    let height = u16at(20);
    if width == 0 || height == 0 || width as u64 * height as u64 * 8 > crate::raster::MAX_RASTER_BYTES {
        return Err(bad("RGB dimensions"));
    }
    let input_len = width * height * (bits as usize / 8);
    if u32at(4) as usize != input_len
        || u32at(8) as usize != input_len
        || 64usize
            .checked_add(input_len)
            .and_then(|n| n.checked_add(u32at(48) as usize))
            != Some(b.len())
    {
        return Err(bad("RGB length"));
    }
    let mut masks = [0u32; 4];
    let mut shifts = [0u32; 4];
    let mut used = 0u32;
    for c in 0..4 {
        let mask = u32at(24 + c * 4);
        let depth = b[40 + c] as u32;
        if c == 3 && flags & 2 == 0 {
            if mask != 0 || depth != 0 {
                return Err(bad("unexpected RGB alpha mask"));
            }
            continue;
        }
        let shift = mask.trailing_zeros();
        if mask == 0
            || depth == 0
            || depth > 16
            || shift + depth > bits as u32
            || (mask >> shift) != ((1u32 << depth) - 1)
            || mask & used != 0
        {
            return Err(bad("RGB channel masks/depth"));
        }
        masks[c] = mask;
        shifts[c] = shift;
        used |= mask;
    }
    let length = width * height * 4;
    let credit = request
        .memory_budget
        .as_ref()
        .map(|m| m.try_reserve(length as u64 * 2))
        .transpose()
        .map_err(DecodeError::Memory)?;
    let mut out = Vec::new();
    out.try_reserve_exact(length)
        .map_err(|e| DecodeError::Memory(rrrah_core::BufferError::Allocate(e)))?;
    out.resize(length, 0u16);
    for (index, pixel) in b[64..64 + input_len].chunks_exact(bits as usize / 8).enumerate() {
        if index % 4096 == 0 {
            request.check_cancelled()?;
        }
        let value = pixel
            .iter()
            .enumerate()
            .fold(0u32, |v, (i, &byte)| v | ((byte as u32) << (i * 8)));
        for c in 0..4 {
            let sample = if masks[c] == 0 {
                65535
            } else {
                let max = masks[c] >> shifts[c];
                let v = (value & masks[c]) >> shifts[c];
                ((v as u64 * 65535 + max as u64 / 2) / max as u64) as u16
            };
            out[index * 4 + c] = sample;
        }
        if flags & 1 != 0 && value == u32at(12) {
            out[index * 4 + 3] = 0;
        }
    }
    let pixels = match credit {
        Some(c) => c.try_adopt(out).map_err(DecodeError::Memory)?.into(),
        None => std::sync::Arc::new(out).into(),
    };
    Ok(DecodedRaster::new(
        width as u32,
        height as u32,
        RasterPixels::Rgba16(pixels),
        RasterColorSpace::AssumedSrgb,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rgb24_zlib_preserves_samples_and_bounds_application_data() {
        use std::io::Write;
        let samples = [33u8, 81, 129, 192, 64, 96, 1, 2, 3];
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(&samples).unwrap();
        let payload = z.finish().unwrap();
        let mut b = vec![0; 64];
        b[..4].copy_from_slice(b"STCI");
        b[4..8].copy_from_slice(&9u32.to_le_bytes());
        b[8..12].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        b[16..20].copy_from_slice(&20u32.to_le_bytes());
        b[20..22].copy_from_slice(&1u16.to_le_bytes());
        b[22..24].copy_from_slice(&3u16.to_le_bytes());
        b[44] = 24;
        b[48..52].copy_from_slice(&3u32.to_le_bytes());
        for (c, mask) in [0xffu32, 0xff00, 0xff0000].into_iter().enumerate() {
            b[24 + c * 4..28 + c * 4].copy_from_slice(&mask.to_le_bytes());
            b[40 + c] = 8;
        }
        b.extend(payload);
        b.extend([7, 8, 9]);
        let budget = rrrah_core::MemoryBudget::new(100);
        let mut request = DecodeRequest::new("rgb24.sti");
        request.memory_budget = Some(budget.clone());
        let image = decode(&b, &request).unwrap();
        let RasterPixels::Rgba16(p) = image.pixels() else {
            panic!()
        };
        assert_eq!(
            p.as_slice(),
            [
                8481, 20817, 33153, 65535, 49344, 16448, 24672, 65535, 257, 514, 771, 65535
            ]
        );
        drop(image);
        assert_eq!(budget.used(), 0);
        for end in 0..b.len() {
            assert!(decode(&b[..end], &request).is_err());
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn zlib_matches_indexed_samples_and_rejects_checksum_length_and_pressure() {
        use std::io::Write;
        let plain = fixture(false);
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(&plain[73..]).unwrap();
        let compressed = z.finish().unwrap();
        let mut source = plain[..73].to_vec();
        source[16..20].copy_from_slice(&25u32.to_le_bytes());
        source[8..12].copy_from_slice(&(compressed.len() as u32).to_le_bytes());
        source.extend(compressed);
        let budget = rrrah_core::MemoryBudget::new(88);
        let mut request = DecodeRequest::new("zlib.sti");
        request.memory_budget = Some(budget.clone());
        let output = decode(&source, &request).unwrap();
        let RasterPixels::Rgba8(p) = output.pixels() else {
            panic!()
        };
        assert_eq!(p.as_slice(), [255, 0, 0, 255, 9, 8, 7, 0, 0, 255, 0, 255]);
        assert_eq!(budget.used(), 12);
        drop(output);
        assert_eq!(budget.used(), 0);
        let mut corrupt = source.clone();
        *corrupt.last_mut().unwrap() ^= 1;
        assert!(decode(&corrupt, &request).is_err());
        assert_eq!(budget.used(), 0);
        let mut trailing = source.clone();
        trailing.push(0);
        let stored = (trailing.len() - 73) as u32;
        trailing[8..12].copy_from_slice(&stored.to_le_bytes());
        assert!(decode(&trailing, &request).is_err());
        assert_eq!(budget.used(), 0);
        for original in [2u32, 4, 1 << 29] {
            let mut invalid = source.clone();
            invalid[4..8].copy_from_slice(&original.to_le_bytes());
            assert!(decode(&invalid, &request).is_err());
            assert_eq!(budget.used(), 0);
        }
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(75));
        assert!(decode(&source, &request).is_err());
        assert_eq!(request.memory_budget.as_ref().unwrap().peak(), 0);
    }
    #[test]
    fn rgb_masks_preserve_normalized_precision_alpha_and_budget() {
        for (bits, flags, masks, depths, source, expected) in [
            (
                16u8,
                5u32,
                [0xf800u32, 0x07e0, 0x001f, 0],
                [5u8, 6, 5, 0],
                vec![0x00, 0xf8, 0xe0, 0x07, 0x1f, 0x00],
                vec![65535u16, 0, 0, 65535, 0, 65535, 0, 0, 0, 0, 65535, 65535],
            ),
            (
                32,
                6,
                [0xff, 0xff00, 0xff0000, 0xff000000],
                [8, 8, 8, 8],
                vec![33, 81, 129, 128, 192, 64, 96, 1, 1, 2, 3, 0],
                vec![
                    8481, 20817, 33153, 32896, 49344, 16448, 24672, 257, 257, 514, 771, 0,
                ],
            ),
        ] {
            let mut b = vec![0; 64];
            b[..4].copy_from_slice(b"STCI");
            b[4..8].copy_from_slice(&(source.len() as u32).to_le_bytes());
            b[8..12].copy_from_slice(&(source.len() as u32).to_le_bytes());
            b[12..16].copy_from_slice(&0x07e0u32.to_le_bytes());
            b[16..20].copy_from_slice(&flags.to_le_bytes());
            b[20..22].copy_from_slice(&1u16.to_le_bytes());
            b[22..24].copy_from_slice(&3u16.to_le_bytes());
            b[44] = bits;
            for c in 0..4 {
                b[24 + c * 4..28 + c * 4].copy_from_slice(&masks[c].to_le_bytes());
                b[40 + c] = depths[c];
            }
            b.extend(source);
            let budget = rrrah_core::MemoryBudget::new(24);
            let mut request = DecodeRequest::new("rgb.sti");
            request.memory_budget = Some(budget.clone());
            let image = decode(&b, &request).unwrap();
            let RasterPixels::Rgba16(p) = image.pixels() else {
                panic!()
            };
            assert_eq!(p.as_slice(), expected);
            let last = image.clone();
            drop(image);
            assert_eq!(budget.used(), 24);
            drop(last);
            assert_eq!(budget.used(), 0);
            let mut overlap = b.clone();
            overlap[28..32].copy_from_slice(&masks[0].to_le_bytes());
            assert!(decode(&overlap, &request).is_err());
            assert_eq!(budget.used(), 0);
            request.memory_budget = Some(rrrah_core::MemoryBudget::new(23));
            assert!(decode(&b, &request).is_err());
            assert_eq!(request.memory_budget.as_ref().unwrap().peak(), 0);
        }
    }
    fn fixture(compressed: bool) -> Vec<u8> {
        let mut b = vec![0; 64];
        b[..4].copy_from_slice(b"STCI");
        b[16..20].copy_from_slice(&(if compressed { 40u32 } else { 9 }).to_le_bytes());
        b[20..22].copy_from_slice(&1u16.to_le_bytes());
        b[22..24].copy_from_slice(&3u16.to_le_bytes());
        b[24..28].copy_from_slice(&3u32.to_le_bytes());
        b[30..33].copy_from_slice(&[8, 8, 8]);
        b[44] = 8;
        b.extend([9, 8, 7, 255, 0, 0, 0, 255, 0]);
        let data = if compressed {
            b[28..30].copy_from_slice(&2u16.to_le_bytes());
            for (offset, len) in [(0u32, 5u32), (5, 4)] {
                b.extend(offset.to_le_bytes());
                b.extend(len.to_le_bytes());
                b.extend([0; 4]);
                b.extend(1u16.to_le_bytes());
                b.extend(3u16.to_le_bytes());
            }
            vec![2, 1, 2, 129, 0, 131, 0, 0, 0]
        } else {
            vec![1, 0, 2]
        };
        // Second ETRLE object is just one transparent run and a terminator.
        let mut data = data;
        if compressed {
            data.truncate(7);
            let dir = 73 + 16;
            b[dir + 4..dir + 8].copy_from_slice(&2u32.to_le_bytes());
        }
        b[4..8].copy_from_slice(&3u32.to_le_bytes());
        b[8..12].copy_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend(data);
        b
    }
    #[test]
    fn indexed_and_etrle_selection_budget_and_last_owner() {
        for compressed in [false, true] {
            let source = fixture(compressed);
            let budget = rrrah_core::MemoryBudget::new(12);
            let mut request = DecodeRequest::new("image.sti");
            request.memory_budget = Some(budget.clone());
            let image = decode(&source, &request).unwrap();
            let RasterPixels::Rgba8(p) = image.pixels() else {
                panic!()
            };
            let expected = if compressed {
                vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 0, 0]
            } else {
                vec![255, 0, 0, 255, 9, 8, 7, 0, 0, 255, 0, 255]
            };
            assert_eq!(p.as_slice(), expected);
            let last = image.clone();
            drop(image);
            assert_eq!(budget.used(), 12);
            drop(last);
            assert_eq!(budget.used(), 0);
            if compressed {
                request.image_index = 1;
                let second = decode(&source, &request).unwrap();
                let RasterPixels::Rgba8(p) = second.pixels() else {
                    panic!()
                };
                assert_eq!(p.as_slice(), [0; 12]);
                drop(second);
            }
            request.image_index = 2;
            assert!(decode(&source, &request).is_err());
            request.image_index = 0;
            request.memory_budget = Some(rrrah_core::MemoryBudget::new(11));
            assert!(decode(&source, &request).is_err());
            assert_eq!(request.memory_budget.as_ref().unwrap().used(), 0);
            for end in 0..source.len() {
                assert!(decode(&source[..end], &request).is_err());
            }
        }
    }
}
