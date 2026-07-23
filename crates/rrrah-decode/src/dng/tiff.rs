//! Bounded TIFF/BigTIFF directory parsing used by the native DNG reader.

use std::{error::Error, fmt};

const CLASSIC_MAGIC: u16 = 42;
const BIG_TIFF_MAGIC: u16 = 43;
const BIG_TIFF_OFFSET_SIZE: u16 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ByteOrder {
    Little,
    Big,
}

impl ByteOrder {
    pub(crate) fn u16(self, bytes: &[u8]) -> u16 {
        let bytes = [bytes[0], bytes[1]];
        match self {
            Self::Little => u16::from_le_bytes(bytes),
            Self::Big => u16::from_be_bytes(bytes),
        }
    }

    pub(crate) fn i16(self, bytes: &[u8]) -> i16 {
        let bytes = [bytes[0], bytes[1]];
        match self {
            Self::Little => i16::from_le_bytes(bytes),
            Self::Big => i16::from_be_bytes(bytes),
        }
    }

    pub(crate) fn u32(self, bytes: &[u8]) -> u32 {
        let bytes = [bytes[0], bytes[1], bytes[2], bytes[3]];
        match self {
            Self::Little => u32::from_le_bytes(bytes),
            Self::Big => u32::from_be_bytes(bytes),
        }
    }

    pub(crate) fn i32(self, bytes: &[u8]) -> i32 {
        let bytes = [bytes[0], bytes[1], bytes[2], bytes[3]];
        match self {
            Self::Little => i32::from_le_bytes(bytes),
            Self::Big => i32::from_be_bytes(bytes),
        }
    }

    pub(crate) fn u64(self, bytes: &[u8]) -> u64 {
        let bytes = [
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ];
        match self {
            Self::Little => u64::from_le_bytes(bytes),
            Self::Big => u64::from_be_bytes(bytes),
        }
    }

