//! Canon CR2 quirks (Stage 2 implementation).
//!
//! CR2 is a little-endian TIFF with a 16-byte header:
//!
//! - `0x00`: `II` byte-order marker (CR2 is little-endian by definition);
//! - `0x02`: classic TIFF magic 42;
//! - `0x04`: `u32` offset of IFD0 (normally 16);
//! - `0x08`: the CR2 signature `CR\x02\0`;
//! - `0x0C`: `u32` absolute offset of the raw (CFA) IFD;
//! - `0x10`: IFD0 (Make/Model/Orientation, thumbnail chain).
//!
//! NOTE on the Stage 2 brief: the brief placed the raw-IFD pointer at file
//! offset 16. That contradicts the CR2 layout documented by `ExifTool`
//! (`ProcessTIFF`: "Canon CR2 images should have an offset of 16" refers to
//! the IFD0 offset stored at bytes 4..8, with the signature at 8..12 and the
//! raw-IFD pointer at 12..16), by the lclevy.free.fr/cr2 format write-up
//! (referenced from `RawSpeed`'s `Cr2Decoder`), and by RawSpeed/dcraw, which
//! reach the raw IFD through the top-level next-IFD chain it terminates.
//! Reading a `u32` at offset 16 would read IFD0's entry count instead. This
//! implementation therefore reads the pointer at `0x0C` (12) and treats a
//! mismatch as a typed error. Flagged for the orchestrator.
//!
//! [`Cr2Quirks::select_raw_ifd`] validates the header pointer with
//! [`CameraFile::parse_ifd_at`] and then returns the matching directory from
//! the already-walked top-level chain (the raw IFD terminates that chain in
//! CR2), which is the only way to satisfy the trait's borrowed return type.
//!
//! # Pixel storage
//!
//! The raw IFD stores one lossless-JPEG (TIFF compression 7) strip. The
//! stream serializes the image as vertical slices (`CR2Slice` tag `0xC640`,
//! SHORT `[count, slice_width, last_slice_width]`): all rows of slice 0,
//! then all rows of slice 1, and so on, with `count` full-width slices plus
//! one final slice of `last_slice_width` (semantics per dcraw
//! `lossless_jpeg_load_raw` and `RawSpeed` `Cr2SliceWidths`, where
//! `num_slices = 1 + count`). After decoding, samples are scattered from
//! slice-major stream order into row-major sensor order.
//!
//! # Typed rejections (no silent degradation, never the embedded JPEG)
//!
//! - missing/wrong `CR\x02\0` signature, big-endian container, zero or
//!   dangling raw-IFD pointer, raw IFD not on the top-level chain;
//! - any `Compression` other than 7 (e.g. uncompressed variants);
//! - `BitsPerSample` outside 2..=16 or mismatching the JPEG precision;
//! - non-CFA `PhotometricInterpretation`, `SamplesPerPixel` != 1, tile
//!   storage, or multi-strip storage;
//! - 3+ JPEG components (subsampled Canon sRAW/mRAW YCC) — not a CFA mosaic;
//! - slice geometry that does not exactly cover `ImageWidth`, or JPEG
//!   dimensions that do not match the IFD geometry;
//! - `raw_width == 3984` (EOS-1D family two-column shift quirk in dcraw),
//!   which would silently misplace pixels without the quirk;
//! - non-2x2 or non-RGB CFA patterns.
//!
//! # Tests
//!
//! All tests build synthetic minimal CR2 byte layouts in memory (no licensed
//! camera files). They validate the header/signature handling, raw-IFD
//! selection, slice geometry parsing, the full LJPEG+slices decode path with
//! a purpose-built synthetic lossless-JPEG encoder, and the typed
//! rejections. They are parser unit tests, NOT proof of compatibility with
//! files produced by real Canon cameras.

use rrrah_core::{
    CfaColor, CfaPattern, DngColorMatrix, LevelGrid, Orientation, Rect, WhiteLevel, select_dng_xyz_to_camera,
};

use crate::{
    DecodeError,
    dng::lossless_jpeg::{self, LosslessJpegError},
};

use super::{
    CameraDirectory, CameraFile, CameraMetadata, CameraQuirks, camera_error, optional_ascii, optional_scalar,
    orientation_from_tag, required_scalar, tags,
};

const FORMAT: &str = "CR2";

/// CR2 signature bytes at file offset 8.
const SIGNATURE: &[u8; 4] = b"CR\x02\0";
/// File offset of the `u32` raw-IFD pointer in the CR2 header.
const RAW_IFD_POINTER_OFFSET: usize = 12;

/// `CR2Slice` (RawImageSegmentation): SHORT `[count, width, last_width]`.
const TAG_CR2_SLICE: u16 = 0xC640;
/// DNG `CFAPattern` (fallback when TIFF/EP 33422 is absent).
const TAG_DNG_CFA_PATTERN: u16 = 0xC612;
/// DNG `BlackLevelRepeatDim` `[rows, columns]`.
const TAG_BLACK_LEVEL_REPEAT_DIM: u16 = 0xC619;
/// DNG `BlackLevel`.
const TAG_BLACK_LEVEL: u16 = 0xC61A;
/// DNG `WhiteLevel`.
const TAG_WHITE_LEVEL: u16 = 0xC61D;
/// DNG `DefaultCropOrigin` `[x, y]`.
const TAG_DEFAULT_CROP_ORIGIN: u16 = 0xC61F;
/// DNG `DefaultCropSize` `[width, height]`.
const TAG_DEFAULT_CROP_SIZE: u16 = 0xC620;
/// DNG `ColorMatrix1` / `ColorMatrix2` (SRATIONAL x9).
const TAG_COLOR_MATRIX_1: u16 = 0xC621;
const TAG_COLOR_MATRIX_2: u16 = 0xC622;
/// DNG `AsShotNeutral` (RATIONAL x3).
const TAG_AS_SHOT_NEUTRAL: u16 = 0xC628;
/// DNG `CalibrationIlluminant1` / `CalibrationIlluminant2`.
const TAG_CALIBRATION_ILLUMINANT_1: u16 = 0xC65A;
const TAG_CALIBRATION_ILLUMINANT_2: u16 = 0xC65B;
/// DNG `ActiveArea` `[top, left, bottom, right]`.
const TAG_ACTIVE_AREA: u16 = 0xC68D;

/// EOS-1D/1Ds width that dcraw shifts by two columns with wraparound; without
/// that model quirk the pixels would be silently misplaced, so reject it.
const UNSUPPORTED_LEGACY_WIDTH: u64 = 3_984;

