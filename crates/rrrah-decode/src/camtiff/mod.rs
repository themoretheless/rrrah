//! Shared camera-TIFF core for the native camera RAW backends.
//!
//! This module is the foundation for the TIFF-family camera formats (Canon
//! CR2, Nikon NEF, Sony ARW, Olympus ORF, Pentax PEF, Panasonic RW2) plus the
//! Fujifilm RAF container. It reuses the clean-room TIFF/`BigTIFF` reader in
//! [`crate::dng::tiff`] and provides:
//!
//! - [`CameraFile`]: bounded parsing of a TIFF-family camera file into a
//!   directory list (top-level next-IFD chain plus one `SubIFDs` level);
//! - [`select_generic_raw_ifd`]: deterministic raw/CFA IFD selection that
//!   skips thumbnails and previews;
//! - the [`CameraQuirks`] trait: per-format hooks plugged into the shared
//!   decode flow in [`NativeCameraDecoder`];
//! - [`adapt_camera`]: the single place that turns decoded pixels plus
//!   [`CameraMetadata`] into an `rrrah_core::DecodedMosaic`;
//! - [`CameraFormat`]: the registration table mapping each camera format to
//!   its quirks implementation.
//!
//! DNG files do NOT go through this module; they keep the existing DNG
//! backend. The router ([`crate::native_router`]) decides that before this
//! code runs.
//!
//! # Stage 2 contract (per-format agents)
//!
//! Each camera format lives in exactly ONE file in this directory
//! (`camtiff/cr2.rs`, `camtiff/nef.rs`, `camtiff/arw.rs`, `camtiff/orf.rs`,
//! `camtiff/pef.rs`, `camtiff/rw2.rs`, `camtiff/raf.rs`). To implement a
//! format, replace that file's placeholder — and ONLY that file. **No edits
//! to shared files** (`camtiff/mod.rs`, `dng/*`, `lib.rs`, `native_router.rs`,
//! `bounded_io.rs`, `sniff.rs`, or any file outside `crates/rrrah-decode`).
//! If a shared helper is genuinely insufficient, stop and report instead of
//! patching around it.
//!
//! A per-format file must define one unit struct (e.g. `Cr2Quirks`)
//! implementing [`CameraQuirks`]:
//!
//! 1. `format_name()` — already provided by the placeholder; keep it.
//! 2. `read_metadata()` — REQUIRED. Build a [`CameraMetadata`] from the
//!    selected raw IFD ([`CameraDirectory`]) and the container
//!    ([`CameraFile`]). Read make/model, CFA pattern, black/white levels,
//!    white balance (`AsShotNeutral` or camera makernote), color matrix,
//!    orientation, and active/crop geometry from whichever tags the format
//!    uses. Use the `optional_*` / `required_*` helpers in this module or the
//!    [`Entry`] accessors from [`crate::dng::tiff`] directly.
//! 3. `decode_pixels()` — REQUIRED. Decode the raw CFA sample plane into a
//!    row-major `Vec<u16>` of exactly `width * height` samples. Reuse the
//!    shared decoders where the storage allows:
//!    `crate::dng::uncompressed::decode_msb_packed` for MSB-first packed rows
//!    and `crate::dng::lossless_jpeg::decode` for lossless-JPEG segments
//!    (TIFF compression 7). Uncompressed 8/16-bit rows can be read with the
//!    container's [`CameraFile::byte_order`]. Honor the `cancelled` callback
//!    at least once per row/strip/tile and return [`DecodeError::Cancelled`]
//!    when it fires.
//! 4. `select_raw_ifd()` — OPTIONAL override. The default
//!    [`select_generic_raw_ifd`] prefers a `SubFileType` 0 directory with CFA
//!    photometric (32803), then the largest primary image. Override when the
//!    format needs special handling — e.g. CR2 stores the raw-IFD offset as
//!    a `u32` at file offset 16 in the CR2 header; parse it from
//!    [`CameraFile::data`] and select that directory (parse it with
//!    [`CameraFile::parse_ifd_at`]).
//! 5. `parse_container()` — OPTIONAL override. The default parses the bytes
//!    as TIFF/`BigTIFF` starting at offset 0. Override for non-TIFF
//!    containers: RAF must parse the `FUJIFILMCCD-RAW ` header and hand the
//!    embedded TIFF payload to [`CameraFile::parse_tiff`].
//!
//! Error rules (project policy, see `docs/DECODE_FORMAT_AUDIT.md`):
//!
//! - Every failure is a typed [`DecodeError`]: use [`camera_error`] with your
//!   `format_name()` for format problems, [`DecodeError::Cancelled`] for
//!   cancellation. Unsupported compressions, bit depths, CFA layouts, or
//!   makernote variants must produce explicit errors — never silent
//!   degradation, never the embedded JPEG.
//! - All offsets, counts, and sizes read from the file go through checked
//!   arithmetic (`checked_mul`, `checked_add`, `usize::try_from`) before any
//!   slice indexing or allocation.
//! - Do not add external dependencies; std plus what the crate already has.
//! - The semantic recipe for your format is [`CameraFormat::recipe`]; bump
//!   the backend contract revision there requires editing `camtiff/mod.rs`,
//!   so instead note any recipe-relevant change in your file's documentation
//!   and flag it in your report — the orchestrator owns shared files.

