//! Panasonic RW2 quirks.
//!
//! # Container
//!
//! RW2 is a little-endian classic TIFF whose header magic is `0x55` (`IIU\0`)
//! instead of the standard 42. The router (`crate::sniff`) already recognizes
//! `IIU\0` and routes here, and the shared reader
//! `crate::dng::tiff::Tiff::parse` accepts the `0x55` magic as the
//! classic-TIFF variant, so [`Rw2Quirks::parse_container`] validates the
//! signature and delegates to [`CameraFile::parse_tiff`] directly. (Some unit
//! tests still normalize the magic byte to 42 before calling
//! [`CameraFile::parse_tiff`]; the IFD bodies are classic TIFF either way.)
//!
//! # Raw storage variants (dispatch in [`Rw2Quirks::decode_pixels`])
//!
//! - **(a) Uncompressed 16-bit** (`Compression = 1`): one `u16` per sample in
//!   container byte order. This covers both true 16-bit storage and the
//!   "14-bit in 16-bit" unpacked variant of higher-end models; the two are
//!   told apart from the packed variant by comparing the declared strip byte
//!   count against `width * height * 2` (and by `BitsPerSample == 16`).
//! - **(b) Panasonic packed** (`Compression = 1`, `BitsPerSample` 12 or 14,
//!   strip byte count != `w*h*2`): the GH-era entropy-coded bitstream decoded
//!   by dcraw's `panasonic_load_raw` + `pana_bits`. The stream is consumed in
//!   `0x4000`-byte blocks; each block is rotated by `0x2008` bytes when the
//!   raw IFD carries tag `0x0118` (`RawDataOffset`, the RW2 case in dcraw),
//!   byte order inside the block is `(vbits >> 3) ^ 0x3ff0` (16-byte groups,
//!   reversed bytes, LSB-first), and 14 columns share two predictor/base
//!   channels with variable-length deltas. Reimplemented here bit-exactly
//!   from dcraw 9.28 (`pana_bits`, `panasonic_load_raw`). A short final block
//!   is zero-filled (dcraw reads past EOF with stale buffer contents; zeros
//!   keep this bounded and deterministic). Note that `ExifTool` reports the
//!   stream ratio as 8 pixels per ~9.14 bytes, NOT a plain "8 pixels in 14
//!   bytes" bit packing — the plain-packing description does not match dcraw
//!   and is therefore not implemented.
//! - **(c) Lossless JPEG** (`Compression = 7`, newer models such as S1/GH6):
//!   a single strip holding one SOF3 lossless-JPEG stream, decoded with the
//!   shared `crate::dng::lossless_jpeg` decoder. Multi-strip LJPEG and
//!   multi-component frames are rejected with typed errors.
//!
//! Everything else (other `Compression` values including the legacy Panasonic
//! RAW/RWL codes 34316/34826/34828/34830, other bit depths, missing CFA) is a
//! typed `DecodeError::NativeCamera { format: "RW2", .. }` rejection. The
//! embedded JPEG (tag `0x002E` `JpgFromRaw`) is never substituted.
//!
//! # Metadata (sources: dcraw `parse_tiff_ifd`, `ExifTool` `PanasonicRaw.pm`)
//!
//! Read from IFD0 (the first top-level directory), which is where Panasonic
//! writes the `PanRaw` tags; the raw pixel IFD is usually IFD0 itself:
//! - Make/Model: standard tags 271/272; Orientation: tag 274.
//! - Dimensions: standard 256/257, falling back to Panasonic `0x0002`/`0x0003`
//!   (`SensorWidth`/`SensorHeight`, which dcraw also treats as image size).
//! - Bits per sample: standard 258, falling back to Panasonic `0x000A`.
//! - CFA: standard `CFARepeatPatternDim`/`CFAPattern` (33421/33422) when
//!   present, else Panasonic tag `0x0009` (1=RGGB, 2=GRBG, 3=GBRG, 4=BGGR).
//! - White balance: tags `0x0024`/`0x0025`/`0x0026` (`WBRedLevel`/
//!   `WBGreenLevel`/`WBBlueLevel`) used as dcraw `cam_mul[]` camera
//!   multipliers, normalized so green is 1.0. Missing/incomplete -> [1; 4].
//! - Black level: tags `0x001C`/`0x001D`/`0x001E` (R/G/B, green duplicated)
//!   expanded to a 2x2 grid in CFA-cell order. All three or none; a partial
//!   set is a typed error. NOTE: `ExifTool` documents that for `RawFormat == 4`
//!   (tag `0x002D`, most RW2 models) 15 must be added to these black levels;
//!   dcraw uses them as-is, and this implementation follows dcraw. Revisit
//!   with real camera files.
//! - White level: max of `0x000E`-`0x0010` (`LinearityLimit` R/G/B) when
//!   present, else `(1 << bits_per_sample) - 1`.
//! - Geometry: active area from `0x0004`-`0x0007` (SensorTop/Left/Bottom/
//!   `RightBorder`, treated as border widths) and crop from `0x002F`-`0x0032`
//!   (`CropTop`/`CropLeft`/`CropBottom`/`CropRight`, treated as absolute
//!   coordinates). Both are emitted only when present and self-consistent.
//! - Color matrix: RW2 carries none; identity `xyz_to_camera` is emitted.
//!
//! # Test status
//!
//! All tests use synthetic in-memory files built byte-by-byte (no licensed
//! camera raws). They validate the parser, the bit-exact dcraw packed
//! decoder against hand-computed vectors, the LJPEG path against a minimal
//! SOF3 stream, and the typed rejections. They are NOT proof of camera
//! compatibility; real-device validation is still pending.

use rrrah_core::{CfaColor, CfaPattern, LevelGrid, Rect, WhiteLevel};

use super::{
    CameraDirectory, CameraFile, CameraMetadata, CameraQuirks, camera_error, optional_ascii, optional_scalar,
    orientation_from_tag, tags,
};
use crate::DecodeError;
use crate::dng::lossless_jpeg::{self, LosslessJpegError};

const FORMAT: &str = "RW2";
/// RW2 header signature: little-endian marker plus Panasonic's 0x55 magic.
const RW2_SIGNATURE: &[u8; 4] = b"IIU\0";
/// Sentinel written instead of a real `StripOffsets` value by some models
/// (`ExifTool`: "this value is 0xffffffff for some models, and `RawDataOffset`
/// must be used").
const STRIP_OFFSET_SENTINEL: u64 = 0xffff_ffff;
/// Rotation applied to each packed 0x4000-byte block when the raw IFD carries
/// `RawDataOffset` (dcraw sets `load_flags = 0x2008` in exactly that case).
const PACKED_BLOCK_LEN: usize = 0x4000;
const PACKED_BLOCK_ROTATE: usize = 0x2008;
const MAX_IMAGE_AREA: u64 = 512 * 1024 * 1024;

/// Panasonic-specific tag numbers (valid in RW2 IFD0 per `ExifTool`
/// `PanasonicRaw.pm`; several double as dcraw's image-geometry tags).
mod pana {
    pub(crate) const SENSOR_WIDTH: u16 = 0x0002;
    pub(crate) const SENSOR_HEIGHT: u16 = 0x0003;
    pub(crate) const SENSOR_TOP_BORDER: u16 = 0x0004;
    pub(crate) const SENSOR_LEFT_BORDER: u16 = 0x0005;
    pub(crate) const SENSOR_BOTTOM_BORDER: u16 = 0x0006;
    pub(crate) const SENSOR_RIGHT_BORDER: u16 = 0x0007;
    pub(crate) const CFA_PATTERN: u16 = 0x0009;
    pub(crate) const BITS_PER_SAMPLE: u16 = 0x000a;
    pub(crate) const LINEARITY_LIMIT_RED: u16 = 0x000e;
    pub(crate) const LINEARITY_LIMIT_GREEN: u16 = 0x000f;
    pub(crate) const LINEARITY_LIMIT_BLUE: u16 = 0x0010;
    pub(crate) const BLACK_LEVEL_RED: u16 = 0x001c;
    pub(crate) const BLACK_LEVEL_GREEN: u16 = 0x001d;
    pub(crate) const BLACK_LEVEL_BLUE: u16 = 0x001e;
    pub(crate) const WB_RED_LEVEL: u16 = 0x0024;
    pub(crate) const WB_GREEN_LEVEL: u16 = 0x0025;
    pub(crate) const WB_BLUE_LEVEL: u16 = 0x0026;
    pub(crate) const CROP_TOP: u16 = 0x002f;
    pub(crate) const CROP_LEFT: u16 = 0x0030;
    pub(crate) const CROP_BOTTOM: u16 = 0x0031;
    pub(crate) const CROP_RIGHT: u16 = 0x0032;
    pub(crate) const RAW_DATA_OFFSET: u16 = 0x0118;
}

/// Registered RW2 quirks.
#[derive(Debug)]
pub(crate) struct Rw2Quirks;

