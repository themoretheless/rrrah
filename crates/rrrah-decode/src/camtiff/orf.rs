//! Olympus / OM System ORF decoder.
//!
//! # Container
//!
//! ORF is a TIFF-family format with a non-standard magic: bytes 0..4 are
//! `IIRO` (most models), `IIRS` (some E-system models), or the big-endian
//! `MMOR`/`MMSR` variants. Everything after the magic word is classic
//! TIFF: a 32-bit first-IFD offset at bytes 4..8 and standard IFD tables.
//! The raw mosaic lives in a `SubIFDs` (330) directory of IFD0.
//!
//! ## Shared-file dependency (landed)
//!
//! [`crate::dng::tiff::Tiff::parse`] accepts the ORF magic values `0x4F52`
//! and `0x5352` as `Variant::Classic` alongside the classic magic 42 and the
//! `BigTIFF` magic 43 (both ORF values are byte-order symmetric: `RO`/`OR`
//! and `RS`/`SR`). `OrfQuirks::parse_container` below validates the ORF
//! signature and then delegates to [`CameraFile::parse_tiff`] directly.
//!
//! # Pixel storage
//!
//! Supported, in recognition order:
//!
//! - `Compression = 1`, `BitsPerSample = 12`: Olympus' proprietary 12-bit
//!   packing. This is NOT the TIFF/EP MSB-first packing: dcraw's
//!   `packed_load_raw` with `load_flags = 24` reads the strip as 32-bit
//!   little-endian words consumed MSB-first, which is equivalent to
//!   reversing the bytes inside every 4-byte group and then applying the
//!   standard MSB-first 12-bit unpacking (two pixels per three stream
//!   bytes, middle byte nibble-swapped relative to plain packing).
//!   Verified against dcraw's bit pump semantics. The strip byte count
//!   must equal the stream length rounded up to a 4-byte word; when it
//!   does not, the packing phase is unverifiable and the file is rejected
//!   with a typed error rather than guessed.
//! - `Compression = 1`, `BitsPerSample = 16`: plain uncompressed `u16`
//!   rows in the container byte order (used by higher-end models).
//! - `Compression = 7`: lossless JPEG via the shared
//!   [`crate::dng::lossless_jpeg`] decoder (single strip only).
//!
//! Explicitly rejected with typed errors (no silent degradation):
//!
//! - any other `Compression` value (Olympus uses e.g. 6 for previews and
//!   proprietary codes on some models);
//! - `BitsPerSample` other than 12/16;
//! - the C-series Huffman variant (`olympus_load_raw` in dcraw) and the
//!   proprietary OM System (OM-1 family) bitstream: both masquerade as
//!   `Compression = 1`, so they are caught by the strict strip byte-count
//!   validation of the 12-bit/16-bit paths and rejected as unverifiable;
//! - tiled storage (never observed in ORF);
//! - missing/unsupported CFA tags or a non-2x2 RGB Bayer pattern.
//!
//! # Metadata
//!
//! Make/Model/Orientation come from IFD0; CFA from the raw IFD's TIFF/EP
//! `CFARepeatPatternDim` (33421) / `CFAPattern` (33422) tags; black level
//! from `BlackLevel` (50714, default 0), white level from `WhiteLevel`
//! (50717, default `2^bps - 1`, documented approximation — Olympus' exact
//! per-model saturation lives in the makernote, which is not parsed);
//! white balance from `AsShotNeutral` (50728) when present; the
//! `ColorMatrix1/2` + `CalibrationIlluminant1/2` pair goes through the
//! shared D65 selection in `rrrah_core`; `ActiveArea` (50829) and
//! `DefaultCropOrigin`/`DefaultCropSize` (50719/50720) are honored when
//! present and integral.
//!
//! # Tests
//!
//! Unit tests build synthetic minimal TIFF headers in memory (standard
//! magic 42; ORF IFD bodies are classic TIFF and the raw-IFD parsing is
//! identical once the container magic is accepted). They validate
//! the parser, the Olympus 12-bit unpacking against an independent
//! dcraw-style bit-pump reference, and the typed rejections. They are NOT
//! evidence of camera compatibility: no licensed camera files are used.

use rrrah_core::{
    CfaColor, CfaPattern, DngColorMatrix, LevelGrid, Orientation, Rect, WhiteLevel, select_dng_xyz_to_camera,
};

use super::{
    CameraDirectory, CameraFile, CameraMetadata, CameraQuirks, camera_error, optional_ascii, optional_scalar,
    orientation_from_tag, required_scalar, tags,
};
use crate::{
    DecodeError,
    dng::lossless_jpeg::{self, LosslessJpegError},
    dng::tiff::ByteOrder,
};

const FORMAT: &str = "ORF";

// DNG/TIFF-EP tags read from the raw IFD beyond the shared `tags` table.
const BLACK_LEVEL: u16 = 50_714;
const WHITE_LEVEL: u16 = 50_717;
const DEFAULT_CROP_ORIGIN: u16 = 50_719;
const DEFAULT_CROP_SIZE: u16 = 50_720;
const COLOR_MATRIX_1: u16 = 50_721;
const COLOR_MATRIX_2: u16 = 50_722;
const AS_SHOT_NEUTRAL: u16 = 50_728;
const CALIBRATION_ILLUMINANT_1: u16 = 50_778;
const CALIBRATION_ILLUMINANT_2: u16 = 50_779;
const ACTIVE_AREA: u16 = 50_829;

/// Olympus/OM System ORF quirks.
#[derive(Debug)]
pub(crate) struct OrfQuirks;

/// Recognizes the ORF magic words at bytes 0..4.
fn orf_magic(data: &[u8]) -> bool {
    matches!(data.get(0..4), Some(b"IIRO" | b"IIRS" | b"MMOR" | b"MMSR"))
}

fn orf_error(message: impl Into<String>) -> DecodeError {
    camera_error(FORMAT, message)
}

