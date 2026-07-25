//! Pentax/Ricoh PEF quirks — Stage 2 implementation.
//!
//! PEF is a classic TIFF container (byte order `II` on every known body, `MM`
//! allowed by the TIFF reader). The raw CFA mosaic lives in a primary
//! directory with `PhotometricInterpretation` 32803 (CFA), so the default
//! container parsing and the generic raw-IFD selection from `camtiff/mod.rs`
//! are used unchanged.
//!
//! # Supported pixel storage
//!
//! - `Compression = 1` (uncompressed), `BitsPerSample` 12 or 14: MSB-first
//!   packed rows decoded with [`decode_msb_packed`]. This covers the older
//!   bodies (*ist D series, K10D, K20D). `BitsPerSample` 16 is also accepted
//!   and read with the container byte order.
//! - `Compression = 7` (lossless JPEG, TIFF/EP): each strip is an independent
//!   ISO 10918-1 SOF3 stream decoded with the shared
//!   [`lossless_jpeg`] decoder. This covers the K-x and every newer body
//!   (K-5, K-3, K-1, ...).
//! - `Compression = 65535` (Pentax-specific Huffman coding): the strip is a
//!   bare MSB-first Huffman bitstream — no SOI/SOF3/SOS/EOI markers and no
//!   0xFF00 byte stuffing — with a custom two-column predictor, and the DC
//!   Huffman table lives in the Pentax makernote instead of an in-stream DHT
//!   segment. Decoded with [`decode_pentax_jpeg`]; see its documentation for
//!   the on-disk layout research (dcraw `pentax_load_raw`, rawspeed
//!   `PentaxDecompressor`, `ExifTool` `Pentax.pm`). This covers the K10D, K20D
//!   and K2000/K-m bodies. Because the entropy segment is NOT a marker-based
//!   T.81 stream and the predictor (two columns left / two rows up, zero
//!   initial predictors) is not one of the T.81 predictors 1-7, the shared
//!   [`lossless_jpeg`] decoder — including its `decode_with_external_tables`
//!   entry point, which still requires a marker-framed SOI/SOF3 stream —
//!   structurally cannot decode this storage; the entropy decoder is
//!   implemented locally in this file. There is NO linearization curve for
//!   Pentax (unlike Nikon NEF 34713), so none is applied.
//!
//! Strips are supported; tiled PEF storage (never produced by Pentax bodies)
//! is rejected with a typed error.
//!
//! # Metadata mapping
//!
//! - Make/Model/Orientation come from IFD0 (tags 271/272/274).
//! - The CFA layout comes from the raw IFD's `CFARepeatPatternDim` (33421)
//!   and `CFAPattern` (33422) tags (TIFF/EP); it must resolve to a 2x2 RGB
//!   Bayer mosaic or the file is rejected.
//! - Black level: DNG `BlackLevel` (50714, with `BlackLevelRepeatDim` 50713)
//!   when present, otherwise a documented default of 0.
//! - White level: DNG `WhiteLevel` (50717) when present, otherwise the
//!   documented bit-depth maximum `(1 << bits) - 1`.
//! - White balance: `AsShotNeutral` (50728, RGB plane order assumed) when
//!   present, normalized to green; otherwise a documented neutral
//!   `[1, 1, 1, 1]` fallback. Pentax stores the real multipliers in the
//!   makernote, which this backend deliberately does not parse yet.
//! - Color: `ColorMatrix1` (50721) or `ColorMatrix2` (50722) verbatim when
//!   present (no illuminant adaptation — unlike the DNG backend), otherwise
//!   an identity matrix. Real PEFs carry no DNG color matrices, so the
//!   identity fallback is the common path; this is recipe-relevant and is
//!   flagged in the Stage 2 report.
//! - Geometry: `ActiveArea` (50829) when present; `crop_area` is left `None`
//!   (PEF has no standard crop tags; cropping is a makernote concern).
//!
//! # Test caveat
//!
//! The unit tests build synthetic minimal TIFF/PEF headers and pixel streams
//! in memory. They are valid parser/decoder tests but are NOT proof of
//! compatibility with real camera-produced PEF files; no licensed camera
//! files are committed.

use rrrah_core::{CfaColor, CfaPattern, LevelGrid, Orientation, Rect, WhiteLevel};

use super::{
    CameraDirectory, CameraFile, CameraMetadata, CameraQuirks, camera_error, optional_ascii, optional_scalar,
    orientation_from_tag, required_scalar, tags,
};
use crate::DecodeError;
use crate::dng::{lossless_jpeg, tiff::ByteOrder, uncompressed::decode_msb_packed};

/// DNG tag: `BlackLevelRepeatDim` (two SHORTs: rows, columns).
const TAG_BLACK_LEVEL_REPEAT_DIM: u16 = 50_713;
/// DNG tag: `BlackLevel`.
const TAG_BLACK_LEVEL: u16 = 50_714;
/// DNG tag: `WhiteLevel`.
const TAG_WHITE_LEVEL: u16 = 50_717;
/// DNG tag: `ColorMatrix1`.
const TAG_COLOR_MATRIX_1: u16 = 50_721;
/// DNG tag: `ColorMatrix2`.
const TAG_COLOR_MATRIX_2: u16 = 50_722;
/// DNG tag: `AsShotNeutral`.
const TAG_AS_SHOT_NEUTRAL: u16 = 50_728;
/// DNG tag: `ActiveArea` (four values: top, left, bottom, right).
const TAG_ACTIVE_AREA: u16 = 50_829;

/// Pentax-specific lossless JPEG with the Huffman table in the makernote.
const COMPRESSION_PENTAX_JPEG: u64 = 65_535;

/// Exif tag in IFD0 carrying the Pentax makernote.
const TAG_MAKERNOTE: u16 = 0x927C;
/// Pentax makernote tag holding the proprietary Huffman table (`ExifTool`
/// `Pentax.pm` tag 0x0220 `HuffmanTable?`, found in K10D/K20D/K2000 PEFs).
const MAKERNOTE_TAG_HUFFMAN_TABLE: u16 = 0x0220;
/// Length of the `AOC\0` + 2 signature bytes prefix Pentax writes before the
/// makernote IFD (dcraw `parse_makernote` skips 6 bytes for `AOC`).
const MAKERNOTE_AOC_PREFIX_LEN: usize = 6;
/// Sanity bound on the makernote IFD entry count (dcraw uses 1000).
const MAX_MAKERNOTE_ENTRIES: u16 = 1_000;

/// Registered PEF quirks implementing the Stage 2 contract.
#[derive(Debug)]
pub(crate) struct PefQuirks;

impl CameraQuirks for PefQuirks {
    fn format_name(&self) -> &'static str {
        "PEF"
    }

    fn read_metadata(
        &self,
        container: &CameraFile<'_>,
        raw: &CameraDirectory<'_>,
    ) -> Result<CameraMetadata, DecodeError> {
        let format = self.format_name();
        let ifd0 = container
            .directories()
            .iter()
            .find(|directory| directory.is_top_level());

        let ascii = |tag: u16| -> Result<Option<String>, DecodeError> {
            if let Some(ifd0) = ifd0
                && let Some(value) = optional_ascii(format, ifd0, tag)?
            {
                return Ok(Some(value));
            }
            optional_ascii(format, raw, tag)
        };
        let make = ascii(tags::MAKE)?.unwrap_or_default();
        let model = ascii(tags::MODEL)?.unwrap_or_default();

        let orientation = match ifd0 {
            Some(ifd0) => match optional_scalar(format, ifd0, tags::ORIENTATION)? {
                Some(value) => orientation_from_tag(format, value)?,
                None => Orientation::Normal,
            },
            None => Orientation::Normal,
        };

        let width = u32::try_from(required_scalar(format, raw, tags::IMAGE_WIDTH)?)
            .map_err(|_| camera_error(format, "image width does not fit u32"))?;
        let height = u32::try_from(required_scalar(format, raw, tags::IMAGE_LENGTH)?)
            .map_err(|_| camera_error(format, "image height does not fit u32"))?;
        if width == 0 || height == 0 {
            return Err(camera_error(format, format!("empty raw image {width}x{height}")));
        }
        let bits_u64 = required_scalar(format, raw, tags::BITS_PER_SAMPLE)?;
        if !(1..=16).contains(&bits_u64) {
            return Err(camera_error(
                format,
                format!("bits per sample {bits_u64} is outside the supported 1..=16 range"),
            ));
        }
        let bits_per_sample =
            u8::try_from(bits_u64).map_err(|_| camera_error(format, "bits per sample does not fit u8"))?;

        Ok(CameraMetadata {
            make,
            model,
            width,
            height,
            bits_per_sample,
            cfa: read_cfa(format, raw)?,
            black_level: read_black_level(format, raw)?,
            white_level: read_white_level(format, raw, bits_per_sample)?,
            white_balance: read_white_balance(format, raw)?,
            xyz_to_camera: read_color_matrix(format, raw)?,
            active_area: read_active_area(format, raw)?,
            crop_area: None,
            orientation,
        })
    }

    fn decode_pixels(
        &self,
        container: &CameraFile<'_>,
        raw: &CameraDirectory<'_>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<u16>, DecodeError> {
        let format = self.format_name();
        let plan = StripPlan::read(format, raw)?;

        let total = plan
            .width
            .checked_mul(plan.height)
            .ok_or_else(|| camera_error(format, "arithmetic overflow computing sample count"))?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(total)
            .map_err(|_| camera_error(format, format!("could not allocate {total} PEF samples")))?;
        pixels.resize(total, 0);

        let compression =
            optional_scalar(format, raw, tags::COMPRESSION)?.unwrap_or(tags::COMPRESSION_UNCOMPRESSED);
        match compression {
            c if c == tags::COMPRESSION_UNCOMPRESSED => {
                decode_uncompressed_strips(format, container, &plan, &mut pixels, cancelled)?;
            }
            c if c == tags::COMPRESSION_LOSSLESS_JPEG => {
                decode_jpeg_strips(format, container, &plan, &mut pixels, cancelled)?;
            }
            COMPRESSION_PENTAX_JPEG => {
                decode_pentax_jpeg(format, container, &plan, &mut pixels, cancelled)?;
            }
            other => {
                return Err(camera_error(
                    format,
                    format!("unsupported PEF compression {other}"),
                ));
            }
        }
        Ok(pixels)
    }
}

/// Validated strip geometry and storage locations for one PEF raw plane.
struct StripPlan {
    width: usize,
    height: usize,
    rows_per_strip: usize,
    bits: u8,
    offsets: Vec<u64>,
    byte_counts: Vec<u64>,
}