mod arw;
mod cr2;
#[cfg(test)]
mod fixture_regression;
mod nef;
mod orf;
mod pef;
mod raf;
mod rw2;

use std::{
    collections::{HashSet, VecDeque},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
    time::Instant,
};

use rrrah_core::{
    CfaPattern, DECODE_CROP_AS_METADATA, DECODE_FULL_SENSOR_RAW, DECODE_IMAGE_INDEX_IN_KEY,
    DECODE_INTEGER_U16, DECODE_SENSOR_COORDINATES, DecodedMosaic, LevelGrid, MosaicRecipeManifest,
    Orientation, Photometric, RawMetadata, Rect, WhiteLevel,
};

use crate::{
    AdaptTimings, DecodeError, DecodeOutput, DecodeRequest, DecodeTimings, RawDecoder,
    bounded_io::read_bounded,
    dng::tiff::{ByteOrder, Entry, Ifd, Limits as TiffLimits, Tiff},
};

/// TIFF tag numbers shared by the camera backends. Formats may read any
/// additional tag (including makernotes) through [`CameraDirectory::entry`].
#[allow(dead_code)] // Many constants are only read by Stage 2 per-format modules.
pub(crate) mod tags {
    pub(crate) const NEW_SUBFILE_TYPE: u16 = 254;
    pub(crate) const IMAGE_WIDTH: u16 = 256;
    pub(crate) const IMAGE_LENGTH: u16 = 257;
    pub(crate) const BITS_PER_SAMPLE: u16 = 258;
    pub(crate) const COMPRESSION: u16 = 259;
    pub(crate) const PHOTOMETRIC_INTERPRETATION: u16 = 262;
    pub(crate) const MAKE: u16 = 271;
    pub(crate) const MODEL: u16 = 272;
    pub(crate) const STRIP_OFFSETS: u16 = 273;
    pub(crate) const ORIENTATION: u16 = 274;
    pub(crate) const SAMPLES_PER_PIXEL: u16 = 277;
    pub(crate) const ROWS_PER_STRIP: u16 = 278;
    pub(crate) const STRIP_BYTE_COUNTS: u16 = 279;
    pub(crate) const TILE_WIDTH: u16 = 322;
    pub(crate) const TILE_LENGTH: u16 = 323;
    pub(crate) const TILE_OFFSETS: u16 = 324;
    pub(crate) const TILE_BYTE_COUNTS: u16 = 325;
    pub(crate) const SUB_IFDS: u16 = 330;
    pub(crate) const CFA_REPEAT_PATTERN_DIM: u16 = 33_421;
    pub(crate) const CFA_PATTERN: u16 = 33_422;

    /// `PhotometricInterpretation` value for a color-filter-array image.
    pub(crate) const PHOTOMETRIC_CFA: u64 = 32_803;
    /// TIFF `Compression` value for uncompressed storage.
    pub(crate) const COMPRESSION_UNCOMPRESSED: u64 = 1;
    /// TIFF `Compression` value for lossless JPEG (new-style).
    pub(crate) const COMPRESSION_LOSSLESS_JPEG: u64 = 7;
}

const MAX_DIRECTORIES: usize = 256;
const MAX_IMAGE_AREA: u64 = 512 * 1024 * 1024;

/// Builds a typed camera-backend error. This is the ONLY error constructor
/// per-format code should need besides [`DecodeError::Cancelled`].
pub(crate) fn camera_error(format: &'static str, message: impl Into<String>) -> DecodeError {
    DecodeError::NativeCamera {
        format,
        message: message.into(),
    }
}

