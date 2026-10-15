//! Bounded parser for classic TIFF directories embedded in CR3 CMT boxes.
//!
//! Offsets are relative to the beginning of the supplied TIFF payload.  This
//! module deliberately does not know Canon maker-note tags.  A caller can
//! traverse standard pointer entries (for example, `SubIFD` or Exif pointers) by
//! reading their `LONG` values and passing each offset to [`Tiff::parse_ifd`].
#![allow(dead_code)]

use std::{collections::HashSet, error::Error, fmt, str};

const TIFF_HEADER_LEN: usize = 8;
const TIFF_MAGIC: u16 = 42;
const IFD_ENTRY_LEN: usize = 12;
const INLINE_VALUE_LEN: usize = 4;

/// Limits applied independently to one TIFF payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_field_names)]
pub struct ParseLimits {
    /// Maximum number of entries accepted in a single IFD.
    pub max_entries_per_ifd: usize,
    /// Maximum number of IFDs followed through the linked `next IFD` chain.
    pub max_ifd_depth: usize,
    /// Maximum encoded byte length accepted for one entry value.
    pub max_value_bytes: usize,
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_entries_per_ifd: 4_096,
            max_ifd_depth: 64,
            max_value_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Byte order declared by the TIFF header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    LittleEndian,
    BigEndian,
}

impl ByteOrder {
    fn read_u16(self, bytes: &[u8]) -> u16 {
        let value = [bytes[0], bytes[1]];
        match self {
            Self::LittleEndian => u16::from_le_bytes(value),
            Self::BigEndian => u16::from_be_bytes(value),
        }
    }

    fn read_u32(self, bytes: &[u8]) -> u32 {
        let value = [bytes[0], bytes[1], bytes[2], bytes[3]];
        match self {
            Self::LittleEndian => u32::from_le_bytes(value),
            Self::BigEndian => u32::from_be_bytes(value),
        }
    }

    fn read_i32(self, bytes: &[u8]) -> i32 {
        let value = [bytes[0], bytes[1], bytes[2], bytes[3]];
        match self {
            Self::LittleEndian => i32::from_le_bytes(value),
            Self::BigEndian => i32::from_be_bytes(value),
        }
    }
}

/// Supported classic TIFF field types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum FieldType {
    Byte = 1,
    Ascii = 2,
    Short = 3,
    Long = 4,
    Rational = 5,
    Undefined = 7,
    Slong = 9,
    Srational = 10,
}

impl FieldType {
    fn from_code(tag: u16, code: u16) -> Result<Self, TiffError> {
        match code {
            1 => Ok(Self::Byte),
            2 => Ok(Self::Ascii),
            3 => Ok(Self::Short),
            4 => Ok(Self::Long),
            5 => Ok(Self::Rational),
            7 => Ok(Self::Undefined),
            9 => Ok(Self::Slong),
            10 => Ok(Self::Srational),
            _ => Err(TiffError::UnsupportedFieldType { tag, code }),
        }
    }

    const fn element_size(self) -> usize {
        match self {
            Self::Byte | Self::Ascii | Self::Undefined => 1,
            Self::Short => 2,
            Self::Long | Self::Slong => 4,
            Self::Rational | Self::Srational => 8,
        }
    }
}

/// Unsigned TIFF rational preserved without division.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rational {
    pub numerator: u32,
    pub denominator: u32,
}

impl Rational {
    /// Returns `None` for a zero denominator.
    pub fn as_f64(self) -> Option<f64> {
        (self.denominator != 0).then(|| f64::from(self.numerator) / f64::from(self.denominator))
    }
}

/// Signed TIFF rational preserved without division.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignedRational {
    pub numerator: i32,
    pub denominator: i32,
}

impl SignedRational {
    /// Returns `None` for a zero denominator.
    pub fn as_f64(self) -> Option<f64> {
        (self.denominator != 0).then(|| f64::from(self.numerator) / f64::from(self.denominator))
    }
}

/// One parsed IFD entry whose encoded value borrows the TIFF payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry<'a> {
    tag: u16,
    field_type: FieldType,
    count: u32,
    encoded: &'a [u8],
    byte_order: ByteOrder,
}

impl<'a> Entry<'a> {
    pub const fn tag(&self) -> u16 {
        self.tag
    }

    pub const fn field_type(&self) -> FieldType {
        self.field_type
    }

    pub const fn count(&self) -> u32 {
        self.count
    }

