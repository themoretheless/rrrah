//! Bounded parsing of Canon timed-metadata (`CTMD`) samples.
//!
//! A CTMD sample is a sequence of records whose first little-endian word is
//! the complete record length. Some records carry a standalone classic-TIFF
//! payload after a 20-byte record header. This module only assigns meaning to
//! the EOS R8 white-balance layout established from the local clean-room
//! fixtures; other layouts are rejected rather than guessed.

use std::{error::Error, fmt};

use super::tiff::{Entry, FieldType, Tiff, TiffError};

const RECORD_LENGTH_LEN: usize = 4;
const TIFF_RECORD_HEADER_LEN: usize = 20;
const TIFF_SIGNATURE_LEN: usize = 4;

const TAG_CANON_LEVELS: u16 = 0x4001;
const EOS_R8_LEVEL_COUNT: u32 = 3_778;
const EOS_R8_LAYOUT_MARKER: u16 = 48;
const LAYOUT_MARKER_INDEX: usize = 0;
const RED_NUMERATOR_INDEX: usize = 105;
const RED_DENOMINATOR_INDEX: usize = 106;
const SECOND_GREEN_INDEX: usize = 107;
const BLUE_NUMERATOR_INDEX: usize = 108;

/// Resource limits for one already bounded CTMD sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ParseLimits {
    pub(crate) max_records: usize,
    pub(crate) max_record_bytes: usize,
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_records: 4_096,
            max_record_bytes: 16 * 1024 * 1024,
        }
    }
}

/// One record whose byte range is contained entirely in the CTMD sample.
#[derive(Clone, Copy)]
pub(crate) struct CtmdRecord<'a> {
    index: usize,
    offset: usize,
    bytes: &'a [u8],
}

impl<'a> CtmdRecord<'a> {
    pub(crate) const fn index(self) -> usize {
        self.index
    }

    pub(crate) const fn offset(self) -> usize {
        self.offset
    }

    pub(crate) const fn len(self) -> usize {
        self.bytes.len()
    }

    pub(crate) const fn is_empty(self) -> bool {
        self.bytes.is_empty()
    }

    /// Returns a structurally identified classic-TIFF payload.
    ///
    /// A merely long record is not treated as TIFF: the byte-order and classic
    /// TIFF magic at the record-relative payload boundary must also match.
    pub(crate) fn classic_tiff_payload(self) -> Option<&'a [u8]> {
        let payload = self.bytes.get(TIFF_RECORD_HEADER_LEN..)?;
        is_classic_tiff_header(payload).then_some(payload)
    }
}

impl fmt::Debug for CtmdRecord<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CtmdRecord")
            .field("index", &self.index)
            .field("offset", &self.offset)
            .field("byte_len", &self.bytes.len())
            .field("has_classic_tiff", &self.classic_tiff_payload().is_some())
            .finish()
    }
}

/// A CTMD sample split into validated, non-overlapping records.
#[derive(Clone, Debug)]
pub(crate) struct Ctmd<'a> {
    records: Vec<CtmdRecord<'a>>,
}

impl<'a> Ctmd<'a> {
    pub(crate) fn parse(data: &'a [u8]) -> Result<Self, CtmdError> {
        Self::parse_with_limits(data, ParseLimits::default())
    }

