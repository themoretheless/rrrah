//! Nikon NEF quirks — Stage 2 implementation.
//!
//! # Container and IFD selection
//!
//! NEF is a classic TIFF (`II` or `MM` byte order); the shared
//! [`CameraFile`] parser handles both. The raw frame usually lives in a
//! `SubIFDs` directory with CFA photometric (32803) and `SubFileType` 0,
//! which the default [`select_generic_raw_ifd`] heuristic picks
//! deterministically (CFA photometric first, then primary, then largest
//! area), so this file overrides neither `parse_container` nor
//! `select_raw_ifd`.
//!
//! # Supported pixel storage
//!
//! - `Compression = 1` with `BitsPerSample` 12 or 14, `SamplesPerPixel = 1`,
//!   strip-organized rows packed MSB-first and byte-aligned per row — the
//!   layout used by Nikon's uncompressed NEF variants. Rows are unpacked
//!   with [`decode_msb_packed`]. The `cancelled` callback is honored once
//!   per strip and once per row and yields [`DecodeError::Cancelled`].
//! - `Compression = 34713` (Nikon lossless / lossy "NEF Compressed") is
//!   decoded natively below; see the next section.
//!
//! # Nikon compression 34713
//!
//! Canonical references: dcraw `nikon_load_raw`, rawspeed
//! `NikonDecompressor`/`NefDecoder`, and rawler `decoders/nef.rs`
//! (`do_decode`). Findings that shape this implementation:
//!
//! - The strip is a BARE MSB-first bitstream: no SOI/SOF3/SOS/EOI markers
//!   and no `0xFF00` byte stuffing (dcraw reads it with `getbits` and
//!   `zero_after_ff == 0`; rawspeed/rawler use a plain MSB bit pump over
//!   the strip). It is therefore NOT a T.81 marker stream and the shared
//!   [`crate::dng::lossless_jpeg::decode_with_external_tables`] entry
//!   point — which requires a valid marker-delimited SOI/SOF3/SOS/EOI
//!   payload — cannot be used, and wrapping the bits in synthetic markers
//!   would fabricate a container the camera never wrote. The predictor
//!   scheme is also not T.81 Annex H: two interleaved predictor channels
//!   (even/odd columns), each seeded at every row start from a vertical
//!   accumulator per row parity, with the four seed values read from the
//!   makernote. Decoding is implemented natively in this file instead.
//! - The Huffman table is NOT in the stream and NOT in the makernote
//!   either: it is one of six built-in tables (dcraw `nikon_tree`),
//!   selected by the curve-blob version byte `v0` (`0x46` ⇒ lossless
//!   table) and the bit depth (+3 for 14-bit), with a switch to the "after
//!   split" table (`+1`) at the makernote split row. Tables are stored
//!   here in DHT layout (16 counts + symbols) matching dcraw byte-for-byte,
//!   including the padded zero symbol of the 12-bit lossy table.
//! - The makernote (Exif tag 0x927C under the IFD0 `ExifIFD` 0x8769) holds a
//!   mini-IFD. Three documented layouts are parsed: format 3
//!   (`"Nikon\0\x02\x10\0\0"` + embedded TIFF header with its own byte
//!   order), format 2 (`"Nikon\0\x01\x00"` + plain IFD at offset 8), and
//!   format 1 (plain IFD at offset 0). The compression/curve blob is the
//!   value of makernote tag 0x8C (`NikonCurve`) or 0x96
//!   (`LinearizationTable`); when both exist the later entry in tag order
//!   (0x96) wins, matching dcraw's sequential overwrite and rawspeed's
//!   0x96-first lookup. Blob layout: `v0`, `v1`, optional 2110-byte skip
//!   (`v0 == 0x49 || v1 == 0x58`), four `u16` predictor seeds, `u16`
//!   curve segment count, then the curve: identity for `v0 == 0x46`
//!   (lossless), piecewise-linear interpolated anchors plus a split row at
//!   absolute blob offset 562 for `v0 == 0x44 && v1 ∈ {0x20, 0x40}`
//!   (`v1 == 0x40` also implies the Z-series "real bit depth = bps − 2"
//!   quirk), otherwise an explicit `csize`-entry table.
//! - The linearization curve is applied through
//!   [`apply_linearization_curve`]; per project policy an out-of-range
//!   sample is a hard typed error, not dcraw's silent clamp. Predictor
//!   values outside the `u16` range are likewise hard errors.
//! - Predictor seed order follows rawspeed/rawler
//!   (`[c0r0, c0r1, c1r0, c1r1]`): the 2nd and 3rd `u16` are swapped
//!   relative to dcraw's sequential read; both modern decoders agree, and
//!   the swapped reading is the one consistent with the vertical-parity
//!   accumulator layout.
//!
//! # Typed rejections (never the embedded JPEG, never silent degradation;
//! project policy: `docs/DECODE_FORMAT_AUDIT.md`)
//!
//! - `Compression = 34713` with multiple strips (all known cameras write a
//!   single full-height strip), a missing/broken makernote, a missing
//!   0x8C/0x96 curve tag, a truncated curve blob, an unbuildable curve
//!   (`csize == 0` in the explicit form), a split row without a following
//!   Huffman table, truncated/invalid entropy data, or curve out-of-range
//!   samples are all explicit errors. D100/D810 units whose firmware
//!   mislabels uncompressed data as 34713 fail typed as well — no
//!   heuristic re-interpretation is attempted (rawspeed's
//!   `NEFIsUncompressed` size heuristic can misclassify genuinely
//!   compressed files, which this project treats as unacceptable silent
//!   degradation).
//! - Any other `Compression` value (including the modern Z-series
//!   high-efficiency / lossy variants) is rejected with the compression
//!   value in the message.
//! - Tiled raw storage (`TileOffsets` without `StripOffsets`),
//!   `SamplesPerPixel != 1`, and bit depths other than 12/14 are rejected
//!   explicitly.
//!
//! # Metadata mapping
//!
//! - Make (271) / Model (272) are read from IFD0, falling back to the raw
//!   IFD; Orientation (274) likewise, defaulting to `Normal`.
//! - CFA comes from the raw IFD's `CFARepeatPatternDim` (33421) and
//!   `CFAPattern` (33422), which Nikon writes for CFA raws. Missing tags,
//!   unknown color indices, or a pattern the display pipeline cannot use
//!   (anything but 2x2 RGB Bayer, same constraint as the DNG backend) are
//!   typed rejections.
//! - BlackLevel/WhiteLevel DNG tags are normally absent in NEF. Documented
//!   defaults are used instead: black level 0 and white level
//!   `2^BitsPerSample - 1`, uniform across the mosaic. This deliberately
//!   ignores Nikon's model-specific black level from the makernote;
//!   refining it is future makernote work.
//! - White balance: `AsShotNeutral` (50728, assumed R,G,B plane order) is
//!   honored when present — rare in NEF — otherwise neutral gains
//!   `[1, 1, 1, 1]` are used (documented fallback; Nikon makernote tag
//!   0x0097 WB parsing is future work).
//! - `xyz_to_camera` is the identity matrix with a zero fourth row: NEF
//!   carries no DNG `ColorMatrix` tags and makernote color parsing is out of
//!   scope. This mirrors the DNG backend's uncalibrated fallback.
//! - `active_area` is the full stored sensor rectangle; `crop_area` is
//!   `None` (NEF has no `DefaultCrop` tags).
//!
//! # Test evidence
//!
//! The unit tests below build synthetic minimal NEF byte streams
//! programmatically (both `II` and `MM` headers, strip-based packed 12-bit
//! pixel data, plus `Compression = 34713` streams encoded with the inverse
//! of the decoder and makernotes in all three documented layouts). They
//! validate the parser, the entropy decoder round-trip, the metadata
//! mapping, and the typed rejections. They are NOT evidence of
//! compatibility with real camera-produced NEF files; no licensed camera
//! files are committed.

use rrrah_core::{CfaColor, CfaPattern, LevelGrid, Orientation, Rect, WhiteLevel};

use super::{
    CameraDirectory, CameraFile, CameraMetadata, CameraQuirks, camera_error, optional_ascii, optional_scalar,
    orientation_from_tag, required_scalar, tags,
};
use crate::{
    DecodeError,
    dng::{
        lossless_jpeg::{LosslessJpegError, apply_linearization_curve},
        tiff::ByteOrder,
        uncompressed::decode_msb_packed,
    },
};

/// TIFF `Compression` value for Nikon's proprietary lossless variant.
const COMPRESSION_NIKON_LOSSLESS: u64 = 34_713;
/// DNG `AsShotNeutral` tag, honored when (rarely) present in a NEF raw IFD.
const AS_SHOT_NEUTRAL: u16 = 50_728;
/// `ExifIFD` pointer tag in IFD0; the Nikon makernote hangs off it.
const TAG_EXIF_IFD: u16 = 0x8769;
/// `MakerNote` tag inside the Exif IFD.
const TAG_MAKER_NOTE: u16 = 0x927C;
/// Nikon makernote tags carrying the compression/curve blob (`NikonCurve`
/// 0x008C, `LinearizationTable` 0x0096). When both are present the later
/// entry in tag order (0x0096) wins, matching dcraw's sequential
/// `meta_offset` overwrite and rawspeed's 0x96-first lookup.
const NIKON_CURVE_TAGS: [u16; 2] = [0x008c, 0x0096];

