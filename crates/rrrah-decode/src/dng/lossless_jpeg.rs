//! Bounded ISO/IEC 10918-1 lossless Huffman JPEG decoding for DNG
//! `Compression = 7`.
//!
//! This implementation follows the marker syntax in ITU-T T.81 Annex B and
//! the lossless prediction process in Annex H. It deliberately implements
//! only the non-differential Huffman process (`SOF3`). Arithmetic coding,
//! hierarchical frames, DNL, and component subsampling are rejected.

use thiserror::Error;

const MARKER_PREFIX: u8 = 0xff;
const SOF3: u8 = 0xc3;
const DHT: u8 = 0xc4;
const SOI: u8 = 0xd8;
const EOI: u8 = 0xd9;
const SOS: u8 = 0xda;
const DRI: u8 = 0xdd;
const COM: u8 = 0xfe;
const RST0: u8 = 0xd0;
const RST7: u8 = 0xd7;
const MAX_COMPONENTS: usize = 4;
const MAX_HUFFMAN_TABLES: usize = 4;
const MAX_OUTPUT_SAMPLES: usize = 128 * 1024 * 1024;
const CANCELLATION_MCU_GRANULARITY: usize = 1_024;
const LINEARIZATION_CHUNK_SAMPLES: usize = 4_096;
const HUFFMAN_LOOKAHEAD_BITS: usize = 10;
const HUFFMAN_LOOKAHEAD_SIZE: usize = 1 << HUFFMAN_LOOKAHEAD_BITS;

/// A complete lossless-JPEG frame.
///
/// `samples` is row-major and component-interleaved in SOF3 component order.
/// Its length is exactly `width * height * component_ids.len()`. The DNG
/// container is responsible for validating that total against its strip/tile
/// contract; JPEG-internal dimensions do not have to equal TIFF dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LosslessJpegImage {
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) precision: u8,
    pub(crate) component_ids: Vec<u8>,
    pub(crate) samples: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum LosslessJpegError {
    #[error("lossless JPEG is missing its initial SOI marker")]
    MissingSoi,
    #[error(
        "lossless JPEG ended at byte {offset} while reading {context}: need {needed} bytes, have {remaining}"
    )]
    UnexpectedEnd {
        offset: usize,
        context: &'static str,
        needed: usize,
        remaining: usize,
    },
    #[error("expected a JPEG marker prefix at byte {offset}, found {actual:#04x}")]
    ExpectedMarkerPrefix { offset: usize, actual: u8 },
    #[error("unexpected stuffed zero outside entropy data at byte {offset}")]
    StuffedZeroOutsideEntropy { offset: usize },
    #[error("lossless JPEG entropy data has stuffed zero after marker fill at byte {offset}")]
    InvalidEntropyStuffing { offset: usize },
    #[error("JPEG marker {marker:#04x} at byte {offset} is unsupported in lossless DNG")]
    UnsupportedMarker { marker: u8, offset: usize },
    #[error("JPEG marker {marker:#04x} at byte {offset} has invalid segment length {length}")]
    InvalidSegmentLength {
        marker: u8,
        offset: usize,
        length: usize,
    },
    #[error("lossless JPEG contains more than one SOF3 frame")]
    DuplicateFrame,
    #[error("lossless JPEG reached SOS or EOI before SOF3")]
    MissingFrame,
    #[error("lossless JPEG SOF3 precision {precision} is outside 2..=16")]
    UnsupportedPrecision { precision: u8 },
    #[error("lossless JPEG SOF3 has empty dimensions {width}x{height}")]
    EmptyDimensions { width: u16, height: u16 },
    #[error("lossless JPEG SOF3 component count {components} is outside 1..=4")]
    UnsupportedComponentCount { components: usize },
    #[error("lossless JPEG SOF3 repeats component identifier {component_id}")]
    DuplicateFrameComponent { component_id: u8 },
    #[error(
        "lossless JPEG component {component_id} uses unsupported sampling factors {horizontal}x{vertical}"
    )]
    UnsupportedSamplingFactors {
        component_id: u8,
        horizontal: u8,
        vertical: u8,
    },
    #[error("lossless JPEG component {component_id} has nonzero quantization selector {selector}")]
    NonzeroQuantizationSelector { component_id: u8, selector: u8 },
    #[error("lossless JPEG output sample count overflows this platform")]
    SampleCountOverflow,
    #[error("lossless JPEG has {samples} output samples, above the {limit}-sample safety limit")]
    SampleLimit { samples: usize, limit: usize },
    #[error("could not allocate {samples} lossless JPEG samples")]
    AllocationFailed { samples: usize },
    #[error("lossless JPEG DHT uses unsupported table class {class}")]
    UnsupportedHuffmanClass { class: u8 },
    #[error("lossless JPEG DHT table id {table_id} is outside 0..=3")]
    UnsupportedHuffmanTable { table_id: u8 },
    #[error("lossless JPEG DHT table {table_id} has no symbols")]
    EmptyHuffmanTable { table_id: u8 },
    #[error("lossless JPEG DHT table {table_id} is oversubscribed at code length {length}")]
    OversubscribedHuffmanTable { table_id: u8, length: u8 },
    #[error("lossless JPEG DHT table {table_id} contains invalid difference category {category}")]
    InvalidDifferenceCategory { table_id: u8, category: u8 },
    #[error("lossless JPEG scan references undefined DC Huffman table {table_id}")]
    MissingHuffmanTable { table_id: u8 },
    #[error("lossless JPEG external Huffman table {table_id} is supplied more than once")]
    DuplicateExternalHuffmanTable { table_id: u8 },
    #[error(
        "lossless JPEG external Huffman table {table_id} declares {declared} symbols but provides {provided}"
    )]
    ExternalHuffmanSymbolCount {
        table_id: u8,
        declared: usize,
        provided: usize,
    },
    #[error(
        "lossless JPEG linearization curve has {curve_len} entries, but sample {sample_index} has value {value}"
    )]
    LinearizationCurveOutOfRange {
        sample_index: usize,
        value: u16,
        curve_len: usize,
    },
    #[error("lossless JPEG SOS component count {components} is outside the SOF3 frame")]
    InvalidScanComponentCount { components: usize },
    #[error("lossless JPEG SOS references unknown component identifier {component_id}")]
    UnknownScanComponent { component_id: u8 },
    #[error("lossless JPEG SOS repeats component identifier {component_id}")]
    DuplicateScanComponent { component_id: u8 },
    #[error("lossless JPEG SOS component identifiers are not in SOF3 order")]
    ScanComponentOrder,
    #[error("lossless JPEG component {component_id} appears in more than one scan")]
    ComponentDecodedTwice { component_id: u8 },
    #[error("lossless JPEG SOS uses nonzero AC table selector {selector}")]
    NonzeroAcTableSelector { selector: u8 },
    #[error("lossless JPEG SOS predictor selection {selection} is outside 1..=7")]
    UnsupportedPredictor { selection: u8 },
    #[error("lossless JPEG SOS spectral-end parameter must be zero, got {value}")]
    NonzeroSpectralEnd { value: u8 },
    #[error("lossless JPEG SOS successive-approximation high bits must be zero, got {value}")]
    NonzeroSuccessiveApproximation { value: u8 },
    #[error("lossless JPEG point transform {point_transform} must be below precision {precision}")]
    InvalidPointTransform { point_transform: u8, precision: u8 },
    #[error("lossless JPEG restart interval segment must contain exactly two payload bytes")]
    InvalidRestartInterval,
    #[error(
        "lossless JPEG restart interval {interval} is not a multiple of the {mcus_per_row}-MCU lossless row"
    )]
    RestartIntervalNotRowAligned { interval: usize, mcus_per_row: usize },
    #[error(
        "lossless JPEG expected restart marker RST{expected} at byte {offset}, found marker {actual:#04x}"
    )]
    UnexpectedRestart { expected: u8, actual: u8, offset: usize },
    #[error("lossless JPEG entropy padding before marker at byte {offset} contains a zero bit")]
    InvalidEntropyPadding { offset: usize },
    #[error("lossless JPEG has {bits} whole or partial entropy bits after the expected samples")]
    ExtraEntropyData { bits: u8 },
    #[error("lossless JPEG entropy data encountered marker {marker:#04x} at byte {offset} mid-symbol")]
    MarkerInsideEntropySymbol { marker: u8, offset: usize },
    #[error("lossless JPEG Huffman bit sequence does not match its DC table")]
    InvalidHuffmanCode,
    #[error(
        "lossless JPEG reconstructed component {component_id} sample {sample_index} as {value}, above transformed maximum {maximum}"
    )]
    ReconstructedSampleOutOfRange {
        component_id: u8,
        sample_index: usize,
        value: u32,
        maximum: u32,
    },
    #[error("lossless JPEG ended before all SOF3 components were decoded")]
    IncompleteComponentScans,
    #[error("lossless JPEG contains {bytes} trailing bytes after EOI")]
    TrailingBytes { bytes: usize },
    #[error("lossless JPEG decoding was cancelled before row {row}")]
    Cancelled { row: usize },
}

#[derive(Debug, Clone, Copy)]
struct FrameComponent {
    id: u8,
}

#[derive(Debug)]
struct Frame {
    precision: u8,
    width: u16,
    height: u16,
    components: Vec<FrameComponent>,
}

