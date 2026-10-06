//! WAD3 mip textures carry a palette; WAD2 requires an external game palette.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidWad(s)
}
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
pub(crate) fn has_magic(b: &[u8]) -> bool {
    b.starts_with(b"WAD3") || b.starts_with(b"WAD2")
}
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
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
    let mut ranges = vec![(directory, end)];
    let mut images = Vec::new();
    for i in 0..count {
        let entry = &b[directory + i * 32..directory + (i + 1) * 32];
        let start = word(entry, 0) as usize;
        let disk = word(entry, 4) as usize;
        let finish = start.checked_add(disk).ok_or_else(|| bad("lump overflow"))?;
        if start < 12 || finish > b.len() || entry[14..16] != [0, 0] {
            return Err(bad("invalid lump extent or padding"));
        }
        if disk > 0 {
            ranges.push((start, finish));
        }
        if entry[12] == 0x43 {
            images.push((entry, start, finish));
        }
    }
    ranges.sort_unstable();
    for pair in ranges.windows(2) {
        if pair[1].0 < pair[0].1 {
            return Err(bad("overlapping lumps or directory"));
        }
    }
    let &(entry, start, finish) =
        images
            .get(request.image_index)
            .ok_or(crate::DecodeError::UnsupportedImageIndex {
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
    let mut pixels = Vec::with_capacity(w as usize * h as usize * 4);
    for (i, value) in tex[at..at + w as usize * h as usize].iter().enumerate() {
        if i % 4096 == 0 {
            request.check_cancelled()?;
        }
        let index = *value as usize;
        pixels.extend_from_slice(&palette[index * 3..index * 3 + 3]);
        pixels.push(if cutout && index == 255 { 0 } else { 255 });
    }
    Ok(DecodedRaster::new(
        w,
        h,
        RasterPixels::Rgba8(Arc::new(pixels).into()),
        RasterColorSpace::AssumedSrgb,
    )?
    .with_image_selection(request.image_index, images.len())?)
}

#[cfg(test)]
mod tests {
    use super::*;
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
