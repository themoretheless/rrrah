//! Bounded structural traversal of reference/identity metadata sequences.
//! Nested dataset fields never enter the root pixel decoder.
use super::{DecodeRequest, RasterDecodeError, bad, dword, implicit_vr, word};

pub(super) fn ignorable(tag: u32) -> bool {
    matches!(
        tag,
        0x00081110
            | 0x00081111
            | 0x00081115
            | 0x00081120
            | 0x00081140
            | 0x00082112
            | 0x00089215
            | 0x00101002
            | 0x00186011
    )
}

struct Walker<'a> {
    bytes: &'a [u8],
    little: bool,
    implicit: bool,
    count: &'a mut usize,
    request: &'a DecodeRequest,
}
impl Walker<'_> {
    fn tick(&mut self) -> Result<(), RasterDecodeError> {
        *self.count += 1;
        if *self.count > 65536 {
            return Err(bad("element count limit"));
        }
        if *self.count % 32 == 0 {
            self.request.check_cancelled()?;
        }
        Ok(())
    }
    fn take(&self, at: &mut usize, length: usize, limit: usize) -> Result<&[u8], RasterDecodeError> {
        let end = at
            .checked_add(length)
            .filter(|end| *end <= limit)
            .ok_or_else(|| bad("sequence value boundary"))?;
        let value = self
            .bytes
            .get(*at..end)
            .ok_or_else(|| bad("truncated sequence value"))?;
        *at = end;
        Ok(value)
    }
    fn item_header(&self, at: &mut usize, limit: usize) -> Result<(u32, u32), RasterDecodeError> {
        let h = self.take(at, 8, limit)?;
        Ok((
            (u32::from(word(h[..2].try_into().unwrap(), self.little)) << 16)
                | u32::from(word(h[2..4].try_into().unwrap(), self.little)),
            dword(h[4..8].try_into().unwrap(), self.little),
        ))
    }
    fn boundary(&self, at: usize, length: u32, parent: usize) -> Result<(usize, bool), RasterDecodeError> {
        if length == u32::MAX {
            return Ok((parent, true));
        }
        if length % 2 != 0 {
            return Err(bad("odd sequence/item length"));
        }
        Ok((
            at.checked_add(length as usize)
                .filter(|end| *end <= parent)
                .ok_or_else(|| bad("sequence/item length boundary"))?,
            false,
        ))
    }
    fn sequence(
        &mut self,
        at: &mut usize,
        length: u32,
        parent: usize,
        depth: usize,
    ) -> Result<(), RasterDecodeError> {
        if depth > 16 {
            return Err(bad("sequence nesting limit"));
        }
        let (limit, undefined) = self.boundary(*at, length, parent)?;
        while *at < limit {
            self.tick()?;
            let (tag, length) = self.item_header(at, limit)?;
            match tag {
                0xfffee0dd if undefined && length == 0 => return Ok(()),
                0xfffee000 => self.dataset(at, length, limit, depth)?,
                _ => return Err(bad("invalid sequence item/delimiter")),
            }
        }
        if undefined {
            Err(bad("missing sequence delimiter"))
        } else {
            Ok(())
        }
    }
    fn dataset(
        &mut self,
        at: &mut usize,
        length: u32,
        parent: usize,
        depth: usize,
    ) -> Result<(), RasterDecodeError> {
        let (limit, undefined) = self.boundary(*at, length, parent)?;
        let mut previous = None;
        while *at < limit {
            self.tick()?;
            let h = self.take(at, 8, limit)?;
            let tag = (u32::from(word(h[..2].try_into().unwrap(), self.little)) << 16)
                | u32::from(word(h[2..4].try_into().unwrap(), self.little));
            if tag >> 16 == 0xfffe {
                if tag == 0xfffee00d && undefined && dword(h[4..8].try_into().unwrap(), self.little) == 0 {
                    return Ok(());
                }
                return Err(bad("invalid item delimiter"));
            }
            if previous.is_some_and(|p| tag <= p) {
                return Err(bad("unordered or duplicate nested tag"));
            }
            previous = Some(tag);
            let vr = if self.implicit {
                implicit_vr(tag).ok_or_else(|| bad("unknown public implicit VR tag"))?
            } else {
                &h[4..6]
            };
            let length = if self.implicit {
                dword(h[4..8].try_into().unwrap(), self.little)
            } else if matches!(
                vr,
                b"OB" | b"OD" | b"OF" | b"OL" | b"OV" | b"OW" | b"SQ" | b"UC" | b"UR" | b"UT" | b"UN"
            ) {
                if h[6..8] != [0, 0] {
                    return Err(bad("reserved nested VR bytes"));
                }
                dword(self.take(at, 4, limit)?.try_into().unwrap(), self.little)
            } else {
                if !matches!(
                    vr,
                    b"AE"
                        | b"AS"
                        | b"AT"
                        | b"CS"
                        | b"DA"
                        | b"DS"
                        | b"DT"
                        | b"FD"
                        | b"FL"
                        | b"IS"
                        | b"LO"
                        | b"LT"
                        | b"PN"
                        | b"SH"
                        | b"SL"
                        | b"SS"
                        | b"ST"
                        | b"TM"
                        | b"UI"
                        | b"UL"
                        | b"US"
                ) {
                    return Err(bad("unsupported nested VR"));
                }
                u32::from(word(h[6..8].try_into().unwrap(), self.little))
            };
            if vr == b"SQ" {
                self.sequence(at, length, limit, depth + 1)?;
            } else {
                if length == u32::MAX || length % 2 != 0 {
                    return Err(bad("undefined/odd nested scalar length"));
                }
                self.take(at, length as usize, limit)?;
            }
        }
        if undefined {
            Err(bad("missing item delimiter"))
        } else {
            Ok(())
        }
    }
}