    pub(crate) fn parse_with_limits(data: &'a [u8], limits: ParseLimits) -> Result<Self, CtmdError> {
        let mut records = Vec::new();
        let mut offset = 0usize;

        while offset < data.len() {
            let remaining = data.len() - offset;
            if remaining < RECORD_LENGTH_LEN {
                return Err(CtmdError::TruncatedRecordLength { offset, remaining });
            }

            let length_bytes = data
                .get(offset..offset + RECORD_LENGTH_LEN)
                .ok_or(CtmdError::TruncatedRecordLength { offset, remaining })?;
            let declared_u32 =
                u32::from_le_bytes([length_bytes[0], length_bytes[1], length_bytes[2], length_bytes[3]]);
            let declared = usize::try_from(declared_u32).map_err(|_| CtmdError::RecordLengthDoesNotFit {
                record_index: records.len(),
                declared: declared_u32,
            })?;

            if declared < RECORD_LENGTH_LEN {
                return Err(CtmdError::RecordTooShort {
                    record_index: records.len(),
                    offset,
                    declared,
                    minimum: RECORD_LENGTH_LEN,
                });
            }
            if declared > limits.max_record_bytes {
                return Err(CtmdError::RecordByteLimitExceeded {
                    record_index: records.len(),
                    declared,
                    limit: limits.max_record_bytes,
                });
            }
            if records.len() >= limits.max_records {
                return Err(CtmdError::RecordCountLimitExceeded {
                    count: records.len() + 1,
                    limit: limits.max_records,
                });
            }

            let end = checked_record_end(records.len(), offset, declared)?;
            if end > data.len() {
                return Err(CtmdError::TruncatedRecord {
                    record_index: records.len(),
                    offset,
                    declared,
                    available: remaining,
                });
            }
            records
                .try_reserve(1)
                .map_err(|_| CtmdError::RecordAllocationFailed {
                    requested: records.len() + 1,
                })?;
            records.push(CtmdRecord {
                index: records.len(),
                offset,
                bytes: &data[offset..end],
            });
            offset = end;
        }

        Ok(Self { records })
    }

    pub(crate) fn records(&self) -> &[CtmdRecord<'a>] {
        &self.records
    }
}

/// Exact integer ratios behind the EOS R8 as-shot white balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EosR8AsShotWhiteBalance {
    pub(crate) red_numerator: u16,
    pub(crate) red_denominator: u16,
    pub(crate) blue_numerator: u16,
    pub(crate) second_green: u16,
}

impl EosR8AsShotWhiteBalance {
    /// Converts RGGB correction levels to RGBG gains relative to the first green.
    pub(crate) fn gains(self) -> [f32; 4] {
        [
            f32::from(self.red_numerator) / f32::from(self.red_denominator),
            1.0,
            f32::from(self.blue_numerator) / f32::from(self.red_denominator),
            f32::from(self.second_green) / f32::from(self.red_denominator),
        ]
    }
}

/// Extracts the unique supported EOS R8 as-shot white balance from CTMD.
pub(crate) fn extract_eos_r8_as_shot_white_balance(
    data: &[u8],
) -> Result<EosR8AsShotWhiteBalance, CtmdError> {
    let (record_index, entry) = find_eos_r8_levels(data)?;
    decode_eos_r8_white_balance(record_index, &entry)
}

