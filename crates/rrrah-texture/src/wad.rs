//! WAD3 mip textures carry a palette; WAD2 requires an external game palette.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
fn output(len: usize, request: &DecodeRequest) -> Result<rrrah_core::PixelBuffer<u8>, RasterDecodeError> {
    Ok(match &request.memory_budget {
        Some(budget) => budget
            .try_buffer(len, 0u8)
            .map_err(crate::DecodeError::Memory)?
            .freeze()
            .into(),
        None => {
            let mut values = Vec::new();
            values
                .try_reserve_exact(len)
                .map_err(|_| RasterDecodeError::OutputTooLarge)?;
            values.resize(len, 0);
            Arc::new(values).into()
        }
    })
}
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidWad(s)
}
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
pub fn has_magic(b: &[u8]) -> bool {
    b.starts_with(b"WAD3") || b.starts_with(b"WAD2")
}
pub fn decode(b: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if b.len() < 12 || !has_magic(b) {
        return Err(bad("truncated header"));
    }
    if b.starts_with(b"WAD2") {
        return Err(bad("WAD2 requires an external palette; not yet implemented"));
    }
    let count = word(b, 4) as usize;
    let directory = word(b, 8) as usize;
    if count == 0 || count > 65536 || directory < 12 {
        return Err(bad("invalid directory"));
    }
    let end = directory
        .checked_add(count * 32)
        .ok_or_else(|| bad("directory overflow"))?;
    if end != b.len() {
        return Err(bad("truncated directory or trailing container bytes"));
    }
    let mut images = 0usize;
    let mut selected = None;
    let mut nonempty = 0usize;
    for i in 0..count {
        if i % 4096 == 0 {
            request.check_cancelled()?;
        }
        let entry = &b[directory + i * 32..directory + (i + 1) * 32];
        let start = word(entry, 0) as usize;
        let disk = word(entry, 4) as usize;
        let finish = start.checked_add(disk).ok_or_else(|| bad("lump overflow"))?;
        if start < 12 || finish > b.len() || entry[14..16] != [0, 0] {
            return Err(bad("invalid lump extent or padding"));
        }
        if disk > 0 {
            nonempty += 1;
        }
        if entry[12] == 0x43 {
            if images == request.image_index {
                selected = Some((entry, start, finish));
            }
            images += 1;
        }
    }
    {
        let mut ranges: rrrah_core::PixelBuffer<(usize, usize)> = match &request.memory_budget {
            Some(budget) => budget
                .try_buffer(nonempty + 1, (0usize, 0usize))
                .map_err(crate::DecodeError::Memory)?
                .freeze()
                .into(),
            None => {
                let mut values = Vec::new();
                values
                    .try_reserve_exact(nonempty + 1)
                    .map_err(|_| RasterDecodeError::OutputTooLarge)?;
                values.resize(nonempty + 1, (0, 0));
                Arc::new(values).into()
            }
        };
        let ranges = ranges.get_mut().unwrap();
        ranges[0] = (directory, end);
        let mut at = 1;
        for (i, entry) in b[directory..end].chunks_exact(32).enumerate() {
            if i % 4096 == 0 {
                request.check_cancelled()?;
            }
            let disk = word(entry, 4) as usize;
            if disk > 0 {
                let start = word(entry, 0) as usize;
                ranges[at] = (start, start + disk);
                at += 1;
            }
        }
        ranges.sort_unstable();
        request.check_cancelled()?;
        for pair in ranges.windows(2) {
            if pair[1].0 < pair[0].1 {
                return Err(bad("overlapping lumps or directory"));
            }
        }
    }
    let (entry, start, finish) = selected.ok_or(crate::DecodeError::UnsupportedImageIndex {
        index: request.image_index,
    })?;
    if entry[13] != 0 || word(entry, 4) != word(entry, 8) {
        return Err(bad("compressed texture lump"));
    }
    let tex = &b[start..finish];
    if tex.len() < 40 {
        return Err(bad("truncated mip header"));
    }
    fn name(v: &[u8]) -> Option<&[u8]> {
        v.iter().position(|c| *c == 0).map(|n| &v[..n])
    }
    let a = name(&entry[16..32]).ok_or_else(|| bad("unterminated directory name"))?;
    let c = name(&tex[..16]).ok_or_else(|| bad("unterminated texture name"))?;
    if !a.eq_ignore_ascii_case(c) {
        return Err(bad("inconsistent texture names"));
    }
    let (w, h) = (word(tex, 16), word(tex, 20));
    if w == 0 || h == 0 || w > 65536 || h > 65536 || w % 8 != 0 || h % 8 != 0 {
        return Err(bad("unsupported texture dimensions"));
    }
    if u64::from(w) * u64::from(h) * 4 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let mut mip_end = 40;
    for level in 0..4 {
        let offset = word(tex, 24 + level * 4) as usize;
        let samples = (w >> level) as usize * (h >> level) as usize;
        let next = offset.checked_add(samples).ok_or_else(|| bad("mip overflow"))?;
        if offset < mip_end || next > tex.len() {
            return Err(bad("overlapping or truncated mips"));
        }
        mip_end = next;
    }
    let pal_end = mip_end.checked_add(770).ok_or_else(|| bad("palette overflow"))?;
    if pal_end > tex.len()
        || u16::from_le_bytes([tex[mip_end], tex[mip_end + 1]]) != 256
        || tex.len() - pal_end > 3
        || tex[pal_end..].iter().any(|v| *v != 0)
    {
        return Err(bad("invalid palette or trailing mip data"));
    }
    let palette = &tex[mip_end + 2..pal_end];
    let at = word(tex, 24) as usize;
    let cutout = c.starts_with(b"{");
    let mut storage = output(w as usize * h as usize * 4, request)?;
    let pixels = storage.get_mut().unwrap();
    for (i, value) in tex[at..at + w as usize * h as usize].iter().enumerate() {
        if i % 4096 == 0 {
            request.check_cancelled()?;
        }
        let index = *value as usize;
        pixels[i * 4..i * 4 + 3].copy_from_slice(&palette[index * 3..index * 3 + 3]);
        pixels[i * 4 + 3] = if cutout && index == 255 { 0 } else { 255 };
    }
    Ok(
        DecodedRaster::new(w, h, RasterPixels::Rgba8(storage), RasterColorSpace::AssumedSrgb)?
            .with_image_selection(request.image_index, images)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_service_directory_is_admitted_released_and_fully_checked() {
        let original = include_bytes!("../../../tests/fixtures/raster/wad3-textures.wad");
        let directory = word(original, 8) as usize;
        let count = word(original, 4) as usize;
        let mut source = original[..directory].to_vec();
        source.resize(directory + 256, 0);
        let new_directory = source.len();
        source.extend(&original[directory..]);
        for i in 0..256 {
            let mut entry = [0u8; 32];
            entry[..4].copy_from_slice(&((directory + i) as u32).to_le_bytes());
            entry[4..8].copy_from_slice(&1u32.to_le_bytes());
            entry[8..12].copy_from_slice(&1u32.to_le_bytes());
            source.extend(entry);
        }
        source[4..8].copy_from_slice(&((count + 256) as u32).to_le_bytes());
        source[8..12].copy_from_slice(&(new_directory as u32).to_le_bytes());
        let nonempty = source[new_directory..]
            .chunks_exact(32)
            .filter(|e| word(e, 4) > 0)
            .count();
        let scratch = ((nonempty + 1) * std::mem::size_of::<(usize, usize)>()) as u64;
        let budget = rrrah_core::MemoryBudget::new(scratch);
        let mut request = DecodeRequest::new("services.wad");
        request.memory_budget = Some(budget.clone());
        let frame = decode(&source, &request).unwrap();
        assert_eq!(frame.image_count(), 2);
        assert_eq!((budget.used(), budget.peak()), (1024, scratch));
        let RasterPixels::Rgba8(pixels) = frame.pixels() else {
            panic!()
        };
        assert_eq!(
            &pixels[..],
            include_bytes!("../../../tests/fixtures/raster/wad3-textures.wad-0.rgba")
        );
        drop(frame);
        assert_eq!(budget.used(), 0);
        let short = rrrah_core::MemoryBudget::new(scratch - 1);
        request.memory_budget = Some(short.clone());
        assert!(matches!(
            decode(&source, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!(short.used(), 0);
        request.memory_budget = Some(budget.clone());
        let last = source.len() - 32;
        source[last..last + 4].copy_from_slice(&12u32.to_le_bytes());
        assert!(matches!(
            decode(&source, &request),
            Err(RasterDecodeError::InvalidWad("overlapping lumps or directory"))
        ));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn oracle_pixels_exact_short_and_shared_budget() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = std::fs::read_to_string(root.join("wad-manifest.tsv")).unwrap();

        let mut cases = 0;
        for line in manifest.lines() {
            let f: Vec<_> = line.split('\t').collect();
            let bytes = std::fs::read(root.join(f[0])).unwrap();
            let oracle = std::fs::read(root.join(f[4])).unwrap();
            let size = oracle.len() as u64;
            let budget = rrrah_core::MemoryBudget::new(size);
            let mut request = DecodeRequest::new(f[0]);
            request.image_index = f[1].parse().unwrap();
            request.memory_budget = Some(budget.clone());
            let frame = decode(&bytes, &request).unwrap();
            let RasterPixels::Rgba8(pixels) = frame.pixels() else {
                panic!()
            };
            assert_eq!(&pixels[..], oracle);
            assert_eq!((budget.used(), budget.peak()), (size, size));
            let shared = frame.clone();
            drop(frame);
            assert_eq!(budget.used(), size);
            drop(shared);
            assert_eq!(budget.used(), 0);
            let short = rrrah_core::MemoryBudget::new(size - 1);
            request.memory_budget = Some(short.clone());
            assert!(matches!(
                decode(&bytes, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(short.used(), 0);
            cases += 1;
        }
        assert_eq!(cases, 4);
    }

    #[test]
    fn malformed_directory_mips_and_palette_fail() {
        let original = include_bytes!("../../../tests/fixtures/raster/wad3-textures.wad");
        let request = DecodeRequest::new("synthetic.wad");
        let directory = word(original, 8) as usize;
        for end in [0, 11, original.len() - 1] {
            assert!(decode(&original[..end], &request).is_err());
        }
        for at in [
            4,
            8,
            directory,
            directory + 4,
            directory + 32,
            directory + 36,
            directory + 40,
            directory + 45,
            directory + 46,
        ] {
            let mut b = original.to_vec();
            b[at] = 255;
            assert!(decode(&b, &request).is_err(), "field {at}");
        }
        let entry = &original[directory + 32..directory + 64];
        let start = word(entry, 0) as usize;
        for at in [16, 20, 24, 28, 32, 36] {
            let mut b = original.to_vec();
            b[start + at..start + at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(decode(&b, &request).is_err());
        }
        let pal = start + word(original, start + 36) as usize + 4;
        let mut b = original.to_vec();
        b[pal..pal + 2].copy_from_slice(&255u16.to_le_bytes());
        assert!(decode(&b, &request).is_err());
        let mut request = request;
        request.image_index = 2;
        assert!(matches!(
            decode(original, &request),
            Err(RasterDecodeError::Source(
                crate::DecodeError::UnsupportedImageIndex { .. }
            ))
        ));
    }
}