    /// Returns the encoded value bytes, excluding unused inline padding.
    pub const fn raw_bytes(&self) -> &'a [u8] {
        self.encoded
    }

    pub fn byte(&self) -> Result<u8, TiffError> {
        self.require_scalar(FieldType::Byte)?;
        Ok(self.encoded[0])
    }

    pub fn byte_values(&self) -> Result<&'a [u8], TiffError> {
        self.require_type(FieldType::Byte)?;
        Ok(self.encoded)
    }

    /// Returns the ASCII field without any trailing NUL terminators.
    pub fn ascii(&self) -> Result<&'a str, TiffError> {
        self.require_type(FieldType::Ascii)?;
        let trimmed_len = self
            .encoded
            .iter()
            .rposition(|byte| *byte != 0)
            .map_or(0, |index| index + 1);
        let trimmed = &self.encoded[..trimmed_len];
        if let Some((index, byte)) = trimmed
            .iter()
            .copied()
            .enumerate()
            .find(|(_, byte)| !byte.is_ascii())
        {
            return Err(TiffError::InvalidAscii {
                tag: self.tag,
                index,
                byte,
            });
        }
        // Every ASCII byte is valid UTF-8. Keep this fallible so the parser has
        // no panic path even if that invariant changes during future edits.
        str::from_utf8(trimmed).map_err(|_| TiffError::InvalidUtf8 { tag: self.tag })
    }

    pub fn ascii_bytes(&self) -> Result<&'a [u8], TiffError> {
        self.require_type(FieldType::Ascii)?;
        Ok(self.encoded)
    }

    pub fn short(&self) -> Result<u16, TiffError> {
        self.require_scalar(FieldType::Short)?;
        Ok(self.byte_order.read_u16(self.encoded))
    }

    pub fn short_values(&self) -> Result<Vec<u16>, TiffError> {
        self.require_type(FieldType::Short)?;
        collect_values(
            self.tag,
            self.encoded.len() / 2,
            self.encoded
                .chunks_exact(2)
                .map(|bytes| self.byte_order.read_u16(bytes)),
        )
    }

    pub fn long(&self) -> Result<u32, TiffError> {
        self.require_scalar(FieldType::Long)?;
        Ok(self.byte_order.read_u32(self.encoded))
    }

    pub fn long_values(&self) -> Result<Vec<u32>, TiffError> {
        self.require_type(FieldType::Long)?;
        collect_values(
            self.tag,
            self.encoded.len() / 4,
            self.encoded
                .chunks_exact(4)
                .map(|bytes| self.byte_order.read_u32(bytes)),
        )
    }

    pub fn rational(&self) -> Result<Rational, TiffError> {
        self.require_scalar(FieldType::Rational)?;
        Ok(self.decode_rational(self.encoded))
    }

    pub fn rational_values(&self) -> Result<Vec<Rational>, TiffError> {
        self.require_type(FieldType::Rational)?;
        collect_values(
            self.tag,
            self.encoded.len() / 8,
            self.encoded
                .chunks_exact(8)
                .map(|bytes| self.decode_rational(bytes)),
        )
    }

    pub fn undefined(&self) -> Result<&'a [u8], TiffError> {
        self.require_type(FieldType::Undefined)?;
        Ok(self.encoded)
    }

    pub fn slong(&self) -> Result<i32, TiffError> {
        self.require_scalar(FieldType::Slong)?;
        Ok(self.byte_order.read_i32(self.encoded))
    }

    pub fn slong_values(&self) -> Result<Vec<i32>, TiffError> {
        self.require_type(FieldType::Slong)?;
        collect_values(
            self.tag,
            self.encoded.len() / 4,
            self.encoded
                .chunks_exact(4)
                .map(|bytes| self.byte_order.read_i32(bytes)),
        )
    }

    pub fn srational(&self) -> Result<SignedRational, TiffError> {
        self.require_scalar(FieldType::Srational)?;
        Ok(self.decode_signed_rational(self.encoded))
    }

    pub fn srational_values(&self) -> Result<Vec<SignedRational>, TiffError> {
        self.require_type(FieldType::Srational)?;
        collect_values(
            self.tag,
            self.encoded.len() / 8,
            self.encoded
                .chunks_exact(8)
                .map(|bytes| self.decode_signed_rational(bytes)),
        )
    }

    fn require_type(&self, expected: FieldType) -> Result<(), TiffError> {
        if self.field_type == expected {
            Ok(())
        } else {
            Err(TiffError::TypeMismatch {
                tag: self.tag,
                expected,
                actual: self.field_type,
            })
        }
    }

    fn require_scalar(&self, expected: FieldType) -> Result<(), TiffError> {
        self.require_type(expected)?;
        if self.count == 1 {
            Ok(())
        } else {
            Err(TiffError::ExpectedScalar {
                tag: self.tag,
                count: self.count,
            })
        }
    }

    fn decode_rational(&self, bytes: &[u8]) -> Rational {
        Rational {
            numerator: self.byte_order.read_u32(&bytes[..4]),
            denominator: self.byte_order.read_u32(&bytes[4..]),
        }
    }

    fn decode_signed_rational(&self, bytes: &[u8]) -> SignedRational {
        SignedRational {
            numerator: self.byte_order.read_i32(&bytes[..4]),
            denominator: self.byte_order.read_i32(&bytes[4..]),
        }
    }
}