impl Frame {
    fn sample_count(&self) -> Result<usize, LosslessJpegError> {
        usize::from(self.width)
            .checked_mul(usize::from(self.height))
            .and_then(|pixels| pixels.checked_mul(self.components.len()))
            .ok_or(LosslessJpegError::SampleCountOverflow)
    }
}

#[derive(Debug, Clone)]
struct HuffmanTable {
    minimum_code: [u32; 17],
    maximum_code: [u32; 17],
    value_offset: [usize; 17],
    has_codes: [bool; 17],
    symbols: Vec<u8>,
    fast: [FastHuffmanCode; HUFFMAN_LOOKAHEAD_SIZE],
}

#[derive(Debug, Clone, Copy)]
struct FastHuffmanCode {
    symbol: u8,
    length: u8,
}

impl FastHuffmanCode {
    const EMPTY: Self = Self { symbol: 0, length: 0 };
}

impl HuffmanTable {
    fn build(table_id: u8, counts: [u8; 16], symbols: Vec<u8>) -> Result<Self, LosslessJpegError> {
        if symbols.is_empty() {
            return Err(LosslessJpegError::EmptyHuffmanTable { table_id });
        }
        if let Some(&category) = symbols.iter().find(|&&category| category > 16) {
            return Err(LosslessJpegError::InvalidDifferenceCategory { table_id, category });
        }

        let mut minimum_code = [0_u32; 17];
        let mut maximum_code = [0_u32; 17];
        let mut value_offset = [0_usize; 17];
        let mut has_codes = [false; 17];
        let mut fast = [FastHuffmanCode::EMPTY; HUFFMAN_LOOKAHEAD_SIZE];
        let mut code = 0_u32;
        let mut symbol_offset = 0usize;
        // Accept complete trees whose final code is all ones. The encoder-side
        // procedure in T.81 avoids those codes so one-fill padding cannot look
        // like a symbol, but shipped Canon DNGs use a complete DC tree. Exact
        // sample counts plus strict trailing-padding validation keep decoding
        // bounded and make accepting that compatibility case deterministic.
        for (index, count) in counts.into_iter().enumerate() {
            let length = index + 1;
            let count = u32::from(count);
            let limit = 1_u32 << length;
            let Some(end) = code.checked_add(count) else {
                return Err(LosslessJpegError::OversubscribedHuffmanTable {
                    table_id,
                    length: u8::try_from(length).unwrap_or(16),
                });
            };
            if end > limit {
                return Err(LosslessJpegError::OversubscribedHuffmanTable {
                    table_id,
                    length: u8::try_from(length).unwrap_or(16),
                });
            }
            if count != 0 {
                let last = end - 1;
                minimum_code[length] = code;
                maximum_code[length] = last;
                value_offset[length] = symbol_offset;
                has_codes[length] = true;
                let count =
                    usize::try_from(count).map_err(|_| LosslessJpegError::OversubscribedHuffmanTable {
                        table_id,
                        length: u8::try_from(length).unwrap_or(16),
                    })?;
                if length <= HUFFMAN_LOOKAHEAD_BITS {
                    let suffix_bits = HUFFMAN_LOOKAHEAD_BITS - length;
                    let repeats = 1_usize << suffix_bits;
                    let canonical_code =
                        usize::try_from(code).map_err(|_| LosslessJpegError::InvalidHuffmanCode)?;
                    for ordinal in 0..count {
                        let start = (canonical_code + ordinal) << suffix_bits;
                        fast[start..start + repeats].fill(FastHuffmanCode {
                            symbol: symbols[symbol_offset + ordinal],
                            length: u8::try_from(length).unwrap_or(10),
                        });
                    }
                }
                symbol_offset += count;
            }
            code = if length == 16 { end } else { end << 1 };
        }

        Ok(Self {
            minimum_code,
            maximum_code,
            value_offset,
            has_codes,
            symbols,
            fast,
        })
    }

    fn decode_symbol(&self, reader: &mut EntropyReader<'_>) -> Result<u8, LosslessJpegError> {
        if let Some(prefix) = reader.peek_bits(u8::try_from(HUFFMAN_LOOKAHEAD_BITS).unwrap_or(10))? {
            let prefix = usize::try_from(prefix).map_err(|_| LosslessJpegError::InvalidHuffmanCode)?;
            let entry = self.fast[prefix];
            if entry.length != 0 {
                reader.consume_bits(entry.length);
                return Ok(entry.symbol);
            }
        }
        let mut code = 0_u32;
        for length in 1..=16 {
            code = (code << 1) | u32::from(reader.read_bit()?);
            if self.has_codes[length]
                && code >= self.minimum_code[length]
                && code <= self.maximum_code[length]
            {
                let index = self.value_offset[length]
                    + usize::try_from(code - self.minimum_code[length])
                        .map_err(|_| LosslessJpegError::InvalidHuffmanCode)?;
                return self
                    .symbols
                    .get(index)
                    .copied()
                    .ok_or(LosslessJpegError::InvalidHuffmanCode);
            }
        }
        Err(LosslessJpegError::InvalidHuffmanCode)
    }
}

#[derive(Debug, Clone, Copy)]
struct ScanComponent {
    frame_index: usize,
    table_id: u8,
}

#[derive(Debug)]
struct Scan {
    components: Vec<ScanComponent>,
    predictor: u8,
    point_transform: u8,
}

#[derive(Debug, Clone, Copy)]
struct Marker {
    code: u8,
    offset: usize,
}

struct MarkerReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> MarkerReader<'a> {
    const fn new(bytes: &'a [u8], position: usize) -> Self {
        Self { bytes, position }
    }

    fn next_marker(&mut self) -> Result<Marker, LosslessJpegError> {
        let offset = self.position;
        let actual = *self
            .bytes
            .get(self.position)
            .ok_or_else(|| unexpected_end(self.position, "JPEG marker prefix", 1, self.bytes.len()))?;
        if actual != MARKER_PREFIX {
            return Err(LosslessJpegError::ExpectedMarkerPrefix { offset, actual });
        }
        self.position += 1;
        while self.bytes.get(self.position) == Some(&MARKER_PREFIX) {
            self.position += 1;
        }
        let code_offset = self.position;
        let code = *self
            .bytes
            .get(self.position)
            .ok_or_else(|| unexpected_end(self.position, "JPEG marker code", 1, self.bytes.len()))?;
        self.position += 1;
        if code == 0 {
            return Err(LosslessJpegError::StuffedZeroOutsideEntropy { offset: code_offset });
        }
        Ok(Marker { code, offset })
    }

    fn segment(&mut self, marker: Marker) -> Result<&'a [u8], LosslessJpegError> {
        let length_bytes = self.read_exact(2, "JPEG marker segment length")?;
        let length = usize::from(u16::from_be_bytes([length_bytes[0], length_bytes[1]]));
        if length < 2 {
            return Err(LosslessJpegError::InvalidSegmentLength {
                marker: marker.code,
                offset: marker.offset,
                length,
            });
        }
        self.read_exact(length - 2, "JPEG marker segment payload")
    }

    fn read_exact(&mut self, length: usize, context: &'static str) -> Result<&'a [u8], LosslessJpegError> {
        let start = self.position;
        let end = start
            .checked_add(length)
            .ok_or_else(|| unexpected_end(start, context, length, self.bytes.len()))?;
        let bytes = self
            .bytes
            .get(start..end)
            .ok_or_else(|| unexpected_end(start, context, length, self.bytes.len()))?;
        self.position = end;
        Ok(bytes)
    }
}

struct EntropyReader<'a> {
    bytes: &'a [u8],
    position: usize,
    bit_buffer: u64,
    bit_count: u8,
    pending_marker: Option<Marker>,
    word_refill: bool,
}

impl<'a> EntropyReader<'a> {
    const fn new(bytes: &'a [u8], position: usize, word_refill: bool) -> Self {
        Self {
            bytes,
            position,
            bit_buffer: 0,
            bit_count: 0,
            pending_marker: None,
            word_refill,
        }
    }

    #[inline]
    fn read_bit(&mut self) -> Result<u8, LosslessJpegError> {
        u8::try_from(self.read_bits(1)?).map_err(|_| LosslessJpegError::InvalidHuffmanCode)
    }

    #[inline]
    fn read_bits(&mut self, count: u8) -> Result<u32, LosslessJpegError> {
        let Some(value) = self.peek_bits(count)? else {
            let marker = self.pending_marker.ok_or(LosslessJpegError::InvalidHuffmanCode)?;
            return Err(LosslessJpegError::MarkerInsideEntropySymbol {
                marker: marker.code,
                offset: marker.offset,
            });
        };
        self.consume_bits(count);
        Ok(value)
    }

    #[inline]
    fn peek_bits(&mut self, count: u8) -> Result<Option<u32>, LosslessJpegError> {
        while self.bit_count < count {
            if self.word_refill && self.try_refill_plain_run() {
                continue;
            }
            let Some(byte) = self.read_entropy_byte()? else {
                return Ok(None);
            };
            self.bit_buffer = (self.bit_buffer << 8) | u64::from(byte);
            self.bit_count += 8;
        }
        let shift = self.bit_count - count;
        let mask = if count == 0 { 0 } else { (1_u64 << count) - 1 };
        let value = (self.bit_buffer >> shift) & mask;
        Ok(Some(
            u32::try_from(value).map_err(|_| LosslessJpegError::InvalidHuffmanCode)?,
        ))
    }

