//! Clean-room primitives for Canon CRX lossless entropy streams.
//!
//! Provenance:
//! - CRX container framing and the bytes used by the regression test were
//!   observed in an EOS R8 `.cr3` supplied by the project owner.
//! - Expected samples were obtained from the project's pre-existing decoder
//!   strictly as black-box output.
//! - The predictor family (MED followed by entropy coding of the residual) is
//!   described by Canon patent US10776956B2.
//! - No Rawler, `LibRaw`, `RawSpeed`, or `ExifTool` decoder source was consulted.
//!
//! The implemented profile is deliberately narrow: `enc_type = 0`,
//! `levels = 0`, one-subband, 14-bit EOS R8 streams. The complete four-plane
//! path is fixture-gated against two owner-supplied CR3 files and their
//! pre-existing black-box pixel oracles.

use std::{error::Error, fmt, mem::MaybeUninit};

/// Size of the compact bootstrap representation used only by the historical
/// `decode_first_row` probe. The production `decode_plane` consumes the real
/// bitstream from bit zero and does not skip these bytes.
pub const LOSSLESS_PLANE_PREFIX_LEN: usize = 8;

/// Initial Rice parameter observed for a first row.
pub const INITIAL_RICE_PARAMETER: u8 = 2;

const CONFIRMED_BIT_DEPTH: u8 = 14;
const FIRST_SAMPLE_MARKER: u32 = 0x80;
const MAX_RICE_PARAMETER: u8 = 30;
const MAX_CONFIRMED_MAPPED_RESIDUAL: u32 = 2 * ((1 << CONFIRMED_BIT_DEPTH) - 1);
const MAX_FULL_UNARY_ZEROES: u32 = 4_096;
const FULL_ESCAPE_PREFIX: u32 = 41;
const FULL_ESCAPE_BITS: u8 = 21;
const FULL_MAX_RICE_PARAMETER: u8 = 15;
const MAX_PLANE_SAMPLES: usize = 64 * 1024 * 1024;
const MAX_PLANE_WIDTH: usize = 64 * 1024;
const RUN_INDEX_TABLE: [u8; 32] = [
    0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 9, 10, 11, 12, 13, 14, 15,
];

/// A bounded entropy decoding failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LosslessError {
    EmptyRow,
    EmptyPlane,
    UnsupportedBitDepth {
        bit_depth: u8,
    },
    TruncatedPlanePrefix {
        needed: usize,
        available: usize,
    },
    InvalidReservedPrefix {
        actual: [u8; 4],
    },
    InvalidFirstSampleMarker {
        expected: u32,
        actual: u32,
    },
    UnexpectedEndOfStream {
        bit_position: usize,
        requested: u8,
        remaining: usize,
    },
    UnaryRunTooLong {
        bit_position: usize,
        limit: u32,
    },
    MappedSymbolOutOfRange {
        value: u32,
        maximum: u32,
    },
    RiceParameterTooLarge {
        parameter: u8,
    },
    ArithmeticOverflow {
        context: &'static str,
    },
    ImpossibleRowWidth {
        width: usize,
        maximum: usize,
    },
    PlaneSizeOverflow {
        width: usize,
        height: usize,
    },
    PlaneSampleLimit {
        samples: usize,
        limit: usize,
    },
    AllocationFailed {
        samples: usize,
    },
    Cancelled {
        row: usize,
    },
    RunLengthOutOfRange {
        run: usize,
        remaining: usize,
    },
    CoefficientOverflow {
        row: usize,
        column: usize,
    },
    SampleOutOfRange {
        index: usize,
        value: i32,
        maximum: u32,
    },
}

impl fmt::Display for LosslessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRow => formatter.write_str("CRX lossless row width is zero"),
            Self::EmptyPlane => formatter.write_str("CRX lossless plane geometry is empty"),
            Self::UnsupportedBitDepth { bit_depth } => write!(
                formatter,
                "CRX lossless first-row framing is confirmed only for 14-bit data, got {bit_depth}"
            ),
            Self::TruncatedPlanePrefix { needed, available } => write!(
                formatter,
                "CRX lossless plane prefix is truncated: need {needed} bytes, have {available}"
            ),
            Self::InvalidReservedPrefix { actual } => {
                write!(
                    formatter,
                    "CRX lossless reserved prefix is not zero: {actual:02x?}"
                )
            }
            Self::InvalidFirstSampleMarker { expected, actual } => write!(
                formatter,
                "CRX lossless first-sample marker is 0x{actual:x}, expected 0x{expected:x}"
            ),
            Self::UnexpectedEndOfStream {
                bit_position,
                requested,
                remaining,
            } => write!(
                formatter,
                "CRX lossless stream ended at bit {bit_position}: requested {requested} bits, {remaining} remain"
            ),
            Self::UnaryRunTooLong { bit_position, limit } => write!(
                formatter,
                "CRX lossless unary code at bit {bit_position} exceeds the {limit}-zero safety limit"
            ),
            Self::MappedSymbolOutOfRange { value, maximum } => write!(
                formatter,
                "CRX lossless mapped residual {value} exceeds the confirmed maximum {maximum}"
            ),
            Self::RiceParameterTooLarge { parameter } => write!(
                formatter,
                "CRX lossless Rice parameter {parameter} exceeds the supported limit"
            ),
            Self::ArithmeticOverflow { context } => {
                write!(
                    formatter,
                    "CRX lossless arithmetic overflow while computing {context}"
                )
            }
            Self::ImpossibleRowWidth { width, maximum } => write!(
                formatter,
                "CRX lossless row width {width} cannot fit in the stream (maximum {maximum})"
            ),
            Self::PlaneSizeOverflow { width, height } => write!(
                formatter,
                "CRX lossless plane geometry {width}x{height} overflows this platform"
            ),
            Self::PlaneSampleLimit { samples, limit } => write!(
                formatter,
                "CRX lossless plane has {samples} samples, above the {limit}-sample safety limit"
            ),
            Self::AllocationFailed { samples } => write!(
                formatter,
                "could not allocate storage for {samples} CRX lossless samples"
            ),
            Self::Cancelled { row } => {
                write!(
                    formatter,
                    "CRX lossless plane decode was cancelled before row {row}"
                )
            }
            Self::RunLengthOutOfRange { run, remaining } => write!(
                formatter,
                "CRX lossless run length {run} exceeds the {remaining} remaining samples"
            ),
            Self::CoefficientOverflow { row, column } => write!(
                formatter,
                "CRX predictor arithmetic overflow at row {row}, column {column}"
            ),
            Self::SampleOutOfRange {
                index,
                value,
                maximum,
            } => write!(
                formatter,
                "CRX lossless sample {index} is {value}, outside 0..={maximum}"
            ),
        }
    }
}