impl StripPlan {
    fn read(format: &'static str, raw: &CameraDirectory<'_>) -> Result<Self, DecodeError> {
        let width_u32 = u32::try_from(required_scalar(format, raw, tags::IMAGE_WIDTH)?)
            .map_err(|_| camera_error(format, "image width does not fit u32"))?;
        let height_u32 = u32::try_from(required_scalar(format, raw, tags::IMAGE_LENGTH)?)
            .map_err(|_| camera_error(format, "image height does not fit u32"))?;
        if width_u32 == 0 || height_u32 == 0 {
            return Err(camera_error(
                format,
                format!("empty raw image {width_u32}x{height_u32}"),
            ));
        }
        let width =
            usize::try_from(width_u32).map_err(|_| camera_error(format, "image width does not fit usize"))?;
        let height = usize::try_from(height_u32)
            .map_err(|_| camera_error(format, "image height does not fit usize"))?;

        let bits_u64 = required_scalar(format, raw, tags::BITS_PER_SAMPLE)?;
        let bits = u8::try_from(bits_u64)
            .map_err(|_| camera_error(format, format!("bits per sample {bits_u64} does not fit u8")))?;

        let samples_per_pixel = optional_scalar(format, raw, tags::SAMPLES_PER_PIXEL)?.unwrap_or(1);
        if samples_per_pixel != 1 {
            return Err(camera_error(
                format,
                format!("samples per pixel {samples_per_pixel} is not the single CFA plane PEF stores"),
            ));
        }
        if raw.entry(format, tags::TILE_OFFSETS)?.is_some() {
            return Err(camera_error(
                format,
                "tiled PEF raw storage is not supported (Pentax bodies write strips)",
            ));
        }

        let offsets = required_values(format, raw, tags::STRIP_OFFSETS)?;
        let byte_counts = required_values(format, raw, tags::STRIP_BYTE_COUNTS)?;
        if offsets.len() != byte_counts.len() {
            return Err(camera_error(
                format,
                format!(
                    "strip offsets ({}) and byte counts ({}) disagree",
                    offsets.len(),
                    byte_counts.len()
                ),
            ));
        }
        let rows_per_strip_u64 =
            optional_scalar(format, raw, tags::ROWS_PER_STRIP)?.unwrap_or(u64::from(height_u32));
        let rows_per_strip = usize::try_from(rows_per_strip_u64)
            .map_err(|_| camera_error(format, "rows per strip does not fit usize"))?;
        if rows_per_strip == 0 {
            return Err(camera_error(format, "rows per strip is zero"));
        }
        let expected_strips = height.div_ceil(rows_per_strip);
        if offsets.len() != expected_strips {
            return Err(camera_error(
                format,
                format!(
                    "PEF has {} strips, expected {expected_strips} for {height} rows at {rows_per_strip} rows/strip",
                    offsets.len()
                ),
            ));
        }
        Ok(Self {
            width,
            height,
            rows_per_strip,
            bits,
            offsets,
            byte_counts,
        })
    }

    /// Rows covered by strip `index` and its first row in the output plane.
    fn strip_rows(&self, format: &'static str, index: usize) -> Result<(usize, usize), DecodeError> {
        let first_row = index
            .checked_mul(self.rows_per_strip)
            .ok_or_else(|| camera_error(format, "arithmetic overflow computing strip first row"))?;
        let rows = self.rows_per_strip.min(self.height.saturating_sub(first_row));
        Ok((first_row, rows))
    }

    /// Bounds-checked slice of strip `index`'s bytes.
    fn strip_bytes<'a>(
        &self,
        format: &'static str,
        data: &'a [u8],
        index: usize,
    ) -> Result<&'a [u8], DecodeError> {
        let start = usize::try_from(self.offsets[index])
            .map_err(|_| camera_error(format, format!("strip {index} offset does not fit usize")))?;
        let length = usize::try_from(self.byte_counts[index])
            .map_err(|_| camera_error(format, format!("strip {index} byte count does not fit usize")))?;
        let end = start.checked_add(length).ok_or_else(|| {
            camera_error(format, format!("arithmetic overflow computing strip {index} end"))
        })?;
        data.get(start..end).ok_or_else(|| {
            camera_error(
                format,
                format!(
                    "strip {index} spans {start}..{end}, beyond the {}-byte file",
                    data.len()
                ),
            )
        })
    }
}

/// Reads a required multi-value unsigned tag from a directory.
fn required_values(
    format: &'static str,
    directory: &CameraDirectory<'_>,
    tag: u16,
) -> Result<Vec<u64>, DecodeError> {
    let entry = directory
        .entry(format, tag)?
        .ok_or_else(|| camera_error(format, format!("required tag {tag} is missing")))?;
    entry
        .unsigned_values()
        .map_err(|error| camera_error(format, format!("tag {tag}: {error}")))
}

/// Uncompressed storage: MSB-first packed rows (12/14-bit) or plain 16-bit
/// words in the container byte order.
fn decode_uncompressed_strips(
    format: &'static str,
    container: &CameraFile<'_>,
    plan: &StripPlan,
    pixels: &mut [u16],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), DecodeError> {
    let bits = plan.bits;
    if !matches!(bits, 12 | 14 | 16) {
        return Err(camera_error(
            format,
            format!("uncompressed PEF bit depth {bits} is not supported (expected 12, 14, or 16)"),
        ));
    }
    let row_bits = plan
        .width
        .checked_mul(usize::from(bits))
        .ok_or_else(|| camera_error(format, "arithmetic overflow computing packed row bits"))?;
    let row_bytes = row_bits
        .checked_add(7)
        .map(|bits| bits / 8)
        .ok_or_else(|| camera_error(format, "arithmetic overflow computing packed row bytes"))?;

    let data = container.data();
    let byte_order = container.byte_order();
    for index in 0..plan.offsets.len() {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let (first_row, rows) = plan.strip_rows(format, index)?;
        let expected = row_bytes
            .checked_mul(rows)
            .ok_or_else(|| camera_error(format, "arithmetic overflow computing strip byte length"))?;
        let bytes = plan.strip_bytes(format, data, index)?;
        if bytes.len() != expected {
            return Err(camera_error(
                format,
                format!(
                    "strip {index} has {} bytes, expected {expected} for {rows} packed {bits}-bit rows",
                    bytes.len()
                ),
            ));
        }
        for row in 0..rows {
            if cancelled() {
                return Err(DecodeError::Cancelled);
            }
            let source_start = row
                .checked_mul(row_bytes)
                .ok_or_else(|| camera_error(format, "arithmetic overflow computing row offset"))?;
            let target_start = first_row
                .checked_add(row)
                .and_then(|absolute| absolute.checked_mul(plan.width))
                .ok_or_else(|| camera_error(format, "arithmetic overflow computing output row"))?;
            let source = &bytes[source_start..source_start + row_bytes];
            let target = &mut pixels[target_start..target_start + plan.width];
            match bits {
                16 => {
                    for (sample, word) in target.iter_mut().zip(source.chunks_exact(2)) {
                        *sample = byte_order.u16(word);
                    }
                }
                _ => decode_msb_packed(source, target, bits).map_err(|error| {
                    camera_error(format, format!("packed strip {index} row {row}: {error}"))
                })?,
            }
        }
    }
    Ok(())
}