    /// Bulk-loads six entropy bytes at once when they cannot start a marker
    /// or a stuffing sequence (no `0xFF` among them). Returns `false` when the
    /// next bytes need the per-byte marker-aware path or fewer than eight
    /// bytes remain; both cases keep their original behavior and error
    /// reporting.
    #[inline]
    fn try_refill_plain_run(&mut self) -> bool {
        // The caller only refills while `bit_count < count <= 15`; the guard
        // keeps the 48-bit append below the 64-bit limit even for future
        // callers with larger counts.
        if self.pending_marker.is_some() || self.bit_count > 16 {
            return false;
        }
        let Some(window) = self.bytes.get(self.position..).and_then(|tail| tail.get(..8)) else {
            return false;
        };
        let word = u64::from_be_bytes(window.try_into().expect("an eight-byte window"));
        // SWAR zero-byte detection over the top six bytes after mapping
        // 0xFF to 0x00; the OR forces the unused bottom two bytes nonzero so
        // they cannot false-positive. On match we leave the marker prefix to
        // the per-byte path.
        let probe = (word ^ 0xffff_ffff_ffff_0000) | 0x0000_0000_0000_0101;
        let has_marker_prefix = probe.wrapping_sub(0x0101_0101_0101_0101) & !probe & 0x8080_8080_8080_8080;
        if has_marker_prefix != 0 {
            return false;
        }
        self.bit_buffer = (self.bit_buffer << 48) | (word >> 16);
        self.bit_count += 48;
        self.position += 6;
        true
    }

    #[inline]
    fn consume_bits(&mut self, count: u8) {
        debug_assert!(count <= self.bit_count);
        self.bit_count -= count;
        self.bit_buffer = if self.bit_count == 0 {
            0
        } else {
            self.bit_buffer & ((1_u64 << self.bit_count) - 1)
        };
    }

    fn read_entropy_byte(&mut self) -> Result<Option<u8>, LosslessJpegError> {
        if self.pending_marker.is_some() {
            return Ok(None);
        }
        let offset = self.position;
        let byte = *self.bytes.get(self.position).ok_or_else(|| {
            unexpected_end(self.position, "lossless JPEG entropy byte", 1, self.bytes.len())
        })?;
        self.position += 1;
        if byte != MARKER_PREFIX {
            return Ok(Some(byte));
        }

        let mut saw_marker_fill = false;
        while self.bytes.get(self.position) == Some(&MARKER_PREFIX) {
            saw_marker_fill = true;
            self.position += 1;
        }
        let code = *self.bytes.get(self.position).ok_or_else(|| {
            unexpected_end(
                self.position,
                "lossless JPEG entropy marker or stuffed zero",
                1,
                self.bytes.len(),
            )
        })?;
        self.position += 1;
        if code == 0 {
            if saw_marker_fill {
                return Err(LosslessJpegError::InvalidEntropyStuffing { offset });
            }
            Ok(Some(MARKER_PREFIX))
        } else {
            self.pending_marker = Some(Marker { code, offset });
            Ok(None)
        }
    }

    fn finish_marker(&mut self) -> Result<Marker, LosslessJpegError> {
        if self.bit_count > 7 {
            return Err(LosslessJpegError::ExtraEntropyData { bits: self.bit_count });
        }
        if self.bit_count != 0 {
            let mask = (1_u64 << self.bit_count) - 1;
            if self.bit_buffer != mask {
                return Err(LosslessJpegError::InvalidEntropyPadding {
                    offset: self.position.saturating_sub(1),
                });
            }
            self.bit_buffer = 0;
            self.bit_count = 0;
        }
        if let Some(marker) = self.pending_marker.take() {
            return Ok(marker);
        }
        let mut marker_reader = MarkerReader::new(self.bytes, self.position);
        let marker = marker_reader.next_marker()?;
        self.position = marker_reader.position;
        Ok(marker)
    }
}

/// A DC Huffman table supplied by the caller instead of an in-stream DHT
/// segment.
///
/// Camera formats such as Nikon NEF compression 34713 and Pentax PEF
/// compression 65535 store a proprietary Huffman table in the makernote and
/// emit SOF3 streams without DHT segments. `counts` holds the 16 code-length
/// counts and `symbols` the symbol list, in the exact layout of a DHT
/// segment payload (T.81 B.2.4.2) minus the leading class/id byte — the same
/// layout makernotes use. The symbol list must contain exactly
/// `counts.iter().sum()` entries.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Retained as tested infrastructure: both NEF 34713 and PEF 65535 turned
// out to be raw bitstreams (not marker JPEG), so no production caller exists yet; kept for
// future camera formats that do emit SOF3 streams with makernote Huffman tables.
pub(crate) struct ExternalHuffmanTable<'a> {
    pub(crate) table_id: u8,
    pub(crate) counts: [u8; 16],
    pub(crate) symbols: &'a [u8],
}

/// Decodes a complete lossless Huffman JPEG payload.
pub(crate) fn decode(
    bytes: &[u8],
    cancelled: &dyn Fn() -> bool,
) -> Result<LosslessJpegImage, LosslessJpegError> {
    decode_impl(bytes, cancelled, true, &[], false)
}

/// `word_refill` exists so benchmarks can A/B the bulk entropy refill against
/// the byte-at-a-time reference path in the same process; production callers
/// use [`decode`], which always enables it.
pub(crate) fn decode_with_refill(
    bytes: &[u8],
    cancelled: &dyn Fn() -> bool,
    word_refill: bool,
) -> Result<LosslessJpegImage, LosslessJpegError> {
    decode_impl(bytes, cancelled, word_refill, &[], false)
}