impl CameraQuirks for OrfQuirks {
    fn format_name(&self) -> &'static str {
        FORMAT
    }

    fn parse_container<'a>(&self, data: &'a [u8]) -> Result<CameraFile<'a>, DecodeError> {
        if !orf_magic(data) {
            return Err(orf_error(
                "missing ORF magic at bytes 0..4 (expected IIRO, IIRS, MMOR, or MMSR)",
            ));
        }
        // The shared `dng::tiff::Tiff::parse` accepts the ORF magic values
        // 0x4F52 and 0x5352 as classic TIFF (see the module docs), so this
        // delegation parses real ORF containers directly.
        CameraFile::parse_tiff(FORMAT, data)
    }

    fn read_metadata(
        &self,
        container: &CameraFile<'_>,
        raw: &CameraDirectory<'_>,
    ) -> Result<CameraMetadata, DecodeError> {
        let width = required_u32(raw, tags::IMAGE_WIDTH)?;
        let height = required_u32(raw, tags::IMAGE_LENGTH)?;
        let bits_per_sample = read_bits_per_sample(raw)?;
        if optional_scalar(FORMAT, raw, tags::PHOTOMETRIC_INTERPRETATION)? != Some(tags::PHOTOMETRIC_CFA) {
            return Err(orf_error(
                "raw IFD photometric is not CFA (32803); RGB/linear ORF variants are unsupported",
            ));
        }
        let samples_per_pixel = optional_scalar(FORMAT, raw, tags::SAMPLES_PER_PIXEL)?.unwrap_or(1);
        if samples_per_pixel != 1 {
            return Err(orf_error(format!(
                "raw IFD has {samples_per_pixel} samples per pixel; only single-plane CFA is supported"
            )));
        }

        let ifd0 = container
            .directories()
            .iter()
            .find(|directory| directory.is_top_level())
            .ok_or_else(|| orf_error("no top-level IFD for camera identification"))?;
        let make = optional_ascii(FORMAT, ifd0, tags::MAKE)?.unwrap_or_else(|| "Olympus".to_owned());
        let model = optional_ascii(FORMAT, ifd0, tags::MODEL)?.unwrap_or_default();
        let orientation = match optional_scalar(FORMAT, ifd0, tags::ORIENTATION)? {
            Some(value) => orientation_from_tag(FORMAT, value)?,
            None => Orientation::Normal,
        };

        let cfa = read_cfa(raw)?;
        let black_level = read_black_level(raw)?;
        let white_level = read_white_level(raw, bits_per_sample)?;
        let white_balance = read_white_balance(raw)?;
        let xyz_to_camera = read_xyz_to_camera(raw)?;
        let active_area = read_active_area(raw, width, height)?;
        let crop_area = read_crop_area(raw, width, height)?;

        Ok(CameraMetadata {
            make,
            model,
            width,
            height,
            bits_per_sample,
            cfa,
            black_level,
            white_level,
            white_balance,
            xyz_to_camera,
            active_area,
            crop_area,
            orientation,
        })
    }

    fn decode_pixels(
        &self,
        container: &CameraFile<'_>,
        raw: &CameraDirectory<'_>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<u16>, DecodeError> {
        let width = required_u32(raw, tags::IMAGE_WIDTH)?;
        let height = required_u32(raw, tags::IMAGE_LENGTH)?;
        let bits_per_sample = read_bits_per_sample(raw)?;
        let compression =
            optional_scalar(FORMAT, raw, tags::COMPRESSION)?.unwrap_or(tags::COMPRESSION_UNCOMPRESSED);

        if raw.entry(FORMAT, tags::TILE_OFFSETS)?.is_some() {
            return Err(orf_error(
                "tiled ORF storage is unsupported (no tiled ORF is known to exist)",
            ));
        }
        let offsets = raw
            .entry(FORMAT, tags::STRIP_OFFSETS)?
            .ok_or_else(|| orf_error("raw IFD has neither strip nor tile storage"))?
            .unsigned_values()
            .map_err(|error| orf_error(format!("StripOffsets: {error}")))?;
        let byte_counts = raw
            .entry(FORMAT, tags::STRIP_BYTE_COUNTS)?
            .ok_or_else(|| orf_error("raw IFD lacks StripByteCounts"))?
            .unsigned_values()
            .map_err(|error| orf_error(format!("StripByteCounts: {error}")))?;
        if offsets.len() != byte_counts.len() {
            return Err(orf_error(format!(
                "StripOffsets has {} entries but StripByteCounts has {}",
                offsets.len(),
                byte_counts.len()
            )));
        }
        let rows_per_strip = optional_scalar(FORMAT, raw, tags::ROWS_PER_STRIP)?.unwrap_or(u64::from(height));
        if rows_per_strip == 0 {
            return Err(orf_error("RowsPerStrip is zero"));
        }

        let width_usize = usize::try_from(width).map_err(|_| DecodeError::DimensionOverflow)?;
        let height_usize = usize::try_from(height).map_err(|_| DecodeError::DimensionOverflow)?;
        let total = width_usize
            .checked_mul(height_usize)
            .ok_or(DecodeError::DimensionOverflow)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(total)
            .map_err(|_| orf_error(format!("could not allocate {total} samples")))?;
        output.resize(total, 0);

        match compression {
            tags::COMPRESSION_UNCOMPRESSED => decode_uncompressed(
                container,
                &mut output,
                width,
                height,
                bits_per_sample,
                rows_per_strip,
                &offsets,
                &byte_counts,
                cancelled,
            ),
            tags::COMPRESSION_LOSSLESS_JPEG => {
                decode_lossless_jpeg(container, &mut output, &offsets, &byte_counts, cancelled)
            }
            actual => Err(orf_error(format!(
                "unsupported ORF compression {actual}; only uncompressed (1) and lossless JPEG (7) are supported"
            ))),
        }?;
        Ok(output)
    }
}

fn required_u32(raw: &CameraDirectory<'_>, tag: u16) -> Result<u32, DecodeError> {
    let value = required_scalar(FORMAT, raw, tag)?;
    u32::try_from(value).map_err(|_| orf_error(format!("tag {tag} value {value} exceeds u32")))
}