/// Huffman tables for the 34713 entropy coding in DHT layout: 16
/// code-length counts, then the symbol list. Byte-identical to dcraw
/// `nikon_tree`, rawspeed `NikonDecompressor::nikon_tree`, and rawler
/// `NIKON_TREE` (their split of the shift nibbles is kept mangled into the
/// symbol bytes, as dcraw stores them). Table 0 declares 14 codes but only
/// 13 documented symbols; dcraw reads the 14th symbol from the zero padding
/// of its flat table, so the zero is spelled out here explicitly. Index
/// selection: base 2 when the curve blob's `v0 == 0x46` ("lossless"), +3
/// for 14-bit data, and +1 ("after split") past the makernote split row.
const NIKON_TREE: [(&[u8; 16], &[u8]); 6] = [
    // 0: 12-bit lossy
    (
        &[0, 1, 5, 1, 1, 1, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0],
        &[5, 4, 3, 6, 2, 7, 1, 0, 8, 9, 11, 10, 12, 0],
    ),
    // 1: 12-bit lossy after split
    (
        &[0, 1, 5, 1, 1, 1, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0],
        &[0x39, 0x5a, 0x38, 0x27, 0x16, 5, 4, 3, 2, 1, 0, 11, 12, 12],
    ),
    // 2: 12-bit lossless
    (
        &[0, 1, 4, 2, 3, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        &[5, 4, 6, 3, 7, 2, 8, 1, 9, 0, 10, 11, 12],
    ),
    // 3: 14-bit lossy
    (
        &[0, 1, 4, 3, 1, 1, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0],
        &[5, 6, 4, 7, 8, 3, 9, 2, 1, 0, 10, 11, 12, 13, 14],
    ),
    // 4: 14-bit lossy after split
    (
        &[0, 1, 5, 1, 1, 1, 1, 1, 1, 1, 2, 0, 0, 0, 0, 0],
        &[8, 0x5c, 0x4b, 0x3a, 0x29, 7, 6, 5, 4, 3, 2, 1, 0, 13, 14],
    ),
    // 5: 14-bit lossless
    (
        &[0, 1, 4, 2, 2, 3, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0],
        &[7, 6, 8, 5, 9, 4, 10, 3, 11, 12, 2, 0, 1, 13, 14],
    ),
];

const NEF: &str = "NEF";

/// Registered NEF quirks.
#[derive(Debug)]
pub(crate) struct NefQuirks;

impl CameraQuirks for NefQuirks {
    fn format_name(&self) -> &'static str {
        NEF
    }

    fn read_metadata(
        &self,
        container: &CameraFile<'_>,
        raw: &CameraDirectory<'_>,
    ) -> Result<CameraMetadata, DecodeError> {
        let ifd0 = container
            .directories()
            .iter()
            .find(|directory| directory.is_top_level())
            .ok_or_else(|| camera_error(NEF, "no top-level IFD0 directory"))?;

        let make = optional_ascii(NEF, ifd0, tags::MAKE)?
            .or(optional_ascii(NEF, raw, tags::MAKE)?)
            .unwrap_or_default();
        let model = optional_ascii(NEF, ifd0, tags::MODEL)?
            .or(optional_ascii(NEF, raw, tags::MODEL)?)
            .unwrap_or_default();
        let orientation = match optional_scalar(NEF, ifd0, tags::ORIENTATION)?.or(optional_scalar(
            NEF,
            raw,
            tags::ORIENTATION,
        )?) {
            Some(value) => orientation_from_tag(NEF, value)?,
            None => Orientation::Normal,
        };

        if let Some(photometric) = optional_scalar(NEF, raw, tags::PHOTOMETRIC_INTERPRETATION)?
            && photometric != tags::PHOTOMETRIC_CFA
        {
            return Err(camera_error(
                NEF,
                format!("raw IFD photometric {photometric} is not CFA (32803)"),
            ));
        }

        let width = to_u32(required_scalar(NEF, raw, tags::IMAGE_WIDTH)?, "ImageWidth")?;
        let height = to_u32(required_scalar(NEF, raw, tags::IMAGE_LENGTH)?, "ImageLength")?;
        if width == 0 || height == 0 {
            return Err(camera_error(NEF, "raw IFD has zero image dimensions"));
        }
        let bits = to_u8(required_scalar(NEF, raw, tags::BITS_PER_SAMPLE)?, "BitsPerSample")?;
        if !(1..=16).contains(&bits) {
            return Err(camera_error(NEF, format!("unsupported BitsPerSample {bits}")));
        }

        let cfa = read_cfa(raw)?;

        // Documented defaults: NEF raw IFDs do not carry DNG BlackLevel /
        // WhiteLevel tags. Black is 0 and white is the full-scale value of
        // the stored bit depth, uniform across the mosaic. Nikon's
        // model-specific makernote black level is future work.
        let black_level = LevelGrid {
            width: 1,
            height: 1,
            components: 1,
            values: vec![0.0],
        };
        let white = (1_u32 << bits) - 1;
        let white_level = WhiteLevel(vec![white as f32]);

        // Documented fallback: neutral gains when AsShotNeutral is absent
        // (the normal NEF case). Nikon makernote 0x0097 WB is future work.
        // Gains are divided in f64, then narrowed to the f32 storage type of
        // `CameraMetadata::white_balance`; the precision loss is intended.
        #[allow(clippy::cast_possible_truncation)]
        let white_balance = match raw.entry(NEF, AS_SHOT_NEUTRAL)? {
            Some(entry) => {
                let neutral = entry
                    .numeric_values()
                    .map_err(|error| camera_error(NEF, format!("AsShotNeutral: {error}")))?;
                if neutral.len() < 3 {
                    return Err(camera_error(
                        NEF,
                        format!("AsShotNeutral has {} values, expected at least 3", neutral.len()),
                    ));
                }
                let mut gains = [0.0_f64; 3];
                for (gain, value) in gains.iter_mut().zip(&neutral) {
                    if *value <= 0.0 {
                        return Err(camera_error(NEF, "AsShotNeutral contains a non-positive value"));
                    }
                    *gain = 1.0 / value;
                }
                let green = gains[1];
                [(gains[0] / green) as f32, 1.0, (gains[2] / green) as f32, 1.0]
            }
            None => [1.0, 1.0, 1.0, 1.0],
        };

        // Uncalibrated identity, mirroring the DNG backend fallback: NEF
        // carries no ColorMatrix tags and makernote color parsing is out of
        // scope.
        let xyz_to_camera = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]];

        Ok(CameraMetadata {
            make,
            model,
            width,
            height,
            bits_per_sample: bits,
            cfa,
            black_level,
            white_level,
            white_balance,
            xyz_to_camera,
            active_area: Some(Rect::full(width, height)),
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
        let compression = required_scalar(NEF, raw, tags::COMPRESSION)?;
        match compression {
            COMPRESSION_NIKON_LOSSLESS => decode_nikon_lossless(container, raw, cancelled),
            tags::COMPRESSION_UNCOMPRESSED => decode_uncompressed(container, raw, cancelled),
            other => Err(unsupported_compression(other)),
        }
    }
}

/// Decodes `Compression = 1` MSB-first packed 12/14-bit strip rows.
// Linear tag validation + strip walk; splitting it up would scatter the
// error context across helpers with long parameter lists.
#[allow(clippy::too_many_lines)]
fn decode_uncompressed(
    container: &CameraFile<'_>,
    raw: &CameraDirectory<'_>,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<u16>, DecodeError> {
    {
        let samples_per_pixel = optional_scalar(NEF, raw, tags::SAMPLES_PER_PIXEL)?.unwrap_or(1);
        if samples_per_pixel != 1 {
            return Err(camera_error(
                NEF,
                format!("unsupported SamplesPerPixel {samples_per_pixel}, expected 1"),
            ));
        }
        let bits = to_u8(required_scalar(NEF, raw, tags::BITS_PER_SAMPLE)?, "BitsPerSample")?;
        if !matches!(bits, 12 | 14) {
            return Err(camera_error(
                NEF,
                format!(
                    "unsupported BitsPerSample {bits} for uncompressed NEF: only MSB-first packed 12/14-bit rows are supported"
                ),
            ));
        }
        let width = to_u32(required_scalar(NEF, raw, tags::IMAGE_WIDTH)?, "ImageWidth")?;
        let height = to_u32(required_scalar(NEF, raw, tags::IMAGE_LENGTH)?, "ImageLength")?;
        if width == 0 || height == 0 {
            return Err(camera_error(NEF, "raw IFD has zero image dimensions"));
        }
        let width =
            usize::try_from(width).map_err(|_| camera_error(NEF, "ImageWidth does not fit in usize"))?;
        let height =
            usize::try_from(height).map_err(|_| camera_error(NEF, "ImageLength does not fit in usize"))?;

        let Some(offsets_entry) = raw.entry(NEF, tags::STRIP_OFFSETS)? else {
            if raw.entry(NEF, tags::TILE_OFFSETS)?.is_some() {
                return Err(camera_error(
                    NEF,
                    "tiled NEF raw storage is not supported (TileOffsets without StripOffsets)",
                ));
            }
            return Err(camera_error(NEF, "raw IFD has no StripOffsets tag"));
        };
        let strip_offsets = offsets_entry
            .unsigned_values()
            .map_err(|error| camera_error(NEF, format!("StripOffsets: {error}")))?;
        let strip_byte_counts = raw
            .entry(NEF, tags::STRIP_BYTE_COUNTS)?
            .ok_or_else(|| camera_error(NEF, "raw IFD has no StripByteCounts tag"))?
            .unsigned_values()
            .map_err(|error| camera_error(NEF, format!("StripByteCounts: {error}")))?;
        if strip_offsets.len() != strip_byte_counts.len() {
            return Err(camera_error(
                NEF,
                format!(
                    "StripOffsets count {} does not match StripByteCounts count {}",
                    strip_offsets.len(),
                    strip_byte_counts.len()
                ),
            ));
        }
        let rows_per_strip_u64 = optional_scalar(NEF, raw, tags::ROWS_PER_STRIP)?.unwrap_or(height as u64);
        if rows_per_strip_u64 == 0 {
            return Err(camera_error(NEF, "RowsPerStrip is zero"));
        }
        let rows_per_strip = usize::try_from(rows_per_strip_u64)
            .map_err(|_| camera_error(NEF, "RowsPerStrip does not fit in usize"))?;

        let row_bytes = width
            .checked_mul(usize::from(bits))
            .and_then(|bits| bits.checked_add(7))
            .map(|bits| bits / 8)
            .ok_or_else(|| camera_error(NEF, "arithmetic overflow computing packed row byte length"))?;
        let expected_strips = height.div_ceil(rows_per_strip);
        if strip_offsets.len() < expected_strips {
            return Err(camera_error(
                NEF,
                format!(
                    "raw IFD has {} strips, expected at least {expected_strips} for {height} rows",
                    strip_offsets.len()
                ),
            ));
        }

        let total = width
            .checked_mul(height)
            .ok_or_else(|| camera_error(NEF, "arithmetic overflow computing sample count"))?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(total)
            .map_err(|_| camera_error(NEF, format!("could not allocate {total} samples")))?;
        pixels.resize(total, 0);

        let data = container.data();
        for strip in 0..expected_strips {
            if cancelled() {
                return Err(DecodeError::Cancelled);
            }
            let first_row = strip
                .checked_mul(rows_per_strip)
                .ok_or_else(|| camera_error(NEF, "arithmetic overflow computing strip first row"))?;
            let rows = rows_per_strip.min(height - first_row);
            let needed = rows
                .checked_mul(row_bytes)
                .ok_or_else(|| camera_error(NEF, "arithmetic overflow computing strip byte length"))?;
            let byte_count = usize::try_from(strip_byte_counts[strip])
                .map_err(|_| camera_error(NEF, "StripByteCounts value does not fit in usize"))?;
            if byte_count < needed {
                return Err(camera_error(
                    NEF,
                    format!("strip {strip} has {byte_count} bytes, expected at least {needed}"),
                ));
            }
            let offset = usize::try_from(strip_offsets[strip])
                .map_err(|_| camera_error(NEF, "StripOffsets value does not fit in usize"))?;
            let end = offset
                .checked_add(needed)
                .ok_or_else(|| camera_error(NEF, "arithmetic overflow computing strip end"))?;
            let strip_data = data.get(offset..end).ok_or_else(|| {
                camera_error(
                    NEF,
                    format!(
                        "strip {strip} at offset {offset} is truncated: need {needed} bytes, have {}",
                        data.len().saturating_sub(offset)
                    ),
                )
            })?;
            for row in 0..rows {
                if cancelled() {
                    return Err(DecodeError::Cancelled);
                }
                let source = &strip_data[row * row_bytes..(row + 1) * row_bytes];
                let target_start = (first_row + row)
                    .checked_mul(width)
                    .ok_or_else(|| camera_error(NEF, "arithmetic overflow computing row offset"))?;
                decode_msb_packed(source, &mut pixels[target_start..target_start + width], bits)
                    .map_err(|error| camera_error(NEF, format!("packed row {}: {error}", first_row + row)))?;
            }
        }
        Ok(pixels)
    }
}

/// Builds the typed rejection for an unsupported `Compression` value.
fn unsupported_compression(compression: u64) -> DecodeError {
    camera_error(
        NEF,
        format!(
            "unsupported NEF compression {compression}: supported are uncompressed (1) MSB-first \
             packed 12/14-bit rows and Nikon lossless (34713); Nikon Z-series high-efficiency/lossy \
             variants are not supported"
        ),
    )
}

// ---------------------------------------------------------------------
// Nikon lossless (Compression = 34713)
// ---------------------------------------------------------------------

/// Typed error helper for makernote problems in the 34713 path.
fn makernote_error(message: impl Into<String>) -> DecodeError {
    camera_error(NEF, format!("Nikon makernote: {}", message.into()))
}