impl CameraQuirks for Rw2Quirks {
    fn format_name(&self) -> &'static str {
        FORMAT
    }

    fn parse_container<'a>(&self, data: &'a [u8]) -> Result<CameraFile<'a>, DecodeError> {
        if !data.starts_with(RW2_SIGNATURE) {
            return Err(camera_error(FORMAT, "not an RW2 file: missing IIU\\0 signature"));
        }
        // The shared `Tiff::parse` accepts the 0x55 magic as classic TIFF
        // (see the module docs).
        CameraFile::parse_tiff(FORMAT, data)
    }

    /// RW2 raw IFDs are not guaranteed to carry standard `ImageWidth`/
    /// `ImageLength` (Panasonic uses `0x0002`/`0x0003`) or a valid
    /// `StripOffsets` (0xffffffff sentinel + `RawDataOffset`), so the generic
    /// selection would miss them. Scores every directory that has dimensions
    /// and raw storage by (looks-like-CFA, pixel area).
    fn select_raw_ifd<'a>(
        &self,
        container: &'a CameraFile<'a>,
    ) -> Result<&'a CameraDirectory<'a>, DecodeError> {
        let mut best: Option<(&CameraDirectory<'a>, (u8, u64))> = None;
        for directory in container.directories() {
            if !has_raw_storage(directory)? {
                continue;
            }
            let Some((width, height)) = dimensions(directory)? else {
                continue;
            };
            let area = u64::from(width)
                .checked_mul(u64::from(height))
                .ok_or_else(|| camera_error(FORMAT, "arithmetic overflow computing raw IFD area"))?;
            if area == 0 || area > MAX_IMAGE_AREA {
                continue;
            }
            let cfa = u8::from(
                optional_scalar(FORMAT, directory, tags::PHOTOMETRIC_INTERPRETATION)?
                    == Some(tags::PHOTOMETRIC_CFA)
                    || directory.entry(FORMAT, pana::CFA_PATTERN)?.is_some()
                    || directory.entry(FORMAT, tags::CFA_PATTERN)?.is_some(),
            );
            let score = (cfa, area);
            if best.is_none_or(|(_, best_score)| score > best_score) {
                best = Some((directory, score));
            }
        }
        best.map(|(directory, _)| directory)
            .ok_or_else(|| camera_error(FORMAT, "no raw image directory found"))
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
            .unwrap_or(raw);

        let make = optional_ascii(FORMAT, ifd0, tags::MAKE)?.unwrap_or_default();
        let model = optional_ascii(FORMAT, ifd0, tags::MODEL)?.unwrap_or_default();
        let orientation = match optional_scalar(FORMAT, ifd0, tags::ORIENTATION)? {
            Some(value) => orientation_from_tag(FORMAT, value)?,
            None => rrrah_core::Orientation::Normal,
        };

        let (width, height) =
            dimensions(raw)?.ok_or_else(|| camera_error(FORMAT, "raw IFD has no image dimensions"))?;
        let bits = bits_per_sample(raw)?;
        let cfa = cfa_pattern(raw)?;

        let black_level = black_level(ifd0, &cfa)?;
        let white_level = white_level(ifd0, bits)?;
        let white_balance = white_balance(ifd0)?;

        let active_area = bordered_rect(
            ifd0,
            pana::SENSOR_LEFT_BORDER,
            pana::SENSOR_TOP_BORDER,
            pana::SENSOR_RIGHT_BORDER,
            pana::SENSOR_BOTTOM_BORDER,
            width,
            height,
        )?;
        let crop_area = crop_rect(ifd0, width, height)?;

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
            // RW2 carries no color matrix; identity keeps the pipeline
            // explicit rather than silently inventing one.
            xyz_to_camera: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]],
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
        let (width, height) =
            dimensions(raw)?.ok_or_else(|| camera_error(FORMAT, "raw IFD has no image dimensions"))?;
        let bits = bits_per_sample(raw)?;
        let compression =
            optional_scalar(FORMAT, raw, tags::COMPRESSION)?.unwrap_or(tags::COMPRESSION_UNCOMPRESSED);
        let storage = storage_segments(container, raw)?;

        let width_usize =
            usize::try_from(width).map_err(|_| camera_error(FORMAT, "raw width overflows usize"))?;
        let height_usize =
            usize::try_from(height).map_err(|_| camera_error(FORMAT, "raw height overflows usize"))?;
        let sample_count = width_usize
            .checked_mul(height_usize)
            .ok_or_else(|| camera_error(FORMAT, "arithmetic overflow computing sample count"))?;
        let unpacked_bytes = u64::try_from(sample_count)
            .ok()
            .and_then(|count| count.checked_mul(2))
            .ok_or_else(|| camera_error(FORMAT, "arithmetic overflow computing frame byte length"))?;

        match compression {
            tags::COMPRESSION_LOSSLESS_JPEG => {
                decode_lossless_jpeg(container, &storage, width, height, sample_count, cancelled)
            }
            tags::COMPRESSION_UNCOMPRESSED => {
                let declared = storage.total_declared_bytes()?;
                if declared == Some(unpacked_bytes) || bits == 16 {
                    decode_unpacked16(container, raw, &storage, width, height, sample_count, cancelled)
                } else if matches!(bits, 12 | 14) {
                    let rotate = if raw.entry(FORMAT, pana::RAW_DATA_OFFSET)?.is_some() {
                        PACKED_BLOCK_ROTATE
                    } else {
                        0
                    };
                    decode_panasonic_packed(
                        container,
                        &storage,
                        rotate,
                        width_usize,
                        height_usize,
                        sample_count,
                        cancelled,
                    )
                } else {
                    Err(camera_error(
                        FORMAT,
                        format!(
                            "unsupported uncompressed storage: BitsPerSample {bits} with declared \
                             strip bytes {declared:?} (expected {unpacked_bytes} for 16-bit unpacked)"
                        ),
                    ))
                }
            }
            actual => Err(camera_error(
                FORMAT,
                format!(
                    "unsupported Compression {actual}; RW2 supports 1 (unpacked/Panasonic packed) \
                     and 7 (lossless JPEG); legacy Panasonic RAW/RWL codes 34316/34826/34828/34830 \
                     are a different container and are not handled"
                ),
            )),
        }
    }
}

/// Image dimensions from standard tags, falling back to the Panasonic sensor
/// tags (dcraw treats tag 2/3 as width/height too).
fn dimensions(directory: &CameraDirectory<'_>) -> Result<Option<(u32, u32)>, DecodeError> {
    let pick = |standard: u16, panasonic: u16| -> Result<Option<u32>, DecodeError> {
        let value = match optional_scalar(FORMAT, directory, standard)? {
            Some(value) => Some(value),
            None => optional_scalar(FORMAT, directory, panasonic)?,
        };
        value
            .map(|value| {
                u32::try_from(value)
                    .map_err(|_| camera_error(FORMAT, format!("dimension tag {standard} exceeds u32")))
            })
            .transpose()
    };
    match (
        pick(tags::IMAGE_WIDTH, pana::SENSOR_WIDTH)?,
        pick(tags::IMAGE_LENGTH, pana::SENSOR_HEIGHT)?,
    ) {
        (Some(width), Some(height)) => Ok(Some((width, height))),
        _ => Ok(None),
    }
}

fn bits_per_sample(directory: &CameraDirectory<'_>) -> Result<u8, DecodeError> {
    let value = match optional_scalar(FORMAT, directory, tags::BITS_PER_SAMPLE)? {
        Some(value) => Some(value),
        None => optional_scalar(FORMAT, directory, pana::BITS_PER_SAMPLE)?,
    }
    .ok_or_else(|| camera_error(FORMAT, "raw IFD has no BitsPerSample"))?;
    let bits = u8::try_from(value)
        .map_err(|_| camera_error(FORMAT, format!("unsupported BitsPerSample {value}")))?;
    if !(1..=16).contains(&bits) {
        return Err(camera_error(FORMAT, format!("unsupported BitsPerSample {bits}")));
    }
    Ok(bits)
}

/// Whether a directory points at raw pixel storage: a real `StripOffsets`
/// value (not the sentinel) or a Panasonic `RawDataOffset`.
fn has_raw_storage(directory: &CameraDirectory<'_>) -> Result<bool, DecodeError> {
    if directory.entry(FORMAT, pana::RAW_DATA_OFFSET)?.is_some() {
        return Ok(true);
    }
    if let Some(entry) = directory.entry(FORMAT, tags::STRIP_OFFSETS)? {
        let values = entry
            .unsigned_values()
            .map_err(|error| camera_error(FORMAT, format!("StripOffsets: {error}")))?;
        return Ok(values
            .iter()
            .any(|&offset| offset != 0 && offset != STRIP_OFFSET_SENTINEL));
    }
    Ok(false)
}

/// Raw pixel segments: strip offsets/counts, or the single `RawDataOffset`
/// fallback used by models that write the `StripOffsets` sentinel (or no strip
/// tags at all, e.g. DC-GH6/DC-GH5M2 per `ExifTool`).
struct Storage {
    offsets: Vec<u64>,
    counts: Vec<u64>,
}