fn find_eos_r8_levels(data: &[u8]) -> Result<(usize, Entry<'_>), CtmdError> {
    let ctmd = Ctmd::parse(data)?;
    let mut candidate: Option<(usize, Entry<'_>)> = None;

    for record in ctmd.records() {
        let Some(payload) = record.classic_tiff_payload() else {
            continue;
        };
        let tiff = Tiff::parse(payload).map_err(|source| CtmdError::InvalidEmbeddedTiff {
            record_index: record.index(),
            record_offset: record.offset(),
            source,
        })?;
        let ifd = tiff
            .first_ifd()
            .map_err(|source| CtmdError::InvalidEmbeddedTiff {
                record_index: record.index(),
                record_offset: record.offset(),
                source,
            })?
            .ok_or(CtmdError::MissingRootIfd {
                record_index: record.index(),
                record_offset: record.offset(),
            })?;

        let mut entries = ifd.entries_with_tag(TAG_CANON_LEVELS);
        let Some(entry) = entries.next() else {
            continue;
        };
        if entries.next().is_some() {
            let occurrences = 2usize.saturating_add(entries.count());
            return Err(CtmdError::AmbiguousWhiteBalanceTags {
                record_index: record.index(),
                occurrences,
            });
        }
        if let Some((first_record_index, _)) = candidate {
            return Err(CtmdError::AmbiguousWhiteBalanceRecords {
                first_record_index,
                second_record_index: record.index(),
            });
        }
        candidate = Some((record.index(), entry.clone()));
    }

    candidate.ok_or(CtmdError::MissingWhiteBalanceRecord)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EosR8SensorLevels {
    pub(crate) black: [u16; 4],
    pub(crate) normal_white: u16,
    pub(crate) specular_white: u16,
}

pub(crate) fn extract_eos_r8_sensor_levels(data: &[u8]) -> Result<EosR8SensorLevels, CtmdError> {
    let (record_index, entry) = find_eos_r8_levels(data)?;
    // Type, count, marker and as-shot ratios identify the exact supported layout.
    decode_eos_r8_white_balance(record_index, &entry)?;
    let values = entry
        .short_values()
        .map_err(|source| CtmdError::InvalidWhiteBalanceValue { record_index, source })?;
    let black = [values[636], values[637], values[638], values[639]];
    let normal_white = values[640];
    let specular_white = values[641];
    if normal_white == 0
        || normal_white > specular_white
        || specular_white > 16383
        || black.iter().any(|value| *value >= normal_white)
    {
        return Err(CtmdError::InvalidSensorLevels { record_index });
    }
    Ok(EosR8SensorLevels {
        black,
        normal_white,
        specular_white,
    })
}

fn decode_eos_r8_white_balance(
    record_index: usize,
    entry: &Entry<'_>,
) -> Result<EosR8AsShotWhiteBalance, CtmdError> {
    if entry.field_type() != FieldType::Short {
        return Err(CtmdError::UnsupportedWhiteBalanceType {
            record_index,
            actual: entry.field_type(),
        });
    }
    if entry.count() != EOS_R8_LEVEL_COUNT {
        return Err(CtmdError::UnsupportedWhiteBalanceCount {
            record_index,
            actual: entry.count(),
            expected: EOS_R8_LEVEL_COUNT,
        });
    }

    let levels = entry
        .short_values()
        .map_err(|source| CtmdError::InvalidWhiteBalanceValue { record_index, source })?;
    let marker = levels
        .get(LAYOUT_MARKER_INDEX)
        .copied()
        .ok_or(CtmdError::UnsupportedWhiteBalanceCount {
            record_index,
            actual: entry.count(),
            expected: EOS_R8_LEVEL_COUNT,
        })?;
    if marker != EOS_R8_LAYOUT_MARKER {
        return Err(CtmdError::UnsupportedWhiteBalanceLayout {
            record_index,
            marker,
            expected: EOS_R8_LAYOUT_MARKER,
        });
    }

    let red_numerator = levels[RED_NUMERATOR_INDEX];
    let red_denominator = levels[RED_DENOMINATOR_INDEX];
    let second_green = levels[SECOND_GREEN_INDEX];
    let blue_numerator = levels[BLUE_NUMERATOR_INDEX];
    if red_denominator == 0 {
        return Err(CtmdError::ZeroWhiteBalanceDenominator {
            record_index,
            channel: WhiteBalanceChannel::Red,
            level_index: RED_DENOMINATOR_INDEX,
        });
    }
    if second_green == 0 {
        return Err(CtmdError::ZeroWhiteBalanceDenominator {
            record_index,
            channel: WhiteBalanceChannel::Blue,
            level_index: SECOND_GREEN_INDEX,
        });
    }

    Ok(EosR8AsShotWhiteBalance {
        red_numerator,
        red_denominator,
        blue_numerator,
        second_green,
    })
}

fn is_classic_tiff_header(data: &[u8]) -> bool {
    if data.len() < TIFF_SIGNATURE_LEN {
        return false;
    }
    matches!(&data[..4], b"II*\0" | b"MM\0*")
}

fn checked_record_end(record_index: usize, offset: usize, declared: usize) -> Result<usize, CtmdError> {
    offset
        .checked_add(declared)
        .ok_or(CtmdError::RecordRangeOverflow {
            record_index,
            offset,
            declared,
        })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WhiteBalanceChannel {
    Red,
    Blue,
}

impl fmt::Display for WhiteBalanceChannel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Red => "red",
            Self::Blue => "blue",
        })
    }
}