/// Lossless-JPEG storage (TIFF/EP compression 7): each strip is an
/// independent SOF3 stream covering `rows_per_strip` full-width rows.
fn decode_jpeg_strips(
    format: &'static str,
    container: &CameraFile<'_>,
    plan: &StripPlan,
    pixels: &mut [u16],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), DecodeError> {
    let data = container.data();
    for index in 0..plan.offsets.len() {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let (first_row, rows) = plan.strip_rows(format, index)?;
        let bytes = plan.strip_bytes(format, data, index)?;
        let decoded = match lossless_jpeg::decode(bytes, cancelled) {
            Ok(decoded) => decoded,
            Err(lossless_jpeg::LosslessJpegError::Cancelled { .. }) => {
                return Err(DecodeError::Cancelled);
            }
            Err(error) => {
                return Err(camera_error(
                    format,
                    format!("lossless JPEG strip {index}: {error}"),
                ));
            }
        };
        if decoded.component_ids.len() != 1 {
            return Err(camera_error(
                format,
                format!(
                    "lossless JPEG strip {index} has {} components, expected the single CFA plane",
                    decoded.component_ids.len()
                ),
            ));
        }
        if decoded.precision != plan.bits {
            return Err(camera_error(
                format,
                format!(
                    "lossless JPEG strip {index} precision {} does not match BitsPerSample {}",
                    decoded.precision, plan.bits
                ),
            ));
        }
        let expected = plan
            .width
            .checked_mul(rows)
            .ok_or_else(|| camera_error(format, "arithmetic overflow computing strip sample count"))?;
        if decoded.samples.len() != expected {
            return Err(camera_error(
                format,
                format!(
                    "lossless JPEG strip {index} decoded {} samples, expected {expected} ({}x{rows})",
                    decoded.samples.len(),
                    decoded.width
                ),
            ));
        }
        let target_start = first_row
            .checked_mul(plan.width)
            .ok_or_else(|| camera_error(format, "arithmetic overflow computing output row"))?;
        pixels[target_start..target_start + expected].copy_from_slice(&decoded.samples);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Pentax compression 65535 (makernote Huffman table + bare bitstream)
// ---------------------------------------------------------------------------
//
// On-disk layout research (clean-room reading of dcraw `pentax_load_raw` and
// `parse_makernote`, rawspeed `PefDecoder`/`PentaxDecompressor`, and the
// ExifTool `Pentax.pm`/`MakerNotes.pm` tag tables):
//
// * The makernote is tag 0x927C of IFD0. Pentax PEF makernotes start with
//   `AOC\0` followed by two signature bytes (real files show `AOC MM` per the
//   archiveteam format wiki); a plain IFD without the prefix also occurs.
//   dcraw ignores the signature bytes and parses the makernote IFD in the
//   CONTAINER byte order; ExifTool likewise treats the makernote byte order
//   as "Unknown" (auto-detected) because it is unreliable. We therefore parse
//   with the container byte order first and fall back to the opposite order
//   only when the container-order parse is structurally invalid.
// * Makernote value offsets are absolute file offsets (dcraw `tiff_get`
//   seeks to `get4() + base` with base = TIFF start).
// * The Huffman table is makernote tag 0x0220, field type UNDEFINED (7).
//   Layout (all u16 in container byte order):
//     - bytes 0..2: depth sentinel; entry count is `(raw + 12) & 15`
//       (rawspeed additionally rejects `raw + 12 > 15`);
//     - bytes 2..14: reserved, skipped by dcraw;
//     - then `depth` u16 code-window values and `depth` u8 code lengths.
//   Entry `i` encodes difference category `i` (the table index IS the T.81
//   SSSS category) with the code given by the top `len` bits of the 12-bit
//   window value; `len` must be 1..=12 (rawspeed validation).
// * The strip payload is a bare MSB-first bitstream: no JPEG markers, no
//   0xFF00 byte stuffing, no restart markers. rawspeed feeds the whole strip
//   to a `BitStreamerMSB`; dcraw seeks to the strip offset and reads bits.
// * Prediction is NOT a T.81 predictor: for columns 0-1 the predictor is the
//   sample two rows up (rows 0-1 predict 0), for columns >= 2 it is the
//   sample two columns left. Difference decoding is standard T.81
//   receive/extend on the category. dcraw rejects samples that do not fit
//   `BitsPerSample` (`hpred >> tiff_bps` -> `derror`).
// * rawspeed requires a single strip and an even width; Pentax bodies write
//   exactly that for this compression. rawspeed also ships a built-in
//   "legacy" table for files lacking the makernote tag; we instead reject
//   with a typed error (like dcraw, which has no fallback) so a wrong table
//   can never silently produce garbage pixels.
// * There is no linearization curve for Pentax (unlike Nikon NEF 34713).

/// Direct-lookup Pentax Huffman table: 12-bit window -> (length << 8) |
/// category, 0 marks an unassigned window.
struct PentaxHuffman {
    window: [u16; 4_096],
}

impl PentaxHuffman {
    /// Parses the proprietary makernote table (tag 0x0220 payload).
    fn parse(format: &'static str, order: ByteOrder, bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < 14 {
            return Err(camera_error(
                format,
                format!(
                    "Pentax makernote Huffman table is {} bytes, need at least 14 (sentinel + reserved)",
                    bytes.len()
                ),
            ));
        }
        let sentinel = order.u16(&bytes[0..2]);
        let depth = usize::from(sentinel.wrapping_add(12) & 15);
        if depth == 0 {
            return Err(camera_error(
                format,
                format!("Pentax makernote Huffman table declares zero codes (sentinel {sentinel:#06x})"),
            ));
        }
        let needed = 14_usize
            .checked_add(depth.checked_mul(3).ok_or_else(|| {
                camera_error(format, "arithmetic overflow computing Pentax Huffman table size")
            })?)
            .ok_or_else(|| camera_error(format, "arithmetic overflow computing Pentax Huffman table size"))?;
        if bytes.len() < needed {
            return Err(camera_error(
                format,
                format!(
                    "Pentax makernote Huffman table declares {depth} codes (needs {needed} bytes), only {} present",
                    bytes.len()
                ),
            ));
        }
        let code_base = 14_usize;
        let length_base = code_base
            .checked_add(depth.checked_mul(2).ok_or_else(|| {
                camera_error(
                    format,
                    "arithmetic overflow computing Pentax Huffman table layout",
                )
            })?)
            .ok_or_else(|| {
                camera_error(
                    format,
                    "arithmetic overflow computing Pentax Huffman table layout",
                )
            })?;

        let mut window = [0_u16; 4_096];
        for index in 0..depth {
            let raw_code = order.u16(&bytes[code_base + index * 2..code_base + index * 2 + 2]);
            if raw_code > 4_095 {
                return Err(camera_error(
                    format,
                    format!("Pentax Huffman code {index} window value {raw_code:#06x} exceeds 12 bits"),
                ));
            }
            let length = bytes[length_base + index];
            if !(1..=12).contains(&length) {
                return Err(camera_error(
                    format,
                    format!("Pentax Huffman code {index} has invalid code length {length} (expected 1..=12)"),
                ));
            }
            let shift = 12_u8 - length;
            let code = raw_code >> shift;
            let first = usize::from(code) << shift;
            let span = 1_usize << shift;
            for slot in &mut window[first..first + span] {
                if *slot != 0 {
                    return Err(camera_error(
                        format,
                        format!("Pentax Huffman code {index} overlaps an earlier code (ambiguous table)"),
                    ));
                }
                *slot = (u16::from(length) << 8) | u16::try_from(index).expect("depth <= 15");
            }
        }
        Ok(Self { window })
    }

    /// Decodes one difference category from the bitstream.
    fn symbol(&self, format: &'static str, bits: &mut MsbBits<'_>) -> Result<u8, DecodeError> {
        let (window, valid) = bits.peek_window();
        let packed = self.window[usize::from(window)];
        let length =
            u8::try_from(packed >> 8).map_err(|_| camera_error(format, "invalid Pentax Huffman slot"))?;
        if length == 0 {
            return Err(camera_error(
                format,
                "Pentax Huffman bitstream hit an unassigned code (corrupt strip or wrong table)",
            ));
        }
        if length > valid {
            return Err(camera_error(
                format,
                format!(
                    "Pentax Huffman code needs {length} bits, only {valid} left in the strip (truncated)"
                ),
            ));
        }
        bits.take(length);
        u8::try_from(packed & 0xff).map_err(|_| camera_error(format, "invalid Pentax Huffman slot"))
    }
}

/// MSB-first bit reader over a bare entropy segment (no markers, no 0xFF00
/// byte stuffing — the Pentax strip payload is consumed whole).
struct MsbBits<'a> {
    bytes: &'a [u8],
    bit_len: usize,
    bit_pos: usize,
}

impl<'a> MsbBits<'a> {
    fn new(format: &'static str, bytes: &'a [u8]) -> Result<Self, DecodeError> {
        let bit_len = bytes
            .len()
            .checked_mul(8)
            .ok_or_else(|| camera_error(format, "arithmetic overflow computing strip bit length"))?;
        Ok(Self {
            bytes,
            bit_len,
            bit_pos: 0,
        })
    }

    fn bit_at(&self, index: usize) -> u16 {
        u16::from((self.bytes[index / 8] >> (7 - (index % 8))) & 1)
    }

    /// Next up-to-12 bits left-aligned in a 12-bit window, plus how many are
    /// actually present in the stream.
    fn peek_window(&self) -> (u16, u8) {
        let remaining = self.bit_len - self.bit_pos;
        let valid = u8::try_from(remaining.min(12)).unwrap_or(12);
        let mut window = 0_u16;
        for offset in 0..usize::from(valid) {
            window |= self.bit_at(self.bit_pos + offset) << (11 - offset);
        }
        (window, valid)
    }

    /// Consumes `count` bits; callers guarantee `count` bits remain.
    fn take(&mut self, count: u8) {
        self.bit_pos += usize::from(count);
    }

    /// T.81 receive: reads `count` raw bits.
    fn read_bits(&mut self, format: &'static str, count: u8) -> Result<u32, DecodeError> {
        let remaining = self.bit_len - self.bit_pos;
        if remaining < usize::from(count) {
            return Err(camera_error(
                format,
                format!("Pentax strip is truncated: need {count} bits, only {remaining} remain"),
            ));
        }
        let mut value = 0_u32;
        for _ in 0..count {
            value = (value << 1) | u32::from(self.bit_at(self.bit_pos));
            self.bit_pos += 1;
        }
        Ok(value)
    }

    /// T.81 extend: maps a received `count`-bit value to a signed difference.
    fn read_difference(&mut self, format: &'static str, category: u8) -> Result<i32, DecodeError> {
        if category == 0 {
            return Ok(0);
        }
        let received = self.read_bits(format, category)?;
        let half = 1_u32 << (category - 1);
        if received < half {
            // Negative range: received in [0, 2^(s-1)) maps below zero.
            i32::try_from(received)
                .map(|value| value - (1_i32 << category) + 1)
                .map_err(|_| camera_error(format, "arithmetic overflow decoding Pentax difference"))
        } else {
            i32::try_from(received)
                .map_err(|_| camera_error(format, "arithmetic overflow decoding Pentax difference"))
        }
    }
}

/// One parsed makernote directory entry (12-byte record, raw value field).
struct MakernoteEntry {
    tag: u16,
    field_type: u16,
    count: u32,
    value_field: [u8; 4],
}

/// Parses the Pentax makernote IFD with `order`; returns `None` when the
/// structure is invalid for that byte order (caller retries the other one).
fn parse_makernote_entries(payload: &[u8], order: ByteOrder) -> Option<Vec<MakernoteEntry>> {
    let base = if payload.len() >= MAKERNOTE_AOC_PREFIX_LEN && payload[..4] == *b"AOC\0" {
        MAKERNOTE_AOC_PREFIX_LEN
    } else {
        0
    };
    let count = usize::from(order.u16(payload.get(base..base + 2)?));
    if count > usize::from(MAX_MAKERNOTE_ENTRIES) {
        return None;
    }
    let table_end = base.checked_add(2)?.checked_add(count.checked_mul(12)?)?;
    if table_end > payload.len() {
        return None;
    }
    let mut entries = Vec::with_capacity(count);
    for index in 0..count {
        let at = base + 2 + index * 12;
        let record = payload.get(at..at + 12)?;
        entries.push(MakernoteEntry {
            tag: order.u16(&record[0..2]),
            field_type: order.u16(&record[2..4]),
            count: order.u32(&record[4..8]),
            value_field: record[8..12].try_into().ok()?,
        });
    }
    Some(entries)
}

