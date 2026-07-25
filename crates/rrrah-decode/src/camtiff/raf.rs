//! Fujifilm RAF quirks — Stage 2 implementation.
//!
//! RAF is NOT a TIFF at offset 0. The container layout (all pointer fields
//! are big-endian `u32`, regardless of the embedded TIFF byte order):
//!
//! - `0x00..0x10`: ASCII magic `FUJIFILMCCD-RAW ` (16 bytes, trailing space);
//! - `0x10..0x14`: ASCII format version (e.g. `0201`);
//! - `0x14..0x1C`: ASCII camera product code (e.g. `FF109502`);
//! - `0x54`: embedded JPEG preview offset, `0x58`: its length — the JPEG
//!   payload is NEVER used by this backend (project policy);
//! - `0x5C`: RAF directory offset, `0x60`: its length. The RAF directory is
//!   a big-endian catalog: `u32` entry count, then per entry `u16` tag,
//!   `u16` byte length, and that many inline bytes. Tags used here:
//!   `0x100` full raw size (`u16` height, `u16` width), `0x110` crop
//!   top/left, `0x111` cropped size, `0x130` layout byte (bit 7 set =
//!   rotated 45° Super-CCD layout), `0x131` X-Trans pattern (36 bytes);
//! - `0x64`: `FujiIFD` offset, `0x68`: its length. On X/GFX-series files this
//!   is a complete embedded TIFF (its own `II`/`MM` header; little-endian on
//!   every file inspected). Its first IFD carries tag `0xF000`
//!   (`FujiLayout`/raw-IFD pointer, TIFF type IFD) pointing at the raw
//!   sub-IFD, which holds the proprietary tags:
//!   `0xF001` full width, `0xF002` full height, `0xF003` bits per sample,
//!   `0xF007` strip offset (relative to the embedded TIFF header),
//!   `0xF008` strip byte count, `0xF00A` black level (1, 4 = 2x2, or
//!   36 = 6x6 X-Trans values), `0xF00E` white-balance G/R/B levels.
//!
//! Pixel storage (single strip in every file inspected):
//!
//! - uncompressed 12/14-bit: LSB-first bit-packed rows (verified against
//!   dcraw `packed_load_raw` and a real X-A2 file; the shared
//!   `decode_msb_packed` helper is MSB-first and therefore does NOT apply —
//!   the LSB unpacker is private to this file, see the report);
//! - uncompressed 16-bit: one `u16` per sample in container byte order;
//! - `Compression = 7` (or a strip too small for unpacked data): handed to
//!   the shared lossless-JPEG decoder. Fuji's proprietary compressed format
//!   used by X-Trans bodies (and GFX) does not start with a JPEG SOI marker
//!   and is rejected explicitly instead of being mis-decoded.
//!
//! CFA policy (see `docs/DECODE_FORMAT_AUDIT.md`: preserving a non-Bayer CFA
//! is not supporting it):
//!
//! - 2x2 RGB Bayer bodies (X-A1/A2/A3/A5/A10, X-T100/T200, GFX) decode.
//!   The phase comes from `CFARepeatPatternDim`/`CFAPattern` when the IFD
//!   carries them; real Fuji Bayer files carry neither, so the documented
//!   default is RGGB anchored at full-sensor (0,0) — every known Fuji Bayer
//!   vendor crop origin is even/even, so crop-relative and sensor-relative
//!   phases coincide (cross-checked with dcraw `filters` and rawspeed
//!   `cameras.xml`).
//! - X-Trans (6x6) is a typed rejection. Detection, in order: RAF-directory
//!   tag `0x131` (rejected inside `parse_container`), a non-2x2
//!   `CFARepeatPatternDim`, or a 36-value `0xF00A` black level.
//! - Rotated Super-CCD (RAF-directory `0x130` bit 7) and pre-X-series RAF
//!   variants without an embedded TIFF are typed rejections as well.
//!
//! Metadata limitations (no silent degradation, explicit documentation):
//!
//! - make/model/orientation live in the embedded JPEG's EXIF TIFF and in the
//!   RAF header; neither is reachable from the embedded-TIFF stage, so make
//!   is the constant `FUJIFILM` (guaranteed by the container magic), model
//!   is a fixed placeholder, and orientation defaults to `Normal`.
//! - the vendor crop (RAF-directory `0x110`/`0x111`) is likewise outside the
//!   embedded TIFF; `active_area`/`crop_area` are `None` (full-sensor
//!   output). Recipe note for the orchestrator: `CameraFormat::Raf::recipe`
//!   advertises `DECODE_CROP_AS_METADATA`; RAF currently reports no crop
//!   rather than a wrong one.
//! - no color matrix exists in RAF (dcraw/rawspeed use model tables);
//!   `xyz_to_camera` is the identity fallback.
//!
//! Test caveat: all tests below build synthetic minimal containers in-memory
//! (no licensed camera files). They validate the parser and the typed
//! rejections, but they are NOT proof of camera compatibility; the layout
//! claims above were cross-checked against dcraw, `LibRaw`, rawspeed, `ExifTool`
//! and public sample headers.

use rrrah_core::{CfaColor, CfaPattern, LevelGrid, Orientation, WhiteLevel};

use crate::{
    DecodeError,
    dng::{
        lossless_jpeg::{self, LosslessJpegError},
        tiff::{Entry, Ifd},
    },
};

use super::{CameraDirectory, CameraFile, CameraMetadata, CameraQuirks, camera_error, tags};

const FORMAT: &str = "RAF";

/// Container magic, 16 bytes including the trailing space.
const RAF_MAGIC: &[u8; 16] = b"FUJIFILMCCD-RAW ";
/// Smallest header that still contains the `FujiIFD` pointer at `0x64`.
const HEADER_MIN_LEN: usize = 0x68;
const OFF_RAF_DIRECTORY_OFFSET: usize = 0x5c;
const OFF_FUJI_IFD_OFFSET: usize = 0x64;
/// RAF directories on real files have ~10 entries; this cap is generous.
const MAX_RAF_DIRECTORY_ENTRIES: u32 = 1_024;
/// Sanity bounds on the stored mosaic (mirrors rawspeed's `RafDecoder`).
const MAX_FUJI_WIDTH: u64 = 11_808;
const MAX_FUJI_HEIGHT: u64 = 8_754;