impl Error for LosslessError {}

/// Bounded, MSB-first reader used by the confirmed CRX entropy path.
///
/// The absolute bit position and the remaining bit count are derived lazily
/// from the refill cursor (`next_byte`) and the reservoir fill level instead
/// of being maintained on every consumed bit. They are only needed for error
/// reporting and for the one-shot `bits_consumed` summary, so per-bit
/// bookkeeping updates just the reservoir.
#[derive(Debug, Clone)]
pub struct MsbBitReader<'a> {
    bytes: &'a [u8],
    next_byte: usize,
    reservoir: u64,
    reservoir_bits: u8,
}

impl<'a> MsbBitReader<'a> {
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            next_byte: 0,
            reservoir: 0,
            reservoir_bits: 0,
        }
    }

    pub const fn bit_position(&self) -> usize {
        self.next_byte
            .saturating_mul(8)
            .saturating_sub(self.reservoir_bits as usize)
    }

    pub const fn bits_remaining(&self) -> usize {
        self.bytes
            .len()
            .saturating_sub(self.next_byte)
            .saturating_mul(8)
            .saturating_add(self.reservoir_bits as usize)
    }

    #[inline]
    pub fn read_bit(&mut self) -> Result<u8, LosslessError> {
        if self.reservoir_bits == 0 {
            self.refill();
            if self.reservoir_bits == 0 {
                return Err(LosslessError::UnexpectedEndOfStream {
                    bit_position: self.bit_position(),
                    requested: 1,
                    remaining: 0,
                });
            }
        }

        let bit = (self.reservoir >> 63) as u8;
        self.consume_buffered(1);
        Ok(bit)
    }

    pub fn read_bits(&mut self, count: u8) -> Result<u32, LosslessError> {
        if count > 32 {
            return Err(LosslessError::ArithmeticOverflow {
                context: "bit-read width",
            });
        }
        if count == 0 {
            return Ok(0);
        }

        // Fast path: the request is fully covered by the reservoir. Buffered
        // bits are a subset of the unread input (`reservoir_bits` is counted
        // in `bits_remaining`), so no end-of-stream check is needed here.
        if count <= self.reservoir_bits {
            // `1 <= count <= 32`, so the shift is `32..=63`.
            #[allow(clippy::cast_possible_truncation)]
            let value = (self.reservoir >> (64 - u32::from(count))) as u32;
            self.consume_buffered(count);
            return Ok(value);
        }

        let remaining = self.bits_remaining();
        if remaining < usize::from(count) {
            return Err(LosslessError::UnexpectedEndOfStream {
                bit_position: self.bit_position(),
                requested: count,
                remaining,
            });
        }

        let mut value = 0_u64;
        let mut needed = count;
        while needed > 0 {
            if self.reservoir_bits == 0 {
                self.refill();
            }
            debug_assert!(self.reservoir_bits > 0);

            let take = needed.min(self.reservoir_bits);
            let part = self.reservoir >> (64 - u32::from(take));
            value = (value << take) | part;
            self.consume_buffered(take);
            needed -= take;
        }
        // `count <= 32`, so the accumulator cannot contain more than 32 bits.
        #[allow(clippy::cast_possible_truncation)]
        let value = value as u32;
        Ok(value)
    }

    fn read_zero_unary(&mut self, limit: u32) -> Result<u32, LosslessError> {
        let mut zeroes = 0_u32;
        loop {
            if self.reservoir_bits == 0 {
                self.refill();
                if self.reservoir_bits == 0 {
                    return Err(LosslessError::UnexpectedEndOfStream {
                        bit_position: self.bit_position(),
                        requested: 1,
                        remaining: 0,
                    });
                }
            }

            let available = u32::from(self.reservoir_bits);
            let leading_zeroes = self.reservoir.leading_zeros().min(available);
            let allowed = limit - zeroes;
            if leading_zeroes > allowed {
                // `leading_zeroes <= reservoir_bits <= 64`, so this branch
                // proves `allowed + 1 <= 64`.
                #[allow(clippy::cast_possible_truncation)]
                let consumed = (allowed + 1) as u8;
                // The original `start` snapshot is the position at function
                // entry; exactly `zeroes` bits have been consumed since.
                let bit_position = self.bit_position().saturating_sub(zeroes as usize);
                self.consume_buffered(consumed);
                return Err(LosslessError::UnaryRunTooLong { bit_position, limit });
            }

            if leading_zeroes < available {
                // `leading_zeroes < reservoir_bits <= 64`.
                #[allow(clippy::cast_possible_truncation)]
                let consumed = (leading_zeroes + 1) as u8;
                self.consume_buffered(consumed);
                return Ok(zeroes + leading_zeroes);
            }

            self.consume_buffered(self.reservoir_bits);
            zeroes += leading_zeroes;
        }
    }

    fn refill(&mut self) {
        debug_assert_eq!(self.reservoir_bits, 0);
        let Some(remaining) = self.bytes.get(self.next_byte..) else {
            return;
        };
        if remaining.len() >= std::mem::size_of::<u64>() {
            let word: [u8; std::mem::size_of::<u64>()] = remaining[..std::mem::size_of::<u64>()]
                .try_into()
                .expect("the full-reservoir branch has eight bytes");
            self.reservoir = u64::from_be_bytes(word);
            self.reservoir_bits = 64;
            self.next_byte += std::mem::size_of::<u64>();
            return;
        }
        if remaining.is_empty() {
            return;
        }

        let mut word = [0_u8; std::mem::size_of::<u64>()];
        word[..remaining.len()].copy_from_slice(remaining);
        self.reservoir = u64::from_be_bytes(word);
        self.reservoir_bits =
            u8::try_from(remaining.len() * 8).expect("a short u64 tail has at most 56 bits");
        self.next_byte += remaining.len();
    }

    #[inline]
    fn consume_buffered(&mut self, count: u8) {
        debug_assert!(count <= self.reservoir_bits);
        if count == 64 {
            self.reservoir = 0;
        } else {
            self.reservoir <<= count;
        }
        self.reservoir_bits -= count;
    }
}