/// Typed "not yet implemented" error. Stage 2 implemented every registered
/// format, so this only survives as the fallback default of the
/// [`CameraQuirks`] hooks for formats that have not overridden them.
pub(crate) fn not_implemented(format: &'static str, stage: &'static str) -> DecodeError {
    camera_error(format, format!("{stage} is not yet implemented for {format}"))
}

/// One parsed image file directory of a camera TIFF.
#[derive(Debug)]
pub(crate) struct CameraDirectory<'a> {
    ifd: Ifd<'a>,
    /// True when reached through the top-level next-IFD chain, false when
    /// reached through a `SubIFDs` (330) entry of a top-level directory.
    top_level: bool,
}

impl<'a> CameraDirectory<'a> {
    /// Looks up a tag, mapping parse failures into the camera error type.
    pub(crate) fn entry(&self, format: &'static str, tag: u16) -> Result<Option<&Entry<'a>>, DecodeError> {
        self.ifd
            .entry(tag)
            .map_err(|error| camera_error(format, format!("tag {tag}: {error}")))
    }

    /// The absolute file offset of this directory.
    #[allow(dead_code)] // Used by Stage 2 per-format modules.
    pub(crate) const fn offset(&self) -> u64 {
        self.ifd.offset
    }

    /// Whether this directory sits on the top-level next-IFD chain.
    #[allow(dead_code)] // Used by Stage 2 per-format modules.
    pub(crate) const fn is_top_level(&self) -> bool {
        self.top_level
    }
}

/// A parsed TIFF-family camera file: container header plus all reachable
/// directories (top-level chain and one `SubIFDs` level).
#[derive(Debug)]
pub(crate) struct CameraFile<'a> {
    data: &'a [u8],
    tiff: Tiff<'a>,
    directories: Vec<CameraDirectory<'a>>,
}

impl<'a> CameraFile<'a> {
    /// Parses `data` as TIFF/`BigTIFF` and walks the directory graph with
    /// cycle and count limits. `format` is the quirks `format_name()` used
    /// for typed errors.
    pub(crate) fn parse_tiff(format: &'static str, data: &'a [u8]) -> Result<Self, DecodeError> {
        let tiff = Tiff::parse(data, TiffLimits::default())
            .map_err(|error| camera_error(format, format!("TIFF header: {error}")))?;
        let mut directories = Vec::new();
        let mut seen = HashSet::new();
        let mut queue = VecDeque::from([(tiff.first_ifd_offset(), true)]);
        while let Some((offset, top_level)) = queue.pop_front() {
            if !seen.insert(offset) {
                return Err(camera_error(
                    format,
                    format!("directory graph cycles at offset {offset}"),
                ));
            }
            if directories.len() >= MAX_DIRECTORIES {
                return Err(camera_error(
                    format,
                    format!("directory count exceeds limit {MAX_DIRECTORIES}"),
                ));
            }
            let ifd = tiff
                .parse_ifd(offset)
                .map_err(|error| camera_error(format, format!("IFD at offset {offset}: {error}")))?;
            if top_level && ifd.next_ifd_offset != 0 {
                queue.push_back((ifd.next_ifd_offset, true));
            }
            if let Some(sub) = ifd
                .entry(tags::SUB_IFDS)
                .map_err(|error| camera_error(format, format!("SubIFDs tag: {error}")))?
            {
                for child in sub
                    .unsigned_values()
                    .map_err(|error| camera_error(format, format!("SubIFDs tag: {error}")))?
                {
                    if child == 0 {
                        return Err(camera_error(format, "zero SubIFD offset"));
                    }
                    queue.push_back((child, false));
                }
            }
            directories.push(CameraDirectory { ifd, top_level });
        }
        if directories.is_empty() {
            return Err(camera_error(format, "file contains no image directories"));
        }
        Ok(Self {
            data,
            tiff,
            directories,
        })
    }