/// Registered Canon CR2 quirks.
#[derive(Debug)]
pub(crate) struct Cr2Quirks;

impl CameraQuirks for Cr2Quirks {
    fn format_name(&self) -> &'static str {
        FORMAT
    }

    fn parse_container<'a>(&self, data: &'a [u8]) -> Result<CameraFile<'a>, DecodeError> {
        if data.get(0..2) != Some(b"II") {
            return Err(camera_error(
                FORMAT,
                "CR2 must be a little-endian (II) TIFF container",
            ));
        }
        if data.get(8..12) != Some(SIGNATURE.as_slice()) {
            return Err(camera_error(
                FORMAT,
                "missing CR\\x02\\0 signature at file offset 8; not a CR2 file",
            ));
        }
        CameraFile::parse_tiff(FORMAT, data)
    }

    fn select_raw_ifd<'a>(
        &self,
        container: &'a CameraFile<'a>,
    ) -> Result<&'a CameraDirectory<'a>, DecodeError> {
        let pointer_bytes = container
            .data()
            .get(RAW_IFD_POINTER_OFFSET..RAW_IFD_POINTER_OFFSET + 4)
            .ok_or_else(|| {
                camera_error(
                    FORMAT,
                    "truncated CR2 header: raw-IFD pointer at offset 12 is missing",
                )
            })?;
        let raw_ifd_offset = u64::from(container.byte_order().u32(pointer_bytes));
        if raw_ifd_offset == 0 {
            return Err(camera_error(FORMAT, "CR2 raw-IFD pointer at offset 12 is zero"));
        }
        // Validate the pointer eagerly so a garbage header fails here with a
        // typed error instead of surfacing later as an unrelated mismatch.
        container.parse_ifd_at(FORMAT, raw_ifd_offset)?;
        // The trait returns a borrowed directory, so the raw IFD must be one
        // of the already-walked directories. In CR2 the raw IFD terminates
        // the top-level next-IFD chain, which `CameraFile::parse_tiff` walks.
        container
            .directories()
            .iter()
            .find(|directory| directory.offset() == raw_ifd_offset)
            .ok_or_else(|| {
                camera_error(
                    FORMAT,
                    format!("raw IFD at offset {raw_ifd_offset} is not on the top-level directory chain"),
                )
            })
    }

    fn read_metadata(
        &self,
        container: &CameraFile<'_>,
        raw: &CameraDirectory<'_>,
    ) -> Result<CameraMetadata, DecodeError> {
        let width = u32_from_scalar(
            required_scalar(FORMAT, raw, tags::IMAGE_WIDTH)?,
            tags::IMAGE_WIDTH,
        )?;
        let height = u32_from_scalar(
            required_scalar(FORMAT, raw, tags::IMAGE_LENGTH)?,
            tags::IMAGE_LENGTH,
        )?;
        let bits = required_scalar(FORMAT, raw, tags::BITS_PER_SAMPLE)?;
        let bits_per_sample = u8::try_from(bits)
            .ok()
            .filter(|bits| (2..=16).contains(bits))
            .ok_or_else(|| {
                camera_error(
                    FORMAT,
                    format!("unsupported BitsPerSample {bits}; expected 2..=16"),
                )
            })?;

        let ifd0 = container
            .directories()
            .first()
            .ok_or_else(|| camera_error(FORMAT, "container has no IFD0"))?;
        let make = optional_ascii(FORMAT, ifd0, tags::MAKE)?.unwrap_or_else(|| "Canon".to_owned());
        let model = optional_ascii(FORMAT, ifd0, tags::MODEL)?.unwrap_or_default();
        let orientation = match optional_scalar(FORMAT, ifd0, tags::ORIENTATION)? {
            Some(value) => orientation_from_tag(FORMAT, value)?,
            None => Orientation::Normal,
        };

        let cfa = read_cfa(raw)?;
        let black_level = read_black_level(raw, bits_per_sample)?;
        let white_level = read_white_level(raw, bits_per_sample)?;
        let white_balance = read_white_balance(raw)?;
        let xyz_to_camera = read_xyz_to_camera(raw)?;
        let active_area = read_active_area(raw)?;
        let crop_area = read_crop_area(raw)?;

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

    // Linear validation pipeline; splitting it would obscure the reject order.
    #[allow(clippy::too_many_lines)]
    fn decode_pixels(
        &self,
        container: &CameraFile<'_>,
        raw: &CameraDirectory<'_>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<u16>, DecodeError> {
        let width = required_scalar(FORMAT, raw, tags::IMAGE_WIDTH)?;
        let height = required_scalar(FORMAT, raw, tags::IMAGE_LENGTH)?;
        if width == 0 || height == 0 {
            return Err(camera_error(FORMAT, "raw IFD declares an empty image"));
        }
        if width == UNSUPPORTED_LEGACY_WIDTH {
            return Err(camera_error(
                FORMAT,
                "EOS-1D family 3984-wide layout needs a two-column shift quirk that is not implemented",
            ));
        }
        let bits = required_scalar(FORMAT, raw, tags::BITS_PER_SAMPLE)?;
        let bits_per_sample = u8::try_from(bits)
            .ok()
            .filter(|bits| (2..=16).contains(bits))
            .ok_or_else(|| {
                camera_error(
                    FORMAT,
                    format!("unsupported BitsPerSample {bits}; expected 2..=16"),
                )
            })?;

        let compression =
            optional_scalar(FORMAT, raw, tags::COMPRESSION)?.unwrap_or(tags::COMPRESSION_UNCOMPRESSED);
        if compression != tags::COMPRESSION_LOSSLESS_JPEG {
            return Err(camera_error(
                FORMAT,
                format!("unsupported compression {compression}; CR2 support is limited to lossless JPEG (7)"),
            ));
        }
        match optional_scalar(FORMAT, raw, tags::PHOTOMETRIC_INTERPRETATION)? {
            Some(tags::PHOTOMETRIC_CFA) => {}
            Some(actual) => {
                return Err(camera_error(
                    FORMAT,
                    format!("unsupported PhotometricInterpretation {actual}; only CFA (32803) is decodable"),
                ));
            }
            None => {
                return Err(camera_error(
                    FORMAT,
                    "raw IFD is missing PhotometricInterpretation",
                ));
            }
        }
        if let Some(samples) = optional_scalar(FORMAT, raw, tags::SAMPLES_PER_PIXEL)?
            && samples != 1
        {
            return Err(camera_error(
                FORMAT,
                format!("unsupported SamplesPerPixel {samples}; only single-plane CFA is decodable"),
            ));
        }
        if raw.entry(FORMAT, tags::TILE_OFFSETS)?.is_some() {
            return Err(camera_error(FORMAT, "tiled CR2 raw storage is not supported"));
        }

        let offsets = required_unsigned_vec(raw, tags::STRIP_OFFSETS)?;
        let byte_counts = required_unsigned_vec(raw, tags::STRIP_BYTE_COUNTS)?;
        if offsets.len() != 1 || byte_counts.len() != 1 {
            return Err(camera_error(
                FORMAT,
                format!(
                    "expected exactly one lossless-JPEG strip, found {} offsets and {} byte counts",
                    offsets.len(),
                    byte_counts.len()
                ),
            ));
        }
        let strip_offset = usize_from_u64(offsets[0], "strip offset")?;
        let strip_len = usize_from_u64(byte_counts[0], "strip byte count")?;
        let strip_end = strip_offset
            .checked_add(strip_len)
            .ok_or_else(|| camera_error(FORMAT, "arithmetic overflow computing strip end"))?;
        let strip = container
            .data()
            .get(strip_offset..strip_end)
            .ok_or_else(|| camera_error(FORMAT, "strip range is outside the bounded file data"))?;

        // Slice layout: `count` full slices of `slice_width` plus one final
        // slice of `last_width`; absent tag or `count == 0` means unsliced.
        let slices = match raw.entry(FORMAT, TAG_CR2_SLICE)? {
            Some(entry) => {
                let values = entry
                    .unsigned_values()
                    .map_err(|error| camera_error(FORMAT, format!("CR2Slice tag: {error}")))?;
                if values.len() != 3 {
                    return Err(camera_error(
                        FORMAT,
                        format!("CR2Slice tag has {} values, expected 3", values.len()),
                    ));
                }
                Some((values[0], values[1], values[2]))
            }
            None => None,
        };

        let image = lossless_jpeg::decode(strip, cancelled).map_err(|error| map_ljpeg_error(&error))?;
        if image.precision != bits_per_sample {
            return Err(camera_error(
                FORMAT,
                format!(
                    "lossless JPEG precision {} does not match BitsPerSample {bits_per_sample}",
                    image.precision
                ),
            ));
        }
        let components = image.component_ids.len();
        if components > 2 {
            return Err(camera_error(
                FORMAT,
                format!(
                    "lossless JPEG has {components} components; subsampled Canon sRAW/mRAW is not a decodable CFA mosaic"
                ),
            ));
        }
        let jpeg_width = usize::from(image.width)
            .checked_mul(components)
            .ok_or_else(|| camera_error(FORMAT, "arithmetic overflow computing JPEG row width"))?;
        let jpeg_height = usize::from(image.height);
        let width_usize = usize_from_u64(width, "image width")?;
        let height_usize = usize_from_u64(height, "image height")?;
        if jpeg_height != height_usize {
            return Err(camera_error(
                FORMAT,
                format!("lossless JPEG height {jpeg_height} does not match ImageLength {height_usize}"),
            ));
        }
        let total = width_usize
            .checked_mul(height_usize)
            .ok_or(DecodeError::DimensionOverflow)?;

        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(total)
            .map_err(|_| camera_error(FORMAT, format!("could not allocate {total} output samples")))?;
        pixels.resize(total, 0);

        match slices {
            Some((count, slice_width, last_width)) if count >= 1 => {
                let slice_width = usize_from_u64(slice_width, "CR2Slice width")?;
                let last_width = usize_from_u64(last_width, "CR2Slice last width")?;
                if slice_width == 0 || last_width == 0 {
                    return Err(camera_error(FORMAT, "CR2Slice declares a zero-width slice"));
                }
                let count = usize_from_u64(count, "CR2Slice count")?;
                let expected_width = count
                    .checked_mul(slice_width)
                    .and_then(|full| full.checked_add(last_width))
                    .ok_or_else(|| {
                        camera_error(FORMAT, "arithmetic overflow computing slice layout width")
                    })?;
                if expected_width != width_usize || jpeg_width != width_usize {
                    return Err(camera_error(
                        FORMAT,
                        format!(
                            "slice layout ({count} x {slice_width} + {last_width} = {expected_width}) and JPEG width {jpeg_width} do not match ImageWidth {width_usize}"
                        ),
                    ));
                }
                let slice_span = slice_width
                    .checked_mul(height_usize)
                    .ok_or_else(|| camera_error(FORMAT, "arithmetic overflow computing slice sample span"))?;
                for (stream_index, &sample) in image.samples.iter().enumerate() {
                    if stream_index.is_multiple_of(65_536) && cancelled() {
                        return Err(DecodeError::Cancelled);
                    }
                    let mut slice_index = stream_index / slice_span;
                    let is_last = slice_index >= count;
                    if is_last {
                        slice_index = count;
                    }
                    let within = stream_index - slice_index * slice_span;
                    let width_here = if is_last { last_width } else { slice_width };
                    let row = within / width_here;
                    let column = within % width_here + slice_index * slice_width;
                    // Geometry validation above guarantees row < height and
                    // column < width, so this index is in bounds.
                    pixels[row * width_usize + column] = sample;
                }
            }
            _ => {
                if jpeg_width != width_usize {
                    return Err(camera_error(
                        FORMAT,
                        format!(
                            "unsliced lossless JPEG width {jpeg_width} does not match ImageWidth {width_usize}"
                        ),
                    ));
                }
                if cancelled() {
                    return Err(DecodeError::Cancelled);
                }
                pixels.copy_from_slice(&image.samples);
            }
        }
        Ok(pixels)
    }
}

