//! DCX page directory with bounded PCX page decoding.
use crate::{DecodeRequest, raster::RasterDecodeError};
use rrrah_core::DecodedRaster;
const MAGIC: &[u8] = &[0xb1, 0x68, 0xde, 0x3a];
const HEADER: usize = 4100;
pub fn has_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}
fn invalid(message: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidDcx(message)
}
pub fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if !has_magic(bytes) {
        return Err(invalid("invalid magic"));
    }
    let table = bytes
        .get(4..HEADER)
        .ok_or_else(|| invalid("truncated page table"))?;
    let index = request.image_index;
    let (mut pages, mut previous, mut selected_start, mut selected_end) = (0usize, None, None, None);
    let mut terminated = false;
    for entry in table.chunks_exact(4) {
        let offset = u32::from_le_bytes(entry.try_into().unwrap()) as usize;
        if offset == 0 {
            terminated = true;
            continue;
        }
        if terminated {
            return Err(invalid("nonzero entry after terminator"));
        }
        if offset < HEADER || offset >= bytes.len() || previous.is_some_and(|last| offset <= last) {
            return Err(invalid("invalid page offset"));
        }
        if pages == index {
            selected_start = Some(offset);
        }
        if index.checked_add(1) == Some(pages) {
            selected_end = Some(offset);
        }
        previous = Some(offset);
        pages += 1;
    }
    if pages == 0 || !terminated {
        return Err(invalid("missing pages or directory terminator"));
    }
    let start = selected_start.ok_or_else(|| invalid("page index out of range"))?;
    let end = selected_end.unwrap_or(bytes.len());
    let mut page_request = request.clone();
    page_request.image_index = 0;
    Ok(crate::pcx::decode(&bytes[start..end], &page_request)?.with_image_selection(index, pages)?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_directory_entries_are_validated_even_after_selected_page() {
        let source = include_bytes!("../../../tests/fixtures/raster/two-pages.dcx");
        let mut request = DecodeRequest::new("directory.dcx");
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(0));
        for (at, value) in [(8, 4100u32), (8, u32::MAX), (16, 4100), (4, 0)] {
            let mut bytes = source.to_vec();
            bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert!(matches!(
                decode(&bytes, &request),
                Err(RasterDecodeError::InvalidDcx(_))
            ));
        }
    }
    #[test]
    fn directory_validation_rejects_bad_offsets() {
        for offset in [1, 4099, 4101, u32::MAX] {
            let mut bytes = vec![0; 4101];
            bytes[..4].copy_from_slice(MAGIC);
            bytes[4..8].copy_from_slice(&offset.to_le_bytes());
            assert!(decode(&bytes, &DecodeRequest::new("image.dcx")).is_err());
        }
    }
    #[test]
    fn public_api_selects_each_page_against_pillow_oracle() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let bytes = std::fs::read(root.join("two-pages.dcx")).unwrap();
        for index in 0..2 {
            let mut request = DecodeRequest::new(root.join("two-pages.dcx"));
            request.image_index = index;
            let frame = decode(&bytes, &request).unwrap();
            let rrrah_core::RasterPixels::Rgba8(p) = frame.pixels() else {
                panic!()
            };
            assert_eq!(
                p.as_slice(),
                std::fs::read(root.join(format!("two-pages.dcx.page{index}.rgba"))).unwrap()
            );
            assert_eq!((frame.image_index(), frame.image_count()), (index, 2));
            let output = p.len() as u64;
            let page = u32::from_le_bytes(bytes[4 + index * 4..8 + index * 4].try_into().unwrap()) as usize;
            let row = u64::from(u16::from_le_bytes(
                bytes[page + 66..page + 68].try_into().unwrap(),
            )) * u64::from(bytes[page + 65]);
            let budget = rrrah_core::MemoryBudget::new(output + row);
            request.memory_budget = Some(budget.clone());
            let admitted = decode(&bytes, &request).unwrap();
            assert_eq!((admitted.image_index(), admitted.image_count()), (index, 2));
            let rrrah_core::RasterPixels::Rgba8(actual) = admitted.pixels() else {
                panic!()
            };
            assert_eq!(&actual[..], &p[..]);
            assert_eq!((budget.used(), budget.peak()), (output, output + row));
            drop(admitted);
            assert_eq!(budget.used(), 0);
            let short = rrrah_core::MemoryBudget::new(output + row - 1);
            request.memory_budget = Some(short.clone());
            assert!(matches!(
                decode(&bytes, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(short.used(), 0);
        }
        let mut request = DecodeRequest::new(root.join("two-pages.dcx"));
        request.image_index = 2;
        assert!(decode(&bytes, &request).is_err());
        request.image_index = usize::MAX;
        assert!(matches!(
            decode(&bytes, &request),
            Err(RasterDecodeError::InvalidDcx("page index out of range"))
        ));
    }
}
