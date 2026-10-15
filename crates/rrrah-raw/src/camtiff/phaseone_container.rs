//! Borrowed, bounded Phase One private directory access. Scalar values are inline,
//! unlike TIFF entries; only explicitly requested payloads are treated as offsets.
use super::camera_error;
use crate::DecodeError;
fn error(message: &str) -> DecodeError {
    camera_error("IIQ", message)
}
fn word(bytes: &[u8], at: usize) -> Result<u32, DecodeError> {
    let end = at.checked_add(4).ok_or(DecodeError::DimensionOverflow)?;
    Ok(u32::from_le_bytes(
        bytes
            .get(at..end)
            .ok_or_else(|| error("truncated private word"))?
            .try_into()
            .unwrap(),
    ))
}
pub(super) struct Directory<'a> {
    bytes: &'a [u8],
    base: usize,
    records: &'a [u8],
    stride: usize,
}
impl<'a> Directory<'a> {
    pub(super) fn open(bytes: &'a [u8], base: usize, calibration: bool) -> Result<Self, DecodeError> {
        let header_end = base.checked_add(12).ok_or(DecodeError::DimensionOverflow)?;
        let header = bytes
            .get(base..header_end)
            .ok_or_else(|| error("truncated private header"))?;
        if &header[..4] != b"IIII" || (!calibration && &header[4..8] != b"CwaR") {
            return Err(error("unqualified private header"));
        }
        let offset = base
            .checked_add(word(header, 8)? as usize)
            .ok_or(DecodeError::DimensionOverflow)?;
        let count = word(bytes, offset)? as usize;
        if count == 0 || count > 4096 {
            return Err(error("private directory count out of bounds"));
        }
        let stride = if calibration { 12 } else { 16 };
        let start = offset.checked_add(8).ok_or(DecodeError::DimensionOverflow)?;
        let end = start
            .checked_add(count * stride)
            .ok_or(DecodeError::DimensionOverflow)?;
        let records = bytes
            .get(start..end)
            .ok_or_else(|| error("truncated private directory"))?;
        Ok(Self {
            bytes,
            base,
            records,
            stride,
        })
    }
    pub(super) fn tags(&self) -> impl Iterator<Item = u32> + '_ {
        self.records
            .chunks_exact(self.stride)
            .map(|record| u32::from_le_bytes(record[..4].try_into().unwrap()))
    }
    fn entry(&self, tag: u32) -> Result<&[u8], DecodeError> {
        let mut found = None;
        for record in self.records.chunks_exact(self.stride) {
            if word(record, 0)? == tag {
                if found.is_some() {
                    return Err(error("duplicate private tag"));
                }
                found = Some(record);
            }
        }
        found.ok_or_else(|| error("missing private tag"))
    }
    pub(super) fn scalar(&self, tag: u32) -> Result<u32, DecodeError> {
        let entry = self.entry(tag)?;
        if self.stride != 16 || word(entry, 4)? != 4 || word(entry, 8)? != 4 {
            return Err(error("invalid inline scalar"));
        }
        word(entry, 12)
    }
    pub(super) fn payload(&self, tag: u32, maximum: usize) -> Result<&'a [u8], DecodeError> {
        let entry = self.entry(tag)?;
        let field = if self.stride == 16 { 8 } else { 4 };
        let length = word(entry, field)? as usize;
        if length == 0 || length > maximum {
            return Err(error("private payload length out of bounds"));
        }
        let relative = word(entry, field + 4)? as usize;
        if relative < 12 {
            return Err(error("private payload overlaps header"));
        }
        let start = self
            .base
            .checked_add(relative)
            .ok_or(DecodeError::DimensionOverflow)?;
        let end = start.checked_add(length).ok_or(DecodeError::DimensionOverflow)?;
        self.bytes
            .get(start..end)
            .ok_or_else(|| error("private payload escapes container"))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_directory_bounds_and_duplicate_tags() {
        let mut b = b"IIIICwaR".to_vec();
        b.extend(12u32.to_le_bytes());
        b.extend(1u32.to_le_bytes());
        b.extend(0u32.to_le_bytes());
        for v in [0x108u32, 4, 4, 4134] {
            b.extend(v.to_le_bytes());
        }
        assert_eq!(
            Directory::open(&b, 0, false).unwrap().scalar(0x108).unwrap(),
            4134
        );
        assert!(Directory::open(&b, 0, false).unwrap().payload(0x108, 8).is_err());
        for end in 0..b.len() {
            assert!(Directory::open(&b[..end], 0, false).is_err());
        }
        b[12..16].copy_from_slice(&2u32.to_le_bytes());
        b.extend_from_within(20..36);
        assert!(Directory::open(&b, 0, false).unwrap().scalar(0x108).is_err());
    }
    #[test]
    #[ignore = "requires pinned P20+ IIQ source"]
    fn real_private_sensor_and_calibration_payloads() {
        let bytes = std::fs::read(std::env::var("RRRAH_IIQ_SOURCE").unwrap()).unwrap();
        let root = Directory::open(&bytes, 8, false).unwrap();
        assert_eq!(root.scalar(0x108).unwrap(), 4134);
        assert_eq!(root.scalar(0x109).unwrap(), 4128);
        assert_eq!(root.scalar(0x10e).unwrap(), 3);
        assert_eq!(root.payload(0x10f, 32 * 1024 * 1024).unwrap().len(), 20583372);
        assert_eq!(root.payload(0x21c, 20000).unwrap().len(), 16512);
        let calibration = root.payload(0x110, 1024 * 1024).unwrap();
        let directory = Directory::open(calibration, 0, true).unwrap();
        for (tag, length) in [(0x400, 14920), (0x416, 66322), (0x410, 526354), (0x40b, 132628)] {
            assert_eq!(directory.payload(tag, 1024 * 1024).unwrap().len(), length);
        }
    }
}