/// Maps a lossless-JPEG failure into the camera error type, preserving
/// cancellation as `DecodeError::Cancelled`.
fn map_ljpeg_error(error: &LosslessJpegError) -> DecodeError {
    if matches!(error, LosslessJpegError::Cancelled { .. }) {
        DecodeError::Cancelled
    } else {
        camera_error(FORMAT, format!("lossless JPEG: {error}"))
    }
}

fn u32_from_scalar(value: u64, tag: u16) -> Result<u32, DecodeError> {
    u32::try_from(value).map_err(|_| camera_error(FORMAT, format!("tag {tag} value {value} exceeds u32")))
}

fn usize_from_u64(value: u64, context: &'static str) -> Result<usize, DecodeError> {
    usize::try_from(value).map_err(|_| camera_error(FORMAT, format!("{context} does not fit usize")))
}

/// Reads an optional numeric (any int/rational type) tag as f64 values.
fn optional_numeric_vec(directory: &CameraDirectory<'_>, tag: u16) -> Result<Option<Vec<f64>>, DecodeError> {
    directory
        .entry(FORMAT, tag)?
        .map(|entry| {
            entry
                .numeric_values()
                .map_err(|error| camera_error(FORMAT, format!("tag {tag}: {error}")))
        })
        .transpose()
}

