//! Quake II WAL stores palette indices; color/alpha come from an external palette.
use crate::{
    DecodeRequest,
    bounded_io::read_bounded,
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
    RasterDecodeError::InvalidWal(s)
}
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
pub fn validate(b: &[u8]) -> Result<(u32, u32, usize), RasterDecodeError> {
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
fn check_request(request: &DecodeRequest) -> Result<(), RasterDecodeError> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    Ok(())
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
    check_request(request)?;
    let bytes = read_bounded(request)?;
    decode_indices(&bytes, request, palette, color_space)
}
fn decode_indices(
    bytes: &[u8],
    request: &DecodeRequest,
    palette: &[[u8; 4]; 256],
    color_space: RasterColorSpace,
) -> Result<DecodedRaster, RasterDecodeError> {
    check_request(request)?;
    let (w, h, at) = validate(bytes)?;
    let count = w as usize * h as usize;
    let mut storage = output(count * 4, request)?;
    let out = storage.get_mut().unwrap();
    for (index, value) in bytes[at..at + count].iter().enumerate() {
        if index % 4096 == 0 {
            request.check_cancelled()?;
        }
        out[index * 4..index * 4 + 4].copy_from_slice(&palette[usize::from(*value)]);
    }
    request.check_cancelled()?;
    Ok(DecodedRaster::new(
        w,
        h,
        RasterPixels::Rgba8(storage),
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
pub fn decode_using_game_palette(
    bytes: &[u8],
    request: &DecodeRequest,
) -> Result<DecodedRaster, RasterDecodeError> {
    check_request(request)?;
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
    crate::pcx::validate_game_palette(&pcx, &palette_request)?;
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
    fn cancelled_game_palette_request_precedes_io_and_header() {
        let mut request = DecodeRequest::new("missing-game/textures/missing.wal");
        let budget = rrrah_core::MemoryBudget::new(0);
        request.memory_budget = Some(budget.clone());
        request.cancellation = Some(crate::GenerationToken::new(
            Arc::new(std::sync::atomic::AtomicU64::new(2)),
            1,
        ));
        assert!(matches!(
            decode_using_game_palette(&[], &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
        assert_eq!((budget.used(), budget.peak()), (0, 0));
    }
    #[test]
    fn unsupported_selection_precedes_palette_io_and_output_admission() {
        let source = include_bytes!("../../../tests/fixtures/raster/palette-grid.wal");
        let palette = [[0u8; 4]; 256];
        let budget = rrrah_core::MemoryBudget::new(0);
        let mut request = DecodeRequest::new("missing-game/textures/missing.wal");
        request.memory_budget = Some(budget.clone());
        for index in [1, 3, 4, usize::MAX] {
            request.image_index = index;
            for result in [
                decode_wal_with_palette(&request, &palette, RasterColorSpace::AssumedSrgb),
                decode_using_game_palette(source, &request),
                decode_indices(source, &request, &palette, RasterColorSpace::AssumedSrgb),
            ] {
                assert!(
                    matches!(result, Err(RasterDecodeError::Source(crate::DecodeError::UnsupportedImageIndex { index: actual })) if actual == index)
                );
            }
            assert_eq!((budget.used(), budget.peak()), (0, 0));
        }
    }
    #[test]
    fn game_palette_uses_source_and_wal_output_budget_without_unused_pcx_pixels() {
        let directory = std::env::temp_dir().join(format!("rrrah-wal-palette-budget-{}-{}",
            std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(directory.join("pics")).unwrap();
        std::fs::create_dir_all(directory.join("textures")).unwrap();
        let pcx = include_bytes!("../../../tests/fixtures/raster/wal-colormap.pcx");
        std::fs::write(directory.join("pics/colormap.pcx"), pcx).unwrap();
        let source = include_bytes!("../../../tests/fixtures/raster/palette-grid.wal");
        let oracle = include_bytes!("../../../tests/fixtures/raster/palette-grid.wal.rgba");
        let peak = (pcx.len() + oracle.len()) as u64;
        let root = rrrah_core::MemoryBudget::new(peak);
        let mut request = DecodeRequest::new(directory.join("textures/grid.wal"));
        request.memory_budget = Some(root.clone());
        let frame = decode_using_game_palette(source, &request).unwrap();
        let RasterPixels::Rgba8(values) = frame.pixels() else { panic!() };
        assert_eq!(&values[..], oracle);
        assert_eq!((root.used(), root.peak()), (oracle.len() as u64, peak));
        let held = frame.clone();
        drop(frame);
        assert_eq!(root.used(), oracle.len() as u64);
        drop(held);
        assert_eq!(root.used(), 0);
        let short = rrrah_core::MemoryBudget::new(peak - 1);
        request.memory_budget = Some(short.clone());
        assert!(matches!(decode_using_game_palette(source, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))));
        assert_eq!(short.used(), 0);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn oracle_pixels_exact_short_and_shared_budget() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = std::fs::read_to_string(root.join("wal-manifest.tsv")).unwrap();
        let colors = std::fs::read(root.join("wal-palette.rgba")).unwrap();
        let palette: [[u8; 4]; 256] = std::array::from_fn(|i| colors[i * 4..i * 4 + 4].try_into().unwrap());
        let mut cases = 0;
        for line in manifest.lines() {
            let f: Vec<_> = line.split('\t').collect();
            let bytes = std::fs::read(root.join(f[0])).unwrap();
            let oracle = std::fs::read(root.join(f[3])).unwrap();
            let size = oracle.len() as u64;
            let budget = rrrah_core::MemoryBudget::new(size);
            let mut request = DecodeRequest::new(f[0]);

            request.memory_budget = Some(budget.clone());
            let frame = decode_indices(&bytes, &request, &palette, RasterColorSpace::AssumedSrgb).unwrap();
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
                decode_indices(&bytes, &request, &palette, RasterColorSpace::AssumedSrgb),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(short.used(), 0);
            cases += 1;
        }
        assert_eq!(cases, 2);
    }

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