/// A malformed CTMD sample or an unsupported/ambiguous WB layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CtmdError {
    TruncatedRecordLength {
        offset: usize,
        remaining: usize,
    },
    RecordLengthDoesNotFit {
        record_index: usize,
        declared: u32,
    },
    RecordTooShort {
        record_index: usize,
        offset: usize,
        declared: usize,
        minimum: usize,
    },
    RecordByteLimitExceeded {
        record_index: usize,
        declared: usize,
        limit: usize,
    },
    RecordCountLimitExceeded {
        count: usize,
        limit: usize,
    },
    RecordRangeOverflow {
        record_index: usize,
        offset: usize,
        declared: usize,
    },
    TruncatedRecord {
        record_index: usize,
        offset: usize,
        declared: usize,
        available: usize,
    },
    RecordAllocationFailed {
        requested: usize,
    },
    InvalidEmbeddedTiff {
        record_index: usize,
        record_offset: usize,
        source: TiffError,
    },
    MissingRootIfd {
        record_index: usize,
        record_offset: usize,
    },
    MissingWhiteBalanceRecord,
    InvalidSensorLevels {
        record_index: usize,
    },
    AmbiguousWhiteBalanceTags {
        record_index: usize,
        occurrences: usize,
    },
    AmbiguousWhiteBalanceRecords {
        first_record_index: usize,
        second_record_index: usize,
    },
    UnsupportedWhiteBalanceType {
        record_index: usize,
        actual: FieldType,
    },
    UnsupportedWhiteBalanceCount {
        record_index: usize,
        actual: u32,
        expected: u32,
    },
    UnsupportedWhiteBalanceLayout {
        record_index: usize,
        marker: u16,
        expected: u16,
    },
    InvalidWhiteBalanceValue {
        record_index: usize,
        source: TiffError,
    },
    ZeroWhiteBalanceDenominator {
        record_index: usize,
        channel: WhiteBalanceChannel,
        level_index: usize,
    },
}