/// Reads and validates `BitsPerSample`: only 12 (Olympus packed) and 16
/// (uncompressed) are decodable; anything else is a typed rejection.
fn read_bits_per_sample(raw: &CameraDirectory<'_>) -> Result<u8, DecodeError> {
    let value = optional_scalar(FORMAT, raw, tags::BITS_PER_SAMPLE)?.unwrap_or(12);
    match value {
        12 | 16 => Ok(u8::try_from(value).expect("12 and 16 fit u8")),
        actual => Err(orf_error(format!(
            "unsupported ORF bit depth {actual}; only 12-bit packed and 16-bit uncompressed are supported"
        ))),
    }
}

/// Reads the CFA from the TIFF/EP tags in the raw IFD. The pattern must be a
/// 2x2 RGB Bayer quad; anything else is rejected instead of guessed.
fn read_cfa(raw: &CameraDirectory<'_>) -> Result<CfaPattern, DecodeError> {
    let dims = raw
        .entry(FORMAT, tags::CFA_REPEAT_PATTERN_DIM)?
        .ok_or_else(|| orf_error("raw IFD lacks CFARepeatPatternDim (33421)"))?
        .unsigned_values()
        .map_err(|error| orf_error(format!("CFARepeatPatternDim: {error}")))?;
    if dims.len() != 2 || dims[0] != 2 || dims[1] != 2 {
        return Err(orf_error(format!(
            "unsupported CFA repeat dimensions {dims:?}; only 2x2 Bayer is supported"
        )));
    }
    let pattern = raw
        .entry(FORMAT, tags::CFA_PATTERN)?
        .ok_or_else(|| orf_error("raw IFD lacks CFAPattern (33422)"))?
        .unsigned_values()
        .map_err(|error| orf_error(format!("CFAPattern: {error}")))?;
    if pattern.len() != 4 {
        return Err(orf_error(format!(
            "CFAPattern has {} cells, expected 4 for a 2x2 CFA",
            pattern.len()
        )));
    }
    let mut cells = Vec::with_capacity(4);
    for value in pattern {
        cells.push(match value {
            0 => CfaColor::Red,
            1 => CfaColor::Green,
            2 => CfaColor::Blue,
            actual => {
                return Err(orf_error(format!(
                    "unsupported CFA color code {actual}; only RGB Bayer is supported"
                )));
            }
        });
    }
    let cfa = CfaPattern {
        width: 2,
        height: 2,
        cells,
    };
    cfa.bayer_quad()
        .map_err(|_| orf_error("ORF CFA pattern is not a 1R/2G/1B Bayer quad"))?;
    Ok(cfa)
}

// Level and gain values arrive as f64 rationals and are stored as f32 in the
// domain metadata; conversions are checked for finiteness at each site.
#[allow(clippy::cast_possible_truncation)]
fn read_black_level(raw: &CameraDirectory<'_>) -> Result<LevelGrid, DecodeError> {
    let value = match raw.entry(FORMAT, BLACK_LEVEL)? {
        Some(entry) => entry
            .numeric_values()
            .map_err(|error| orf_error(format!("BlackLevel: {error}")))?
            .first()
            .copied()
            .map(|value| value as f32)
            .filter(|value| value.is_finite())
            .ok_or_else(|| orf_error("BlackLevel is empty or non-finite"))?,
        // Documented default: Olympus black level is 0 for the supported
        // 12-bit packed and 16-bit uncompressed storage.
        None => 0.0,
    };
    Ok(LevelGrid {
        width: 1,
        height: 1,
        components: 1,
        values: vec![value],
    })
}

#[allow(clippy::cast_possible_truncation)]
fn read_white_level(raw: &CameraDirectory<'_>, bits_per_sample: u8) -> Result<WhiteLevel, DecodeError> {
    let value = match raw.entry(FORMAT, WHITE_LEVEL)? {
        Some(entry) => entry
            .numeric_values()
            .map_err(|error| orf_error(format!("WhiteLevel: {error}")))?
            .first()
            .copied()
            .map(|value| value as f32)
            .filter(|value| value.is_finite())
            .ok_or_else(|| orf_error("WhiteLevel is empty or non-finite"))?,
        // Documented default: full-scale for the bit depth. Olympus' exact
        // per-model saturation point is makernote-only and not parsed.
        None => ((1_u32 << bits_per_sample) - 1) as f32,
    };
    Ok(WhiteLevel(vec![value]))
}

/// White balance from `AsShotNeutral` (R, G, B), normalized so green is 1,
/// expanded to the four CFA planes as [R, G, B, G]. Defaults to unity.
#[allow(clippy::cast_possible_truncation)]
fn read_white_balance(raw: &CameraDirectory<'_>) -> Result<[f32; 4], DecodeError> {
    let Some(entry) = raw.entry(FORMAT, AS_SHOT_NEUTRAL)? else {
        return Ok([1.0; 4]);
    };
    let neutral = entry
        .numeric_values()
        .map_err(|error| orf_error(format!("AsShotNeutral: {error}")))?;
    if neutral.len() != 3 {
        return Err(orf_error(format!(
            "AsShotNeutral has {} values, expected 3",
            neutral.len()
        )));
    }
    let mut gains = [0.0_f32; 3];
    for (destination, source) in gains.iter_mut().zip(&neutral) {
        if *source <= 0.0 {
            return Err(orf_error("AsShotNeutral contains a non-positive component"));
        }
        *destination = (1.0 / source) as f32;
        if !destination.is_finite() {
            return Err(orf_error("AsShotNeutral produced a non-finite gain"));
        }
    }
    let green = gains[1];
    Ok([gains[0] / green, 1.0, gains[2] / green, 1.0])
}