pub(super) fn skip(
    bytes: &[u8],
    at: &mut usize,
    length: u32,
    (little, implicit): (bool, bool),
    count: &mut usize,
    request: &DecodeRequest,
) -> Result<(), RasterDecodeError> {
    Walker {
        bytes,
        little,
        implicit,
        count,
        request,
    }
    .sequence(at, length, bytes.len(), 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(value: &[u8], undefined: bool) -> Vec<u8> {
        let mut out = vec![0xfe, 0xff, 0, 0xe0];
        out.extend_from_slice(&(if undefined { u32::MAX } else { value.len() as u32 }).to_le_bytes());
        out.extend_from_slice(value);
        if undefined {
            out.extend_from_slice(&[0xfe, 0xff, 0x0d, 0xe0, 0, 0, 0, 0]);
        }
        out
    }
    fn sequence(value: &[u8], undefined: bool) -> Vec<u8> {
        let mut out = vec![8, 0, 0x40, 0x11, b'S', b'Q', 0, 0];
        out.extend_from_slice(&(if undefined { u32::MAX } else { value.len() as u32 }).to_le_bytes());
        out.extend_from_slice(value);
        if undefined {
            out.extend_from_slice(&[0xfe, 0xff, 0xdd, 0xe0, 0, 0, 0, 0]);
        }
        out
    }
    fn walk(value: &[u8], length: u32) -> Result<usize, RasterDecodeError> {
        let mut at = 0;
        let mut count = 0;
        skip(
            value,
            &mut at,
            length,
            (true, false),
            &mut count,
            &DecodeRequest::new("metadata.dcm"),
        )?;
        assert_eq!(at, value.len());
        Ok(count)
    }
    #[test]
    fn malformed_delimiters_boundaries_and_scalar_headers_are_rejected() {
        for seq_undefined in [false, true] {
            for item_undefined in [false, true] {
                let scalar = [0x10, 0, 0x20, 0, b'L', b'O', 2, 0, b'A', b' '];
                let encoded = sequence(&item(&scalar, item_undefined), seq_undefined);
                let value = &encoded[12..];
                let length = u32::from_le_bytes(encoded[8..12].try_into().unwrap());
                assert!(walk(value, length).is_ok());
                for end in 0..value.len() {
                    assert!(
                        walk(&value[..end], length).is_err(),
                        "truncated sequence {seq_undefined}/{item_undefined}/{end}"
                    );
                }
                let mut bad_tag = value.to_vec();
                bad_tag[2] = 0x0d;
                assert!(walk(&bad_tag, length).is_err());
                let mut odd = value.to_vec();
                odd[4..8].copy_from_slice(&1u32.to_le_bytes());
                assert!(walk(&odd, length).is_err());
                let mut unknown_vr = value.to_vec();
                unknown_vr[12..14].copy_from_slice(b"XX");
                assert!(walk(&unknown_vr, length).is_err());
                let mut duplicate = item(&[scalar, scalar].concat(), item_undefined);
                if seq_undefined {
                    duplicate.extend_from_slice(&[0xfe, 0xff, 0xdd, 0xe0, 0, 0, 0, 0]);
                }
                let n = if seq_undefined {
                    u32::MAX
                } else {
                    duplicate.len() as u32
                };
                assert!(walk(&duplicate, n).is_err());
                if seq_undefined {
                    let mut nonzero = value.to_vec();
                    let last = nonzero.len() - 1;
                    nonzero[last] = 1;
                    assert!(walk(&nonzero, length).is_err());
                }
            }
        }
    }
    #[test]
    fn depth_and_total_item_limits_are_bounded_and_cancellation_is_observed() {
        for undefined in [false, true] {
            let mut value = Vec::new();
            for _ in 1..16 {
                value = item(&sequence(&value, undefined), undefined);
            }
            let length = if undefined { u32::MAX } else { value.len() as u32 };
            let mut valid = value.clone();
            if undefined {
                valid.extend_from_slice(&[0xfe, 0xff, 0xdd, 0xe0, 0, 0, 0, 0]);
            }
            assert!(walk(&valid, length).is_ok());
            let mut deep = item(&sequence(&value, undefined), undefined);
            if undefined {
                deep.extend_from_slice(&[0xfe, 0xff, 0xdd, 0xe0, 0, 0, 0, 0]);
            }
            let length = if undefined { u32::MAX } else { deep.len() as u32 };
            assert!(matches!(
                walk(&deep, length),
                Err(RasterDecodeError::InvalidDicom("sequence nesting limit"))
            ));
        }
        let value = item(&[], false).repeat(65537);
        assert!(matches!(
            walk(&value, value.len() as u32),
            Err(RasterDecodeError::InvalidDicom("element count limit"))
        ));
        let mut request = DecodeRequest::new("cancelled.dcm");
        request.cancellation = Some(crate::GenerationToken::new(
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(2)),
            1,
        ));
        let mut at = 0;
        let mut count = 0;
        assert!(matches!(
            skip(
                &value,
                &mut at,
                value.len() as u32,
                (true, false),
                &mut count,
                &request
            ),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
        assert_eq!(count, 32);
    }
}