/// Locates the Huffman table payload (makernote tag 0x0220) in the file.
/// Inline values (<= 4 bytes) are copied out; out-of-line values are sliced
/// from the file (their offsets are absolute from the TIFF start).
fn read_makernote_huffman_bytes<'a>(
    format: &'static str,
    container: &CameraFile<'a>,
) -> Result<std::borrow::Cow<'a, [u8]>, DecodeError> {
    let ifd0 = container
        .directories()
        .iter()
        .find(|directory| directory.is_top_level())
        .ok_or_else(|| camera_error(format, "PEF has no top-level directory to hold the makernote"))?;
    let makernote = ifd0.entry(format, TAG_MAKERNOTE)?.ok_or_else(|| {
        camera_error(
            format,
            "compression 65535 requires the Pentax makernote (IFD0 tag 37500) carrying the Huffman table",
        )
    })?;
    let payload = makernote.raw_bytes();
    let container_order = container.byte_order();
    let other_order = match container_order {
        ByteOrder::Little => ByteOrder::Big,
        ByteOrder::Big => ByteOrder::Little,
    };
    // dcraw parses the makernote IFD in container byte order (the `AOC\0MM`
    // signature bytes are decorative); fall back to the opposite order only
    // when the container-order parse is structurally invalid.
    let (entries, order) = match parse_makernote_entries(payload, container_order) {
        Some(entries) => (entries, container_order),
        None => match parse_makernote_entries(payload, other_order) {
            Some(entries) => (entries, other_order),
            None => {
                return Err(camera_error(
                    format,
                    "Pentax makernote (tag 37500) is not a parseable IFD in either byte order",
                ));
            }
        },
    };
    let Some(entry) = entries
        .iter()
        .find(|entry| entry.tag == MAKERNOTE_TAG_HUFFMAN_TABLE)
    else {
        return Err(camera_error(
            format,
            "Pentax makernote has no Huffman table (tag 0x0220); cannot decode compression 65535 \
             (rawspeed's built-in legacy table fallback is deliberately not used)",
        ));
    };
    if entry.field_type != 7 {
        return Err(camera_error(
            format,
            format!(
                "Pentax makernote Huffman table (tag 0x0220) has field type {}, expected UNDEFINED (7)",
                entry.field_type
            ),
        ));
    }
    let count = usize::try_from(entry.count)
        .map_err(|_| camera_error(format, "Pentax Huffman table byte count does not fit usize"))?;
    if count <= 4 {
        // Inline value: the first `count` bytes of the 4-byte value field
        // (UNDEFINED payload has no byte-order semantics). Always too short
        // for a real table; `PentaxHuffman::parse` produces the typed error.
        return entry
            .value_field
            .get(..count)
            .map(|bytes| std::borrow::Cow::Owned(bytes.to_vec()))
            .ok_or_else(|| camera_error(format, "invalid inline Pentax Huffman table value"));
    }
    // Out-of-line value: offset is absolute from the TIFF start (dcraw base).
    let start = usize::try_from(order.u32(&entry.value_field))
        .map_err(|_| camera_error(format, "Pentax Huffman table offset does not fit usize"))?;
    let end = start
        .checked_add(count)
        .ok_or_else(|| camera_error(format, "arithmetic overflow computing Pentax Huffman table end"))?;
    container
        .data()
        .get(start..end)
        .map(std::borrow::Cow::Borrowed)
        .ok_or_else(|| {
            camera_error(
                format,
                format!(
                    "Pentax Huffman table spans {start}..{end}, beyond the {}-byte file",
                    container.data().len()
                ),
            )
        })
}

/// Pentax compression 65535: decodes the whole CFA plane from a single bare
/// Huffman bitstream using the makernote table. No linearization curve.
fn decode_pentax_jpeg(
    format: &'static str,
    container: &CameraFile<'_>,
    plan: &StripPlan,
    pixels: &mut [u16],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), DecodeError> {
    if plan.offsets.len() != 1 {
        return Err(camera_error(
            format,
            format!(
                "compression 65535 stores the raw plane as one Huffman-coded strip, found {} \
                 (multi-strip Pentax streams are not produced by Pentax bodies)",
                plan.offsets.len()
            ),
        ));
    }
    if !plan.width.is_multiple_of(2) {
        return Err(camera_error(
            format,
            format!(
                "compression 65535 uses a two-column predictor and requires an even width, got {}",
                plan.width
            ),
        ));
    }
    if !(1..=16).contains(&plan.bits) {
        return Err(camera_error(
            format,
            format!(
                "compression 65535 bit depth {} is outside the supported 1..=16 range",
                plan.bits
            ),
        ));
    }
    let max_sample = 1_u32
        .checked_shl(u32::from(plan.bits))
        .and_then(|limit| limit.checked_sub(1))
        .ok_or_else(|| camera_error(format, "arithmetic overflow computing the sample range"))?;

    let table_bytes = read_makernote_huffman_bytes(format, container)?;
    let huffman = PentaxHuffman::parse(format, container.byte_order(), &table_bytes)?;
    let bytes = plan.strip_bytes(format, container.data(), 0)?;
    let mut bits = MsbBits::new(format, bytes)?;

    for row in 0..plan.height {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        for col in 0..plan.width {
            let index = row
                .checked_mul(plan.width)
                .and_then(|row_base| row_base.checked_add(col))
                .ok_or_else(|| camera_error(format, "arithmetic overflow computing the sample index"))?;
            let category = huffman.symbol(format, &mut bits)?;
            let difference = bits.read_difference(format, category)?;
            // Pentax predictor: two columns left, or two rows up for the
            // first two columns (zero for the first two rows).
            let predicted = if col >= 2 {
                i32::from(pixels[index - 2])
            } else if row >= 2 {
                let two_rows = 2_usize
                    .checked_mul(plan.width)
                    .ok_or_else(|| camera_error(format, "arithmetic overflow computing the predictor"))?;
                i32::from(pixels[index - two_rows])
            } else {
                0
            };
            let value = predicted + difference;
            if value < 0 || u32::try_from(value).map_or(true, |sample| sample > max_sample) {
                return Err(camera_error(
                    format,
                    format!(
                        "compression 65535 decoded sample {value} at row {row} column {col} is outside 0..={max_sample} ({}-bit)",
                        plan.bits
                    ),
                ));
            }
            pixels[index] =
                u16::try_from(value).map_err(|_| camera_error(format, "sample does not fit u16"))?;
        }
    }
    // `pixels` was pre-sized to exactly width * height by the caller and the
    // loops above cover every row/column, so the plane is fully written.
    Ok(())
}

/// Reads the TIFF/EP CFA tags and validates a 2x2 RGB Bayer mosaic.
fn read_cfa(format: &'static str, raw: &CameraDirectory<'_>) -> Result<CfaPattern, DecodeError> {
    let dims = required_values(format, raw, tags::CFA_REPEAT_PATTERN_DIM)?;
    if dims.len() != 2 {
        return Err(camera_error(
            format,
            format!("CFARepeatPatternDim has {} values, expected 2", dims.len()),
        ));
    }
    // TIFF/EP and DNG both order the repeat dimensions as [rows, columns].
    let rows =
        usize::try_from(dims[0]).map_err(|_| camera_error(format, "CFA repeat rows do not fit usize"))?;
    let columns =
        usize::try_from(dims[1]).map_err(|_| camera_error(format, "CFA repeat columns do not fit usize"))?;
    if rows == 0 || columns == 0 {
        return Err(camera_error(format, "CFA repeat dimensions are zero"));
    }
    let cell_count = rows
        .checked_mul(columns)
        .ok_or_else(|| camera_error(format, "arithmetic overflow computing CFA cell count"))?;

    let pattern = required_values(format, raw, tags::CFA_PATTERN)?;
    if pattern.len() != cell_count {
        return Err(camera_error(
            format,
            format!(
                "CFAPattern has {} cells, expected {cell_count} for a {rows}x{columns} repeat",
                pattern.len()
            ),
        ));
    }
    let mut cells = Vec::new();
    cells
        .try_reserve_exact(cell_count)
        .map_err(|_| camera_error(format, format!("could not allocate {cell_count} CFA cells")))?;
    for value in pattern {
        let color = match value {
            0 => CfaColor::Red,
            1 => CfaColor::Green,
            2 => CfaColor::Blue,
            3 => CfaColor::Cyan,
            4 => CfaColor::Magenta,
            5 => CfaColor::Yellow,
            6 => CfaColor::White,
            other => {
                return Err(camera_error(
                    format,
                    format!("CFAPattern contains unknown color {other}"),
                ));
            }
        };
        cells.push(color);
    }
    let cfa = CfaPattern {
        width: u8::try_from(columns).map_err(|_| camera_error(format, "CFA columns exceed 255"))?,
        height: u8::try_from(rows).map_err(|_| camera_error(format, "CFA rows exceed 255"))?,
        cells,
    };
    cfa.bayer_quad()
        .map_err(|_| camera_error(format, "PEF CFA pattern is not a 2x2 RGB Bayer mosaic"))?;
    Ok(cfa)
}

/// Reads DNG `BlackLevel`/`BlackLevelRepeatDim`; defaults to a 1x1 grid of 0.
fn read_black_level(format: &'static str, raw: &CameraDirectory<'_>) -> Result<LevelGrid, DecodeError> {
    let Some(entry) = raw.entry(format, TAG_BLACK_LEVEL)? else {
        return Ok(LevelGrid {
            width: 1,
            height: 1,
            components: 1,
            values: vec![0.0],
        });
    };
    let values = entry
        .numeric_values()
        .map_err(|error| camera_error(format, format!("tag {TAG_BLACK_LEVEL}: {error}")))?;
    if values.is_empty() {
        return Err(camera_error(format, "BlackLevel is empty"));
    }
    let (rows, columns) = match raw.entry(format, TAG_BLACK_LEVEL_REPEAT_DIM)? {
        Some(dims) => {
            let dims = dims.unsigned_values().map_err(|error| {
                camera_error(format, format!("tag {TAG_BLACK_LEVEL_REPEAT_DIM}: {error}"))
            })?;
            if dims.len() != 2 {
                return Err(camera_error(
                    format,
                    format!("BlackLevelRepeatDim has {} values, expected 2", dims.len()),
                ));
            }
            (dims[0], dims[1])
        }
        None => (1, 1),
    };
    let expected = rows
        .checked_mul(columns)
        .ok_or_else(|| camera_error(format, "arithmetic overflow computing black-level grid"))?;
    if expected != values.len() as u64 {
        return Err(camera_error(
            format,
            format!(
                "BlackLevel has {} values, expected {expected} for a {rows}x{columns} repeat grid",
                values.len()
            ),
        ));
    }
    let mut converted = Vec::new();
    converted
        .try_reserve_exact(values.len())
        .map_err(|_| camera_error(format, "could not allocate black-level grid"))?;
    for value in values {
        #[allow(clippy::cast_possible_truncation)]
        let value = value as f32;
        if !value.is_finite() {
            return Err(camera_error(format, "BlackLevel is outside finite f32 range"));
        }
        converted.push(value);
    }
    Ok(LevelGrid {
        width: u8::try_from(columns)
            .map_err(|_| camera_error(format, "black-level grid columns exceed 255"))?,
        height: u8::try_from(rows).map_err(|_| camera_error(format, "black-level grid rows exceed 255"))?,
        components: 1,
        values: converted,
    })
}

/// Reads DNG `WhiteLevel`; defaults to the bit-depth maximum.
fn read_white_level(
    format: &'static str,
    raw: &CameraDirectory<'_>,
    bits: u8,
) -> Result<WhiteLevel, DecodeError> {
    let Some(entry) = raw.entry(format, TAG_WHITE_LEVEL)? else {
        // Documented default: full-scale for the stored bit depth.
        let maximum = (1_u64 << bits) - 1;
        #[allow(clippy::cast_precision_loss)]
        return Ok(WhiteLevel(vec![maximum as f32]));
    };
    let values = entry
        .unsigned_values()
        .map_err(|error| camera_error(format, format!("tag {TAG_WHITE_LEVEL}: {error}")))?;
    if values.is_empty() {
        return Err(camera_error(format, "WhiteLevel is empty"));
    }
    #[allow(clippy::cast_precision_loss)]
    Ok(WhiteLevel(values.into_iter().map(|value| value as f32).collect()))
}