/// One classic TIFF image file directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ifd<'a> {
    offset: u32,
    entries: Vec<Entry<'a>>,
    next_ifd_offset: u32,
}

impl<'a> Ifd<'a> {
    pub const fn offset(&self) -> u32 {
        self.offset
    }

    pub fn entries(&self) -> &[Entry<'a>] {
        &self.entries
    }

    /// Returns the first entry carrying `tag`.
    pub fn entry(&self, tag: u16) -> Option<&Entry<'a>> {
        self.entries.iter().find(|entry| entry.tag == tag)
    }

    pub fn entries_with_tag(&self, tag: u16) -> impl Iterator<Item = &Entry<'a>> {
        self.entries.iter().filter(move |entry| entry.tag == tag)
    }

    pub const fn next_ifd_offset(&self) -> u32 {
        self.next_ifd_offset
    }
}

/// Parsed classic TIFF header and a borrowed payload.
#[derive(Debug, Clone, Copy)]
pub struct Tiff<'a> {
    data: &'a [u8],
    byte_order: ByteOrder,
    first_ifd_offset: u32,
    limits: ParseLimits,
}

impl<'a> Tiff<'a> {
    /// Parses a classic TIFF header using conservative default limits.
    pub fn parse(data: &'a [u8]) -> Result<Self, TiffError> {
        Self::parse_with_limits(data, ParseLimits::default())
    }

    /// Parses a classic TIFF header with caller-supplied resource limits.
    pub fn parse_with_limits(data: &'a [u8], limits: ParseLimits) -> Result<Self, TiffError> {
        let header = checked_slice(data, 0, TIFF_HEADER_LEN)?;
        let byte_order = match &header[..2] {
            b"II" => ByteOrder::LittleEndian,
            b"MM" => ByteOrder::BigEndian,
            marker => {
                return Err(TiffError::InvalidByteOrder {
                    marker: [marker[0], marker[1]],
                });
            }
        };
        let magic = byte_order.read_u16(&header[2..4]);
        if magic != TIFF_MAGIC {
            return Err(TiffError::InvalidMagic { found: magic });
        }
        let first_ifd_offset = byte_order.read_u32(&header[4..8]);
        Ok(Self {
            data,
            byte_order,
            first_ifd_offset,
            limits,
        })
    }

    pub const fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    pub const fn first_ifd_offset(&self) -> u32 {
        self.first_ifd_offset
    }