/// Reads an optional unsigned-integer tag as u64 values.
fn optional_unsigned_vec(directory: &CameraDirectory<'_>, tag: u16) -> Result<Option<Vec<u64>>, DecodeError> {
    directory
        .entry(FORMAT, tag)?
        .map(|entry| {
            entry
                .unsigned_values()
                .map_err(|error| camera_error(FORMAT, format!("tag {tag}: {error}")))
        })
        .transpose()
}

/// Reads a required unsigned-integer tag as u64 values.
fn required_unsigned_vec(directory: &CameraDirectory<'_>, tag: u16) -> Result<Vec<u64>, DecodeError> {
    optional_unsigned_vec(directory, tag)?
        .ok_or_else(|| camera_error(FORMAT, format!("required tag {tag} is missing")))
}

fn finite_f32(value: f64, context: &'static str) -> Result<f32, DecodeError> {
    #[allow(clippy::cast_possible_truncation)]
    let converted = value as f32;
    if converted.is_finite() {
        Ok(converted)
    } else {
        Err(camera_error(
            FORMAT,
            format!("{context} is outside finite f32 range"),
        ))
    }
}

/// Maps a TIFF/EP CFA color code (0=Red .. 6=White) to the domain color.
fn cfa_color(value: u64) -> Result<CfaColor, DecodeError> {
    match value {
        0 => Ok(CfaColor::Red),
        1 => Ok(CfaColor::Green),
        2 => Ok(CfaColor::Blue),
        3 => Ok(CfaColor::Cyan),
        4 => Ok(CfaColor::Magenta),
        5 => Ok(CfaColor::Yellow),
        6 => Ok(CfaColor::White),
        actual => Err(camera_error(
            FORMAT,
            format!("unsupported CFA color code {actual}"),
        )),
    }
}

/// Builds the CFA pattern from TIFF/EP `CFARepeatPatternDim` (33421) plus
/// `CFAPattern` (33422), falling back to the DNG `CFAPattern` (0xC612) that
/// newer CR2s also carry. The display pipeline requires a 2x2 RGB Bayer
/// mosaic; anything else is a typed rejection.
fn read_cfa(raw: &CameraDirectory<'_>) -> Result<CfaPattern, DecodeError> {
    let pattern_values = match optional_unsigned_vec(raw, tags::CFA_PATTERN)? {
        Some(values) => values,
        None => optional_unsigned_vec(raw, TAG_DNG_CFA_PATTERN)?
            .ok_or_else(|| camera_error(FORMAT, "raw IFD has no CFAPattern (33422 or 0xC612)"))?,
    };
    let (rows, columns) = match optional_unsigned_vec(raw, tags::CFA_REPEAT_PATTERN_DIM)? {
        Some(dims) => {
            if dims.len() != 2 {
                return Err(camera_error(
                    FORMAT,
                    format!("CFARepeatPatternDim has {} values, expected 2", dims.len()),
                ));
            }
            (dims[0], dims[1])
        }
        // Without an explicit repeat dimension only a 2x2 Bayer is plausible.
        None => (2, 2),
    };
    let expected = rows
        .checked_mul(columns)
        .ok_or_else(|| camera_error(FORMAT, "arithmetic overflow computing CFA cell count"))?;
    if expected == 0 || expected != pattern_values.len() as u64 {
        return Err(camera_error(
            FORMAT,
            format!(
                "CFAPattern has {} cells for a {rows}x{columns} repeat",
                pattern_values.len()
            ),
        ));
    }
    let cells = pattern_values
        .iter()
        .map(|&value| cfa_color(value))
        .collect::<Result<Vec<_>, _>>()?;
    let cfa = CfaPattern {
        width: u8::try_from(columns).map_err(|_| camera_error(FORMAT, "CFA width exceeds 255"))?,
        height: u8::try_from(rows).map_err(|_| camera_error(FORMAT, "CFA height exceeds 255"))?,
        cells,
    };
    cfa.bayer_quad().map_err(|_| {
        camera_error(
            FORMAT,
            "unsupported CFA layout; only a 2x2 RGB Bayer mosaic is decodable",
        )
    })?;
    Ok(cfa)
}