    pub(crate) fn i64(self, bytes: &[u8]) -> i64 {
        let bytes = [
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ];
        match self {
            Self::Little => i64::from_le_bytes(bytes),
            Self::Big => i64::from_be_bytes(bytes),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Variant {
    Classic,
    BigTiff,
}

impl Variant {
    const fn entry_count_width(self) -> usize {
        match self {
            Self::Classic => 2,
            Self::BigTiff => 8,
        }
    }

    const fn entry_width(self) -> usize {
        match self {
            Self::Classic => 12,
            Self::BigTiff => 20,
        }
    }

    const fn inline_width(self) -> usize {
        match self {
            Self::Classic => 4,
            Self::BigTiff => 8,
        }
    }

    const fn next_ifd_width(self) -> usize {
        self.inline_width()
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
    pub(crate) max_entries_per_ifd: usize,
    pub(crate) max_value_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries_per_ifd: 4_096,
            max_value_bytes: 256 * 1024 * 1024,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Tiff<'a> {
    data: &'a [u8],
    byte_order: ByteOrder,
    variant: Variant,
    first_ifd_offset: u64,
    limits: Limits,
}

impl<'a> Tiff<'a> {
    pub(crate) fn parse(data: &'a [u8], limits: Limits) -> Result<Self, TiffError> {
        let byte_order = match data.get(0..2) {
            Some(b"II") => ByteOrder::Little,
            Some(b"MM") => ByteOrder::Big,
            _ => return Err(TiffError::InvalidByteOrder),
        };
        let magic = read_slice(data, 2, 2, "TIFF magic").map(|bytes| byte_order.u16(bytes))?;
        let (variant, first_ifd_offset) = match magic {
            CLASSIC_MAGIC => {
                let bytes = read_slice(data, 4, 4, "classic TIFF first IFD")?;
                (Variant::Classic, u64::from(byte_order.u32(bytes)))
            }
            BIG_TIFF_MAGIC => {
                let offset_size = byte_order.u16(read_slice(data, 4, 2, "BigTIFF offset size")?);
                let reserved = byte_order.u16(read_slice(data, 6, 2, "BigTIFF reserved word")?);
                if offset_size != BIG_TIFF_OFFSET_SIZE || reserved != 0 {
                    return Err(TiffError::InvalidBigTiffHeader {
                        offset_size,
                        reserved,
                    });
                }
                let offset = byte_order.u64(read_slice(data, 8, 8, "BigTIFF first IFD")?);
                (Variant::BigTiff, offset)
            }
            _ => return Err(TiffError::InvalidMagic { actual: magic }),
        };
        if first_ifd_offset == 0 {
            return Err(TiffError::MissingFirstIfd);
        }
        Ok(Self {
            data,
            byte_order,
            variant,
            first_ifd_offset,
            limits,
        })
    }

    pub(crate) const fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    #[cfg(test)]
    pub(crate) const fn variant(&self) -> Variant {
        self.variant
    }

    pub(crate) const fn first_ifd_offset(&self) -> u64 {
        self.first_ifd_offset
    }

    pub(crate) fn parse_ifd(&self, offset: u64) -> Result<Ifd<'a>, TiffError> {
        let offset = to_usize(offset, "IFD offset")?;
        let count_width = self.variant.entry_count_width();
        let count_bytes = read_slice(self.data, offset, count_width, "IFD entry count")?;
        let entry_count_u64 = match self.variant {
            Variant::Classic => u64::from(self.byte_order.u16(count_bytes)),
            Variant::BigTiff => self.byte_order.u64(count_bytes),
        };
        let entry_count = to_usize(entry_count_u64, "IFD entry count")?;
        if entry_count > self.limits.max_entries_per_ifd {
            return Err(TiffError::EntryLimit {
                actual: entry_count,
                limit: self.limits.max_entries_per_ifd,
            });
        }

        let entries_start = offset
            .checked_add(count_width)
            .ok_or(TiffError::ArithmeticOverflow("IFD entries start"))?;
        let table_bytes = entry_count
            .checked_mul(self.variant.entry_width())
            .ok_or(TiffError::ArithmeticOverflow("IFD table byte length"))?;
        let entries_end = entries_start
            .checked_add(table_bytes)
            .ok_or(TiffError::ArithmeticOverflow("IFD table end"))?;
        let next_end = entries_end
            .checked_add(self.variant.next_ifd_width())
            .ok_or(TiffError::ArithmeticOverflow("next IFD field end"))?;
        read_slice(
            self.data,
            entries_start,
            next_end - entries_start,
            "IFD table and next offset",
        )?;

        let mut entries = Vec::new();
        entries
            .try_reserve_exact(entry_count)
            .map_err(|_| TiffError::AllocationFailed {
                elements: entry_count,
            })?;
        for index in 0..entry_count {
            let relative = index
                .checked_mul(self.variant.entry_width())
                .ok_or(TiffError::ArithmeticOverflow("IFD entry position"))?;
            let entry_offset = entries_start
                .checked_add(relative)
                .ok_or(TiffError::ArithmeticOverflow("IFD entry offset"))?;
            entries.push(self.parse_entry(entry_offset)?);
        }

        let next_bytes = &self.data[entries_end..next_end];
        let next_ifd_offset = match self.variant {
            Variant::Classic => u64::from(self.byte_order.u32(next_bytes)),
            Variant::BigTiff => self.byte_order.u64(next_bytes),
        };
        Ok(Ifd {
            offset: u64::try_from(offset).map_err(|_| TiffError::ArithmeticOverflow("IFD offset"))?,
            entries,
            next_ifd_offset,
        })
    }

    fn parse_entry(&self, offset: usize) -> Result<Entry<'a>, TiffError> {
        let entry_bytes = read_slice(self.data, offset, self.variant.entry_width(), "IFD entry")?;
        let tag = self.byte_order.u16(&entry_bytes[0..2]);
        let type_code = self.byte_order.u16(&entry_bytes[2..4]);
        let field_type = FieldType::from_code(tag, type_code)?;
        let count = match self.variant {
            Variant::Classic => u64::from(self.byte_order.u32(&entry_bytes[4..8])),
            Variant::BigTiff => self.byte_order.u64(&entry_bytes[4..12]),
        };
        let byte_len_u64 = count
            .checked_mul(field_type.element_width())
            .ok_or(TiffError::ArithmeticOverflow("IFD value byte length"))?;
        let byte_len = to_usize(byte_len_u64, "IFD value byte length")?;
        if byte_len > self.limits.max_value_bytes {
            return Err(TiffError::ValueLimit {
                tag,
                actual: byte_len,
                limit: self.limits.max_value_bytes,
            });
        }

        let value_field_start = match self.variant {
            Variant::Classic => 8,
            Variant::BigTiff => 12,
        };
        let value_field = &entry_bytes[value_field_start..value_field_start + self.variant.inline_width()];
        let encoded = if byte_len <= self.variant.inline_width() {
            &value_field[..byte_len]
        } else {
            let value_offset = match self.variant {
                Variant::Classic => u64::from(self.byte_order.u32(value_field)),
                Variant::BigTiff => self.byte_order.u64(value_field),
            };
            let value_offset = to_usize(value_offset, "IFD value offset")?;
            read_slice(self.data, value_offset, byte_len, "IFD value")?
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

#[derive(Debug)]
pub(crate) struct Ifd<'a> {
    pub(crate) offset: u64,
    pub(crate) entries: Vec<Entry<'a>>,
    pub(crate) next_ifd_offset: u64,
}

impl<'a> Ifd<'a> {
    pub(crate) fn entry(&self, tag: u16) -> Result<Option<&Entry<'a>>, TiffError> {
        let mut matches = self.entries.iter().filter(|entry| entry.tag == tag);
        let first = matches.next();
        if first.is_some() && matches.next().is_some() {
            return Err(TiffError::DuplicateTag {
                ifd_offset: self.offset,
                tag,
            });
        }
        Ok(first)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldType {
    Byte,
    Ascii,
    Short,
    Long,
    Rational,
    Sbyte,
    Undefined,
    Sshort,
    Slong,
    Srational,
    Float,
    Double,
    Ifd,
    Long8,
    Slong8,
    Ifd8,
}

impl FieldType {
    fn from_code(tag: u16, code: u16) -> Result<Self, TiffError> {
        match code {
            1 => Ok(Self::Byte),
            2 => Ok(Self::Ascii),
            3 => Ok(Self::Short),
            4 => Ok(Self::Long),
            5 => Ok(Self::Rational),
            6 => Ok(Self::Sbyte),
            7 => Ok(Self::Undefined),
            8 => Ok(Self::Sshort),
            9 => Ok(Self::Slong),
            10 => Ok(Self::Srational),
            11 => Ok(Self::Float),
            12 => Ok(Self::Double),
            13 => Ok(Self::Ifd),
            16 => Ok(Self::Long8),
            17 => Ok(Self::Slong8),
            18 => Ok(Self::Ifd8),
            _ => Err(TiffError::UnsupportedFieldType { tag, code }),
        }
    }

    const fn element_width(self) -> u64 {
        match self {
            Self::Byte | Self::Ascii | Self::Sbyte | Self::Undefined => 1,
            Self::Short | Self::Sshort => 2,
            Self::Long | Self::Slong | Self::Float | Self::Ifd => 4,
            Self::Rational | Self::Srational | Self::Double | Self::Long8 | Self::Slong8 | Self::Ifd8 => 8,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Entry<'a> {
    pub(crate) tag: u16,
    pub(crate) field_type: FieldType,
    pub(crate) count: u64,
    encoded: &'a [u8],
    byte_order: ByteOrder,
}

impl<'a> Entry<'a> {
    pub(crate) const fn raw_bytes(&self) -> &'a [u8] {
        self.encoded
    }

    pub(crate) fn unsigned_values(&self) -> Result<Vec<u64>, TiffError> {
        let count = to_usize(self.count, "unsigned value count")?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| TiffError::AllocationFailed { elements: count })?;
        match self.field_type {
            FieldType::Byte => values.extend(self.encoded.iter().map(|value| u64::from(*value))),
            FieldType::Short => values.extend(
                self.encoded
                    .chunks_exact(2)
                    .map(|bytes| u64::from(self.byte_order.u16(bytes))),
            ),
            FieldType::Long | FieldType::Ifd => values.extend(
                self.encoded
                    .chunks_exact(4)
                    .map(|bytes| u64::from(self.byte_order.u32(bytes))),
            ),
            FieldType::Long8 | FieldType::Ifd8 => values.extend(
                self.encoded
                    .chunks_exact(8)
                    .map(|bytes| self.byte_order.u64(bytes)),
            ),
            actual => {
                return Err(TiffError::TypeMismatch {
                    tag: self.tag,
                    expected: "BYTE, SHORT, LONG, IFD, LONG8, or IFD8",
                    actual,
                });
            }
        }
        Ok(values)
    }

    pub(crate) fn unsigned_scalar(&self) -> Result<u64, TiffError> {
        if self.count != 1 {
            return Err(TiffError::CountMismatch {
                tag: self.tag,
                expected: 1,
                actual: self.count,
            });
        }
        self.unsigned_values()?
            .into_iter()
            .next()
            .ok_or(TiffError::CountMismatch {
                tag: self.tag,
                expected: 1,
                actual: 0,
            })
    }

    pub(crate) fn numeric_values(&self) -> Result<Vec<f64>, TiffError> {
        let count = to_usize(self.count, "numeric value count")?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| TiffError::AllocationFailed { elements: count })?;
        match self.field_type {
            FieldType::Byte => values.extend(self.encoded.iter().map(|value| f64::from(*value))),
            FieldType::Sbyte => values.extend(
                self.encoded
                    .iter()
                    .map(|value| f64::from(i8::from_ne_bytes([*value]))),
            ),
            FieldType::Short => values.extend(
                self.encoded
                    .chunks_exact(2)
                    .map(|bytes| f64::from(self.byte_order.u16(bytes))),
            ),
            FieldType::Sshort => values.extend(
                self.encoded
                    .chunks_exact(2)
                    .map(|bytes| f64::from(self.byte_order.i16(bytes))),
            ),
            FieldType::Long | FieldType::Ifd => values.extend(
                self.encoded
                    .chunks_exact(4)
                    .map(|bytes| f64::from(self.byte_order.u32(bytes))),
            ),
            FieldType::Slong => values.extend(
                self.encoded
                    .chunks_exact(4)
                    .map(|bytes| f64::from(self.byte_order.i32(bytes))),
            ),
            FieldType::Rational => {
                for bytes in self.encoded.chunks_exact(8) {
                    let denominator = self.byte_order.u32(&bytes[4..8]);
                    if denominator == 0 {
                        return Err(TiffError::ZeroDenominator { tag: self.tag });
                    }
                    values.push(f64::from(self.byte_order.u32(&bytes[..4])) / f64::from(denominator));
                }
            }
            FieldType::Srational => {
                for bytes in self.encoded.chunks_exact(8) {
                    let denominator = self.byte_order.i32(&bytes[4..8]);
                    if denominator == 0 {
                        return Err(TiffError::ZeroDenominator { tag: self.tag });
                    }
                    values.push(f64::from(self.byte_order.i32(&bytes[..4])) / f64::from(denominator));
                }
            }
            FieldType::Float => values.extend(
                self.encoded
                    .chunks_exact(4)
                    .map(|bytes| f64::from(f32::from_bits(self.byte_order.u32(bytes)))),
            ),
            FieldType::Double => values.extend(
                self.encoded
                    .chunks_exact(8)
                    .map(|bytes| f64::from_bits(self.byte_order.u64(bytes))),
            ),
            FieldType::Long8 | FieldType::Ifd8 => values.extend(
                self.encoded
                    .chunks_exact(8)
                    .map(|bytes| self.byte_order.u64(bytes) as f64),
            ),
            FieldType::Slong8 => values.extend(
                self.encoded
                    .chunks_exact(8)
                    .map(|bytes| self.byte_order.i64(bytes) as f64),
            ),
            FieldType::Ascii | FieldType::Undefined => {
                return Err(TiffError::TypeMismatch {
                    tag: self.tag,
                    expected: "numeric TIFF field",
                    actual: self.field_type,
                });
            }
        }
        if values.iter().any(|value| !value.is_finite()) {
            return Err(TiffError::NonFiniteValue { tag: self.tag });
        }
        Ok(values)
    }

    pub(crate) fn ascii(&self) -> Result<&'a str, TiffError> {
        if self.field_type != FieldType::Ascii {
            return Err(TiffError::TypeMismatch {
                tag: self.tag,
                expected: "ASCII",
                actual: self.field_type,
            });
        }
        let trimmed = self.encoded.strip_suffix(&[0]).unwrap_or(self.encoded);
        if trimmed.contains(&0) {
            return Err(TiffError::EmbeddedAsciiNul { tag: self.tag });
        }
        std::str::from_utf8(trimmed).map_err(|_| TiffError::InvalidUtf8 { tag: self.tag })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TiffError {
    InvalidByteOrder,
    Truncated {
        context: &'static str,
        offset: usize,
        needed: usize,
        available: usize,
    },
    InvalidMagic {
        actual: u16,
    },
    InvalidBigTiffHeader {
        offset_size: u16,
        reserved: u16,
    },
    MissingFirstIfd,
    UnsupportedFieldType {
        tag: u16,
        code: u16,
    },
    EntryLimit {
        actual: usize,
        limit: usize,
    },
    ValueLimit {
        tag: u16,
        actual: usize,
        limit: usize,
    },
    DuplicateTag {
        ifd_offset: u64,
        tag: u16,
    },
    TypeMismatch {
        tag: u16,
        expected: &'static str,
        actual: FieldType,
    },
    CountMismatch {
        tag: u16,
        expected: u64,
        actual: u64,
    },
    ZeroDenominator {
        tag: u16,
    },
    NonFiniteValue {
        tag: u16,
    },
    EmbeddedAsciiNul {
        tag: u16,
    },
    InvalidUtf8 {
        tag: u16,
    },
    ArithmeticOverflow(&'static str),
    AllocationFailed {
        elements: usize,
    },
}

impl fmt::Display for TiffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidByteOrder => formatter.write_str("invalid TIFF byte-order marker"),
            Self::Truncated {
                context,
                offset,
                needed,
                available,
            } => write!(
                formatter,
                "truncated {context} at offset {offset}: need {needed} bytes, have {available}"
            ),
            Self::InvalidMagic { actual } => {
                write!(formatter, "invalid TIFF magic {actual}, expected 42 or 43")
            }
            Self::InvalidBigTiffHeader {
                offset_size,
                reserved,
            } => write!(
                formatter,
                "invalid BigTIFF header: offset size {offset_size}, reserved {reserved}"
            ),
            Self::MissingFirstIfd => formatter.write_str("TIFF first IFD offset is zero"),
            Self::UnsupportedFieldType { tag, code } => {
                write!(formatter, "TIFF tag {tag} uses unsupported field type {code}")
            }
            Self::EntryLimit { actual, limit } => {
                write!(formatter, "TIFF IFD has {actual} entries, limit is {limit}")
            }
            Self::ValueLimit { tag, actual, limit } => write!(
                formatter,
                "TIFF tag {tag} has {actual} value bytes, limit is {limit}"
            ),
            Self::DuplicateTag { ifd_offset, tag } => {
                write!(formatter, "TIFF IFD at {ifd_offset} repeats tag {tag}")
            }
            Self::TypeMismatch {
                tag,
                expected,
                actual,
            } => write!(
                formatter,
                "TIFF tag {tag} has type {actual:?}, expected {expected}"
            ),
            Self::CountMismatch {
                tag,
                expected,
                actual,
            } => write!(
                formatter,
                "TIFF tag {tag} has count {actual}, expected {expected}"
            ),
            Self::ZeroDenominator { tag } => {
                write!(formatter, "TIFF tag {tag} contains a zero denominator")
            }
            Self::NonFiniteValue { tag } => {
                write!(formatter, "TIFF tag {tag} contains a non-finite value")
            }
            Self::EmbeddedAsciiNul { tag } => {
                write!(formatter, "TIFF ASCII tag {tag} contains an embedded NUL")
            }
            Self::InvalidUtf8 { tag } => write!(formatter, "TIFF ASCII tag {tag} is not UTF-8"),
            Self::ArithmeticOverflow(context) => {
                write!(formatter, "arithmetic overflow while computing {context}")
            }
            Self::AllocationFailed { elements } => {
                write!(formatter, "could not allocate {elements} TIFF elements")
            }
        }
    }
}

impl Error for TiffError {}

fn read_slice<'a>(
    data: &'a [u8],
    offset: usize,
    length: usize,
    context: &'static str,
) -> Result<&'a [u8], TiffError> {
    let end = offset
        .checked_add(length)
        .ok_or(TiffError::ArithmeticOverflow(context))?;
    data.get(offset..end).ok_or_else(|| TiffError::Truncated {
        context,
        offset,
        needed: length,
        available: data.len().saturating_sub(offset),
    })
}

fn to_usize(value: u64, context: &'static str) -> Result<usize, TiffError> {
    usize::try_from(value).map_err(|_| TiffError::ArithmeticOverflow(context))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_classic_little_endian_inline_and_out_of_line_values() {
        let mut bytes = vec![0_u8; 48];
        bytes[..8].copy_from_slice(&[b'I', b'I', 42, 0, 8, 0, 0, 0]);
        bytes[8..10].copy_from_slice(&2_u16.to_le_bytes());
        write_classic_entry(&mut bytes, 10, 256, 3, 1, [7, 0, 0, 0]);
        write_classic_entry(&mut bytes, 22, 50714, 5, 1, 40_u32.to_le_bytes());
        bytes[34..38].copy_from_slice(&0_u32.to_le_bytes());
        bytes[40..44].copy_from_slice(&3_u32.to_le_bytes());
        bytes[44..48].copy_from_slice(&2_u32.to_le_bytes());

        let tiff = Tiff::parse(&bytes, Limits::default()).unwrap();
        assert_eq!(tiff.variant(), Variant::Classic);
        let ifd = tiff.parse_ifd(tiff.first_ifd_offset()).unwrap();
        assert_eq!(ifd.entry(256).unwrap().unwrap().unsigned_scalar().unwrap(), 7);
        assert_eq!(
            ifd.entry(50714).unwrap().unwrap().numeric_values().unwrap(),
            [1.5]
        );
    }

    #[test]
    fn parses_big_tiff_and_ifd8_offsets() {
        let mut bytes = vec![0_u8; 64];
        bytes[..16].copy_from_slice(&[b'M', b'M', 0, 43, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 16]);
        bytes[16..24].copy_from_slice(&1_u64.to_be_bytes());
        bytes[24..26].copy_from_slice(&330_u16.to_be_bytes());
        bytes[26..28].copy_from_slice(&18_u16.to_be_bytes());
        bytes[28..36].copy_from_slice(&1_u64.to_be_bytes());
        bytes[36..44].copy_from_slice(&52_u64.to_be_bytes());
        bytes[44..52].copy_from_slice(&0_u64.to_be_bytes());
        bytes[52..60].copy_from_slice(&0_u64.to_be_bytes());

        let tiff = Tiff::parse(&bytes, Limits::default()).unwrap();
        assert_eq!(tiff.variant(), Variant::BigTiff);
        assert_eq!(
            tiff.parse_ifd(16)
                .unwrap()
                .entry(330)
                .unwrap()
                .unwrap()
                .unsigned_values()
                .unwrap(),
            [52]
        );
    }

    #[test]
    fn rejects_out_of_range_values_before_borrowing() {
        let mut bytes = vec![0_u8; 26];
        bytes[..8].copy_from_slice(&[b'I', b'I', 42, 0, 8, 0, 0, 0]);
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        write_classic_entry(&mut bytes, 10, 50714, 5, 1, u32::MAX.to_le_bytes());
        bytes[22..26].copy_from_slice(&0_u32.to_le_bytes());

        assert!(matches!(
            Tiff::parse(&bytes, Limits::default()).unwrap().parse_ifd(8),
            Err(TiffError::Truncated {
                context: "IFD value",
                ..
            })
        ));
    }

    fn write_classic_entry(
        bytes: &mut [u8],
        offset: usize,
        tag: u16,
        field_type: u16,
        count: u32,
        value: [u8; 4],
    ) {
        bytes[offset..offset + 2].copy_from_slice(&tag.to_le_bytes());
        bytes[offset + 2..offset + 4].copy_from_slice(&field_type.to_le_bytes());
        bytes[offset + 4..offset + 8].copy_from_slice(&count.to_le_bytes());
        bytes[offset + 8..offset + 12].copy_from_slice(&value);
    }
}