/// Decodes a complete lossless Huffman JPEG payload, seeding the DC Huffman
/// tables from `external_tables` instead of requiring in-stream DHT segments.
///
/// Seeding happens before marker parsing, so an in-stream DHT segment still
/// redefines a seeded slot (standard T.81 redefinition). With
/// `prefer_external` set, in-stream DHT segments are fully validated but
/// cannot overwrite a seeded slot, letting the caller force the makernote
/// table. All other behavior matches [`decode`].
#[allow(dead_code)] // No production caller yet (NEF/PEF are raw bitstreams, not marker JPEG);
// retained with its tests as infrastructure for future marker-stream camera formats.
pub(crate) fn decode_with_external_tables(
    bytes: &[u8],
    cancelled: &dyn Fn() -> bool,
    external_tables: &[ExternalHuffmanTable<'_>],
    prefer_external: bool,
) -> Result<LosslessJpegImage, LosslessJpegError> {
    decode_impl(bytes, cancelled, true, external_tables, prefer_external)
}

#[allow(clippy::too_many_lines)]
fn decode_impl(
    bytes: &[u8],
    cancelled: &dyn Fn() -> bool,
    word_refill: bool,
    external_tables: &[ExternalHuffmanTable<'_>],
    prefer_external: bool,
) -> Result<LosslessJpegImage, LosslessJpegError> {
    let mut reader = MarkerReader::new(bytes, 0);
    if !matches!(reader.next_marker(), Ok(Marker { code: SOI, .. })) {
        return Err(LosslessJpegError::MissingSoi);
    }

    let mut frame: Option<Frame> = None;
    let mut huffman_tables: [Option<HuffmanTable>; MAX_HUFFMAN_TABLES] = std::array::from_fn(|_| None);
    // Seed caller-supplied tables before marker parsing so scans without
    // in-stream DHT segments can reference them. The symbol-count check must
    // run before `HuffmanTable::build`, which assumes the two agree.
    let mut external_slots = [false; MAX_HUFFMAN_TABLES];
    for table in external_tables {
        let slot = usize::from(table.table_id);
        if slot >= MAX_HUFFMAN_TABLES {
            return Err(LosslessJpegError::UnsupportedHuffmanTable {
                table_id: table.table_id,
            });
        }
        if external_slots[slot] {
            return Err(LosslessJpegError::DuplicateExternalHuffmanTable {
                table_id: table.table_id,
            });
        }
        let declared = table
            .counts
            .iter()
            .map(|count| usize::from(*count))
            .sum::<usize>();
        if declared != table.symbols.len() {
            return Err(LosslessJpegError::ExternalHuffmanSymbolCount {
                table_id: table.table_id,
                declared,
                provided: table.symbols.len(),
            });
        }
        huffman_tables[slot] = Some(HuffmanTable::build(
            table.table_id,
            table.counts,
            table.symbols.to_vec(),
        )?);
        external_slots[slot] = true;
    }
    let mut restart_interval = 0usize;
    let mut transformed_samples: Option<Vec<u16>> = None;
    let mut decoded_components = [false; MAX_COMPONENTS];
    let mut scans = 0usize;
    let mut component_point_transforms = [None; MAX_COMPONENTS];
    let mut pending_marker = None;

    loop {
        if cancelled() {
            return Err(LosslessJpegError::Cancelled { row: 0 });
        }
        let marker = pending_marker.take().map_or_else(|| reader.next_marker(), Ok)?;
        match marker.code {
            SOF3 => {
                if frame.is_some() {
                    return Err(LosslessJpegError::DuplicateFrame);
                }
                let payload = reader.segment(marker)?;
                let parsed = parse_frame(payload)?;
                let sample_count = parsed.sample_count()?;
                if sample_count > MAX_OUTPUT_SAMPLES {
                    return Err(LosslessJpegError::SampleLimit {
                        samples: sample_count,
                        limit: MAX_OUTPUT_SAMPLES,
                    });
                }
                let mut samples = Vec::new();
                samples
                    .try_reserve_exact(sample_count)
                    .map_err(|_| LosslessJpegError::AllocationFailed {
                        samples: sample_count,
                    })?;
                samples.resize(sample_count, 0);
                transformed_samples = Some(samples);
                frame = Some(parsed);
            }
            DHT => {
                let payload = reader.segment(marker)?;
                if prefer_external {
                    // Validate the segment fully, but never let it overwrite a
                    // caller-seeded slot; unseeded slots still update.
                    let mut parsed: [Option<HuffmanTable>; MAX_HUFFMAN_TABLES] =
                        std::array::from_fn(|_| None);
                    parse_huffman_tables(payload, &mut parsed)?;
                    for (slot, parsed_table) in parsed.iter_mut().enumerate() {
                        if !external_slots[slot] && parsed_table.is_some() {
                            huffman_tables[slot] = parsed_table.take();
                        }
                    }
                } else {
                    parse_huffman_tables(payload, &mut huffman_tables)?;
                }
            }
            DRI => {
                let payload = reader.segment(marker)?;
                if payload.len() != 2 {
                    return Err(LosslessJpegError::InvalidRestartInterval);
                }
                restart_interval = usize::from(u16::from_be_bytes([payload[0], payload[1]]));
            }
            SOS => {
                let frame = frame.as_ref().ok_or(LosslessJpegError::MissingFrame)?;
                let payload = reader.segment(marker)?;
                let scan = parse_scan(payload, frame, decoded_components, &huffman_tables)?;
                for component in &scan.components {
                    component_point_transforms[component.frame_index] = Some(scan.point_transform);
                }
                let samples = transformed_samples
                    .as_mut()
                    .ok_or(LosslessJpegError::MissingFrame)?;
                let (next_marker, next_position) = decode_scan(
                    bytes,
                    reader.position,
                    frame,
                    &scan,
                    &huffman_tables,
                    restart_interval,
                    samples,
                    cancelled,
                    word_refill,
                )?;
                reader.position = next_position;
                for component in &scan.components {
                    decoded_components[component.frame_index] = true;
                }
                scans += 1;
                pending_marker = Some(next_marker);
            }
            EOI => {
                let frame = frame.as_ref().ok_or(LosslessJpegError::MissingFrame)?;
                if scans == 0
                    || decoded_components[..frame.components.len()]
                        .iter()
                        .any(|decoded| !decoded)
                {
                    return Err(LosslessJpegError::IncompleteComponentScans);
                }
                if reader.position != bytes.len() {
                    return Err(LosslessJpegError::TrailingBytes {
                        bytes: bytes.len() - reader.position,
                    });
                }
                let mut samples = transformed_samples
                    .take()
                    .ok_or(LosslessJpegError::MissingFrame)?;
                let component_count = frame.components.len();
                let point_transforms = &component_point_transforms[..component_count];
                if point_transforms.iter().any(Option::is_none) {
                    return Err(LosslessJpegError::IncompleteComponentScans);
                }
                if point_transforms.iter().any(|transform| *transform != Some(0)) {
                    let row_samples = usize::from(frame.width)
                        .checked_mul(component_count)
                        .ok_or(LosslessJpegError::SampleCountOverflow)?;
                    for (row, samples) in samples.chunks_exact_mut(row_samples).enumerate() {
                        if cancelled() {
                            return Err(LosslessJpegError::Cancelled { row });
                        }
                        for pixel in samples.chunks_exact_mut(component_count) {
                            for (component_index, sample) in pixel.iter_mut().enumerate() {
                                *sample <<= point_transforms[component_index]
                                    .ok_or(LosslessJpegError::IncompleteComponentScans)?;
                            }
                        }
                    }
                }
                return Ok(LosslessJpegImage {
                    width: frame.width,
                    height: frame.height,
                    precision: frame.precision,
                    component_ids: frame.components.iter().map(|component| component.id).collect(),
                    samples,
                });
            }
            0xe0..=0xef | COM => {
                let _ = reader.segment(marker)?;
            }
            SOI | RST0..=RST7 => {
                return Err(LosslessJpegError::UnsupportedMarker {
                    marker: marker.code,
                    offset: marker.offset,
                });
            }
            other => {
                return Err(LosslessJpegError::UnsupportedMarker {
                    marker: other,
                    offset: marker.offset,
                });
            }
        }
    }
}

fn parse_frame(payload: &[u8]) -> Result<Frame, LosslessJpegError> {
    if payload.len() < 6 {
        return Err(LosslessJpegError::InvalidSegmentLength {
            marker: SOF3,
            offset: 0,
            length: payload.len() + 2,
        });
    }
    let precision = payload[0];
    if !(2..=16).contains(&precision) {
        return Err(LosslessJpegError::UnsupportedPrecision { precision });
    }
    let height = u16::from_be_bytes([payload[1], payload[2]]);
    let width = u16::from_be_bytes([payload[3], payload[4]]);
    if width == 0 || height == 0 {
        return Err(LosslessJpegError::EmptyDimensions { width, height });
    }
    let component_count = usize::from(payload[5]);
    if !(1..=MAX_COMPONENTS).contains(&component_count) {
        return Err(LosslessJpegError::UnsupportedComponentCount {
            components: component_count,
        });
    }
    let expected = 6 + component_count * 3;
    if payload.len() != expected {
        return Err(LosslessJpegError::InvalidSegmentLength {
            marker: SOF3,
            offset: 0,
            length: payload.len() + 2,
        });
    }

    let mut components = Vec::new();
    components
        .try_reserve_exact(component_count)
        .map_err(|_| LosslessJpegError::AllocationFailed {
            samples: component_count,
        })?;
    for encoded in payload[6..].chunks_exact(3) {
        let id = encoded[0];
        if components
            .iter()
            .any(|component: &FrameComponent| component.id == id)
        {
            return Err(LosslessJpegError::DuplicateFrameComponent { component_id: id });
        }
        let horizontal = encoded[1] >> 4;
        let vertical = encoded[1] & 0x0f;
        if horizontal != 1 || vertical != 1 {
            return Err(LosslessJpegError::UnsupportedSamplingFactors {
                component_id: id,
                horizontal,
                vertical,
            });
        }
        if encoded[2] != 0 {
            return Err(LosslessJpegError::NonzeroQuantizationSelector {
                component_id: id,
                selector: encoded[2],
            });
        }
        components.push(FrameComponent { id });
    }
    Ok(Frame {
        precision,
        width,
        height,
        components,
    })
}

fn parse_huffman_tables(
    mut payload: &[u8],
    tables: &mut [Option<HuffmanTable>; MAX_HUFFMAN_TABLES],
) -> Result<(), LosslessJpegError> {
    while !payload.is_empty() {
        if payload.len() < 17 {
            return Err(LosslessJpegError::InvalidSegmentLength {
                marker: DHT,
                offset: 0,
                length: payload.len() + 2,
            });
        }
        let destination = payload[0];
        let class = destination >> 4;
        let table_id = destination & 0x0f;
        if class != 0 {
            return Err(LosslessJpegError::UnsupportedHuffmanClass { class });
        }
        if usize::from(table_id) >= tables.len() {
            return Err(LosslessJpegError::UnsupportedHuffmanTable { table_id });
        }
        let mut counts = [0_u8; 16];
        counts.copy_from_slice(&payload[1..17]);
        let symbol_count = counts.iter().map(|count| usize::from(*count)).sum::<usize>();
        let needed = 17usize
            .checked_add(symbol_count)
            .ok_or(LosslessJpegError::SampleCountOverflow)?;
        if payload.len() < needed {
            return Err(LosslessJpegError::InvalidSegmentLength {
                marker: DHT,
                offset: 0,
                length: payload.len() + 2,
            });
        }
        let symbols = payload[17..needed].to_vec();
        tables[usize::from(table_id)] = Some(HuffmanTable::build(table_id, counts, symbols)?);
        payload = &payload[needed..];
    }
    Ok(())
}

fn parse_scan(
    payload: &[u8],
    frame: &Frame,
    decoded_components: [bool; MAX_COMPONENTS],
    tables: &[Option<HuffmanTable>; MAX_HUFFMAN_TABLES],
) -> Result<Scan, LosslessJpegError> {
    let Some(&component_count) = payload.first() else {
        return Err(LosslessJpegError::InvalidSegmentLength {
            marker: SOS,
            offset: 0,
            length: payload.len() + 2,
        });
    };
    let component_count = usize::from(component_count);
    if component_count == 0 || component_count > frame.components.len() {
        return Err(LosslessJpegError::InvalidScanComponentCount {
            components: component_count,
        });
    }
    let expected = 4 + component_count * 2;
    if payload.len() != expected {
        return Err(LosslessJpegError::InvalidSegmentLength {
            marker: SOS,
            offset: 0,
            length: payload.len() + 2,
        });
    }

    let mut components = Vec::new();
    components
        .try_reserve_exact(component_count)
        .map_err(|_| LosslessJpegError::AllocationFailed {
            samples: component_count,
        })?;
    let mut previous_frame_index = None;
    for encoded in payload[1..=component_count * 2].chunks_exact(2) {
        let component_id = encoded[0];
        let Some(frame_index) = frame
            .components
            .iter()
            .position(|component| component.id == component_id)
        else {
            return Err(LosslessJpegError::UnknownScanComponent { component_id });
        };
        if components
            .iter()
            .any(|component: &ScanComponent| component.frame_index == frame_index)
        {
            return Err(LosslessJpegError::DuplicateScanComponent { component_id });
        }
        if previous_frame_index.is_some_and(|previous| frame_index <= previous) {
            return Err(LosslessJpegError::ScanComponentOrder);
        }
        if decoded_components[frame_index] {
            return Err(LosslessJpegError::ComponentDecodedTwice { component_id });
        }
        let table_id = encoded[1] >> 4;
        let ac_selector = encoded[1] & 0x0f;
        if ac_selector != 0 {
            return Err(LosslessJpegError::NonzeroAcTableSelector {
                selector: ac_selector,
            });
        }
        if usize::from(table_id) >= tables.len() {
            return Err(LosslessJpegError::UnsupportedHuffmanTable { table_id });
        }
        if tables[usize::from(table_id)].is_none() {
            return Err(LosslessJpegError::MissingHuffmanTable { table_id });
        }
        components.push(ScanComponent {
            frame_index,
            table_id,
        });
        previous_frame_index = Some(frame_index);
    }

    let parameter_start = 1 + component_count * 2;
    let predictor = payload[parameter_start];
    if !(1..=7).contains(&predictor) {
        return Err(LosslessJpegError::UnsupportedPredictor { selection: predictor });
    }
    let spectral_end = payload[parameter_start + 1];
    if spectral_end != 0 {
        return Err(LosslessJpegError::NonzeroSpectralEnd { value: spectral_end });
    }
    let approximation = payload[parameter_start + 2];
    let high = approximation >> 4;
    let point_transform = approximation & 0x0f;
    if high != 0 {
        return Err(LosslessJpegError::NonzeroSuccessiveApproximation { value: high });
    }
    if point_transform >= frame.precision {
        return Err(LosslessJpegError::InvalidPointTransform {
            point_transform,
            precision: frame.precision,
        });
    }
    Ok(Scan {
        components,
        predictor,
        point_transform,
    })
}

#[allow(clippy::too_many_arguments)]
fn decode_scan(
    bytes: &[u8],
    entropy_position: usize,
    frame: &Frame,
    scan: &Scan,
    huffman_tables: &[Option<HuffmanTable>; MAX_HUFFMAN_TABLES],
    restart_interval: usize,
    samples: &mut [u16],
    cancelled: &dyn Fn() -> bool,
    word_refill: bool,
) -> Result<(Marker, usize), LosslessJpegError> {
    let width = usize::from(frame.width);
    let height = usize::from(frame.height);
    let mcu_count = width
        .checked_mul(height)
        .ok_or(LosslessJpegError::SampleCountOverflow)?;
    if restart_interval != 0 && !restart_interval.is_multiple_of(width) {
        return Err(LosslessJpegError::RestartIntervalNotRowAligned {
            interval: restart_interval,
            mcus_per_row: width,
        });
    }
    let transformed_precision = frame.precision - scan.point_transform;
    let initial_predictor = 1_i32 << (transformed_precision - 1);
    let transformed_maximum = (1_u32 << transformed_precision) - 1;
    let frame_components = frame.components.len();
    let mut entropy = EntropyReader::new(bytes, entropy_position, word_refill);
    let mut expected_restart = 0_u8;
    let mut interval_start_mcu = 0usize;
    let mut interval_start_row = 0usize;

    for mcu in 0..mcu_count {
        let x = mcu % width;
        let y = mcu / width;
        if (x == 0 || mcu.is_multiple_of(CANCELLATION_MCU_GRANULARITY)) && cancelled() {
            return Err(LosslessJpegError::Cancelled { row: y });
        }
        if restart_interval != 0 && mcu != 0 && mcu.is_multiple_of(restart_interval) {
            let marker = entropy.finish_marker()?;
            let expected = RST0 + expected_restart;
            if marker.code != expected {
                return Err(LosslessJpegError::UnexpectedRestart {
                    expected: expected_restart,
                    actual: marker.code,
                    offset: marker.offset,
                });
            }
            expected_restart = (expected_restart + 1) & 7;
            interval_start_mcu = mcu;
            interval_start_row = y;
        }

        for component in &scan.components {
            let table = huffman_tables[usize::from(component.table_id)].as_ref().ok_or(
                LosslessJpegError::MissingHuffmanTable {
                    table_id: component.table_id,
                },
            )?;
            let category = table.decode_symbol(&mut entropy)?;
            let difference = decode_difference(category, &mut entropy)?;
            let sample_index = mcu
                .checked_mul(frame_components)
                .and_then(|index| index.checked_add(component.frame_index))
                .ok_or(LosslessJpegError::SampleCountOverflow)?;
            let predictor = if mcu == interval_start_mcu {
                initial_predictor
            } else if y == interval_start_row {
                i32::from(samples[sample_index - frame_components])
            } else if x == 0 {
                i32::from(samples[sample_index - width * frame_components])
            } else {
                let left = i32::from(samples[sample_index - frame_components]);
                let above = i32::from(samples[sample_index - width * frame_components]);
                let upper_left = i32::from(samples[sample_index - (width + 1) * frame_components]);
                select_predictor(scan.predictor, left, above, upper_left)
            };
            let reconstructed = (predictor + difference) & 0xffff;
            let reconstructed =
                u32::try_from(reconstructed).map_err(|_| LosslessJpegError::SampleCountOverflow)?;
            if reconstructed > transformed_maximum {
                return Err(LosslessJpegError::ReconstructedSampleOutOfRange {
                    component_id: frame.components[component.frame_index].id,
                    sample_index: mcu,
                    value: reconstructed,
                    maximum: transformed_maximum,
                });
            }
            samples[sample_index] =
                u16::try_from(reconstructed).map_err(|_| LosslessJpegError::SampleCountOverflow)?;
        }
    }

    let marker = entropy.finish_marker()?;
    Ok((marker, entropy.position))
}

fn decode_difference(category: u8, entropy: &mut EntropyReader<'_>) -> Result<i32, LosslessJpegError> {
    match category {
        0 => Ok(0),
        1..=15 => {
            let encoded = entropy.read_bits(category)?;
            let threshold = 1_u32 << (category - 1);
            if encoded < threshold {
                Ok(i32::try_from(encoded).unwrap_or(0) + 1 - (1_i32 << category))
            } else {
                Ok(i32::try_from(encoded).unwrap_or(i32::MAX))
            }
        }
        16 => Ok(32_768),
        _ => Err(LosslessJpegError::InvalidHuffmanCode),
    }
}

fn select_predictor(selection: u8, left: i32, above: i32, upper_left: i32) -> i32 {
    match selection {
        1 => left,
        2 => above,
        3 => upper_left,
        4 => left + above - upper_left,
        5 => left + ((above - upper_left) >> 1),
        6 => above + ((left - upper_left) >> 1),
        7 => (left + above) >> 1,
        _ => unreachable!("SOS predictor was validated"),
    }
}

/// Applies a camera linearization curve to decoded samples in place.
///
/// Nikon NEF compression 34713 stores a `u16` lookup table in the makernote
/// (tags 0x8C/0x96) that maps each entropy-decoded sample to its linearized
/// value. Every sample must be a valid index into `curve`; an out-of-range
/// sample is a hard error rather than a clamp so makernote/geometry
/// mismatches surface instead of silently corrupting pixels. `cancelled` is
/// polled once per [`LINEARIZATION_CHUNK_SAMPLES`] samples; a cancellation
/// reports the number of samples processed so far in the `row` field of
/// [`LosslessJpegError::Cancelled`]. Samples processed before the
/// cancellation remain linearized, so callers must discard the buffer on
/// error.
pub(crate) fn apply_linearization_curve(
    samples: &mut [u16],
    curve: &[u16],
    cancelled: &dyn Fn() -> bool,
) -> Result<(), LosslessJpegError> {
    for (chunk_index, chunk) in samples.chunks_mut(LINEARIZATION_CHUNK_SAMPLES).enumerate() {
        let processed = chunk_index * LINEARIZATION_CHUNK_SAMPLES;
        if cancelled() {
            return Err(LosslessJpegError::Cancelled { row: processed });
        }
        for (offset, sample) in chunk.iter_mut().enumerate() {
            let index = usize::from(*sample);
            let Some(&mapped) = curve.get(index) else {
                return Err(LosslessJpegError::LinearizationCurveOutOfRange {
                    sample_index: processed + offset,
                    value: *sample,
                    curve_len: curve.len(),
                });
            };
            *sample = mapped;
        }
    }
    Ok(())
}

fn unexpected_end(
    offset: usize,
    context: &'static str,
    needed: usize,
    total_length: usize,
) -> LosslessJpegError {
    LosslessJpegError::UnexpectedEnd {
        offset,
        context,
        needed,
        remaining: total_length.saturating_sub(offset),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PRECISION: u8 = 12;

    struct BitWriter {
        bytes: Vec<u8>,
        current: u8,
        used: u8,
    }

    impl BitWriter {
        const fn new() -> Self {
            Self {
                bytes: Vec::new(),
                current: 0,
                used: 0,
            }
        }

        fn write(&mut self, value: u32, bits: u8) {
            for shift in (0..bits).rev() {
                self.current = (self.current << 1) | u8::try_from((value >> shift) & 1).unwrap();
                self.used += 1;
                if self.used == 8 {
                    self.flush_byte();
                }
            }
        }

        fn pad_ones(&mut self) {
            if self.used != 0 {
                while self.used != 8 {
                    self.current = (self.current << 1) | 1;
                    self.used += 1;
                }
                self.flush_byte();
            }
        }

        fn flush_byte(&mut self) {
            self.bytes.push(self.current);
            if self.current == 0xff {
                self.bytes.push(0);
            }
            self.current = 0;
            self.used = 0;
        }
    }

    fn marker(output: &mut Vec<u8>, code: u8) {
        output.extend_from_slice(&[MARKER_PREFIX, code]);
    }

    fn segment(output: &mut Vec<u8>, code: u8, payload: &[u8]) {
        marker(output, code);
        let length = u16::try_from(payload.len() + 2).unwrap();
        output.extend_from_slice(&length.to_be_bytes());
        output.extend_from_slice(payload);
    }

    fn category_and_bits(difference: i32) -> (u8, u32) {
        if difference == 0 {
            return (0, 0);
        }
        if difference == -32_768 || difference == 32_768 {
            return (16, 0);
        }
        let magnitude = difference.unsigned_abs();
        let category = u8::try_from(32 - magnitude.leading_zeros()).unwrap();
        if difference > 0 {
            (category, magnitude)
        } else {
            let mask = (1_u32 << category) - 1;
            (
                category,
                u32::try_from(difference + i32::try_from(mask).unwrap()).unwrap(),
            )
        }
    }

    fn signed_modulo_difference(sample: i32, predictor: i32) -> i32 {
        let modulo = (sample - predictor) & 0xffff;
        if modulo > 32_767 { modulo - 65_536 } else { modulo }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_fixture(
        width: u16,
        height: u16,
        precision: u8,
        components: usize,
        predictor: u8,
        point_transform: u8,
        restart_interval: usize,
        samples: &[u16],
    ) -> Vec<u8> {
        let scan_order = (0..components).collect::<Vec<_>>();
        build_fixture_with_scan_order(
            width,
            height,
            precision,
            components,
            predictor,
            point_transform,
            restart_interval,
            &scan_order,
            samples,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build_fixture_with_scan_order(
        width: u16,
        height: u16,
        precision: u8,
        components: usize,
        predictor: u8,
        point_transform: u8,
        restart_interval: usize,
        scan_order: &[usize],
        samples: &[u16],
    ) -> Vec<u8> {
        assert!((1..=4).contains(&components));
        assert_eq!(scan_order.len(), components);
        assert!(
            (0..components)
                .all(|component| scan_order.iter().filter(|&&item| item == component).count() == 1)
        );
        assert_eq!(
            samples.len(),
            usize::from(width) * usize::from(height) * components
        );
        let transformed = samples
            .iter()
            .map(|sample| *sample >> point_transform)
            .collect::<Vec<_>>();

        let mut output = Vec::new();
        marker(&mut output, SOI);

        let mut dht = vec![0];
        dht.extend_from_slice(&[0, 0, 0, 0, 17, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        dht.extend(0_u8..=16);
        segment(&mut output, DHT, &dht);

        let mut frame = vec![precision];
        frame.extend_from_slice(&height.to_be_bytes());
        frame.extend_from_slice(&width.to_be_bytes());
        frame.push(u8::try_from(components).unwrap());
        for component in 0..components {
            frame.extend_from_slice(&[u8::try_from(component + 1).unwrap(), 0x11, 0]);
        }
        segment(&mut output, SOF3, &frame);

        if restart_interval != 0 {
            segment(
                &mut output,
                DRI,
                &u16::try_from(restart_interval).unwrap().to_be_bytes(),
            );
        }

        let mut scan = vec![u8::try_from(components).unwrap()];
        for &component in scan_order {
            scan.extend_from_slice(&[u8::try_from(component + 1).unwrap(), 0]);
        }
        scan.extend_from_slice(&[predictor, 0, point_transform]);
        segment(&mut output, SOS, &scan);

        let width = usize::from(width);
        let mcu_count = width * usize::from(height);
        let frame_components = components;
        let initial = 1_i32 << (precision - point_transform - 1);
        let mut bits = BitWriter::new();
        let mut restart_index = 0_u8;
        let mut interval_start_mcu = 0usize;
        let mut interval_start_row = 0usize;
        for mcu in 0..mcu_count {
            if restart_interval != 0 && mcu != 0 && mcu.is_multiple_of(restart_interval) {
                bits.pad_ones();
                output.extend_from_slice(&bits.bytes);
                bits.bytes.clear();
                marker(&mut output, RST0 + restart_index);
                restart_index = (restart_index + 1) & 7;
                interval_start_mcu = mcu;
                interval_start_row = mcu / width;
            }
            let x = mcu % width;
            let y = mcu / width;
            for &component in scan_order {
                let index = mcu * frame_components + component;
                let sample = i32::from(transformed[index]);
                let predicted = if mcu == interval_start_mcu {
                    initial
                } else if y == interval_start_row {
                    i32::from(transformed[index - frame_components])
                } else if x == 0 {
                    i32::from(transformed[index - width * frame_components])
                } else {
                    let left = i32::from(transformed[index - frame_components]);
                    let above = i32::from(transformed[index - width * frame_components]);
                    let upper_left = i32::from(transformed[index - (width + 1) * frame_components]);
                    select_predictor(predictor, left, above, upper_left)
                };
                let difference = signed_modulo_difference(sample, predicted);
                let (category, encoded) = category_and_bits(difference);
                bits.write(u32::from(category), 5);
                if category < 16 {
                    bits.write(encoded, category);
                }
            }
        }
        bits.pad_ones();
        output.extend_from_slice(&bits.bytes);
        marker(&mut output, EOI);
        output
    }

    fn build_noninterleaved_fixture(
        width: u16,
        height: u16,
        precision: u8,
        predictor: u8,
        scan_order: &[usize],
        point_transforms: &[u8],
        samples: &[u16],
    ) -> Vec<u8> {
        let components = scan_order.len();
        assert!((1..=4).contains(&components));
        assert_eq!(point_transforms.len(), components);
        assert!(
            (0..components)
                .all(|component| scan_order.iter().filter(|&&item| item == component).count() == 1)
        );
        assert_eq!(
            samples.len(),
            usize::from(width) * usize::from(height) * components
        );
        let transformed = samples
            .iter()
            .enumerate()
            .map(|(index, sample)| *sample >> point_transforms[index % components])
            .collect::<Vec<_>>();

        let mut output = Vec::new();
        marker(&mut output, SOI);
        let mut dht = vec![0];
        dht.extend_from_slice(&[0, 0, 0, 0, 17, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        dht.extend(0_u8..=16);
        segment(&mut output, DHT, &dht);

        let mut frame = vec![precision];
        frame.extend_from_slice(&height.to_be_bytes());
        frame.extend_from_slice(&width.to_be_bytes());
        frame.push(u8::try_from(components).unwrap());
        for component in 0..components {
            frame.extend_from_slice(&[u8::try_from(component + 1).unwrap(), 0x11, 0]);
        }
        segment(&mut output, SOF3, &frame);

        let width = usize::from(width);
        let mcu_count = width * usize::from(height);
        for &component in scan_order {
            let point_transform = point_transforms[component];
            let initial = 1_i32 << (precision - point_transform - 1);
            let scan = [
                1,
                u8::try_from(component + 1).unwrap(),
                0,
                predictor,
                0,
                point_transform,
            ];
            segment(&mut output, SOS, &scan);

            let mut bits = BitWriter::new();
            for mcu in 0..mcu_count {
                let x = mcu % width;
                let y = mcu / width;
                let index = mcu * components + component;
                let predicted = if mcu == 0 {
                    initial
                } else if y == 0 {
                    i32::from(transformed[index - components])
                } else if x == 0 {
                    i32::from(transformed[index - width * components])
                } else {
                    let left = i32::from(transformed[index - components]);
                    let above = i32::from(transformed[index - width * components]);
                    let upper_left = i32::from(transformed[index - (width + 1) * components]);
                    select_predictor(predictor, left, above, upper_left)
                };
                let difference = signed_modulo_difference(i32::from(transformed[index]), predicted);
                let (category, encoded) = category_and_bits(difference);
                bits.write(u32::from(category), 5);
                if category < 16 {
                    bits.write(encoded, category);
                }
            }
            bits.pad_ones();
            output.extend_from_slice(&bits.bytes);
        }
        marker(&mut output, EOI);
        output
    }

    fn patterned_samples(width: usize, height: usize, components: usize) -> Vec<u16> {
        let mut samples = Vec::with_capacity(width * height * components);
        for row in 0..height {
            for column in 0..width {
                for component in 0..components {
                    samples.push(
                        u16::try_from(300 + component * 200 + row * 29 + column * 11 + (row * column) % 7)
                            .unwrap(),
                    );
                }
            }
        }
        samples
    }

    #[test]
    fn decodes_all_seven_annex_h_predictors() {
        let samples = patterned_samples(5, 4, 1);
        for predictor in 1..=7 {
            let fixture = build_fixture(5, 4, TEST_PRECISION, 1, predictor, 0, 0, &samples);
            let decoded = decode(&fixture, &|| false).unwrap();
            assert_eq!(decoded.width, 5);
            assert_eq!(decoded.height, 4);
            assert_eq!(decoded.precision, TEST_PRECISION);
            assert_eq!(decoded.component_ids, [1]);
            assert_eq!(decoded.samples, samples, "predictor {predictor}");
        }
    }

    #[test]
    fn decodes_one_through_four_interleaved_components() {
        for components in 1..=4 {
            let samples = patterned_samples(4, 3, components);
            let fixture = build_fixture(4, 3, TEST_PRECISION, components, 4, 0, 0, &samples);
            let decoded = decode(&fixture, &|| false).unwrap();
            assert_eq!(
                decoded.component_ids,
                (1..=u8::try_from(components).unwrap()).collect::<Vec<_>>()
            );
            assert_eq!(decoded.samples, samples);
        }
    }

    #[test]
    fn rejects_interleaved_scan_components_out_of_sof_order() {
        let samples = patterned_samples(4, 3, 3);
        let fixture = build_fixture_with_scan_order(4, 3, TEST_PRECISION, 3, 5, 0, 0, &[2, 0, 1], &samples);
        assert_eq!(
            decode(&fixture, &|| false),
            Err(LosslessJpegError::ScanComponentOrder)
        );
    }

    #[test]
    fn decodes_noninterleaved_component_scans_in_any_order() {
        let samples = patterned_samples(5, 4, 3);
        let fixture = build_noninterleaved_fixture(5, 4, TEST_PRECISION, 7, &[2, 0, 1], &[0, 0, 0], &samples);
        let decoded = decode(&fixture, &|| false).unwrap();
        assert_eq!(decoded.component_ids, [1, 2, 3]);
        assert_eq!(decoded.samples, samples);
    }

    #[test]
    fn applies_point_transform_per_noninterleaved_component_scan() {
        let point_transforms = [1, 2, 3];
        let samples = patterned_samples(5, 4, 3)
            .into_iter()
            .enumerate()
            .map(|(index, sample)| {
                let transform = point_transforms[index % point_transforms.len()];
                sample & !((1_u16 << transform) - 1)
            })
            .collect::<Vec<_>>();
        let fixture =
            build_noninterleaved_fixture(5, 4, TEST_PRECISION, 6, &[2, 0, 1], &point_transforms, &samples);
        let decoded = decode(&fixture, &|| false).unwrap();
        assert_eq!(decoded.samples, samples);
    }

    #[test]
    fn applies_point_transform_after_prediction() {
        let samples = patterned_samples(4, 3, 2)
            .into_iter()
            .map(|sample| sample & !3)
            .collect::<Vec<_>>();
        let fixture = build_fixture(4, 3, 12, 2, 7, 2, 0, &samples);
        let decoded = decode(&fixture, &|| false).unwrap();
        assert_eq!(decoded.samples, samples);
    }

    #[test]
    fn resets_prediction_at_row_aligned_restart_markers() {
        let samples = patterned_samples(4, 5, 3);
        let fixture = build_fixture(4, 5, TEST_PRECISION, 3, 6, 0, 8, &samples);
        let decoded = decode(&fixture, &|| false).unwrap();
        assert_eq!(decoded.samples, samples);
    }

    #[test]
    fn restart_sequence_wraps_from_rst7_to_rst0() {
        let samples = patterned_samples(2, 18, 1);
        let fixture = build_fixture(2, 18, TEST_PRECISION, 1, 4, 0, 2, &samples);
        assert_eq!(decode(&fixture, &|| false).unwrap().samples, samples);
    }

    #[test]
    fn predictor_formulas_cover_negative_odd_deltas_independently() {
        assert_eq!(select_predictor(1, 100, 80, 91), 100);
        assert_eq!(select_predictor(2, 100, 80, 91), 80);
        assert_eq!(select_predictor(3, 100, 80, 91), 91);
        assert_eq!(select_predictor(4, 100, 80, 91), 89);
        assert_eq!(select_predictor(5, 100, 80, 91), 94);
        assert_eq!(select_predictor(6, 100, 80, 91), 84);
        assert_eq!(select_predictor(7, 100, 80, 91), 90);
    }

    #[test]
    fn decodes_category_sixteen_modulo_difference() {
        let samples = [0_u16];
        let fixture = build_fixture(1, 1, 16, 1, 1, 0, 0, &samples);
        let decoded = decode(&fixture, &|| false).unwrap();
        assert_eq!(decoded.samples, samples);
    }

    #[test]
    fn word_refill_matches_bytewise_on_stuffed_restart_stream() {
        // Samples chosen so the entropy stream contains many 0xFF bytes
        // (large pseudo-random differences) plus row-aligned restart markers,
        // exercising both the bulk fast path and the per-byte fallback.
        let width = 37_u16;
        let height = 23_u16;
        let mut state = 0x243f_6a88_85a3_08d3_u64;
        let samples = (0..usize::from(width) * usize::from(height))
            .map(|_| {
                state ^= state >> 12;
                state ^= state << 25;
                state ^= state >> 27;
                u16::try_from(state.wrapping_mul(0x2545_f491_4f6c_dd1d) & 0xfff).unwrap()
            })
            .collect::<Vec<_>>();
        let fixture = build_fixture(
            width,
            height,
            TEST_PRECISION,
            1,
            4,
            0,
            usize::from(width),
            &samples,
        );
        assert!(
            fixture.windows(2).any(|pair| pair == [MARKER_PREFIX, 0]),
            "fixture must contain stuffed 0xFF00 pairs"
        );
        let word = decode(&fixture, &|| false).unwrap();
        let bytewise = decode_with_refill(&fixture, &|| false, false).unwrap();
        assert_eq!(word, bytewise);
        assert_eq!(word.samples, samples);
    }

    #[test]
    fn entropy_reader_unstuffs_ff_and_finds_eoi() {
        let mut reader = EntropyReader::new(&[0xff, 0x00, 0x80, 0xff, EOI], 0, true);
        assert_eq!(reader.read_bits(8), Ok(0xff));
        assert_eq!(reader.read_bits(1), Ok(1));
        for _ in 0..7 {
            let _ = reader.read_bit().unwrap();
        }
        assert_eq!(reader.finish_marker().unwrap().code, EOI);
    }

    #[test]
    fn accepts_marker_fill_before_initial_soi() {
        let samples = patterned_samples(2, 2, 1);
        let mut fixture = build_fixture(2, 2, TEST_PRECISION, 1, 1, 0, 0, &samples);
        fixture.insert(1, MARKER_PREFIX);
        assert_eq!(decode(&fixture, &|| false).unwrap().samples, samples);
    }

    #[test]
    fn rejects_stuffed_zero_after_entropy_marker_fill() {
        let mut reader = EntropyReader::new(&[MARKER_PREFIX, MARKER_PREFIX, 0], 0, true);
        assert_eq!(
            reader.read_bits(8),
            Err(LosslessJpegError::InvalidEntropyStuffing { offset: 0 })
        );
    }

    #[test]
    fn accepts_the_complete_huffman_table_shape_used_by_canon() {
        let counts = [1, 0, 2, 2, 2, 3, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0];
        let symbols = vec![3, 4, 2, 5, 1, 6, 0, 7, 8, 9, 10, 11, 12, 13, 0];
        HuffmanTable::build(0, counts, symbols).unwrap();
    }

    #[test]
    fn parses_multiple_tables_and_allows_later_dht_redefinition() {
        let mut tables: [Option<HuffmanTable>; MAX_HUFFMAN_TABLES] = std::array::from_fn(|_| None);
        let mut payload = vec![0];
        payload.extend_from_slice(&[0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        payload.push(2);
        payload.push(1);
        payload.extend_from_slice(&[0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        payload.push(3);
        parse_huffman_tables(&payload, &mut tables).unwrap();
        assert_eq!(tables[0].as_ref().unwrap().symbols, [2]);
        assert_eq!(tables[1].as_ref().unwrap().symbols, [3]);

        let mut replacement = vec![0];
        replacement.extend_from_slice(&[0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        replacement.push(4);
        parse_huffman_tables(&replacement, &mut tables).unwrap();
        assert_eq!(tables[0].as_ref().unwrap().symbols, [4]);
        assert_eq!(tables[1].as_ref().unwrap().symbols, [3]);
    }

    #[test]
    fn huffman_fallback_decodes_codes_longer_than_fast_lookup() {
        let table =
            HuffmanTable::build(0, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0], vec![5]).unwrap();
        let mut reader = EntropyReader::new(&[0, 0x1f, MARKER_PREFIX, EOI], 0, true);
        assert_eq!(table.decode_symbol(&mut reader), Ok(5));
        assert_eq!(reader.finish_marker().unwrap().code, EOI);
    }

    #[test]
    fn rejects_wrong_restart_number() {
        let samples = patterned_samples(4, 5, 1);
        let mut fixture = build_fixture(4, 5, TEST_PRECISION, 1, 1, 0, 4, &samples);
        let restart = fixture
            .windows(2)
            .position(|bytes| bytes == [MARKER_PREFIX, RST0])
            .unwrap();
        fixture[restart + 1] = RST0 + 3;
        assert!(matches!(
            decode(&fixture, &|| false),
            Err(LosslessJpegError::UnexpectedRestart {
                expected: 0,
                actual,
                ..
            }) if actual == RST0 + 3
        ));
    }

    #[test]
    fn rejects_restart_interval_inside_a_lossless_row() {
        let samples = patterned_samples(4, 3, 1);
        let fixture = build_fixture(4, 3, TEST_PRECISION, 1, 1, 0, 2, &samples);
        assert!(matches!(
            decode(&fixture, &|| false),
            Err(LosslessJpegError::RestartIntervalNotRowAligned {
                interval: 2,
                mcus_per_row: 4,
            })
        ));
    }

    #[test]
    fn rejects_truncated_and_non_sof3_inputs() {
        assert_eq!(decode(&[], &|| false), Err(LosslessJpegError::MissingSoi));
        let truncated = [MARKER_PREFIX, SOI, MARKER_PREFIX, DHT, 0, 20, 0];
        assert!(matches!(
            decode(&truncated, &|| false),
            Err(LosslessJpegError::UnexpectedEnd { .. })
        ));
        let unsupported = [MARKER_PREFIX, SOI, MARKER_PREFIX, 0xc0];
        assert!(matches!(
            decode(&unsupported, &|| false),
            Err(LosslessJpegError::UnsupportedMarker { marker: 0xc0, .. })
        ));

        let lossy_quantization_table = [MARKER_PREFIX, SOI, MARKER_PREFIX, 0xdb, 0, 2];
        assert!(matches!(
            decode(&lossy_quantization_table, &|| false),
            Err(LosslessJpegError::UnsupportedMarker { marker: 0xdb, .. })
        ));
    }

    #[test]
    fn rejects_zero_padding_and_marker_inside_a_symbol() {
        let mut zero_padding = build_fixture(1, 1, TEST_PRECISION, 1, 1, 0, 0, &[1 << (TEST_PRECISION - 1)]);
        let entropy_offset = zero_padding.len() - 3;
        zero_padding[entropy_offset] &= !1;
        assert!(matches!(
            decode(&zero_padding, &|| false),
            Err(LosslessJpegError::InvalidEntropyPadding { .. })
        ));

        let mut marker_inside = build_fixture(1, 1, TEST_PRECISION, 1, 1, 0, 0, &[1 << (TEST_PRECISION - 1)]);
        let entropy_offset = marker_inside.len() - 3;
        marker_inside[entropy_offset] = MARKER_PREFIX;
        assert!(matches!(
            decode(&marker_inside, &|| false),
            Err(LosslessJpegError::MarkerInsideEntropySymbol { marker: EOI, .. })
        ));
    }

    #[test]
    fn rejects_component_subsampling() {
        let samples = patterned_samples(2, 2, 1);
        let mut fixture = build_fixture(2, 2, TEST_PRECISION, 1, 1, 0, 0, &samples);
        let sof = fixture
            .windows(2)
            .position(|bytes| bytes == [MARKER_PREFIX, SOF3])
            .unwrap();
        fixture[sof + 11] = 0x21;
        assert!(matches!(
            decode(&fixture, &|| false),
            Err(LosslessJpegError::UnsupportedSamplingFactors {
                horizontal: 2,
                vertical: 1,
                ..
            })
        ));
    }

    #[test]
    fn cancellation_is_polled_during_entropy_decode() {
        use std::cell::Cell;

        let samples = patterned_samples(64, 64, 1);
        let fixture = build_fixture(64, 64, TEST_PRECISION, 1, 1, 0, 0, &samples);
        let polls = Cell::new(0usize);
        let cancelled = || {
            let next = polls.get() + 1;
            polls.set(next);
            next >= 4
        };
        assert!(matches!(
            decode(&fixture, &cancelled),
            Err(LosslessJpegError::Cancelled { .. })
        ));
    }

    /// The counts/symbols pair matching the DHT segment every fixture
    /// builder emits: 17 canonical five-bit codes mapping to categories
    /// 0..=16.
    const FIXTURE_COUNTS: [u8; 16] = [0, 0, 0, 0, 17, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    const FIXTURE_SYMBOLS: [u8; 17] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

    fn fixture_external_table() -> ExternalHuffmanTable<'static> {
        ExternalHuffmanTable {
            table_id: 0,
            counts: FIXTURE_COUNTS,
            symbols: &FIXTURE_SYMBOLS,
        }
    }

    /// Removes the first DHT segment, simulating camera streams (Nikon NEF
    /// compression 34713, Pentax PEF compression 65535) that carry their
    /// Huffman table in the makernote instead of in-band.
    fn strip_dht_segment(fixture: &mut Vec<u8>) {
        assert_eq!(&fixture[..2], &[MARKER_PREFIX, SOI]);
        assert_eq!(&fixture[2..4], &[MARKER_PREFIX, DHT]);
        let length = usize::from(u16::from_be_bytes([fixture[4], fixture[5]]));
        fixture.drain(2..2 + 2 + length);
    }

    #[test]
    fn decodes_stream_without_dht_using_external_table() {
        let samples = patterned_samples(6, 5, 2);
        let mut fixture = build_fixture(6, 5, TEST_PRECISION, 2, 4, 0, 0, &samples);
        strip_dht_segment(&mut fixture);
        assert_eq!(
            decode(&fixture, &|| false),
            Err(LosslessJpegError::MissingHuffmanTable { table_id: 0 })
        );
        let decoded =
            decode_with_external_tables(&fixture, &|| false, &[fixture_external_table()], false).unwrap();
        assert_eq!(decoded.width, 6);
        assert_eq!(decoded.height, 5);
        assert_eq!(decoded.precision, TEST_PRECISION);
        assert_eq!(decoded.component_ids, [1, 2]);
        assert_eq!(decoded.samples, samples);
    }

    #[test]
    fn external_table_symbol_count_must_match_counts() {
        let samples = patterned_samples(2, 2, 1);
        let mut fixture = build_fixture(2, 2, TEST_PRECISION, 1, 1, 0, 0, &samples);
        strip_dht_segment(&mut fixture);
        let short = ExternalHuffmanTable {
            table_id: 0,
            counts: FIXTURE_COUNTS,
            symbols: &FIXTURE_SYMBOLS[..2],
        };
        assert_eq!(
            decode_with_external_tables(&fixture, &|| false, &[short], false),
            Err(LosslessJpegError::ExternalHuffmanSymbolCount {
                table_id: 0,
                declared: 17,
                provided: 2,
            })
        );
        let duplicate = [fixture_external_table(), fixture_external_table()];
        assert_eq!(
            decode_with_external_tables(&fixture, &|| false, &duplicate, false),
            Err(LosslessJpegError::DuplicateExternalHuffmanTable { table_id: 0 })
        );
        let out_of_range_id = ExternalHuffmanTable {
            table_id: 4,
            ..fixture_external_table()
        };
        assert_eq!(
            decode_with_external_tables(&fixture, &|| false, &[out_of_range_id], false),
            Err(LosslessJpegError::UnsupportedHuffmanTable { table_id: 4 })
        );
    }

    #[test]
    fn in_stream_dht_overrides_external_table_unless_forced() {
        let samples = patterned_samples(4, 3, 1);
        let fixture = build_fixture(4, 3, TEST_PRECISION, 1, 1, 0, 0, &samples);
        // A valid but wrong external table: the same code lengths with the
        // category symbols reversed.
        let mut reversed_symbols = FIXTURE_SYMBOLS;
        reversed_symbols.reverse();
        let wrong = ExternalHuffmanTable {
            table_id: 0,
            counts: FIXTURE_COUNTS,
            symbols: &reversed_symbols,
        };
        // Default: the in-stream DHT redefines the seeded slot and wins.
        let decoded =
            decode_with_external_tables(&fixture, &|| false, std::slice::from_ref(&wrong), false).unwrap();
        assert_eq!(decoded.samples, samples);
        // Forced: the makernote-style external table wins, so decoding can no
        // longer reproduce the fixture's samples.
        let forced = decode_with_external_tables(&fixture, &|| false, &[wrong], true);
        assert!(forced.is_err() || forced.unwrap().samples != samples);
    }

    #[test]
    fn applies_linearization_curve_with_checked_bounds() {
        let curve = [0_u16, 100, 200, 300];
        let mut samples = vec![0, 1, 2, 3, 2, 0];
        apply_linearization_curve(&mut samples, &curve, &|| false).unwrap();
        assert_eq!(samples, [0, 100, 200, 300, 200, 0]);

        let mut out_of_range = vec![0_u16, 4, 1];
        assert_eq!(
            apply_linearization_curve(&mut out_of_range, &curve, &|| false),
            Err(LosslessJpegError::LinearizationCurveOutOfRange {
                sample_index: 1,
                value: 4,
                curve_len: 4,
            })
        );

        let mut empty_curve_samples = vec![0_u16];
        assert_eq!(
            apply_linearization_curve(&mut empty_curve_samples, &[], &|| false),
            Err(LosslessJpegError::LinearizationCurveOutOfRange {
                sample_index: 0,
                value: 0,
                curve_len: 0,
            })
        );
    }

    #[test]
    fn linearization_curve_honours_cancellation() {
        use std::cell::Cell;

        let mut samples = vec![0_u16; LINEARIZATION_CHUNK_SAMPLES * 2 + 1];
        let curve = [7_u16];
        let polls = Cell::new(0usize);
        let cancelled = || {
            let next = polls.get() + 1;
            polls.set(next);
            next >= 2
        };
        assert_eq!(
            apply_linearization_curve(&mut samples, &curve, &cancelled),
            Err(LosslessJpegError::Cancelled {
                row: LINEARIZATION_CHUNK_SAMPLES,
            })
        );
        // The first chunk was linearized before cancellation reported.
        assert!(
            samples[..LINEARIZATION_CHUNK_SAMPLES]
                .iter()
                .all(|&sample| sample == 7)
        );
    }
}