/// Builds the black-level grid from `BlackLevelRepeatDim` + `BlackLevel`.
fn read_black_level(raw: &CameraDirectory<'_>, bits_per_sample: u8) -> Result<LevelGrid, DecodeError> {
    let Some(values) = optional_numeric_vec(raw, TAG_BLACK_LEVEL)? else {
        return Ok(LevelGrid {
            width: 1,
            height: 1,
            components: 1,
            values: vec![0.0],
        });
    };
    let (rows, columns) = match optional_unsigned_vec(raw, TAG_BLACK_LEVEL_REPEAT_DIM)? {
        Some(dims) => {
            if dims.len() != 2 {
                return Err(camera_error(
                    FORMAT,
                    format!("BlackLevelRepeatDim has {} values, expected 2", dims.len()),
                ));
            }
            (dims[0], dims[1])
        }
        None => match values.len() {
            1 => (1, 1),
            4 => (2, 2),
            actual => {
                return Err(camera_error(
                    FORMAT,
                    format!("BlackLevel has {actual} values but no BlackLevelRepeatDim to interpret them"),
                ));
            }
        },
    };
    let expected = rows
        .checked_mul(columns)
        .ok_or_else(|| camera_error(FORMAT, "arithmetic overflow computing black-level grid"))?;
    if expected == 0 || expected != values.len() as u64 {
        return Err(camera_error(
            FORMAT,
            format!(
                "BlackLevel has {} values for a {rows}x{columns} repeat grid",
                values.len()
            ),
        ));
    }
    let maximum = f64::from((1_u32 << bits_per_sample) - 1);
    let values = values
        .into_iter()
        .map(|value| {
            if value < 0.0 || value > maximum {
                return Err(camera_error(
                    FORMAT,
                    format!("BlackLevel {value} is outside 0..={maximum} for {bits_per_sample}-bit data"),
                ));
            }
            finite_f32(value, "BlackLevel")
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LevelGrid {
        width: u8::try_from(columns)
            .map_err(|_| camera_error(FORMAT, "black-level grid width exceeds 255"))?,
        height: u8::try_from(rows)
            .map_err(|_| camera_error(FORMAT, "black-level grid height exceeds 255"))?,
        components: 1,
        values,
    })
}

/// Builds the white level, defaulting to the full-scale code value.
fn read_white_level(raw: &CameraDirectory<'_>, bits_per_sample: u8) -> Result<WhiteLevel, DecodeError> {
    let full_scale = f64::from((1_u32 << bits_per_sample) - 1);
    let values = match optional_numeric_vec(raw, TAG_WHITE_LEVEL)? {
        Some(values) if !values.is_empty() => values
            .into_iter()
            .map(|value| finite_f32(value, "WhiteLevel"))
            .collect::<Result<Vec<_>, _>>()?,
        _ => vec![finite_f32(full_scale, "WhiteLevel")?],
    };
    Ok(WhiteLevel(values))
}

/// Derives per-channel white-balance gains from `AsShotNeutral`, normalized
/// so the green gain is 1, mirrored into the two green rows of a Bayer quad.
fn read_white_balance(raw: &CameraDirectory<'_>) -> Result<[f32; 4], DecodeError> {
    let Some(neutral) = optional_numeric_vec(raw, TAG_AS_SHOT_NEUTRAL)? else {
        return Ok([1.0; 4]);
    };
    if neutral.len() != 3 {
        return Err(camera_error(
            FORMAT,
            format!("AsShotNeutral has {} values, expected 3", neutral.len()),
        ));
    }
    let mut gains = [0.0_f32; 3];
    for (gain, value) in gains.iter_mut().zip(neutral) {
        if value <= 0.0 {
            return Err(camera_error(
                FORMAT,
                format!("AsShotNeutral value {value} is not positive"),
            ));
        }
        *gain = finite_f32(1.0 / value, "white-balance gain")?;
    }
    let green = gains[1];
    if green <= 0.0 {
        return Err(camera_error(
            FORMAT,
            "AsShotNeutral produced a non-positive green gain",
        ));
    }
    for gain in &mut gains {
        *gain /= green;
    }
    Ok([gains[0], gains[1], gains[2], gains[1]])
}

/// Selects the D65-referenced XYZ->camera matrix from the CR2 calibration
/// pair, mirroring the DNG backend. Without usable matrices the identity
/// fallback is kept.
fn read_xyz_to_camera(raw: &CameraDirectory<'_>) -> Result<[[f32; 3]; 4], DecodeError> {
    let candidate = |matrix_tag: u16, illuminant_tag: u16| -> Result<Option<DngColorMatrix>, DecodeError> {
        let matrix = optional_numeric_vec(raw, matrix_tag)?;
        let illuminant = optional_scalar(FORMAT, raw, illuminant_tag)?
            .map(|value| {
                u16::try_from(value)
                    .map_err(|_| camera_error(FORMAT, format!("tag {illuminant_tag} exceeds u16")))
            })
            .transpose()?;
        Ok(matrix.filter(|flat| flat.len() == 9).map(|flat| DngColorMatrix {
            xyz_to_camera: [
                [flat[0], flat[1], flat[2]],
                [flat[3], flat[4], flat[5]],
                [flat[6], flat[7], flat[8]],
            ],
            illuminant,
        }))
    };
    let first = candidate(TAG_COLOR_MATRIX_1, TAG_CALIBRATION_ILLUMINANT_1)?;
    let second = candidate(TAG_COLOR_MATRIX_2, TAG_CALIBRATION_ILLUMINANT_2)?;
    let Some(matrix) = select_dng_xyz_to_camera(first, second) else {
        return Ok([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]]);
    };
    let mut result = [[0.0_f32; 3]; 4];
    for (destination, source) in result.iter_mut().zip(matrix) {
        for (destination, value) in destination.iter_mut().zip(source) {
            *destination = finite_f32(value, "color matrix")?;
        }
    }
    Ok(result)
}

/// Converts `ActiveArea` `[top, left, bottom, right]` into a sensor rect.
fn read_active_area(raw: &CameraDirectory<'_>) -> Result<Option<Rect>, DecodeError> {
    let Some(values) = optional_unsigned_vec(raw, TAG_ACTIVE_AREA)? else {
        return Ok(None);
    };
    if values.len() != 4 {
        return Err(camera_error(
            FORMAT,
            format!("ActiveArea has {} values, expected 4", values.len()),
        ));
    }
    let top = u32_from_scalar(values[0], TAG_ACTIVE_AREA)?;
    let left = u32_from_scalar(values[1], TAG_ACTIVE_AREA)?;
    let bottom = u32_from_scalar(values[2], TAG_ACTIVE_AREA)?;
    let right = u32_from_scalar(values[3], TAG_ACTIVE_AREA)?;
    let height = bottom
        .checked_sub(top)
        .ok_or_else(|| camera_error(FORMAT, "ActiveArea bottom is above its top"))?;
    let width = right
        .checked_sub(left)
        .ok_or_else(|| camera_error(FORMAT, "ActiveArea right is left of its left edge"))?;
    Ok(Some(Rect::new(left, top, width, height)))
}

/// Converts `DefaultCropOrigin` + `DefaultCropSize` into a crop rect.
fn read_crop_area(raw: &CameraDirectory<'_>) -> Result<Option<Rect>, DecodeError> {
    let (Some(origin), Some(size)) = (
        optional_numeric_vec(raw, TAG_DEFAULT_CROP_ORIGIN)?,
        optional_numeric_vec(raw, TAG_DEFAULT_CROP_SIZE)?,
    ) else {
        return Ok(None);
    };
    if origin.len() != 2 || size.len() != 2 {
        return Err(camera_error(
            FORMAT,
            format!(
                "DefaultCropOrigin/DefaultCropSize have {}/{} values, expected 2 each",
                origin.len(),
                size.len()
            ),
        ));
    }
    let exact = |value: f64, field: &'static str| -> Result<u32, DecodeError> {
        if !value.is_finite() || value < 0.0 || value > f64::from(u32::MAX) || value.fract() != 0.0 {
            return Err(camera_error(
                FORMAT,
                format!("{field}={value} is not a non-negative integer"),
            ));
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Ok(value as u32)
    };
    Ok(Some(Rect::new(
        exact(origin[0], "DefaultCropOrigin x")?,
        exact(origin[1], "DefaultCropOrigin y")?,
        exact(size[0], "DefaultCropSize width")?,
        exact(size[1], "DefaultCropSize height")?,
    )))
}

#[cfg(test)]
mod tests {
    //! Synthetic-CR2 unit tests. Every byte layout is built in the test; no
    //! camera-produced files are used. These tests exercise the parser and
    //! the slice scatter, not compatibility with real Canon cameras.