impl fmt::Display for CtmdError {
    #[allow(clippy::too_many_lines)]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSensorLevels { record_index } => write!(
                formatter,
                "CTMD record {record_index} has invalid EOS R8 sensor levels"
            ),
            Self::TruncatedRecordLength { offset, remaining } => write!(
                formatter,
                "CTMD record length at byte {offset} is truncated ({remaining} bytes remain)"
            ),
            Self::RecordLengthDoesNotFit {
                record_index,
                declared,
            } => write!(
                formatter,
                "CTMD record {record_index} length {declared} does not fit this platform"
            ),
            Self::RecordTooShort {
                record_index,
                offset,
                declared,
                minimum,
            } => write!(
                formatter,
                "CTMD record {record_index} at byte {offset} declares {declared} bytes, below minimum {minimum}"
            ),
            Self::RecordByteLimitExceeded {
                record_index,
                declared,
                limit,
            } => write!(
                formatter,
                "CTMD record {record_index} declares {declared} bytes, exceeding limit {limit}"
            ),
            Self::RecordCountLimitExceeded { count, limit } => {
                write!(
                    formatter,
                    "CTMD has at least {count} records, exceeding limit {limit}"
                )
            }
            Self::RecordRangeOverflow {
                record_index,
                offset,
                declared,
            } => write!(
                formatter,
                "CTMD record {record_index} range {offset} + {declared} overflows"
            ),
            Self::TruncatedRecord {
                record_index,
                offset,
                declared,
                available,
            } => write!(
                formatter,
                "CTMD record {record_index} at byte {offset} declares {declared} bytes, but only {available} remain"
            ),
            Self::RecordAllocationFailed { requested } => {
                write!(
                    formatter,
                    "could not allocate storage for {requested} CTMD records"
                )
            }
            Self::InvalidEmbeddedTiff {
                record_index,
                record_offset,
                source,
            } => write!(
                formatter,
                "CTMD record {record_index} at byte {record_offset} has invalid classic TIFF: {source}"
            ),
            Self::MissingRootIfd {
                record_index,
                record_offset,
            } => write!(
                formatter,
                "CTMD record {record_index} at byte {record_offset} has no TIFF root IFD"
            ),
            Self::MissingWhiteBalanceRecord => {
                formatter.write_str("CTMD has no root TIFF tag 0x4001 white-balance record")
            }
            Self::AmbiguousWhiteBalanceTags {
                record_index,
                occurrences,
            } => write!(
                formatter,
                "CTMD record {record_index} has {occurrences} root TIFF tags 0x4001"
            ),
            Self::AmbiguousWhiteBalanceRecords {
                first_record_index,
                second_record_index,
            } => write!(
                formatter,
                "CTMD records {first_record_index} and {second_record_index} both carry root TIFF tag 0x4001"
            ),
            Self::UnsupportedWhiteBalanceType { record_index, actual } => write!(
                formatter,
                "CTMD record {record_index} tag 0x4001 has unsupported TIFF type {actual:?}, expected SHORT"
            ),
            Self::UnsupportedWhiteBalanceCount {
                record_index,
                actual,
                expected,
            } => write!(
                formatter,
                "CTMD record {record_index} tag 0x4001 has {actual} values, expected {expected}"
            ),
            Self::UnsupportedWhiteBalanceLayout {
                record_index,
                marker,
                expected,
            } => write!(
                formatter,
                "CTMD record {record_index} tag 0x4001 layout marker is {marker}, expected supported EOS R8 marker {expected}"
            ),
            Self::InvalidWhiteBalanceValue { record_index, source } => write!(
                formatter,
                "CTMD record {record_index} tag 0x4001 could not be decoded: {source}"
            ),
            Self::ZeroWhiteBalanceDenominator {
                record_index,
                channel,
                level_index,
            } => write!(
                formatter,
                "CTMD record {record_index} has a zero {channel} white-balance denominator at level {level_index}"
            ),
        }
    }
}

impl Error for CtmdError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidEmbeddedTiff { source, .. } | Self::InvalidWhiteBalanceValue { source, .. } => {
                Some(source)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct TestEntry {
        tag: u16,
        field_type: u16,
        count: u32,
        value: Vec<u8>,
    }

    #[test]
    fn parses_records_and_extracts_exact_eos_r8_ratios() {
        let mut ctmd = size_only_record();
        ctmd.extend(tiff_record(&levels_tiff(&eos_r8_levels())));

        let parsed = Ctmd::parse(&ctmd).expect("synthetic CTMD should parse");
        assert_eq!(parsed.records().len(), 2);
        assert_eq!(parsed.records()[0].offset(), 0);
        assert_eq!(parsed.records()[0].len(), 4);
        assert!(parsed.records()[0].classic_tiff_payload().is_none());
        assert_eq!(parsed.records()[1].offset(), 4);
        assert!(parsed.records()[1].classic_tiff_payload().is_some());

        let white_balance = extract_eos_r8_as_shot_white_balance(&ctmd).expect("supported WB should decode");
        assert_eq!(
            white_balance,
            EosR8AsShotWhiteBalance {
                red_numerator: 1_678,
                red_denominator: 1_024,
                blue_numerator: 1_659,
                second_green: 1_024,
            }
        );
        let gains = white_balance.gains();
        assert_eq!(gains[0].to_bits(), (1_678.0_f32 / 1_024.0).to_bits());
        assert_eq!(gains[1].to_bits(), 1.0_f32.to_bits());
        assert_eq!(gains[2].to_bits(), (1_659.0_f32 / 1_024.0).to_bits());
        assert_eq!(gains[3].to_bits(), 1.0_f32.to_bits());
    }