    /// Parses the header's first IFD, or returns `None` for a zero root offset.
    pub fn first_ifd(&self) -> Result<Option<Ifd<'a>>, TiffError> {
        (self.first_ifd_offset != 0)
            .then(|| self.parse_ifd(self.first_ifd_offset))
            .transpose()
    }

    /// Parses one IFD at a TIFF-relative offset.
    ///
    /// Pointer tags are intentionally not followed automatically.  Read their
    /// `LONG` value(s) and call this method for each offset.
    pub fn parse_ifd(&self, offset: u32) -> Result<Ifd<'a>, TiffError> {
        if offset == 0 {
            return Err(TiffError::ZeroIfdOffset);
        }
        let offset_usize = usize::try_from(offset).map_err(|_| TiffError::OffsetDoesNotFit { offset })?;
        let count_bytes = checked_slice(self.data, offset_usize, 2)?;
        let entry_count = usize::from(self.byte_order.read_u16(count_bytes));
        if entry_count > self.limits.max_entries_per_ifd {
            return Err(TiffError::EntryLimitExceeded {
                offset,
                count: entry_count,
                limit: self.limits.max_entries_per_ifd,
            });
        }

        let entries_bytes = entry_count
            .checked_mul(IFD_ENTRY_LEN)
            .ok_or(TiffError::DirectoryLengthOverflow { entry_count })?;
        let table_offset = offset_usize.checked_add(2).ok_or(TiffError::RangeOverflow {
            offset: offset_usize,
            length: 2,
        })?;
        let table = checked_slice(self.data, table_offset, entries_bytes)?;
        let next_offset_position =
            table_offset
                .checked_add(entries_bytes)
                .ok_or(TiffError::RangeOverflow {
                    offset: table_offset,
                    length: entries_bytes,
                })?;
        let next_ifd_offset = self
            .byte_order
            .read_u32(checked_slice(self.data, next_offset_position, 4)?);

        let mut entries = Vec::new();
        entries
            .try_reserve_exact(entry_count)
            .map_err(|_| TiffError::EntryAllocationFailed { entry_count })?;
        for encoded_entry in table.chunks_exact(IFD_ENTRY_LEN) {
            entries.push(self.parse_entry(encoded_entry)?);
        }

        Ok(Ifd {
            offset,
            entries,
            next_ifd_offset,
        })
    }

    /// Follows only the classic linked `next IFD` chain.
    ///
    /// The first repeated offset is rejected as a cycle.  `SubIFD` and Exif
    /// pointer tags remain under caller control and can use [`Self::parse_ifd`].
    pub fn linked_ifds(&self, start_offset: u32) -> Result<Vec<Ifd<'a>>, TiffError> {
        let mut offset = start_offset;
        let mut visited = HashSet::new();
        let mut ifds = Vec::new();

        while offset != 0 {
            if !visited.insert(offset) {
                return Err(TiffError::IfdCycle { offset });
            }
            if ifds.len() >= self.limits.max_ifd_depth {
                return Err(TiffError::IfdDepthLimitExceeded {
                    limit: self.limits.max_ifd_depth,
                });
            }
            let ifd = self.parse_ifd(offset)?;
            offset = ifd.next_ifd_offset;
            ifds.push(ifd);
        }

        Ok(ifds)
    }

    /// Follows the linked chain beginning at the header's root offset.
    pub fn root_ifds(&self) -> Result<Vec<Ifd<'a>>, TiffError> {
        self.linked_ifds(self.first_ifd_offset)
    }

    fn parse_entry(&self, encoded_entry: &'a [u8]) -> Result<Entry<'a>, TiffError> {
        let tag = self.byte_order.read_u16(&encoded_entry[..2]);
        let field_type = FieldType::from_code(tag, self.byte_order.read_u16(&encoded_entry[2..4]))?;
        let count = self.byte_order.read_u32(&encoded_entry[4..8]);
        let count_usize =
            usize::try_from(count).map_err(|_| TiffError::ValueCountDoesNotFit { tag, count })?;
        let element_size = field_type.element_size();
        let value_bytes = count_usize
            .checked_mul(element_size)
            .ok_or(TiffError::ValueLengthOverflow {
                tag,
                count,
                element_size,
            })?;
        if value_bytes > self.limits.max_value_bytes {
            return Err(TiffError::ValueLimitExceeded {
                tag,
                bytes: value_bytes,
                limit: self.limits.max_value_bytes,
            });
        }

        let value_or_offset = &encoded_entry[8..12];
        let encoded = if value_bytes <= INLINE_VALUE_LEN {
            &value_or_offset[..value_bytes]
        } else {
            let value_offset = self.byte_order.read_u32(value_or_offset);
            let value_offset_usize = usize::try_from(value_offset)
                .map_err(|_| TiffError::OffsetDoesNotFit { offset: value_offset })?;
            checked_slice(self.data, value_offset_usize, value_bytes)?
        };

        Ok(Entry {
            tag,
            field_type,
            count,
            encoded,
            byte_order: self.byte_order,
        })
    }
}