/// Reads `AsShotNeutral` (RGB plane order assumed) normalized to green;
/// defaults to a documented neutral `[1, 1, 1, 1]`.
fn read_white_balance(format: &'static str, raw: &CameraDirectory<'_>) -> Result<[f32; 4], DecodeError> {
    let Some(entry) = raw.entry(format, TAG_AS_SHOT_NEUTRAL)? else {
        return Ok([1.0; 4]);
    };
    let neutral = entry
        .numeric_values()
        .map_err(|error| camera_error(format, format!("tag {TAG_AS_SHOT_NEUTRAL}: {error}")))?;
    if neutral.len() != 3 && neutral.len() != 4 {
        return Err(camera_error(
            format,
            format!("AsShotNeutral has {} values, expected 3 or 4", neutral.len()),
        ));
    }
    let mut gains = [0.0_f32; 3];
    for (gain, value) in gains.iter_mut().zip(&neutral) {
        if *value <= 0.0 {
            return Err(camera_error(
                format,
                "AsShotNeutral contains a non-positive value",
            ));
        }
        #[allow(clippy::cast_possible_truncation)]
        let converted = (1.0 / value) as f32;
        if !converted.is_finite() {
            return Err(camera_error(
                format,
                "AsShotNeutral gain is outside finite f32 range",
            ));
        }
        *gain = converted;
    }
    let green = gains[1];
    Ok([gains[0] / green, 1.0, gains[2] / green, 1.0])
}

/// Reads `ColorMatrix1`/`ColorMatrix2` verbatim when present; otherwise the
/// identity matrix. No illuminant adaptation (unlike the DNG backend).
fn read_color_matrix(format: &'static str, raw: &CameraDirectory<'_>) -> Result<[[f32; 3]; 4], DecodeError> {
    const IDENTITY: [[f32; 3]; 4] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0]];
    for tag in [TAG_COLOR_MATRIX_1, TAG_COLOR_MATRIX_2] {
        let Some(entry) = raw.entry(format, tag)? else {
            continue;
        };
        let values = entry
            .numeric_values()
            .map_err(|error| camera_error(format, format!("tag {tag}: {error}")))?;
        if values.len() != 9 {
            return Err(camera_error(
                format,
                format!("color matrix tag {tag} has {} values, expected 9", values.len()),
            ));
        }
        let mut matrix = [[0.0_f32; 3]; 4];
        for (row, chunk) in values.chunks_exact(3).enumerate() {
            for (column, value) in chunk.iter().enumerate() {
                #[allow(clippy::cast_possible_truncation)]
                let converted = *value as f32;
                if !converted.is_finite() {
                    return Err(camera_error(format, "color matrix is outside finite f32 range"));
                }
                matrix[row][column] = converted;
            }
        }
        return Ok(matrix);
    }
    Ok(IDENTITY)
}