    #[test]
    fn retains_second_green_gain_without_renormalizing_blue() {
        let mut levels = eos_r8_levels();
        levels[RED_DENOMINATOR_INDEX] = 1000;
        levels[SECOND_GREEN_INDEX] = 1200;
        levels[BLUE_NUMERATOR_INDEX] = 1800;
        let gains = extract_eos_r8_as_shot_white_balance(&tiff_record(&levels_tiff(&levels)))
            .unwrap()
            .gains();
        assert_eq!(
            gains.map(f32::to_bits),
            [1.678_f32, 1.0, 1.8, 1.2].map(f32::to_bits)
        );
    }

    #[test]
    fn reads_dynamic_sensor_levels_and_rejects_invalid_ordering() {
        let mut levels = eos_r8_levels();
        levels[636..640].copy_from_slice(&[500, 501, 502, 503]);
        levels[640] = 12000;
        levels[641] = 14500;
        let bytes = tiff_record(&levels_tiff(&levels));
        assert_eq!(
            extract_eos_r8_sensor_levels(&bytes).unwrap(),
            EosR8SensorLevels {
                black: [500, 501, 502, 503],
                normal_white: 12000,
                specular_white: 14500
            }
        );
        for (normal, specular) in [(0, 14000), (15000, 14000), (12000, 17000), (400, 14000)] {
            levels[640] = normal;
            levels[641] = specular;
            assert!(matches!(
                extract_eos_r8_sensor_levels(&tiff_record(&levels_tiff(&levels))),
                Err(CtmdError::InvalidSensorLevels { .. })
            ));
        }
    }

    #[test]
    fn rejects_truncated_and_invalid_record_lengths() {
        assert!(matches!(
            Ctmd::parse(&[1, 2, 3]),
            Err(CtmdError::TruncatedRecordLength {
                offset: 0,
                remaining: 3
            })
        ));
        assert!(matches!(
            Ctmd::parse(&[0, 0, 0, 0]),
            Err(CtmdError::RecordTooShort {
                record_index: 0,
                declared: 0,
                ..
            })
        ));
        assert!(matches!(
            Ctmd::parse(&[8, 0, 0, 0, 1]),
            Err(CtmdError::TruncatedRecord {
                record_index: 0,
                declared: 8,
                available: 5,
                ..
            })
        ));
        assert!(matches!(
            checked_record_end(7, usize::MAX - 1, 4),
            Err(CtmdError::RecordRangeOverflow { record_index: 7, .. })
        ));
    }

    #[test]
    fn enforces_record_count_and_byte_limits() {
        let data = [4, 0, 0, 0, 4, 0, 0, 0];
        assert!(matches!(
            Ctmd::parse_with_limits(
                &data,
                ParseLimits {
                    max_records: 1,
                    max_record_bytes: 4,
                }
            ),
            Err(CtmdError::RecordCountLimitExceeded { count: 2, limit: 1 })
        ));
        assert!(matches!(
            Ctmd::parse_with_limits(
                &data[..4],
                ParseLimits {
                    max_records: 1,
                    max_record_bytes: 3,
                }
            ),
            Err(CtmdError::RecordByteLimitExceeded {
                record_index: 0,
                declared: 4,
                limit: 3,
            })
        ));
    }