/// State of the confirmed adaptive Rice code used across the first row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdaptiveRice {
    parameter: u8,
}

impl AdaptiveRice {
    pub const fn new(parameter: u8) -> Self {
        Self { parameter }
    }

    pub const fn parameter(self) -> u8 {
        self.parameter
    }

    pub fn decode(&mut self, reader: &mut MsbBitReader<'_>) -> Result<u32, LosslessError> {
        if self.parameter > MAX_RICE_PARAMETER {
            return Err(LosslessError::RiceParameterTooLarge {
                parameter: self.parameter,
            });
        }

        let maximum_quotient = MAX_CONFIRMED_MAPPED_RESIDUAL >> self.parameter;
        let quotient = reader.read_zero_unary(maximum_quotient)?;
        let remainder = reader.read_bits(self.parameter)?;
        let mapped = quotient
            .checked_shl(u32::from(self.parameter))
            .and_then(|high| high.checked_add(remainder))
            .ok_or(LosslessError::ArithmeticOverflow {
                context: "Rice symbol",
            })?;
        if mapped > MAX_CONFIRMED_MAPPED_RESIDUAL {
            return Err(LosslessError::MappedSymbolOutOfRange {
                value: mapped,
                maximum: MAX_CONFIRMED_MAPPED_RESIDUAL,
            });
        }

        self.adapt(mapped);
        Ok(mapped)
    }

    fn adapt(&mut self, mapped: u32) {
        let old_parameter = self.parameter;
        let quotient = mapped >> old_parameter;
        let mut next = old_parameter + u8::from(quotient > 2) + u8::from(quotient > 5);
        if old_parameter > 0 {
            let lower_threshold = 1_u32 << (old_parameter - 1);
            if mapped < lower_threshold {
                next -= 1;
            }
        }
        self.parameter = next.min(MAX_RICE_PARAMETER);
    }
}

/// Result of decoding the independently confirmed first-row path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstRow {
    pub samples: Vec<u16>,
    /// Bits consumed from the beginning of the complete plane chunk, including
    /// the eight-byte first-sample prefix.
    pub bits_consumed: usize,
    pub final_rice_parameter: u8,
}

/// Historical narrow probe retained for its compact first-64 fixture vector.
///
/// Production callers must use `decode_plane`, whose bootstrap, run and row
/// contexts model the complete observed EOS R8 stream.
pub fn decode_first_row(plane_chunk: &[u8], width: usize, bit_depth: u8) -> Result<FirstRow, LosslessError> {
    if width == 0 {
        return Err(LosslessError::EmptyRow);
    }
    if bit_depth != CONFIRMED_BIT_DEPTH {
        return Err(LosslessError::UnsupportedBitDepth { bit_depth });
    }
    if plane_chunk.len() < LOSSLESS_PLANE_PREFIX_LEN {
        return Err(LosslessError::TruncatedPlanePrefix {
            needed: LOSSLESS_PLANE_PREFIX_LEN,
            available: plane_chunk.len(),
        });
    }
    let maximum_width = plane_chunk[LOSSLESS_PLANE_PREFIX_LEN..]
        .len()
        .saturating_mul(8)
        .saturating_add(1);
    if width > maximum_width {
        return Err(LosslessError::ImpossibleRowWidth {
            width,
            maximum: maximum_width,
        });
    }

    let reserved = [plane_chunk[0], plane_chunk[1], plane_chunk[2], plane_chunk[3]];
    if reserved != [0; 4] {
        return Err(LosslessError::InvalidReservedPrefix { actual: reserved });
    }

    let first_word = u32::from_be_bytes([plane_chunk[4], plane_chunk[5], plane_chunk[6], plane_chunk[7]]);
    let marker = first_word >> bit_depth;
    if marker != FIRST_SAMPLE_MARKER {
        return Err(LosslessError::InvalidFirstSampleMarker {
            expected: FIRST_SAMPLE_MARKER,
            actual: marker,
        });
    }

    let sample_mask = (1_u32 << bit_depth) - 1;
    let stored = first_word & sample_mask;
    let first_mapped = (!stored) & sample_mask;
    let first = unmap_signed(first_mapped);
    let first = checked_sample(first, 0, sample_mask)?;

    let mut samples = Vec::new();
    samples
        .try_reserve_exact(width)
        .map_err(|_| LosslessError::ArithmeticOverflow {
            context: "first-row sample allocation",
        })?;
    samples.push(first);

    let mut reader = MsbBitReader::new(&plane_chunk[LOSSLESS_PLANE_PREFIX_LEN..]);
    let mut rice = AdaptiveRice::new(INITIAL_RICE_PARAMETER);
    while samples.len() < width {
        let mapped = rice.decode(&mut reader)?;
        let residual = unmap_signed(mapped);
        let predictor = i32::from(*samples.last().ok_or(LosslessError::EmptyRow)?);
        let value = predictor
            .checked_add(residual)
            .ok_or(LosslessError::ArithmeticOverflow {
                context: "first-row prediction",
            })?;
        samples.push(checked_sample(value, samples.len(), sample_mask)?);
    }

    Ok(FirstRow {
        samples,
        bits_consumed: LOSSLESS_PLANE_PREFIX_LEN * 8 + reader.bit_position(),
        final_rice_parameter: rice.parameter(),
    })
}