/// Reads DNG `ActiveArea` (top, left, bottom, right) when present.
fn read_active_area(format: &'static str, raw: &CameraDirectory<'_>) -> Result<Option<Rect>, DecodeError> {
    let Some(entry) = raw.entry(format, TAG_ACTIVE_AREA)? else {
        return Ok(None);
    };
    let values = entry
        .unsigned_values()
        .map_err(|error| camera_error(format, format!("tag {TAG_ACTIVE_AREA}: {error}")))?;
    if values.len() != 4 {
        return Err(camera_error(
            format,
            format!("ActiveArea has {} values, expected 4", values.len()),
        ));
    }
    let (top, left, bottom, right) = (values[0], values[1], values[2], values[3]);
    let width = right
        .checked_sub(left)
        .ok_or_else(|| camera_error(format, "ActiveArea right edge is left of the left edge"))?;
    let height = bottom
        .checked_sub(top)
        .ok_or_else(|| camera_error(format, "ActiveArea bottom edge is above the top edge"))?;
    if width == 0 || height == 0 {
        return Err(camera_error(format, "ActiveArea is empty"));
    }
    let to_u32 = |value: u64, field: &'static str| {
        u32::try_from(value).map_err(|_| camera_error(format, format!("ActiveArea {field} does not fit u32")))
    };
    Ok(Some(Rect::new(
        to_u32(left, "left")?,
        to_u32(top, "top")?,
        to_u32(width, "width")?,
        to_u32(height, "height")?,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Synthetic TIFF/PEF builder -------------------------------------
    //
    // Builds classic little-endian TIFFs in memory: an 8-byte header, IFD0
    // with Make/Model/Orientation, and a raw IFD on the next-IFD chain. All
    // bytes are synthetic; no camera files are used.

    type TestEntry = (u16, u16, u32, Vec<u8>);

    struct TiffBuilder {
        bytes: Vec<u8>,
    }

    impl TiffBuilder {
        fn new() -> Self {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(b"II");
            bytes.extend_from_slice(&42_u16.to_le_bytes());
            bytes.extend_from_slice(&8_u32.to_le_bytes());
            Self { bytes }
        }

        /// Appends an IFD with a zero next-IFD link and returns its offset.
        /// Out-of-line values (>4 bytes) are appended after the table.
        fn add_ifd(&mut self, entries: &[TestEntry]) -> usize {
            let offset = self.bytes.len();
            let mut sorted = entries.to_vec();
            sorted.sort_by_key(|entry| entry.0);
            let count = u16::try_from(sorted.len()).unwrap();
            self.bytes.extend_from_slice(&count.to_le_bytes());
            let table_start = self.bytes.len();
            self.bytes.resize(table_start + sorted.len() * 12, 0);
            self.bytes.extend_from_slice(&0_u32.to_le_bytes()); // next IFD link
            for (index, (tag, field_type, count, value)) in sorted.iter().enumerate() {
                let at = table_start + index * 12;
                self.bytes[at..at + 2].copy_from_slice(&tag.to_le_bytes());
                self.bytes[at + 2..at + 4].copy_from_slice(&field_type.to_le_bytes());
                self.bytes[at + 4..at + 8].copy_from_slice(&count.to_le_bytes());
                if value.len() <= 4 {
                    self.bytes[at + 8..at + 8 + value.len()].copy_from_slice(value);
                } else {
                    let value_offset = u32::try_from(self.bytes.len()).unwrap();
                    self.bytes.extend_from_slice(value);
                    self.bytes[at + 8..at + 12].copy_from_slice(&value_offset.to_le_bytes());
                }
            }
            offset
        }

        /// Absolute offset of the value field of `tag` inside the IFD at
        /// `ifd_offset` (used to patch strip offsets after appending data).
        fn entry_value_offset(&self, ifd_offset: usize, tag: u16) -> usize {
            let count = u16::from_le_bytes(self.bytes[ifd_offset..ifd_offset + 2].try_into().unwrap());
            for index in 0..usize::from(count) {
                let at = ifd_offset + 2 + index * 12;
                let entry_tag = u16::from_le_bytes(self.bytes[at..at + 2].try_into().unwrap());
                if entry_tag == tag {
                    return at + 8;
                }
            }
            panic!("tag {tag} not found in synthetic IFD");
        }

        fn append_data(&mut self, data: &[u8]) -> u32 {
            let offset = u32::try_from(self.bytes.len()).unwrap();
            self.bytes.extend_from_slice(data);
            offset
        }

        fn patch_u32(&mut self, at: usize, value: u32) {
            self.bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
    }

    fn short(value: u16) -> Vec<u8> {
        value.to_le_bytes().to_vec()
    }

    fn long(value: u32) -> Vec<u8> {
        value.to_le_bytes().to_vec()
    }

    fn ascii(text: &str) -> Vec<u8> {
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(0);
        bytes
    }

    /// Builds a synthetic PEF: IFD0 with Pentax Make/Model, then a raw IFD
    /// with a single strip of `pixels`. `extra_raw` appends optional raw-IFD
    /// entries (`WhiteLevel`, `AsShotNeutral`, ...). `with_cfa` controls whether
    /// the TIFF/EP CFA tags are emitted.
    fn synthetic_pef(
        width: u32,
        height: u32,
        bits: u16,
        compression: u16,
        pixels: &[u8],
        extra_raw: &[TestEntry],
        with_cfa: bool,
    ) -> Vec<u8> {
        let mut builder = TiffBuilder::new();
        let ifd0 = builder.add_ifd(&[
            (271, 2, 7, ascii("PENTAX")),
            (272, 2, 14, ascii("PENTAX K-TEST")),
            (274, 3, 1, short(1)),
        ]);
        let mut raw_entries: Vec<TestEntry> = vec![
            (254, 4, 1, long(0)),
            (256, 4, 1, long(width)),
            (257, 4, 1, long(height)),
            (258, 3, 1, short(bits)),
            (259, 3, 1, short(compression)),
            (262, 3, 1, short(32_803)),
            (273, 4, 1, long(0)), // patched below
            (277, 3, 1, short(1)),
            (278, 4, 1, long(height)),
            (279, 4, 1, long(u32::try_from(pixels.len()).unwrap())),
        ];
        if with_cfa {
            // CFARepeatPatternDim [rows=2, columns=2] and RGGB CFAPattern.
            raw_entries.push((33_421, 3, 2, [short(2), short(2)].concat()));
            raw_entries.push((33_422, 1, 4, vec![0, 1, 1, 2]));
        }
        raw_entries.extend_from_slice(extra_raw);
        let raw_ifd = builder.add_ifd(&raw_entries);
        // Link IFD0 -> raw IFD through the next-IFD chain.
        let ifd0_entry_count = 3_usize;
        let ifd0_next_field = ifd0 + 2 + ifd0_entry_count * 12;
        builder.patch_u32(ifd0_next_field, u32::try_from(raw_ifd).unwrap());
        let strip_offset_field = builder.entry_value_offset(raw_ifd, 273);
        let pixel_offset = builder.append_data(pixels);
        builder.patch_u32(strip_offset_field, pixel_offset);
        builder.bytes
    }

    // ---- Pixel stream encoders (test-side) -------------------------------

    /// MSB-first packing mirroring `decode_msb_packed`'s contract; each row
    /// is padded to a whole byte, matching camera strip layout.
    fn encode_msb_packed(samples: &[u16], width: usize, bits: u8) -> Vec<u8> {
        assert!(width > 0 && samples.len().is_multiple_of(width));
        let row_bytes = (width * usize::from(bits)).div_ceil(8);
        let mut encoded = vec![0_u8; row_bytes * (samples.len() / width)];
        for (row_index, row) in samples.chunks_exact(width).enumerate() {
            let row_base = row_index * row_bytes;
            let mut bit = 0_usize;
            for &sample in row {
                assert!(sample < (1 << bits));
                for shift in (0..bits).rev() {
                    if (sample >> shift) & 1 == 1 {
                        encoded[row_base + bit / 8] |= 1 << (7 - (bit % 8));
                    }
                    bit += 1;
                }
            }
        }
        encoded
    }

    /// Minimal lossless-JPEG (SOF3) encoder producing streams the shared
    /// decoder accepts: one component, predictor 1 (left), point transform 0,
    /// and a Huffman table assigning sequential 4-bit codes to difference
    /// categories 0..=12.
    fn encode_lossless_jpeg(width: u16, height: u16, precision: u8, samples: &[u16]) -> Vec<u8> {
        assert_eq!(samples.len(), usize::from(width) * usize::from(height));
        let width_usize = usize::from(width);
        let mut out = Vec::new();
        out.extend_from_slice(&[0xff, 0xd8]); // SOI
        // DHT: table 0, 13 codes of length 4 for categories 0..=12.
        out.extend_from_slice(&[0xff, 0xc4]);
        out.extend_from_slice(&(2_u16 + 1 + 16 + 13).to_be_bytes());
        out.push(0x00); // DC class, table id 0
        let mut counts = [0_u8; 16];
        counts[3] = 13;
        out.extend_from_slice(&counts);
        out.extend(0_u8..=12);
        // SOF3.
        out.extend_from_slice(&[0xff, 0xc3]);
        out.extend_from_slice(&(8_u16 + 3).to_be_bytes());
        out.push(precision);
        out.extend_from_slice(&height.to_be_bytes());
        out.extend_from_slice(&width.to_be_bytes());
        out.push(1); // one component
        out.extend_from_slice(&[1, 0x11, 0]); // id 1, 1x1 sampling, tq 0
        // SOS: predictor 1, Se 0, Ah/Al 0.
        out.extend_from_slice(&[0xff, 0xda]);
        out.extend_from_slice(&(6_u16 + 2).to_be_bytes());
        out.push(1); // one scan component
        out.extend_from_slice(&[1, 0x00]); // id 1, DC table 0
        out.extend_from_slice(&[1, 0, 0]); // Ss=1, Se=0, AhAl=0

        // Entropy data: standard T.81 boundary predictors.
        let initial = 1_i32 << (precision - 1);
        let mut writer = BitWriter::new();
        for y in 0..usize::from(height) {
            for x in 0..width_usize {
                let index = y * width_usize + x;
                let sample = i32::from(samples[index]);
                let predicted = if x == 0 && y == 0 {
                    initial
                } else if y == 0 || x != 0 {
                    i32::from(samples[index - 1])
                } else {
                    i32::from(samples[index - width_usize])
                };
                let difference = sample - predicted;
                let category = if difference == 0 {
                    0_u8
                } else {
                    u8::try_from(32 - difference.unsigned_abs().leading_zeros()).unwrap()
                };
                assert!(category <= 12, "test table covers categories 0..=12");
                writer.write(u32::from(category), 4);
                if category > 0 {
                    let encoded = if difference >= 0 {
                        u32::try_from(difference).unwrap()
                    } else {
                        u32::try_from((1_i32 << category) + difference - 1).unwrap()
                    };
                    writer.write(encoded, category);
                }
            }
        }
        out.extend_from_slice(&writer.finish());
        out.extend_from_slice(&[0xff, 0xd9]); // EOI
        out
    }

    struct BitWriter {
        bytes: Vec<u8>,
        current: u8,
        filled: u8,
        stuffing: bool,
    }

    impl BitWriter {
        fn new() -> Self {
            Self {
                bytes: Vec::new(),
                current: 0,
                filled: 0,
                stuffing: true,
            }
        }

        /// Pentax compression 65535 strips are bare bitstreams: 0xFF bytes
        /// are data, not markers, so no 0xFF00 byte stuffing is applied.
        fn without_stuffing() -> Self {
            Self {
                stuffing: false,
                ..Self::new()
            }
        }

        fn write(&mut self, value: u32, bits: u8) {
            for shift in (0..bits).rev() {
                self.current = (self.current << 1) | u8::try_from((value >> shift) & 1).unwrap();
                self.filled += 1;
                if self.filled == 8 {
                    self.emit(self.current);
                    self.current = 0;
                    self.filled = 0;
                }
            }
        }

        fn emit(&mut self, byte: u8) {
            self.bytes.push(byte);
            if self.stuffing && byte == 0xff {
                self.bytes.push(0x00); // byte stuffing
            }
        }

        fn finish(mut self) -> Vec<u8> {
            if self.filled > 0 {
                let padding = 8 - self.filled;
                self.emit((self.current << padding) | ((1 << padding) - 1));
            }
            self.bytes
        }
    }

    // ---- Tests ------------------------------------------------------------

    #[test]
    fn metadata_from_synthetic_pef() {
        let samples = [100_u16, 200, 300, 400, 500, 600, 700, 800];
        let packed = encode_msb_packed(&samples, 4, 12);
        let bytes = synthetic_pef(4, 2, 12, 1, &packed, &[], true);
        let quirks = PefQuirks;
        let container = quirks.parse_container(&bytes).unwrap();
        assert_eq!(container.directories().len(), 2);
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let metadata = quirks.read_metadata(&container, raw).unwrap();

        assert_eq!(metadata.make, "PENTAX");
        assert_eq!(metadata.model, "PENTAX K-TEST");
        assert_eq!((metadata.width, metadata.height), (4, 2));
        assert_eq!(metadata.bits_per_sample, 12);
        assert_eq!(metadata.orientation, Orientation::Normal);
        assert_eq!(metadata.cfa.width, 2);
        assert_eq!(metadata.cfa.height, 2);
        assert_eq!(
            metadata.cfa.cells,
            [CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue]
        );
        // Documented defaults: black 0, white = 2^12 - 1, neutral WB.
        assert_eq!(metadata.black_level.values, [0.0]);
        assert_eq!(metadata.white_level.0, [4095.0]);
        // Default WB gains are exact dyadic values; still compare with an
        // epsilon to keep `clippy::float_cmp` quiet.
        for (actual, expected) in metadata.white_balance.iter().zip([1.0_f32; 4]) {
            assert!((actual - expected).abs() < f32::EPSILON);
        }
        assert_eq!(
            metadata.xyz_to_camera,
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0]]
        );
        assert_eq!(metadata.active_area, None);
        assert_eq!(metadata.crop_area, None);
    }

    #[test]
    fn packed_12_and_14_bit_round_trip() {
        for bits in [12_u8, 14] {
            let mask = (1_u16 << bits) - 1;
            let mut state = 0x243f_6a88_85a3_08d3_u64;
            let mut next = move || {
                state ^= state >> 12;
                state ^= state << 25;
                state ^= state >> 27;
                state.wrapping_mul(0x2545_f491_4f6c_dd1d)
            };
            let (width, height) = (7_u32, 5_u32); // odd width exercises tails
            let samples: Vec<u16> = (0..width * height)
                .map(|index| match index % 11 {
                    0 => 0,
                    1 => mask,
                    _ => u16::try_from(next() & u64::from(mask)).unwrap(),
                })
                .collect();
            let packed = encode_msb_packed(&samples, usize::try_from(width).unwrap(), bits);
            let bytes = synthetic_pef(width, height, u16::from(bits), 1, &packed, &[], true);
            let quirks = PefQuirks;
            let container = quirks.parse_container(&bytes).unwrap();
            let raw = quirks.select_raw_ifd(&container).unwrap();
            let decoded = quirks.decode_pixels(&container, raw, &|| false).unwrap();
            assert_eq!(decoded, samples, "{bits}-bit packed round trip");
        }
    }

    #[test]
    fn lossless_jpeg_strip_round_trip() {
        let (width, height, precision) = (6_u16, 4_u16, 12_u8);
        let samples: Vec<u16> = (0..usize::from(width) * usize::from(height))
            .map(|index| {
                // Smooth gradients keep difference categories small.
                let index = i32::try_from(index).unwrap();
                u16::try_from(2_048 + (index % 7) * 3 - (index / 6) * 2).unwrap()
            })
            .collect();
        let stream = encode_lossless_jpeg(width, height, precision, &samples);
        let bytes = synthetic_pef(
            u32::from(width),
            u32::from(height),
            u16::from(precision),
            7,
            &stream,
            &[],
            true,
        );
        let quirks = PefQuirks;
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let decoded = quirks.decode_pixels(&container, raw, &|| false).unwrap();
        assert_eq!(decoded, samples);
    }

    // ---- Pentax compression 65535 (makernote Huffman) --------------------

    /// Test table in the proprietary makernote layout: per difference
    /// category 0..=12 the (12-bit window value, code length) pair. The codes
    /// are the canonical ones of rawspeed's legacy Pentax table
    /// (counts {0,2,3,1,1,1,1,1,1,2}, symbols {3,4,2,5,1,6,0,7,8,9,10,11,12}):
    /// 00->3, 01->4, 100->2, 101->5, 110->1, 1110->6, 11110->0, then
    /// 111110->7 .. 1111111111->12.
    // Bit patterns are written as literal codes; digit separators would hurt
    // readability here, so the lint is allowed locally.
    #[allow(clippy::unreadable_literal)]
    const TEST_PENTAX_TABLE: [(u16, u8); 13] = [
        (0b11110 << 7, 5),
        (0b110 << 9, 3),
        (0b100 << 9, 3),
        (0b00 << 10, 2),
        (0b01 << 10, 2),
        (0b101 << 9, 3),
        (0b1110 << 8, 4),
        (0b111110 << 6, 6),
        (0b1111110 << 5, 7),
        (0b11111110 << 4, 8),
        (0b111111110 << 3, 9),
        (0b1111111110 << 2, 10),
        (0b1111111111 << 2, 10),
    ];

    /// Serializes `TEST_PENTAX_TABLE` into the makernote tag 0x0220 layout
    /// (little-endian, matching the synthetic II container): depth sentinel,
    /// 12 reserved bytes, window values, code lengths.
    fn pentax_huffman_table_bytes() -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&1_u16.to_le_bytes()); // sentinel: (1 + 12) & 15 = 13 codes
        out.extend_from_slice(&[0; 12]);
        for &(window, _) in &TEST_PENTAX_TABLE {
            out.extend_from_slice(&window.to_le_bytes());
        }
        for &(_, length) in &TEST_PENTAX_TABLE {
            out.push(length);
        }
        out
    }

    /// Builds a Pentax makernote payload: `AOC\0` + two signature bytes + an
    /// IFD. Real PEFs show `AOC MM` while the (container-order little-endian)
    /// entries stay decodable in container order — dcraw's exact behavior,
    /// mirrored here. Values must fit the 4-byte inline field; offsets are
    /// already absolute.
    fn pentax_makernote(entries: &[TestEntry]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"AOC\0MM");
        out.extend_from_slice(&u16::try_from(entries.len()).unwrap().to_le_bytes());
        for &(tag, field_type, count, ref value) in entries {
            assert!(value.len() <= 4);
            out.extend_from_slice(&tag.to_le_bytes());
            out.extend_from_slice(&field_type.to_le_bytes());
            out.extend_from_slice(&count.to_le_bytes());
            let mut field = [0_u8; 4];
            field[..value.len()].copy_from_slice(value);
            out.extend_from_slice(&field);
        }
        out.extend_from_slice(&0_u32.to_le_bytes()); // next IFD link
        out
    }

    /// Makernote variants for the compression-65535 tests.
    enum TestMakernote {
        /// No makernote tag in IFD0 at all.
        Absent,
        /// A makernote without the Huffman table tag 0x0220.
        NoHuffmanTag,
        /// Tag 0x0220 with the given field type and payload bytes.
        Table { bytes: Vec<u8>, field_type: u16 },
    }

    /// Builds a synthetic compression-65535 PEF with a Pentax makernote in
    /// IFD0 and a single bare-bitstream strip.
    fn synthetic_pef_65535(
        width: u32,
        height: u32,
        bits: u16,
        stream: &[u8],
        makernote: TestMakernote,
    ) -> Vec<u8> {
        let mut builder = TiffBuilder::new();
        let mut ifd0_entries: Vec<TestEntry> = vec![
            (271, 2, 7, ascii("PENTAX")),
            (272, 2, 14, ascii("PENTAX K-TEST")),
            (274, 3, 1, short(1)),
        ];
        // Out-of-line Huffman table payloads are appended only after the
        // IFDs, so the makernote entry carries a placeholder offset that is
        // patched once the table's absolute position is known.
        let mut table_to_append: Option<Vec<u8>> = None;
        match makernote {
            TestMakernote::Absent => {}
            TestMakernote::NoHuffmanTag => {
                let payload = pentax_makernote(&[(0x0001, 3, 1, short(0))]);
                ifd0_entries.push((0x927C, 7, u32::try_from(payload.len()).unwrap(), payload));
            }
            TestMakernote::Table { bytes, field_type } => {
                let count = u32::try_from(bytes.len()).unwrap();
                if bytes.len() <= 4 {
                    // Inline value (the decoder must reject it as too short).
                    let payload = pentax_makernote(&[(0x0220, field_type, count, bytes)]);
                    ifd0_entries.push((0x927C, 7, u32::try_from(payload.len()).unwrap(), payload));
                } else {
                    let payload = pentax_makernote(&[(0x0220, field_type, count, long(0))]);
                    ifd0_entries.push((0x927C, 7, u32::try_from(payload.len()).unwrap(), payload));
                    table_to_append = Some(bytes);
                }
            }
        }
        let ifd0 = builder.add_ifd(&ifd0_entries);
        let raw_entries: Vec<TestEntry> = vec![
            (254, 4, 1, long(0)),
            (256, 4, 1, long(width)),
            (257, 4, 1, long(height)),
            (258, 3, 1, short(bits)),
            (259, 3, 1, short(65_535)),
            (262, 3, 1, short(32_803)),
            (273, 4, 1, long(0)), // patched below
            (277, 3, 1, short(1)),
            (278, 4, 1, long(height)),
            (279, 4, 1, long(u32::try_from(stream.len()).unwrap())),
            (33_421, 3, 2, [short(2), short(2)].concat()),
            (33_422, 1, 4, vec![0, 1, 1, 2]),
        ];
        let raw_ifd = builder.add_ifd(&raw_entries);
        let next_link = ifd0 + 2 + ifd0_entries.len() * 12;
        builder.patch_u32(next_link, u32::try_from(raw_ifd).unwrap());
        let strip_offset_field = builder.entry_value_offset(raw_ifd, 273);
        let stream_offset = builder.append_data(stream);
        builder.patch_u32(strip_offset_field, stream_offset);
        if let Some(table) = table_to_append {
            let table_offset = builder.append_data(&table);
            // The makernote payload is the out-of-line value of tag 0x927C;
            // the 0x0220 entry's offset field sits at AOC prefix (6) + count
            // (2) + tag/type/count (8) inside it.
            let makernote_value_field = builder.entry_value_offset(ifd0, 0x927C);
            let payload_offset = u32::from_le_bytes(
                builder.bytes[makernote_value_field..makernote_value_field + 4]
                    .try_into()
                    .unwrap(),
            ) as usize;
            builder.patch_u32(payload_offset + 6 + 2 + 8, table_offset);
        }
        builder.bytes
    }

    /// Encodes `samples` as a Pentax compression-65535 bare bitstream with
    /// `TEST_PENTAX_TABLE`: two-column predictor, category + extra bits, no
    /// markers and no byte stuffing.
    fn encode_pentax_stream(width: usize, height: usize, samples: &[u16]) -> Vec<u8> {
        assert_eq!(samples.len(), width * height);
        let mut writer = BitWriter::without_stuffing();
        for row in 0..height {
            for col in 0..width {
                let index = row * width + col;
                let sample = i32::from(samples[index]);
                let predicted = if col >= 2 {
                    i32::from(samples[index - 2])
                } else if row >= 2 {
                    i32::from(samples[index - 2 * width])
                } else {
                    0
                };
                let difference = sample - predicted;
                let category = if difference == 0 {
                    0_u8
                } else {
                    u8::try_from(32 - difference.unsigned_abs().leading_zeros()).unwrap()
                };
                assert!(usize::from(category) < TEST_PENTAX_TABLE.len());
                let (window, length) = TEST_PENTAX_TABLE[usize::from(category)];
                writer.write(u32::from(window >> (12 - length)), length);
                if category > 0 {
                    let encoded = if difference >= 0 {
                        u32::try_from(difference).unwrap()
                    } else {
                        u32::try_from((1_i32 << category) + difference - 1).unwrap()
                    };
                    writer.write(encoded, category);
                }
            }
        }
        writer.finish()
    }

    fn decode_65535(bytes: &[u8], cancelled: bool) -> Result<Vec<u16>, DecodeError> {
        let quirks = PefQuirks;
        let container = quirks.parse_container(bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        quirks.decode_pixels(&container, raw, &|| cancelled)
    }

    #[test]
    fn pentax_jpeg_makernote_table_round_trip() {
        let (width, height, precision) = (6_usize, 5_usize, 12_u8);
        let samples: Vec<u16> = (0..width * height)
            .map(|index| {
                let i = i32::try_from(index).unwrap();
                // Smooth two-column gradient keeps difference categories small.
                u16::try_from(2_000 + (i % 7) * 41 - (i / 6) * 13).unwrap()
            })
            .collect();
        let stream = encode_pentax_stream(width, height, &samples);
        let bytes = synthetic_pef_65535(
            u32::try_from(width).unwrap(),
            u32::try_from(height).unwrap(),
            u16::from(precision),
            &stream,
            TestMakernote::Table {
                bytes: pentax_huffman_table_bytes(),
                field_type: 7,
            },
        );
        let decoded = decode_65535(&bytes, false).unwrap();
        assert_eq!(decoded, samples);
    }

    #[test]
    fn pentax_jpeg_extreme_samples_round_trip() {
        // Alternating 0 / 4095 columns exercise the two-column predictor and
        // the full 12-bit sample range (categories stay small per parity).
        let (width, height) = (4_usize, 6_usize);
        let samples: Vec<u16> = (0..width * height)
            .map(|index| {
                let row = index / width;
                let col = index % width;
                let ramp = u16::try_from((row * 3) % 17).unwrap();
                if col % 2 == 0 { ramp } else { 4_095 - ramp }
            })
            .collect();
        let stream = encode_pentax_stream(width, height, &samples);
        let bytes = synthetic_pef_65535(
            4,
            6,
            12,
            &stream,
            TestMakernote::Table {
                bytes: pentax_huffman_table_bytes(),
                field_type: 7,
            },
        );
        let decoded = decode_65535(&bytes, false).unwrap();
        assert_eq!(decoded, samples);
    }

    #[test]
    fn compression_65535_without_makernote_is_a_typed_rejection() {
        let bytes = synthetic_pef_65535(4, 2, 12, &[0; 8], TestMakernote::Absent);
        let error = decode_65535(&bytes, false).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "PEF", ref message } if message.contains("37500") && message.contains("makernote")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn compression_65535_without_huffman_tag_is_a_typed_rejection() {
        let bytes = synthetic_pef_65535(4, 2, 12, &[0; 8], TestMakernote::NoHuffmanTag);
        let error = decode_65535(&bytes, false).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "PEF", ref message } if message.contains("0x0220")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn compression_65535_wrong_table_field_type_is_a_typed_rejection() {
        let bytes = synthetic_pef_65535(
            4,
            2,
            12,
            &[0; 8],
            TestMakernote::Table {
                bytes: pentax_huffman_table_bytes(),
                field_type: 3, // SHORT instead of UNDEFINED
            },
        );
        let error = decode_65535(&bytes, false).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "PEF", ref message } if message.contains("field type 3")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn compression_65535_inline_short_table_is_a_typed_rejection() {
        let bytes = synthetic_pef_65535(
            4,
            2,
            12,
            &[0; 8],
            TestMakernote::Table {
                bytes: vec![1, 0], // 2-byte inline value: far too short
                field_type: 7,
            },
        );
        let error = decode_65535(&bytes, false).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "PEF", ref message } if message.contains("at least 14")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn compression_65535_truncated_table_is_a_typed_rejection() {
        // Depth sentinel promises 13 codes (53 bytes) but only 24 are present.
        let mut table = pentax_huffman_table_bytes();
        table.truncate(24);
        let bytes = synthetic_pef_65535(
            4,
            2,
            12,
            &[0; 8],
            TestMakernote::Table {
                bytes: table,
                field_type: 7,
            },
        );
        let error = decode_65535(&bytes, false).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "PEF", ref message } if message.contains("declares 13 codes")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn compression_65535_invalid_code_length_is_a_typed_rejection() {
        let mut table = pentax_huffman_table_bytes();
        let lengths_base = 14 + 13 * 2;
        table[lengths_base] = 13; // code length 13 exceeds the 1..=12 limit
        let bytes = synthetic_pef_65535(
            4,
            2,
            12,
            &[0; 8],
            TestMakernote::Table {
                bytes: table,
                field_type: 7,
            },
        );
        let error = decode_65535(&bytes, false).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "PEF", ref message } if message.contains("invalid code length 13")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn compression_65535_ambiguous_table_is_a_typed_rejection() {
        let mut table = pentax_huffman_table_bytes();
        // Give code 1 the same window/length as code 0: overlapping windows.
        table[16] = table[14];
        table[17] = table[15];
        let lengths_base = 14 + 13 * 2;
        table[lengths_base + 1] = table[lengths_base];
        let bytes = synthetic_pef_65535(
            4,
            2,
            12,
            &[0; 8],
            TestMakernote::Table {
                bytes: table,
                field_type: 7,
            },
        );
        let error = decode_65535(&bytes, false).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "PEF", ref message } if message.contains("overlaps")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn compression_65535_truncated_stream_is_a_typed_rejection() {
        let (width, height) = (6_usize, 4_usize);
        let samples: Vec<u16> = (0..width * height)
            .map(|index| u16::try_from(2_048 + (i32::try_from(index).unwrap() % 7) * 3).unwrap())
            .collect();
        let stream = encode_pentax_stream(width, height, &samples);
        let truncated = stream[..stream.len() / 3].to_vec();
        let bytes = synthetic_pef_65535(
            6,
            4,
            12,
            &truncated,
            TestMakernote::Table {
                bytes: pentax_huffman_table_bytes(),
                field_type: 7,
            },
        );
        let error = decode_65535(&bytes, false).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "PEF", .. }),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn compression_65535_out_of_range_sample_is_a_typed_rejection() {
        // (0,0) = 4095; (0,2) predicts 4095 and adds +1 -> 4096 > 12-bit max.
        let (width, height) = (4_usize, 2_usize);
        let samples = [4_095_u16, 0, 4_096, 0, 1, 2, 3, 4];
        let stream = encode_pentax_stream(width, height, &samples);
        let bytes = synthetic_pef_65535(
            4,
            2,
            12,
            &stream,
            TestMakernote::Table {
                bytes: pentax_huffman_table_bytes(),
                field_type: 7,
            },
        );
        let error = decode_65535(&bytes, false).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "PEF", ref message } if message.contains("outside 0..=4095")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn compression_65535_odd_width_is_a_typed_rejection() {
        let bytes = synthetic_pef_65535(5, 2, 12, &[0; 16], TestMakernote::Absent);
        let error = decode_65535(&bytes, false).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "PEF", ref message } if message.contains("even width")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn compression_65535_multiple_strips_is_a_typed_rejection() {
        // Two strips of one row each; compression 65535 requires exactly one.
        let (width, height) = (4_u32, 2_u32);
        let mut builder = TiffBuilder::new();
        let ifd0 = builder.add_ifd(&[
            (271, 2, 7, ascii("PENTAX")),
            (272, 2, 14, ascii("PENTAX K-TEST")),
            (274, 3, 1, short(1)),
        ]);
        let raw_ifd = builder.add_ifd(&[
            (254, 4, 1, long(0)),
            (256, 4, 1, long(width)),
            (257, 4, 1, long(height)),
            (258, 3, 1, short(12)),
            (259, 3, 1, short(65_535)),
            (262, 3, 1, short(32_803)),
            (273, 4, 2, vec![0; 8]),
            (277, 3, 1, short(1)),
            (278, 4, 1, long(1)),
            (279, 4, 2, vec![0; 8]),
            (33_421, 3, 2, [short(2), short(2)].concat()),
            (33_422, 1, 4, vec![0, 1, 1, 2]),
        ]);
        builder.patch_u32(ifd0 + 2 + 3 * 12, u32::try_from(raw_ifd).unwrap());
        let offsets_field = builder.entry_value_offset(raw_ifd, 273);
        let counts_field = builder.entry_value_offset(raw_ifd, 279);
        let offsets_array = u32::from_le_bytes(
            builder.bytes[offsets_field..offsets_field + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        let counts_array =
            u32::from_le_bytes(builder.bytes[counts_field..counts_field + 4].try_into().unwrap()) as usize;
        let first = builder.append_data(&[0; 4]);
        let second = builder.append_data(&[0; 4]);
        builder.patch_u32(offsets_array, first);
        builder.patch_u32(offsets_array + 4, second);
        builder.patch_u32(counts_array, 4);
        builder.patch_u32(counts_array + 4, 4);

        let error = decode_65535(&builder.bytes, false).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "PEF", ref message } if message.contains("one Huffman-coded strip")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn compression_65535_cancelled_returns_cancelled() {
        let (width, height) = (6_usize, 4_usize);
        let samples: Vec<u16> = (0..width * height)
            .map(|index| u16::try_from(2_048 + (i32::try_from(index).unwrap() % 7) * 3).unwrap())
            .collect();
        let stream = encode_pentax_stream(width, height, &samples);
        let bytes = synthetic_pef_65535(
            6,
            4,
            12,
            &stream,
            TestMakernote::Table {
                bytes: pentax_huffman_table_bytes(),
                field_type: 7,
            },
        );
        let error = decode_65535(&bytes, true).unwrap_err();
        assert!(matches!(error, DecodeError::Cancelled));
    }

    #[test]
    fn unknown_compression_is_a_typed_rejection() {
        let bytes = synthetic_pef(4, 2, 12, 3, &[0; 12], &[], true);
        let quirks = PefQuirks;
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.decode_pixels(&container, raw, &|| false).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "PEF", ref message } if message.contains("compression 3")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn unsupported_bit_depth_is_a_typed_rejection() {
        let bytes = synthetic_pef(4, 2, 8, 1, &[0; 8], &[], true);
        let quirks = PefQuirks;
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.decode_pixels(&container, raw, &|| false).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "PEF", ref message } if message.contains("bit depth 8")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn missing_cfa_tags_are_a_typed_rejection() {
        let packed = encode_msb_packed(&[1, 2, 3, 4, 5, 6, 7, 8], 4, 12);
        let bytes = synthetic_pef(4, 2, 12, 1, &packed, &[], false);
        let quirks = PefQuirks;
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.read_metadata(&container, raw).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "PEF", ref message } if message.contains("33421")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn cancellation_returns_cancelled() {
        let packed = encode_msb_packed(&[1, 2, 3, 4, 5, 6, 7, 8], 4, 12);
        let bytes = synthetic_pef(4, 2, 12, 1, &packed, &[], true);
        let quirks = PefQuirks;
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.decode_pixels(&container, raw, &|| true).unwrap_err();
        assert!(matches!(error, DecodeError::Cancelled));
    }

    #[test]
    fn dng_level_and_white_balance_tags_override_defaults() {
        let mut rationals = Vec::new();
        for (numerator, denominator) in [(1_u32, 2_u32), (1, 1), (1, 4)] {
            rationals.extend_from_slice(&numerator.to_le_bytes());
            rationals.extend_from_slice(&denominator.to_le_bytes());
        }
        let extra: Vec<TestEntry> = vec![
            (TAG_BLACK_LEVEL, 3, 1, short(64)),
            (TAG_WHITE_LEVEL, 3, 1, short(3_950)),
            (TAG_AS_SHOT_NEUTRAL, 5, 3, rationals),
            (
                TAG_ACTIVE_AREA,
                4,
                4,
                [long(0), long(0), long(2), long(4)].concat(),
            ),
        ];
        let packed = encode_msb_packed(&[100, 200, 300, 400, 500, 600, 700, 800], 4, 12);
        let bytes = synthetic_pef(4, 2, 12, 1, &packed, &extra, true);
        let quirks = PefQuirks;
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let metadata = quirks.read_metadata(&container, raw).unwrap();

        assert_eq!(metadata.black_level.values, [64.0]);
        assert_eq!(metadata.white_level.0, [3_950.0]);
        // neutral = [1/2, 1, 1/4] -> gains [2, 1, 4] normalized by green.
        for (actual, expected) in metadata.white_balance.iter().zip([2.0_f32, 1.0, 4.0, 1.0]) {
            assert!((actual - expected).abs() < f32::EPSILON);
        }
        assert_eq!(metadata.active_area, Some(Rect::new(0, 0, 4, 2)));
    }

    #[test]
    fn multi_strip_packed_decode() {
        // Two strips of one row each: exercises per-strip offsets/counts.
        let (width, height) = (4_u32, 2_u32);
        let samples = [0xabc_u16, 1, 2, 3, 4, 5, 6, 0x123];
        let row = encode_msb_packed(&samples[..4], 4, 12);
        let row2 = encode_msb_packed(&samples[4..], 4, 12);
        let mut builder = TiffBuilder::new();
        let ifd0 = builder.add_ifd(&[
            (271, 2, 7, ascii("PENTAX")),
            (272, 2, 14, ascii("PENTAX K-TEST")),
            (274, 3, 1, short(1)),
        ]);
        let raw_ifd = {
            let entries: Vec<TestEntry> = vec![
                (254, 4, 1, long(0)),
                (256, 4, 1, long(width)),
                (257, 4, 1, long(height)),
                (258, 3, 1, short(12)),
                (259, 3, 1, short(1)),
                (262, 3, 1, short(32_803)),
                // Two LONGs -> out-of-line, patched after data is appended.
                (273, 4, 2, vec![0; 8]),
                (277, 3, 1, short(1)),
                (278, 4, 1, long(1)),
                (279, 4, 2, vec![0; 8]),
                (33_421, 3, 2, [short(2), short(2)].concat()),
                (33_422, 1, 4, vec![0, 1, 1, 2]),
            ];
            builder.add_ifd(&entries)
        };
        builder.patch_u32(ifd0 + 2 + 3 * 12, u32::try_from(raw_ifd).unwrap());
        let offsets_field = builder.entry_value_offset(raw_ifd, 273);
        let counts_field = builder.entry_value_offset(raw_ifd, 279);
        // Out-of-line value arrays live right after the IFD table; read the
        // pointers from the entry value fields.
        let offsets_array = u32::from_le_bytes(
            builder.bytes[offsets_field..offsets_field + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        let counts_array =
            u32::from_le_bytes(builder.bytes[counts_field..counts_field + 4].try_into().unwrap()) as usize;
        let first = builder.append_data(&row);
        let second = builder.append_data(&row2);
        builder.patch_u32(offsets_array, first);
        builder.patch_u32(offsets_array + 4, second);
        builder.patch_u32(counts_array, u32::try_from(row.len()).unwrap());
        builder.patch_u32(counts_array + 4, u32::try_from(row2.len()).unwrap());

        let quirks = PefQuirks;
        let container = quirks.parse_container(&builder.bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let decoded = quirks.decode_pixels(&container, raw, &|| false).unwrap();
        assert_eq!(decoded, samples);
    }
}
