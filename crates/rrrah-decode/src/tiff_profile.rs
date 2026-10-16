//! Borrow ICC bytes from the header-selected classic TIFF/BigTIFF directory.
use crate::{DecodeRequest, raster::RasterDecodeError};
fn bad(reason: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidTiffDirectory(reason)
}
pub(super) fn profile<'a>(
    bytes: &'a [u8],
    request: &DecodeRequest,
) -> Result<Option<&'a [u8]>, RasterDecodeError> {
    request.check_cancelled()?;
    let little = bytes.starts_with(b"II");
    if !little && !bytes.starts_with(b"MM") {
        return Err(bad("ICC TIFF byte order"));
    }
    let number = |at: usize, size: usize| -> Result<u64, RasterDecodeError> {
        let data = bytes
            .get(at..at.checked_add(size).ok_or_else(|| bad("ICC offset overflow"))?)
            .ok_or_else(|| bad("truncated ICC directory"))?;
        Ok(if little {
            data.iter().rev().fold(0, |n, &b| (n << 8) | u64::from(b))
        } else {
            data.iter().fold(0, |n, &b| (n << 8) | u64::from(b))
        })
    };
    let (header, pointer, count_size, entry_size, value_at) = match number(2, 2)? {
        42 => (4, 4, 2, 12, 8),
        43 if number(4, 2)? == 8 && number(6, 2)? == 0 => (8, 8, 8, 20, 12),
        _ => return Err(bad("ICC TIFF version")),
    };
    let directory = usize::try_from(number(header, pointer)?).map_err(|_| bad("ICC directory offset"))?;
    let count = usize::try_from(number(directory, count_size)?).map_err(|_| bad("ICC directory count"))?;
    if count > 4096 {
        return Err(bad("excessive ICC directory entries"));
    }
    let table_end = directory
        .checked_add(count_size)
        .and_then(|n| n.checked_add(count.checked_mul(entry_size)?))
        .ok_or_else(|| bad("ICC directory overflow"))?;
    number(table_end, pointer)?; // Validate the complete table and next pointer.
    let mut found = None;
    for index in 0..count {
        request.check_cancelled()?;
        let at = directory + count_size + index * entry_size;
        if number(at, 2)? != 34675 {
            continue;
        }
        if found.is_some() {
            return Err(bad("duplicate TIFF ICC profile"));
        }
        if !matches!(number(at + 2, 2)?, 1 | 7) {
            return Err(bad("unsupported TIFF ICC field type"));
        }
        let length = usize::try_from(number(at + 4, pointer)?).map_err(|_| bad("ICC profile length"))?;
        if length == 0 {
            return Err(bad("empty TIFF ICC profile"));
        }
        let start = if length <= pointer {
            at + value_at
        } else {
            usize::try_from(number(at + value_at, pointer)?).map_err(|_| bad("ICC profile offset"))?
        };
        let end = start
            .checked_add(length)
            .ok_or_else(|| bad("ICC profile extent overflow"))?;
        found = Some(
            bytes
                .get(start..end)
                .ok_or_else(|| bad("truncated TIFF ICC profile"))?,
        );
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(out: &mut [u8], at: usize, size: usize, value: u64, little: bool) {
        let encoded = if little {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        };
        out[at..at + size].copy_from_slice(if little {
            &encoded[..size]
        } else {
            &encoded[8 - size..]
        });
    }
    fn fixture(big: bool, little: bool, data: &[u8]) -> Vec<u8> {
        let (directory, pointer, count_size, entry_size, value_at) =
            if big { (16, 8, 8, 20, 12) } else { (8, 4, 2, 12, 8) };
        let offset = directory + count_size + entry_size + pointer;
        let mut bytes = vec![0; offset + data.len()];
        bytes[..2].copy_from_slice(if little { b"II" } else { b"MM" });
        write(&mut bytes, 2, 2, if big { 43 } else { 42 }, little);
        if big {
            write(&mut bytes, 4, 2, 8, little);
        }
        write(
            &mut bytes,
            if big { 8 } else { 4 },
            pointer,
            directory as u64,
            little,
        );
        write(&mut bytes, directory, count_size, 1, little);
        let at = directory + count_size;
        write(&mut bytes, at, 2, 34675, little);
        write(&mut bytes, at + 2, 2, 7, little);
        write(&mut bytes, at + 4, pointer, data.len() as u64, little);
        if data.len() <= pointer {
            bytes[at + value_at..at + value_at + data.len()].copy_from_slice(data);
        } else {
            write(&mut bytes, at + value_at, pointer, offset as u64, little);
            bytes[offset..].copy_from_slice(data);
        }
        bytes
    }
    #[test]
    fn classic_big_both_endians_inline_external_and_truncation() {
        let request = DecodeRequest::new("profile.tif");
        for big in [false, true] {
            for little in [false, true] {
                for data in [b"abc".as_slice(), b"longer-profile-bytes"] {
                    let bytes = fixture(big, little, data);
                    assert_eq!(profile(&bytes, &request).unwrap(), Some(data));
                    let directory = if big { 16 } else { 8 };
                    let count_size = if big { 8 } else { 2 };
                    let table_end = directory + count_size + if big { 20 + 8 } else { 12 + 4 };
                    for end in 0..table_end {
                        assert!(profile(&bytes[..end], &request).is_err());
                    }
                    if data.len() > if big { 8 } else { 4 } {
                        assert!(profile(&bytes[..bytes.len() - 1], &request).is_err());
                    }
                    let mut wrong = bytes.clone();
                    wrong[directory + count_size + 2..directory + count_size + 4].fill(0);
                    assert!(profile(&wrong, &request).is_err());
                }
            }
        }
    }
    #[test]
    fn selected_pages_keep_distinct_inline_profiles_and_restore_headers() {
        for big in [false, true] {
            for little in [false, true] {
                let (directory, pointer, count_size, entry_size, header) =
                    if big { (16, 8, 8, 20, 8) } else { (8, 4, 2, 12, 4) };
                let next = directory + count_size + entry_size;
                let mut source = fixture(big, little, b"one");
                source.truncate(next + pointer);
                source.resize(source.len().div_ceil(8) * 8, 0);
                let second = source.len();
                let other = fixture(big, little, b"two");
                source.extend_from_slice(&other[directory..next + pointer]);
                write(&mut source, next, pointer, second as u64, little);
                let original = source.clone();
                for (index, expected) in [b"one", b"two"].into_iter().enumerate() {
                    let mut request = DecodeRequest::new("pages.tif");
                    request.image_index = index;
                    let (at, size, selected, count) =
                        super::super::tiff_pages::selected_pointer(&source, &request).unwrap();
                    assert_eq!((at, size, count), (header, pointer, 2));
                    let before = source[at..at + size].to_vec();
                    source[at..at + size].copy_from_slice(&selected[..size]);
                    assert_eq!(profile(&source, &request).unwrap(), Some(expected.as_slice()));
                    source[at..at + size].copy_from_slice(&before);
                    assert_eq!(source, original);
                }
                let mut missing = DecodeRequest::new("pages.tif");
                missing.image_index = 2;
                assert!(super::super::tiff_pages::selected_pointer(&source, &missing).is_err());
                assert_eq!(source, original);
                // Metadata refusal on a selected page must restore the borrowed header.
                write(&mut source, second + count_size + 4, pointer, 0, little);
                let malformed = source.clone();
                let root = rrrah_core::MemoryBudget::new(0);
                let mut request = DecodeRequest::new("pages.tif");
                request.image_index = 1;
                request.memory_budget = Some(root.clone());
                assert!(matches!(
                    super::super::decode_raster_bytes(&mut source, &request),
                    Err(RasterDecodeError::InvalidTiffDirectory("empty TIFF ICC profile"))
                ));
                assert_eq!(source, malformed);
                assert_eq!((root.used(), root.peak()), (0, 0));
                assert_eq!(
                    profile(&source, &DecodeRequest::new("pages.tif")).unwrap(),
                    Some(b"one".as_slice())
                );
            }
        }
    }

    #[test]
    fn missing_duplicate_empty_excessive_and_bad_extent_profiles_are_explicit() {
        let request = DecodeRequest::new("bad-profile.tif");
        for big in [false, true] {
            for little in [false, true] {
                let (directory, pointer, count_size, entry_size, value_at) =
                    if big { (16, 8, 8, 20, 12) } else { (8, 4, 2, 12, 8) };
                let at = directory + count_size;
                let source = fixture(big, little, b"longer-profile-bytes");
                let mut missing = source.clone();
                write(&mut missing, at, 2, 400, little);
                assert_eq!(profile(&missing, &request).unwrap(), None);
                let mut empty = source.clone();
                write(&mut empty, at + 4, pointer, 0, little);
                assert!(matches!(
                    profile(&empty, &request),
                    Err(RasterDecodeError::InvalidTiffDirectory("empty TIFF ICC profile"))
                ));
                let mut overflow = source.clone();
                write(&mut overflow, at + value_at, pointer, u64::MAX, little);
                assert!(matches!(
                    profile(&overflow, &request),
                    Err(RasterDecodeError::InvalidTiffDirectory(_))
                ));
                let mut excessive = source;
                write(&mut excessive, directory, count_size, 4097, little);
                assert!(matches!(
                    profile(&excessive, &request),
                    Err(RasterDecodeError::InvalidTiffDirectory(
                        "excessive ICC directory entries"
                    ))
                ));
                let mut duplicate = fixture(big, little, b"abc");
                let entry = duplicate[at..at + entry_size].to_vec();
                let end = at + entry_size * 2;
                duplicate.resize(end + pointer, 0);
                duplicate[at + entry_size..end].copy_from_slice(&entry);
                duplicate[end..end + pointer].fill(0);
                write(&mut duplicate, directory, count_size, 2, little);
                assert!(matches!(
                    profile(&duplicate, &request),
                    Err(RasterDecodeError::InvalidTiffDirectory(
                        "duplicate TIFF ICC profile"
                    ))
                ));
            }
        }
    }

    #[test]
    fn cancellation_precedes_invalid_header_and_releases_no_credit() {
        use std::sync::{Arc, atomic::AtomicU64};
        let root = rrrah_core::MemoryBudget::new(0);
        let mut request = DecodeRequest::new("cancelled.tif");
        request.memory_budget = Some(root.clone());
        request.cancellation = Some(crate::GenerationToken::new(Arc::new(AtomicU64::new(2)), 1));
        assert!(matches!(
            profile(&[], &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
        assert_eq!((root.used(), root.peak()), (0, 0));
    }
}