#[allow(clippy::cast_possible_wrap)]
fn unmap_signed(mapped: u32) -> i32 {
    ((mapped >> 1) as i32) ^ -((mapped & 1) as i32)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn checked_sample(value: i32, index: usize, maximum: u32) -> Result<u16, LosslessError> {
    let unsigned = value as u32;
    if unsigned > maximum {
        return Err(LosslessError::SampleOutOfRange {
            index,
            value,
            maximum,
        });
    }
    Ok(unsigned as u16)
}

#[derive(Debug)]
struct FullEntropy<'a> {
    reader: MsbBitReader<'a>,
    rice_parameter: u8,
    run_index: u8,
}

impl<'a> FullEntropy<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self {
            reader: MsbBitReader::new(bytes),
            rice_parameter: 0,
            run_index: 0,
        }
    }

    #[inline]
    fn decode_rice(&mut self, adapt: bool) -> Result<u32, LosslessError> {
        let old_parameter = self.rice_parameter;

        // Fused fast path: the unary quotient, its terminating one and the
        // `old_parameter` remainder bits are all present in the reservoir, so
        // the symbol decodes with one leading-zero count and one consume
        // instead of two separate reader calls. The escape prefix and any
        // reservoir-underflow case fall back to the general path below.
        let prefix = self.reader.reservoir.leading_zeros();
        let symbol_bits = prefix + 1 + u32::from(old_parameter);
        if prefix < FULL_ESCAPE_PREFIX && symbol_bits <= u32::from(self.reader.reservoir_bits) {
            // `prefix <= 40` and `old_parameter <= FULL_MAX_RICE_PARAMETER`,
            // so `symbol_bits <= 56` and every shift below is in range.
            let value = if old_parameter == 0 {
                prefix
            } else {
                #[allow(clippy::cast_possible_truncation)]
                let remainder =
                    ((self.reader.reservoir << (prefix + 1)) >> (64 - u32::from(old_parameter))) as u32;
                (prefix << old_parameter) + remainder
            };
            #[allow(clippy::cast_possible_truncation)]
            self.reader.consume_buffered(symbol_bits as u8);
            if adapt {
                self.update_rice_parameter(value);
            }
            return Ok(value);
        }

        let prefix = self.reader.read_zero_unary(MAX_FULL_UNARY_ZEROES)?;
        let value = if prefix >= FULL_ESCAPE_PREFIX {
            self.reader.read_bits(FULL_ESCAPE_BITS)?
        } else {
            let remainder = self.reader.read_bits(old_parameter)?;
            (prefix << old_parameter) + remainder
        };
        if adapt {
            self.update_rice_parameter(value);
        }
        Ok(value)
    }

    fn update_rice_parameter(&mut self, value: u32) {
        let old_parameter = self.rice_parameter;
        let quotient = value >> old_parameter;
        let mut next = old_parameter + u8::from(quotient > 2) + u8::from(quotient > 5);
        if old_parameter > 0 && value < (1_u32 << (old_parameter - 1)) {
            next -= 1;
        }
        self.rice_parameter = next.min(FULL_MAX_RICE_PARAMETER);
    }

    fn decode_run(&mut self, remaining: usize) -> Result<usize, LosslessError> {
        let mut run = 1usize;
        while run != remaining && self.reader.read_bit()? == 1 {
            let shift = RUN_INDEX_TABLE[usize::from(self.run_index)];
            let increment = 1usize << shift;
            run += increment;
            if run > remaining {
                run = remaining;
                break;
            }
            self.run_index = self.run_index.saturating_add(1).min(31);
        }
        if run < remaining {
            let width = RUN_INDEX_TABLE[usize::from(self.run_index)];
            let tail = usize::try_from(self.reader.read_bits(width)?).map_err(|_| {
                LosslessError::ArithmeticOverflow {
                    context: "CRX run tail",
                }
            })?;
            run += tail;
            self.run_index = self.run_index.saturating_sub(1);
        }
        if run > remaining {
            return Err(LosslessError::RunLengthOutOfRange { run, remaining });
        }
        Ok(run)
    }
}

