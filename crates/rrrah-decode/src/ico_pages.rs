//! Stable quality-ranked ICO selection, preserving the codec's default best entry.
use crate::{DecodeError, DecodeRequest, raster::RasterDecodeError};
pub(super) fn has_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0, 0, 1, 0])
}

pub(super) fn selected_entry(
    bytes: &[u8],
    request: &DecodeRequest,
) -> Result<([u8; 16], usize), RasterDecodeError> {
    let cursor = bytes.starts_with(&[0, 0, 2, 0]);
    let bad = |reason| {
        if cursor {
            RasterDecodeError::InvalidCursor(reason)
        } else {
            RasterDecodeError::InvalidIconDirectory(reason)
        }
    };
    let header = bytes.get(..6).ok_or_else(|| bad("truncated header"))?;
    let count = usize::from(u16::from_le_bytes([header[4], header[5]]));
    if count == 0 || count > 4096 {
        return Err(bad("invalid entry count"));
    }
    if request.image_index >= count {
        return Err(DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    let end = 6 + count * 16;
    let entries = bytes.get(6..end).ok_or_else(|| bad("truncated directory"))?;
    // Admit bounded ranking scratch before allocating it; release before pixels.
    type Rank = (std::cmp::Reverse<u16>, std::cmp::Reverse<u32>, bool, usize);
    let fallback = rrrah_core::MemoryBudget::new((4096 * std::mem::size_of::<Rank>()) as u64);
    let budget = request.memory_budget.as_ref().unwrap_or(&fallback);
    let mut ranked = budget
        .try_buffer(
            count,
            (std::cmp::Reverse(0u16), std::cmp::Reverse(0u32), false, 0usize),
        )
        .map_err(DecodeError::Memory)?;
    for (index, entry) in entries.chunks_exact(16).enumerate() {
        request.check_cancelled()?;
        let size = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as u64;
        let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as u64;
        if entry[3] != 0
            || size == 0
            || offset < end as u64
            || offset
                .checked_add(size)
                .is_none_or(|end| end > bytes.len() as u64)
        {
            return Err(bad("invalid payload bounds"));
        }
        let width = if entry[0] == 0 { 256 } else { u32::from(entry[0]) };
        let height = if entry[1] == 0 { 256 } else { u32::from(entry[1]) };
        let depth = if cursor {
            let x = u32::from(u16::from_le_bytes([entry[4], entry[5]]));
            let y = u32::from(u16::from_le_bytes([entry[6], entry[7]]));
            if x >= width || y >= height {
                return Err(bad("hotspot outside image"));
            }
            0
        } else {
            u16::from_le_bytes([entry[6], entry[7]])
        };
        // image's best_entry prefers the final entry for a maximal tie;
        // otherwise its first maximal entry wins. Preserve this default.
        ranked[index] = (
            std::cmp::Reverse(depth),
            std::cmp::Reverse(width * height),
            index != count - 1,
            index,
        );
    }
    ranked.sort_unstable();
    let index = ranked[request.image_index].3;
    Ok((entries[index * 16..index * 16 + 16].try_into().unwrap(), count))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quality_order_keeps_best_default_and_selected_samples_metadata() {
        let mut bytes = include_bytes!("../../../tests/fixtures/raster/ico-selected-images.ico").to_vec();
        let original = bytes.clone();
        let root = rrrah_core::MemoryBudget::new(65536);
        let mut request = DecodeRequest::new("icons.ico");
        request.memory_budget = Some(root.clone());
        for (index, width, height, rgba) in [
            (0, 2, 2, [255, 0, 0, 128]),
            (1, 1, 3, [0, 0, 255, 255]),
            (2, 3, 1, [0, 255, 0, 64]),
        ] {
            request.image_index = index;
            let frame = crate::raster::decode_raster_bytes(&mut bytes, &request).unwrap();
            assert_eq!((frame.width(), frame.height()), (width, height));
            assert_eq!((frame.image_index(), frame.image_count()), (index, 3));
            let rrrah_core::RasterPixels::Rgba8(values) = frame.pixels() else {
                panic!()
            };
            assert!(values.chunks_exact(4).all(|pixel| pixel == rgba));
            assert_eq!(bytes, original);
            drop(frame);
            assert_eq!(root.used(), 0);
        }
        request.image_index = 3;
        assert!(crate::raster::decode_raster_bytes(&mut bytes, &request).is_err());
        assert_eq!(bytes, original);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn malformed_directory_and_failed_output_admission_preserve_input() {
        let original = include_bytes!("../../../tests/fixtures/raster/ico-selected-images.ico");
        let mut request = DecodeRequest::new("icons.ico");
        request.image_index = 1;
        for bytes in [original[..21].to_vec(), {
            let mut bytes = original.to_vec();
            bytes[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
            bytes
        }] {
            assert!(selected_entry(&bytes, &request).is_err());
        }
        let mut bytes = original.to_vec();
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(0));
        assert!(crate::raster::decode_raster_bytes(&mut bytes, &request).is_err());
        assert_eq!(&bytes[..], original);
        assert_eq!(request.memory_budget.as_ref().unwrap().used(), 0);
        // Ranking fits, but the existing16x16 BMP-backed icon's pixel output does not.
        let original = include_bytes!("../../../tests/fixtures/raster/pattern.ico");
        let mut bytes = original.to_vec();
        request.image_index = 0;
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(64));
        assert!(crate::raster::decode_raster_bytes(&mut bytes, &request).is_err());
        assert_eq!(&bytes[..], original);
        assert_eq!(request.memory_budget.as_ref().unwrap().used(), 0);
    }
}

#[cfg(test)]
mod cursor_selection_tests {
    use super::*;
    #[test]
    fn selected_cursor_hotspot_matches_pixels_and_default_tie_rule() {
        let original = include_bytes!("../../../tests/fixtures/raster/cur-selected-images.cur");
        let mut normalized = original.to_vec();
        assert_eq!(
            crate::cursor::normalize(&mut normalized, true).unwrap(),
            Some((0, 1))
        );
        let root = rrrah_core::MemoryBudget::new(65536);
        let mut request = DecodeRequest::new("icons.cur");
        request.memory_budget = Some(root.clone());
        for (index, size, rgba, hotspot) in [
            (0, 2, [255, 0, 0, 128], (0, 1)),
            (1, 2, [0, 255, 0, 64], (1, 0)),
            (2, 1, [0, 0, 255, 255], (0, 0)),
        ] {
            let mut bytes = original.to_vec();
            request.image_index = index;
            let frame = crate::raster::decode_raster_bytes(&mut bytes, &request).unwrap();
            assert_eq!(
                (
                    frame.width(),
                    frame.height(),
                    frame.image_index(),
                    frame.image_count()
                ),
                (size, size, index, 3)
            );
            assert_eq!(frame.hotspot(), Some(hotspot));
            let rrrah_core::RasterPixels::Rgba8(values) = frame.pixels() else {
                panic!()
            };
            assert!(values.chunks_exact(4).all(|pixel| pixel == rgba));
            assert_eq!(&bytes[..], original);
            drop(frame);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn cursor_invalid_hotspot_refuses_without_mutation_or_credit() {
        let mut bytes = include_bytes!("../../../tests/fixtures/raster/cur-selected-images.cur").to_vec();
        bytes[10..12].copy_from_slice(&2u16.to_le_bytes());
        let original = bytes.clone();
        let root = rrrah_core::MemoryBudget::new(65536);
        let mut request = DecodeRequest::new("icons.cur");
        request.image_index = 1;
        request.memory_budget = Some(root.clone());
        assert!(matches!(
            crate::raster::decode_raster_bytes(&mut bytes, &request),
            Err(RasterDecodeError::InvalidCursor("hotspot outside image"))
        ));
        assert_eq!(bytes, original);
        assert_eq!(root.used(), 0);
    }
}