/// Fujifilm proprietary tags inside the embedded TIFF.
mod fuji {
    pub(crate) const RAW_IFD_POINTER: u16 = 0xf000;
    pub(crate) const FULL_WIDTH: u16 = 0xf001;
    pub(crate) const FULL_HEIGHT: u16 = 0xf002;
    pub(crate) const BITS_PER_SAMPLE: u16 = 0xf003;
    pub(crate) const STRIP_OFFSETS: u16 = 0xf007;
    pub(crate) const STRIP_BYTE_COUNTS: u16 = 0xf008;
    pub(crate) const BLACK_LEVEL: u16 = 0xf00a;
    pub(crate) const WB_GRB_LEVELS: u16 = 0xf00e;
}

/// Tags of the big-endian RAF directory at header offset `0x5C`.
mod raf_dir {
    pub(crate) const LAYOUT: u16 = 0x130;
    pub(crate) const XTRANS_PATTERN: u16 = 0x131;
}

/// Registered RAF quirks.
#[derive(Debug)]
pub(crate) struct RafQuirks;

impl CameraQuirks for RafQuirks {
    fn format_name(&self) -> &'static str {
        FORMAT
    }

    fn parse_container<'a>(&self, data: &'a [u8]) -> Result<CameraFile<'a>, DecodeError> {
        if data.len() < HEADER_MIN_LEN {
            return Err(camera_error(
                FORMAT,
                format!(
                    "RAF header is truncated: need at least {HEADER_MIN_LEN} bytes, have {}",
                    data.len()
                ),
            ));
        }
        if data[..RAF_MAGIC.len()] != RAF_MAGIC[..] {
            return Err(camera_error(FORMAT, "missing FUJIFILMCCD-RAW magic"));
        }

        // The RAF directory holds the authoritative CFA-layout signals; it
        // lives outside the embedded TIFF, so X-Trans and rotated Super-CCD
        // bodies are rejected here, before any TIFF work.
        Self::check_raf_directory(data)?;

        let tiff_offset = usize::try_from(be_u32(data, OFF_FUJI_IFD_OFFSET)?)
            .map_err(|_| camera_error(FORMAT, "FujiIFD offset does not fit usize"))?;
        if tiff_offset == 0 {
            return Err(camera_error(
                FORMAT,
                "no embedded TIFF (FujiIFD): pre-X-series / Super-CCD RAF variants are not supported",
            ));
        }
        let payload = data.get(tiff_offset..).ok_or_else(|| {
            camera_error(
                FORMAT,
                format!(
                    "FujiIFD offset {tiff_offset} is beyond end of file ({} bytes)",
                    data.len()
                ),
            )
        })?;
        // Offsets inside the embedded TIFF (the raw-IFD pointer and the
        // strip offset) are relative to its own header, which is exactly
        // the start of this slice.
        CameraFile::parse_tiff(FORMAT, payload)
    }

    fn select_raw_ifd<'a>(
        &self,
        container: &'a CameraFile<'a>,
    ) -> Result<&'a CameraDirectory<'a>, DecodeError> {
        // The Fuji raw sub-IFD is reached through the proprietary 0xF000
        // pointer, not through the generic SubIFDs chain that
        // `CameraFile::parse_tiff` walks. Prefer the directory that carries
        // that pointer; accept a directory that directly holds the Fuji
        // strip tags as a fallback shape.
        for directory in container.directories() {
            if directory.entry(FORMAT, fuji::RAW_IFD_POINTER)?.is_some() {
                return Ok(directory);
            }
        }
        for directory in container.directories() {
            if directory.entry(FORMAT, fuji::STRIP_OFFSETS)?.is_some() {
                return Ok(directory);
            }
        }
        super::select_generic_raw_ifd(container, FORMAT)
    }

    fn read_metadata(
        &self,
        container: &CameraFile<'_>,
        raw: &CameraDirectory<'_>,
    ) -> Result<CameraMetadata, DecodeError> {
        let raw_ifd = RawIfd::resolve(container, raw)?;
        let (width, height) = geometry(&raw_ifd)?;
        let bits_per_sample = bits_per_sample(&raw_ifd)?;
        let cfa = cfa_pattern(&raw_ifd)?;
        let black_level = black_level(&raw_ifd)?;
        let white_level = WhiteLevel(vec![((1_u32 << bits_per_sample) - 1) as f32]);
        let white_balance = white_balance(&raw_ifd)?;
        let orientation = match optional_u64(&raw_ifd, tags::ORIENTATION)? {
            Some(value) => super::orientation_from_tag(FORMAT, value)?,
            None => Orientation::Normal,
        };
        // Make/model/orientation live in the embedded JPEG's EXIF, which is
        // unreachable from the embedded-TIFF stage (see module docs).
        let make = "FUJIFILM".to_owned();
        let model = "FUJIFILM camera".to_owned();
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
        let raw_ifd = RawIfd::resolve(container, raw)?;
        let (width, height) = geometry(&raw_ifd)?;
        let width_usize =
            usize::try_from(width).map_err(|_| camera_error(FORMAT, "raw width does not fit usize"))?;
        let height_usize =
            usize::try_from(height).map_err(|_| camera_error(FORMAT, "raw height does not fit usize"))?;
        let sample_count = width_usize
            .checked_mul(height_usize)
            .ok_or_else(|| camera_error(FORMAT, "overflow computing RAF sample count"))?;

        let offsets = required_values(&raw_ifd, fuji::STRIP_OFFSETS)
            .or_else(|_| required_values(&raw_ifd, tags::STRIP_OFFSETS))?;
        let counts = required_values(&raw_ifd, fuji::STRIP_BYTE_COUNTS)
            .or_else(|_| required_values(&raw_ifd, tags::STRIP_BYTE_COUNTS))?;
        if offsets.len() != 1 || counts.len() != 1 {
            return Err(camera_error(
                FORMAT,
                format!(
                    "expected exactly one raw strip, got {} offsets and {} byte counts",
                    offsets.len(),
                    counts.len()
                ),
            ));
        }
        let strip_offset = usize::try_from(offsets[0])
            .map_err(|_| camera_error(FORMAT, "strip offset does not fit usize"))?;
        let strip_length = usize::try_from(counts[0])
            .map_err(|_| camera_error(FORMAT, "strip byte count does not fit usize"))?;
        let strip_end = strip_offset
            .checked_add(strip_length)
            .ok_or_else(|| camera_error(FORMAT, "overflow computing strip end"))?;
        let strip = container.data().get(strip_offset..strip_end).ok_or_else(|| {
            camera_error(
                FORMAT,
                format!(
                    "raw strip [{strip_offset}, {strip_end}) is beyond end of embedded TIFF ({} bytes)",
                    container.data().len()
                ),
            )
        })?;

        let tag_bits = bits_per_sample(&raw_ifd)?;
        let pixels_bits = u128::from(width)
            .checked_mul(u128::from(height))
            .ok_or_else(|| camera_error(FORMAT, "overflow computing pixel count"))?;
        let strip_bits = (strip_length as u128)
            .checked_mul(8)
            .ok_or_else(|| camera_error(FORMAT, "overflow computing strip bit length"))?;

        match optional_u64(&raw_ifd, tags::COMPRESSION)? {
            Some(tags::COMPRESSION_UNCOMPRESSED) => {
                decode_uncompressed(container, strip, width_usize, height_usize, tag_bits, cancelled)
            }
            Some(tags::COMPRESSION_LOSSLESS_JPEG) => {
                decode_lossless_jpeg(strip, width, height, sample_count, cancelled)
            }
            Some(other) => Err(camera_error(
                FORMAT,
                format!("unsupported RAF compression {other} (only 1 = uncompressed and 7 = lossless JPEG)"),
            )),
            None => {
                // Fuji Bayer files carry no Compression tag; infer storage
                // from the strip size exactly like rawspeed does. A strip
                // too small for 12-bit unpacked data must be compressed.
                let storage_bits = if strip_bits >= 16 * pixels_bits {
                    16
                } else if strip_bits >= 14 * pixels_bits {
                    14
                } else if strip_bits >= 12 * pixels_bits {
                    12
                } else {
                    return decode_lossless_jpeg(strip, width, height, sample_count, cancelled);
                };
                decode_uncompressed(
                    container,
                    strip,
                    width_usize,
                    height_usize,
                    storage_bits,
                    cancelled,
                )
            }
        }
    }
}