/// Decodes a `Compression = 34713` raw IFD: makernote curve blob +
/// Nikon entropy coding + linearization curve.
// Linear tag validation + row decode loop; splitting it up would scatter
// the error context across helpers with long parameter lists.
#[allow(clippy::too_many_lines)]
fn decode_nikon_lossless(
    container: &CameraFile<'_>,
    raw: &CameraDirectory<'_>,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<u16>, DecodeError> {
    let samples_per_pixel = optional_scalar(NEF, raw, tags::SAMPLES_PER_PIXEL)?.unwrap_or(1);
    if samples_per_pixel != 1 {
        return Err(camera_error(
            NEF,
            format!("unsupported SamplesPerPixel {samples_per_pixel} for Nikon lossless (34713), expected 1"),
        ));
    }
    let bits = to_u8(required_scalar(NEF, raw, tags::BITS_PER_SAMPLE)?, "BitsPerSample")?;
    if !matches!(bits, 12 | 14) {
        return Err(camera_error(
            NEF,
            format!(
                "unsupported BitsPerSample {bits} for Nikon lossless (34713): only 12/14-bit is documented"
            ),
        ));
    }
    let width = to_u32(required_scalar(NEF, raw, tags::IMAGE_WIDTH)?, "ImageWidth")?;
    let height = to_u32(required_scalar(NEF, raw, tags::IMAGE_LENGTH)?, "ImageLength")?;
    if width == 0 || height == 0 {
        return Err(camera_error(NEF, "raw IFD has zero image dimensions"));
    }
    let width = usize::try_from(width).map_err(|_| camera_error(NEF, "ImageWidth does not fit in usize"))?;
    let height =
        usize::try_from(height).map_err(|_| camera_error(NEF, "ImageLength does not fit in usize"))?;

    let strip_offsets = raw
        .entry(NEF, tags::STRIP_OFFSETS)?
        .ok_or_else(|| camera_error(NEF, "raw IFD has no StripOffsets tag"))?
        .unsigned_values()
        .map_err(|error| camera_error(NEF, format!("StripOffsets: {error}")))?;
    let strip_byte_counts = raw
        .entry(NEF, tags::STRIP_BYTE_COUNTS)?
        .ok_or_else(|| camera_error(NEF, "raw IFD has no StripByteCounts tag"))?
        .unsigned_values()
        .map_err(|error| camera_error(NEF, format!("StripByteCounts: {error}")))?;
    if strip_offsets.len() != strip_byte_counts.len() {
        return Err(camera_error(
            NEF,
            format!(
                "StripOffsets count {} does not match StripByteCounts count {}",
                strip_offsets.len(),
                strip_byte_counts.len()
            ),
        ));
    }
    if strip_offsets.len() != 1 {
        return Err(camera_error(
            NEF,
            format!(
                "Nikon lossless (34713) with {} strips is not supported: every documented camera writes a single full-height strip",
                strip_offsets.len()
            ),
        ));
    }
    let offset = usize::try_from(strip_offsets[0])
        .map_err(|_| camera_error(NEF, "StripOffsets value does not fit in usize"))?;
    let byte_count = usize::try_from(strip_byte_counts[0])
        .map_err(|_| camera_error(NEF, "StripByteCounts value does not fit in usize"))?;
    let end = offset
        .checked_add(byte_count)
        .ok_or_else(|| camera_error(NEF, "arithmetic overflow computing strip end"))?;
    let strip = container.data().get(offset..end).ok_or_else(|| {
        camera_error(
            NEF,
            format!(
                "Nikon lossless strip at offset {offset} is truncated: need {byte_count} bytes, have {}",
                container.data().len().saturating_sub(offset)
            ),
        )
    })?;

    let (meta, meta_order) = nikon_curve_blob(container)?;
    let curve = parse_curve_blob(meta, meta_order, bits, height)?;

    let total = width
        .checked_mul(height)
        .ok_or_else(|| camera_error(NEF, "arithmetic overflow computing sample count"))?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(total)
        .map_err(|_| camera_error(NEF, format!("could not allocate {total} samples")))?;
    pixels.resize(total, 0);

    let (counts, symbols) = NIKON_TREE[curve.huff_select];
    let mut huffman = NikonHuffman::build(counts, symbols)?;
    let mut reader = MsbReader::new(strip);
    // Vertical accumulators indexed [column parity][row parity], seeded in
    // the makernote order used by rawspeed/rawler: c0r0, c0r1, c1r0, c1r1.
    let mut up = [[curve.vpred[0], curve.vpred[1]], [curve.vpred[2], curve.vpred[3]]];
    for row in 0..height {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        if curve.split != 0 && row == curve.split {
            let next = curve.huff_select + 1;
            let Some(&(counts, symbols)) = NIKON_TREE.get(next) else {
                return Err(camera_error(
                    NEF,
                    format!(
                        "Nikon lossless split at row {row} requires Huffman table {next}, which does not exist"
                    ),
                ));
            };
            huffman = NikonHuffman::build(counts, symbols)?;
        }
        let row_parity = row & 1;
        let mut left = [0_i64; 2];
        for col in 0..width {
            let column_parity = col & 1;
            let diff = decode_nikon_diff(&mut reader, &huffman)?;
            if col < 2 {
                up[column_parity][row_parity] += diff;
                left[column_parity] = up[column_parity][row_parity];
            } else {
                left[column_parity] += diff;
            }
            let value = left[column_parity];
            pixels[row * width + col] = u16::try_from(value).map_err(|_| {
                camera_error(
                    NEF,
                    format!(
                        "Nikon lossless predictor at row {row} column {col} is {value}, outside the u16 range"
                    ),
                )
            })?;
        }
    }
    apply_linearization_curve(&mut pixels, &curve.curve, cancelled).map_err(|error| match error {
        LosslessJpegError::Cancelled { .. } => DecodeError::Cancelled,
        other => camera_error(NEF, format!("Nikon lossless linearization curve: {other}")),
    })?;
    if pixels.len() != total {
        return Err(camera_error(
            NEF,
            format!(
                "Nikon lossless decoded {} samples, expected {total}",
                pixels.len()
            ),
        ));
    }
    Ok(pixels)
}

/// Locates the Nikon compression/curve blob: IFD0 → `ExifIFD` (0x8769) →
/// `MakerNote` (0x927C) → mini-IFD → tag 0x8C/0x96 value. Returns the blob
/// bytes and the byte order its multi-byte fields are stored in.
fn nikon_curve_blob<'a>(container: &CameraFile<'a>) -> Result<(&'a [u8], ByteOrder), DecodeError> {
    let ifd0 = container
        .directories()
        .iter()
        .find(|directory| directory.is_top_level())
        .ok_or_else(|| camera_error(NEF, "no top-level IFD0 directory"))?;
    let exif_offset = optional_scalar(NEF, ifd0, TAG_EXIF_IFD)?.ok_or_else(|| {
        makernote_error(
            "Nikon lossless (34713) needs the makernote curve tag, but IFD0 has no ExifIFD (0x8769) pointer",
        )
    })?;
    let exif = container.parse_ifd_at(NEF, exif_offset)?;
    let makernote = exif
        .entry(TAG_MAKER_NOTE)
        .map_err(|error| makernote_error(format!("MakerNote tag: {error}")))?
        .ok_or_else(|| {
            makernote_error(
                "Nikon lossless (34713) needs the Nikon makernote (Exif tag 0x927C), which is absent",
            )
        })?
        .raw_bytes();
    makernote_curve_value(makernote, container.byte_order())
}

/// Parses the makernote mini-IFD and extracts the curve tag value. Covers
/// the three documented Nikon makernote layouts: format 3 (signature +
/// embedded TIFF header with its own byte order), format 2 (signature +
/// plain IFD at offset 8), and format 1 (plain IFD at offset 0). All reads
/// are bounded to the makernote value slice.
fn makernote_curve_value(
    makernote: &[u8],
    container_order: ByteOrder,
) -> Result<(&[u8], ByteOrder), DecodeError> {
    let (base, order, ifd_relative) = if makernote.len() >= 8 && &makernote[..5] == b"Nikon" {
        let embedded = makernote.len() >= 18
            && matches!(&makernote[10..12], b"II" | b"MM")
            && (if &makernote[10..12] == b"II" {
                ByteOrder::Little
            } else {
                ByteOrder::Big
            })
            .u16(&makernote[12..14])
                == 42;
        if embedded {
            let order = if &makernote[10..12] == b"II" {
                ByteOrder::Little
            } else {
                ByteOrder::Big
            };
            let ifd_offset = usize::try_from(order.u32(&makernote[14..18]))
                .map_err(|_| makernote_error("embedded TIFF IFD offset does not fit in usize"))?;
            (10_usize, order, ifd_offset)
        } else {
            // Format 2: "Nikon\0\x01\x00" followed by a plain IFD at 8.
            (0_usize, container_order, 8_usize)
        }
    } else {
        // Format 1: plain IFD at 0, container byte order.
        (0_usize, container_order, 0_usize)
    };

    let ifd_at = base
        .checked_add(ifd_relative)
        .ok_or_else(|| makernote_error("arithmetic overflow computing makernote IFD offset"))?;
    let count_bytes = makernote
        .get(
            ifd_at
                ..ifd_at
                    .checked_add(2)
                    .ok_or_else(|| makernote_error("makernote IFD offset overflow"))?,
        )
        .ok_or_else(|| makernote_error("makernote IFD is truncated before the entry count"))?;
    let entry_count = usize::from(order.u16(count_bytes));
    let entries_end = entry_count
        .checked_mul(12)
        .and_then(|table| table.checked_add(2))
        .and_then(|length| ifd_at.checked_add(length))
        .ok_or_else(|| makernote_error("arithmetic overflow computing makernote IFD end"))?;
    if entries_end > makernote.len() {
        return Err(makernote_error(format!(
            "makernote IFD declares {entry_count} entries but the makernote is only {} bytes",
            makernote.len()
        )));
    }

    // Last matching entry wins (0x96 follows 0x8C in tag order), matching
    // dcraw's sequential overwrite and rawspeed's 0x96-first lookup.
    let mut found: Option<&[u8]> = None;
    for index in 0..entry_count {
        let at = ifd_at + 2 + index * 12;
        let tag = order.u16(&makernote[at..at + 2]);
        if !NIKON_CURVE_TAGS.contains(&tag) {
            continue;
        }
        let type_code = order.u16(&makernote[at + 2..at + 4]);
        let type_width = makernote_type_width(type_code).ok_or_else(|| {
            makernote_error(format!(
                "curve tag {tag:#06x} has unsupported field type {type_code}"
            ))
        })?;
        let count = usize::try_from(order.u32(&makernote[at + 4..at + 8]))
            .map_err(|_| makernote_error("curve tag value count does not fit in usize"))?;
        let byte_len = count
            .checked_mul(type_width)
            .ok_or_else(|| makernote_error("arithmetic overflow computing curve tag value length"))?;
        let value = if byte_len <= 4 {
            let value_end = at
                .checked_add(8)
                .and_then(|start| start.checked_add(byte_len))
                .ok_or_else(|| makernote_error("arithmetic overflow computing inline curve value"))?;
            makernote
                .get(at + 8..value_end)
                .ok_or_else(|| makernote_error("inline curve tag value is truncated"))?
        } else {
            let offset = usize::try_from(order.u32(&makernote[at + 8..at + 12]))
                .map_err(|_| makernote_error("curve tag value offset does not fit in usize"))?;
            let value_at = base
                .checked_add(offset)
                .ok_or_else(|| makernote_error("arithmetic overflow computing curve tag value offset"))?;
            let value_end = value_at
                .checked_add(byte_len)
                .ok_or_else(|| makernote_error("arithmetic overflow computing curve tag value end"))?;
            makernote.get(value_at..value_end).ok_or_else(|| {
                makernote_error(format!(
                    "curve tag {tag:#06x} value of {byte_len} bytes at offset {value_at} exceeds the {}-byte makernote",
                    makernote.len()
                ))
            })?
        };
        found = Some(value);
    }
    found
        .map(|value| (value, order))
        .ok_or_else(|| makernote_error("no NikonCurve/LinearizationTable tag (0x8C/0x96)"))
}

/// TIFF field-type width used by the makernote mini-IFD parser.
const fn makernote_type_width(type_code: u16) -> Option<usize> {
    match type_code {
        1 | 2 | 6 | 7 => Some(1),
        3 | 8 => Some(2),
        4 | 9 | 11 => Some(4),
        5 | 10 | 12 => Some(8),
        _ => None,
    }
}

/// Reads a `u16` from the curve blob in the makernote byte order.
fn read_curve_u16(
    meta: &[u8],
    order: ByteOrder,
    at: usize,
    context: &'static str,
) -> Result<u16, DecodeError> {
    let end = at
        .checked_add(2)
        .ok_or_else(|| makernote_error(format!("arithmetic overflow reading {context}")))?;
    meta.get(at..end).map(|bytes| order.u16(bytes)).ok_or_else(|| {
        makernote_error(format!(
            "curve blob is truncated reading {context} (need {end} bytes, have {})",
            meta.len()
        ))
    })
}