    /// The complete bounded source bytes (for fixed-offset headers such as
    /// the CR2 raw-IFD pointer at file offset 16).
    #[allow(dead_code)] // Used by Stage 2 per-format modules.
    pub(crate) const fn data(&self) -> &'a [u8] {
        self.data
    }

    /// Container byte order for manual tag or pixel unpacking.
    #[allow(dead_code)] // Used by Stage 2 per-format modules.
    pub(crate) const fn byte_order(&self) -> ByteOrder {
        self.tiff.byte_order()
    }

    /// Every directory reachable from the header, top-level chain first.
    pub(crate) fn directories(&self) -> &[CameraDirectory<'a>] {
        &self.directories
    }

    /// Parses an additional IFD at an arbitrary file offset (e.g. the CR2
    /// raw-IFD pointer stored outside the directory chain).
    #[allow(dead_code)] // Used by Stage 2 per-format modules (CR2).
    pub(crate) fn parse_ifd_at(&self, format: &'static str, offset: u64) -> Result<Ifd<'a>, DecodeError> {
        self.tiff
            .parse_ifd(offset)
            .map_err(|error| camera_error(format, format!("IFD at offset {offset}: {error}")))
    }
}

/// Deterministic raw/CFA IFD selection shared by the camera backends.
///
/// Scores every directory that has image dimensions and pixel storage
/// (strip or tile offsets) by `(has CFA photometric, is primary SubFileType,
/// pixel area)` and takes the strict maximum in walk order, which skips
/// thumbnails and previews and stays deterministic on ties.
pub(crate) fn select_generic_raw_ifd<'a>(
    file: &'a CameraFile<'a>,
    format: &'static str,
) -> Result<&'a CameraDirectory<'a>, DecodeError> {
    let mut best: Option<(&CameraDirectory<'a>, (u8, u8, u64))> = None;
    for directory in file.directories() {
        let has_storage = directory.entry(format, tags::STRIP_OFFSETS)?.is_some()
            || directory.entry(format, tags::TILE_OFFSETS)?.is_some();
        if !has_storage {
            continue;
        }
        let (Some(width), Some(height)) = (
            optional_scalar(format, directory, tags::IMAGE_WIDTH)?,
            optional_scalar(format, directory, tags::IMAGE_LENGTH)?,
        ) else {
            continue;
        };
        let area = width
            .checked_mul(height)
            .ok_or_else(|| camera_error(format, "arithmetic overflow computing raw IFD area"))?;
        if area == 0 || area > MAX_IMAGE_AREA {
            continue;
        }
        let cfa = u8::from(
            optional_scalar(format, directory, tags::PHOTOMETRIC_INTERPRETATION)?
                == Some(tags::PHOTOMETRIC_CFA),
        );
        let primary = u8::from(optional_scalar(format, directory, tags::NEW_SUBFILE_TYPE)?.unwrap_or(0) == 0);
        let score = (cfa, primary, area);
        if best.is_none_or(|(_, best_score)| score > best_score) {
            best = Some((directory, score));
        }
    }
    best.map(|(directory, _)| directory)
        .ok_or_else(|| camera_error(format, "no raw image directory found"))
}

/// Reads an optional single unsigned tag value.
pub(crate) fn optional_scalar(
    format: &'static str,
    directory: &CameraDirectory<'_>,
    tag: u16,
) -> Result<Option<u64>, DecodeError> {
    directory
        .entry(format, tag)?
        .map(|entry| {
            entry
                .unsigned_scalar()
                .map_err(|error| camera_error(format, format!("tag {tag}: {error}")))
        })
        .transpose()
}

/// Reads a required single unsigned tag value.
#[allow(dead_code)] // Used by Stage 2 per-format modules.
pub(crate) fn required_scalar(
    format: &'static str,
    directory: &CameraDirectory<'_>,
    tag: u16,
) -> Result<u64, DecodeError> {
    optional_scalar(format, directory, tag)?
        .ok_or_else(|| camera_error(format, format!("required tag {tag} is missing")))
}

/// Reads an optional ASCII tag, trimmed; empty values become `None`.
#[allow(dead_code)] // Used by Stage 2 per-format modules.
pub(crate) fn optional_ascii(
    format: &'static str,
    directory: &CameraDirectory<'_>,
    tag: u16,
) -> Result<Option<String>, DecodeError> {
    directory
        .entry(format, tag)?
        .map(|entry| {
            entry
                .ascii()
                .map(|value| value.trim().to_owned())
                .map_err(|error| camera_error(format, format!("tag {tag}: {error}")))
        })
        .transpose()
        .map(|value| value.filter(|text| !text.is_empty()))
}

/// Maps a TIFF Orientation (1–8) value to the domain orientation.
#[allow(dead_code)] // Used by Stage 2 per-format modules.
pub(crate) fn orientation_from_tag(format: &'static str, value: u64) -> Result<Orientation, DecodeError> {
    match value {
        1 => Ok(Orientation::Normal),
        2 => Ok(Orientation::HorizontalFlip),
        3 => Ok(Orientation::Rotate180),
        4 => Ok(Orientation::VerticalFlip),
        5 => Ok(Orientation::Transpose),
        6 => Ok(Orientation::Rotate90),
        7 => Ok(Orientation::Transverse),
        8 => Ok(Orientation::Rotate270),
        actual => Err(camera_error(format, format!("invalid orientation {actual}"))),
    }
}

/// Everything [`adapt_camera`] needs to build the domain mosaic. Per-format
/// `read_metadata` implementations produce this from their tags.
#[derive(Debug, Clone)]
pub(crate) struct CameraMetadata {
    pub(crate) make: String,
    pub(crate) model: String,
    /// Full stored mosaic width in samples.
    pub(crate) width: u32,
    /// Full stored mosaic height in samples.
    pub(crate) height: u32,
    /// Output bit depth of the decoded samples (after any linearization).
    pub(crate) bits_per_sample: u8,
    pub(crate) cfa: CfaPattern,
    pub(crate) black_level: LevelGrid,
    pub(crate) white_level: WhiteLevel,
    pub(crate) white_balance: [f32; 4],
    pub(crate) xyz_to_camera: [[f32; 3]; 4],
    pub(crate) active_area: Option<Rect>,
    pub(crate) crop_area: Option<Rect>,
    pub(crate) orientation: Orientation,
}

/// Adapts decoded camera pixels to the domain mosaic, mirroring the DNG
/// backend's adaptation. Validates that the pixel count matches the declared
/// geometry exactly; anything else is a typed format error.
pub(crate) fn adapt_camera(
    format: &'static str,
    metadata: CameraMetadata,
    pixels: Vec<u16>,
) -> Result<(DecodedMosaic, AdaptTimings), DecodeError> {
    let total_started = Instant::now();
    let expected = usize::try_from(metadata.width)
        .ok()
        .and_then(|width| {
            usize::try_from(metadata.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or(DecodeError::DimensionOverflow)?;
    if pixels.len() != expected {
        return Err(camera_error(
            format,
            format!(
                "decoded {} samples, expected {} ({}x{})",
                pixels.len(),
                expected,
                metadata.width,
                metadata.height
            ),
        ));
    }

    let layout_started = Instant::now();
    let cfa = Some(metadata.cfa);
    let layout_cfa = layout_started.elapsed();
    let levels_started = Instant::now();
    let black_level = metadata.black_level;
    let white_level = metadata.white_level;
    let levels = levels_started.elapsed();
    let color_started = Instant::now();
    let white_balance = metadata.white_balance;
    let xyz_to_camera = metadata.xyz_to_camera;
    let color = color_started.elapsed();
    let geometry_started = Instant::now();
    let active_area = metadata.active_area;
    let crop_area = metadata.crop_area;
    let orientation = metadata.orientation;
    let geometry = geometry_started.elapsed();

    let finalize_started = Instant::now();
    let raw_metadata = RawMetadata {
        make: metadata.make,
        model: metadata.model,
        width: metadata.width,
        height: metadata.height,
        components_per_pixel: 1,
        bits_per_sample: metadata.bits_per_sample,
        photometric: Photometric::Cfa,
        cfa,
        black_level,
        white_level,
        white_balance,
        xyz_to_camera,
        active_area,
        crop_area,
        orientation,
    };
    let mosaic = DecodedMosaic::new(raw_metadata, Arc::new(pixels))?;
    let finalize = finalize_started.elapsed();
    let total = total_started.elapsed();

    Ok((
        mosaic,
        AdaptTimings {
            layout_cfa,
            levels,
            color,
            geometry,
            finalize,
            total,
        },
    ))
}

/// Per-format hooks plugged into the shared camera decode flow. See the
/// module-level Stage 2 contract for what each method must do.
pub(crate) trait CameraQuirks: Send + Sync {
    /// Short upper-case format name used in typed errors ("CR2", "NEF", ...).
    fn format_name(&self) -> &'static str;

    /// Parses the container. Default: TIFF/`BigTIFF` at offset 0.
    fn parse_container<'a>(&self, data: &'a [u8]) -> Result<CameraFile<'a>, DecodeError> {
        CameraFile::parse_tiff(self.format_name(), data)
    }

    /// Selects the raw IFD. Default: [`select_generic_raw_ifd`].
    fn select_raw_ifd<'a>(
        &self,
        container: &'a CameraFile<'a>,
    ) -> Result<&'a CameraDirectory<'a>, DecodeError> {
        select_generic_raw_ifd(container, self.format_name())
    }

    /// Extracts adaptation metadata from the selected raw IFD. Stage 2 must
    /// implement this; the placeholder default is a typed not-implemented
    /// error.
    fn read_metadata(
        &self,
        container: &CameraFile<'_>,
        raw: &CameraDirectory<'_>,
    ) -> Result<CameraMetadata, DecodeError> {
        let _ = (container, raw);
        Err(not_implemented(self.format_name(), "metadata extraction"))
    }

    /// Decodes the CFA sample plane. Stage 2 must implement this; the
    /// placeholder default is a typed not-implemented error.
    fn decode_pixels(
        &self,
        container: &CameraFile<'_>,
        raw: &CameraDirectory<'_>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<u16>, DecodeError> {
        let _ = (container, raw, cancelled);
        Err(not_implemented(self.format_name(), "pixel storage decode"))
    }
}

/// Camera formats registered with the shared backend. Backend IDs 2 (CR3)
/// and 3 (DNG) are taken by the existing backends; camera formats use 4..=10.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CameraFormat {
    Cr2,
    Nef,
    Arw,
    Orf,
    Pef,
    Rw2,
    Raf,
}