/// Decodes one complete lossless EOS R8 CRX parity plane.
///
/// The callback is polled before every row. The four-plane scheduler also
/// combines it with its internal stop flag, so an error in one plane asks the
/// other three workers to finish no later than their current row boundary.
pub(crate) fn decode_plane(
    plane_chunk: &[u8],
    width: usize,
    height: usize,
    bit_depth: u8,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u16>, LosslessError> {
    let sample_count = checked_plane_sample_count(width, height, bit_depth)?;
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(sample_count)
        .map_err(|_| LosslessError::AllocationFailed {
            samples: sample_count,
        })?;
    decode_plane_rows(
        plane_chunk,
        width,
        height,
        bit_depth,
        cancelled,
        &mut |_, mut row_samples| {
            samples.extend_from_slice(&row_samples);
            row_samples.clear();
            Some(row_samples)
        },
    )?;
    debug_assert_eq!(samples.len(), sample_count);
    Ok(samples)
}

/// Decodes a plane one row at a time while recycling the row output buffer.
///
/// The sink takes ownership of each complete row and must return an empty
/// buffer for the next row. This supports bounded streaming assembly without
/// weakening the checked entropy and sample-range contract.
pub(crate) fn decode_plane_rows(
    plane_chunk: &[u8],
    width: usize,
    height: usize,
    bit_depth: u8,
    cancelled: &dyn Fn() -> bool,
    emit_row: &mut dyn FnMut(usize, Vec<u16>) -> Option<Vec<u16>>,
) -> Result<(), LosslessError> {
    checked_plane_sample_count(width, height, bit_depth)?;
    let row_storage = width
        .checked_add(2)
        .ok_or(LosslessError::PlaneSizeOverflow { width, height })?;
    let mut previous = allocate_coefficients(row_storage)?;
    let mut current = allocate_coefficients(row_storage)?;
    let mut row_samples = Vec::new();
    row_samples
        .try_reserve_exact(width)
        .map_err(|_| LosslessError::AllocationFailed { samples: width })?;
    let mut entropy = FullEntropy::new(plane_chunk);
    let midpoint = 1_i32 << (bit_depth - 1);
    let maximum = (1_u32 << bit_depth) - 1;

    for row in 0..height {
        if cancelled() {
            return Err(LosslessError::Cancelled { row });
        }
        row_samples.clear();
        let output_spec = RowOutputSpec {
            row_start: row * width,
            midpoint,
            maximum,
        };
        if row == 0 {
            decode_top_row(&mut entropy, &mut current, &mut row_samples, width, output_spec)?;
        } else {
            std::mem::swap(&mut previous, &mut current);
            decode_context_row(
                &mut entropy,
                &previous,
                &mut current,
                &mut row_samples,
                width,
                output_spec,
            )?;
        }
        debug_assert_eq!(row_samples.len(), width);
        row_samples = emit_row(row, row_samples).ok_or(LosslessError::Cancelled { row })?;
        row_samples.clear();
        if row_samples.capacity() < width {
            row_samples
                .try_reserve_exact(width)
                .map_err(|_| LosslessError::AllocationFailed { samples: width })?;
        }
    }
    Ok(())
}

fn checked_plane_sample_count(width: usize, height: usize, bit_depth: u8) -> Result<usize, LosslessError> {
    if width == 0 || height == 0 {
        return Err(LosslessError::EmptyPlane);
    }
    if bit_depth != CONFIRMED_BIT_DEPTH {
        return Err(LosslessError::UnsupportedBitDepth { bit_depth });
    }
    if width > MAX_PLANE_WIDTH {
        return Err(LosslessError::ImpossibleRowWidth {
            width,
            maximum: MAX_PLANE_WIDTH,
        });
    }
    let sample_count = width
        .checked_mul(height)
        .ok_or(LosslessError::PlaneSizeOverflow { width, height })?;
    if sample_count > MAX_PLANE_SAMPLES {
        return Err(LosslessError::PlaneSampleLimit {
            samples: sample_count,
            limit: MAX_PLANE_SAMPLES,
        });
    }
    Ok(sample_count)
}

fn allocate_coefficients(samples: usize) -> Result<Vec<i32>, LosslessError> {
    let mut coefficients = Vec::new();
    coefficients
        .try_reserve_exact(samples)
        .map_err(|_| LosslessError::AllocationFailed { samples })?;
    coefficients.resize(samples, 0);
    Ok(coefficients)
}

#[derive(Clone, Copy)]
struct RowOutputSpec {
    row_start: usize,
    midpoint: i32,
    maximum: u32,
}

#[allow(unsafe_code)] // justified per-access; keeps the workspace deny intact elsewhere
fn decode_top_row(
    entropy: &mut FullEntropy<'_>,
    current: &mut [i32],
    output: &mut Vec<u16>,
    width: usize,
    output_spec: RowOutputSpec,
) -> Result<(), LosslessError> {
    // Row-buffer invariants relied on by the `get_unchecked` uses below:
    // `current` has exactly `width + 2` slots (see `decode_plane_rows`), the
    // loop keeps `1 <= column <= width` with `column - 1 + remaining == width`
    // and clamps every run to the remaining sample count, so
    // `column - 1 ..= column + run` never leave the row. `output` is empty
    // with capacity for `width` samples; samples are written into its spare
    // capacity and committed with `set_len` only after the whole row was
    // produced, so an error return leaves the vector empty. The debug
    // assertions double-check these invariants.
    debug_assert_eq!(current.len(), width + 2);
    debug_assert_eq!(output.len(), 0);
    debug_assert!(output.capacity() >= width);
    let row_buffer = &mut output.spare_capacity_mut()[..width];
    let mut emitted = 0usize;

    let mut remaining = width;
    let mut column = 1usize;
    while remaining > 1 {
        // SAFETY: `1 <= column <= width`, so `column - 1` and `column` index
        // the `width + 2` row slots.
        let predictor = unsafe {
            let left = *current.get_unchecked(column - 1);
            *current.get_unchecked_mut(column) = left;
            left
        };
        if predictor == 0 && entropy.reader.read_bit()? == 1 {
            let run = entropy.decode_run(remaining)?;
            // SAFETY: `run <= remaining` and `column - 1 + remaining == width`
            // give `column + run <= width + 1`, inside the `width + 2` slots.
            unsafe {
                current.get_unchecked_mut(column..column + run).fill(predictor);
            }
            let sample = checked_sample(
                predictor + output_spec.midpoint,
                output_spec.row_start + column - 1,
                output_spec.maximum,
            )?;
            emit_run(row_buffer, &mut emitted, run, sample);
            column += run;
            remaining -= run;
            if remaining == 0 {
                break;
            }
            // SAFETY: `remaining >= 1` keeps `column <= width`.
            unsafe {
                *current.get_unchecked_mut(column) = 0;
            }
        }
        let residual = unmap_signed(entropy.decode_rice(true)?);
        // SAFETY: `column <= width`.
        let predictor = unsafe { *current.get_unchecked(column) };
        emit_coefficient(
            current,
            row_buffer,
            &mut emitted,
            column,
            predictor,
            residual,
            output_spec,
        )?;
        column += 1;
        remaining -= 1;
    }
    if remaining == 1 {
        let residual = unmap_signed(entropy.decode_rice(true)?);
        // SAFETY: `column == width` here, so `column - 1` is in bounds.
        let predictor = unsafe { *current.get_unchecked(column - 1) };
        emit_coefficient(
            current,
            row_buffer,
            &mut emitted,
            column,
            predictor,
            residual,
            output_spec,
        )?;
        column += 1;
    }
    // SAFETY: `column == width + 1` now, the last of the `width + 2` slots.
    unsafe {
        *current.get_unchecked_mut(column) = *current.get_unchecked(column - 1) + 1;
    }
    debug_assert_eq!(emitted, width);
    // SAFETY: `emitted == width` samples were written into `row_buffer`, the
    // spare capacity of `output`, and `u16` has no drop glue.
    unsafe {
        output.set_len(width);
    }
    Ok(())
}

#[allow(unsafe_code)] // justified per-access; keeps the workspace deny intact elsewhere
fn decode_context_row(
    entropy: &mut FullEntropy<'_>,
    previous: &[i32],
    current: &mut [i32],
    output: &mut Vec<u16>,
    width: usize,
    output_spec: RowOutputSpec,
) -> Result<(), LosslessError> {
    // Same row-buffer invariants as `decode_top_row`.
    debug_assert_eq!(previous.len(), width + 2);
    debug_assert_eq!(current.len(), width + 2);
    debug_assert_eq!(output.len(), 0);
    debug_assert!(output.capacity() >= width);
    let row_buffer = &mut output.spare_capacity_mut()[..width];
    let mut emitted = 0usize;

    // SAFETY: `width >= 1` (enforced by `checked_plane_sample_count`), so
    // indices 0 and 1 are inside the `width + 2` row slots.
    unsafe {
        *current.get_unchecked_mut(0) = *previous.get_unchecked(1);
    }
    let mut remaining = width;
    let mut column = 1usize;

    while remaining > 1 {
        // SAFETY: `1 <= column <= width`, so `column - 1 ..= column + 1`
        // index the `width + 2` row slots.
        let (left, above, upper_left, above_right) = unsafe {
            (
                *current.get_unchecked(column - 1),
                *previous.get_unchecked(column),
                *previous.get_unchecked(column - 1),
                *previous.get_unchecked(column + 1),
            )
        };

        let predictor = if left == above && left == above_right {
            if entropy.reader.read_bit()? == 1 {
                let run = entropy.decode_run(remaining)?;
                // SAFETY: `run <= remaining` and
                // `column - 1 + remaining == width` give
                // `column + run <= width + 1`, inside the `width + 2` slots.
                unsafe {
                    current.get_unchecked_mut(column..column + run).fill(left);
                }
                let sample = checked_sample(
                    left + output_spec.midpoint,
                    output_spec.row_start + column - 1,
                    output_spec.maximum,
                )?;
                emit_run(row_buffer, &mut emitted, run, sample);
                column += run;
                remaining -= run;
                if remaining == 0 {
                    break;
                }
            }
            // SAFETY: `column <= width` after the clamped run.
            unsafe { *previous.get_unchecked(column) }
        } else {
            median_predictor(left, above, upper_left)
        };

        let mapped = entropy.decode_rice(false)?;
        let residual = unmap_signed(mapped);
        emit_coefficient(
            current,
            row_buffer,
            &mut emitted,
            column,
            predictor,
            residual,
            output_spec,
        )?;
        let adapted = if remaining > 1 {
            // SAFETY: `column <= width`, so `column + 1 <= width + 1`.
            unsafe {
                adjusted_context_symbol(
                    mapped,
                    *previous.get_unchecked(column + 1),
                    *previous.get_unchecked(column),
                )
            }
        } else {
            mapped
        };
        entropy.update_rice_parameter(adapted);
        column += 1;
        remaining -= 1;
    }

    if remaining == 1 {
        // SAFETY: `column == width`, so `column - 1` and `column` are in bounds.
        let predictor = unsafe {
            median_predictor(
                *current.get_unchecked(column - 1),
                *previous.get_unchecked(column),
                *previous.get_unchecked(column - 1),
            )
        };
        let residual = unmap_signed(entropy.decode_rice(true)?);
        emit_coefficient(
            current,
            row_buffer,
            &mut emitted,
            column,
            predictor,
            residual,
            output_spec,
        )?;
        column += 1;
    }
    // SAFETY: `column == width + 1`, the last of the `width + 2` slots.
    unsafe {
        *current.get_unchecked_mut(column) = *current.get_unchecked(column - 1) + 1;
    }
    debug_assert_eq!(emitted, width);
    // SAFETY: `emitted == width` samples were written into `row_buffer`, the
    // spare capacity of `output`, and `u16` has no drop glue.
    unsafe {
        output.set_len(width);
    }
    Ok(())
}

fn median_predictor(left: i32, above: i32, upper_left: i32) -> i32 {
    left + above - upper_left.clamp(left.min(above), left.max(above))
}

fn adjusted_context_symbol(mapped: u32, above_right: i32, above: i32) -> u32 {
    let delta = (above_right - above).unsigned_abs() * 2;
    (mapped + delta) >> 1
}

/// Writes `run` copies of an already range-checked run sample into the spare
/// capacity of the row output.
#[allow(unsafe_code)] // justified by the run-length invariant, see below
fn emit_run(row_buffer: &mut [MaybeUninit<u16>], emitted: &mut usize, run: usize, sample: u16) {
    // SAFETY: `run` was decoded against the remaining sample count of the
    // row, so `*emitted + run <= width == row_buffer.len()`.
    unsafe {
        row_buffer
            .get_unchecked_mut(*emitted..*emitted + run)
            .fill(MaybeUninit::new(sample));
    }
    *emitted += run;
}

/// Reconstructs one coefficient, range-checks it with a single unsigned
/// comparison (negative values wrap above `maximum`) and writes the sample
/// into the spare capacity of the row output.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[allow(unsafe_code)] // justified by the row-buffer invariants, see below
#[inline]
fn emit_coefficient(
    current: &mut [i32],
    row_buffer: &mut [MaybeUninit<u16>],
    emitted: &mut usize,
    column: usize,
    predictor: i32,
    residual: i32,
    output_spec: RowOutputSpec,
) -> Result<(), LosslessError> {
    let coefficient = predictor + residual;
    let value = coefficient + output_spec.midpoint;
    let unsigned = value as u32;
    if unsigned > output_spec.maximum {
        return Err(LosslessError::SampleOutOfRange {
            index: output_spec.row_start + column - 1,
            value,
            maximum: output_spec.maximum,
        });
    }
    // SAFETY: the row loops emit at most one sample per remaining sample, so
    // `*emitted < width == row_buffer.len()`, and `column <= width` keeps the
    // `width + 2` coefficient row in bounds (see the row decoders).
    unsafe {
        row_buffer.get_unchecked_mut(*emitted).write(unsigned as u16);
        *current.get_unchecked_mut(column) = coefficient;
    }
    *emitted += 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msb_reader_is_bounded_and_crosses_bytes() {
        let mut reader = MsbBitReader::new(&[0b1010_1100, 0b0111_0001]);
        assert_eq!(reader.read_bits(3), Ok(0b101));
        assert_eq!(reader.read_bits(7), Ok(0b011_0001));
        assert_eq!(reader.read_bits(6), Ok(0b11_0001));
        assert_eq!(reader.bit_position(), 16);
        assert!(matches!(
            reader.read_bit(),
            Err(LosslessError::UnexpectedEndOfStream {
                bit_position: 16,
                requested: 1,
                remaining: 0
            })
        ));
    }

    #[test]
    fn msb_reader_fixed_width_reads_cross_reservoir_boundaries() {
        let encoded = [
            0xde, 0xad, 0xbe, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
        ];
        let mut reader = MsbBitReader::new(&encoded);

        assert_eq!(reader.read_bits(4), Ok(0xd));
        assert_eq!(reader.read_bits(32), Ok(0xeadb_eef0));
        assert_eq!(reader.read_bits(28), Ok(0x0123_4567));
        assert_eq!(reader.read_bits(32), Ok(0x89ab_cdef));
        assert_eq!(reader.bit_position(), encoded.len() * 8);
        assert_eq!(reader.bits_remaining(), 0);
    }

    #[test]
    fn msb_reader_failed_fixed_width_read_does_not_consume_buffered_bits() {
        let mut reader = MsbBitReader::new(&[0b1010_1010]);

        assert_eq!(reader.read_bits(3), Ok(0b101));
        assert_eq!(
            reader.read_bits(6),
            Err(LosslessError::UnexpectedEndOfStream {
                bit_position: 3,
                requested: 6,
                remaining: 5,
            })
        );
        assert_eq!(reader.bit_position(), 3);
        assert_eq!(reader.read_bits(5), Ok(0b0_1010));
        assert_eq!(reader.bit_position(), 8);
    }

    #[test]
    fn msb_reader_unary_scan_crosses_a_zero_word() {
        let encoded = [0, 0, 0, 0, 0, 0, 0, 0, 0b0010_0000];
        let mut reader = MsbBitReader::new(&encoded);

        assert_eq!(reader.read_zero_unary(128), Ok(66));
        assert_eq!(reader.bit_position(), 67);
        assert_eq!(reader.bits_remaining(), 5);
    }

    #[test]
    fn msb_reader_unary_limit_consumes_the_offending_zero() {
        let mut reader = MsbBitReader::new(&[0b0001_0000]);

        assert_eq!(
            reader.read_zero_unary(2),
            Err(LosslessError::UnaryRunTooLong {
                bit_position: 0,
                limit: 2,
            })
        );
        assert_eq!(reader.bit_position(), 3);
        assert_eq!(reader.read_bit(), Ok(1));
    }

    #[test]
    fn msb_reader_unary_eof_reports_the_consumed_bit_position() {
        let mut reader = MsbBitReader::new(&[0; 8]);

        assert_eq!(
            reader.read_zero_unary(128),
            Err(LosslessError::UnexpectedEndOfStream {
                bit_position: 64,
                requested: 1,
                remaining: 0,
            })
        );
        assert_eq!(reader.bit_position(), 64);
    }

    #[test]
    fn adaptive_rice_matches_confirmed_transition_rule() {
        let mut rice = AdaptiveRice::new(2);
        let mut reader = MsbBitReader::new(&[0b0100_1111, 0b0100_1100]);

        assert_eq!(rice.decode(&mut reader), Ok(4));
        assert_eq!(rice.parameter(), 2);
        assert_eq!(rice.decode(&mut reader), Ok(3));
        assert_eq!(rice.parameter(), 2);
        assert_eq!(rice.decode(&mut reader), Ok(1));
        assert_eq!(rice.parameter(), 1);
        assert_eq!(rice.decode(&mut reader), Ok(5));
        assert_eq!(rice.parameter(), 1);
    }

    #[test]
    fn adaptive_rice_increases_by_at_most_two_for_one_symbol() {
        let mut rice = AdaptiveRice::new(0);

        rice.adapt(12);

        assert_eq!(rice.parameter(), 2);
    }

    #[test]
    fn branchless_median_predictor_matches_the_piecewise_definition() {
        for left in -8..=8 {
            for above in -8..=8 {
                for upper_left in -8..=8 {
                    let expected = if upper_left <= left.min(above) {
                        left.max(above)
                    } else if upper_left >= left.max(above) {
                        left.min(above)
                    } else {
                        left + above - upper_left
                    };
                    assert_eq!(median_predictor(left, above, upper_left), expected);
                }
            }
        }
    }

    #[test]
    fn eos_r8_plane_zero_first_64_samples_match_black_box_oracle() {
        // First 64 bytes of plane 0 from the owner-supplied EOS R8 fixture.
        let encoded = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x3b, 0xfb, 0x4f, 0x4c, 0x0a, 0x33, 0xd4, 0xb8, 0xd2, 0x84,
            0xe0, 0x35, 0x51, 0x2d, 0x70, 0x5b, 0x54, 0x84, 0x64, 0xf6, 0x21, 0x38, 0xc4, 0x98, 0xbd, 0x8c,
            0x3f, 0x17, 0x50, 0xd1, 0x26, 0x45, 0x1c, 0x63, 0x7c, 0x2b, 0x1e, 0x99, 0x6a, 0x28, 0x91, 0x3e,
            0x03, 0xec, 0x1e, 0x86, 0x1e, 0x85, 0xd0, 0x3e, 0xd2, 0x9b, 0xc9, 0x74, 0x8a, 0xd2, 0x21, 0xb7,
        ];
        let expected = [
            514, 516, 514, 513, 510, 516, 516, 517, 513, 512, 513, 514, 513, 513, 514, 514, 513, 514, 513,
            515, 512, 512, 515, 514, 515, 515, 515, 513, 515, 514, 515, 514, 514, 516, 514, 512, 513, 513,
            513, 514, 516, 512, 516, 514, 513, 513, 514, 516, 513, 513, 514, 514, 512, 514, 512, 515, 513,
            512, 512, 513, 513, 515, 514, 513,
        ];

        let decoded = decode_first_row(&encoded, expected.len(), 14).unwrap();
        assert_eq!(decoded.samples, expected);
        assert_eq!(decoded.bits_consumed, 263);
        assert_eq!(decoded.final_rice_parameter, 1);

        let complete_path = decode_plane(&encoded, expected.len(), 1, 14, &|| false).unwrap();
        assert_eq!(complete_path, expected);
    }

    #[test]
    fn full_plane_checks_cancellation_and_resource_limits_before_decode() {
        assert_eq!(
            decode_plane(&[0xff], 1, 1, 14, &|| true),
            Err(LosslessError::Cancelled { row: 0 })
        );
        assert_eq!(
            decode_plane(&[], 1, MAX_PLANE_SAMPLES + 1, 14, &|| false),
            Err(LosslessError::PlaneSampleLimit {
                samples: MAX_PLANE_SAMPLES + 1,
                limit: MAX_PLANE_SAMPLES,
            })
        );
        assert_eq!(
            decode_plane(&[], MAX_PLANE_WIDTH + 1, 1, 14, &|| false),
            Err(LosslessError::ImpossibleRowWidth {
                width: MAX_PLANE_WIDTH + 1,
                maximum: MAX_PLANE_WIDTH,
            })
        );
    }

    #[test]
    fn prefix_validation_rejects_unproven_layouts() {
        assert_eq!(
            decode_first_row(&[], 1, 14),
            Err(LosslessError::TruncatedPlanePrefix {
                needed: 8,
                available: 0
            })
        );
        assert_eq!(
            decode_first_row(&[0; 8], 1, 12),
            Err(LosslessError::UnsupportedBitDepth { bit_depth: 12 })
        );

        let mut bad_reserved = [0_u8; 8];
        bad_reserved[0] = 1;
        assert!(matches!(
            decode_first_row(&bad_reserved, 1, 14),
            Err(LosslessError::InvalidReservedPrefix { .. })
        ));
    }

    #[test]
    fn rejects_impossible_width_before_allocating_samples() {
        let encoded = [0_u8; LOSSLESS_PLANE_PREFIX_LEN];
        assert_eq!(
            decode_first_row(&encoded, usize::MAX, CONFIRMED_BIT_DEPTH),
            Err(LosslessError::ImpossibleRowWidth {
                width: usize::MAX,
                maximum: 1,
            })
        );
    }

    #[test]
    fn rejects_pathological_unary_run_at_the_confirmed_symbol_bound() {
        let mut encoded = vec![0_u8; LOSSLESS_PLANE_PREFIX_LEN + 1_025];
        encoded[4..8].copy_from_slice(&0x0020_3bfb_u32.to_be_bytes());

        assert!(matches!(
            decode_first_row(&encoded, 2, CONFIRMED_BIT_DEPTH),
            Err(LosslessError::UnaryRunTooLong { limit: 8_191, .. })
        ));
    }
}