    use super::*;

    // ----- synthetic TIFF/CR2 builders -------------------------------------

    const TYPE_BYTE: u16 = 1;
    const TYPE_ASCII: u16 = 2;
    const TYPE_SHORT: u16 = 3;
    const TYPE_LONG: u16 = 4;
    const TYPE_RATIONAL: u16 = 5;
    const TYPE_SRATIONAL: u16 = 10;

    /// One pending IFD entry: tag, field type, element count, raw value bytes.
    type PendingEntry = (u16, u16, u32, Vec<u8>);

    fn short_entry(tag: u16, values: &[u16]) -> PendingEntry {
        let bytes = values.iter().flat_map(|value| value.to_le_bytes()).collect();
        (tag, TYPE_SHORT, u32::try_from(values.len()).unwrap(), bytes)
    }

    fn long_entry(tag: u16, values: &[u32]) -> PendingEntry {
        let bytes = values.iter().flat_map(|value| value.to_le_bytes()).collect();
        (tag, TYPE_LONG, u32::try_from(values.len()).unwrap(), bytes)
    }

    fn byte_entry(tag: u16, values: &[u8]) -> PendingEntry {
        (
            tag,
            TYPE_BYTE,
            u32::try_from(values.len()).unwrap(),
            values.to_vec(),
        )
    }

    fn ascii_entry(tag: u16, text: &str) -> PendingEntry {
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(0);
        (tag, TYPE_ASCII, u32::try_from(bytes.len()).unwrap(), bytes)
    }

    fn rational_entry(tag: u16, values: &[(u32, u32)]) -> PendingEntry {
        let bytes = values
            .iter()
            .flat_map(|&(num, den)| [num.to_le_bytes(), den.to_le_bytes()].concat())
            .collect();
        (tag, TYPE_RATIONAL, u32::try_from(values.len()).unwrap(), bytes)
    }

    fn srational_entry(tag: u16, values: &[(i32, i32)]) -> PendingEntry {
        let bytes = values
            .iter()
            .flat_map(|&(num, den)| [num.to_le_bytes(), den.to_le_bytes()].concat())
            .collect();
        (tag, TYPE_SRATIONAL, u32::try_from(values.len()).unwrap(), bytes)
    }

    /// Serializes one classic little-endian IFD at `offset`. Out-of-line
    /// values are appended to `blob`; `blob_start` is the absolute file
    /// offset at which the blob will be placed (shared by all IFDs, so it
    /// must sit after every IFD table).
    fn write_ifd(
        bytes: &mut [u8],
        offset: usize,
        entries: &[PendingEntry],
        next: u32,
        blob_start: usize,
        blob: &mut Vec<u8>,
    ) {
        let mut sorted = entries.to_vec();
        sorted.sort_by_key(|entry| entry.0);
        bytes[offset..offset + 2].copy_from_slice(&u16::try_from(sorted.len()).unwrap().to_le_bytes());
        for (index, (tag, field_type, count, value)) in sorted.iter().enumerate() {
            let at = offset + 2 + index * 12;
            bytes[at..at + 2].copy_from_slice(&tag.to_le_bytes());
            bytes[at + 2..at + 4].copy_from_slice(&field_type.to_le_bytes());
            bytes[at + 4..at + 8].copy_from_slice(&count.to_le_bytes());
            if value.len() <= 4 {
                let mut inline = [0_u8; 4];
                inline[..value.len()].copy_from_slice(value);
                bytes[at + 8..at + 12].copy_from_slice(&inline);
            } else {
                let value_offset = blob_start + blob.len();
                bytes[at + 8..at + 12].copy_from_slice(&u32::try_from(value_offset).unwrap().to_le_bytes());
                blob.extend_from_slice(value);
                if !blob.len().is_multiple_of(2) {
                    blob.push(0);
                }
            }
        }
        let next_at = offset + 2 + sorted.len() * 12;
        bytes[next_at..next_at + 4].copy_from_slice(&next.to_le_bytes());
    }

    /// Assembles a synthetic CR2: 16-byte header, IFD0 at 16, raw IFD chained
    /// after IFD0 and pointed at by the header pointer at offset 12, then the
    /// shared value blob, then the strip bytes.
    fn build_cr2(ifd0_entries: &[PendingEntry], raw_entries: &[PendingEntry], strip: &[u8]) -> Vec<u8> {
        let ifd0_offset = 16_usize;
        let raw_ifd_offset = ifd0_offset + 2 + ifd0_entries.len() * 12 + 4;
        let raw_ifd_end = raw_ifd_offset + 2 + raw_entries.len() * 12 + 4;

        let mut bytes = vec![0_u8; raw_ifd_end];
        bytes[0..4].copy_from_slice(&[b'I', b'I', 42, 0]);
        bytes[4..8].copy_from_slice(&u32::try_from(ifd0_offset).unwrap().to_le_bytes());
        bytes[8..12].copy_from_slice(SIGNATURE);
        bytes[12..16].copy_from_slice(&u32::try_from(raw_ifd_offset).unwrap().to_le_bytes());

        let mut blob = Vec::new();
        write_ifd(
            &mut bytes,
            ifd0_offset,
            ifd0_entries,
            u32::try_from(raw_ifd_offset).unwrap(),
            raw_ifd_end,
            &mut blob,
        );
        write_ifd(&mut bytes, raw_ifd_offset, raw_entries, 0, raw_ifd_end, &mut blob);
        bytes.extend_from_slice(&blob);

        // Patch strip offsets: every LONG placeholder value of u32::MAX in
        // tag 273 and u32::MAX-1 in tag 279 is replaced by the real strip
        // position/size.
        let strip_offset = bytes.len();
        bytes.extend_from_slice(strip);
        for entry in raw_entries {
            if entry.0 == 273 && entry.3 == u32::MAX.to_le_bytes() {
                patch_inline_long(
                    &mut bytes,
                    raw_ifd_offset,
                    raw_entries,
                    273,
                    u32::try_from(strip_offset).unwrap(),
                );
            }
            if entry.0 == 279 && entry.3 == (u32::MAX - 1).to_le_bytes() {
                patch_inline_long(
                    &mut bytes,
                    raw_ifd_offset,
                    raw_entries,
                    279,
                    u32::try_from(strip.len()).unwrap(),
                );
            }
        }
        bytes
    }