impl CameraFormat {
    /// Unique cache-facing backend ID for this format's mosaic recipe.
    pub(crate) const fn backend_id(self) -> u32 {
        match self {
            Self::Cr2 => 4,
            Self::Nef => 5,
            Self::Arw => 6,
            Self::Orf => 7,
            Self::Pef => 8,
            Self::Rw2 => 9,
            Self::Raf => 10,
        }
    }

    /// The registered quirks implementation for this format.
    pub(crate) fn quirks(self) -> &'static dyn CameraQuirks {
        match self {
            Self::Cr2 => &cr2::Cr2Quirks,
            Self::Nef => &nef::NefQuirks,
            Self::Arw => &arw::ArwQuirks,
            Self::Orf => &orf::OrfQuirks,
            Self::Pef => &pef::PefQuirks,
            Self::Rw2 => &rw2::Rw2Quirks,
            Self::Raf => &raf::RafQuirks,
        }
    }

    /// Semantic cache contract for this format. Revisions are 2/1/1 for most
    /// formats, 3/1/1 for NEF and PEF: the backend contract revision was
    /// bumped from 1 to 2 when Stage 2 replaced the typed placeholder
    /// decoders with real per-format decoding, and from 2 to 3 for NEF and
    /// PEF when they gained native decoding of compressions 34713 (Nikon
    /// lossless) and 65535 (Pentax lossless), so mosaics cached under the
    /// older contracts cannot be reused.
    pub(crate) fn recipe(self) -> MosaicRecipeManifest {
        const NATIVE_CAMERA_DECODE_FLAGS: u32 = DECODE_FULL_SENSOR_RAW
            | DECODE_INTEGER_U16
            | DECODE_SENSOR_COORDINATES
            | DECODE_CROP_AS_METADATA
            | DECODE_IMAGE_INDEX_IN_KEY;
        let backend_revision = match self {
            Self::Nef | Self::Pef => 3,
            Self::Cr2 | Self::Arw | Self::Orf | Self::Rw2 | Self::Raf => 2,
        };
        MosaicRecipeManifest::new(
            self.backend_id(),
            backend_revision,
            1,
            1,
            NATIVE_CAMERA_DECODE_FLAGS,
            crate::WORKSPACE_LOCK_DIGEST,
        )
    }
}

