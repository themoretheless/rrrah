//! Quake II WAL stores palette indices; color/alpha come from an external palette.
use crate::{
    DecodeRequest,
    bounded_io::read_bounded,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidWal(s)
}
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
pub(crate) fn validate(b: &[u8]) -> Result<(u32, u32, usize), RasterDecodeError> {
    if b.len() < 100 {
        return Err(bad("truncated header"));
    }
    let (w, h) = (word(b, 32), word(b, 36));
    if w == 0 || h == 0 || w > 65536 || h > 65536 {
        return Err(bad("invalid dimensions"));
    }
    if u64::from(w) * u64::from(h) * 4 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let mut end = 100;
    for level in 0..4 {
        let offset = word(b, 40 + level * 4) as usize;
        let count = ((w >> level).max(1) as usize) * ((h >> level).max(1) as usize);
        if offset < end {
            return Err(bad("overlapping mip levels or header"));
        }
        let next = offset
            .checked_add(count)
            .ok_or_else(|| bad("mip size overflow"))?;
        if next > b.len() {
            return Err(bad("truncated mip payload"));
        }
        // Non-contiguous levels are permitted by the indexed offsets. Gap
        // bytes have no pixel meaning, and are never interpreted as palette.
        end = next;
    }
    if end != b.len() {
        return Err(bad("trailing data or alternate WAL variant"));
    }
    Ok((w, h, word(b, 40) as usize))
}
/// Decode the base image of a Quake II WAL with a caller-supplied straight
/// RGBA palette and its color interpretation. WAL contains neither a palette
/// nor a per-pixel alpha declaration; no implicit game palette is substituted.
/// Other mip levels are validated. Animation chains require caller resolution.
pub fn decode_wal_with_palette(
    request: &DecodeRequest,
    palette: &[[u8; 4]; 256],
    color_space: RasterColorSpace,
) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    let bytes = read_bounded(request)?;
    decode_indices(&bytes, request, palette, color_space)
}
fn decode_indices(
    bytes: &[u8],
    request: &DecodeRequest,
    palette: &[[u8; 4]; 256],
    color_space: RasterColorSpace,
) -> Result<DecodedRaster, RasterDecodeError> {
    let (w, h, at) = validate(bytes)?;
    let count = w as usize * h as usize;
    let mut out = Vec::with_capacity(count * 4);
    for (index, value) in bytes[at..at + count].iter().enumerate() {
        if index % 4096 == 0 {
            request.check_cancelled()?;
        }
        out.extend(palette[usize::from(*value)]);
    }
    request.check_cancelled()?;
    Ok(DecodedRaster::new(
        w,
        h,
        RasterPixels::Rgba8(Arc::new(out).into()),
        color_space,
    )?)
}
/// Canonical external game palette dependency, even if it is missing.
pub fn wal_palette_path(path: &std::path::Path) -> Option<std::path::PathBuf> {
    if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("wal")) {
        return None;
    }
    path.parent()?
        .ancestors()
        .find(|p| p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("textures")))?
        .parent()
        .map(|root| root.join("pics/colormap.pcx"))
}
pub(crate) fn decode_using_game_palette(
    bytes: &[u8],
    request: &DecodeRequest,
) -> Result<DecodedRaster, RasterDecodeError> {
    validate(bytes)?;
    let mut palette_request = request.clone();
    palette_request.path = wal_palette_path(&request.path).ok_or(RasterDecodeError::WalPaletteRequired)?;
    palette_request.image_index = 0;
    if !palette_request.path.exists() {
        return Err(RasterDecodeError::WalPaletteRequired);
    }
    let pcx = read_bounded(&palette_request)?;
    if pcx.len() < 128 + 769 || pcx[3] != 8 || pcx[65] != 1 || pcx[pcx.len() - 769] != 12 {
        return Err(bad("game palette must be an indexed 8-bit PCX"));
    }
    crate::pcx::decode(&pcx, &palette_request)?;
    let mut palette = [[0u8; 4]; 256];
    for (index, rgb) in pcx[pcx.len() - 768..].chunks_exact(3).enumerate() {
        palette[index][..3].copy_from_slice(rgb);
        palette[index][3] = if index == 255 { 0 } else { 255 };
    }
    decode_indices(bytes, request, &palette, RasterColorSpace::AssumedSrgb)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_mip_directory_and_sizes_are_rejected() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/palette-grid.wal");
        for at in [32, 36, 40, 44, 48, 52] {
            let mut b = bytes.to_vec();
            b[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(validate(&b).is_err());
        }
        assert!(validate(&bytes[..99]).is_err());
        assert!(validate(&bytes[..bytes.len() - 1]).is_err());
        let mut b = bytes.to_vec();
        b.push(0);
        assert!(validate(&b).is_err());
    }
}