impl RafQuirks {
    /// Parses the big-endian RAF directory and rejects CFA layouts this
    /// backend must not decode: X-Trans (`0x131`) and rotated 45° Super-CCD
    /// (`0x130` bit 7). Geometry from this directory is intentionally not
    /// trusted ahead of the embedded TIFF tags.
    fn check_raf_directory(data: &[u8]) -> Result<(), DecodeError> {
        let dir_offset = usize::try_from(be_u32(data, OFF_RAF_DIRECTORY_OFFSET)?)
            .map_err(|_| camera_error(FORMAT, "RAF directory offset does not fit usize"))?;
        if dir_offset == 0 {
            return Err(camera_error(FORMAT, "RAF directory offset is zero"));
        }
        let count_bytes = data
            .get(
                dir_offset
                    ..dir_offset
                        .checked_add(4)
                        .ok_or_else(|| camera_error(FORMAT, "overflow locating RAF directory"))?,
            )
            .ok_or_else(|| {
                camera_error(
                    FORMAT,
                    format!("RAF directory offset {dir_offset} is beyond end of file"),
                )
            })?;
        let entries = u32::from_be_bytes(count_bytes.try_into().expect("a four-byte slice"));
        if entries == 0 || entries > MAX_RAF_DIRECTORY_ENTRIES {
            return Err(camera_error(
                FORMAT,
                format!("RAF directory entry count {entries} is outside 1..={MAX_RAF_DIRECTORY_ENTRIES}"),
            ));
        }
        let mut cursor = dir_offset
            .checked_add(4)
            .ok_or_else(|| camera_error(FORMAT, "overflow walking RAF directory"))?;
        for _ in 0..entries {
            let header = data
                .get(
                    cursor
                        ..cursor
                            .checked_add(4)
                            .ok_or_else(|| camera_error(FORMAT, "overflow walking RAF directory"))?,
                )
                .ok_or_else(|| camera_error(FORMAT, "RAF directory is truncated mid-entry"))?;
            let tag = u16::from_be_bytes([header[0], header[1]]);
            let length = usize::from(u16::from_be_bytes([header[2], header[3]]));
            let value_start = cursor
                .checked_add(4)
                .ok_or_else(|| camera_error(FORMAT, "overflow walking RAF directory"))?;
            let value_end = value_start
                .checked_add(length)
                .ok_or_else(|| camera_error(FORMAT, "overflow walking RAF directory"))?;
            let value = data
                .get(value_start..value_end)
                .ok_or_else(|| camera_error(FORMAT, "RAF directory entry is truncated"))?;
            match tag {
                raf_dir::XTRANS_PATTERN => {
                    return Err(camera_error(
                        FORMAT,
                        "X-Trans (6x6) CFA is not supported: the project fast path decodes only 2x2 RGB Bayer; \
                         RAF directory tag 0x131 marks this as an X-Trans file",
                    ));
                }
                raf_dir::LAYOUT if value.first().is_some_and(|byte| byte & 0x80 != 0) => {
                    return Err(camera_error(
                        FORMAT,
                        "rotated 45-degree Super-CCD layout (RAF directory tag 0x130) is not supported",
                    ));
                }
                _ => {}
            }
            cursor = value_end;
        }
        Ok(())
    }
}

/// The directory that actually holds the raw tags: either the raw sub-IFD
/// reached through the proprietary `0xF000` pointer, or a directory that
/// directly carries the Fuji strip tags.
enum RawIfd<'a> {
    Sub(Ifd<'a>),
    Direct(&'a CameraDirectory<'a>),
}

impl<'a> RawIfd<'a> {
    fn resolve(container: &CameraFile<'a>, raw: &'a CameraDirectory<'a>) -> Result<Self, DecodeError> {
        if let Some(pointer) = raw
            .entry(FORMAT, fuji::RAW_IFD_POINTER)?
            .map(|entry| {
                entry
                    .unsigned_scalar()
                    .map_err(|error| camera_error(FORMAT, format!("tag 0xf000: {error}")))
            })
            .transpose()?
        {
            if pointer == 0 {
                return Err(camera_error(FORMAT, "Fuji raw-IFD pointer (tag 0xf000) is zero"));
            }
            return Ok(Self::Sub(container.parse_ifd_at(FORMAT, pointer)?));
        }
        if raw.entry(FORMAT, fuji::STRIP_OFFSETS)?.is_some() {
            return Ok(Self::Direct(raw));
        }
        Err(camera_error(
            FORMAT,
            "neither the Fuji raw-IFD pointer (0xf000) nor Fuji strip offsets (0xf007) were found",
        ))
    }

    fn entry(&self, tag: u16) -> Result<Option<&Entry<'a>>, DecodeError> {
        match self {
            Self::Sub(ifd) => ifd
                .entry(tag)
                .map_err(|error| camera_error(FORMAT, format!("tag {tag:#06x}: {error}"))),
            Self::Direct(directory) => directory.entry(FORMAT, tag),
        }
    }
}

fn optional_u64(raw: &RawIfd<'_>, tag: u16) -> Result<Option<u64>, DecodeError> {
    raw.entry(tag)?
        .map(|entry| {
            entry
                .unsigned_scalar()
                .map_err(|error| camera_error(FORMAT, format!("tag {tag:#06x}: {error}")))
        })
        .transpose()
}