fn read_color_matrix(
    raw: &CameraDirectory<'_>,
    matrix_tag: u16,
    illuminant_tag: u16,
) -> Result<Option<DngColorMatrix>, DecodeError> {
    let Some(entry) = raw.entry(FORMAT, matrix_tag)? else {
        return Ok(None);
    };
    let values = entry
        .numeric_values()
        .map_err(|error| orf_error(format!("tag {matrix_tag}: {error}")))?;
    if values.len() != 9 {
        return Err(orf_error(format!(
            "color matrix tag {matrix_tag} has {} values, expected 9",
            values.len()
        )));
    }
    let illuminant = optional_scalar(FORMAT, raw, illuminant_tag)?
        .map(|value| u16::try_from(value).map_err(|_| orf_error(format!("tag {illuminant_tag} exceeds u16"))))
        .transpose()?;
    Ok(Some(DngColorMatrix {
        xyz_to_camera: [
            [values[0], values[1], values[2]],
            [values[3], values[4], values[5]],
            [values[6], values[7], values[8]],
        ],
        illuminant,
    }))
}

/// Builds the XYZ -> camera matrix, mirroring the DNG backend: the D65
/// calibration pair is selected in f64; the fourth (second green) plane row
/// stays zero, and the default is identity with a zero fourth row.
#[allow(clippy::cast_possible_truncation)]
fn read_xyz_to_camera(raw: &CameraDirectory<'_>) -> Result<[[f32; 3]; 4], DecodeError> {
    let first = read_color_matrix(raw, COLOR_MATRIX_1, CALIBRATION_ILLUMINANT_1)?;
    let second = read_color_matrix(raw, COLOR_MATRIX_2, CALIBRATION_ILLUMINANT_2)?;
    let Some(matrix) = select_dng_xyz_to_camera(first, second) else {
        return Ok([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]]);
    };
    let mut result = [[0.0_f32; 3]; 4];
    for (destination, source) in result.iter_mut().zip(matrix.iter()) {
        for (dst, src) in destination.iter_mut().zip(source.iter()) {
            *dst = *src as f32;
            if !dst.is_finite() {
                return Err(orf_error("color matrix produced a non-finite value"));
            }
        }
    }
    Ok(result)
}

fn read_active_area(raw: &CameraDirectory<'_>, width: u32, height: u32) -> Result<Option<Rect>, DecodeError> {
    let Some(entry) = raw.entry(FORMAT, ACTIVE_AREA)? else {
        return Ok(None);
    };
    let values = entry
        .unsigned_values()
        .map_err(|error| orf_error(format!("ActiveArea: {error}")))?;
    if values.len() != 4 {
        return Err(orf_error(format!(
            "ActiveArea has {} values, expected 4 (top, left, bottom, right)",
            values.len()
        )));
    }
    let to_u32 = |value: u64| {
        u32::try_from(value).map_err(|_| orf_error(format!("ActiveArea value {value} exceeds u32")))
    };
    let (top, left, bottom, right) = (
        to_u32(values[0])?,
        to_u32(values[1])?,
        to_u32(values[2])?,
        to_u32(values[3])?,
    );
    if bottom < top || right < left {
        return Err(orf_error("ActiveArea has inverted bounds"));
    }
    let rect = Rect::new(left, top, right - left, bottom - top);
    if !rect.fits_within(width, height) {
        return Err(orf_error("ActiveArea extends past the stored mosaic"));
    }
    Ok(Some(rect))
}

fn read_crop_area(raw: &CameraDirectory<'_>, width: u32, height: u32) -> Result<Option<Rect>, DecodeError> {
    let (Some(origin), Some(size)) = (
        raw.entry(FORMAT, DEFAULT_CROP_ORIGIN)?,
        raw.entry(FORMAT, DEFAULT_CROP_SIZE)?,
    ) else {
        return Ok(None);
    };
    let exact_pair = |entry: &crate::dng::tiff::Entry<'_>, tag: u16| -> Result<(u32, u32), DecodeError> {
        let values = entry
            .numeric_values()
            .map_err(|error| orf_error(format!("tag {tag}: {error}")))?;
        if values.len() != 2 {
            return Err(orf_error(format!(
                "tag {tag} has {} values, expected 2",
                values.len()
            )));
        }
        let mut pair = (0_u32, 0_u32);
        for (destination, value) in [&mut pair.0, &mut pair.1].into_iter().zip(values) {
            if value < 0.0 || value.fract() != 0.0 || value > f64::from(u32::MAX) {
                return Err(orf_error(format!("tag {tag} value {value} is not a u32 integer")));
            }
            // Range- and integrality-checked above.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                *destination = value as u32;
            }
        }
        Ok(pair)
    };
    let (x, y) = exact_pair(origin, DEFAULT_CROP_ORIGIN)?;
    let (crop_width, crop_height) = exact_pair(size, DEFAULT_CROP_SIZE)?;
    let rect = Rect::new(x, y, crop_width, crop_height);
    if !rect.fits_within(width, height) {
        return Err(orf_error("DefaultCrop rectangle extends past the stored mosaic"));
    }
    Ok(Some(rect))
}

/// Bounds-checked view of one strip's encoded bytes.
fn strip_bytes<'a>(
    container: &CameraFile<'a>,
    offset: u64,
    byte_count: u64,
) -> Result<&'a [u8], DecodeError> {
    let start = usize::try_from(offset).map_err(|_| orf_error("strip offset overflows usize"))?;
    let length = usize::try_from(byte_count).map_err(|_| orf_error("strip byte count overflows usize"))?;
    let end = start
        .checked_add(length)
        .ok_or_else(|| orf_error("strip extent overflows usize"))?;
    container.data().get(start..end).ok_or_else(|| {
        orf_error(format!(
            "strip at offset {offset} with {byte_count} bytes is out of bounds"
        ))
    })
}

