//! CUR directory validation before adapting pixel entries to ICO.
use crate::raster::RasterDecodeError;
fn valid_directory(bytes: &[u8]) -> bool {
    if !bytes.starts_with(&[0, 0, 2, 0]) || bytes.len() < 6 {
        return false;
    }
    let count = usize::from(u16::from_le_bytes([bytes[4], bytes[5]]));
    let end = 6 + count * 16;
    if count == 0 || bytes.len() < end {
        return false;
    }
    bytes[6..end].chunks_exact(16).all(|entry| {
        let length = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
        entry[3] == 0
            && length > 0
            && offset >= end
            && offset.checked_add(length).is_some_and(|end| end <= bytes.len())
    })
}
pub(crate) fn normalize(
    bytes: &mut [u8],
    explicit_cursor: bool,
) -> Result<Option<(u32, u32)>, RasterDecodeError> {
    if !valid_directory(bytes) {
        if explicit_cursor && bytes.starts_with(&[0, 0, 2, 0]) {
            return Err(RasterDecodeError::InvalidCursor("invalid image directory"));
        }
        return Ok(None);
    }
    let count = usize::from(u16::from_le_bytes([bytes[4], bytes[5]]));
    let mut selected = None;
    let mut best_area = 0u32;
    for entry in bytes[6..6 + count * 16].chunks_exact_mut(16) {
        let width = if entry[0] == 0 { 256 } else { u16::from(entry[0]) };
        let height = if entry[1] == 0 { 256 } else { u16::from(entry[1]) };
        if u16::from_le_bytes([entry[4], entry[5]]) >= width
            || u16::from_le_bytes([entry[6], entry[7]]) >= height
        {
            return Err(RasterDecodeError::InvalidCursor("hotspot outside image"));
        }
        let area = u32::from(width) * u32::from(height);
        if area >= best_area {
            best_area = area;
            selected = Some((
                u32::from(u16::from_le_bytes([entry[4], entry[5]])),
                u32::from(u16::from_le_bytes([entry[6], entry[7]])),
            ));
        }
        entry[4..8].fill(0);
    }
    bytes[2] = 1;
    Ok(selected)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_offsets_and_hotspots() {
        let base = include_bytes!("../../../tests/fixtures/raster/pattern.ico.cur");
        for (start, value) in [(18, 1u32), (14, u32::MAX)] {
            let mut bytes = base.to_vec();
            bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
            assert!(normalize(&mut bytes, true).is_err());
        }
        let mut bytes = base.to_vec();
        bytes[10..12].copy_from_slice(&16u16.to_le_bytes());
        assert!(normalize(&mut bytes, true).is_err());
    }
    #[test]
    fn tga_with_overlapping_signature_is_not_a_cursor() {
        let mut bytes = vec![0; 18 + 8 * 8 * 3];
        bytes[2] = 2;
        bytes[4] = 1;
        bytes[12] = 8;
        bytes[14] = 8;
        bytes[16] = 24;
        assert!(normalize(&mut bytes, false).unwrap().is_none());
        assert_eq!(bytes[2], 2);
        let path =
            std::env::temp_dir().join(format!("rrrah-tga-cursor-signature-{}.tga", std::process::id()));
        std::fs::write(&path, &bytes).unwrap();
        let decoded = crate::decode_raster_file(&path);
        std::fs::remove_file(path).unwrap();
        let decoded = decoded.unwrap();
        assert_eq!((decoded.width(), decoded.height()), (8, 8));
    }
}
