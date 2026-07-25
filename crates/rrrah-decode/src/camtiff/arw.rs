//! Sony ARW quirks — Stage 2 implementation.
//!
//! ARW is a classic little-endian TIFF; the raw image is a single-sample CFA
//! plane selected by the shared generic selector (CFA photometric 32803 or
//! the largest primary directory). Three storage variants are handled here,
//! following the same taxonomy as LibRaw/rawspeed:
//!
//! - **Compression 1 (uncompressed)** — ARW 1.0-era DSLR/SLT bodies and the
//!   "uncompressed" mode of ARW 2.x bodies. Implemented for 16-bit words
//!   (12/14-bit data in a 16-bit container, container byte order) and for
//!   LSB-first packed 12/14-bit rows, dispatched on the actual strip byte
//!   totals. Sony packs LSB-first, so the shared MSB `decode_msb_packed`
//!   helper is NOT applicable to the packed rows.
//! - **Compression 32767 (Sony proprietary)** — two different encodings share
//!   this tag value, distinguished by byte count exactly like rawspeed's
//!   `ArwDecoder::decodeRawInternal`: when the strip holds exactly
//!   `width * height * bits / 8` bytes the data is NOT entropy coded and is
//!   either ARW 2.x "cRAW" (8 bits/pixel, block delta encoding) or, for
//!   12 bits/pixel, plain LSB-packed uncompressed rows. Otherwise the file
//!   uses the ARW 1.0 Huffman/curve encoding (`sony_arw_load_raw`), which is
//!   explicitly rejected with a typed error. 14-bit data under 32767 is
//!   rejected too (rawspeed parity; uncompressed 14-bit bodies tag
//!   compression 1).
//! - **Compression 7 (lossless JPEG)** — Alpha 1 and later; decoded through
//!   the shared TIFF/EP lossless-JPEG decoder, strips or tiles.
//!
//! The ARW 2.x cRAW block decoder reimplements dcraw's `sony_arw2_load_raw`
//! as clarified by rawspeed's `SonyArw2Decompressor`: the row is one byte
//! per pixel read as a continuous LSB-first bit stream; every 128-bit
//! (16-byte) block stores an 11-bit max, an 11-bit min, the 4-bit block
//! indices of the max/min pixels, and fourteen 7-bit deltas shifted by a
//! block-computed exponent, clamped to 11 bits. Blocks alternate between
//! even and odd column parities, so two blocks (32 bytes) yield 32
//! consecutive pixels. Output is `pixel << 1` (12-bit scale), matching the
//! identity tone curve dcraw/LibRaw use for these files. References:
//! <https://github.com/LibRaw/LibRaw> (`decoders_dcraw.cpp`),
//! <https://github.com/darktable-org/rawspeed> (`SonyArw2Decompressor.cpp`,
//! `ArwDecoder.cpp`), <https://github.com/lclevy/sony_raw>.
//!
//! Levels: the Sony makernote black/white levels sit in an encrypted tag
//! section on ARW 2.x bodies and are not read. When the DNG-style
//! `BlackLevel` (50714) / `WhiteLevel` (50717) tags are absent, defaults
//! follow the `LibRaw` `identify.cpp` rule: 512 for `cRAW` (12-bit output
//! scale) and `128 << (bits - 12)` otherwise; white defaults to
//! `(1 << bits) - 1`.
//! White balance and color matrices are not recoverable without the
//! encrypted makernote; neutral unity values are used, as documented.
//!
//! The unit tests below build synthetic minimal TIFF byte layouts in memory.
//! They validate the parser, the storage dispatch, and the ARW2 block math
//! against hand-computed vectors, but they are NOT evidence of compatibility
//! with real camera files — no licensed camera files are committed.

use rrrah_core::{CfaColor, CfaPattern, LevelGrid, Orientation, WhiteLevel};

use super::{
    CameraDirectory, CameraFile, CameraMetadata, CameraQuirks, camera_error, optional_ascii, optional_scalar,
    orientation_from_tag, required_scalar, tags,
};
use crate::{DecodeError, dng::lossless_jpeg};

const FORMAT: &str = "ARW";

/// Sony's proprietary compression marker shared by ARW 1.0 Huffman/curve
/// storage and the ARW 2.x block-delta / uncompressed variants.
const COMPRESSION_SONY: u64 = 32_767;
/// DNG-style `BlackLevel` tag some ARW 2.3 bodies write into the raw IFD.
const TAG_BLACK_LEVEL: u16 = 50_714;
/// DNG-style `WhiteLevel` tag (0xC61D).
const TAG_WHITE_LEVEL: u16 = 50_717;
/// Defensive cap on the decoded mosaic sample count.
const MAX_SAMPLES: usize = 512 * 1024 * 1024;

/// Registered ARW quirks.
#[derive(Debug)]
pub(crate) struct ArwQuirks;