/// Malformed input or a configured resource-limit violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TiffError {
    RangeOverflow {
        offset: usize,
        length: usize,
    },
    Truncated {
        offset: usize,
        length: usize,
        data_len: usize,
    },
    InvalidByteOrder {
        marker: [u8; 2],
    },
    InvalidMagic {
        found: u16,
    },
    ZeroIfdOffset,
    OffsetDoesNotFit {
        offset: u32,
    },
    DirectoryLengthOverflow {
        entry_count: usize,
    },
    EntryLimitExceeded {
        offset: u32,
        count: usize,
        limit: usize,
    },
    EntryAllocationFailed {
        entry_count: usize,
    },
    IfdDepthLimitExceeded {
        limit: usize,
    },
    IfdCycle {
        offset: u32,
    },
    UnsupportedFieldType {
        tag: u16,
        code: u16,
    },
    ValueCountDoesNotFit {
        tag: u16,
        count: u32,
    },
    ValueLengthOverflow {
        tag: u16,
        count: u32,
        element_size: usize,
    },
    ValueLimitExceeded {
        tag: u16,
        bytes: usize,
        limit: usize,
    },
    ValueAllocationFailed {
        tag: u16,
        count: usize,
    },
    TypeMismatch {
        tag: u16,
        expected: FieldType,
        actual: FieldType,
    },
    ExpectedScalar {
        tag: u16,
        count: u32,
    },
    InvalidAscii {
        tag: u16,
        index: usize,
        byte: u8,
    },
    InvalidUtf8 {
        tag: u16,
    },
}

impl fmt::Display for TiffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RangeOverflow { offset, length } => {
                write!(formatter, "TIFF range {offset} + {length} overflows")
            }
            Self::Truncated {
                offset,
                length,
                data_len,
            } => write!(
                formatter,
                "TIFF range {offset}..+{length} exceeds {data_len}-byte payload"
            ),
            Self::InvalidByteOrder { marker } => {
                write!(formatter, "invalid TIFF byte-order marker {marker:02x?}")
            }
            Self::InvalidMagic { found } => {
                write!(formatter, "invalid classic TIFF magic {found}, expected 42")
            }
            Self::ZeroIfdOffset => formatter.write_str("cannot parse an IFD at offset zero"),
            Self::OffsetDoesNotFit { offset } => {
                write!(formatter, "TIFF offset {offset} does not fit this platform")
            }
            Self::DirectoryLengthOverflow { entry_count } => {
                write!(formatter, "IFD table length overflows for {entry_count} entries")
            }
            Self::EntryLimitExceeded { offset, count, limit } => write!(
                formatter,
                "IFD at {offset} has {count} entries, exceeding limit {limit}"
            ),
            Self::EntryAllocationFailed { entry_count } => {
                write!(formatter, "could not allocate {entry_count} IFD entries")
            }
            Self::IfdDepthLimitExceeded { limit } => {
                write!(formatter, "linked IFD chain exceeds depth limit {limit}")
            }
            Self::IfdCycle { offset } => {
                write!(formatter, "linked IFD chain repeats offset {offset}")
            }
            Self::UnsupportedFieldType { tag, code } => {
                write!(formatter, "tag {tag:#06x} uses unsupported TIFF type {code}")
            }
            Self::ValueCountDoesNotFit { tag, count } => write!(
                formatter,
                "tag {tag:#06x} count {count} does not fit this platform"
            ),
            Self::ValueLengthOverflow {
                tag,
                count,
                element_size,
            } => write!(
                formatter,
                "tag {tag:#06x} value length {count} * {element_size} overflows"
            ),
            Self::ValueLimitExceeded { tag, bytes, limit } => write!(
                formatter,
                "tag {tag:#06x} value has {bytes} bytes, exceeding limit {limit}"
            ),
            Self::ValueAllocationFailed { tag, count } => {
                write!(
                    formatter,
                    "could not allocate {count} decoded values for tag {tag:#06x}"
                )
            }
            Self::TypeMismatch {
                tag,
                expected,
                actual,
            } => write!(
                formatter,
                "tag {tag:#06x} has type {actual:?}, expected {expected:?}"
            ),
            Self::ExpectedScalar { tag, count } => {
                write!(formatter, "tag {tag:#06x} has {count} values, expected one")
            }
            Self::InvalidAscii { tag, index, byte } => write!(
                formatter,
                "tag {tag:#06x} has non-ASCII byte {byte:#04x} at index {index}"
            ),
            Self::InvalidUtf8 { tag } => {
                write!(formatter, "tag {tag:#06x} has invalid UTF-8")
            }
        }
    }
}

impl Error for TiffError {}

fn checked_slice(data: &[u8], offset: usize, length: usize) -> Result<&[u8], TiffError> {
    let end = offset
        .checked_add(length)
        .ok_or(TiffError::RangeOverflow { offset, length })?;
    data.get(offset..end).ok_or(TiffError::Truncated {
        offset,
        length,
        data_len: data.len(),
    })
}