    #[test]
    fn rejects_ambiguous_records_and_tags() {
        let record = tiff_record(&levels_tiff(&eos_r8_levels()));
        let mut duplicate_records = record.clone();
        duplicate_records.extend(record);
        assert!(matches!(
            extract_eos_r8_as_shot_white_balance(&duplicate_records),
            Err(CtmdError::AmbiguousWhiteBalanceRecords {
                first_record_index: 0,
                second_record_index: 1,
            })
        ));

        let levels = short_bytes(&eos_r8_levels());
        let duplicate_tags = build_tiff(&[
            TestEntry {
                tag: TAG_CANON_LEVELS,
                field_type: FieldType::Short as u16,
                count: EOS_R8_LEVEL_COUNT,
                value: levels.clone(),
            },
            TestEntry {
                tag: TAG_CANON_LEVELS,
                field_type: FieldType::Short as u16,
                count: EOS_R8_LEVEL_COUNT,
                value: levels,
            },
        ]);
        assert!(matches!(
            extract_eos_r8_as_shot_white_balance(&tiff_record(&duplicate_tags)),
            Err(CtmdError::AmbiguousWhiteBalanceTags {
                record_index: 0,
                occurrences: 2,
            })
        ));
    }

    #[test]
    fn rejects_unsupported_type_count_and_layout_marker() {
        let unsupported_type = build_tiff(&[TestEntry {
            tag: TAG_CANON_LEVELS,
            field_type: FieldType::Byte as u16,
            count: EOS_R8_LEVEL_COUNT,
            value: vec![0; EOS_R8_LEVEL_COUNT as usize],
        }]);
        assert!(matches!(
            extract_eos_r8_as_shot_white_balance(&tiff_record(&unsupported_type)),
            Err(CtmdError::UnsupportedWhiteBalanceType {
                actual: FieldType::Byte,
                ..
            })
        ));

        let mut short_layout = eos_r8_levels();
        short_layout.pop();
        let unsupported_count = build_tiff(&[TestEntry {
            tag: TAG_CANON_LEVELS,
            field_type: FieldType::Short as u16,
            count: EOS_R8_LEVEL_COUNT - 1,
            value: short_bytes(&short_layout),
        }]);
        assert!(matches!(
            extract_eos_r8_as_shot_white_balance(&tiff_record(&unsupported_count)),
            Err(CtmdError::UnsupportedWhiteBalanceCount {
                actual: 3_777,
                expected: EOS_R8_LEVEL_COUNT,
                ..
            })
        ));

        let mut unknown_layout = eos_r8_levels();
        unknown_layout[LAYOUT_MARKER_INDEX] = EOS_R8_LAYOUT_MARKER + 1;
        assert!(matches!(
            extract_eos_r8_as_shot_white_balance(&tiff_record(&levels_tiff(&unknown_layout))),
            Err(CtmdError::UnsupportedWhiteBalanceLayout {
                marker: 49,
                expected: EOS_R8_LAYOUT_MARKER,
                ..
            })
        ));
    }

    #[test]
    fn rejects_zero_denominators() {
        let mut red_zero = eos_r8_levels();
        red_zero[RED_DENOMINATOR_INDEX] = 0;
        assert!(matches!(
            extract_eos_r8_as_shot_white_balance(&tiff_record(&levels_tiff(&red_zero))),
            Err(CtmdError::ZeroWhiteBalanceDenominator {
                channel: WhiteBalanceChannel::Red,
                level_index: RED_DENOMINATOR_INDEX,
                ..
            })
        ));

        let mut blue_zero = eos_r8_levels();
        blue_zero[SECOND_GREEN_INDEX] = 0;
        assert!(matches!(
            extract_eos_r8_as_shot_white_balance(&tiff_record(&levels_tiff(&blue_zero))),
            Err(CtmdError::ZeroWhiteBalanceDenominator {
                channel: WhiteBalanceChannel::Blue,
                level_index: SECOND_GREEN_INDEX,
                ..
            })
        ));
    }

    #[test]
    fn rejects_tiff_signature_with_truncated_header() {
        let record = tiff_record(b"II*\0");
        assert!(matches!(
            extract_eos_r8_as_shot_white_balance(&record),
            Err(CtmdError::InvalidEmbeddedTiff { record_index: 0, .. })
        ));
    }