/// Per-strip row span; the last strip takes the remaining rows.
fn strip_rows(strip_index: usize, rows_per_strip: u64, height: u32) -> Result<(u32, u32), DecodeError> {
    let index = u32::try_from(strip_index).map_err(|_| orf_error("strip index exceeds u32"))?;
    let first_row = u64::from(index)
        .checked_mul(rows_per_strip)
        .ok_or_else(|| orf_error("strip row position overflows u64"))?;
    if first_row >= u64::from(height) {
        return Err(orf_error(format!(
            "strip {strip_index} starts at row {first_row}, past the image height {height}"
        )));
    }
    let first_row = u32::try_from(first_row).expect("bounded by height");
    let remaining = height - first_row;
    let rows = u32::try_from(rows_per_strip.min(u64::from(remaining)))
        .map_err(|_| orf_error("rows per strip exceeds u32"))?;
    Ok((first_row, rows))
}

#[allow(clippy::too_many_arguments)]
fn decode_uncompressed(
    container: &CameraFile<'_>,
    output: &mut [u16],
    width: u32,
    height: u32,
    bits_per_sample: u8,
    rows_per_strip: u64,
    offsets: &[u64],
    byte_counts: &[u64],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), DecodeError> {
    let width_usize = usize::try_from(width).map_err(|_| DecodeError::DimensionOverflow)?;
    let byte_order = container.byte_order();
    for (index, (&offset, &byte_count)) in offsets.iter().zip(byte_counts).enumerate() {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let (first_row, rows) = strip_rows(index, rows_per_strip, height)?;
        let rows_usize = usize::try_from(rows).map_err(|_| DecodeError::DimensionOverflow)?;
        let strip = strip_bytes(container, offset, byte_count)?;
        match bits_per_sample {
            12 => decode_olympus_packed_strip(
                strip,
                byte_count,
                width_usize,
                rows_usize,
                first_row,
                output,
                cancelled,
            )?,
            16 => decode_u16_strip(
                strip,
                byte_count,
                width_usize,
                rows_usize,
                first_row,
                byte_order,
                output,
                cancelled,
            )?,
            actual => {
                return Err(orf_error(format!(
                    "unsupported ORF bit depth {actual}; only 12-bit packed and 16-bit uncompressed are supported"
                )));
            }
        }
    }
    Ok(())
}

/// Olympus' proprietary 12-bit packing (dcraw `packed_load_raw` with
/// `load_flags = 24`): the byte stream is read as 32-bit little-endian words
/// consumed MSB-first, equivalent to reversing each 4-byte group and then
/// applying standard MSB-first 12-bit unpacking. The stream is continuous
/// across the strip, so the declared byte count must cover whole 32-bit
/// words: `round_up_4(width * rows * 3 / 2)`. Anything else makes the
/// packing phase unverifiable and is rejected.
#[allow(clippy::too_many_arguments)]
fn decode_olympus_packed_strip(
    strip: &[u8],
    declared_count: u64,
    width: usize,
    rows: usize,
    first_row: u32,
    output: &mut [u16],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), DecodeError> {
    if !width.is_multiple_of(2) {
        return Err(orf_error(
            "odd raw width is unsupported for Olympus 12-bit packed storage",
        ));
    }
    // Even width: every row is exactly width * 3 / 2 stream bytes.
    let row_stream_bytes = width
        .checked_mul(3)
        .and_then(|value| value.checked_div(2))
        .ok_or_else(|| orf_error("Olympus 12-bit row size overflows usize"))?;
    let stream_bytes = row_stream_bytes
        .checked_mul(rows)
        .ok_or_else(|| orf_error("Olympus 12-bit strip size overflows usize"))?;
    let word_padded = stream_bytes
        .checked_add(3)
        .map(|value| value & !3)
        .ok_or_else(|| orf_error("Olympus 12-bit strip size overflows usize"))?;
    let declared =
        usize::try_from(declared_count).map_err(|_| orf_error("strip byte count overflows usize"))?;
    if declared != word_padded || strip.len() != declared {
        return Err(orf_error(format!(
            "Olympus 12-bit packed strip declares {declared_count} bytes, expected {word_padded} \
             ({stream_bytes} stream bytes rounded to 32-bit words); cannot verify the packing phase, \
             this may be a proprietary Olympus Huffman/bitstream variant"
        )));
    }
    for row in 0..rows {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let output_row = usize::try_from(first_row)
            .ok()
            .and_then(|first| first.checked_add(row))
            .and_then(|absolute| absolute.checked_mul(width))
            .ok_or(DecodeError::DimensionOverflow)?;
        let base = row
            .checked_mul(row_stream_bytes)
            .ok_or_else(|| orf_error("Olympus 12-bit stream position overflows usize"))?;
        unpack_olympus12_row(strip, base, &mut output[output_row..output_row + width]);
    }
    Ok(())
}

/// One Olympus packed-12 row. `base` is the absolute stream-byte position of
/// the row start inside `strip`. Stream byte `i` is file byte
/// `(i & !3) + (3 - (i & 3))` — the dcraw 32-bit-word bit pump.
fn unpack_olympus12_row(strip: &[u8], base: usize, out: &mut [u16]) {
    for (pair, pixels) in out.chunks_exact_mut(2).enumerate() {
        let stream = base + pair * 3;
        let byte0 = olympus_stream_byte(strip, stream);
        let byte1 = olympus_stream_byte(strip, stream + 1);
        let byte2 = olympus_stream_byte(strip, stream + 2);
        pixels[0] = (u16::from(byte0) << 4) | (u16::from(byte1) >> 4);
        pixels[1] = ((u16::from(byte1) & 0x0f) << 8) | u16::from(byte2);
    }
}

/// Maps an Olympus 12-bit stream position to a file byte: positions are read
/// MSB-first from 32-bit little-endian words (dcraw `load_flags = 24`).
/// Positions past the declared strip end read as zero; the caller validates
/// the declared length so this only covers the final word's padding.
fn olympus_stream_byte(data: &[u8], stream_index: usize) -> u8 {
    let file_index = (stream_index & !3) + (3 - (stream_index & 3));
    data.get(file_index).copied().unwrap_or(0)
}