/// Parsed content of the makernote 0x8C/0x96 compression/curve blob.
#[derive(Debug)]
struct NikonCurve {
    /// Index into [`NIKON_TREE`] for rows before the split.
    huff_select: usize,
    /// Predictor seeds in makernote order: c0r0, c0r1, c1r0, c1r1.
    vpred: [i64; 4],
    /// Linearization table indexed by the reconstructed predictor value.
    curve: Vec<u16>,
    /// First row coded with the "after split" Huffman table; 0 = no split.
    split: usize,
}

/// Parses the 0x8C/0x96 blob following dcraw `nikon_load_raw` /
/// rawspeed `NikonDecompressor::createCurve`.
fn parse_curve_blob(
    meta: &[u8],
    order: ByteOrder,
    bits: u8,
    height: usize,
) -> Result<NikonCurve, DecodeError> {
    let v0 = *meta
        .first()
        .ok_or_else(|| makernote_error("curve blob is empty"))?;
    let v1 = *meta
        .get(1)
        .ok_or_else(|| makernote_error("curve blob is shorter than 2 bytes"))?;
    let mut position = 2_usize;
    if v0 == 0x49 || v1 == 0x58 {
        position = position
            .checked_add(2_110)
            .ok_or_else(|| makernote_error("arithmetic overflow in curve blob version skip"))?;
    }
    let mut huff_select = if v0 == 0x46 { 2 } else { 0 };
    if bits == 14 {
        huff_select += 3;
    }
    let mut vpred = [0_i64; 4];
    for (index, slot) in vpred.iter_mut().enumerate() {
        *slot = i64::from(read_curve_u16(
            meta,
            order,
            position + index * 2,
            "vertical predictor seed",
        )?);
    }
    position += 8;
    // Z-series quirk (rawspeed/rawler): v0=0x44, v1=0x40 files declare 14
    // bits but store 12-bit data.
    let real_bits = if v0 == 0x44 && v1 == 0x40 {
        bits.checked_sub(2)
            .ok_or_else(|| makernote_error("Z-series bit-depth quirk underflows BitsPerSample"))?
    } else {
        bits
    };
    let max = 1_usize << real_bits;
    // `real_bits` is at most 14 (validated 12/14-bit input), so `max` and
    // the interpolation step always fit in u16/u32; the conversions below
    // are infallible in practice but stay checked.
    let max_u16 = u16::try_from(max).map_err(|_| makernote_error("curve size exceeds u16"))?;
    let csize = usize::from(read_curve_u16(meta, order, position, "curve segment count")?);
    position += 2;
    let step = if csize > 1 { max / (csize - 1) } else { 0 };

    let mut split = 0_usize;
    let curve = if v0 == 0x44 && (v1 == 0x20 || v1 == 0x40) && step > 0 {
        // Piecewise-linear interpolated curve with a trailing sentinel slot.
        let mut points: Vec<u16> = Vec::new();
        points
            .try_reserve_exact(max + 1)
            .map_err(|_| makernote_error(format!("could not allocate {} curve points", max + 1)))?;
        points.extend(0..=max_u16);
        for k in 0..csize {
            points[k * step] = read_curve_u16(meta, order, position + 2 * k, "curve anchor")?;
        }
        let step32 = u32::try_from(step).map_err(|_| makernote_error("curve step exceeds u32"))?;
        for i in 0..max {
            let b_scale = i % step;
            let a_pos = i - b_scale;
            let b_scale32 =
                u32::try_from(b_scale).map_err(|_| makernote_error("curve weight exceeds u32"))?;
            let interpolated = ((step32 - b_scale32) * u32::from(points[a_pos])
                + b_scale32 * u32::from(points[a_pos + step]))
                / step32;
            points[i] = u16::try_from(interpolated)
                .map_err(|_| makernote_error("interpolated curve value exceeds u16"))?;
        }
        split = usize::from(read_curve_u16(meta, order, 562, "split row")?);
        // A split outside the image does not actually happen (rawspeed).
        if split >= height {
            split = 0;
        }
        points.truncate(max);
        points
    } else if v0 != 0x46 {
        // Explicit curve table.
        if csize == 0 || csize > 0x4001 {
            return Err(makernote_error(format!(
                "cannot build the linearization curve from segment count {csize}"
            )));
        }
        let mut table = Vec::new();
        table
            .try_reserve_exact(csize)
            .map_err(|_| makernote_error(format!("could not allocate {csize} curve values")))?;
        for k in 0..csize {
            table.push(read_curve_u16(meta, order, position + 2 * k, "curve value")?);
        }
        table
    } else {
        // v0 == 0x46: lossless, the curve is the identity and the blob's
        // curve fields are not read (dcraw/rawspeed).
        let mut identity = Vec::new();
        identity
            .try_reserve_exact(max)
            .map_err(|_| makernote_error(format!("could not allocate {max} curve values")))?;
        identity.extend(0..max_u16);
        identity
    };
    Ok(NikonCurve {
        huff_select,
        vpred,
        curve,
        split,
    })
}

/// MSB-first bit reader over the bare entropy stream (no markers, no byte
/// stuffing).
struct MsbReader<'a> {
    bytes: &'a [u8],
    next: usize,
    buffer: u64,
    available: u8,
}

impl<'a> MsbReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            next: 0,
            buffer: 0,
            available: 0,
        }
    }

    #[inline]
    fn fill(&mut self) {
        while self.available <= 56 && self.next < self.bytes.len() {
            self.buffer = (self.buffer << 8) | u64::from(self.bytes[self.next]);
            self.next += 1;
            self.available += 8;
        }
    }

    /// Returns the next `count` bits right-padded with zeros when the stream
    /// is exhausted, plus how many real bits were actually available.
    #[inline]
    fn peek(&mut self, count: u8) -> (u32, u8) {
        debug_assert!(count <= 16);
        self.fill();
        let real = self.available.min(count);
        let value = if real == 0 {
            0
        } else {
            u32::try_from(self.buffer >> (self.available - real)).unwrap_or(u32::MAX) << (count - real)
        };
        (value & ((1_u32 << count) - 1), real)
    }

    #[inline]
    fn consume(&mut self, count: u8) {
        debug_assert!(count <= self.available);
        self.available -= count;
        self.buffer &= if self.available == 0 {
            0
        } else {
            (1_u64 << self.available) - 1
        };
    }

    #[inline]
    fn read_bits(&mut self, count: u8) -> Result<u32, DecodeError> {
        debug_assert!(count <= 16);
        self.fill();
        if self.available < count {
            return Err(camera_error(
                NEF,
                format!(
                    "Nikon lossless entropy data is truncated: need {count} bits, have {} at byte {}",
                    self.available, self.next
                ),
            ));
        }
        let value = u32::try_from(self.buffer >> (self.available - count)).unwrap_or(u32::MAX)
            & ((1_u32 << count) - 1);
        self.consume(count);
        Ok(value)
    }
}

/// Canonical Huffman decoding table built from DHT-layout counts+symbols,
/// as a direct lookup indexed by the next `max_len` stream bits.
struct NikonHuffman {
    max_len: u8,
    table: Vec<u32>,
}

impl NikonHuffman {
    fn build(counts: &[u8; 16], symbols: &[u8]) -> Result<Self, DecodeError> {
        let declared = counts.iter().map(|count| usize::from(*count)).sum::<usize>();
        if declared != symbols.len() {
            return Err(camera_error(
                NEF,
                format!(
                    "internal error: Nikon Huffman table declares {declared} symbols but provides {}",
                    symbols.len()
                ),
            ));
        }
        let max_len = (1..=16_u8)
            .rev()
            .find(|&length| counts[usize::from(length - 1)] != 0)
            .ok_or_else(|| camera_error(NEF, "internal error: Nikon Huffman table is empty"))?;
        let size = 1_usize << max_len;
        let mut table = Vec::new();
        table
            .try_reserve_exact(size)
            .map_err(|_| camera_error(NEF, format!("could not allocate {size} Huffman slots")))?;
        table.resize(size, 0);
        let mut code = 0_u32;
        let mut symbol_index = 0_usize;
        for length in 1..=16_u8 {
            let count = u32::from(counts[usize::from(length - 1)]);
            let limit = 1_u32 << length;
            if code.checked_add(count).is_none_or(|end| end > limit) {
                return Err(camera_error(
                    NEF,
                    format!("internal error: Nikon Huffman table is oversubscribed at code length {length}"),
                ));
            }
            for _ in 0..count {
                let packed = (u32::from(length) << 8) | u32::from(symbols[symbol_index]);
                symbol_index += 1;
                let start = usize::try_from(code << (max_len - length))
                    .map_err(|_| camera_error(NEF, "internal error: Huffman slot index overflow"))?;
                let fill = 1_usize << (max_len - length);
                table[start..start + fill].fill(packed);
                code += 1;
            }
            code <<= 1;
        }
        Ok(Self { max_len, table })
    }

    #[inline]
    fn decode_symbol(&self, reader: &mut MsbReader<'_>) -> Result<u8, DecodeError> {
        let (bits, real) = reader.peek(self.max_len);
        let packed = self.table[usize::try_from(bits).unwrap_or(0)];
        let length = u8::try_from(packed >> 8)
            .map_err(|_| camera_error(NEF, "internal error: Huffman code length overflow"))?;
        if length == 0 {
            return Err(camera_error(
                NEF,
                "Nikon lossless entropy data contains an invalid Huffman code",
            ));
        }
        if length > real {
            return Err(camera_error(
                NEF,
                "Nikon lossless entropy data is truncated in the middle of a Huffman code",
            ));
        }
        reader.consume(length);
        u8::try_from(packed & 0xff).map_err(|_| camera_error(NEF, "internal error: Huffman symbol overflow"))
    }
}

/// Decodes one Nikon difference value: the Huffman symbol packs the
/// difference category in its low nibble and a post-shift in its high
/// nibble (zero for the lossless tables), per dcraw `nikon_load_raw`.
fn decode_nikon_diff(reader: &mut MsbReader<'_>, huffman: &NikonHuffman) -> Result<i64, DecodeError> {
    let symbol = huffman.decode_symbol(reader)?;
    let length = symbol & 0x0f;
    let shift = symbol >> 4;
    if length == 0 {
        return Ok(0);
    }
    if shift > length {
        return Err(camera_error(
            NEF,
            format!("internal error: Nikon Huffman symbol {symbol:#04x} has shift above its length"),
        ));
    }
    let raw = reader.read_bits(length - shift)?;
    let mut diff = i64::from((((raw << 1) + 1) << shift) >> 1);
    if diff & (1_i64 << (length - 1)) == 0 {
        diff -= (1_i64 << length) - i64::from(shift == 0);
    }
    Ok(diff)
}