impl Storage {
    /// Sum of declared strip byte counts, if any are declared; `None` when
    /// the file carries no usable `StripByteCounts` (some models write 0).
    fn total_declared_bytes(&self) -> Result<Option<u64>, DecodeError> {
        if self.counts.is_empty() || self.counts.iter().all(|&count| count == 0) {
            return Ok(None);
        }
        self.counts
            .iter()
            .try_fold(0_u64, |total, &count| total.checked_add(count))
            .map(Some)
            .ok_or_else(|| camera_error(FORMAT, "arithmetic overflow summing strip byte counts"))
    }
}

fn storage_segments(container: &CameraFile<'_>, raw: &CameraDirectory<'_>) -> Result<Storage, DecodeError> {
    let file_len = u64::try_from(container.data().len())
        .map_err(|_| camera_error(FORMAT, "file length overflows u64"))?;
    let read_values = |tag: u16| -> Result<Vec<u64>, DecodeError> {
        raw.entry(FORMAT, tag)?
            .map(|entry| {
                entry
                    .unsigned_values()
                    .map_err(|error| camera_error(FORMAT, format!("tag {tag}: {error}")))
            })
            .transpose()
            .map(Option::unwrap_or_default)
    };
    let mut offsets = read_values(tags::STRIP_OFFSETS)?;
    let mut counts = read_values(tags::STRIP_BYTE_COUNTS)?;
    let usable = offsets
        .iter()
        .any(|&offset| offset != 0 && offset != STRIP_OFFSET_SENTINEL);
    if !usable {
        let fallback = optional_scalar(FORMAT, raw, pana::RAW_DATA_OFFSET)?
            .ok_or_else(|| camera_error(FORMAT, "raw IFD has neither StripOffsets nor RawDataOffset"))?;
        if fallback == 0 || fallback == STRIP_OFFSET_SENTINEL || fallback >= file_len {
            return Err(camera_error(FORMAT, format!("invalid RawDataOffset {fallback}")));
        }
        offsets = vec![fallback];
        counts = Vec::new();
    }
    for &offset in &offsets {
        if offset >= file_len {
            return Err(camera_error(
                FORMAT,
                format!("strip offset {offset} is past end of file"),
            ));
        }
    }
    if !counts.is_empty() && counts.len() != offsets.len() {
        return Err(camera_error(
            FORMAT,
            format!(
                "StripOffsets count {} does not match StripByteCounts count {}",
                offsets.len(),
                counts.len()
            ),
        ));
    }
    Ok(Storage { offsets, counts })
}

/// CFA pattern: standard TIFF/EP tags first, then the Panasonic `0x0009`
/// enumeration. Only 2x2 RGB Bayer patterns are supported.
fn cfa_pattern(raw: &CameraDirectory<'_>) -> Result<CfaPattern, DecodeError> {
    if let Some(entry) = raw.entry(FORMAT, tags::CFA_PATTERN)? {
        let cells = entry
            .unsigned_values()
            .map_err(|error| camera_error(FORMAT, format!("CFAPattern: {error}")))?;
        let dims = raw
            .entry(FORMAT, tags::CFA_REPEAT_PATTERN_DIM)?
            .map(crate::dng::tiff::Entry::unsigned_values)
            .transpose()
            .map_err(|error| camera_error(FORMAT, format!("CFARepeatPatternDim: {error}")))?;
        if dims.as_deref() != Some(&[2, 2]) || cells.len() != 4 {
            return Err(camera_error(
                FORMAT,
                format!(
                    "unsupported CFA layout: dims {dims:?}, {} cells (only 2x2 Bayer is supported)",
                    cells.len()
                ),
            ));
        }
        let map = |value: u64| -> Result<CfaColor, DecodeError> {
            match value {
                0 => Ok(CfaColor::Red),
                1 => Ok(CfaColor::Green),
                2 => Ok(CfaColor::Blue),
                actual => Err(camera_error(
                    FORMAT,
                    format!("unsupported CFA color code {actual} (only RGB Bayer is supported)"),
                )),
            }
        };
        let cells = cells
            .iter()
            .map(|&value| map(value))
            .collect::<Result<Vec<_>, _>>()?;
        let cfa = CfaPattern {
            width: 2,
            height: 2,
            cells,
        };
        cfa.bayer_quad()
            .map_err(|error| camera_error(FORMAT, format!("unsupported CFA pattern: {error}")))?;
        return Ok(cfa);
    }
    match optional_scalar(FORMAT, raw, pana::CFA_PATTERN)? {
        // ExifTool PanasonicRaw.pm PrintConv for tag 0x0009.
        Some(1) => Ok(bayer([
            CfaColor::Red,
            CfaColor::Green,
            CfaColor::Green,
            CfaColor::Blue,
        ])),
        Some(2) => Ok(bayer([
            CfaColor::Green,
            CfaColor::Red,
            CfaColor::Blue,
            CfaColor::Green,
        ])),
        Some(3) => Ok(bayer([
            CfaColor::Green,
            CfaColor::Blue,
            CfaColor::Red,
            CfaColor::Green,
        ])),
        Some(4) => Ok(bayer([
            CfaColor::Blue,
            CfaColor::Green,
            CfaColor::Green,
            CfaColor::Red,
        ])),
        Some(actual) => Err(camera_error(
            FORMAT,
            format!("unsupported Panasonic CFA pattern code {actual}"),
        )),
        None => Err(camera_error(
            FORMAT,
            "raw IFD carries neither CFAPattern (33422) nor the Panasonic CFA tag 0x0009",
        )),
    }
}

fn bayer(cells: [CfaColor; 4]) -> CfaPattern {
    CfaPattern {
        width: 2,
        height: 2,
        cells: cells.to_vec(),
    }
}

/// Black level from tags 0x001C/0x001D/0x001E (R/G/B, green duplicated)
/// expanded to a 2x2 grid in CFA-cell order; defaults to 0 when absent.
fn black_level(ifd0: &CameraDirectory<'_>, cfa: &CfaPattern) -> Result<LevelGrid, DecodeError> {
    let red = optional_scalar(FORMAT, ifd0, pana::BLACK_LEVEL_RED)?;
    let green = optional_scalar(FORMAT, ifd0, pana::BLACK_LEVEL_GREEN)?;
    let blue = optional_scalar(FORMAT, ifd0, pana::BLACK_LEVEL_BLUE)?;
    let (Some(red), Some(green), Some(blue)) = (red, green, blue) else {
        if red.is_none() && green.is_none() && blue.is_none() {
            return Ok(LevelGrid {
                width: 1,
                height: 1,
                components: 1,
                values: vec![0.0],
            });
        }
        return Err(camera_error(
            FORMAT,
            "partial Panasonic black level: tags 0x001C/0x001D/0x001E must appear together",
        ));
    };
    let level = |color: &CfaColor| -> Result<f32, DecodeError> {
        let value = match color {
            CfaColor::Red => red,
            CfaColor::Green => green,
            CfaColor::Blue => blue,
            other => {
                return Err(camera_error(
                    FORMAT,
                    format!("black level cannot be assigned to non-RGB CFA cell {other:?}"),
                ));
            }
        };
        let value = u16::try_from(value)
            .map_err(|_| camera_error(FORMAT, format!("black level {value} exceeds u16")))?;
        Ok(f32::from(value))
    };
    let values = cfa.cells.iter().map(level).collect::<Result<Vec<_>, _>>()?;
    Ok(LevelGrid {
        width: 2,
        height: 2,
        components: 1,
        values,
    })
}

/// White level: max of the `LinearityLimit` tags when present, else the
/// bit-depth maximum.
fn white_level(ifd0: &CameraDirectory<'_>, bits: u8) -> Result<WhiteLevel, DecodeError> {
    let mut limit: Option<u64> = None;
    for tag in [
        pana::LINEARITY_LIMIT_RED,
        pana::LINEARITY_LIMIT_GREEN,
        pana::LINEARITY_LIMIT_BLUE,
    ] {
        if let Some(value) = optional_scalar(FORMAT, ifd0, tag)? {
            limit = Some(limit.map_or(value, |current| current.max(value)));
        }
    }
    let level = match limit {
        Some(value) => u32::try_from(value)
            .map_err(|_| camera_error(FORMAT, format!("white level {value} exceeds u32")))?,
        None => (1_u32 << u32::from(bits)) - 1,
    };
    Ok(WhiteLevel(vec![level as f32]))
}