impl CameraQuirks for ArwQuirks {
    fn format_name(&self) -> &'static str {
        FORMAT
    }

    fn read_metadata(
        &self,
        container: &CameraFile<'_>,
        raw: &CameraDirectory<'_>,
    ) -> Result<CameraMetadata, DecodeError> {
        let geometry = Geometry::read(raw)?;
        let cfa = read_cfa(raw)?;
        let make = optional_ascii(FORMAT, raw, tags::MAKE)?
            .or(first_ascii(container, tags::MAKE)?)
            .unwrap_or_else(|| "SONY".to_owned());
        let model = optional_ascii(FORMAT, raw, tags::MODEL)?
            .or(first_ascii(container, tags::MODEL)?)
            .unwrap_or_else(|| "unknown Sony camera".to_owned());
        let orientation = match optional_scalar(FORMAT, raw, tags::ORIENTATION)?
            .or(first_scalar(container, tags::ORIENTATION)?)
        {
            Some(value) => orientation_from_tag(FORMAT, value)?,
            None => Orientation::Normal,
        };

        let output_bits = geometry.output_bits();
        let black_level = read_black_level(raw, geometry)?;
        let white_level = match optional_scalar(FORMAT, raw, TAG_WHITE_LEVEL)? {
            Some(value) => WhiteLevel(vec![value as f32]),
            None => WhiteLevel(vec![((1_u32 << output_bits) - 1) as f32]),
        };

        Ok(CameraMetadata {
            make,
            model,
            width: geometry.width,
            height: geometry.height,
            bits_per_sample: output_bits,
            cfa,
            black_level,
            white_level,
            // Sony white balance and color matrices live in the (partly
            // encrypted) makernote; neutral defaults, as documented above.
            white_balance: [1.0; 4],
            xyz_to_camera: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]],
            active_area: None,
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
        let geometry = Geometry::read(raw)?;
        if let Some(spp) = optional_scalar(FORMAT, raw, tags::SAMPLES_PER_PIXEL)?
            && spp != 1
        {
            return Err(error(format!(
                "raw IFD has {spp} samples per pixel, only single-sample CFA is supported"
            )));
        }
        if optional_scalar(FORMAT, raw, tags::PHOTOMETRIC_INTERPRETATION)? != Some(tags::PHOTOMETRIC_CFA) {
            return Err(error("raw IFD is missing CFA photometric interpretation (32803)"));
        }

        let samples = geometry.samples()?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(samples)
            .map_err(|_| error(format!("could not allocate {samples} ARW samples")))?;
        pixels.resize(samples, 0);

        match geometry.compression {
            tags::COMPRESSION_UNCOMPRESSED => {
                decode_uncompressed(container, raw, geometry, &mut pixels, cancelled)?;
            }
            COMPRESSION_SONY => {
                decode_sony(container, raw, geometry, &mut pixels, cancelled)?;
            }
            tags::COMPRESSION_LOSSLESS_JPEG => {
                decode_lossless_jpeg(container, raw, geometry, &mut pixels, cancelled)?;
            }
            actual => {
                return Err(error(format!(
                    "unsupported ARW compression {actual} (supported: 1 uncompressed, \
                     7 lossless JPEG, 32767 Sony ARW 2.x)"
                )));
            }
        }
        Ok(pixels)
    }
}

/// Raw-IFD geometry shared by metadata and pixel decoding.
#[derive(Debug, Clone, Copy)]
struct Geometry {
    width: u32,
    height: u32,
    /// Stored bits per sample from the IFD (8 for ARW 2.x cRAW).
    bits_per_sample: u8,
    compression: u64,
}

impl Geometry {
    fn read(raw: &CameraDirectory<'_>) -> Result<Self, DecodeError> {
        let width = u32::try_from(required_scalar(FORMAT, raw, tags::IMAGE_WIDTH)?)
            .map_err(|_| error("image width does not fit u32"))?;
        let height = u32::try_from(required_scalar(FORMAT, raw, tags::IMAGE_LENGTH)?)
            .map_err(|_| error("image height does not fit u32"))?;
        let bits_per_sample = u8::try_from(required_scalar(FORMAT, raw, tags::BITS_PER_SAMPLE)?)
            .map_err(|_| error("bits per sample does not fit u8"))?;
        let compression = required_scalar(FORMAT, raw, tags::COMPRESSION)?;
        if width == 0 || height == 0 {
            return Err(error("raw IFD has zero image dimensions"));
        }
        Ok(Self {
            width,
            height,
            bits_per_sample,
            compression,
        })
    }

    fn width_usize(self) -> Result<usize, DecodeError> {
        usize::try_from(self.width).map_err(|_| error("image width overflows usize"))
    }

    fn height_usize(self) -> Result<usize, DecodeError> {
        usize::try_from(self.height).map_err(|_| error("image height overflows usize"))
    }

    fn samples(self) -> Result<usize, DecodeError> {
        let samples = self
            .width_usize()?
            .checked_mul(self.height_usize()?)
            .ok_or_else(|| error("image area overflows usize"))?;
        if samples > MAX_SAMPLES {
            return Err(error(format!(
                "image area {samples} samples exceeds the {MAX_SAMPLES} safety limit"
            )));
        }
        Ok(samples)
    }

    /// Decoded output bit depth: ARW 2.x cRAW stores 8 bits/pixel but
    /// decodes to a 12-bit scale (`pixel << 1`).
    const fn output_bits(self) -> u8 {
        if self.compression == COMPRESSION_SONY && self.bits_per_sample == 8 {
            12
        } else {
            self.bits_per_sample
        }
    }
}

fn error(message: impl Into<String>) -> DecodeError {
    camera_error(FORMAT, message)
}

