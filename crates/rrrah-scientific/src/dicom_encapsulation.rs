//! RLE encapsulation: one fragment per frame; offsets address item headers.
use super::{DecodeRequest, RasterDecodeError, bad};
fn header(bytes: &[u8], at: &mut usize) -> Result<(u32, usize), RasterDecodeError> {
    let h = bytes
        .get(*at..*at + 8)
        .ok_or_else(|| bad("truncated encapsulated item"))?;
    *at += 8;
    Ok((
        u32::from_le_bytes(h[..4].try_into().unwrap()),
        u32::from_le_bytes(h[4..8].try_into().unwrap()) as usize,
    ))
}
fn value<'a>(bytes: &'a [u8], at: &mut usize, len: usize) -> Result<&'a [u8], RasterDecodeError> {
    if len % 2 != 0 || len == u32::MAX as usize {
        return Err(bad("invalid encapsulated item length"));
    }
    let end = at
        .checked_add(len)
        .ok_or_else(|| bad("encapsulated item overflow"))?;
    let value = bytes
        .get(*at..end)
        .ok_or_else(|| bad("truncated encapsulated item value"))?;
    *at = end;
    Ok(value)
}
pub(super) fn select<'a>(
    bytes: &'a [u8],
    at: &mut usize,
    frames: usize,
    index: usize,
    request: &DecodeRequest,
) -> Result<&'a [u8], RasterDecodeError> {
    if frames == 0 || frames > 65536 || index >= frames {
        return Err(bad("RLE frame selection"));
    }
    let (tag, len) = header(bytes, at)?;
    if tag != 0xe000fffe || (len != 0 && len != frames * 4) {
        return Err(bad("RLE basic offset table"));
    }
    let offsets = value(bytes, at, len)?;
    let first = *at;
    let mut selected = None;
    for i in 0..frames {
        if i % 32 == 0 {
            request.check_cancelled()?;
        }
        let item = *at;
        let (tag, len) = header(bytes, at)?;
        if tag != 0xe000fffe || len < 64 {
            return Err(bad("RLE requires one fragment per frame"));
        }
        if !offsets.is_empty() {
            let expected = u32::from_le_bytes(offsets[i * 4..i * 4 + 4].try_into().unwrap()) as usize;
            if expected != item - first {
                return Err(bad("RLE basic offset does not address frame item"));
            }
        }
        let data = value(bytes, at, len)?;
        if i == index {
            selected = Some(data);
        }
    }
    let (tag, len) = header(bytes, at)?;
    if tag != 0xe0ddfffe || len != 0 {
        return Err(bad("RLE sequence delimiter/frame count"));
    }
    Ok(selected.unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn container(bot: bool) -> Vec<u8> {
        let mut b = vec![0xfe, 0xff, 0, 0xe0];
        b.extend_from_slice(&(if bot { 8u32 } else { 0 }).to_le_bytes());
        if bot {
            b.extend_from_slice(&0u32.to_le_bytes());
            b.extend_from_slice(&72u32.to_le_bytes());
        }
        for value in [1u8, 2] {
            b.extend_from_slice(&[0xfe, 0xff, 0, 0xe0, 64, 0, 0, 0]);
            b.extend_from_slice(&[value; 64]);
        }
        b.extend_from_slice(&[0xfe, 0xff, 0xdd, 0xe0, 0, 0, 0, 0]);
        b
    }
    #[test]
    fn exact_and_empty_bot_selection_and_all_truncations() {
        let request = DecodeRequest::new("enc.dcm");
        for bot in [false, true] {
            let bytes = container(bot);
            for index in 0..2 {
                let mut at = 0;
                assert_eq!(
                    select(&bytes, &mut at, 2, index, &request).unwrap(),
                    &[index as u8 + 1; 64]
                );
                assert_eq!(at, bytes.len());
            }
            for end in 0..bytes.len() {
                assert!(select(&bytes[..end], &mut 0, 2, 0, &request).is_err());
            }
            assert!(select(&bytes, &mut 0, 2, 2, &request).is_err());
            assert!(select(&bytes, &mut 0, 1, 0, &request).is_err());
            let mut bad = bytes.clone();
            let last = bad.len() - 1;
            bad[last] = 1;
            assert!(select(&bad, &mut 0, 2, 0, &request).is_err());
            if bot {
                let mut bad = bytes.clone();
                bad[12..16].copy_from_slice(&64u32.to_le_bytes());
                assert!(select(&bad, &mut 0, 2, 0, &request).is_err());
            }
        }
    }
}