/// White balance gains from the WB level tags (dcraw `cam_mul[]`),
/// normalized so green is 1.0; defaults to unity when absent.
fn white_balance(ifd0: &CameraDirectory<'_>) -> Result<[f32; 4], DecodeError> {
    let red = optional_scalar(FORMAT, ifd0, pana::WB_RED_LEVEL)?;
    let green = optional_scalar(FORMAT, ifd0, pana::WB_GREEN_LEVEL)?;
    let blue = optional_scalar(FORMAT, ifd0, pana::WB_BLUE_LEVEL)?;
    let (Some(red), Some(green), Some(blue)) = (red, green, blue) else {
        if red.is_none() && green.is_none() && blue.is_none() {
            return Ok([1.0; 4]);
        }
        return Err(camera_error(
            FORMAT,
            "partial Panasonic white balance: tags 0x0024/0x0025/0x0026 must appear together",
        ));
    };
    let green = f32::from(
        u16::try_from(green)
            .map_err(|_| camera_error(FORMAT, format!("WB green level {green} exceeds u16")))?,
    );
    if green <= 0.0 {
        return Err(camera_error(FORMAT, "WB green level must be positive"));
    }
    let gain = |value: u64| -> Result<f32, DecodeError> {
        let value = u16::try_from(value)
            .map_err(|_| camera_error(FORMAT, format!("WB level {value} exceeds u16")))?;
        Ok(f32::from(value) / green)
    };
    Ok([gain(red)?, 1.0, gain(blue)?, 1.0])
}

/// Active area from the four Sensor*Border tags (treated as border widths);
/// `None` unless all four are present and leave a non-empty rectangle.
fn bordered_rect(
    ifd0: &CameraDirectory<'_>,
    left_tag: u16,
    top_tag: u16,
    right_tag: u16,
    bottom_tag: u16,
    width: u32,
    height: u32,
) -> Result<Option<Rect>, DecodeError> {
    let left = optional_scalar(FORMAT, ifd0, left_tag)?;
    let top = optional_scalar(FORMAT, ifd0, top_tag)?;
    let right = optional_scalar(FORMAT, ifd0, right_tag)?;
    let bottom = optional_scalar(FORMAT, ifd0, bottom_tag)?;
    let (Some(left), Some(top), Some(right), Some(bottom)) = (left, top, right, bottom) else {
        return Ok(None);
    };
    let to_u32 = |value: u64, tag: u16| {
        u32::try_from(value).map_err(|_| camera_error(FORMAT, format!("border tag {tag} exceeds u32")))
    };
    let (left, top) = (to_u32(left, left_tag)?, to_u32(top, top_tag)?);
    let (right, bottom) = (to_u32(right, right_tag)?, to_u32(bottom, bottom_tag)?);
    let Some(inner_width) = width.checked_sub(left).and_then(|value| value.checked_sub(right)) else {
        return Ok(None);
    };
    let Some(inner_height) = height
        .checked_sub(top)
        .and_then(|value| value.checked_sub(bottom))
    else {
        return Ok(None);
    };
    if inner_width == 0 || inner_height == 0 {
        return Ok(None);
    }
    Ok(Some(Rect::new(left, top, inner_width, inner_height)))
}

/// Crop rectangle from the Crop* tags (treated as absolute sensor
/// coordinates); `None` unless all four are present and consistent.
fn crop_rect(ifd0: &CameraDirectory<'_>, width: u32, height: u32) -> Result<Option<Rect>, DecodeError> {
    let top = optional_scalar(FORMAT, ifd0, pana::CROP_TOP)?;
    let left = optional_scalar(FORMAT, ifd0, pana::CROP_LEFT)?;
    let bottom = optional_scalar(FORMAT, ifd0, pana::CROP_BOTTOM)?;
    let right = optional_scalar(FORMAT, ifd0, pana::CROP_RIGHT)?;
    let (Some(top), Some(left), Some(bottom), Some(right)) = (top, left, bottom, right) else {
        return Ok(None);
    };
    let to_u32 = |value: u64, tag: u16| {
        u32::try_from(value).map_err(|_| camera_error(FORMAT, format!("crop tag {tag} exceeds u32")))
    };
    let (top, left) = (to_u32(top, pana::CROP_TOP)?, to_u32(left, pana::CROP_LEFT)?);
    let (bottom, right) = (
        to_u32(bottom, pana::CROP_BOTTOM)?,
        to_u32(right, pana::CROP_RIGHT)?,
    );
    if right <= left || bottom <= top || right > width || bottom > height {
        return Ok(None);
    }
    Ok(Some(Rect::new(left, top, right - left, bottom - top)))
}