fn read_cfa(raw: &CameraDirectory<'_>) -> Result<CfaPattern, DecodeError> {
    let dims_entry = raw
        .entry(NEF, tags::CFA_REPEAT_PATTERN_DIM)?
        .ok_or_else(|| camera_error(NEF, "raw IFD lacks CFARepeatPatternDim (33421)"))?;
    let dims = dims_entry
        .unsigned_values()
        .map_err(|error| camera_error(NEF, format!("CFARepeatPatternDim: {error}")))?;
    let [rows, columns] = dims.as_slice() else {
        return Err(camera_error(
            NEF,
            format!("CFARepeatPatternDim has {} values, expected 2", dims.len()),
        ));
    };
    let expected = usize::try_from(
        rows.checked_mul(*columns)
            .ok_or_else(|| camera_error(NEF, "arithmetic overflow computing CFA pattern cell count"))?,
    )
    .map_err(|_| camera_error(NEF, "CFA pattern cell count does not fit in usize"))?;

    let pattern_entry = raw
        .entry(NEF, tags::CFA_PATTERN)?
        .ok_or_else(|| camera_error(NEF, "raw IFD lacks CFAPattern (33422)"))?;
    let pattern = pattern_entry
        .unsigned_values()
        .map_err(|error| camera_error(NEF, format!("CFAPattern: {error}")))?;
    if pattern.len() != expected {
        return Err(camera_error(
            NEF,
            format!("CFAPattern has {} cells, expected {expected}", pattern.len()),
        ));
    }
    let cells = pattern
        .iter()
        .map(|&value| match value {
            0 => Ok(CfaColor::Red),
            1 => Ok(CfaColor::Green),
            2 => Ok(CfaColor::Blue),
            3 => Ok(CfaColor::Cyan),
            4 => Ok(CfaColor::Magenta),
            5 => Ok(CfaColor::Yellow),
            6 => Ok(CfaColor::White),
            actual => Err(camera_error(
                NEF,
                format!("unknown CFAPattern color index {actual}"),
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let cfa = CfaPattern {
        width: u8::try_from(*columns).map_err(|_| camera_error(NEF, "CFA pattern width exceeds 255"))?,
        height: u8::try_from(*rows).map_err(|_| camera_error(NEF, "CFA pattern height exceeds 255"))?,
        cells,
    };
    cfa.bayer_quad().map_err(|_| {
        camera_error(
            NEF,
            "unsupported CFA pattern: the display pipeline currently requires a 2x2 RGB Bayer CFA",
        )
    })?;
    Ok(cfa)
}

fn to_u32(value: u64, context: &'static str) -> Result<u32, DecodeError> {
    u32::try_from(value).map_err(|_| camera_error(NEF, format!("{context} value {value} exceeds u32")))
}

fn to_u8(value: u64, context: &'static str) -> Result<u8, DecodeError> {
    u8::try_from(value).map_err(|_| camera_error(NEF, format!("{context} value {value} exceeds u8")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camtiff::CameraQuirks;

    // ------------------------------------------------------------------
    // Synthetic NEF builder (programmatic, no camera files).
    // ------------------------------------------------------------------

    /// One TIFF entry: tag, field type code, and value bytes already encoded
    /// in the file byte order. `count` is derived from the type width.
    #[derive(Clone)]
    struct TestEntry {
        tag: u16,
        field_type: u16,
        data: Vec<u8>,
    }

    const fn type_width(field_type: u16) -> usize {
        match field_type {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 => 4,
            _ => 8,
        }
    }

    fn w16(le: bool, value: u16) -> [u8; 2] {
        if le {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        }
    }

    fn w32(le: bool, value: u32) -> [u8; 4] {
        if le {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        }
    }

    fn short_entry(le: bool, tag: u16, value: u16) -> TestEntry {
        TestEntry {
            tag,
            field_type: 3,
            data: w16(le, value).to_vec(),
        }
    }

    fn long_entry(le: bool, tag: u16, value: u32) -> TestEntry {
        TestEntry {
            tag,
            field_type: 4,
            data: w32(le, value).to_vec(),
        }
    }

    fn ascii_entry(tag: u16, text: &str) -> TestEntry {
        let mut data = text.as_bytes().to_vec();
        data.push(0);
        TestEntry {
            tag,
            field_type: 2,
            data,
        }
    }

    fn bytes_entry(tag: u16, data: &[u8]) -> TestEntry {
        TestEntry {
            tag,
            field_type: 1,
            data: data.to_vec(),
        }
    }

    fn shorts_entry(le: bool, tag: u16, values: &[u16]) -> TestEntry {
        let mut data = Vec::new();
        for &value in values {
            data.extend_from_slice(&w16(le, value));
        }
        TestEntry {
            tag,
            field_type: 3,
            data,
        }
    }

    fn longs_entry(le: bool, tag: u16, values: &[u32]) -> TestEntry {
        let mut data = Vec::new();
        for &value in values {
            data.extend_from_slice(&w32(le, value));
        }
        TestEntry {
            tag,
            field_type: 4,
            data,
        }
    }

    /// Assembles a classic TIFF with IFD0 at offset 8 and one `SubIFD`.
    ///
    /// `entries` receives the `SubIFD` offset and returns `(ifd0, raw)`
    /// entries; the `StripOffsets` (273) entry in the raw IFD is patched by
    /// the builder from `strip_lengths`, so callers fill it with zeros.
    /// Pixel bytes are appended last.
    fn assemble_nef(
        le: bool,
        strip_lengths: &[usize],
        pixels: &[u8],
        entries: impl Fn(u32) -> (Vec<TestEntry>, Vec<TestEntry>),
    ) -> Vec<u8> {
        let (ifd0_probe, _) = entries(0);
        let ifd0_size = 2 + 12 * ifd0_probe.len() + 4;
        let sub_at = u32::try_from(8 + ifd0_size).unwrap();
        let (ifd0, mut raw) = entries(sub_at);
        let raw_size = 2 + 12 * raw.len() + 4;
        let values_start = usize::try_from(sub_at).unwrap() + raw_size;

        // Assign out-of-line value offsets to the IFD0 entries first; IFD0
        // never changes after this point.
        let mut cursor = values_start;
        let mut ifd0_value_offsets = Vec::new();
        for entry in &ifd0 {
            if entry.data.len() > 4 {
                ifd0_value_offsets.push(Some(u32::try_from(cursor).unwrap()));
                cursor += entry.data.len();
                // Keep 2-byte alignment for SHORT arrays.
                cursor += cursor % 2;
            } else {
                ifd0_value_offsets.push(None);
            }
        }
        let raw_values_start = cursor;

        // Patch the StripOffsets entry, then lay out the raw values region.
        // Patching can switch the entry between inline and out-of-line, so
        // iterate until the pixel offset is stable (two passes always
        // suffice: the patched entry length only depends on the strip count).
        let mut pixel_at = usize::MAX;
        let mut raw_value_offsets = vec![None; raw.len()];
        let mut converged = false;
        for _ in 0..4 {
            let mut cursor = raw_values_start;
            raw_value_offsets.clear();
            for entry in &raw {
                if entry.data.len() > 4 {
                    raw_value_offsets.push(Some(u32::try_from(cursor).unwrap()));
                    cursor += entry.data.len();
                    cursor += cursor % 2;
                } else {
                    raw_value_offsets.push(None);
                }
            }
            if cursor == pixel_at {
                converged = true;
                break;
            }
            pixel_at = cursor;
            let mut strip_offsets = Vec::new();
            let mut at = u32::try_from(pixel_at).unwrap();
            for &length in strip_lengths {
                strip_offsets.push(at);
                at = at.checked_add(u32::try_from(length).unwrap()).unwrap();
            }
            for entry in &mut raw {
                if entry.tag == 273 {
                    entry.data = longs_entry(le, 273, &strip_offsets).data;
                }
            }
        }
        assert!(converged, "strip-offset patching did not converge");

        let mut bytes = vec![0_u8; pixel_at + pixels.len()];
        if le {
            bytes[..8].copy_from_slice(&[b'I', b'I', 42, 0, 8, 0, 0, 0]);
        } else {
            bytes[..8].copy_from_slice(&[b'M', b'M', 0, 42, 0, 0, 0, 8]);
        }
        write_ifd(&mut bytes, le, 8, &ifd0, &ifd0_value_offsets, 0);
        write_ifd(
            &mut bytes,
            le,
            usize::try_from(sub_at).unwrap(),
            &raw,
            &raw_value_offsets,
            0,
        );
        // Write out-of-line value blobs.
        for (table, offsets) in [(&ifd0, &ifd0_value_offsets), (&raw, &raw_value_offsets)] {
            for (entry, &offset) in table.iter().zip(offsets.iter()) {
                if let Some(at) = offset {
                    let at = usize::try_from(at).unwrap();
                    bytes[at..at + entry.data.len()].copy_from_slice(&entry.data);
                }
            }
        }
        bytes[pixel_at..pixel_at + pixels.len()].copy_from_slice(pixels);
        bytes
    }

    fn write_ifd(
        bytes: &mut [u8],
        le: bool,
        at: usize,
        entries: &[TestEntry],
        value_offsets: &[Option<u32>],
        next: u32,
    ) {
        bytes[at..at + 2].copy_from_slice(&w16(le, u16::try_from(entries.len()).unwrap()));
        for (index, (entry, &value_offset)) in entries.iter().zip(value_offsets.iter()).enumerate() {
            let start = at + 2 + index * 12;
            bytes[start..start + 2].copy_from_slice(&w16(le, entry.tag));
            bytes[start + 2..start + 4].copy_from_slice(&w16(le, entry.field_type));
            let count = u32::try_from(entry.data.len() / type_width(entry.field_type)).unwrap();
            bytes[start + 4..start + 8].copy_from_slice(&w32(le, count));
            if let Some(offset) = value_offset {
                bytes[start + 8..start + 12].copy_from_slice(&w32(le, offset));
            } else {
                let mut field = [0_u8; 4];
                field[..entry.data.len()].copy_from_slice(&entry.data);
                bytes[start + 8..start + 12].copy_from_slice(&field);
            }
        }
        let next_at = at + 2 + entries.len() * 12;
        bytes[next_at..next_at + 4].copy_from_slice(&w32(le, next));
    }

    /// MSB-first packed-row encoder, the inverse of `decode_msb_packed`.
    fn encode_msb(samples: &[u16], bits_per_sample: u8) -> Vec<u8> {
        let row_bytes = (samples.len() * usize::from(bits_per_sample)).div_ceil(8);
        let mut encoded = vec![0_u8; row_bytes];
        let mut bit_position = 0_usize;
        for &sample in samples {
            for shift in (0..bits_per_sample).rev() {
                if (sample >> shift) & 1 == 1 {
                    encoded[bit_position / 8] |= 1 << (7 - (bit_position % 8));
                }
                bit_position += 1;
            }
        }
        encoded
    }

    /// Standard raw-IFD entries for a synthetic uncompressed NEF.
    /// `strip_count` selects an inline (1) or out-of-line (>1) `StripOffsets`
    /// entry; the builder patches the actual offsets.
    #[allow(clippy::too_many_arguments)]
    fn raw_entries(
        le: bool,
        width: u32,
        height: u32,
        bits: u16,
        compression: u16,
        strip_count: usize,
        strip_bytes: &[u32],
        rows_per_strip: u32,
        cfa_cells: Option<[u8; 4]>,
    ) -> Vec<TestEntry> {
        let mut entries = vec![
            long_entry(le, 254, 0),      // SubFileType: primary
            long_entry(le, 256, width),  // ImageWidth
            long_entry(le, 257, height), // ImageLength
            short_entry(le, 258, bits),  // BitsPerSample
            short_entry(le, 259, compression),
            short_entry(le, 262, 32_803), // PhotometricInterpretation: CFA
            longs_entry(le, 273, &vec![0; strip_count]),
            short_entry(le, 277, 1), // SamplesPerPixel
            long_entry(le, 278, rows_per_strip),
            longs_entry(le, 279, strip_bytes),
        ];
        if let Some(cells) = cfa_cells {
            entries.push(shorts_entry(le, 33_421, &[2, 2]));
            entries.push(bytes_entry(33_422, &cells));
        }
        entries
    }

    fn ifd0_entries(le: bool, sub_at: u32) -> Vec<TestEntry> {
        vec![
            long_entry(le, 254, 1), // reduced-resolution preview
            ascii_entry(271, "NIKON CORPORATION"),
            ascii_entry(272, "NIKON D TEST"),
            short_entry(le, 274, 1),
            long_entry(le, 330, sub_at), // SubIFDs
        ]
    }

    /// Builds a valid synthetic NEF with one strip of 12-bit packed data.
    fn synthetic_nef(le: bool) -> (Vec<u8>, Vec<u16>) {
        let width = 5_u32;
        let height = 4_u32;
        let samples: Vec<u16> = (0..width * height)
            .map(|index| u16::try_from((index * 173 + 11) % 4_096).unwrap())
            .collect();
        let mut pixels = Vec::new();
        for row in samples.chunks(width as usize) {
            let encoded = encode_msb(row, 12);
            pixels.extend_from_slice(&encoded);
        }
        let strip_lengths = [pixels.len()];
        let strip_total = u32::try_from(pixels.len()).unwrap();
        let bytes = assemble_nef(le, &strip_lengths, &pixels, |sub_at| {
            (
                ifd0_entries(le, sub_at),
                raw_entries(
                    le,
                    width,
                    height,
                    12,
                    1,
                    1,
                    &[strip_total],
                    height,
                    Some([0, 1, 1, 2]),
                ),
            )
        });
        (bytes, samples)
    }

    fn quirks() -> NefQuirks {
        NefQuirks
    }

    // ------------------------------------------------------------------
    // Tests
    // ------------------------------------------------------------------

    #[test]
    fn decodes_little_endian_packed_12bit_round_trip() {
        let (bytes, samples) = synthetic_nef(true);
        let quirks = quirks();
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();

        let metadata = quirks.read_metadata(&container, raw).unwrap();
        assert_eq!(metadata.make, "NIKON CORPORATION");
        assert_eq!(metadata.model, "NIKON D TEST");
        assert_eq!(metadata.orientation, Orientation::Normal);
        assert_eq!((metadata.width, metadata.height), (5, 4));
        assert_eq!(metadata.bits_per_sample, 12);
        assert_eq!(
            metadata.cfa.cells,
            [CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue]
        );
        // Exact constants are intended: the documented fallback is exactly
        // [1.0; 4] with no computation involved.
        #[allow(clippy::float_cmp)]
        {
            assert_eq!(metadata.black_level.values, [0.0]);
            assert_eq!(metadata.white_level.0, [4_095.0]);
            assert_eq!(metadata.white_balance, [1.0, 1.0, 1.0, 1.0]);
        }

        let pixels = quirks.decode_pixels(&container, raw, &|| false).unwrap();
        assert_eq!(pixels, samples);
    }

    #[test]
    fn decodes_big_endian_packed_12bit_round_trip() {
        let (bytes, samples) = synthetic_nef(false);
        let quirks = quirks();
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let metadata = quirks.read_metadata(&container, raw).unwrap();
        assert_eq!(metadata.make, "NIKON CORPORATION");
        let pixels = quirks.decode_pixels(&container, raw, &|| false).unwrap();
        assert_eq!(pixels, samples);
    }

    #[test]
    fn decodes_multi_strip_14bit() {
        let le = true;
        let width = 6_u32;
        let height = 4_u32;
        let samples: Vec<u16> = (0..width * height)
            .map(|index| u16::try_from((index * 977 + 5) % 16_384).unwrap())
            .collect();
        let mut pixels = Vec::new();
        let mut strip_lengths = Vec::new();
        let mut strip_bytes = Vec::new();
        // Two strips of two rows each.
        for strip_samples in samples.chunks((width * 2) as usize) {
            let mut strip_data = Vec::new();
            for row in strip_samples.chunks(width as usize) {
                strip_data.extend_from_slice(&encode_msb(row, 14));
            }
            strip_lengths.push(strip_data.len());
            strip_bytes.push(u32::try_from(strip_data.len()).unwrap());
            pixels.extend_from_slice(&strip_data);
        }
        let bytes = assemble_nef(le, &strip_lengths, &pixels, |sub_at| {
            (
                ifd0_entries(le, sub_at),
                raw_entries(le, width, height, 14, 1, 2, &strip_bytes, 2, Some([1, 0, 2, 1])),
            )
        });
        let quirks = quirks();
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let metadata = quirks.read_metadata(&container, raw).unwrap();
        assert_eq!(metadata.white_level.0, [16_383.0]);
        assert_eq!(
            metadata.cfa.cells,
            [CfaColor::Green, CfaColor::Red, CfaColor::Blue, CfaColor::Green]
        );
        let decoded = quirks.decode_pixels(&container, raw, &|| false).unwrap();
        assert_eq!(decoded, samples);
    }

    #[test]
    fn selects_the_sub_ifd_raw_directory() {
        let (bytes, _) = synthetic_nef(true);
        let quirks = quirks();
        let container = quirks.parse_container(&bytes).unwrap();
        assert_eq!(container.directories().len(), 2);
        let raw = quirks.select_raw_ifd(&container).unwrap();
        assert!(!raw.is_top_level());
    }

    #[test]
    fn rejects_nikon_lossless_without_makernote() {
        let (mut bytes, _) = synthetic_nef(true);
        patch_compression(&mut bytes, true, 34_713);
        let quirks = quirks();
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.decode_pixels(&container, raw, &|| false).unwrap_err();
        match error {
            DecodeError::NativeCamera { format, message } => {
                assert_eq!(format, "NEF");
                assert!(message.contains("makernote"), "unexpected message: {message}");
                assert!(message.contains("ExifIFD"), "unexpected message: {message}");
            }
            other => panic!("expected typed NativeCamera error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_other_compressions_with_typed_error() {
        let (mut bytes, _) = synthetic_nef(true);
        patch_compression(&mut bytes, true, 8); // e.g. deflate / lossy family
        let quirks = quirks();
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.decode_pixels(&container, raw, &|| false).unwrap_err();
        match error {
            DecodeError::NativeCamera { format, message } => {
                assert_eq!(format, "NEF");
                assert!(message.contains("compression 8"), "unexpected message: {message}");
            }
            other => panic!("expected typed NativeCamera error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_unsupported_bit_depth() {
        let le = true;
        let samples = [0_u16; 4];
        let pixels = encode_msb(&samples, 8);
        let strip_total = u32::try_from(pixels.len()).unwrap();
        let bytes = assemble_nef(le, &[pixels.len()], &pixels, |sub_at| {
            (
                ifd0_entries(le, sub_at),
                raw_entries(le, 4, 1, 8, 1, 1, &[strip_total], 1, Some([0, 1, 1, 2])),
            )
        });
        let quirks = quirks();
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.decode_pixels(&container, raw, &|| false).unwrap_err();
        assert!(
            matches!(&error, DecodeError::NativeCamera { message, .. } if message.contains("BitsPerSample 8")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn rejects_missing_cfa_tags() {
        let le = true;
        let samples = [1_u16, 2, 3, 4];
        let pixels = encode_msb(&samples, 12);
        let strip_total = u32::try_from(pixels.len()).unwrap();
        let bytes = assemble_nef(le, &[pixels.len()], &pixels, |sub_at| {
            (
                ifd0_entries(le, sub_at),
                raw_entries(le, 4, 1, 12, 1, 1, &[strip_total], 1, None),
            )
        });
        let quirks = quirks();
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.read_metadata(&container, raw).unwrap_err();
        assert!(
            matches!(&error, DecodeError::NativeCamera { message, .. } if message.contains("CFARepeatPatternDim")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn rejects_non_rgb_bayer_cfa() {
        let le = true;
        let samples = [1_u16, 2, 3, 4];
        let pixels = encode_msb(&samples, 12);
        let strip_total = u32::try_from(pixels.len()).unwrap();
        let bytes = assemble_nef(le, &[pixels.len()], &pixels, |sub_at| {
            (
                ifd0_entries(le, sub_at),
                raw_entries(le, 4, 1, 12, 1, 1, &[strip_total], 1, Some([1, 1, 1, 1])),
            )
        });
        let quirks = quirks();
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.read_metadata(&container, raw).unwrap_err();
        assert!(
            matches!(&error, DecodeError::NativeCamera { message, .. } if message.contains("2x2 RGB Bayer")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn honors_cancelled_callback() {
        let (bytes, _) = synthetic_nef(true);
        let quirks = quirks();
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.decode_pixels(&container, raw, &|| true).unwrap_err();
        assert!(matches!(error, DecodeError::Cancelled));
    }

    #[test]
    fn rejects_truncated_strip_data() {
        let (bytes, _) = synthetic_nef(true);
        // Truncate the file inside the pixel payload.
        let truncated = bytes[..bytes.len() - 2].to_vec();
        let quirks = quirks();
        let container = quirks.parse_container(&truncated).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.decode_pixels(&container, raw, &|| false).unwrap_err();
        assert!(
            matches!(&error, DecodeError::NativeCamera { message, .. } if message.contains("truncated")),
            "unexpected error: {error:?}"
        );
    }

    /// Rewrites the Compression (259) entry value inside the raw `SubIFD` of a
    /// little-endian synthetic file. Only valid for the single-strip
    /// synthetic layout where the value is an inline SHORT.
    fn patch_compression(bytes: &mut [u8], le: bool, compression: u16) {
        assert!(le, "patch_compression only supports little-endian synthetics");
        let read_u16 = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
        let read_u32 =
            |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
        // Find the SubIFDs (330) entry in IFD0 to locate the raw directory.
        let ifd0_count = usize::from(read_u16(8));
        let mut sub_at = None;
        for index in 0..ifd0_count {
            let start = 8 + 2 + index * 12;
            if read_u16(start) == 330 {
                sub_at = Some(usize::try_from(read_u32(start + 8)).unwrap());
            }
        }
        let sub_at = sub_at.expect("synthetic IFD0 must carry a SubIFDs entry");
        let raw_count = usize::from(read_u16(sub_at));
        for index in 0..raw_count {
            let start = sub_at + 2 + index * 12;
            if read_u16(start) == 259 {
                let encoded = w16(le, compression);
                bytes[start + 8] = encoded[0];
                bytes[start + 9] = encoded[1];
                return;
            }
        }
        panic!("compression entry not found");
    }

    // ------------------------------------------------------------------
    // Synthetic compressed (Compression = 34713) NEF builder.
    // ------------------------------------------------------------------

    /// Nikon makernote layouts covered by the mini-IFD parser.
    #[derive(Clone, Copy)]
    enum MakernoteVariant {
        /// Format 1: plain IFD at offset 0 in the container byte order.
        Plain,
        /// Format 2: `"Nikon\0\x01\x00"` + plain IFD at offset 8 in the
        /// container byte order.
        SignatureV1,
        /// Format 3: `"Nikon\0\x02\x10\0\0"` + embedded TIFF header with its
        /// own byte order (the argument), IFD at offset 18.
        SignatureV2Tiff(bool),
    }

    /// Builds a makernote mini-IFD with UNDEFINED (type 7) entries; values
    /// longer than 4 bytes are stored after the IFD.
    fn build_makernote(container_le: bool, variant: MakernoteVariant, entries: &[(u16, Vec<u8>)]) -> Vec<u8> {
        let (le, base, ifd_at) = match variant {
            MakernoteVariant::Plain => (container_le, 0_usize, 0_usize),
            MakernoteVariant::SignatureV1 => (container_le, 0_usize, 8_usize),
            MakernoteVariant::SignatureV2Tiff(order_le) => (order_le, 10_usize, 18_usize),
        };
        let values_start = ifd_at + 2 + 12 * entries.len() + 4;
        let mut out = vec![0_u8; values_start];
        match variant {
            MakernoteVariant::Plain => {}
            MakernoteVariant::SignatureV1 => out[..8].copy_from_slice(b"Nikon\0\x01\x00"),
            MakernoteVariant::SignatureV2Tiff(order_le) => {
                out[..10].copy_from_slice(b"Nikon\0\x02\x10\x00\x00");
                out[10..12].copy_from_slice(if order_le { b"II" } else { b"MM" });
                out[12..14].copy_from_slice(&w16(order_le, 42));
                out[14..18].copy_from_slice(&w32(order_le, 8));
            }
        }
        out[ifd_at..ifd_at + 2].copy_from_slice(&w16(le, u16::try_from(entries.len()).unwrap()));
        let mut cursor = values_start;
        for (index, (tag, value)) in entries.iter().enumerate() {
            let start = ifd_at + 2 + index * 12;
            out[start..start + 2].copy_from_slice(&w16(le, *tag));
            out[start + 2..start + 4].copy_from_slice(&w16(le, 7));
            out[start + 4..start + 8].copy_from_slice(&w32(le, u32::try_from(value.len()).unwrap()));
            if value.len() <= 4 {
                out[start + 8..start + 8 + value.len()].copy_from_slice(value);
            } else {
                out[start + 8..start + 12].copy_from_slice(&w32(le, u32::try_from(cursor - base).unwrap()));
                out.extend_from_slice(value);
                cursor += value.len();
            }
        }
        out
    }

    /// Lossless curve blob: v0 = 0x46, four predictor seeds, csize = 0.
    /// Multi-byte fields use the makernote byte order.
    fn curve_blob_lossless(mn_le: bool, vpred: [u16; 4]) -> Vec<u8> {
        let mut blob = vec![0x46, 0x00];
        for value in vpred {
            blob.extend_from_slice(&w16(mn_le, value));
        }
        blob.extend_from_slice(&w16(mn_le, 0));
        blob
    }

    /// Explicit-table curve blob: v0 = 0x44 with v1 outside {0x20, 0x40}.
    fn curve_blob_explicit(mn_le: bool, v1: u8, vpred: [u16; 4], table: &[u16]) -> Vec<u8> {
        let mut blob = vec![0x44, v1];
        for value in vpred {
            blob.extend_from_slice(&w16(mn_le, value));
        }
        blob.extend_from_slice(&w16(mn_le, u16::try_from(table.len()).unwrap()));
        for &value in table {
            blob.extend_from_slice(&w16(mn_le, value));
        }
        blob
    }

    /// Interpolated-curve blob: v0 = 0x44, v1 = 0x20, anchors plus the split
    /// row at the absolute blob offset 562.
    fn curve_blob_interpolated(mn_le: bool, vpred: [u16; 4], anchors: &[u16], split: u16) -> Vec<u8> {
        let mut blob = vec![0x44, 0x20];
        for value in vpred {
            blob.extend_from_slice(&w16(mn_le, value));
        }
        blob.extend_from_slice(&w16(mn_le, u16::try_from(anchors.len()).unwrap()));
        for &value in anchors {
            blob.extend_from_slice(&w16(mn_le, value));
        }
        blob.resize(562, 0);
        blob.extend_from_slice(&w16(mn_le, split));
        blob
    }

    /// Assembles a NEF with a `Compression = 34713` raw `SubIFD` and, when a
    /// makernote is given, the IFD0 → `ExifIFD` (0x8769) → `MakerNote` (0x927C)
    /// chain. Layout: header, IFD0, raw `SubIFD`, optional Exif IFD, then the
    /// value region (make/model strings, makernote, multi-strip offset and
    /// byte-count arrays, strip data).
    // Linear byte-layout builder; the offsets only make sense read top to
    // bottom, so splitting it would hurt clarity.
    #[allow(clippy::too_many_lines)]
    fn assemble_compressed_nef(
        le: bool,
        width: u32,
        height: u32,
        bits: u16,
        makernote: Option<&[u8]>,
        strips: &[&[u8]],
    ) -> Vec<u8> {
        let ifd0_count = if makernote.is_some() { 5_usize } else { 4 };
        let raw_at = 8 + 2 + 12 * ifd0_count + 4;
        let exif_at = raw_at + 2 + 12 * 12 + 4;
        let mut cursor = if makernote.is_some() {
            exif_at + 2 + 12 + 4
        } else {
            exif_at
        };
        cursor += cursor % 2;

        let make = b"NIKON\0".as_slice();
        let model = b"NIKON D TEST\0".as_slice();
        let make_at = cursor;
        cursor += make.len();
        cursor += cursor % 2;
        let model_at = cursor;
        cursor += model.len();
        cursor += cursor % 2;
        let makernote_at = makernote.map(|blob| {
            let at = cursor;
            cursor += blob.len();
            cursor += cursor % 2;
            at
        });
        let multi = strips.len() > 1;
        let strip_offsets_at = multi.then_some(cursor);
        if multi {
            cursor += 4 * strips.len();
        }
        let byte_counts_at = multi.then_some(cursor);
        if multi {
            cursor += 4 * strips.len();
        }
        let mut strip_offsets = Vec::new();
        for strip in strips {
            strip_offsets.push(u32::try_from(cursor).unwrap());
            cursor += strip.len();
        }
        let strip_byte_counts: Vec<u32> = strips
            .iter()
            .map(|strip| u32::try_from(strip.len()).unwrap())
            .collect();

        let mut bytes = vec![0_u8; cursor];
        if le {
            bytes[..8].copy_from_slice(&[b'I', b'I', 42, 0, 8, 0, 0, 0]);
        } else {
            bytes[..8].copy_from_slice(&[b'M', b'M', 0, 42, 0, 0, 0, 8]);
        }

        let mut ifd0 = vec![
            TestEntry {
                tag: 271,
                field_type: 2,
                data: make.to_vec(),
            },
            TestEntry {
                tag: 272,
                field_type: 2,
                data: model.to_vec(),
            },
            short_entry(le, 274, 1),
            long_entry(le, 330, u32::try_from(raw_at).unwrap()),
        ];
        let mut ifd0_offsets = vec![
            Some(u32::try_from(make_at).unwrap()),
            Some(u32::try_from(model_at).unwrap()),
            None,
            None,
        ];
        if makernote.is_some() {
            ifd0.push(long_entry(le, 0x8769, u32::try_from(exif_at).unwrap()));
            ifd0_offsets.push(None);
        }
        write_ifd(&mut bytes, le, 8, &ifd0, &ifd0_offsets, 0);

        let raw = vec![
            long_entry(le, 254, 0),
            long_entry(le, 256, width),
            long_entry(le, 257, height),
            short_entry(le, 258, bits),
            short_entry(le, 259, 34_713),
            short_entry(le, 262, 32_803),
            longs_entry(le, 273, &strip_offsets),
            short_entry(le, 277, 1),
            long_entry(le, 278, height),
            longs_entry(le, 279, &strip_byte_counts),
            shorts_entry(le, 33_421, &[2, 2]),
            bytes_entry(33_422, &[0, 1, 1, 2]),
        ];
        let raw_offsets: Vec<Option<u32>> = raw
            .iter()
            .map(|entry| {
                if entry.data.len() > 4 {
                    let at = match entry.tag {
                        273 => strip_offsets_at.unwrap(),
                        279 => byte_counts_at.unwrap(),
                        other => panic!("unexpected out-of-line raw entry {other}"),
                    };
                    Some(u32::try_from(at).unwrap())
                } else {
                    None
                }
            })
            .collect();
        write_ifd(&mut bytes, le, raw_at, &raw, &raw_offsets, 0);

        if let (Some(blob), Some(at)) = (makernote, makernote_at) {
            let exif = vec![TestEntry {
                tag: 0x927c,
                field_type: 7,
                data: blob.to_vec(),
            }];
            write_ifd(
                &mut bytes,
                le,
                exif_at,
                &exif,
                &[Some(u32::try_from(at).unwrap())],
                0,
            );
            bytes[at..at + blob.len()].copy_from_slice(blob);
        }

        bytes[make_at..make_at + make.len()].copy_from_slice(make);
        bytes[model_at..model_at + model.len()].copy_from_slice(model);
        if let Some(at) = strip_offsets_at {
            for (index, &offset) in strip_offsets.iter().enumerate() {
                bytes[at + 4 * index..at + 4 * index + 4].copy_from_slice(&w32(le, offset));
            }
        }
        if let Some(at) = byte_counts_at {
            for (index, &count) in strip_byte_counts.iter().enumerate() {
                bytes[at + 4 * index..at + 4 * index + 4].copy_from_slice(&w32(le, count));
            }
        }
        for (strip, &offset) in strips.iter().zip(&strip_offsets) {
            let at = usize::try_from(offset).unwrap();
            bytes[at..at + strip.len()].copy_from_slice(strip);
        }
        bytes
    }

    /// MSB-first bit writer, the inverse of [`MsbReader`].
    struct BitSink {
        bytes: Vec<u8>,
        bit_position: usize,
    }

    impl BitSink {
        const fn new() -> Self {
            Self {
                bytes: Vec::new(),
                bit_position: 0,
            }
        }

        fn write(&mut self, value: u32, bits: u8) {
            for shift in (0..bits).rev() {
                if self.bit_position / 8 == self.bytes.len() {
                    self.bytes.push(0);
                }
                if (value >> shift) & 1 == 1 {
                    self.bytes[self.bit_position / 8] |= 1 << (7 - (self.bit_position % 8));
                }
                self.bit_position += 1;
            }
        }

        fn finish(self) -> Vec<u8> {
            self.bytes
        }
    }

    /// Canonical Huffman codes from a DHT-layout table as `(symbol, code,
    /// length)`, mirroring the code assignment of `NikonHuffman::build`.
    fn huff_codes(counts: &[u8; 16], symbols: &[u8]) -> Vec<(u8, u32, u8)> {
        let mut codes = Vec::new();
        let mut code = 0_u32;
        let mut index = 0_usize;
        for length in 1..=16_u8 {
            for _ in 0..counts[usize::from(length - 1)] {
                codes.push((symbols[index], code, length));
                index += 1;
                code += 1;
            }
            code <<= 1;
        }
        codes
    }

    /// Emits one difference with a shift-0 symbol: the category is the bit
    /// length of `|diff|`, exactly inverting `decode_nikon_diff` for shl = 0.
    fn emit_plain(sink: &mut BitSink, codes: &[(u8, u32, u8)], diff: i64) {
        if diff == 0 {
            let &(_, code, length) = codes
                .iter()
                .find(|(symbol, _, _)| symbol.trailing_zeros() >= 4)
                .expect("table must have a zero symbol");
            sink.write(code, length);
            return;
        }
        let category = u8::try_from(64 - diff.unsigned_abs().leading_zeros()).unwrap();
        let &(_, code, length) = codes
            .iter()
            .find(|(symbol, _, _)| symbol & 15 == category && symbol >> 4 == 0)
            .unwrap_or_else(|| panic!("no shift-0 symbol for category {category}"));
        sink.write(code, length);
        let raw = if diff > 0 {
            diff
        } else {
            diff + (1_i64 << category) - 1
        };
        sink.write(u32::try_from(raw).unwrap(), category);
    }

    /// Emits one difference for the lossy tables: plain shift-0 categories,
    /// plus the 0x16 shift symbol (len 6, shl 1) for odd differences with
    /// magnitude 33..=63, inverting `decode_nikon_diff` for shl = 1.
    fn emit_lossy(sink: &mut BitSink, codes: &[(u8, u32, u8)], diff: i64) {
        let magnitude = diff.unsigned_abs();
        if magnitude >= 33 {
            assert!(
                magnitude <= 63 && diff % 2 != 0,
                "test diff {diff} is not representable with the 0x16 symbol"
            );
            let &(_, code, length) = codes
                .iter()
                .find(|(symbol, _, _)| *symbol == 0x16)
                .expect("only the after-split table has the 0x16 symbol");
            sink.write(code, length);
            // Decoder: base = 2*raw + 1; positive diffs are base, negative
            // diffs are base - 64.
            let raw = if diff > 0 {
                (diff - 1) / 2
            } else {
                i64::midpoint(diff, 63)
            };
            sink.write(u32::try_from(raw).unwrap(), 5);
            return;
        }
        emit_plain(sink, codes, diff);
    }

    /// Per-symbol difference emitter plugged into `nikon_encode`.
    type DiffEmitter = dyn Fn(&mut BitSink, &[(u8, u32, u8)], i64);

    /// Encodes row-major pre-curve predictor values into the bare Nikon
    /// entropy stream, mirroring the decoder's accumulator structure
    /// (including the split-row table switch). `emit` selects the
    /// per-symbol difference coding.
    fn nikon_encode(
        samples: &[u16],
        width: usize,
        height: usize,
        vpred: [u16; 4],
        main_tree: usize,
        split: usize,
        emit: &DiffEmitter,
    ) -> Vec<u8> {
        let (counts, symbols) = NIKON_TREE[main_tree];
        let main_codes = huff_codes(counts, symbols);
        let split_codes = (split != 0).then(|| {
            let (counts, symbols) = NIKON_TREE[main_tree + 1];
            huff_codes(counts, symbols)
        });
        let mut sink = BitSink::new();
        let mut up = [
            [i64::from(vpred[0]), i64::from(vpred[1])],
            [i64::from(vpred[2]), i64::from(vpred[3])],
        ];
        for row in 0..height {
            let codes = if split != 0 && row >= split {
                split_codes.as_ref().unwrap()
            } else {
                &main_codes
            };
            let row_parity = row & 1;
            let mut left = [0_i64; 2];
            for col in 0..width {
                let column_parity = col & 1;
                let value = i64::from(samples[row * width + col]);
                let predictor = if col < 2 {
                    up[column_parity][row_parity]
                } else {
                    left[column_parity]
                };
                emit(&mut sink, codes, value - predictor);
                if col < 2 {
                    up[column_parity][row_parity] = value;
                }
                left[column_parity] = value;
            }
        }
        sink.finish()
    }

    /// Deterministic pre-curve predictor samples following the decoder's
    /// accumulator structure, with deltas drawn cyclically from `deltas`.
    fn predicted_samples(width: usize, height: usize, vpred: [u16; 4], deltas: &[i64]) -> Vec<u16> {
        let mut samples = vec![0_u16; width * height];
        let mut up = [
            [i64::from(vpred[0]), i64::from(vpred[1])],
            [i64::from(vpred[2]), i64::from(vpred[3])],
        ];
        let mut step = 0_usize;
        for row in 0..height {
            let row_parity = row & 1;
            let mut left = [0_i64; 2];
            for col in 0..width {
                let column_parity = col & 1;
                let predictor = if col < 2 {
                    up[column_parity][row_parity]
                } else {
                    left[column_parity]
                };
                let value = predictor + deltas[step % deltas.len()];
                step += 1;
                samples[row * width + col] = u16::try_from(value).expect("test sample outside the u16 range");
                if col < 2 {
                    up[column_parity][row_parity] = value;
                }
                left[column_parity] = value;
            }
        }
        samples
    }

    /// Valid 12-bit lossless 6x4 stream + format-2 makernote pair reused by
    /// the rejection tests.
    fn lossless_fixture(le: bool) -> (Vec<u8>, Vec<u8>, Vec<u16>) {
        let vpred = [1_000_u16, 1_001, 1_002, 1_003];
        let samples = predicted_samples(6, 4, vpred, &[-4, -1, 0, 2, 5, -3]);
        let stream = nikon_encode(&samples, 6, 4, vpred, 2, 0, &emit_plain);
        let blob = curve_blob_lossless(le, vpred);
        let makernote = build_makernote(le, MakernoteVariant::SignatureV1, &[(0x8c, blob)]);
        (stream, makernote, samples)
    }

    fn decode_compressed(bytes: &[u8]) -> Result<Vec<u16>, DecodeError> {
        let quirks = quirks();
        let container = quirks.parse_container(bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        quirks.decode_pixels(&container, raw, &|| false)
    }

    // ------------------------------------------------------------------
    // Compression 34713 round-trip tests.
    // ------------------------------------------------------------------

    #[test]
    fn decodes_12bit_lossless_makernote_format3_mixed_byte_order() {
        let container_le = true;
        let makernote_le = false; // embedded TIFF in the opposite byte order
        let (width, height) = (6_usize, 4_usize);
        let vpred = [1_000_u16, 1_001, 1_002, 1_003];
        let samples = predicted_samples(width, height, vpred, &[-16, -9, -3, 0, 1, 4, 8, 15, -13, 7]);
        let stream = nikon_encode(&samples, width, height, vpred, 2, 0, &emit_plain);
        let blob = curve_blob_lossless(makernote_le, vpred);
        let makernote = build_makernote(
            container_le,
            MakernoteVariant::SignatureV2Tiff(makernote_le),
            &[(0x8c, blob)],
        );
        let bytes = assemble_compressed_nef(container_le, 6, 4, 12, Some(&makernote), &[&stream]);
        let quirks = quirks();
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let metadata = quirks.read_metadata(&container, raw).unwrap();
        assert_eq!(metadata.make, "NIKON");
        assert_eq!(metadata.bits_per_sample, 12);
        assert_eq!(metadata.white_level.0, [4_095.0]);
        let decoded = quirks.decode_pixels(&container, raw, &|| false).unwrap();
        // The lossless curve is the identity, so the decoded pixels equal
        // the predictor samples exactly. Distinct vpred seeds catch any
        // seed-order mix-up in the vertical accumulators.
        assert_eq!(decoded, samples);
    }

    #[test]
    fn decodes_14bit_lossless_makernote_format2_last_curve_tag_wins() {
        let container_le = false; // MM container; format 2 inherits its order
        let (width, height) = (6_usize, 4_usize);
        let vpred = [2_000_u16, 2_001, 2_002, 2_003];
        let samples = predicted_samples(width, height, vpred, &[-25, -7, 0, 3, 11, 26, -31, 5]);
        // huff_select = 2 (lossless) + 3 (14-bit) = 5.
        let stream = nikon_encode(&samples, width, height, vpred, 5, 0, &emit_plain);
        let decoy = curve_blob_lossless(container_le, [500, 501, 502, 503]);
        let blob = curve_blob_lossless(container_le, vpred);
        let makernote = build_makernote(
            container_le,
            MakernoteVariant::SignatureV1,
            &[(0x8c, decoy), (0x96, blob)],
        );
        let bytes = assemble_compressed_nef(container_le, 6, 4, 14, Some(&makernote), &[&stream]);
        let decoded = decode_compressed(&bytes).unwrap();
        // Only passes when the later 0x96 entry overrides the 0x8C decoy
        // seeds, matching dcraw/rawspeed.
        assert_eq!(decoded, samples);
    }

    #[test]
    fn decodes_12bit_lossless_makernote_format1_plain() {
        let container_le = false;
        let (width, height) = (4_usize, 6_usize);
        let vpred = [100_u16, 200, 300, 400];
        let samples = predicted_samples(width, height, vpred, &[-8, -2, 1, 6, -5, 3]);
        let stream = nikon_encode(&samples, width, height, vpred, 2, 0, &emit_plain);
        let blob = curve_blob_lossless(container_le, vpred);
        let makernote = build_makernote(container_le, MakernoteVariant::Plain, &[(0x96, blob)]);
        let bytes = assemble_compressed_nef(container_le, 4, 6, 12, Some(&makernote), &[&stream]);
        let decoded = decode_compressed(&bytes).unwrap();
        assert_eq!(decoded, samples);
    }

    #[test]
    fn decodes_12bit_lossy_interpolated_curve_with_split() {
        let container_le = true;
        let (width, height) = (8_usize, 6_usize);
        let vpred = [2_000_u16, 2_001, 2_002, 2_003];
        let anchors: Vec<u16> = (0..9).map(|k| k * 1_000).collect();
        let split = 3_usize;
        // Rows 0..2 (main table 0): deltas with magnitude < 33. Rows 3..5
        // (after-split table 1): small deltas plus 0x16 shift-symbol deltas
        // (odd, magnitude 33..=63).
        let deltas = [
            -2, 1, 0, 3, -1, 2, 1, -3, // row 0
            0, -1, 2, -2, 3, 1, -3, 0, // row 1
            1, 2, -1, 0, -3, 2, 1, -2, // row 2
            33, -45, 4, 1, -2, 0, 2, -1, // row 3 (split engaged)
            -63, 1, 35, -1, 2, 0, -3, 1, // row 4
            0, 63, -35, 2, -1, 1, -2, 0, // row 5
        ];
        let samples = predicted_samples(width, height, vpred, &deltas);
        let stream = nikon_encode(&samples, width, height, vpred, 0, split, &emit_lossy);
        let blob = curve_blob_interpolated(container_le, vpred, &anchors, u16::try_from(split).unwrap());
        let makernote = build_makernote(container_le, MakernoteVariant::SignatureV1, &[(0x8c, blob)]);
        let bytes = assemble_compressed_nef(container_le, 8, 6, 12, Some(&makernote), &[&stream]);

        // Expected: the piecewise-linear curve applied to the predictors.
        let step = 4_096_usize / (anchors.len() - 1);
        let expected: Vec<u16> = samples
            .iter()
            .map(|&predictor| {
                let index = usize::from(predictor);
                let b = index % step;
                let a = index - b;
                u16::try_from(
                    (usize::from(anchors[a / step]) * (step - b) + usize::from(anchors[a / step + 1]) * b)
                        / step,
                )
                .unwrap()
            })
            .collect();
        let decoded = decode_compressed(&bytes).unwrap();
        assert_eq!(decoded, expected);
    }

    // ------------------------------------------------------------------
    // Compression 34713 typed rejections.
    // ------------------------------------------------------------------

    #[test]
    fn rejects_nikon_lossless_multi_strip() {
        let (stream, makernote, _) = lossless_fixture(true);
        let bytes = assemble_compressed_nef(true, 6, 4, 12, Some(&makernote), &[&stream, &stream]);
        let error = decode_compressed(&bytes).unwrap_err();
        assert!(
            matches!(&error, DecodeError::NativeCamera { message, .. } if message.contains("strips")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn rejects_nikon_lossless_16bit() {
        let (stream, makernote, _) = lossless_fixture(true);
        let bytes = assemble_compressed_nef(true, 6, 4, 16, Some(&makernote), &[&stream]);
        let error = decode_compressed(&bytes).unwrap_err();
        assert!(
            matches!(&error, DecodeError::NativeCamera { message, .. } if message.contains("BitsPerSample 16")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn rejects_broken_makernote_tiff_header() {
        let (stream, _, _) = lossless_fixture(true);
        // Format-3 signature with a wild embedded IFD offset.
        let mut makernote = b"Nikon\0\x02\x10\x00\x00".to_vec();
        makernote.extend_from_slice(b"II");
        makernote.extend_from_slice(&w16(true, 42));
        makernote.extend_from_slice(&w32(true, 0xffff_fff0));
        let bytes = assemble_compressed_nef(true, 6, 4, 12, Some(&makernote), &[&stream]);
        let error = decode_compressed(&bytes).unwrap_err();
        assert!(
            matches!(&error, DecodeError::NativeCamera { message, .. } if message.contains("makernote")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn rejects_makernote_without_curve_tag() {
        let (stream, _, _) = lossless_fixture(true);
        let makernote = build_makernote(
            true,
            MakernoteVariant::SignatureV1,
            &[(0x0001, vec![1, 2, 3, 4, 5])],
        );
        let bytes = assemble_compressed_nef(true, 6, 4, 12, Some(&makernote), &[&stream]);
        let error = decode_compressed(&bytes).unwrap_err();
        assert!(
            matches!(&error, DecodeError::NativeCamera { message, .. } if message.contains("NikonCurve")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn rejects_truncated_curve_blob() {
        let (stream, _, _) = lossless_fixture(true);
        // A 4-byte inline value: too short for the four predictor seeds.
        let makernote = build_makernote(
            true,
            MakernoteVariant::SignatureV1,
            &[(0x8c, vec![0x46, 0x00, 0x10, 0x00])],
        );
        let bytes = assemble_compressed_nef(true, 6, 4, 12, Some(&makernote), &[&stream]);
        let error = decode_compressed(&bytes).unwrap_err();
        assert!(
            matches!(&error, DecodeError::NativeCamera { message, .. } if message.contains("truncated")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn rejects_curve_out_of_range_samples() {
        let container_le = true;
        let vpred = [1_000_u16, 1_001, 1_002, 1_003];
        let samples = predicted_samples(6, 4, vpred, &[-4, -1, 0, 2, 5, -3]);
        // Explicit 16-entry curve; predictors near 1000 are far outside it
        // and must be a hard error, not dcraw's silent clamp.
        let table: Vec<u16> = (0..16).collect();
        let blob = curve_blob_explicit(container_le, 0x10, vpred, &table);
        let makernote = build_makernote(container_le, MakernoteVariant::SignatureV1, &[(0x8c, blob)]);
        let stream = nikon_encode(&samples, 6, 4, vpred, 0, 0, &emit_plain);
        let bytes = assemble_compressed_nef(container_le, 6, 4, 12, Some(&makernote), &[&stream]);
        let error = decode_compressed(&bytes).unwrap_err();
        assert!(
            matches!(&error, DecodeError::NativeCamera { message, .. } if message.contains("linearization curve")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn rejects_truncated_entropy_stream() {
        let (stream, makernote, _) = lossless_fixture(true);
        let short = &stream[..stream.len() / 2];
        let bytes = assemble_compressed_nef(true, 6, 4, 12, Some(&makernote), &[short]);
        let error = decode_compressed(&bytes).unwrap_err();
        assert!(
            matches!(&error, DecodeError::NativeCamera { message, .. } if message.contains("truncated")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn honors_cancelled_callback_compressed() {
        let (stream, makernote, _) = lossless_fixture(true);
        let bytes = assemble_compressed_nef(true, 6, 4, 12, Some(&makernote), &[&stream]);
        let quirks = quirks();
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.decode_pixels(&container, raw, &|| true).unwrap_err();
        assert!(matches!(error, DecodeError::Cancelled));
    }
}