fn optional_values(raw: &RawIfd<'_>, tag: u16) -> Result<Option<Vec<u64>>, DecodeError> {
    raw.entry(tag)?
        .map(|entry| {
            entry
                .unsigned_values()
                .map_err(|error| camera_error(FORMAT, format!("tag {tag:#06x}: {error}")))
        })
        .transpose()
}

fn required_values(raw: &RawIfd<'_>, tag: u16) -> Result<Vec<u64>, DecodeError> {
    optional_values(raw, tag)?
        .ok_or_else(|| camera_error(FORMAT, format!("required tag {tag:#06x} is missing")))
}

/// Reads a big-endian `u32` at a fixed header offset (bounds-checked).
fn be_u32(data: &[u8], offset: usize) -> Result<u32, DecodeError> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| camera_error(FORMAT, format!("RAF header is truncated at offset {offset:#x}")))?;
    Ok(u32::from_be_bytes(bytes.try_into().expect("a four-byte slice")))
}

/// Full stored mosaic geometry: Fuji tags first, baseline TIFF tags as a
/// fallback, with sanity bounds.
fn geometry(raw: &RawIfd<'_>) -> Result<(u32, u32), DecodeError> {
    let width = optional_u64(raw, fuji::FULL_WIDTH)?
        .or(optional_u64(raw, tags::IMAGE_WIDTH)?)
        .ok_or_else(|| camera_error(FORMAT, "raw width tag (0xf001 or 0x0100) is missing"))?;
    let height = optional_u64(raw, fuji::FULL_HEIGHT)?
        .or(optional_u64(raw, tags::IMAGE_LENGTH)?)
        .ok_or_else(|| camera_error(FORMAT, "raw height tag (0xf002 or 0x0101) is missing"))?;
    if width == 0 || height == 0 || width > MAX_FUJI_WIDTH || height > MAX_FUJI_HEIGHT {
        return Err(camera_error(
            FORMAT,
            format!("unexpected RAF dimensions {width}x{height}"),
        ));
    }
    let width = u32::try_from(width).map_err(|_| camera_error(FORMAT, "raw width exceeds u32"))?;
    let height = u32::try_from(height).map_err(|_| camera_error(FORMAT, "raw height exceeds u32"))?;
    Ok((width, height))
}

/// Declared sample depth: Fuji tag first, baseline tag as a fallback.
fn bits_per_sample(raw: &RawIfd<'_>) -> Result<u8, DecodeError> {
    let bits = optional_u64(raw, fuji::BITS_PER_SAMPLE)?
        .or(optional_u64(raw, tags::BITS_PER_SAMPLE)?)
        .unwrap_or(16);
    let bits =
        u8::try_from(bits).map_err(|_| camera_error(FORMAT, format!("bits per sample {bits} exceeds u8")))?;
    if !(9..=16).contains(&bits) {
        return Err(camera_error(
            FORMAT,
            format!("unsupported RAF bit depth {bits} (expected 9..=16)"),
        ));
    }
    Ok(bits)
}

/// Builds the CFA description, rejecting every non-2x2-RGB-Bayer layout
/// with a typed error.
fn cfa_pattern(raw: &RawIfd<'_>) -> Result<CfaPattern, DecodeError> {
    if let Some(dims) = optional_values(raw, tags::CFA_REPEAT_PATTERN_DIM)? {
        if dims.len() != 2 {
            return Err(camera_error(
                FORMAT,
                format!("CFARepeatPatternDim has {} values, expected 2", dims.len()),
            ));
        }
        if dims != [2, 2] {
            return Err(camera_error(
                FORMAT,
                format!(
                    "unsupported CFA repeat pattern {}x{}: the project fast path decodes only 2x2 RGB Bayer \
                     (X-Trans 6x6 and other layouts are preserved as an explicit rejection, never remapped)",
                    dims[0], dims[1]
                ),
            ));
        }
        let pattern_entry = raw.entry(tags::CFA_PATTERN)?.ok_or_else(|| {
            camera_error(FORMAT, "CFARepeatPatternDim is present but CFAPattern is missing")
        })?;
        let bytes = pattern_entry.raw_bytes();
        if bytes.len() != 4 {
            return Err(camera_error(
                FORMAT,
                format!(
                    "CFAPattern has {} bytes, expected 4 for a 2x2 pattern",
                    bytes.len()
                ),
            ));
        }
        let mut cells = Vec::with_capacity(4);
        for &code in bytes {
            cells.push(match code {
                0 => CfaColor::Red,
                1 => CfaColor::Green,
                2 => CfaColor::Blue,
                other => {
                    return Err(camera_error(
                        FORMAT,
                        format!("CFAPattern color code {other} is not RGB (0/1/2)"),
                    ));
                }
            });
        }
        return validated_bayer(cells);
    }

    // Real Fuji Bayer files carry no CFA tags at all. A 36-value black
    // level is Fuji's 6x6 X-Trans black grid (cross-checked on X-T10):
    // reject it rather than guessing a layout.
    if let Some(black) = optional_values(raw, fuji::BLACK_LEVEL)?
        && black.len() == 36
    {
        return Err(camera_error(
            FORMAT,
            "X-Trans (6x6) CFA is not supported: black level tag 0xf00a holds 36 values (6x6 grid); \
             the project fast path decodes only 2x2 RGB Bayer",
        ));
    }
    // Documented default: every known Fuji Bayer body (X-A1/A2/A3/A5/A10,
    // X-T100/T200) is RGGB anchored at full-sensor (0,0); all known vendor
    // crop origins are even/even so crop-relative phase matches (dcraw
    // filters = 0x94949494, rawspeed cameras.xml).
    validated_bayer(vec![
        CfaColor::Red,
        CfaColor::Green,
        CfaColor::Green,
        CfaColor::Blue,
    ])
}

fn validated_bayer(cells: Vec<CfaColor>) -> Result<CfaPattern, DecodeError> {
    let cfa = CfaPattern {
        width: 2,
        height: 2,
        cells,
    };
    cfa.bayer_quad().map_err(|_| {
        camera_error(
            FORMAT,
            "CFA pattern is not a 2x2 RGB Bayer quad (1 red, 2 green, 1 blue)",
        )
    })?;
    Ok(cfa)
}