/// Shared production backend for every registered camera format.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NativeCameraDecoder {
    format: CameraFormat,
}

impl NativeCameraDecoder {
    pub(crate) const fn new(format: CameraFormat) -> Self {
        Self { format }
    }
}

impl RawDecoder for NativeCameraDecoder {
    fn mosaic_recipe(&self, _request: &DecodeRequest) -> Result<MosaicRecipeManifest, DecodeError> {
        Ok(self.format.recipe())
    }

    fn decode(&self, request: &DecodeRequest) -> Result<DecodeOutput, DecodeError> {
        let total_started = Instant::now();
        request.check_cancelled()?;
        if request.image_index != 0 {
            return Err(DecodeError::UnsupportedImageIndex {
                index: request.image_index,
            });
        }

        let source_started = Instant::now();
        let data = read_bounded(request)?;
        let source_open = source_started.elapsed();
        request.check_cancelled()?;

        let quirks = self.format.quirks();
        let decoder_select_started = Instant::now();
        let container = catch_unwind(AssertUnwindSafe(|| quirks.parse_container(&data)))
            .map_err(|_| DecodeError::DecoderPanicked)??;
        let raw = catch_unwind(AssertUnwindSafe(|| quirks.select_raw_ifd(&container)))
            .map_err(|_| DecodeError::DecoderPanicked)??;
        let decoder_select = decoder_select_started.elapsed();
        request.check_cancelled()?;

        let cancelled = || {
            request
                .cancellation
                .as_ref()
                .is_some_and(crate::GenerationToken::is_cancelled)
        };
        let raw_image_started = Instant::now();
        let metadata = catch_unwind(AssertUnwindSafe(|| quirks.read_metadata(&container, raw)))
            .map_err(|_| DecodeError::DecoderPanicked)??;
        let pixels = catch_unwind(AssertUnwindSafe(|| {
            quirks.decode_pixels(&container, raw, &cancelled)
        }))
        .map_err(|_| DecodeError::DecoderPanicked)??;
        drop(data);
        let raw_image = raw_image_started.elapsed();
        request.check_cancelled()?;

        let (mosaic, adapt) = adapt_camera(quirks.format_name(), metadata, pixels)?;
        let adapt_metadata = adapt.total;
        let raw_decode = decoder_select.saturating_add(raw_image);
        request.check_cancelled()?;

        Ok(DecodeOutput {
            mosaic,
            timings: DecodeTimings {
                source_open,
                decoder_select,
                raw_image,
                raw_decode,
                native: None,
                dng: None,
                adapt,
                adapt_metadata,
                total: total_started.elapsed(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a minimal little-endian TIFF with two directories: a small
    /// reduced-resolution IFD0 (`SubFileType` 1) and a large CFA IFD1.
    fn synthetic_camera_tiff() -> Vec<u8> {
        fn write_ifd(bytes: &mut [u8], at: usize, entries: &[(u16, u32)], next: u32) {
            let count = u16::try_from(entries.len()).unwrap();
            bytes[at..at + 2].copy_from_slice(&count.to_le_bytes());
            for (index, &(tag, value)) in entries.iter().enumerate() {
                let start = at + 2 + index * 12;
                bytes[start..start + 2].copy_from_slice(&tag.to_le_bytes());
                bytes[start + 2..start + 4].copy_from_slice(&3_u16.to_le_bytes()); // SHORT
                bytes[start + 4..start + 8].copy_from_slice(&1_u32.to_le_bytes()); // count
                bytes[start + 8..start + 12].copy_from_slice(&value.to_le_bytes());
            }
            let next_at = at + 2 + entries.len() * 12;
            bytes[next_at..next_at + 4].copy_from_slice(&next.to_le_bytes());
        }

        let mut bytes = vec![0_u8; 512];
        bytes[..8].copy_from_slice(&[b'I', b'I', 42, 0, 8, 0, 0, 0]);
        // IFD0 at 8: 64x48 preview, SubFileType 1, strip storage.
        write_ifd(
            &mut bytes,
            8,
            &[(254, 1), (256, 64), (257, 48), (262, 1), (273, 400), (279, 6_144)],
            200,
        );
        // IFD1 at 200: 6000x4000 CFA raw, SubFileType 0, strip storage.
        write_ifd(
            &mut bytes,
            200,
            &[
                (254, 0),
                (256, 6_000),
                (257, 4_000),
                (262, 32_803),
                (273, 430),
                (279, 48_000),
            ],
            0,
        );
        bytes
    }

    #[test]
    fn generic_selection_prefers_the_large_cfa_directory() {
        let bytes = synthetic_camera_tiff();
        let file = CameraFile::parse_tiff("TEST", &bytes).unwrap();
        assert_eq!(file.directories().len(), 2);
        let raw = select_generic_raw_ifd(&file, "TEST").unwrap();
        assert_eq!(raw.offset(), 200);
    }

    #[test]
    fn parse_rejects_directory_cycles() {
        let mut bytes = vec![0_u8; 64];
        bytes[..8].copy_from_slice(&[b'I', b'I', 42, 0, 8, 0, 0, 0]);
        // IFD at 8 with zero entries whose next-IFD offset points at itself.
        bytes[8..10].copy_from_slice(&0_u16.to_le_bytes());
        bytes[10..14].copy_from_slice(&8_u32.to_le_bytes());
        let error = CameraFile::parse_tiff("TEST", &bytes).unwrap_err();
        assert!(matches!(error, DecodeError::NativeCamera { format: "TEST", .. }));
    }

    #[test]
    fn implemented_quirks_fail_typed_without_not_implemented() {
        // Stage 2 replaced every placeholder with a real implementation. On a
        // generic synthetic camera TIFF the per-format hooks are allowed to
        // fail (missing format-specific tags, truncated storage) — but every
        // failure must stay a typed `NativeCamera` error carrying the format
        // name, and none may be the retired "not yet implemented" text.
        let bytes = synthetic_camera_tiff();
        let file = CameraFile::parse_tiff("TEST", &bytes).unwrap();
        let raw = select_generic_raw_ifd(&file, "TEST").unwrap();
        for format in [
            CameraFormat::Cr2,
            CameraFormat::Nef,
            CameraFormat::Arw,
            CameraFormat::Orf,
            CameraFormat::Pef,
            CameraFormat::Rw2,
            CameraFormat::Raf,
        ] {
            let quirks = format.quirks();
            let outcomes = [
                quirks.read_metadata(&file, raw).map(drop),
                quirks.decode_pixels(&file, raw, &|| false).map(drop),
            ];
            for outcome in outcomes {
                match outcome {
                    Ok(()) => {}
                    Err(DecodeError::NativeCamera {
                        format: name,
                        message,
                    }) => {
                        assert_eq!(
                            name,
                            quirks.format_name(),
                            "{format:?} errors must carry their own format name"
                        );
                        assert!(
                            !message.contains("not yet implemented"),
                            "{format:?} is implemented and must not report the placeholder error: {message}"
                        );
                    }
                    Err(other) => panic!("{format:?} must fail typed, got {other:?}"),
                }
            }
        }
    }

    #[test]
    fn camera_recipes_use_the_expected_backend_revisions() {
        // Canonical manifest layout: backend id at bytes 8..12, backend
        // contract revision at 12..16, adapter revision at 16..20, mosaic
        // model revision at 20..24 (see rrrah_core::MosaicRecipeManifest).
        for format in [
            CameraFormat::Cr2,
            CameraFormat::Nef,
            CameraFormat::Arw,
            CameraFormat::Orf,
            CameraFormat::Pef,
            CameraFormat::Rw2,
            CameraFormat::Raf,
        ] {
            let bytes = format.recipe().canonical_bytes();
            assert_eq!(
                &bytes[8..12],
                &format.backend_id().to_le_bytes(),
                "{format:?} backend id"
            );
            let expected_backend_revision: u32 = match format {
                // NEF 34713 and PEF 65535 native decoding bumped the backend
                // contract revision from 2 to 3.
                CameraFormat::Nef | CameraFormat::Pef => 3,
                _ => 2,
            };
            assert_eq!(
                &bytes[12..16],
                &expected_backend_revision.to_le_bytes(),
                "{format:?} backend contract revision (2 = real decoding replaced the \
                 placeholder; 3 = NEF/PEF native compression decoding)"
            );
            assert_eq!(
                &bytes[16..20],
                &1_u32.to_le_bytes(),
                "{format:?} adapter revision"
            );
            assert_eq!(
                &bytes[20..24],
                &1_u32.to_le_bytes(),
                "{format:?} mosaic model revision"
            );
        }
    }

    #[test]
    fn camera_recipes_are_distinct_from_existing_backends() {
        let mut ids = std::collections::HashSet::new();
        for format in [
            CameraFormat::Cr2,
            CameraFormat::Nef,
            CameraFormat::Arw,
            CameraFormat::Orf,
            CameraFormat::Pef,
            CameraFormat::Rw2,
            CameraFormat::Raf,
        ] {
            assert!(ids.insert(format.recipe()), "{format:?} recipe must be unique");
            assert_ne!(format.recipe(), crate::NATIVE_EOS_R8_MOSAIC_CONTRACT_1);
            assert_ne!(format.recipe(), crate::NATIVE_DNG_MOSAIC_CONTRACT_1);
        }
    }
}