    fn patch_inline_long(
        bytes: &mut [u8],
        ifd_offset: usize,
        entries: &[PendingEntry],
        tag: u16,
        value: u32,
    ) {
        let mut sorted = entries.to_vec();
        sorted.sort_by_key(|entry| entry.0);
        let sorted_position = sorted.iter().position(|entry| entry.0 == tag).unwrap();
        let at = ifd_offset + 2 + sorted_position * 12 + 8;
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    // ----- synthetic lossless-JPEG encoder (test-only mirror of the decoder's
    // accepted subset: one DHT, SOF3, one interleaved SOS, predictor 1) -----

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
            while self.used != 0 {
                self.current = (self.current << 1) | 1;
                self.used += 1;
                if self.used == 8 {
                    self.flush_byte();
                }
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

    fn category_and_bits(difference: i32) -> (u8, u32) {
        if difference == 0 {
            return (0, 0);
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

    /// Encodes single-component lossless JPEG with predictor 1, matching the
    /// decoder's scan: first sample uses the initial predictor, the rest of
    /// row 0 uses the left sample, later rows start from the sample above.
    fn ljpeg_encode(width: u16, height: u16, precision: u8, samples: &[u16]) -> Vec<u8> {
        assert_eq!(samples.len(), usize::from(width) * usize::from(height));
        let mut output = vec![0xff, 0xd8]; // SOI
        // DHT table 0: 17 codes of length 5 for categories 0..=16 (complete
        // tree shape that shipped Canon DNGs also use).
        let mut dht = vec![0_u8];
        dht.extend_from_slice(&[0, 0, 0, 0, 17, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        dht.extend(0_u8..=16);
        output.extend_from_slice(&[0xff, 0xc4]);
        output.extend_from_slice(&u16::try_from(dht.len() + 2).unwrap().to_be_bytes());
        output.extend_from_slice(&dht);
        // SOF3.
        let mut frame = vec![precision];
        frame.extend_from_slice(&height.to_be_bytes());
        frame.extend_from_slice(&width.to_be_bytes());
        frame.extend_from_slice(&[1, 1, 0x11, 0]);
        output.extend_from_slice(&[0xff, 0xc3]);
        output.extend_from_slice(&u16::try_from(frame.len() + 2).unwrap().to_be_bytes());
        output.extend_from_slice(&frame);
        // SOS: one component, predictor 1, no point transform.
        let scan = [1, 1, 0, 1, 0, 0];
        output.extend_from_slice(&[0xff, 0xda]);
        output.extend_from_slice(&u16::try_from(scan.len() + 2).unwrap().to_be_bytes());
        output.extend_from_slice(&scan);
        // Entropy data.
        let width = usize::from(width);
        let initial = 1_i32 << (precision - 1);
        let mut bits = BitWriter::new();
        for index in 0..samples.len() {
            let x = index % width;
            let y = index / width;
            let predicted = if index == 0 {
                initial
            } else if y == 0 {
                i32::from(samples[index - 1])
            } else if x == 0 {
                i32::from(samples[index - width])
            } else {
                i32::from(samples[index - 1])
            };
            let modulo = (i32::from(samples[index]) - predicted) & 0xffff;
            let difference = if modulo > 32_767 { modulo - 65_536 } else { modulo };
            let (category, encoded) = category_and_bits(difference);
            bits.write(u32::from(category), 5);
            if category < 16 {
                bits.write(encoded, category);
            }
        }
        bits.pad_ones();
        output.extend_from_slice(&bits.bytes);
        output.extend_from_slice(&[0xff, 0xd9]); // EOI
        output
    }

    /// Standard raw-IFD entries for a sliced 10x6 12-bit image: slices
    /// `[2, 4, 2]` (two 4-wide slices plus a 2-wide last slice).
    fn raw_entries(strip: &[u8]) -> Vec<PendingEntry> {
        let _ = strip;
        vec![
            long_entry(256, &[10]),                 // ImageWidth
            long_entry(257, &[6]),                  // ImageLength
            short_entry(258, &[12]),                // BitsPerSample
            short_entry(259, &[7]),                 // Compression = lossless JPEG
            short_entry(262, &[32_803]),            // Photometric = CFA
            long_entry(273, &[u32::MAX]),           // StripOffsets (patched)
            short_entry(277, &[1]),                 // SamplesPerPixel
            long_entry(279, &[u32::MAX - 1]),       // StripByteCounts (patched)
            short_entry(33421, &[2, 2]),            // CFARepeatPatternDim
            byte_entry(33422, &[0, 1, 1, 2]),       // CFAPattern RGGB
            short_entry(TAG_CR2_SLICE, &[2, 4, 2]), // CR2 slices
            short_entry(TAG_BLACK_LEVEL_REPEAT_DIM, &[2, 2]),
            short_entry(TAG_BLACK_LEVEL, &[64, 64, 66, 66]),
            long_entry(TAG_WHITE_LEVEL, &[4_095]),
            rational_entry(TAG_AS_SHOT_NEUTRAL, &[(1, 2), (1, 1), (1, 4)]),
            srational_entry(
                TAG_COLOR_MATRIX_2,
                &[
                    (1, 1),
                    (0, 1),
                    (0, 1),
                    (0, 1),
                    (1, 1),
                    (0, 1),
                    (0, 1),
                    (0, 1),
                    (1, 1),
                ],
            ),
            short_entry(TAG_CALIBRATION_ILLUMINANT_2, &[21]), // D65
            long_entry(TAG_ACTIVE_AREA, &[0, 0, 6, 10]),
        ]
    }

    fn ifd0_entries() -> Vec<PendingEntry> {
        vec![
            ascii_entry(271, "Canon"),
            ascii_entry(272, "Canon EOS 80D"),
            short_entry(274, &[1]),
        ]
    }

    /// Samples in row-major sensor order for the 10x6 test image.
    fn sensor_samples() -> Vec<u16> {
        (0..60_u16).map(|index| 1_000 + index * 17 % 900).collect()
    }

    /// Converts row-major sensor samples into slice-major stream order for
    /// the `[2, 4, 2]` layout of the test image.
    fn stream_order(samples: &[u16]) -> Vec<u16> {
        let (width, height) = (10_usize, 6_usize);
        let mut stream = Vec::with_capacity(samples.len());
        for (slice_start, slice_width) in [(0_usize, 4_usize), (4, 4), (8, 2)] {
            for row in 0..height {
                for column in slice_start..slice_start + slice_width {
                    stream.push(samples[row * width + column]);
                }
            }
        }
        stream
    }

    fn synthetic_cr2() -> Vec<u8> {
        let stream = stream_order(&sensor_samples());
        let strip = ljpeg_encode(10, 6, 12, &stream);
        build_cr2(&ifd0_entries(), &raw_entries(&strip), &strip)
    }

    fn parse_and_select(bytes: &[u8]) -> (usize, CameraFile<'_>) {
        let quirks = Cr2Quirks;
        let container = quirks.parse_container(bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        (usize::try_from(raw.offset()).unwrap(), container)
    }

    // ----- tests -----------------------------------------------------------

    #[test]
    fn parses_header_signature_and_selects_raw_ifd_via_pointer() {
        let bytes = synthetic_cr2();
        let expected_raw_offset = 16 + 2 + ifd0_entries().len() * 12 + 4;
        let (raw_offset, container) = parse_and_select(&bytes);
        assert_eq!(raw_offset, expected_raw_offset);
        // The raw IFD terminates the top-level chain.
        assert_eq!(container.directories().len(), 2);
    }

    #[test]
    fn rejects_wrong_signature_and_non_little_endian() {
        let quirks = Cr2Quirks;
        let mut bytes = synthetic_cr2();
        bytes[8..12].copy_from_slice(b"CR\x03\0");
        let error = quirks.parse_container(&bytes).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "CR2", ref message } if message.contains("signature")),
            "unexpected error: {error:?}"
        );

        let mut big_endian = synthetic_cr2();
        big_endian[0..2].copy_from_slice(b"MM");
        let error = quirks.parse_container(&big_endian).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "CR2", ref message } if message.contains("little-endian")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn rejects_dangling_raw_ifd_pointer() {
        let quirks = Cr2Quirks;
        let mut bytes = synthetic_cr2();
        bytes[12..16].copy_from_slice(&(u32::MAX - 8).to_le_bytes());
        let container = quirks.parse_container(&bytes).unwrap();
        let error = quirks.select_raw_ifd(&container).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "CR2", .. }),
            "unexpected error: {error:?}"
        );

        bytes[12..16].copy_from_slice(&0_u32.to_le_bytes());
        let container = quirks.parse_container(&bytes).unwrap();
        let error = quirks.select_raw_ifd(&container).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "CR2", ref message } if message.contains("zero")),
            "unexpected error: {error:?}"
        );
    }

    // White-balance gains are exact reciprocals of powers of two, so strict
    // float comparison is intentional here.
    #[allow(clippy::float_cmp)]
    #[test]
    fn reads_metadata_from_dng_like_cr2_tags() {
        let bytes = synthetic_cr2();
        let quirks = Cr2Quirks;
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let metadata = quirks.read_metadata(&container, raw).unwrap();

        assert_eq!(metadata.make, "Canon");
        assert_eq!(metadata.model, "Canon EOS 80D");
        assert_eq!((metadata.width, metadata.height), (10, 6));
        assert_eq!(metadata.bits_per_sample, 12);
        assert_eq!(metadata.orientation, Orientation::Normal);
        assert_eq!(metadata.cfa.width, 2);
        assert_eq!(metadata.cfa.height, 2);
        assert_eq!(
            metadata.cfa.cells,
            [CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue]
        );
        assert_eq!(metadata.black_level.values, [64.0, 64.0, 66.0, 66.0]);
        assert_eq!((metadata.black_level.width, metadata.black_level.height), (2, 2));
        assert_eq!(metadata.white_level.0, [4_095.0]);
        // Gains: 1/(1/2)=2, 1/1=1, 1/(1/4)=4; green-normalized.
        assert_eq!(metadata.white_balance, [2.0, 1.0, 4.0, 1.0]);
        assert_eq!(metadata.active_area, Some(Rect::new(0, 0, 10, 6)));
    }

    #[test]
    fn decodes_ljpeg_slices_into_row_major_sensor_order() {
        let bytes = synthetic_cr2();
        let quirks = Cr2Quirks;
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let pixels = quirks.decode_pixels(&container, raw, &|| false).unwrap();
        assert_eq!(pixels, sensor_samples());
    }

    #[test]
    fn decodes_unsliced_stream_as_identity() {
        let mut entries = raw_entries(&[]);
        entries.retain(|entry| entry.0 != TAG_CR2_SLICE);
        let samples = sensor_samples();
        let strip = ljpeg_encode(10, 6, 12, &samples);
        let bytes = build_cr2(&ifd0_entries(), &entries, &strip);
        let quirks = Cr2Quirks;
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let pixels = quirks.decode_pixels(&container, raw, &|| false).unwrap();
        assert_eq!(pixels, samples);
    }

    #[test]
    fn rejects_unsupported_compression() {
        let mut entries = raw_entries(&[]);
        for entry in &mut entries {
            if entry.0 == 259 {
                *entry = short_entry(259, &[1]);
            }
        }
        let samples = sensor_samples();
        let strip = ljpeg_encode(10, 6, 12, &samples);
        let bytes = build_cr2(&ifd0_entries(), &entries, &strip);
        let quirks = Cr2Quirks;
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.decode_pixels(&container, raw, &|| false).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "CR2", ref message } if message.contains("compression 1")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn rejects_slice_geometry_mismatch() {
        let mut entries = raw_entries(&[]);
        for entry in &mut entries {
            if entry.0 == TAG_CR2_SLICE {
                *entry = short_entry(TAG_CR2_SLICE, &[2, 3, 2]); // 3*2+2 = 8 != 10
            }
        }
        let stream = stream_order(&sensor_samples());
        let strip = ljpeg_encode(10, 6, 12, &stream);
        let bytes = build_cr2(&ifd0_entries(), &entries, &strip);
        let quirks = Cr2Quirks;
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.decode_pixels(&container, raw, &|| false).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "CR2", ref message } if message.contains("slice layout")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn rejects_non_cfa_photometric() {
        let mut entries = raw_entries(&[]);
        for entry in &mut entries {
            if entry.0 == 262 {
                *entry = short_entry(262, &[6]); // YCbCr (sRAW-style)
            }
        }
        let stream = stream_order(&sensor_samples());
        let strip = ljpeg_encode(10, 6, 12, &stream);
        let bytes = build_cr2(&ifd0_entries(), &entries, &strip);
        let quirks = Cr2Quirks;
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.decode_pixels(&container, raw, &|| false).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "CR2", ref message } if message.contains("PhotometricInterpretation 6")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn cancellation_during_decode_is_typed() {
        let bytes = synthetic_cr2();
        let quirks = Cr2Quirks;
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.decode_pixels(&container, raw, &|| true).unwrap_err();
        assert!(
            matches!(error, DecodeError::Cancelled),
            "unexpected error: {error:?}"
        );
    }
}