fn collect_values<T>(tag: u16, count: usize, values: impl Iterator<Item = T>) -> Result<Vec<T>, TiffError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| TiffError::ValueAllocationFailed { tag, count })?;
    output.extend(values);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT_IFD: u32 = 8;

    #[derive(Clone, Copy)]
    struct TestEntry<'a> {
        tag: u16,
        field_type: FieldType,
        count: u32,
        value: &'a [u8],
    }

    fn push_u16(bytes: &mut Vec<u8>, order: ByteOrder, value: u16) {
        match order {
            ByteOrder::LittleEndian => bytes.extend_from_slice(&value.to_le_bytes()),
            ByteOrder::BigEndian => bytes.extend_from_slice(&value.to_be_bytes()),
        }
    }

    fn push_u32(bytes: &mut Vec<u8>, order: ByteOrder, value: u32) {
        match order {
            ByteOrder::LittleEndian => bytes.extend_from_slice(&value.to_le_bytes()),
            ByteOrder::BigEndian => bytes.extend_from_slice(&value.to_be_bytes()),
        }
    }

    fn tiff_with_entries(order: ByteOrder, entries: &[TestEntry<'_>]) -> Vec<u8> {
        let entry_count = u16::try_from(entries.len()).unwrap();
        let directory_len = 2 + entries.len() * IFD_ENTRY_LEN + 4;
        let mut next_payload_offset = usize::try_from(ROOT_IFD).unwrap() + directory_len;
        let mut payloads = Vec::new();
        let mut bytes = Vec::new();

        bytes.extend_from_slice(match order {
            ByteOrder::LittleEndian => b"II",
            ByteOrder::BigEndian => b"MM",
        });
        push_u16(&mut bytes, order, TIFF_MAGIC);
        push_u32(&mut bytes, order, ROOT_IFD);
        push_u16(&mut bytes, order, entry_count);

        for entry in entries {
            assert_eq!(
                entry.value.len(),
                usize::try_from(entry.count).unwrap() * entry.field_type.element_size()
            );
            push_u16(&mut bytes, order, entry.tag);
            push_u16(&mut bytes, order, entry.field_type as u16);
            push_u32(&mut bytes, order, entry.count);
            if entry.value.len() <= INLINE_VALUE_LEN {
                bytes.extend_from_slice(entry.value);
                bytes.resize(bytes.len() + INLINE_VALUE_LEN - entry.value.len(), 0);
            } else {
                push_u32(&mut bytes, order, u32::try_from(next_payload_offset).unwrap());
                payloads.extend_from_slice(entry.value);
                next_payload_offset += entry.value.len();
            }
        }
        push_u32(&mut bytes, order, 0);
        bytes.extend_from_slice(&payloads);
        bytes
    }

    fn encoded_u16(order: ByteOrder, values: &[u16]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for value in values {
            push_u16(&mut bytes, order, *value);
        }
        bytes
    }

    fn encoded_u32(order: ByteOrder, values: &[u32]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for value in values {
            push_u32(&mut bytes, order, *value);
        }
        bytes
    }

    fn encoded_i32(order: ByteOrder, values: &[i32]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for value in values {
            let encoded = match order {
                ByteOrder::LittleEndian => value.to_le_bytes(),
                ByteOrder::BigEndian => value.to_be_bytes(),
            };
            bytes.extend_from_slice(&encoded);
        }
        bytes
    }

    #[test]
    fn little_endian_inline_and_out_of_line_values() {
        let order = ByteOrder::LittleEndian;
        let short = encoded_u16(order, &[0x1234]);
        let longs = encoded_u32(order, &[10, 20]);
        let data = tiff_with_entries(
            order,
            &[
                TestEntry {
                    tag: 0x0100,
                    field_type: FieldType::Short,
                    count: 1,
                    value: &short,
                },
                TestEntry {
                    tag: 0x0101,
                    field_type: FieldType::Long,
                    count: 2,
                    value: &longs,
                },
                TestEntry {
                    tag: 0x0102,
                    field_type: FieldType::Ascii,
                    count: 6,
                    value: b"Canon\0",
                },
                TestEntry {
                    tag: 0x0103,
                    field_type: FieldType::Byte,
                    count: 3,
                    value: &[1, 2, 3],
                },
                TestEntry {
                    tag: 0x0104,
                    field_type: FieldType::Undefined,
                    count: 4,
                    value: &[4, 5, 6, 7],
                },
            ],
        );

        let tiff = Tiff::parse(&data).unwrap();
        assert_eq!(tiff.byte_order(), order);
        let ifd = tiff.first_ifd().unwrap().unwrap();
        assert_eq!(ifd.entry(0x0100).unwrap().short().unwrap(), 0x1234);
        assert_eq!(ifd.entry(0x0101).unwrap().long_values().unwrap(), vec![10, 20]);
        assert_eq!(ifd.entry(0x0102).unwrap().ascii().unwrap(), "Canon");
        assert_eq!(ifd.entry(0x0103).unwrap().byte_values().unwrap(), &[1, 2, 3]);
        assert_eq!(ifd.entry(0x0104).unwrap().undefined().unwrap(), &[4, 5, 6, 7]);
    }

    #[test]
    fn big_endian_arrays_and_signed_values() {
        let order = ByteOrder::BigEndian;
        let shorts = encoded_u16(order, &[0x1122, 0x3344]);
        let slong = encoded_i32(order, &[-7]);
        let srational = encoded_i32(order, &[-3, 2]);
        let data = tiff_with_entries(
            order,
            &[
                TestEntry {
                    tag: 1,
                    field_type: FieldType::Short,
                    count: 2,
                    value: &shorts,
                },
                TestEntry {
                    tag: 2,
                    field_type: FieldType::Slong,
                    count: 1,
                    value: &slong,
                },
                TestEntry {
                    tag: 3,
                    field_type: FieldType::Srational,
                    count: 1,
                    value: &srational,
                },
            ],
        );

        let tiff = Tiff::parse(&data).unwrap();
        let ifd = tiff.first_ifd().unwrap().unwrap();
        assert_eq!(
            ifd.entry(1).unwrap().short_values().unwrap(),
            vec![0x1122, 0x3344]
        );
        assert_eq!(ifd.entry(2).unwrap().slong().unwrap(), -7);
        assert_eq!(
            ifd.entry(3).unwrap().srational().unwrap(),
            SignedRational {
                numerator: -3,
                denominator: 2,
            }
        );
    }

    #[test]
    fn unsigned_rational_arrays_preserve_exact_components() {
        let order = ByteOrder::LittleEndian;
        let values = encoded_u32(order, &[1, 3, 5, 0]);
        let data = tiff_with_entries(
            order,
            &[TestEntry {
                tag: 0x829a,
                field_type: FieldType::Rational,
                count: 2,
                value: &values,
            }],
        );

        let ifd = Tiff::parse(&data).unwrap().first_ifd().unwrap().unwrap();
        let rationals = ifd.entry(0x829a).unwrap().rational_values().unwrap();
        assert_eq!(
            rationals,
            vec![
                Rational {
                    numerator: 1,
                    denominator: 3,
                },
                Rational {
                    numerator: 5,
                    denominator: 0,
                },
            ]
        );
        assert_eq!(rationals[0].as_f64(), Some(1.0 / 3.0));
        assert_eq!(rationals[1].as_f64(), None);
    }

    #[test]
    fn caller_can_follow_pointer_entry_explicitly() {
        let order = ByteOrder::LittleEndian;
        let pointer = encoded_u32(order, &[40]);
        let mut data = tiff_with_entries(
            order,
            &[TestEntry {
                tag: 0x014a,
                field_type: FieldType::Long,
                count: 1,
                value: &pointer,
            }],
        );
        data.resize(40, 0);
        push_u16(&mut data, order, 0);
        push_u32(&mut data, order, 0);

        let tiff = Tiff::parse(&data).unwrap();
        let root = tiff.first_ifd().unwrap().unwrap();
        let child_offset = root.entry(0x014a).unwrap().long().unwrap();
        let child = tiff.parse_ifd(child_offset).unwrap();
        assert_eq!(child.offset(), 40);
        assert!(child.entries().is_empty());
    }

    #[test]
    fn follows_linked_ifds_and_rejects_cycles() {
        let order = ByteOrder::LittleEndian;
        let mut linked = Vec::new();
        linked.extend_from_slice(b"II");
        push_u16(&mut linked, order, TIFF_MAGIC);
        push_u32(&mut linked, order, ROOT_IFD);
        push_u16(&mut linked, order, 0);
        push_u32(&mut linked, order, 14);
        push_u16(&mut linked, order, 0);
        push_u32(&mut linked, order, 0);
        let tiff = Tiff::parse(&linked).unwrap();
        let ifds = tiff.root_ifds().unwrap();
        assert_eq!(ifds.iter().map(Ifd::offset).collect::<Vec<_>>(), vec![8, 14]);

        let mut cyclic = linked;
        cyclic[10..14].copy_from_slice(&ROOT_IFD.to_le_bytes());
        let error = Tiff::parse(&cyclic).unwrap().root_ifds().unwrap_err();
        assert_eq!(error, TiffError::IfdCycle { offset: ROOT_IFD });
    }

    #[test]
    fn rejects_truncated_header_directory_and_value() {
        assert!(matches!(Tiff::parse(b"II"), Err(TiffError::Truncated { .. })));

        let mut directory = Vec::new();
        directory.extend_from_slice(b"II");
        push_u16(&mut directory, ByteOrder::LittleEndian, TIFF_MAGIC);
        push_u32(&mut directory, ByteOrder::LittleEndian, ROOT_IFD);
        let error = Tiff::parse(&directory).unwrap().first_ifd().unwrap_err();
        assert!(matches!(
            error,
            TiffError::Truncated {
                offset: 8,
                length: 2,
                ..
            }
        ));

        let values = encoded_u32(ByteOrder::LittleEndian, &[1, 2]);
        let mut out_of_line = tiff_with_entries(
            ByteOrder::LittleEndian,
            &[TestEntry {
                tag: 1,
                field_type: FieldType::Long,
                count: 2,
                value: &values,
            }],
        );
        out_of_line[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
        let error = Tiff::parse(&out_of_line).unwrap().first_ifd().unwrap_err();
        assert!(matches!(error, TiffError::Truncated { .. }));
    }

    #[test]
    fn checked_range_reports_arithmetic_overflow() {
        let error = checked_slice(&[], usize::MAX, 2).unwrap_err();
        assert_eq!(
            error,
            TiffError::RangeOverflow {
                offset: usize::MAX,
                length: 2,
            }
        );
    }

    #[test]
    fn enforces_entry_depth_and_value_limits() {
        let order = ByteOrder::LittleEndian;
        let short = encoded_u16(order, &[1]);
        let data = tiff_with_entries(
            order,
            &[TestEntry {
                tag: 1,
                field_type: FieldType::Short,
                count: 1,
                value: &short,
            }],
        );
        let limits = ParseLimits {
            max_entries_per_ifd: 0,
            ..ParseLimits::default()
        };
        assert!(matches!(
            Tiff::parse_with_limits(&data, limits).unwrap().first_ifd(),
            Err(TiffError::EntryLimitExceeded { .. })
        ));

        let values = encoded_u32(order, &[1, 2]);
        let data = tiff_with_entries(
            order,
            &[TestEntry {
                tag: 2,
                field_type: FieldType::Long,
                count: 2,
                value: &values,
            }],
        );
        let limits = ParseLimits {
            max_value_bytes: 7,
            ..ParseLimits::default()
        };
        assert!(matches!(
            Tiff::parse_with_limits(&data, limits).unwrap().first_ifd(),
            Err(TiffError::ValueLimitExceeded {
                bytes: 8,
                limit: 7,
                ..
            })
        ));

        let mut one_ifd = Vec::new();
        one_ifd.extend_from_slice(b"II");
        push_u16(&mut one_ifd, order, TIFF_MAGIC);
        push_u32(&mut one_ifd, order, ROOT_IFD);
        push_u16(&mut one_ifd, order, 0);
        push_u32(&mut one_ifd, order, 0);
        let limits = ParseLimits {
            max_ifd_depth: 0,
            ..ParseLimits::default()
        };
        assert_eq!(
            Tiff::parse_with_limits(&one_ifd, limits)
                .unwrap()
                .root_ifds()
                .unwrap_err(),
            TiffError::IfdDepthLimitExceeded { limit: 0 }
        );
    }

    #[test]
    fn typed_accessors_reject_wrong_types_and_non_scalars() {
        let order = ByteOrder::LittleEndian;
        let values = encoded_u16(order, &[1, 2]);
        let data = tiff_with_entries(
            order,
            &[TestEntry {
                tag: 7,
                field_type: FieldType::Short,
                count: 2,
                value: &values,
            }],
        );
        let ifd = Tiff::parse(&data).unwrap().first_ifd().unwrap().unwrap();
        let entry = ifd.entry(7).unwrap();
        assert!(matches!(entry.long(), Err(TiffError::TypeMismatch { .. })));
        assert_eq!(
            entry.short().unwrap_err(),
            TiffError::ExpectedScalar { tag: 7, count: 2 }
        );
    }
}
