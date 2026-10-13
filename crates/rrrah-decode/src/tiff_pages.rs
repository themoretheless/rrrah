//! Bounded top-level IFD selection. Pixel/SubIFD offsets remain file-relative.
use crate::{DecodeError, DecodeRequest, raster::RasterDecodeError};

pub(super) fn has_magic(bytes: &[u8]) -> bool {
    [b"II*\0".as_slice(), b"MM\0*", b"II+\0", b"MM\0+"].iter()
        .any(|magic| bytes.starts_with(magic))
}

pub(super) fn selected_pointer(bytes: &[u8], request: &DecodeRequest)
    -> Result<(usize, usize, [u8; 8], usize), RasterDecodeError>
{
    let bad = |reason| RasterDecodeError::InvalidTiffDirectory(reason);
    let little = bytes.starts_with(b"II");
    let number = |at: usize, size: usize| -> Result<u64, RasterDecodeError> {
        let data = bytes.get(at..at.checked_add(size).ok_or_else(|| bad("offset overflow"))?)
            .ok_or_else(|| bad("truncated directory"))?;
        Ok(if little { data.iter().rev().fold(0, |n, b| (n << 8) | u64::from(*b)) }
            else { data.iter().fold(0, |n, b| (n << 8) | u64::from(*b)) })
    };
    let (header, pointer, count_size, entry_size) = match number(2, 2)? {
        42 => (4, 4, 2, 12),
        43 if number(4, 2)? == 8 && number(6, 2)? == 0 => (8, 8, 8, 20),
        _ => return Err(bad("invalid TIFF header")),
    };
    let missing = || DecodeError::UnsupportedImageIndex { index: request.image_index };
    if request.image_index >= 4096 { return Err(missing().into()); }
    let mut visited = [0_u64; 4096];
    let mut offset = number(header, pointer)?;
    let mut selected = None;
    for page in 0..4096 {
        request.check_cancelled()?;
        if offset == 0 { return Err(missing().into()); }
        if visited[..page].contains(&offset) { return Err(bad("cyclic page chain")); }
        visited[page] = offset;
        let directory = usize::try_from(offset).map_err(|_| bad("directory offset overflow"))?;
        let count = usize::try_from(number(directory, count_size)?).map_err(|_| bad("entry count overflow"))?;
        if count > 4096 { return Err(bad("excessive directory entries")); }
        let next = count.checked_mul(entry_size).and_then(|n| n.checked_add(count_size))
            .and_then(|n| n.checked_add(directory)).ok_or_else(|| bad("directory size overflow"))?;
        // Validate the complete table and next pointer even at the selected page.
        let following = number(next, pointer)?;
        if page == request.image_index as usize {
            let encoded = if little { offset.to_le_bytes() } else { offset.to_be_bytes() };
            let mut encoded_pointer = [0; 8];
            encoded_pointer[..pointer].copy_from_slice(if little { &encoded[..pointer] } else { &encoded[8-pointer..] });
            // Count the whole bounded top-level chain for viewer navigation.
            selected = Some(encoded_pointer);
        }
        if following == 0 {
            return selected.map(|value| (header, pointer, value, page + 1))
                .ok_or_else(|| missing().into());
        }
        offset = following;
    }
    Err(bad("excessive page chain"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put(bytes: &mut Vec<u8>, n: u64, size: usize, little: bool) {
        let data = if little { n.to_le_bytes() } else { n.to_be_bytes() };
        bytes.extend_from_slice(if little { &data[..size] } else { &data[8-size..] });
    }
    fn fixture(little: bool, big: bool) -> Vec<u8> {
        let (header, pointer, count, entry) = if big { (16, 8, 8, 20) } else { (8, 4, 2, 12) };
        let stride = count + 10 * entry + pointer;
        let data_at = header + 2 * stride;
        let mut bytes = if little { b"II".to_vec() } else { b"MM".to_vec() };
        put(&mut bytes, if big {43} else {42}, 2, little);
        if big { put(&mut bytes, 8, 2, little); put(&mut bytes, 0, 2, little); }
        put(&mut bytes, header as u64, pointer, little);
        for page in 0..2 {
            put(&mut bytes, 10, count, little);
            let tags = [(256,4,if page==0 {2} else {1}), (257,4,if page==0 {1} else {2}),
                (258,3,8), (259,3,1), (262,3,1), (273,4,(data_at+2*page) as u64),
                (274,3,if page==0 {1} else {6}), (277,3,1), (278,4,2), (279,4,2)];
            for (tag, kind, value) in tags {
                put(&mut bytes, tag, 2, little); put(&mut bytes, kind, 2, little);
                put(&mut bytes, 1, pointer, little);
                let size = if kind==3 {2} else {4};
                put(&mut bytes, value, size, little);
                bytes.extend(std::iter::repeat_n(0, pointer-size));
            }
            put(&mut bytes, if page==0 {(header+stride) as u64} else {0}, pointer, little);
        }
        bytes.extend([11,233,31,197]);
        bytes
    }
    #[test]
    fn classic_and_big_endian_pages_keep_selected_orientation_and_source() {
        for little in [false,true] { for big in [false,true] {
            let mut bytes = fixture(little,big); let original = bytes.clone();
            let root = rrrah_core::MemoryBudget::new(1024);
            let mut request = DecodeRequest::new("pages.tiff"); request.memory_budget=Some(root.clone());
            for (page, gray) in [(0,[11,233]),(1,[197,31]),(0,[11,233])] {
                request.image_index=page;
                let frame = crate::raster::decode_raster_bytes(&mut bytes,&request).unwrap();
                assert_eq!((frame.width(),frame.height()),(2,1));
                assert_eq!((frame.image_index(),frame.image_count()),(page,2));
                let rrrah_core::RasterPixels::Rgba8(pixels)=frame.pixels() else {panic!()};
                let expected: Vec<_> = gray.into_iter().flat_map(|v|[v,v,v,255]).collect();
                assert_eq!(&pixels[..], &expected);
                assert_eq!(bytes,original);
                drop(frame); assert_eq!(root.used(),0);
            }
            request.image_index=2;
            assert!(matches!(crate::raster::decode_raster_bytes(&mut bytes,&request),
                Err(RasterDecodeError::Source(DecodeError::UnsupportedImageIndex { index:2 }))));
            assert_eq!(bytes,original); assert_eq!(root.used(),0);
        }}
    }
    #[test]
    fn selected_file_magic_and_failed_admission_preserve_source() {
        let path=std::env::temp_dir().join(format!("rrrah-tiff-pages-{}.bin",std::process::id()));
        let mut bytes=fixture(true,false);
        std::fs::write(&path,&bytes).unwrap();
        let mut request=DecodeRequest::new(&path); request.image_index=1;
        let frame=crate::raster::decode_raster(&request).unwrap();
        assert_eq!((frame.width(),frame.height()),(2,1));
        let original=bytes.clone();
        let root=rrrah_core::MemoryBudget::new(0);request.memory_budget=Some(root.clone());
        assert!(crate::raster::decode_raster_bytes(&mut bytes,&request).is_err());
        assert_eq!(bytes,original);assert_eq!(root.used(),0);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn cyclic_truncated_and_excessive_pages_refuse_without_mutation() {
        let mut request=DecodeRequest::new("bad.tif"); request.image_index=2;
        let mut bytes=fixture(true,false);
        // Second IFD loops back to first.
        let second_next=8+126+122;
        bytes[second_next..second_next+4].copy_from_slice(&8u32.to_le_bytes());
        assert!(matches!(selected_pointer(&bytes,&request),Err(RasterDecodeError::InvalidTiffDirectory("cyclic page chain"))));
        request.image_index=1;
        bytes.truncate(140);
        assert!(selected_pointer(&bytes,&request).is_err());
        request.image_index=usize::MAX;
        assert!(selected_pointer(&fixture(true,true),&request).is_err());
    }
}