/// Finds the first non-empty ASCII tag across all directories (identity
/// tags usually live in IFD0, not the raw `SubIFD`).
fn first_ascii(container: &CameraFile<'_>, tag: u16) -> Result<Option<String>, DecodeError> {
    for directory in container.directories() {
        if let Some(value) = optional_ascii(FORMAT, directory, tag)? {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

/// Finds the first scalar tag across all directories.
fn first_scalar(container: &CameraFile<'_>, tag: u16) -> Result<Option<u64>, DecodeError> {
    for directory in container.directories() {
        if let Some(value) = optional_scalar(FORMAT, directory, tag)? {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

/// Reads the 2x2 Bayer CFA from the standard TIFF/EP tags. Sony bodies are
/// all 2x2 RGB Bayer; anything else is a typed rejection.
fn read_cfa(raw: &CameraDirectory<'_>) -> Result<CfaPattern, DecodeError> {
    let dims_entry = raw
        .entry(FORMAT, tags::CFA_REPEAT_PATTERN_DIM)?
        .ok_or_else(|| error("raw IFD is missing CFARepeatPatternDim (33421)"))?;
    let dims = dims_entry
        .unsigned_values()
        .map_err(|err| error(format!("CFARepeatPatternDim: {err}")))?;
    if dims != [2, 2] {
        return Err(error(format!(
            "unsupported CFA repeat dimensions {dims:?} (only 2x2 Bayer is supported)"
        )));
    }
    let pattern_entry = raw
        .entry(FORMAT, tags::CFA_PATTERN)?
        .ok_or_else(|| error("raw IFD is missing CFAPattern (33422)"))?;
    let pattern = pattern_entry
        .unsigned_values()
        .map_err(|err| error(format!("CFAPattern: {err}")))?;
    if pattern.len() != 4 {
        return Err(error(format!(
            "CFAPattern has {} cells, expected 4 for a 2x2 Bayer pattern",
            pattern.len()
        )));
    }
    let cells = pattern
        .iter()
        .map(|&code| match code {
            0 => Ok(CfaColor::Red),
            1 => Ok(CfaColor::Green),
            2 => Ok(CfaColor::Blue),
            actual => Err(error(format!(
                "unsupported CFA color code {actual} (Sony is always RGB Bayer)"
            ))),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CfaPattern {
        width: 2,
        height: 2,
        cells,
    })
}

/// Reads the DNG-style `BlackLevel` tag when present; otherwise falls back
/// to the documented `LibRaw` defaults (512 for `cRAW` on its 12-bit output
/// scale, `128 << (bits - 12)` otherwise).
#[allow(clippy::cast_possible_truncation)] // tag values are validated finite and non-negative
fn read_black_level(raw: &CameraDirectory<'_>, geometry: Geometry) -> Result<LevelGrid, DecodeError> {
    if let Some(entry) = raw.entry(FORMAT, TAG_BLACK_LEVEL)? {
        let values = entry
            .numeric_values()
            .map_err(|err| error(format!("BlackLevel: {err}")))?;
        if values.is_empty() {
            return Err(error("BlackLevel tag is empty"));
        }
        if values.iter().any(|value| !value.is_finite() || *value < 0.0) {
            return Err(error("BlackLevel contains invalid values"));
        }
        if values.len() == 4 {
            return Ok(LevelGrid {
                width: 2,
                height: 2,
                components: 1,
                values: values.iter().map(|value| *value as f32).collect(),
            });
        }
        return Ok(LevelGrid {
            width: 1,
            height: 1,
            components: 1,
            values: vec![values[0] as f32],
        });
    }
    let default = if geometry.compression == COMPRESSION_SONY && geometry.bits_per_sample == 8 {
        512.0
    } else {
        let bits = geometry.bits_per_sample;
        if bits >= 12 {
            (128_u32 << (bits - 12)) as f32
        } else {
            (128_u32 >> (12 - bits)) as f32
        }
    };
    Ok(LevelGrid {
        width: 1,
        height: 1,
        components: 1,
        values: vec![default],
    })
}

/// A validated strip/tile segment view into the source bytes.
#[derive(Debug)]
struct Segment<'a> {
    bytes: &'a [u8],
}

/// Reads strip segments plus rows-per-strip, validating counts and bounds
/// with checked arithmetic before any slicing.
fn strip_segments<'a>(
    container: &CameraFile<'a>,
    raw: &CameraDirectory<'_>,
    geometry: Geometry,
) -> Result<(Vec<Segment<'a>>, usize), DecodeError> {
    let offsets = segment_values(raw, tags::STRIP_OFFSETS)?;
    let counts = segment_values(raw, tags::STRIP_BYTE_COUNTS)?;
    if offsets.len() != counts.len() {
        return Err(error(format!(
            "StripOffsets has {} entries but StripByteCounts has {}",
            offsets.len(),
            counts.len()
        )));
    }
    let height = geometry.height_usize()?;
    let rows_per_strip = match optional_scalar(FORMAT, raw, tags::ROWS_PER_STRIP)? {
        Some(value) => usize::try_from(value).map_err(|_| error("RowsPerStrip overflows usize"))?,
        None => height,
    };
    if rows_per_strip == 0 {
        return Err(error("RowsPerStrip is zero"));
    }
    let expected = height.div_ceil(rows_per_strip);
    if offsets.len() != expected {
        return Err(error(format!(
            "expected {expected} strips for {height} rows at {rows_per_strip} rows per strip, \
             found {}",
            offsets.len()
        )));
    }
    let segments = offsets
        .iter()
        .zip(&counts)
        .map(|(offset, count)| {
            let start = usize::try_from(*offset).map_err(|_| error("strip offset overflows usize"))?;
            let length = usize::try_from(*count).map_err(|_| error("strip byte count overflows usize"))?;
            let end = start
                .checked_add(length)
                .ok_or_else(|| error("strip extent overflows usize"))?;
            let bytes = container.data().get(start..end).ok_or_else(|| {
                error(format!(
                    "strip at offset {start} with {length} bytes is out of bounds"
                ))
            })?;
            Ok(Segment { bytes })
        })
        .collect::<Result<Vec<_>, DecodeError>>()?;
    Ok((segments, rows_per_strip))
}

fn segment_values(raw: &CameraDirectory<'_>, tag: u16) -> Result<Vec<u64>, DecodeError> {
    raw.entry(FORMAT, tag)?
        .ok_or_else(|| error(format!("required tag {tag} is missing")))?
        .unsigned_values()
        .map_err(|err| error(format!("tag {tag}: {err}")))
}

/// Compression 1: 16-bit words (container byte order) or LSB-first packed
/// rows, dispatched on the actual strip byte totals.
fn decode_uncompressed(
    container: &CameraFile<'_>,
    raw: &CameraDirectory<'_>,
    geometry: Geometry,
    output: &mut [u16],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), DecodeError> {
    if !matches!(geometry.bits_per_sample, 12 | 14 | 16) {
        return Err(error(format!(
            "unsupported uncompressed bit depth {} (supported: 12, 14, 16)",
            geometry.bits_per_sample
        )));
    }
    let (segments, rows_per_strip) = strip_segments(container, raw, geometry)?;
    let total: usize = segments.iter().map(|segment| segment.bytes.len()).sum();
    let width = geometry.width_usize()?;
    let height = geometry.height_usize()?;
    let word_row_bytes = width
        .checked_mul(2)
        .ok_or_else(|| error("16-bit row byte length overflows"))?;
    let packed_row_bytes = packed_row_bytes(width, geometry.bits_per_sample)?;
    let word_total = word_row_bytes
        .checked_mul(height)
        .ok_or_else(|| error("16-bit image byte length overflows"))?;
    let packed_total = packed_row_bytes
        .checked_mul(height)
        .ok_or_else(|| error("packed image byte length overflows"))?;

    if total == word_total {
        let byte_order = container.byte_order();
        decode_strip_rows(
            &segments,
            rows_per_strip,
            geometry,
            word_row_bytes,
            output,
            cancelled,
            |row_bytes, row_output| {
                decode_sixteen_bit_row(row_bytes, row_output, byte_order);
                Ok(())
            },
        )
    } else if geometry.bits_per_sample != 16 && total == packed_total {
        decode_strip_rows(
            &segments,
            rows_per_strip,
            geometry,
            packed_row_bytes,
            output,
            cancelled,
            |row_bytes, row_output| decode_lsb_packed_row(row_bytes, row_output, geometry.bits_per_sample),
        )
    } else {
        Err(error(format!(
            "uncompressed strips hold {total} bytes, expected {word_total} (16-bit words) \
             or {packed_total} (packed {}-bit)",
            geometry.bits_per_sample
        )))
    }
}

/// Compression 32767 dispatch, mirroring rawspeed's ARW1/ARW2 discriminator:
/// a strip payload of exactly `width * height * bits / 8` bytes is NOT
/// entropy coded; anything else is the ARW 1.0 Huffman/curve encoding, which
/// is not implemented and is rejected with an explicit typed error.
fn decode_sony(
    container: &CameraFile<'_>,
    raw: &CameraDirectory<'_>,
    geometry: Geometry,
    output: &mut [u16],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), DecodeError> {
    let (segments, rows_per_strip) = strip_segments(container, raw, geometry)?;
    let total: usize = segments.iter().map(|segment| segment.bytes.len()).sum();
    let width = geometry.width_usize()?;
    let height = geometry.height_usize()?;
    let packed_row = packed_row_bytes(width, geometry.bits_per_sample)?;
    let packed_total = packed_row
        .checked_mul(height)
        .ok_or_else(|| error("Sony payload byte length overflows"))?;

    if total != packed_total {
        return Err(error(format!(
            "compression 32767 with {} payload bytes does not match the {}-bit uncompressed \
             size {packed_total}: this is the ARW 1.0 Huffman/curve encoding (DSLR-A100 era), \
             which is not supported",
            total, geometry.bits_per_sample
        )));
    }
    match geometry.bits_per_sample {
        8 => decode_arw2(&segments, rows_per_strip, geometry, output, cancelled),
        12 => decode_strip_rows(
            &segments,
            rows_per_strip,
            geometry,
            packed_row,
            output,
            cancelled,
            |row_bytes, row_output| decode_lsb_packed_row(row_bytes, row_output, 12),
        ),
        actual => Err(error(format!(
            "compression 32767 with {actual} bits per sample is not supported \
             (ARW 2.x uncompressed 14-bit bodies tag compression 1 instead)"
        ))),
    }
}

/// ARW 2.x "cRAW" block-delta decoding (dcraw `sony_arw2_load_raw` /
/// rawspeed `SonyArw2Decompressor`). Rows are independent, one stored byte
/// per pixel, and each 16-byte block yields 16 pixels of one column parity,
/// so the image width must be a multiple of 32.
fn decode_arw2(
    segments: &[Segment<'_>],
    rows_per_strip: usize,
    geometry: Geometry,
    output: &mut [u16],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), DecodeError> {
    let width = geometry.width_usize()?;
    if width < 32 || !width.is_multiple_of(32) {
        return Err(error(format!(
            "ARW 2.x cRAW width {width} is not a positive multiple of 32"
        )));
    }
    decode_strip_rows(
        segments,
        rows_per_strip,
        geometry,
        width, // one stored byte per pixel
        output,
        cancelled,
        decode_arw2_row,
    )
}

/// Decodes one ARW 2.x row: a continuous LSB-first bit stream of
/// `width / 16` 128-bit blocks alternating even/odd column parity.
fn decode_arw2_row(row_bytes: &[u8], row_output: &mut [u16]) -> Result<(), DecodeError> {
    let width = row_output.len();
    debug_assert!(width >= 32 && width.is_multiple_of(32));
    debug_assert_eq!(row_bytes.len(), width);
    let mut bits = LsbBitReader::new(row_bytes);
    let mut col = 0_usize;
    while col < width {
        let max = bits.get_bits(11)?;
        let min = bits.get_bits(11)?;
        let imax = bits.get_bits(4)?;
        let imin = bits.get_bits(4)?;
        if imax == imin {
            return Err(error(
                "ARW 2.x block names the same pixel as both min and max (corrupt payload)",
            ));
        }
        let imax = usize::try_from(imax).map_err(|_| error("ARW 2.x block index overflows usize"))?;
        let imin = usize::try_from(imin).map_err(|_| error("ARW 2.x block index overflows usize"))?;
        // Exponent from the block spread, exactly as dcraw/rawspeed.
        let spread = i32::try_from(max).map_err(|_| error("ARW 2.x block max overflows i32"))?
            - i32::try_from(min).map_err(|_| error("ARW 2.x block min overflows i32"))?;
        let mut shift = 0_u32;
        while shift < 4 && (0x80_i32 << shift) <= spread {
            shift += 1;
        }
        for index in 0..16_usize {
            let value = if index == imax {
                max
            } else if index == imin {
                min
            } else {
                ((bits.get_bits(7)? << shift) + min).min(0x7ff)
            };
            // 11-bit values scaled to 12 bits; the tone curve dcraw/LibRaw
            // apply here is the identity for Sony files.
            row_output[col + 2 * index] =
                u16::try_from(value << 1).map_err(|_| error("ARW 2.x pixel overflows u16"))?;
        }
        col += if col & 1 == 1 { 31 } else { 1 };
    }
    Ok(())
}

/// Compression 7: lossless JPEG strips or tiles through the shared decoder.
fn decode_lossless_jpeg(
    container: &CameraFile<'_>,
    raw: &CameraDirectory<'_>,
    geometry: Geometry,
    output: &mut [u16],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), DecodeError> {
    if raw.entry(FORMAT, tags::TILE_OFFSETS)?.is_some() {
        decode_jpeg_tiles(container, raw, geometry, output, cancelled)
    } else {
        decode_jpeg_strips(container, raw, geometry, output, cancelled)
    }
}

fn decode_jpeg_strips(
    container: &CameraFile<'_>,
    raw: &CameraDirectory<'_>,
    geometry: Geometry,
    output: &mut [u16],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), DecodeError> {
    let (segments, rows_per_strip) = strip_segments(container, raw, geometry)?;
    let width = geometry.width_usize()?;
    let height = geometry.height_usize()?;
    for (index, segment) in segments.iter().enumerate() {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let first_row = index
            .checked_mul(rows_per_strip)
            .ok_or_else(|| error("JPEG strip first row overflows"))?;
        let rows = rows_per_strip.min(height - first_row);
        let expected = width
            .checked_mul(rows)
            .ok_or_else(|| error("JPEG strip sample count overflows"))?;
        let decoded = decode_jpeg_segment(segment.bytes, cancelled)?;
        validate_jpeg_segment(geometry, expected, &decoded)?;
        output[first_row * width..(first_row + rows) * width].copy_from_slice(&decoded.samples);
    }
    Ok(())
}

fn decode_jpeg_tiles(
    container: &CameraFile<'_>,
    raw: &CameraDirectory<'_>,
    geometry: Geometry,
    output: &mut [u16],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), DecodeError> {
    let tile_width = usize::try_from(required_scalar(FORMAT, raw, tags::TILE_WIDTH)?)
        .map_err(|_| error("TileWidth overflows usize"))?;
    let tile_height = usize::try_from(required_scalar(FORMAT, raw, tags::TILE_LENGTH)?)
        .map_err(|_| error("TileLength overflows usize"))?;
    if tile_width == 0 || tile_height == 0 {
        return Err(error("zero JPEG tile dimensions"));
    }
    let offsets = segment_values(raw, tags::TILE_OFFSETS)?;
    let counts = segment_values(raw, tags::TILE_BYTE_COUNTS)?;
    if offsets.len() != counts.len() {
        return Err(error(format!(
            "TileOffsets has {} entries but TileByteCounts has {}",
            offsets.len(),
            counts.len()
        )));
    }
    let width = geometry.width_usize()?;
    let height = geometry.height_usize()?;
    let tiles_x = width.div_ceil(tile_width);
    let tiles_y = height.div_ceil(tile_height);
    let expected_tiles = tiles_x
        .checked_mul(tiles_y)
        .ok_or_else(|| error("JPEG tile count overflows"))?;
    if offsets.len() != expected_tiles {
        return Err(error(format!(
            "expected {expected_tiles} JPEG tiles ({tiles_x}x{tiles_y}), found {}",
            offsets.len()
        )));
    }
    let expected_samples = tile_width
        .checked_mul(tile_height)
        .ok_or_else(|| error("JPEG tile sample count overflows"))?;

    for (index, (offset, count)) in offsets.iter().zip(&counts).enumerate() {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let start = usize::try_from(*offset).map_err(|_| error("tile offset overflows usize"))?;
        let length = usize::try_from(*count).map_err(|_| error("tile byte count overflows usize"))?;
        let end = start
            .checked_add(length)
            .ok_or_else(|| error("tile extent overflows usize"))?;
        let bytes = container.data().get(start..end).ok_or_else(|| {
            error(format!(
                "tile at offset {start} with {length} bytes is out of bounds"
            ))
        })?;
        let decoded = decode_jpeg_segment(bytes, cancelled)?;
        validate_jpeg_segment(geometry, expected_samples, &decoded)?;

        let first_x = (index % tiles_x)
            .checked_mul(tile_width)
            .ok_or_else(|| error("JPEG tile left overflows"))?;
        let first_y = (index / tiles_x)
            .checked_mul(tile_height)
            .ok_or_else(|| error("JPEG tile top overflows"))?;
        let copy_width = tile_width.min(width - first_x);
        let copy_height = tile_height.min(height - first_y);
        for row in 0..copy_height {
            if cancelled() {
                return Err(DecodeError::Cancelled);
            }
            let source_start = row
                .checked_mul(tile_width)
                .ok_or_else(|| error("JPEG tile source row overflows"))?;
            let target_start = (first_y + row)
                .checked_mul(width)
                .and_then(|base| base.checked_add(first_x))
                .ok_or_else(|| error("JPEG tile target row overflows"))?;
            output[target_start..target_start + copy_width]
                .copy_from_slice(&decoded.samples[source_start..source_start + copy_width]);
        }
    }
    Ok(())
}

fn decode_jpeg_segment(
    bytes: &[u8],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<lossless_jpeg::LosslessJpegImage, DecodeError> {
    lossless_jpeg::decode(bytes, cancelled).map_err(|err| match err {
        lossless_jpeg::LosslessJpegError::Cancelled { .. } => DecodeError::Cancelled,
        other => error(format!("lossless JPEG segment: {other}")),
    })
}

fn validate_jpeg_segment(
    geometry: Geometry,
    expected_samples: usize,
    decoded: &lossless_jpeg::LosslessJpegImage,
) -> Result<(), DecodeError> {
    if decoded.precision != geometry.bits_per_sample {
        return Err(error(format!(
            "lossless JPEG precision {} does not match IFD bits per sample {}",
            decoded.precision, geometry.bits_per_sample
        )));
    }
    if decoded.component_ids.len() != 1 {
        return Err(error(format!(
            "lossless JPEG segment has {} components, only single-component CFA is supported",
            decoded.component_ids.len()
        )));
    }
    if decoded.samples.len() != expected_samples {
        return Err(error(format!(
            "lossless JPEG segment decoded {} samples, expected {expected_samples}",
            decoded.samples.len()
        )));
    }
    Ok(())
}

/// Row-wise strip driver: validates per-strip byte lengths, honors the
/// cancellation callback once per row, and delegates row decoding.
fn decode_strip_rows(
    segments: &[Segment<'_>],
    rows_per_strip: usize,
    geometry: Geometry,
    row_bytes: usize,
    output: &mut [u16],
    cancelled: &(dyn Fn() -> bool + Sync),
    mut decode_row: impl FnMut(&[u8], &mut [u16]) -> Result<(), DecodeError>,
) -> Result<(), DecodeError> {
    let width = geometry.width_usize()?;
    let height = geometry.height_usize()?;
    for (index, segment) in segments.iter().enumerate() {
        let first_row = index
            .checked_mul(rows_per_strip)
            .ok_or_else(|| error("strip first row overflows"))?;
        let rows = rows_per_strip.min(height - first_row);
        let expected = row_bytes
            .checked_mul(rows)
            .ok_or_else(|| error("strip byte length overflows"))?;
        if segment.bytes.len() != expected {
            return Err(error(format!(
                "strip {index} has {} bytes, expected {expected} ({rows} rows of {row_bytes} bytes)",
                segment.bytes.len()
            )));
        }
        for row in 0..rows {
            if cancelled() {
                return Err(DecodeError::Cancelled);
            }
            let source_start = row
                .checked_mul(row_bytes)
                .ok_or_else(|| error("strip source row overflows"))?;
            let target_start = (first_row + row)
                .checked_mul(width)
                .ok_or_else(|| error("strip target row overflows"))?;
            decode_row(
                &segment.bytes[source_start..source_start + row_bytes],
                &mut output[target_start..target_start + width],
            )?;
        }
    }
    Ok(())
}

fn decode_sixteen_bit_row(row_bytes: &[u8], row_output: &mut [u16], byte_order: crate::dng::tiff::ByteOrder) {
    debug_assert_eq!(row_bytes.len(), row_output.len() * 2);
    for (target, bytes) in row_output.iter_mut().zip(row_bytes.chunks_exact(2)) {
        *target = byte_order.u16(bytes);
    }
}

/// Sony packs sub-16-bit samples LSB-first (2 pixels in 3 bytes for 12-bit),
/// which the shared MSB-first `decode_msb_packed` helper cannot decode.
fn decode_lsb_packed_row(
    row_bytes: &[u8],
    row_output: &mut [u16],
    bits_per_sample: u8,
) -> Result<(), DecodeError> {
    debug_assert!(matches!(bits_per_sample, 9..=15));
    let mut reservoir = 0_u64;
    let mut reservoir_bits = 0_u8;
    let mut next_byte = 0_usize;
    let mask = (1_u64 << bits_per_sample) - 1;
    for target in row_output {
        while reservoir_bits < bits_per_sample {
            let byte = row_bytes
                .get(next_byte)
                .copied()
                .ok_or_else(|| error("truncated packed ARW row"))?;
            reservoir |= u64::from(byte) << reservoir_bits;
            reservoir_bits += 8;
            next_byte += 1;
        }
        *target = u16::try_from(reservoir & mask).map_err(|_| error("packed ARW sample overflows u16"))?;
        reservoir >>= bits_per_sample;
        reservoir_bits -= bits_per_sample;
    }
    Ok(())
}

fn packed_row_bytes(width: usize, bits_per_sample: u8) -> Result<usize, DecodeError> {
    width
        .checked_mul(usize::from(bits_per_sample))
        .and_then(|bits| bits.checked_add(7))
        .map(|bits| bits / 8)
        .ok_or_else(|| error("packed row byte length overflows"))
}

/// Continuous LSB-first bit reader over one ARW 2.x row.
#[derive(Debug)]
struct LsbBitReader<'a> {
    bytes: &'a [u8],
    bit_position: usize,
}

impl<'a> LsbBitReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            bit_position: 0,
        }
    }

    fn get_bits(&mut self, count: u8) -> Result<u32, DecodeError> {
        debug_assert!(count <= 32);
        let mut value = 0_u32;
        let mut filled = 0_u8;
        while filled < count {
            let byte_index = self.bit_position / 8;
            let bit_index = u8::try_from(self.bit_position % 8).expect("a remainder modulo 8 fits u8");
            let byte = self
                .bytes
                .get(byte_index)
                .copied()
                .ok_or_else(|| error("truncated ARW 2.x row bit stream"))?;
            let take = (8 - bit_index).min(count - filled);
            let mask = (1_u16 << take) - 1;
            value |= u32::from(u16::from(byte >> bit_index) & mask) << filled;
            self.bit_position += usize::from(take);
            filled += take;
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TYPE_BYTE: u16 = 1;
    const TYPE_ASCII: u16 = 2;
    const TYPE_SHORT: u16 = 3;
    const TYPE_LONG: u16 = 4;

    type EntrySpec = (u16, u16, u32, [u8; 4]);

    fn short(tag: u16, value: u16) -> EntrySpec {
        let bytes = value.to_le_bytes();
        (tag, TYPE_SHORT, 1, [bytes[0], bytes[1], 0, 0])
    }

    fn long(tag: u16, value: u32) -> EntrySpec {
        (tag, TYPE_LONG, 1, value.to_le_bytes())
    }

    fn shorts2(tag: u16, first: u16, second: u16) -> EntrySpec {
        let a = first.to_le_bytes();
        let b = second.to_le_bytes();
        (tag, TYPE_SHORT, 2, [a[0], a[1], b[0], b[1]])
    }

    fn bytes4(tag: u16, value: [u8; 4]) -> EntrySpec {
        (tag, TYPE_BYTE, 4, value)
    }

    fn ascii4(tag: u16, value: [u8; 4]) -> EntrySpec {
        (tag, TYPE_ASCII, 4, value)
    }

    const fn ifd_size(entries: usize) -> usize {
        2 + 12 * entries + 4
    }

    /// Builds a little-endian TIFF with an identity IFD0 and a raw CFA IFD1
    /// whose single strip starts right after the directory tables.
    fn synthetic_arw(width: u32, height: u32, bits: u16, compression: u16, pixels: &[u8]) -> Vec<u8> {
        let ifd0 = [ascii4(271, *b"SONY"), ascii4(272, *b"A7T\0"), short(274, 1)];
        let pixel_offset = 8 + ifd_size(ifd0.len()) + ifd_size(12);
        let ifd1 = [
            long(254, 0),
            long(256, width),
            long(257, height),
            short(258, bits),
            short(259, compression),
            short(262, 32_803),
            long(273, u32::try_from(pixel_offset).unwrap()),
            short(277, 1),
            long(278, height),
            long(279, u32::try_from(pixels.len()).unwrap()),
            shorts2(33_421, 2, 2),
            bytes4(33_422, [0, 1, 1, 2]),
        ];
        let mut bytes = vec![0_u8; pixel_offset];
        bytes[..8].copy_from_slice(&[b'I', b'I', 42, 0, 8, 0, 0, 0]);
        write_ifd(
            &mut bytes,
            8,
            &ifd0,
            u32::try_from(8 + ifd_size(ifd0.len())).unwrap(),
        );
        write_ifd(&mut bytes, 8 + ifd_size(ifd0.len()), &ifd1, 0);
        bytes.extend_from_slice(pixels);
        bytes
    }

    fn write_ifd(bytes: &mut [u8], at: usize, entries: &[EntrySpec], next: u32) {
        bytes[at..at + 2].copy_from_slice(&u16::try_from(entries.len()).unwrap().to_le_bytes());
        for (index, &(tag, field_type, count, inline)) in entries.iter().enumerate() {
            let start = at + 2 + index * 12;
            bytes[start..start + 2].copy_from_slice(&tag.to_le_bytes());
            bytes[start + 2..start + 4].copy_from_slice(&field_type.to_le_bytes());
            bytes[start + 4..start + 8].copy_from_slice(&count.to_le_bytes());
            bytes[start + 8..start + 12].copy_from_slice(&inline);
        }
        let next_at = at + 2 + entries.len() * 12;
        bytes[next_at..next_at + 4].copy_from_slice(&next.to_le_bytes());
    }

    fn decode(bytes: &[u8]) -> Result<(CameraMetadata, Vec<u16>), DecodeError> {
        let quirks = ArwQuirks;
        let container = quirks.parse_container(bytes)?;
        let raw = quirks.select_raw_ifd(&container)?;
        let metadata = quirks.read_metadata(&container, raw)?;
        let pixels = quirks.decode_pixels(&container, raw, &|| false)?;
        Ok((metadata, pixels))
    }

    #[test]
    fn parses_header_selects_raw_ifd_and_reads_metadata() {
        let mut pixel_bytes = Vec::new();
        for value in [1_u16, 2, 3, 4, 400, 500, 600, 16_383] {
            pixel_bytes.extend_from_slice(&value.to_le_bytes());
        }
        let file = synthetic_arw(4, 2, 16, 1, &pixel_bytes);
        let quirks = ArwQuirks;
        let container = quirks.parse_container(&file).unwrap();
        assert_eq!(container.directories().len(), 2);
        let raw = quirks.select_raw_ifd(&container).unwrap();
        assert_eq!(usize::try_from(raw.offset()).unwrap(), 8 + ifd_size(3));

        let metadata = quirks.read_metadata(&container, raw).unwrap();
        assert_eq!(metadata.make, "SONY");
        assert_eq!(metadata.model, "A7T");
        assert_eq!((metadata.width, metadata.height), (4, 2));
        assert_eq!(metadata.bits_per_sample, 16);
        assert_eq!(
            metadata.cfa.cells,
            [CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue]
        );
        // 16-bit uncompressed default: 128 << (16 - 12).
        assert_eq!(metadata.black_level.values, [2048.0]);
        assert_eq!(metadata.white_level.0, [65_535.0]);
        assert_eq!(metadata.orientation, Orientation::Normal);
    }

    #[test]
    fn decodes_uncompressed_sixteen_bit_words() {
        let values = [1_u16, 2, 3, 4, 400, 500, 600, 16_383];
        let mut pixel_bytes = Vec::new();
        for value in values {
            pixel_bytes.extend_from_slice(&value.to_le_bytes());
        }
        let file = synthetic_arw(4, 2, 16, 1, &pixel_bytes);
        let (metadata, pixels) = decode(&file).unwrap();
        assert_eq!(pixels, values);
        assert_eq!(metadata.bits_per_sample, 16);
    }

    #[test]
    fn decodes_uncompressed_lsb_packed_twelve_bit() {
        // LSB-first packing of [0x001, 0xabc, 0xfff, 0x123].
        let packed = [0x01_u8, 0xc0, 0xab, 0xff, 0x3f, 0x12];
        let file = synthetic_arw(4, 1, 12, 1, &packed);
        let (metadata, pixels) = decode(&file).unwrap();
        assert_eq!(pixels, [0x001, 0xabc, 0xfff, 0x123]);
        assert_eq!(metadata.bits_per_sample, 12);
        // 12-bit default black: 128.
        assert_eq!(metadata.black_level.values, [128.0]);
        assert_eq!(metadata.white_level.0, [4095.0]);
    }

    #[test]
    fn decodes_sony_32767_uncompressed_twelve_bit_variant() {
        // Compression 32767 with a byte count matching 12-bit unpacked size
        // is plain LSB-packed storage (A900-style), not entropy coded.
        let packed = [0x01_u8, 0xc0, 0xab, 0xff, 0x3f, 0x12];
        let file = synthetic_arw(4, 1, 12, 32_767, &packed);
        let (_, pixels) = decode(&file).unwrap();
        assert_eq!(pixels, [0x001, 0xabc, 0xfff, 0x123]);
    }

    #[test]
    fn rejects_arw1_huffman_curve_variant_with_typed_error() {
        // Byte count does not match the uncompressed size: this is the
        // DSLR-A100-era Huffman/curve encoding.
        let file = synthetic_arw(4, 1, 12, 32_767, &[1, 2, 3, 4, 5]);
        let error = decode(&file).unwrap_err();
        let DecodeError::NativeCamera {
            format: "ARW",
            message,
        } = error
        else {
            panic!("expected typed ARW error, got {error:?}");
        };
        assert!(
            message.contains("ARW 1.0") && message.contains("not supported"),
            "unexpected message: {message}"
        );
    }

    /// Encodes one 16-byte ARW 2.x block (LSB-first): 30-bit header plus
    /// fourteen 7-bit deltas for the non-extremal pixels.
    fn arw2_block(max: u16, min: u16, imax: u8, imin: u8, deltas: [u16; 14]) -> [u8; 16] {
        let mut acc = 0_u128;
        let mut shift = 0_u32;
        let mut push = |value: u16, bits: u32| {
            acc |= u128::from(value) << shift;
            shift += bits;
        };
        push(max, 11);
        push(min, 11);
        push(u16::from(imax), 4);
        push(u16::from(imin), 4);
        let mut delta = deltas.iter();
        for index in 0..16_u8 {
            if index == imax || index == imin {
                continue;
            }
            push(*delta.next().expect("fourteen deltas"), 7);
        }
        assert_eq!(shift, 128);
        acc.to_le_bytes()
    }

    /// Independent reference for what a block must decode to (pre-shift
    /// 11-bit values), mirroring the documented dcraw/rawspeed semantics.
    fn arw2_reference(max: u16, min: u16, imax: u8, imin: u8, deltas: [u16; 14]) -> [u16; 16] {
        let spread = i32::from(max) - i32::from(min);
        let mut shift = 0_u32;
        while shift < 4 && (0x80_i32 << shift) <= spread {
            shift += 1;
        }
        let mut output = [0_u16; 16];
        let mut delta = deltas.iter();
        for (index, target) in output.iter_mut().enumerate() {
            let index = u8::try_from(index).unwrap();
            let value = if index == imax {
                u32::from(max)
            } else if index == imin {
                u32::from(min)
            } else {
                ((u32::from(*delta.next().expect("fourteen deltas")) << shift) + u32::from(min)).min(0x7ff)
            };
            *target = u16::try_from(value << 1).unwrap();
        }
        output
    }

    #[test]
    fn decodes_arw2_craw_delta_blocks() {
        // Width 32 = one even-parity block plus one odd-parity block.
        let deltas0 = [0, 1, 2, 3, 127, 64, 33, 7, 15, 16, 31, 63, 5, 100];
        let block0 = arw2_block(700, 100, 3, 9, deltas0);
        let expected0 = arw2_reference(700, 100, 3, 9, deltas0);
        // Min-heavy block exercises the 0x7ff clamp (127 + 2000 > 0x7ff).
        let deltas1 = [127; 14];
        let block1 = arw2_block(0x7ff, 2_000, 15, 0, deltas1);
        let expected1 = arw2_reference(0x7ff, 2_000, 15, 0, deltas1);

        let mut row = Vec::new();
        row.extend_from_slice(&block0);
        row.extend_from_slice(&block1);
        let file = synthetic_arw(32, 1, 8, 32_767, &row);

        let (metadata, pixels) = decode(&file).unwrap();
        let mut expected = vec![0_u16; 32];
        for index in 0..16 {
            expected[2 * index] = expected0[index];
            expected[2 * index + 1] = expected1[index];
        }
        assert_eq!(pixels, expected);
        // cRAW decodes to the 12-bit scale with the documented 512 black.
        assert_eq!(metadata.bits_per_sample, 12);
        assert_eq!(metadata.black_level.values, [512.0]);
        assert_eq!(metadata.white_level.0, [4095.0]);
    }

    #[test]
    fn rejects_arw2_width_not_multiple_of_32() {
        let file = synthetic_arw(30, 1, 8, 32_767, &[0_u8; 30]);
        let error = decode(&file).unwrap_err();
        let DecodeError::NativeCamera {
            format: "ARW",
            message,
        } = error
        else {
            panic!("expected typed ARW error, got {error:?}");
        };
        assert!(
            message.contains("multiple of 32"),
            "unexpected message: {message}"
        );
    }

    #[test]
    fn rejects_arw2_block_with_equal_extrema_indices() {
        // Equal imax/imin: the decoder rejects the block from its 30-bit
        // header before reading any delta, so the tail content is padding.
        let mut acc = 0_u128;
        let mut shift = 0_u32;
        let mut push = |value: u16, bits: u32| {
            acc |= u128::from(value) << shift;
            shift += bits;
        };
        push(700, 11);
        push(100, 11);
        push(5, 4);
        push(5, 4);
        for _ in 0..14 {
            push(0, 7);
        }
        assert_eq!(shift, 128);
        let block = acc.to_le_bytes();
        let mut row = Vec::new();
        row.extend_from_slice(&block);
        row.extend_from_slice(&block);
        let file = synthetic_arw(32, 1, 8, 32_767, &row);
        let error = decode(&file).unwrap_err();
        let DecodeError::NativeCamera {
            format: "ARW",
            message,
        } = error
        else {
            panic!("expected typed ARW error, got {error:?}");
        };
        assert!(message.contains("min and max"), "unexpected message: {message}");
    }

    #[test]
    fn rejects_unsupported_compression_with_typed_error() {
        let file = synthetic_arw(4, 2, 12, 6, &[0_u8; 12]);
        let error = decode(&file).unwrap_err();
        let DecodeError::NativeCamera {
            format: "ARW",
            message,
        } = error
        else {
            panic!("expected typed ARW error, got {error:?}");
        };
        assert!(
            message.contains("unsupported ARW compression 6"),
            "unexpected message: {message}"
        );
    }

    #[test]
    fn rejects_missing_cfa_tags_with_typed_error() {
        // Rebuild a file whose raw IFD lacks the CFA pattern tags.
        let pixels = [0_u8; 16];
        let ifd0 = [ascii4(271, *b"SONY")];
        let pixel_offset = 8 + ifd_size(ifd0.len()) + ifd_size(10);
        let ifd1 = [
            long(254, 0),
            long(256, 4),
            long(257, 2),
            short(258, 16),
            short(259, 1),
            short(262, 32_803),
            long(273, u32::try_from(pixel_offset).unwrap()),
            short(277, 1),
            long(278, 2),
            long(279, 16),
        ];
        let mut bytes = vec![0_u8; pixel_offset];
        bytes[..8].copy_from_slice(&[b'I', b'I', 42, 0, 8, 0, 0, 0]);
        write_ifd(
            &mut bytes,
            8,
            &ifd0,
            u32::try_from(8 + ifd_size(ifd0.len())).unwrap(),
        );
        write_ifd(&mut bytes, 8 + ifd_size(ifd0.len()), &ifd1, 0);
        bytes.extend_from_slice(&pixels);

        let quirks = ArwQuirks;
        let container = quirks.parse_container(&bytes).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.read_metadata(&container, raw).unwrap_err();
        let DecodeError::NativeCamera {
            format: "ARW",
            message,
        } = error
        else {
            panic!("expected typed ARW error, got {error:?}");
        };
        assert!(
            message.contains("CFARepeatPatternDim"),
            "unexpected message: {message}"
        );
    }

    #[test]
    fn cancelled_callback_aborts_decode() {
        let mut pixel_bytes = Vec::new();
        for value in [1_u16, 2, 3, 4, 400, 500, 600, 16_383] {
            pixel_bytes.extend_from_slice(&value.to_le_bytes());
        }
        let file = synthetic_arw(4, 2, 16, 1, &pixel_bytes);
        let quirks = ArwQuirks;
        let container = quirks.parse_container(&file).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.decode_pixels(&container, raw, &|| true).unwrap_err();
        assert!(matches!(error, DecodeError::Cancelled));
    }
}