/// Uncompressed 16-bit rows in the container byte order.
#[allow(clippy::too_many_arguments)]
fn decode_u16_strip(
    strip: &[u8],
    declared_count: u64,
    width: usize,
    rows: usize,
    first_row: u32,
    byte_order: ByteOrder,
    output: &mut [u16],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), DecodeError> {
    let expected = width
        .checked_mul(rows)
        .and_then(|samples| samples.checked_mul(2))
        .ok_or_else(|| orf_error("16-bit strip size overflows usize"))?;
    let declared =
        usize::try_from(declared_count).map_err(|_| orf_error("strip byte count overflows usize"))?;
    if declared != expected || strip.len() != declared {
        return Err(orf_error(format!(
            "16-bit strip declares {declared_count} bytes, expected {expected} \
             ({width} x {rows} samples); cannot verify uncompressed storage, \
             this may be a proprietary Olympus Huffman/bitstream variant"
        )));
    }
    for row in 0..rows {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let output_row = usize::try_from(first_row)
            .ok()
            .and_then(|first| first.checked_add(row))
            .and_then(|absolute| absolute.checked_mul(width))
            .ok_or(DecodeError::DimensionOverflow)?;
        let row_start = row
            .checked_mul(width)
            .and_then(|samples| samples.checked_mul(2))
            .ok_or_else(|| orf_error("16-bit row offset overflows usize"))?;
        let row_bytes = &strip[row_start..row_start + width * 2];
        for (pixel, bytes) in output[output_row..output_row + width]
            .iter_mut()
            .zip(row_bytes.chunks_exact(2))
        {
            *pixel = byte_order.u16(bytes);
        }
    }
    Ok(())
}

fn decode_lossless_jpeg(
    container: &CameraFile<'_>,
    output: &mut [u16],
    offsets: &[u64],
    byte_counts: &[u64],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), DecodeError> {
    if offsets.len() != 1 {
        return Err(orf_error(format!(
            "lossless-JPEG ORF has {} strips; only single-strip storage is supported",
            offsets.len()
        )));
    }
    if cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let strip = strip_bytes(container, offsets[0], byte_counts[0])?;
    let image = lossless_jpeg::decode(strip, &|| cancelled()).map_err(|error| match error {
        LosslessJpegError::Cancelled { .. } => DecodeError::Cancelled,
        other => orf_error(format!("lossless JPEG: {other}")),
    })?;
    if image.component_ids.len() != 1 {
        return Err(orf_error(format!(
            "lossless-JPEG ORF has {} components; only single-component CFA is supported",
            image.component_ids.len()
        )));
    }
    if image.samples.len() != output.len() {
        return Err(orf_error(format!(
            "lossless-JPEG ORF decoded {} samples, expected {} ({}x{} from the raw IFD)",
            image.samples.len(),
            output.len(),
            image.width,
            image.height
        )));
    }
    output.copy_from_slice(&image.samples);
    Ok(())
}

