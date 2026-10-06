//! DCX page directory with bounded PCX page decoding.
use crate::{DecodeRequest, raster::RasterDecodeError};
use rrrah_core::DecodedRaster;
const MAGIC: &[u8] = &[0xb1, 0x68, 0xde, 0x3a];
const HEADER: usize = 4100;
pub(crate) fn has_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}
fn invalid(message: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidDcx(message)
}
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if !has_magic(bytes) {
        return Err(invalid("invalid magic"));
    }
    let table = bytes
        .get(4..HEADER)
        .ok_or_else(|| invalid("truncated page table"))?;
    let mut offsets = Vec::new();
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
        if offset < HEADER || offset >= bytes.len() || offsets.last().is_some_and(|last| offset <= *last) {
            return Err(invalid("invalid page offset"));
        }
        offsets.push(offset);
    }
    if offsets.is_empty() || !terminated {
        return Err(invalid("missing pages or directory terminator"));
    }
    let index = request.image_index as usize;
    let start = *offsets
        .get(index)
        .ok_or_else(|| invalid("page index out of range"))?;
    let end = offsets.get(index + 1).copied().unwrap_or(bytes.len());
    Ok(crate::pcx::decode(&bytes[start..end], request)?.with_image_selection(index, offsets.len())?)
}
#[cfg(test)]
mod tests {
    use super::*;
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
        for index in 0..2 {
            let mut request = DecodeRequest::new(root.join("two-pages.dcx"));
            request.image_index = index;
            let frame = crate::decode_raster(&request).unwrap();
            let rrrah_core::RasterPixels::Rgba8(p) = frame.pixels() else {
                panic!()
            };
            assert_eq!(
                p.as_slice(),
                std::fs::read(root.join(format!("two-pages.dcx.page{index}.rgba"))).unwrap()
            );
        }
        let mut request = DecodeRequest::new(root.join("two-pages.dcx"));
        request.image_index = 2;
        assert!(crate::decode_raster(&request).is_err());
    }
}