    #[test]
    fn record_debug_does_not_expose_payload_bytes() {
        let payload = tiff_record(&levels_tiff(&eos_r8_levels()));
        let parsed = Ctmd::parse(&payload).expect("synthetic CTMD should parse");
        let debug = format!("{:?}", parsed.records()[0]);
        assert!(debug.contains("byte_len"));
        assert!(!debug.contains("1678"));
        assert!(!debug.contains("1659"));
    }

    fn size_only_record() -> Vec<u8> {
        u32::try_from(RECORD_LENGTH_LEN)
            .expect("test record length fits u32")
            .to_le_bytes()
            .to_vec()
    }

    fn eos_r8_levels() -> Vec<u16> {
        let mut levels = vec![0; EOS_R8_LEVEL_COUNT as usize];
        levels[LAYOUT_MARKER_INDEX] = EOS_R8_LAYOUT_MARKER;
        levels[RED_NUMERATOR_INDEX] = 1_678;
        levels[RED_DENOMINATOR_INDEX] = 1_024;
        levels[SECOND_GREEN_INDEX] = 1_024;
        levels[BLUE_NUMERATOR_INDEX] = 1_659;
        levels[636..640].copy_from_slice(&[512; 4]);
        levels[640] = 12_735;
        levels[641] = 14_008;
        levels
    }

    fn levels_tiff(levels: &[u16]) -> Vec<u8> {
        build_tiff(&[TestEntry {
            tag: TAG_CANON_LEVELS,
            field_type: FieldType::Short as u16,
            count: u32::try_from(levels.len()).expect("test level count fits u32"),
            value: short_bytes(levels),
        }])
    }

    fn short_bytes(values: &[u16]) -> Vec<u8> {
        values.iter().flat_map(|value| value.to_le_bytes()).collect()
    }

    fn tiff_record(tiff: &[u8]) -> Vec<u8> {
        let record_len = TIFF_RECORD_HEADER_LEN
            .checked_add(tiff.len())
            .expect("test record length should not overflow");
        let mut record = vec![0; TIFF_RECORD_HEADER_LEN];
        record[..4].copy_from_slice(
            &u32::try_from(record_len)
                .expect("test record length should fit u32")
                .to_le_bytes(),
        );
        record.extend_from_slice(tiff);
        record
    }

    fn build_tiff(entries: &[TestEntry]) -> Vec<u8> {
        const IFD_OFFSET: usize = 8;
        const IFD_ENTRY_LEN: usize = 12;

        let ifd_len = 2 + entries.len() * IFD_ENTRY_LEN + 4;
        let mut tiff = vec![0; IFD_OFFSET + ifd_len];
        tiff[..4].copy_from_slice(b"II*\0");
        tiff[4..8].copy_from_slice(
            &u32::try_from(IFD_OFFSET)
                .expect("test IFD offset fits u32")
                .to_le_bytes(),
        );
        tiff[IFD_OFFSET..IFD_OFFSET + 2].copy_from_slice(
            &u16::try_from(entries.len())
                .expect("test entry count fits u16")
                .to_le_bytes(),
        );

        for (index, entry) in entries.iter().enumerate() {
            let offset = IFD_OFFSET + 2 + index * IFD_ENTRY_LEN;
            tiff[offset..offset + 2].copy_from_slice(&entry.tag.to_le_bytes());
            tiff[offset + 2..offset + 4].copy_from_slice(&entry.field_type.to_le_bytes());
            tiff[offset + 4..offset + 8].copy_from_slice(&entry.count.to_le_bytes());
            if entry.value.len() <= 4 {
                tiff[offset + 8..offset + 8 + entry.value.len()].copy_from_slice(&entry.value);
            } else {
                let value_offset = u32::try_from(tiff.len()).expect("test TIFF value offset fits u32");
                tiff[offset + 8..offset + 12].copy_from_slice(&value_offset.to_le_bytes());
                tiff.extend_from_slice(&entry.value);
            }
        }
        tiff
    }
}