fn black_level(raw: &RawIfd<'_>) -> Result<LevelGrid, DecodeError> {
    let Some(values) = optional_values(raw, fuji::BLACK_LEVEL)? else {
        // DNG-style default; every Fuji file inspected carries the tag.
        return Ok(LevelGrid {
            width: 1,
            height: 1,
            components: 1,
            values: vec![0.0],
        });
    };
    let grid = match values.len() {
        1 => (1, 1),
        4 => (2, 2),
        36 => {
            return Err(camera_error(
                FORMAT,
                "X-Trans (6x6) CFA is not supported: black level tag 0xf00a holds 36 values (6x6 grid)",
            ));
        }
        count => {
            return Err(camera_error(
                FORMAT,
                format!("black level tag 0xf00a has {count} values, expected 1 or 4"),
            ));
        }
    };
    let levels: Vec<f32> = values.iter().map(|&value| value as f32).collect();
    if levels.iter().any(|value| !value.is_finite()) {
        return Err(camera_error(FORMAT, "black level contains a non-finite value"));
    }
    Ok(LevelGrid {
        width: grid.0,
        height: grid.1,
        components: 1,
        values: levels,
    })
}

/// White balance from Fuji's G/R/B levels (`0xf00e`), normalized to green.
fn white_balance(raw: &RawIfd<'_>) -> Result<[f32; 4], DecodeError> {
    let Some(values) = optional_values(raw, fuji::WB_GRB_LEVELS)? else {
        return Ok([1.0; 4]);
    };
    if values.len() != 3 {
        return Err(camera_error(
            FORMAT,
            format!(
                "white-balance tag 0xf00e has {} values, expected 3 (G, R, B)",
                values.len()
            ),
        ));
    }
    let (green, red, blue) = (values[0] as f32, values[1] as f32, values[2] as f32);
    if green <= 0.0 || red <= 0.0 || blue <= 0.0 {
        return Err(camera_error(
            FORMAT,
            format!("non-positive white-balance level in tag 0xf00e (G={green}, R={red}, B={blue})"),
        ));
    }
    Ok([red / green, 1.0, blue / green, 1.0])
}