/// Variant (a): uncompressed 16-bit rows (also covers 14-bit-in-16-bit
/// unpacked storage), strip by strip in container byte order.
fn decode_unpacked16(
    container: &CameraFile<'_>,
    raw: &CameraDirectory<'_>,
    storage: &Storage,
    width: u32,
    height: u32,
    sample_count: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<u16>, DecodeError> {
    let width = usize::try_from(width).map_err(|_| camera_error(FORMAT, "raw width overflows usize"))?;
    let height = usize::try_from(height).map_err(|_| camera_error(FORMAT, "raw height overflows usize"))?;
    let row_bytes = width
        .checked_mul(2)
        .ok_or_else(|| camera_error(FORMAT, "arithmetic overflow computing row byte length"))?;
    let rows_per_strip = match optional_scalar(FORMAT, raw, tags::ROWS_PER_STRIP)? {
        Some(value) => {
            usize::try_from(value).map_err(|_| camera_error(FORMAT, "RowsPerStrip overflows usize"))?
        }
        None => height,
    };
    if rows_per_strip == 0 {
        return Err(camera_error(FORMAT, "RowsPerStrip is zero"));
    }
    let expected_strips = height.div_ceil(rows_per_strip);
    if storage.offsets.len() != expected_strips {
        return Err(camera_error(
            FORMAT,
            format!(
                "expected {expected_strips} strips for {height} rows at {rows_per_strip} rows/strip, \
                 found {}",
                storage.offsets.len()
            ),
        ));
    }

    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(sample_count)
        .map_err(|_| camera_error(FORMAT, format!("could not allocate {sample_count} samples")))?;
    pixels.resize(sample_count, 0);

    let byte_order = container.byte_order();
    let data = container.data();
    let mut first_row = 0_usize;
    for (index, &offset) in storage.offsets.iter().enumerate() {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let rows = rows_per_strip.min(height - first_row);
        let strip_bytes = rows
            .checked_mul(row_bytes)
            .ok_or_else(|| camera_error(FORMAT, "arithmetic overflow computing strip byte length"))?;
        let start = usize::try_from(offset)
            .map_err(|_| camera_error(FORMAT, format!("strip offset {offset} overflows usize")))?;
        let end = start
            .checked_add(strip_bytes)
            .ok_or_else(|| camera_error(FORMAT, "arithmetic overflow computing strip end"))?;
        let bytes = data.get(start..end).ok_or_else(|| {
            camera_error(
                FORMAT,
                format!("strip {index} is truncated: need {strip_bytes} bytes at offset {offset}"),
            )
        })?;
        let target_start = first_row
            .checked_mul(width)
            .ok_or_else(|| camera_error(FORMAT, "arithmetic overflow computing strip target"))?;
        let target = &mut pixels[target_start..target_start + rows * width];
        for (sample, chunk) in target.iter_mut().zip(bytes.chunks_exact(2)) {
            *sample = byte_order.u16(chunk);
        }
        first_row += rows;
    }
    Ok(pixels)
}

/// Variant (b): dcraw's `panasonic_load_raw` bitstream, reimplemented
/// bit-exactly (`pana_bits` block reader + 14-column predictor/delta loop).
fn decode_panasonic_packed(
    container: &CameraFile<'_>,
    storage: &Storage,
    rotate: usize,
    width: usize,
    height: usize,
    sample_count: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<u16>, DecodeError> {
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(sample_count)
        .map_err(|_| camera_error(FORMAT, format!("could not allocate {sample_count} samples")))?;
    pixels.resize(sample_count, 0);

    let offset = usize::try_from(storage.offsets[0])
        .map_err(|_| camera_error(FORMAT, "packed data offset overflows usize"))?;
    let mut bits = PanaBits::new(container.data(), offset, rotate);
    bits.reset();

    // dcraw keeps `sh` across columns and rows; pred/nonz reset every
    // 14 columns (at i == 0).
    let mut sh: i32 = 0;
    let mut pred = [0_i32; 2];
    let mut nonz = [0_i32; 2];
    for row in 0..height {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        for col in 0..width {
            let i = col % 14;
            if i == 0 {
                pred = [0, 0];
                nonz = [0, 0];
            }
            if i % 3 == 2 {
                sh = 4 >> (3 - bits.read(2)?);
            }
            let channel = i & 1;
            if nonz[channel] != 0 {
                let j = bits.read(8)?;
                if j != 0 {
                    pred[channel] -= 0x80 << sh;
                    if pred[channel] < 0 || sh == 4 {
                        pred[channel] &= !(-1_i32 << sh);
                    }
                    pred[channel] += j << sh;
                }
            } else {
                let n = bits.read(8)?;
                nonz[channel] = n;
                if n != 0 || i > 11 {
                    pred[channel] = (n << 4) | bits.read(4)?;
                }
            }
            pixels[row * width + col] = u16::try_from(pred[col & 1] & 0xffff).expect("masked to 16 bits");
        }
    }
    Ok(pixels)
}

/// dcraw's `pana_bits`: a 0x4000-byte block buffer refilled on demand. Each
/// refill consumes `0x4000 - rotate` then `rotate` file bytes into the
/// corresponding buffer halves (the rotation places `RawDataOffset`-era
/// padding at the block tail). Bits are addressed by a decrementing counter:
/// `byte = (vbits >> 3) ^ 0x3ff0`, LSB-first within the little-endian pair.
struct PanaBits<'a> {
    data: &'a [u8],
    cursor: usize,
    rotate: usize,
    // One spare byte: dcraw evaluates `buf[byte] | buf[byte + 1] << 8` even
    // when `byte == 0x3fff` (the extra byte never influences the value
    // there, but indexing must stay in bounds). Heap-allocated: the 16 KiB
    // block exceeds the stack-array limit.
    buffer: Box<[u8]>,
    vbits: i32,
}

impl<'a> PanaBits<'a> {
    fn new(data: &'a [u8], offset: usize, rotate: usize) -> Self {
        debug_assert!(rotate < PACKED_BLOCK_LEN);
        Self {
            data,
            cursor: offset,
            rotate,
            buffer: vec![0; PACKED_BLOCK_LEN + 1].into_boxed_slice(),
            vbits: 0,
        }
    }

    /// `pana_bits(0)`: resets the bit counter (the next read refills).
    fn reset(&mut self) {
        self.vbits = 0;
    }

    fn refill(&mut self) {
        // dcraw relies on fread short-read semantics at EOF; zero-filling
        // keeps the tail deterministic. Decoding normally stops long before
        // the padding is reached.
        let first = PACKED_BLOCK_LEN - self.rotate;
        self.read_into(self.rotate, first);
        self.read_into(0, self.rotate);
    }

    fn read_into(&mut self, destination: usize, length: usize) {
        if length == 0 {
            return;
        }
        let available = self.data.len().saturating_sub(self.cursor);
        let take = available.min(length);
        self.buffer[destination..destination + take]
            .copy_from_slice(&self.data[self.cursor..self.cursor + take]);
        self.cursor += take;
        // Remaining bytes keep their previous (zero-initialized) contents.
    }

    /// Reads `nbits` (1..=16) bits, mirroring dcraw's `pana_bits`.
    fn read(&mut self, nbits: u8) -> Result<i32, DecodeError> {
        debug_assert!((1..=16).contains(&nbits));
        if self.vbits == 0 {
            self.refill();
        }
        self.vbits = (self.vbits - i32::from(nbits)) & 0x1_ffff;
        let byte = usize::try_from((self.vbits >> 3) ^ 0x3ff0)
            .map_err(|_| camera_error(FORMAT, "packed bitstream address overflow"))?;
        let word = u32::from(self.buffer[byte]) | (u32::from(self.buffer[byte + 1]) << 8);
        let shift = u32::try_from(self.vbits & 7)
            .map_err(|_| camera_error(FORMAT, "packed bitstream shift overflow"))?;
        let mask = (1_u32 << u32::from(nbits)) - 1;
        i32::try_from((word >> shift) & mask)
            .map_err(|_| camera_error(FORMAT, "packed bitstream value overflow"))
    }
}

/// Variant (c): one lossless-JPEG strip (TIFF compression 7) decoded with the
/// shared DNG decoder.
fn decode_lossless_jpeg(
    container: &CameraFile<'_>,
    storage: &Storage,
    width: u32,
    height: u32,
    sample_count: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<u16>, DecodeError> {
    if storage.offsets.len() != 1 {
        return Err(camera_error(
            FORMAT,
            format!(
                "lossless-JPEG RW2 must have exactly one strip, found {}",
                storage.offsets.len()
            ),
        ));
    }
    if cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let start = usize::try_from(storage.offsets[0])
        .map_err(|_| camera_error(FORMAT, "strip offset overflows usize"))?;
    let end = match storage.counts.first().copied().filter(|&count| count != 0) {
        Some(count) => {
            let count = usize::try_from(count)
                .map_err(|_| camera_error(FORMAT, "strip byte count overflows usize"))?;
            start
                .checked_add(count)
                .ok_or_else(|| camera_error(FORMAT, "arithmetic overflow computing strip end"))?
        }
        // No declared byte count: hand the decoder the rest of the file.
        None => container.data().len(),
    };
    let bytes = container.data().get(start..end).ok_or_else(|| {
        camera_error(
            FORMAT,
            format!("lossless-JPEG strip at offset {start} is truncated"),
        )
    })?;
    let image = lossless_jpeg::decode(bytes, cancelled).map_err(|error| {
        if matches!(error, LosslessJpegError::Cancelled { .. }) {
            DecodeError::Cancelled
        } else {
            camera_error(FORMAT, format!("lossless JPEG: {error}"))
        }
    })?;
    if image.component_ids.len() != 1 {
        return Err(camera_error(
            FORMAT,
            format!(
                "lossless-JPEG frame has {} components; only single-component CFA is supported",
                image.component_ids.len()
            ),
        ));
    }
    if u32::from(image.width) != width || u32::from(image.height) != height {
        return Err(camera_error(
            FORMAT,
            format!(
                "lossless-JPEG frame {}x{} does not match raw IFD dimensions {width}x{height}",
                image.width, image.height
            ),
        ));
    }
    if image.samples.len() != sample_count {
        return Err(camera_error(
            FORMAT,
            format!(
                "lossless JPEG decoded {} samples, expected {sample_count}",
                image.samples.len()
            ),
        ));
    }
    Ok(image.samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Synthetic RW2 file builder -------------------------------------

    #[derive(Clone)]
    enum Val {
        U16(u16),
        U32(u32),
        U32s(Vec<u32>),
        Text(&'static str),
    }

    #[derive(Clone)]
    struct Spec {
        tag: u16,
        val: Val,
    }

    fn short(tag: u16, value: u16) -> Spec {
        Spec {
            tag,
            val: Val::U16(value),
        }
    }

    fn long(tag: u16, value: u32) -> Spec {
        Spec {
            tag,
            val: Val::U32(value),
        }
    }

    fn encoded(val: &Val) -> (u16, u32, Vec<u8>) {
        match val {
            Val::U16(value) => (3, 1, value.to_le_bytes().to_vec()),
            Val::U32(value) => (4, 1, value.to_le_bytes().to_vec()),
            Val::U32s(values) => (
                4,
                u32::try_from(values.len()).unwrap(),
                values.iter().flat_map(|value| value.to_le_bytes()).collect(),
            ),
            Val::Text(text) => {
                let mut bytes = text.as_bytes().to_vec();
                bytes.push(0);
                (2, u32::try_from(bytes.len()).unwrap(), bytes)
            }
        }
    }

    /// Layout-only pass: offset at which the blob lands for these IFDs.
    fn blob_offset_for(ifds: &[Vec<Spec>]) -> usize {
        let mut cursor = 8_usize;
        for specs in ifds {
            cursor += 2 + 12 * specs.len() + 4;
            for spec in specs {
                let (_, _, bytes) = encoded(&spec.val);
                if bytes.len() > 4 {
                    cursor += bytes.len();
                }
            }
        }
        cursor
    }

    /// Builds a complete RW2 file with the real `IIU\0` signature. Returns
    /// the bytes and the offset where `blob` was placed.
    fn build_rw2(ifds: &[Vec<Spec>], blob: &[u8]) -> (Vec<u8>, usize) {
        let blob_offset = blob_offset_for(ifds);
        let mut bytes = vec![0_u8; blob_offset + blob.len()];
        bytes[0..4].copy_from_slice(RW2_SIGNATURE);
        bytes[4..8].copy_from_slice(&8_u32.to_le_bytes());

        let mut cursor = 8_usize;
        for (index, specs) in ifds.iter().enumerate() {
            let ifd_offset = cursor;
            cursor += 2 + 12 * specs.len() + 4;
            bytes[ifd_offset..ifd_offset + 2]
                .copy_from_slice(&u16::try_from(specs.len()).unwrap().to_le_bytes());
            for (entry_index, spec) in specs.iter().enumerate() {
                let at = ifd_offset + 2 + 12 * entry_index;
                let (field_type, count, encoded_bytes) = encoded(&spec.val);
                bytes[at..at + 2].copy_from_slice(&spec.tag.to_le_bytes());
                bytes[at + 2..at + 4].copy_from_slice(&field_type.to_le_bytes());
                bytes[at + 4..at + 8].copy_from_slice(&count.to_le_bytes());
                if encoded_bytes.len() <= 4 {
                    bytes[at + 8..at + 8 + encoded_bytes.len()].copy_from_slice(&encoded_bytes);
                } else {
                    bytes[at + 8..at + 12].copy_from_slice(&u32::try_from(cursor).unwrap().to_le_bytes());
                    bytes[cursor..cursor + encoded_bytes.len()].copy_from_slice(&encoded_bytes);
                    cursor += encoded_bytes.len();
                }
            }
            let next_at = ifd_offset + 2 + 12 * specs.len();
            let next = if index + 1 < ifds.len() {
                u32::try_from(cursor).unwrap()
            } else {
                0
            };
            bytes[next_at..next_at + 4].copy_from_slice(&next.to_le_bytes());
        }
        bytes[blob_offset..].copy_from_slice(blob);
        (bytes, blob_offset)
    }

    /// The shared TIFF reader now accepts the 0x55 magic (see module docs);
    /// tests keep normalizing the header as belt-and-braces so they also
    /// document that the IFD bodies are plain classic TIFF. The normalized
    /// copy is leaked for the lifetime of the test process — a `CameraFile`
    /// borrows its input, so it cannot be returned alongside the owned
    /// buffer.
    fn parse_normalized(data: &[u8]) -> CameraFile<'_> {
        let mut normalized = data.to_vec();
        normalized[2] = 42;
        CameraFile::parse_tiff(FORMAT, Box::leak(normalized.into_boxed_slice()))
            .expect("synthetic RW2 must parse")
    }

    fn quirks() -> Rw2Quirks {
        Rw2Quirks
    }

    // --- Panasonic packed bitstream encoder (test-only) ------------------

    /// Encodes `(value, nbits)` tokens into one 0x4000-byte file block using
    /// the exact inverse of `PanaBits`, including the block rotation.
    fn encode_pana_block(tokens: &[(u32, u8)], rotate: usize) -> [u8; PACKED_BLOCK_LEN] {
        let mut buffer = [0_u8; PACKED_BLOCK_LEN];
        let mut vbits: i32 = 0;
        for &(value, nbits) in tokens {
            vbits = (vbits - i32::from(nbits)) & 0x1_ffff;
            let byte = usize::try_from((vbits >> 3) ^ 0x3ff0).unwrap();
            let shift = u32::try_from(vbits & 7).unwrap();
            let word = value << shift;
            buffer[byte] |= (word & 0xff) as u8;
            if shift + u32::from(nbits) > 8 {
                buffer[byte + 1] |= ((word >> 8) & 0xff) as u8;
            }
        }
        let mut file = [0_u8; PACKED_BLOCK_LEN];
        for (k, slot) in file.iter_mut().enumerate() {
            *slot = buffer[(k + rotate) % PACKED_BLOCK_LEN];
        }
        file
    }

    /// Token stream for one packed row of `width` constant pixels, matching
    /// the dcraw loop (base load at i = 0/1, sh refresh at i % 3 == 2,
    /// zero deltas elsewhere).
    fn constant_row_tokens(value: u16) -> Vec<(u32, u8)> {
        let mut tokens = Vec::new();
        for i in 0..14_usize {
            if i % 3 == 2 {
                tokens.push((3, 2)); // sh = 4 >> (3 - 3) = 4
            }
            if i < 2 {
                tokens.push((u32::from(value >> 4), 8));
                tokens.push((u32::from(value & 0xf), 4));
            } else {
                tokens.push((0, 8)); // j == 0: predictor unchanged
            }
        }
        tokens
    }

    fn packed_rw2(width: u16, height: u16, block: &[u8; PACKED_BLOCK_LEN]) -> Vec<u8> {
        let strip_placeholder = vec![
            long(pana::SENSOR_WIDTH, u32::from(width)),
            long(pana::SENSOR_HEIGHT, u32::from(height)),
            short(pana::BITS_PER_SAMPLE, 12),
            short(tags::COMPRESSION, 1),
            short(pana::CFA_PATTERN, 1),
            long(tags::STRIP_OFFSETS, 0),
            long(tags::STRIP_BYTE_COUNTS, u32::try_from(PACKED_BLOCK_LEN).unwrap()),
            long(pana::RAW_DATA_OFFSET, 0),
        ];
        let blob_offset = u32::try_from(blob_offset_for(&[strip_placeholder])).unwrap();
        let specs = vec![
            long(pana::SENSOR_WIDTH, u32::from(width)),
            long(pana::SENSOR_HEIGHT, u32::from(height)),
            short(pana::BITS_PER_SAMPLE, 12),
            short(tags::COMPRESSION, 1),
            short(pana::CFA_PATTERN, 1),
            long(tags::STRIP_OFFSETS, blob_offset),
            long(tags::STRIP_BYTE_COUNTS, u32::try_from(PACKED_BLOCK_LEN).unwrap()),
            long(pana::RAW_DATA_OFFSET, blob_offset),
        ];
        build_rw2(&[specs], block).0
    }

    // --- Tests ------------------------------------------------------------

    #[test]
    fn rejects_non_rw2_signature() {
        let (bytes, _) = build_rw2(&[vec![short(tags::IMAGE_WIDTH, 4)]], &[]);
        let mut standard = bytes.clone();
        standard[2] = 42; // standard TIFF magic
        let error = quirks().parse_container(&standard).unwrap_err();
        assert!(
            matches!(&error, DecodeError::NativeCamera { format: FORMAT, .. }),
            "expected typed RW2 error, got {error:?}"
        );
    }

    #[test]
    fn parse_container_recognizes_iiu_signature() {
        let (bytes, _) = build_rw2(&[vec![short(pana::SENSOR_WIDTH, 4)]], &[]);
        // The shared TIFF reader accepts the 0x55 magic as classic TIFF, so a
        // well-formed `IIU\0` container must parse end to end.
        let file = quirks()
            .parse_container(&bytes)
            .expect("IIU\\0 container parses through the shared reader");
        assert!(!file.directories().is_empty());
    }

    /// Full Panasonic metadata IFD (`STRIP_OFFSETS` placeholder last; the test
    /// patches it with the real blob offset).
    fn metadata_placeholder() -> Vec<Spec> {
        vec![
            Spec {
                tag: pana::SENSOR_WIDTH,
                val: Val::U16(14),
            },
            Spec {
                tag: pana::SENSOR_HEIGHT,
                val: Val::U16(12),
            },
            Spec {
                tag: pana::SENSOR_TOP_BORDER,
                val: Val::U16(2),
            },
            Spec {
                tag: pana::SENSOR_LEFT_BORDER,
                val: Val::U16(4),
            },
            Spec {
                tag: pana::SENSOR_BOTTOM_BORDER,
                val: Val::U16(2),
            },
            Spec {
                tag: pana::SENSOR_RIGHT_BORDER,
                val: Val::U16(4),
            },
            Spec {
                tag: pana::CFA_PATTERN,
                val: Val::U16(4),
            }, // BGGR
            Spec {
                tag: pana::BITS_PER_SAMPLE,
                val: Val::U16(12),
            },
            Spec {
                tag: pana::LINEARITY_LIMIT_RED,
                val: Val::U16(4000),
            },
            Spec {
                tag: pana::LINEARITY_LIMIT_GREEN,
                val: Val::U16(4095),
            },
            Spec {
                tag: pana::LINEARITY_LIMIT_BLUE,
                val: Val::U16(4000),
            },
            Spec {
                tag: pana::BLACK_LEVEL_RED,
                val: Val::U16(100),
            },
            Spec {
                tag: pana::BLACK_LEVEL_GREEN,
                val: Val::U16(50),
            },
            Spec {
                tag: pana::BLACK_LEVEL_BLUE,
                val: Val::U16(120),
            },
            Spec {
                tag: pana::WB_RED_LEVEL,
                val: Val::U16(512),
            },
            Spec {
                tag: pana::WB_GREEN_LEVEL,
                val: Val::U16(256),
            },
            Spec {
                tag: pana::WB_BLUE_LEVEL,
                val: Val::U16(384),
            },
            Spec {
                tag: pana::CROP_TOP,
                val: Val::U16(3),
            },
            Spec {
                tag: pana::CROP_LEFT,
                val: Val::U16(5),
            },
            Spec {
                tag: pana::CROP_BOTTOM,
                val: Val::U16(11),
            },
            Spec {
                tag: pana::CROP_RIGHT,
                val: Val::U16(13),
            },
            Spec {
                tag: tags::MAKE,
                val: Val::Text("Panasonic"),
            },
            Spec {
                tag: tags::MODEL,
                val: Val::Text("DMC-GH2"),
            },
            short(tags::ORIENTATION, 6),
            long(tags::STRIP_OFFSETS, 0),
        ]
    }

    #[test]
    fn reads_panasonic_metadata_tags() {
        let blob = [0_u8; 16];
        let placeholder = metadata_placeholder();
        let blob_offset = u32::try_from(blob_offset_for(std::slice::from_ref(&placeholder))).unwrap();
        let mut specs = placeholder;
        let last = specs.len() - 1;
        specs[last] = long(tags::STRIP_OFFSETS, blob_offset);
        let (bytes, _) = build_rw2(&[specs], &blob);

        let file = parse_normalized(&bytes);
        let raw = quirks().select_raw_ifd(&file).unwrap();
        let metadata = quirks().read_metadata(&file, raw).unwrap();

        assert_eq!(metadata.make, "Panasonic");
        assert_eq!(metadata.model, "DMC-GH2");
        assert_eq!((metadata.width, metadata.height), (14, 12));
        assert_eq!(metadata.bits_per_sample, 12);
        assert_eq!(metadata.orientation, rrrah_core::Orientation::Rotate90);
        assert_eq!(
            metadata.cfa.cells,
            [CfaColor::Blue, CfaColor::Green, CfaColor::Green, CfaColor::Red]
        );
        // BGGR cells map black [B,G,G,R] -> [120, 50, 50, 100].
        assert_eq!(metadata.black_level.values, [120.0, 50.0, 50.0, 100.0]);
        assert_eq!(metadata.white_level.0, [4095.0]);
        // Exact binary fractions (512/256, 384/256): comparison is deterministic.
        #[allow(clippy::float_cmp)]
        {
            assert_eq!(metadata.white_balance, [2.0, 1.0, 1.5, 1.0]);
        }
        assert_eq!(metadata.active_area, Some(Rect::new(4, 2, 6, 8)));
        assert_eq!(metadata.crop_area, Some(Rect::new(5, 3, 8, 8)));
    }

    #[test]
    fn selects_the_directory_with_sensor_tags() {
        let blob = [0_u8; 64];
        let ifd0 = vec![
            short(tags::IMAGE_WIDTH, 64),
            short(tags::IMAGE_LENGTH, 48),
            short(tags::PHOTOMETRIC_INTERPRETATION, 1),
            long(tags::STRIP_OFFSETS, 0),
        ];
        let ifd1 = vec![
            long(pana::SENSOR_WIDTH, 300),
            long(pana::SENSOR_HEIGHT, 200),
            short(pana::CFA_PATTERN, 1),
            short(pana::BITS_PER_SAMPLE, 12),
            long(tags::STRIP_OFFSETS, 0),
        ];
        let blob_offset = u32::try_from(blob_offset_for(&[ifd0.clone(), ifd1.clone()])).unwrap();
        let ifd0 = vec![
            short(tags::IMAGE_WIDTH, 64),
            short(tags::IMAGE_LENGTH, 48),
            short(tags::PHOTOMETRIC_INTERPRETATION, 1),
            long(tags::STRIP_OFFSETS, blob_offset),
        ];
        let ifd1 = vec![
            long(pana::SENSOR_WIDTH, 300),
            long(pana::SENSOR_HEIGHT, 200),
            short(pana::CFA_PATTERN, 1),
            short(pana::BITS_PER_SAMPLE, 12),
            long(tags::STRIP_OFFSETS, blob_offset),
        ];
        let (bytes, _) = build_rw2(&[ifd0, ifd1], &blob);
        let file = parse_normalized(&bytes);
        let raw = quirks().select_raw_ifd(&file).unwrap();
        let (width, height) = dimensions(raw).unwrap().unwrap();
        assert_eq!((width, height), (300, 200));
    }

    #[test]
    fn decodes_unpacked_16_bit_strips() {
        let samples: Vec<u16> = (0..12).map(|v| v * 257).collect();
        let mut blob = Vec::new();
        for sample in &samples {
            blob.extend_from_slice(&sample.to_le_bytes());
        }
        let placeholder = vec![
            long(pana::SENSOR_WIDTH, 6),
            long(pana::SENSOR_HEIGHT, 2),
            short(tags::BITS_PER_SAMPLE, 16),
            short(tags::COMPRESSION, 1),
            short(pana::CFA_PATTERN, 1),
            long(tags::STRIP_OFFSETS, 0),
            long(tags::STRIP_BYTE_COUNTS, 24),
        ];
        let blob_offset = u32::try_from(blob_offset_for(std::slice::from_ref(&placeholder))).unwrap();
        let specs = vec![
            long(pana::SENSOR_WIDTH, 6),
            long(pana::SENSOR_HEIGHT, 2),
            short(tags::BITS_PER_SAMPLE, 16),
            short(tags::COMPRESSION, 1),
            short(pana::CFA_PATTERN, 1),
            long(tags::STRIP_OFFSETS, blob_offset),
            long(tags::STRIP_BYTE_COUNTS, 24),
        ];
        let (bytes, _) = build_rw2(&[specs], &blob);
        let file = parse_normalized(&bytes);
        let raw = quirks().select_raw_ifd(&file).unwrap();
        let pixels = quirks().decode_pixels(&file, raw, &|| false).unwrap();
        assert_eq!(pixels, samples);
    }

    #[test]
    fn decodes_panasonic_packed_constant_rows() {
        let mut tokens = constant_row_tokens(0xabc);
        tokens.extend(constant_row_tokens(0xabc));
        let block = encode_pana_block(&tokens, PACKED_BLOCK_ROTATE);
        let bytes = packed_rw2(14, 2, &block);
        let file = parse_normalized(&bytes);
        let raw = quirks().select_raw_ifd(&file).unwrap();
        let pixels = quirks().decode_pixels(&file, raw, &|| false).unwrap();
        assert_eq!(pixels, vec![0xabc_u16; 28]);
    }

    #[test]
    fn decodes_panasonic_packed_delta_paths() {
        // Hand-computed per dcraw's panasonic_load_raw (see module docs):
        // col0/1: base load 0x500; col2: sh=2, j=3 -> 0x30C; col4: j=1 ->
        // 0x110; col5: sh=0, j=2 -> 0x482; col7: j=255 -> 0x501; col8: sh=1,
        // j=4 -> 0x18; col9: j=128 -> 0x501; col10: j=200, negative-mask ->
        // 0x190; col11: sh=4, j=16, sh==4-mask -> 0x101; rest: j=0.
        let tokens: Vec<(u32, u8)> = vec![
            (0x50, 8),
            (0, 4), // i=0: pred[0] = 0x500
            (0x50, 8),
            (0, 4), // i=1: pred[1] = 0x500
            (2, 2),
            (3, 8), // i=2: sh=2, j=3
            (0, 8), // i=3
            (1, 8), // i=4
            (0, 2),
            (2, 8),   // i=5: sh=0, j=2
            (0, 8),   // i=6
            (255, 8), // i=7
            (1, 2),
            (4, 8),   // i=8: sh=1, j=4
            (128, 8), // i=9
            (200, 8), // i=10
            (3, 2),
            (16, 8), // i=11: sh=4, j=16
            (0, 8),  // i=12
            (0, 8),  // i=13
        ];
        let block = encode_pana_block(&tokens, PACKED_BLOCK_ROTATE);
        let bytes = packed_rw2(14, 1, &block);
        let file = parse_normalized(&bytes);
        let raw = quirks().select_raw_ifd(&file).unwrap();
        let pixels = quirks().decode_pixels(&file, raw, &|| false).unwrap();
        assert_eq!(
            pixels,
            [
                0x500, 0x500, 0x30c, 0x500, 0x110, 0x482, 0x110, 0x501, 0x18, 0x501, 0x190, 0x101, 0x190,
                0x101
            ]
        );
    }

    #[test]
    fn decodes_lossless_jpeg_strip() {
        let samples = [1000_u16, 1002, 999, 12, 500, 501, 502, 503];
        let stream = ljpeg_stream(4, 2, 12, &samples);
        let placeholder = vec![
            long(pana::SENSOR_WIDTH, 4),
            long(pana::SENSOR_HEIGHT, 2),
            short(tags::BITS_PER_SAMPLE, 12),
            short(tags::COMPRESSION, 7),
            short(pana::CFA_PATTERN, 1),
            long(tags::STRIP_OFFSETS, 0),
            long(tags::STRIP_BYTE_COUNTS, u32::try_from(stream.len()).unwrap()),
        ];
        let blob_offset = u32::try_from(blob_offset_for(std::slice::from_ref(&placeholder))).unwrap();
        let specs = vec![
            long(pana::SENSOR_WIDTH, 4),
            long(pana::SENSOR_HEIGHT, 2),
            short(tags::BITS_PER_SAMPLE, 12),
            short(tags::COMPRESSION, 7),
            short(pana::CFA_PATTERN, 1),
            long(tags::STRIP_OFFSETS, blob_offset),
            long(tags::STRIP_BYTE_COUNTS, u32::try_from(stream.len()).unwrap()),
        ];
        let (bytes, _) = build_rw2(&[specs], &stream);
        let file = parse_normalized(&bytes);
        let raw = quirks().select_raw_ifd(&file).unwrap();
        let pixels = quirks().decode_pixels(&file, raw, &|| false).unwrap();
        assert_eq!(pixels, samples);
    }

    #[test]
    fn rejects_unsupported_compression() {
        let bytes = rw2_with(4, 2, 12, 6, 16);
        let file = parse_normalized(&bytes);
        let raw = quirks().select_raw_ifd(&file).unwrap();
        let error = quirks().decode_pixels(&file, raw, &|| false).unwrap_err();
        assert!(
            matches!(&error, DecodeError::NativeCamera { format: FORMAT, message } if message.contains("Compression 6")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn rejects_unsupported_uncompressed_bit_depth() {
        // Compression 1, BitsPerSample 10, byte count that matches neither
        // variant: explicit typed rejection, never silent mis-decode.
        let bytes = rw2_with(4, 2, 10, 1, 10);
        let file = parse_normalized(&bytes);
        let raw = quirks().select_raw_ifd(&file).unwrap();
        let error = quirks().decode_pixels(&file, raw, &|| false).unwrap_err();
        assert!(
            matches!(&error, DecodeError::NativeCamera { format: FORMAT, message } if message.contains("BitsPerSample 10")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn rejects_multi_strip_lossless_jpeg() {
        let placeholder = vec![
            long(pana::SENSOR_WIDTH, 4),
            long(pana::SENSOR_HEIGHT, 2),
            short(tags::BITS_PER_SAMPLE, 12),
            short(tags::COMPRESSION, 7),
            short(pana::CFA_PATTERN, 1),
            Spec {
                tag: tags::STRIP_OFFSETS,
                val: Val::U32s(vec![0, 0]),
            },
            Spec {
                tag: tags::STRIP_BYTE_COUNTS,
                val: Val::U32s(vec![8, 8]),
            },
        ];
        let blob_offset = u32::try_from(blob_offset_for(std::slice::from_ref(&placeholder))).unwrap();
        let specs = vec![
            long(pana::SENSOR_WIDTH, 4),
            long(pana::SENSOR_HEIGHT, 2),
            short(tags::BITS_PER_SAMPLE, 12),
            short(tags::COMPRESSION, 7),
            short(pana::CFA_PATTERN, 1),
            Spec {
                tag: tags::STRIP_OFFSETS,
                val: Val::U32s(vec![blob_offset, blob_offset]),
            },
            Spec {
                tag: tags::STRIP_BYTE_COUNTS,
                val: Val::U32s(vec![8, 8]),
            },
        ];
        let (bytes, _) = build_rw2(&[specs], &[0_u8; 16]);
        let file = parse_normalized(&bytes);
        let raw = quirks().select_raw_ifd(&file).unwrap();
        let error = quirks().decode_pixels(&file, raw, &|| false).unwrap_err();
        assert!(
            matches!(&error, DecodeError::NativeCamera { format: FORMAT, message } if message.contains("exactly one strip")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn rejects_missing_cfa() {
        let blob = [0_u8; 16];
        let placeholder = vec![
            long(pana::SENSOR_WIDTH, 4),
            long(pana::SENSOR_HEIGHT, 2),
            short(tags::BITS_PER_SAMPLE, 16),
            short(tags::COMPRESSION, 1),
            long(tags::STRIP_OFFSETS, 0),
            long(tags::STRIP_BYTE_COUNTS, 16),
        ];
        let blob_offset = u32::try_from(blob_offset_for(std::slice::from_ref(&placeholder))).unwrap();
        let specs = vec![
            long(pana::SENSOR_WIDTH, 4),
            long(pana::SENSOR_HEIGHT, 2),
            short(tags::BITS_PER_SAMPLE, 16),
            short(tags::COMPRESSION, 1),
            long(tags::STRIP_OFFSETS, blob_offset),
            long(tags::STRIP_BYTE_COUNTS, 16),
        ];
        let (bytes, _) = build_rw2(&[specs], &blob);
        let file = parse_normalized(&bytes);
        let raw = quirks().select_raw_ifd(&file).unwrap();
        let error = quirks().read_metadata(&file, raw).unwrap_err();
        assert!(
            matches!(&error, DecodeError::NativeCamera { format: FORMAT, message } if message.contains("CFA")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn honours_cancellation() {
        let blob = [0_u8; 16];
        let placeholder = vec![
            long(pana::SENSOR_WIDTH, 4),
            long(pana::SENSOR_HEIGHT, 2),
            short(tags::BITS_PER_SAMPLE, 16),
            short(tags::COMPRESSION, 1),
            short(pana::CFA_PATTERN, 1),
            long(tags::STRIP_OFFSETS, 0),
            long(tags::STRIP_BYTE_COUNTS, 16),
        ];
        let blob_offset = u32::try_from(blob_offset_for(std::slice::from_ref(&placeholder))).unwrap();
        let specs = vec![
            long(pana::SENSOR_WIDTH, 4),
            long(pana::SENSOR_HEIGHT, 2),
            short(tags::BITS_PER_SAMPLE, 16),
            short(tags::COMPRESSION, 1),
            short(pana::CFA_PATTERN, 1),
            long(tags::STRIP_OFFSETS, blob_offset),
            long(tags::STRIP_BYTE_COUNTS, 16),
        ];
        let (bytes, _) = build_rw2(&[specs], &blob);
        let file = parse_normalized(&bytes);
        let raw = quirks().select_raw_ifd(&file).unwrap();
        assert!(matches!(
            quirks().decode_pixels(&file, raw, &|| true),
            Err(DecodeError::Cancelled)
        ));
    }

    /// Minimal file with tunable bps/compression for rejection tests.
    fn rw2_with(width: u16, height: u16, bps: u16, compression: u16, blob_len: usize) -> Vec<u8> {
        let blob = vec![0_u8; blob_len];
        let placeholder = vec![
            long(pana::SENSOR_WIDTH, u32::from(width)),
            long(pana::SENSOR_HEIGHT, u32::from(height)),
            short(tags::BITS_PER_SAMPLE, bps),
            short(tags::COMPRESSION, compression),
            short(pana::CFA_PATTERN, 1),
            long(tags::STRIP_OFFSETS, 0),
            long(tags::STRIP_BYTE_COUNTS, u32::try_from(blob_len).unwrap()),
        ];
        let blob_offset = u32::try_from(blob_offset_for(std::slice::from_ref(&placeholder))).unwrap();
        let specs = vec![
            long(pana::SENSOR_WIDTH, u32::from(width)),
            long(pana::SENSOR_HEIGHT, u32::from(height)),
            short(tags::BITS_PER_SAMPLE, bps),
            short(tags::COMPRESSION, compression),
            short(pana::CFA_PATTERN, 1),
            long(tags::STRIP_OFFSETS, blob_offset),
            long(tags::STRIP_BYTE_COUNTS, u32::try_from(blob_len).unwrap()),
        ];
        build_rw2(&[specs], &blob).0
    }

    // --- Minimal lossless-JPEG (SOF3) encoder for the compression-7 test ---

    fn ljpeg_stream(width: u16, height: u16, precision: u8, samples: &[u16]) -> Vec<u8> {
        struct BitWriter {
            bytes: Vec<u8>,
            current: u8,
            used: u8,
        }
        impl BitWriter {
            fn write(&mut self, value: u32, bits: u8) {
                for shift in (0..bits).rev() {
                    self.current = (self.current << 1) | u8::try_from((value >> shift) & 1).unwrap();
                    self.used += 1;
                    if self.used == 8 {
                        self.flush();
                    }
                }
            }
            fn pad_ones(&mut self) {
                while self.used != 0 {
                    self.current = (self.current << 1) | 1;
                    self.used += 1;
                    if self.used == 8 {
                        self.flush();
                    }
                }
            }
            fn flush(&mut self) {
                self.bytes.push(self.current);
                if self.current == 0xff {
                    self.bytes.push(0);
                }
                self.current = 0;
                self.used = 0;
            }
        }
        fn segment(output: &mut Vec<u8>, code: u8, payload: &[u8]) {
            output.extend_from_slice(&[0xff, code]);
            let length = u16::try_from(payload.len() + 2).unwrap();
            output.extend_from_slice(&length.to_be_bytes());
            output.extend_from_slice(payload);
        }

        assert_eq!(samples.len(), usize::from(width) * usize::from(height));
        let mut output = vec![0xff, 0xd8]; // SOI
        // DHT: table 0, 17 symbols of code length 5 covering categories 0..=16.
        let mut dht = vec![0_u8];
        dht.extend_from_slice(&[0, 0, 0, 0, 17, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        dht.extend(0_u8..=16);
        segment(&mut output, 0xc4, &dht);
        // SOF3: single component.
        let mut frame = vec![precision];
        frame.extend_from_slice(&height.to_be_bytes());
        frame.extend_from_slice(&width.to_be_bytes());
        frame.extend_from_slice(&[1, 1, 0x11, 0]);
        segment(&mut output, 0xc3, &frame);
        // SOS: predictor 1, point transform 0.
        segment(&mut output, 0xda, &[1, 1, 0, 1, 0, 0]);

        let initial = 1_i32 << (precision - 1);
        let width = usize::from(width);
        let mut bits = BitWriter {
            bytes: Vec::new(),
            current: 0,
            used: 0,
        };
        for (index, &sample) in samples.iter().enumerate() {
            let predicted = if index == 0 {
                initial
            } else if index < width {
                i32::from(samples[index - 1])
            } else if index % width == 0 {
                i32::from(samples[index - width])
            } else {
                i32::from(samples[index - 1]) // predictor 1: left
            };
            let modulo = (i32::from(sample) - predicted) & 0xffff;
            let difference = if modulo > 32_767 { modulo - 65_536 } else { modulo };
            let (category, encoded) = if difference == 0 {
                (0_u8, 0_u32)
            } else {
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
            };
            bits.write(u32::from(category), 5);
            bits.write(encoded, category);
        }
        bits.pad_ones();
        output.extend_from_slice(&bits.bytes);
        output.extend_from_slice(&[0xff, 0xd9]); // EOI
        output
    }
}