#[cfg(test)]
mod tests {
    // The TIFF fixture builder packs small, known-range values with `as`
    // casts; strict float equality is intended for exact decoded values.
    #![allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::float_cmp,
        clippy::too_many_arguments
    )]

    use super::*;

    const BYTE_T: u16 = 1;
    const ASCII_T: u16 = 2;
    const SHORT_T: u16 = 3;
    const LONG_T: u16 = 4;

    type TestEntry = (u16, u16, u32, Vec<u8>);

    fn short(tag: u16, values: &[u16]) -> TestEntry {
        let data = values.iter().flat_map(|value| value.to_le_bytes()).collect();
        (tag, SHORT_T, values.len() as u32, data)
    }

    fn long(tag: u16, values: &[u32]) -> TestEntry {
        let data = values.iter().flat_map(|value| value.to_le_bytes()).collect();
        (tag, LONG_T, values.len() as u32, data)
    }

    fn ascii(tag: u16, text: &str) -> TestEntry {
        let mut data = text.as_bytes().to_vec();
        data.push(0);
        (tag, ASCII_T, data.len() as u32, data)
    }

    /// Builds an ORF-like TIFF with the standard magic 42: IFD0 with
    /// make/model/orientation and a `SubIFDs` pointer to the raw IFD. Real
    /// ORF headers carry the `0x4F52`/`0x5352` magic, which the shared
    /// reader now accepts; the IFD bodies exercised here are classic TIFF in
    /// both cases.
    fn build_fixture(
        width: u32,
        height: u32,
        bits: u16,
        compression: u16,
        strip_data: &[u8],
        declared_count: u32,
        cfa: Option<[u8; 4]>,
        extra_raw: Vec<TestEntry>,
    ) -> Vec<u8> {
        let mut ifd0: Vec<TestEntry> = vec![
            ascii(tags::MAKE, "OLYMPUS"),
            ascii(tags::MODEL, "E-TEST"),
            short(tags::ORIENTATION, &[1]),
            long(tags::SUB_IFDS, &[0]), // patched below
        ];
        let mut raw: Vec<TestEntry> = vec![
            long(tags::IMAGE_WIDTH, &[width]),
            long(tags::IMAGE_LENGTH, &[height]),
            short(tags::BITS_PER_SAMPLE, &[bits]),
            short(tags::COMPRESSION, &[compression]),
            short(tags::PHOTOMETRIC_INTERPRETATION, &[32_803]),
            long(tags::STRIP_OFFSETS, &[0]), // patched below
            short(tags::SAMPLES_PER_PIXEL, &[1]),
            long(tags::ROWS_PER_STRIP, &[height]),
            long(tags::STRIP_BYTE_COUNTS, &[declared_count]),
        ];
        if let Some(pattern) = cfa {
            raw.push(short(tags::CFA_REPEAT_PATTERN_DIM, &[2, 2]));
            raw.push((tags::CFA_PATTERN, BYTE_T, 4, pattern.to_vec()));
        }
        raw.extend(extra_raw);
        ifd0.sort_by_key(|entry| entry.0);
        raw.sort_by_key(|entry| entry.0);

        let ifd0_size = 2 + 12 * ifd0.len() + 4;
        let raw_offset = 8 + ifd0_size;
        let raw_size = 2 + 12 * raw.len() + 4;
        let tables_end = raw_offset + raw_size;
        for entry in &mut ifd0 {
            if entry.0 == tags::SUB_IFDS {
                entry.3 = (raw_offset as u32).to_le_bytes().to_vec();
            }
        }

        let mut bytes = vec![0_u8; tables_end];
        bytes[..8].copy_from_slice(&[b'I', b'I', 42, 0, 8, 0, 0, 0]);
        let mut extra: Vec<u8> = Vec::new();
        let mut cursor = tables_end;

        let write_ifd = |bytes: &mut [u8],
                         at: usize,
                         entries: &[TestEntry],
                         extra: &mut Vec<u8>,
                         cursor: &mut usize,
                         strip_data: Option<&[u8]>|
         -> Option<usize> {
            bytes[at..at + 2].copy_from_slice(&(entries.len() as u16).to_le_bytes());
            let mut strip_value_slot = None;
            for (index, (tag, field_type, count, data)) in entries.iter().enumerate() {
                let start = at + 2 + index * 12;
                bytes[start..start + 2].copy_from_slice(&tag.to_le_bytes());
                bytes[start + 2..start + 4].copy_from_slice(&field_type.to_le_bytes());
                bytes[start + 4..start + 8].copy_from_slice(&count.to_le_bytes());
                if *tag == tags::STRIP_OFFSETS {
                    strip_value_slot = Some(start + 8);
                    continue;
                }
                if data.len() <= 4 {
                    let mut inline = [0_u8; 4];
                    inline[..data.len()].copy_from_slice(data);
                    bytes[start + 8..start + 12].copy_from_slice(&inline);
                } else {
                    bytes[start + 8..start + 12].copy_from_slice(&(*cursor as u32).to_le_bytes());
                    extra.extend_from_slice(data);
                    *cursor += data.len();
                }
            }
            let next_at = at + 2 + entries.len() * 12;
            bytes[next_at..next_at + 4].copy_from_slice(&0_u32.to_le_bytes());
            if let Some(data) = strip_data {
                extra.extend_from_slice(data);
            }
            strip_value_slot
        };

        let none_slot = write_ifd(&mut bytes, 8, &ifd0, &mut extra, &mut cursor, None);
        assert!(none_slot.is_none());
        let strip_slot = write_ifd(
            &mut bytes,
            raw_offset,
            &raw,
            &mut extra,
            &mut cursor,
            Some(strip_data),
        );
        let strip_offset = cursor as u32;
        bytes.extend_from_slice(&extra);
        let slot = strip_slot.expect("raw IFD carries the strip offset entry");
        bytes[slot..slot + 4].copy_from_slice(&strip_offset.to_le_bytes());
        bytes
    }

    fn parse_fixture(bytes: &[u8]) -> CameraFile<'_> {
        CameraFile::parse_tiff(FORMAT, bytes).expect("synthetic ORF fixture parses")
    }

    /// Independent reference: dcraw `packed_load_raw` with `tiff_bps` 12 and
    /// `load_flags` 24 (32-bit little-endian words consumed MSB-first).
    fn dcraw_reference_olympus12(data: &[u8], pixels: usize) -> Vec<u16> {
        let mut bitbuf = 0_u64;
        let mut vbits = 0_i64;
        let mut position = 0_usize;
        (0..pixels)
            .map(|_| {
                vbits -= 12;
                while vbits < 0 {
                    bitbuf <<= 32;
                    for i in 0..4 {
                        bitbuf |= u64::from(data.get(position).copied().unwrap_or(0)) << (i * 8);
                        position += 1;
                    }
                    vbits += 32;
                }
                let shift = (64 - 12 - vbits) as u32;
                ((bitbuf << shift) >> (64 - 12)) as u16
            })
            .collect()
    }

    #[test]
    fn recognizes_orf_magics() {
        assert!(orf_magic(b"IIRO\x08\0\0\0"));
        assert!(orf_magic(b"IIRS\x08\0\0\0"));
        assert!(orf_magic(b"MMOR\0\0\0\x08"));
        assert!(orf_magic(b"MMSR\0\0\0\x08"));
        assert!(!orf_magic(b"II*\0\x08\0\0\0"));
        assert!(!orf_magic(b"MM\0*\0\0\0\x08"));
        assert!(!orf_magic(b"II"));
    }

    #[test]
    fn parse_container_rejects_non_orf_bytes() {
        let bytes = build_fixture(8, 2, 12, 1, &[0_u8; 24], 24, Some([0, 1, 1, 2]), Vec::new());
        let error = OrfQuirks.parse_container(&bytes).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: FORMAT, .. }),
            "standard TIFF magic must not enter the ORF backend: {error}"
        );
    }

    #[test]
    fn selects_subifd_and_reads_metadata() {
        // ActiveArea (top=0, left=2, bottom=2, right=6) and WhiteLevel.
        let extra = vec![long(ACTIVE_AREA, &[0, 2, 2, 6]), short(WHITE_LEVEL, &[3_995])];
        let strip = vec![0_u8; 24];
        let bytes = build_fixture(8, 2, 12, 1, &strip, 24, Some([0, 1, 1, 2]), extra);
        let file = parse_fixture(&bytes);
        let raw = OrfQuirks.select_raw_ifd(&file).expect("raw IFD selected");
        assert!(!raw.is_top_level(), "the raw image must come from the SubIFD");
        let metadata = OrfQuirks.read_metadata(&file, raw).expect("metadata");
        assert_eq!((metadata.width, metadata.height), (8, 2));
        assert_eq!(metadata.bits_per_sample, 12);
        assert_eq!(metadata.make, "OLYMPUS");
        assert_eq!(metadata.model, "E-TEST");
        assert_eq!(metadata.orientation, Orientation::Normal);
        assert_eq!(
            metadata.cfa.cells,
            [CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue]
        );
        assert_eq!(metadata.black_level.values, [0.0]);
        assert_eq!(metadata.white_level.0, [3_995.0]);
        assert_eq!(metadata.white_balance, [1.0; 4]);
        assert_eq!(metadata.active_area, Some(Rect::new(2, 0, 4, 2)));
        assert_eq!(metadata.crop_area, None);
    }

    #[test]
    fn white_level_defaults_to_full_scale() {
        let strip = vec![0_u8; 24];
        let bytes = build_fixture(8, 2, 12, 1, &strip, 24, Some([1, 0, 2, 1]), Vec::new());
        let file = parse_fixture(&bytes);
        let raw = OrfQuirks.select_raw_ifd(&file).unwrap();
        let metadata = OrfQuirks.read_metadata(&file, raw).unwrap();
        assert_eq!(metadata.white_level.0, [4_095.0]);
    }

    #[test]
    fn olympus12_unpacking_matches_dcraw_reference() {
        let width = 8_u32;
        let height = 2_u32;
        let pixels = (width * height) as usize;
        // 16 pixels * 12 bits = 24 stream bytes, already 4-byte aligned.
        let strip: Vec<u8> = (0..24).map(|i| (i * 7 + 3) as u8).collect();
        let bytes = build_fixture(
            width,
            height,
            12,
            1,
            &strip,
            strip.len() as u32,
            Some([0, 1, 1, 2]),
            Vec::new(),
        );
        let file = parse_fixture(&bytes);
        let raw = OrfQuirks.select_raw_ifd(&file).unwrap();
        let decoded = OrfQuirks.decode_pixels(&file, raw, &|| false).unwrap();
        assert_eq!(decoded, dcraw_reference_olympus12(&strip, pixels));
        // Hand-computed first group: p0 = b3<<4 | b2>>4, p1 = (b2&0xF)<<8 | b1.
        assert_eq!(
            decoded[0],
            (u16::from(strip[3]) << 4) | (u16::from(strip[2]) >> 4)
        );
        assert_eq!(
            decoded[1],
            ((u16::from(strip[2]) & 0x0f) << 8) | u16::from(strip[1])
        );
    }

    #[test]
    fn uncompressed16_decodes_little_endian() {
        let width = 4_u32;
        let height = 2_u32;
        let samples: Vec<u16> = (0..8).map(|i| 1_000 + i * 100).collect();
        let strip: Vec<u8> = samples.iter().flat_map(|value| value.to_le_bytes()).collect();
        let bytes = build_fixture(
            width,
            height,
            16,
            1,
            &strip,
            strip.len() as u32,
            Some([0, 1, 1, 2]),
            Vec::new(),
        );
        let file = parse_fixture(&bytes);
        let raw = OrfQuirks.select_raw_ifd(&file).unwrap();
        let decoded = OrfQuirks.decode_pixels(&file, raw, &|| false).unwrap();
        assert_eq!(decoded, samples);
    }

    #[test]
    fn rejects_unsupported_compression() {
        let strip = vec![0_u8; 24];
        let bytes = build_fixture(8, 2, 12, 6, &strip, 24, Some([0, 1, 1, 2]), Vec::new());
        let file = parse_fixture(&bytes);
        let raw = OrfQuirks.select_raw_ifd(&file).unwrap();
        let error = OrfQuirks.decode_pixels(&file, raw, &|| false).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: FORMAT, ref message }
                if message.contains("compression 6")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_unverifiable_packed_byte_count() {
        // 16 pixels at 12 bits need 24 bytes; declaring 32 (the 16-bit size)
        // means the storage cannot be verified and must not be guessed.
        let strip = vec![0_u8; 32];
        let bytes = build_fixture(8, 2, 12, 1, &strip, 32, Some([0, 1, 1, 2]), Vec::new());
        let file = parse_fixture(&bytes);
        let raw = OrfQuirks.select_raw_ifd(&file).unwrap();
        let error = OrfQuirks.decode_pixels(&file, raw, &|| false).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: FORMAT, ref message }
                if message.contains("12-bit packed strip")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_unsupported_bit_depth() {
        let strip = vec![0_u8; 24];
        let bytes = build_fixture(8, 2, 14, 1, &strip, 24, Some([0, 1, 1, 2]), Vec::new());
        let file = parse_fixture(&bytes);
        let raw = OrfQuirks.select_raw_ifd(&file).unwrap();
        for error in [
            OrfQuirks.read_metadata(&file, raw).unwrap_err(),
            OrfQuirks.decode_pixels(&file, raw, &|| false).unwrap_err(),
        ] {
            assert!(
                matches!(error, DecodeError::NativeCamera { format: FORMAT, ref message }
                    if message.contains("bit depth 14")),
                "unexpected error: {error}"
            );
        }
    }

    #[test]
    fn rejects_missing_cfa_tags() {
        let strip = vec![0_u8; 24];
        let bytes = build_fixture(8, 2, 12, 1, &strip, 24, None, Vec::new());
        let file = parse_fixture(&bytes);
        let raw = OrfQuirks.select_raw_ifd(&file).unwrap();
        let error = OrfQuirks.read_metadata(&file, raw).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: FORMAT, ref message }
                if message.contains("CFARepeatPatternDim")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_non_bayer_cfa() {
        let strip = vec![0_u8; 24];
        // Four green cells: 2x2 but not a 1R/2G/1B Bayer quad.
        let bytes = build_fixture(8, 2, 12, 1, &strip, 24, Some([1, 1, 1, 1]), Vec::new());
        let file = parse_fixture(&bytes);
        let raw = OrfQuirks.select_raw_ifd(&file).unwrap();
        let error = OrfQuirks.read_metadata(&file, raw).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: FORMAT, ref message }
                if message.contains("Bayer")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn cancellation_is_typed() {
        let strip = vec![0_u8; 24];
        let bytes = build_fixture(8, 2, 12, 1, &strip, 24, Some([0, 1, 1, 2]), Vec::new());
        let file = parse_fixture(&bytes);
        let raw = OrfQuirks.select_raw_ifd(&file).unwrap();
        let error = OrfQuirks.decode_pixels(&file, raw, &|| true).unwrap_err();
        assert!(
            matches!(error, DecodeError::Cancelled),
            "unexpected error: {error}"
        );
    }
}