/// Decodes the single-strip uncompressed storage: 16-bit samples in
/// container byte order, or Fuji's LSB-first bit packing for 9..=15 bits.
fn decode_uncompressed(
    container: &CameraFile<'_>,
    strip: &[u8],
    width: usize,
    height: usize,
    bits_per_sample: u8,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<u16>, DecodeError> {
    let row_bytes = if bits_per_sample == 16 {
        width
            .checked_mul(2)
            .ok_or_else(|| camera_error(FORMAT, "overflow computing RAF row byte length"))?
    } else {
        width
            .checked_mul(usize::from(bits_per_sample))
            .and_then(|bits| bits.checked_add(7))
            .map(|bits| bits / 8)
            .ok_or_else(|| camera_error(FORMAT, "overflow computing RAF row byte length"))?
    };
    let expected = row_bytes
        .checked_mul(height)
        .ok_or_else(|| camera_error(FORMAT, "overflow computing RAF strip byte length"))?;
    if strip.len() < expected {
        return Err(camera_error(
            FORMAT,
            format!(
                "raw strip holds {} bytes, unpacked {width}x{height} at {bits_per_sample} bits needs {expected}",
                strip.len()
            ),
        ));
    }
    let sample_count = width
        .checked_mul(height)
        .ok_or_else(|| camera_error(FORMAT, "overflow computing RAF sample count"))?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(sample_count)
        .map_err(|_| camera_error(FORMAT, format!("could not allocate {sample_count} RAF samples")))?;
    pixels.resize(sample_count, 0);

    let byte_order = container.byte_order();
    for row in 0..height {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let source = &strip[row * row_bytes..(row + 1) * row_bytes];
        let target = &mut pixels[row * width..(row + 1) * width];
        if bits_per_sample == 16 {
            for (sample, bytes) in target.iter_mut().zip(source.chunks_exact(2)) {
                *sample = byte_order.u16(bytes);
            }
        } else {
            decode_lsb_packed(source, target, bits_per_sample);
        }
    }
    Ok(pixels)
}

/// Fuji's uncompressed packing is LSB-first (least-significant bits of the
/// first byte hold the low bits of the first sample), the exact mirror of
/// the shared MSB-first `decode_msb_packed`. Verified against dcraw
/// `packed_load_raw` and a real X-A2 file. Kept private because shared
/// files are orchestrator-owned.
fn decode_lsb_packed(encoded: &[u8], output: &mut [u16], bits_per_sample: u8) {
    debug_assert!((9..=15).contains(&bits_per_sample));
    let mask = (1_u32 << bits_per_sample) - 1;
    let mut accumulator = 0_u32;
    let mut available = 0_u8;
    let mut next_byte = 0_usize;
    for sample in output {
        while available < bits_per_sample {
            accumulator |= u32::from(encoded[next_byte]) << available;
            available += 8;
            next_byte += 1;
        }
        *sample = u16::try_from(accumulator & mask).expect("masked sample fits u16");
        accumulator >>= bits_per_sample;
        available -= bits_per_sample;
    }
}

/// Compression-7 path through the shared lossless-JPEG decoder. Fuji's
/// proprietary compressed format (X-Trans bodies, GFX) does not begin with
/// a JPEG SOI marker and is rejected explicitly instead of producing a
/// confusing parser error or, worse, wrong pixels.
fn decode_lossless_jpeg(
    strip: &[u8],
    width: u32,
    height: u32,
    sample_count: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<u16>, DecodeError> {
    if !strip.starts_with(&[0xff, 0xd8]) {
        return Err(camera_error(
            FORMAT,
            "Fujifilm proprietary compressed raw (no JPEG SOI marker) is not supported; \
             only standard lossless-JPEG (SOF3) compression 7 is decodable",
        ));
    }
    let image = lossless_jpeg::decode(strip, cancelled).map_err(|error| match error {
        LosslessJpegError::Cancelled { .. } => DecodeError::Cancelled,
        other => camera_error(FORMAT, format!("lossless JPEG: {other}")),
    })?;
    if image.component_ids.len() != 1 {
        return Err(camera_error(
            FORMAT,
            format!(
                "lossless JPEG has {} components, expected 1 for a CFA mosaic",
                image.component_ids.len()
            ),
        ));
    }
    if u64::from(image.width) != u64::from(width) || u64::from(image.height) != u64::from(height) {
        return Err(camera_error(
            FORMAT,
            format!(
                "lossless JPEG dimensions {}x{} do not match the raw IFD {}x{}",
                image.width, image.height, width, height
            ),
        ));
    }
    if image.samples.len() != sample_count {
        return Err(camera_error(
            FORMAT,
            format!(
                "lossless JPEG produced {} samples, expected {sample_count}",
                image.samples.len()
            ),
        ));
    }
    Ok(image.samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----- synthetic container builders ----------------------------------

    fn raf_header(jpeg: (u32, u32), raf_dir: (u32, u32), tiff: (u32, u32)) -> Vec<u8> {
        let mut header = vec![0_u8; 0x94];
        header[..16].copy_from_slice(RAF_MAGIC);
        header[0x10..0x14].copy_from_slice(b"0201");
        header[0x14..0x1c].copy_from_slice(b"FF109502");
        header[0x1c..0x20].copy_from_slice(b"X-A2");
        header[0x3c..0x40].copy_from_slice(b"0100");
        for (offset, value) in [
            (0x54, jpeg.0),
            (0x58, jpeg.1),
            (0x5c, raf_dir.0),
            (0x60, raf_dir.1),
            (0x64, tiff.0),
            (0x68, tiff.1),
        ] {
            header[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        }
        header
    }

    fn raf_directory(entries: &[(u16, &[u8])]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&u32::try_from(entries.len()).unwrap().to_be_bytes());
        for &(tag, value) in entries {
            bytes.extend_from_slice(&tag.to_be_bytes());
            bytes.extend_from_slice(&u16::try_from(value.len()).unwrap().to_be_bytes());
            bytes.extend_from_slice(value);
        }
        bytes
    }

    fn u16be(values: &[u16]) -> Vec<u8> {
        values.iter().flat_map(|value| value.to_be_bytes()).collect()
    }

    /// Builds a little-endian embedded TIFF whose first IFD only carries the
    /// 0xF000 pointer to the raw sub-IFD. `extra_sub_entries` are appended to
    /// the sub-IFD as (tag, type, count, inline-value-bytes) with 4-byte
    /// inline values; longer payloads go out of line automatically.
    // Test fixture builder: each parameter maps to one RAF/TIFF field, so
    // grouping them would only obscure the fixture layout.
    #[allow(clippy::too_many_arguments)]
    fn fuji_tiff(
        width: u32,
        height: u32,
        bits: u32,
        black: &[u32],
        wb_grb: Option<[u32; 3]>,
        compression: Option<u16>,
        cfa_tags: Option<&[u8]>,
        pixels: &[u8],
    ) -> Vec<u8> {
        // Sub-IFD entries: (tag, type, count, value bytes).
        let mut entries: Vec<(u16, u16, u32, Vec<u8>)> = vec![
            (0xf001, 4, 1, width.to_le_bytes().to_vec()),
            (0xf002, 4, 1, height.to_le_bytes().to_vec()),
            (0xf003, 4, 1, bits.to_le_bytes().to_vec()),
            // placeholder strip offset, patched below
            (0xf007, 4, 1, 0_u32.to_le_bytes().to_vec()),
            (
                0xf008,
                4,
                1,
                u32::try_from(pixels.len()).unwrap().to_le_bytes().to_vec(),
            ),
        ];
        if let Some(compression) = compression {
            entries.push((0x0103, 3, 1, {
                let mut value = compression.to_le_bytes().to_vec();
                value.extend_from_slice(&[0, 0]);
                value
            }));
        }
        if !black.is_empty() {
            entries.push((0xf00a, 4, u32::try_from(black.len()).unwrap(), {
                black.iter().flat_map(|value| value.to_le_bytes()).collect()
            }));
        }
        if let Some(wb) = wb_grb {
            entries.push((0xf00e, 4, 3, wb.iter().flat_map(|v| v.to_le_bytes()).collect()));
        }
        if let Some(cfa) = cfa_tags {
            entries.push((0x828d, 3, 2, vec![2, 0, 2, 0]));
            entries.push((0x828e, 1, 4, cfa.to_vec()));
        }
        entries.sort_by_key(|entry| entry.0);

        let ifd0_len = 2 + 12 + 4;
        let sub_offset = 8 + ifd0_len;
        let sub_table_len = 2 + 12 * entries.len() + 4;
        let mut out_of_line = Vec::new();
        let mut cursor = sub_offset + sub_table_len;
        // Reserve out-of-line space first to learn the pixel offset.
        for (_, _, _, value) in &entries {
            if value.len() > 4 {
                cursor += value.len();
                out_of_line.push(cursor - value.len());
            }
        }
        let pixel_offset = u32::try_from(cursor).unwrap();
        for entry in &mut entries {
            if entry.0 == 0xf007 {
                entry.3 = pixel_offset.to_le_bytes().to_vec();
            }
        }

        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II");
        tiff.extend_from_slice(&42_u16.to_le_bytes());
        tiff.extend_from_slice(&8_u32.to_le_bytes());
        // IFD0: single 0xF000 entry pointing at the sub-IFD.
        tiff.extend_from_slice(&1_u16.to_le_bytes());
        tiff.extend_from_slice(&0xf000_u16.to_le_bytes());
        tiff.extend_from_slice(&13_u16.to_le_bytes()); // IFD type
        tiff.extend_from_slice(&1_u32.to_le_bytes());
        tiff.extend_from_slice(&u32::try_from(sub_offset).unwrap().to_le_bytes());
        tiff.extend_from_slice(&0_u32.to_le_bytes());
        // Raw sub-IFD.
        tiff.extend_from_slice(&u16::try_from(entries.len()).unwrap().to_le_bytes());
        let mut out_of_line = out_of_line.iter();
        for (tag, field_type, count, value) in &entries {
            tiff.extend_from_slice(&tag.to_le_bytes());
            tiff.extend_from_slice(&field_type.to_le_bytes());
            tiff.extend_from_slice(&count.to_le_bytes());
            if value.len() <= 4 {
                let mut inline = value.clone();
                inline.resize(4, 0);
                tiff.extend_from_slice(&inline);
            } else {
                tiff.extend_from_slice(&u32::try_from(*out_of_line.next().unwrap()).unwrap().to_le_bytes());
            }
        }
        tiff.extend_from_slice(&0_u32.to_le_bytes());
        for (_, _, _, value) in &entries {
            if value.len() > 4 {
                tiff.extend_from_slice(value);
            }
        }
        assert_eq!(tiff.len(), pixel_offset as usize);
        tiff.extend_from_slice(pixels);
        tiff
    }

    fn pack_lsb(samples: &[u16], bits: u8) -> Vec<u8> {
        let mut bytes = vec![0_u8; (samples.len() * usize::from(bits)).div_ceil(8)];
        let mut bit_position = 0_usize;
        for &sample in samples {
            for shift in 0..bits {
                if (sample >> shift) & 1 == 1 {
                    bytes[bit_position / 8] |= 1 << (bit_position % 8);
                }
                bit_position += 1;
            }
        }
        bytes
    }

    struct SyntheticRaf {
        data: Vec<u8>,
        width: usize,
        height: usize,
    }

    /// Assembles a complete synthetic Bayer RAF around an uncompressed
    /// 12-bit strip.
    fn synthetic_bayer_raf() -> SyntheticRaf {
        let (width, height) = (8_usize, 4_usize);
        let samples: Vec<u16> = (0..width * height)
            .map(|index| u16::try_from((index * 37 + 11) & 0xfff).unwrap())
            .collect();
        let packed = pack_lsb(&samples, 12);
        let tiff = fuji_tiff(
            u32::try_from(width).unwrap(),
            u32::try_from(height).unwrap(),
            12,
            &[255, 256, 257, 258],
            Some([300, 600, 450]),
            None,
            None,
            &packed,
        );
        let dir = raf_directory(&[
            (
                0x100,
                &u16be(&[u16::try_from(height).unwrap(), u16::try_from(width).unwrap()]),
            ),
            (0x110, &u16be(&[12, 28])),
            (
                0x111,
                &u16be(&[u16::try_from(height).unwrap(), u16::try_from(width).unwrap()]),
            ),
            (0x130, &[0x0a, 0, 0, 0]),
        ]);
        let header = raf_header(
            (0, 0),
            (0x94, u32::try_from(dir.len()).unwrap()),
            (0x200, u32::try_from(tiff.len()).unwrap()),
        );
        let mut data = header;
        data.extend_from_slice(&dir);
        data.resize(0x200, 0);
        data.extend_from_slice(&tiff);
        SyntheticRaf { data, width, height }
    }

    // ----- tests ----------------------------------------------------------

    #[test]
    fn parses_header_selects_fuji_ifd_and_decodes_lsb_packed_bayer() {
        let SyntheticRaf { data, width, height } = synthetic_bayer_raf();
        let quirks = RafQuirks;
        let container = quirks.parse_container(&data).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let metadata = quirks.read_metadata(&container, raw).unwrap();
        assert_eq!(metadata.make, "FUJIFILM");
        assert_eq!(metadata.width as usize, width);
        assert_eq!(metadata.height as usize, height);
        assert_eq!(metadata.bits_per_sample, 12);
        assert_eq!(
            metadata.cfa.cells,
            vec![CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue]
        );
        assert_eq!(metadata.black_level.values, vec![255.0, 256.0, 257.0, 258.0]);
        assert_eq!(metadata.white_level.0, vec![4095.0]);
        let wb = metadata.white_balance;
        assert!((wb[0] - 2.0).abs() < 1e-6 && (wb[1] - 1.0).abs() < 1e-6 && (wb[2] - 1.5).abs() < 1e-6);

        let pixels = quirks.decode_pixels(&container, raw, &|| false).unwrap();
        let expected: Vec<u16> = (0..width * height)
            .map(|index| u16::try_from((index * 37 + 11) & 0xfff).unwrap())
            .collect();
        assert_eq!(pixels, expected);
    }

    #[test]
    fn rejects_xtrans_via_raf_directory_tag() {
        let mut bytes = raf_header((0, 0), (0x94, 44), (0x200, 16));
        let mut dir = raf_directory(&[(0x131, &[2_u8, 1, 1, 0, 1, 0][..])]);
        dir.resize(44, 0);
        bytes.extend_from_slice(&dir);
        bytes.resize(0x200, 0);
        let error = RafQuirks.parse_container(&bytes).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "RAF", ref message } if message.contains("X-Trans")),
            "expected typed X-Trans rejection, got {error:?}"
        );
    }

    #[test]
    fn rejects_xtrans_via_thirty_six_black_levels() {
        // No 0x131 in the RAF directory; the 6x6 black grid is the signal.
        let black = [255_u32; 36];
        let tiff = fuji_tiff(8, 4, 14, &black, None, None, None, &[0_u8; 8]);
        let dir = raf_directory(&[(0x130, &[0x0a, 0, 0, 0][..])]);
        let mut data = raf_header(
            (0, 0),
            (0x94, u32::try_from(dir.len()).unwrap()),
            (0x200, u32::try_from(tiff.len()).unwrap()),
        );
        data.extend_from_slice(&dir);
        data.resize(0x200, 0);
        data.extend_from_slice(&tiff);
        let quirks = RafQuirks;
        let container = quirks.parse_container(&data).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.read_metadata(&container, raw).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "RAF", ref message } if message.contains("X-Trans")),
            "expected typed X-Trans rejection, got {error:?}"
        );
    }

    #[test]
    fn rejects_non_2x2_cfa_repeat_pattern_dim() {
        let tiff = fuji_tiff(8, 4, 12, &[255], None, None, Some(&[0, 1, 1, 2]), &[]);
        // Rewrite the repeat-dim inline value to 6x6.
        let mut tiff = tiff;
        let needle = [0x8d, 0x82, 3, 0, 2, 0, 0, 0, 2, 0, 2, 0];
        let position = tiff.windows(needle.len()).position(|w| w == needle).unwrap();
        tiff[position + 8..position + 12].copy_from_slice(&[6, 0, 6, 0]);
        let dir = raf_directory(&[(0x130, &[0x0a, 0, 0, 0][..])]);
        let mut data = raf_header(
            (0, 0),
            (0x94, u32::try_from(dir.len()).unwrap()),
            (0x200, u32::try_from(tiff.len()).unwrap()),
        );
        data.extend_from_slice(&dir);
        data.resize(0x200, 0);
        data.extend_from_slice(&tiff);
        let quirks = RafQuirks;
        let container = quirks.parse_container(&data).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let error = quirks.read_metadata(&container, raw).unwrap_err();
        assert!(
            matches!(error, DecodeError::NativeCamera { format: "RAF", ref message } if message.contains("6x6") || message.contains("repeat pattern")),
            "expected typed non-Bayer rejection, got {error:?}"
        );
    }

    #[test]
    fn rejects_broken_headers_with_typed_errors() {
        // Bad magic.
        let mut bad = raf_header((0, 0), (0x94, 4), (0x200, 8));
        bad[0] = b'X';
        assert!(matches!(
            RafQuirks.parse_container(&bad),
            Err(DecodeError::NativeCamera { format: "RAF", .. })
        ));
        // Truncated header.
        assert!(matches!(
            RafQuirks.parse_container(&[0_u8; 0x20]),
            Err(DecodeError::NativeCamera { format: "RAF", ref message }) if message.contains("truncated")
        ));
        // Zero FujiIFD offset. A valid RAF directory is required here:
        // directory checks run before the FujiIFD checks.
        let dir = raf_directory(&[(0x130, &[0x0a, 0, 0, 0][..])]);
        let mut no_tiff = raf_header((0, 0), (0x94, u32::try_from(dir.len()).unwrap()), (0, 0));
        no_tiff.extend_from_slice(&dir);
        assert!(matches!(
            RafQuirks.parse_container(&pad_to(no_tiff, 0x200)),
            Err(DecodeError::NativeCamera { format: "RAF", ref message }) if message.contains("no embedded TIFF")
        ));
        // FujiIFD offset beyond EOF.
        let mut beyond = raf_header((0, 0), (0x94, u32::try_from(dir.len()).unwrap()), (0x8000, 8));
        beyond.extend_from_slice(&dir);
        assert!(matches!(
            RafQuirks.parse_container(&pad_to(beyond, 0x200)),
            Err(DecodeError::NativeCamera { format: "RAF", ref message }) if message.contains("beyond end of file")
        ));
        // Zero RAF-directory offset.
        let no_dir = raf_header((0, 0), (0, 0), (0x200, 8));
        assert!(matches!(
            RafQuirks.parse_container(&pad_to(no_dir, 0x200)),
            Err(DecodeError::NativeCamera { format: "RAF", ref message }) if message.contains("RAF directory offset is zero")
        ));
    }

    #[test]
    fn rejects_rotated_super_ccd_layout() {
        let mut bytes = raf_header((0, 0), (0x94, 8), (0x200, 16));
        bytes.extend_from_slice(&raf_directory(&[(0x130, &[0x8a, 0, 0, 0][..])]));
        bytes.resize(0x200, 0);
        assert!(matches!(
            RafQuirks.parse_container(&bytes),
            Err(DecodeError::NativeCamera { format: "RAF", ref message }) if message.contains("Super-CCD")
        ));
    }

    #[test]
    fn rejects_unsupported_compression_tag() {
        let packed = pack_lsb(&[0_u16; 32], 12);
        let tiff = fuji_tiff(8, 4, 12, &[255, 255, 255, 255], None, Some(5), None, &packed);
        let dir = raf_directory(&[(0x130, &[0x0a, 0, 0, 0][..])]);
        let mut data = raf_header(
            (0, 0),
            (0x94, u32::try_from(dir.len()).unwrap()),
            (0x200, u32::try_from(tiff.len()).unwrap()),
        );
        data.extend_from_slice(&dir);
        data.resize(0x200, 0);
        data.extend_from_slice(&tiff);
        let quirks = RafQuirks;
        let container = quirks.parse_container(&data).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        assert!(matches!(
            quirks.decode_pixels(&container, raw, &|| false),
            Err(DecodeError::NativeCamera { format: "RAF", ref message }) if message.contains("compression 5")
        ));
    }

    #[test]
    fn rejects_fuji_proprietary_compression_without_soi() {
        // Strip too small even for 12-bit unpacked data (8x4 px needs 48
        // bytes; the size heuristic then assumes compression 7) and not
        // starting with the FF D8 JPEG SOI marker.
        let garbage = vec![0x11_u8; 32];
        let tiff = fuji_tiff(8, 4, 14, &[255, 255, 255, 255], None, None, None, &garbage);
        let dir = raf_directory(&[(0x130, &[0x0a, 0, 0, 0][..])]);
        let mut data = raf_header(
            (0, 0),
            (0x94, u32::try_from(dir.len()).unwrap()),
            (0x200, u32::try_from(tiff.len()).unwrap()),
        );
        data.extend_from_slice(&dir);
        data.resize(0x200, 0);
        data.extend_from_slice(&tiff);
        let quirks = RafQuirks;
        let container = quirks.parse_container(&data).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        assert!(matches!(
            quirks.decode_pixels(&container, raw, &|| false),
            Err(DecodeError::NativeCamera { format: "RAF", ref message }) if message.contains("proprietary")
        ));
    }

    #[test]
    fn decodes_uncompressed_sixteen_bit_with_container_byte_order() {
        let samples: Vec<u16> = (0..24_u16).map(|index| index * 100).collect();
        let packed: Vec<u8> = samples.iter().flat_map(|value| value.to_le_bytes()).collect();
        let tiff = fuji_tiff(6, 4, 16, &[64], None, None, None, &packed);
        let dir = raf_directory(&[(0x130, &[0x0a, 0, 0, 0][..])]);
        let mut data = raf_header(
            (0, 0),
            (0x94, u32::try_from(dir.len()).unwrap()),
            (0x200, u32::try_from(tiff.len()).unwrap()),
        );
        data.extend_from_slice(&dir);
        data.resize(0x200, 0);
        data.extend_from_slice(&tiff);
        let quirks = RafQuirks;
        let container = quirks.parse_container(&data).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        let metadata = quirks.read_metadata(&container, raw).unwrap();
        assert_eq!(metadata.bits_per_sample, 16);
        assert_eq!(metadata.black_level.values, vec![64.0]);
        assert_eq!(quirks.decode_pixels(&container, raw, &|| false).unwrap(), samples);
    }

    #[test]
    fn honours_cancellation_per_row() {
        let SyntheticRaf { data, .. } = synthetic_bayer_raf();
        let quirks = RafQuirks;
        let container = quirks.parse_container(&data).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        assert!(matches!(
            quirks.decode_pixels(&container, raw, &|| true),
            Err(DecodeError::Cancelled)
        ));
    }

    #[test]
    fn rejects_cfa_pattern_with_non_rgb_colors() {
        // 2x2 dim but a four-color (e.g. RGBW) pattern: code 3 is not RGB.
        let packed = pack_lsb(&[0_u16; 32], 12);
        let tiff = fuji_tiff(
            8,
            4,
            12,
            &[255, 255, 255, 255],
            None,
            None,
            Some(&[0, 1, 1, 3]),
            &packed,
        );
        let dir = raf_directory(&[(0x130, &[0x0a, 0, 0, 0][..])]);
        let mut data = raf_header(
            (0, 0),
            (0x94, u32::try_from(dir.len()).unwrap()),
            (0x200, u32::try_from(tiff.len()).unwrap()),
        );
        data.extend_from_slice(&dir);
        data.resize(0x200, 0);
        data.extend_from_slice(&tiff);
        let quirks = RafQuirks;
        let container = quirks.parse_container(&data).unwrap();
        let raw = quirks.select_raw_ifd(&container).unwrap();
        assert!(matches!(
            quirks.read_metadata(&container, raw),
            Err(DecodeError::NativeCamera { format: "RAF", ref message }) if message.contains("not RGB")
        ));
    }

    fn pad_to(mut bytes: Vec<u8>, len: usize) -> Vec<u8> {
        bytes.resize(len, 0);
        bytes
    }
}
