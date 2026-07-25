//! Clean-room native Digital Negative (DNG) parsing and CFA decode.
//!
//! This product includes DNG technology under license by Adobe.
//!
//! The implementation follows the public Adobe DNG 1.7.1.0 and TIFF 6.0
//! specifications. It does not use source code from `Rawler`, `LibRaw`,
//! `RawSpeed`, `ExifTool`, or the Adobe DNG SDK.

#[cfg(test)]
mod fixture_regression;
pub(crate) mod lossless_jpeg;
mod lossless_storage;
mod parallel;
pub(crate) mod tiff;
pub(crate) mod uncompressed;

use std::{
    collections::{HashSet, VecDeque},
    error::Error,
    fmt,
    time::{Duration, Instant},
};

use lossless_jpeg::LosslessJpegError;
use tiff::{ByteOrder, Entry, FieldType, Ifd, Limits as TiffLimits, Tiff, TiffError};

const TAG_NEW_SUBFILE_TYPE: u16 = 254;
const TAG_IMAGE_WIDTH: u16 = 256;
const TAG_IMAGE_LENGTH: u16 = 257;
const TAG_BITS_PER_SAMPLE: u16 = 258;
const TAG_COMPRESSION: u16 = 259;
const TAG_PHOTOMETRIC_INTERPRETATION: u16 = 262;
const TAG_FILL_ORDER: u16 = 266;
const TAG_MAKE: u16 = 271;
const TAG_MODEL: u16 = 272;
const TAG_STRIP_OFFSETS: u16 = 273;
const TAG_ORIENTATION: u16 = 274;
const TAG_SAMPLES_PER_PIXEL: u16 = 277;
const TAG_ROWS_PER_STRIP: u16 = 278;
const TAG_STRIP_BYTE_COUNTS: u16 = 279;
const TAG_PLANAR_CONFIGURATION: u16 = 284;
const TAG_PREDICTOR: u16 = 317;
const TAG_TILE_WIDTH: u16 = 322;
const TAG_TILE_LENGTH: u16 = 323;
const TAG_TILE_OFFSETS: u16 = 324;
const TAG_TILE_BYTE_COUNTS: u16 = 325;
const TAG_SUB_IFDS: u16 = 330;
const TAG_CFA_REPEAT_PATTERN_DIM: u16 = 33_421;
const TAG_CFA_PATTERN: u16 = 33_422;
const TAG_DNG_VERSION: u16 = 50_706;
const TAG_DNG_BACKWARD_VERSION: u16 = 50_707;
const TAG_UNIQUE_CAMERA_MODEL: u16 = 50_708;
const TAG_CFA_PLANE_COLOR: u16 = 50_710;
const TAG_CFA_LAYOUT: u16 = 50_711;
const TAG_LINEARIZATION_TABLE: u16 = 50_712;
const TAG_BLACK_LEVEL_REPEAT_DIM: u16 = 50_713;
const TAG_BLACK_LEVEL: u16 = 50_714;
const TAG_BLACK_LEVEL_DELTA_H: u16 = 50_715;
const TAG_BLACK_LEVEL_DELTA_V: u16 = 50_716;
const TAG_WHITE_LEVEL: u16 = 50_717;
const TAG_DEFAULT_CROP_ORIGIN: u16 = 50_719;
const TAG_DEFAULT_CROP_SIZE: u16 = 50_720;
const TAG_COLOR_MATRIX_1: u16 = 50_721;
const TAG_COLOR_MATRIX_2: u16 = 50_722;
const TAG_AS_SHOT_NEUTRAL: u16 = 50_728;
const TAG_CALIBRATION_ILLUMINANT_1: u16 = 50_778;
const TAG_CALIBRATION_ILLUMINANT_2: u16 = 50_779;
const TAG_ACTIVE_AREA: u16 = 50_829;

const PHOTOMETRIC_CFA: u64 = 32_803;
const COMPRESSION_UNCOMPRESSED: u64 = 1;
const COMPRESSION_JPEG: u64 = 7;
const MAX_DNG_VERSION: [u8; 4] = [1, 7, 1, 0];
const MAX_DIRECTORIES: usize = 256;
const MAX_DIRECTORY_DEPTH: usize = 32;
const MAX_SEGMENTS: usize = 1_048_576;
const MAX_IMAGE_SAMPLES: usize = 128 * 1024 * 1024;
const MAX_CFA_CELLS: usize = 256;
const MAX_LINEARIZATION_ENTRIES: usize = 65_536;

/// Parses the highest-resolution primary CFA image from a classic TIFF or
/// `BigTIFF` DNG payload.
#[allow(clippy::too_many_lines)]
pub(crate) fn parse(data: &[u8]) -> Result<DngImage<'_>, DngError> {
    let total_started = Instant::now();
    let tiff_header_started = Instant::now();
    let tiff = Tiff::parse(data, TiffLimits::default())?;
    let tiff_header = tiff_header_started.elapsed();
    let ifd_walk_started = Instant::now();
    let directories = collect_directories(&tiff)?;
    let ifd_walk = ifd_walk_started.elapsed();
    let root = directories
        .iter()
        .find(|directory| directory.ifd.offset == tiff.first_ifd_offset())
        .ok_or(DngError::MissingIfd0)?;

    let raw_ifd_select_started = Instant::now();
    let raw = select_raw_ifd(&directories)?;
    let raw_ifd_select = raw_ifd_select_started.elapsed();
    let metadata_started = Instant::now();
    let dng_version = version(root.entry_required(TAG_DNG_VERSION)?, TAG_DNG_VERSION)?;
    let backward_version = root
        .entry(TAG_DNG_BACKWARD_VERSION)?
        .map_or(Ok([dng_version[0], dng_version[1], 0, 0]), |entry| {
            version(entry, TAG_DNG_BACKWARD_VERSION)
        })?;
    if backward_version > MAX_DNG_VERSION {
        return Err(DngError::UnsupportedBackwardVersion {
            actual: backward_version,
            maximum: MAX_DNG_VERSION,
        });
    }
    let camera_model = root
        .entry_required(TAG_UNIQUE_CAMERA_MODEL)?
        .ascii()?
        .trim()
        .to_owned();
    let make = optional_ascii(root, TAG_MAKE)?.unwrap_or_else(|| {
        camera_model
            .split_ascii_whitespace()
            .next()
            .unwrap_or_default()
            .to_owned()
    });
    let model = optional_ascii(root, TAG_MODEL)?.unwrap_or_else(|| camera_model.clone());
    let width = required_u32(raw, TAG_IMAGE_WIDTH)?;
    let height = required_u32(raw, TAG_IMAGE_LENGTH)?;
    let sample_count = checked_sample_count(width, height)?;
    let samples_per_pixel = optional_u16(raw, TAG_SAMPLES_PER_PIXEL)?.unwrap_or(1);
    if samples_per_pixel != 1 {
        return Err(DngError::UnsupportedSamplesPerPixel {
            actual: samples_per_pixel,
        });
    }
    let sample_format = optional_u16(raw, 339)?.unwrap_or(1);
    if sample_format != 1 {
        return Err(DngError::UnsupportedSampleFormat {
            actual: sample_format,
        });
    }
    let planar_configuration = optional_u16(raw, TAG_PLANAR_CONFIGURATION)?.unwrap_or(1);
    if planar_configuration != 1 {
        return Err(DngError::UnsupportedPlanarConfiguration {
            actual: planar_configuration,
        });
    }
    let fill_order = optional_u16(raw, TAG_FILL_ORDER)?.unwrap_or(1);
    if fill_order != 1 {
        return Err(DngError::UnsupportedFillOrder { actual: fill_order });
    }

    let bits_values = raw.entry_required(TAG_BITS_PER_SAMPLE)?.unsigned_values()?;
    if bits_values.len() != 1 {
        return Err(DngError::InvalidTagCount {
            tag: TAG_BITS_PER_SAMPLE,
            expected: 1,
            actual: bits_values.len(),
        });
    }
    let stored_bits_per_sample =
        u8::try_from(bits_values[0]).map_err(|_| DngError::UnsupportedBitsPerSample {
            actual: bits_values[0],
        })?;
    if !(8..=16).contains(&stored_bits_per_sample) {
        return Err(DngError::UnsupportedBitsPerSample {
            actual: u64::from(stored_bits_per_sample),
        });
    }

    let compression_code = raw.entry_required(TAG_COMPRESSION)?.unsigned_scalar()?;
    let compression = match compression_code {
        COMPRESSION_UNCOMPRESSED => Compression::Uncompressed,
        COMPRESSION_JPEG => Compression::LosslessJpeg,
        actual => return Err(DngError::UnsupportedCompression { actual }),
    };
    let predictor = optional_u16(raw, TAG_PREDICTOR)?.unwrap_or(1);
    if compression == Compression::Uncompressed && predictor != 1 {
        return Err(DngError::UnsupportedPredictor { actual: predictor });
    }

    let cfa = parse_cfa(raw)?;
    let active_area = parse_active_area(raw, width, height)?;
    let crop = parse_default_crop(raw, active_area)?;
    let black_level = parse_black_level(raw, active_area, samples_per_pixel)?;
    let white_level = parse_white_level(raw, stored_bits_per_sample, samples_per_pixel)?;
    let linearization_table = parse_linearization_table(raw)?;
    let output_bits_per_sample = output_bit_depth(
        stored_bits_per_sample,
        &white_level,
        linearization_table.as_deref(),
    )?;
    let color_planes = cfa.plane_colors.len();
    let color_matrix_1 = parse_optional_matrix(
        raw,
        &root.ifd,
        TAG_COLOR_MATRIX_1,
        color_planes
            .checked_mul(3)
            .ok_or(DngError::ArithmeticOverflow("ColorMatrix1 count"))?,
    )?;
    let color_matrix_2 = parse_optional_matrix(
        raw,
        &root.ifd,
        TAG_COLOR_MATRIX_2,
        color_planes
            .checked_mul(3)
            .ok_or(DngError::ArithmeticOverflow("ColorMatrix2 count"))?,
    )?;
    let calibration_illuminant_1 = optional_u16_scoped(raw, &root.ifd, TAG_CALIBRATION_ILLUMINANT_1)?;
    let calibration_illuminant_2 = optional_u16_scoped(raw, &root.ifd, TAG_CALIBRATION_ILLUMINANT_2)?;
    let as_shot_neutral = parse_optional_vector(raw, &root.ifd, TAG_AS_SHOT_NEUTRAL, color_planes)?;
    let orientation = parse_orientation(raw, &root.ifd)?;
    let metadata_elapsed = metadata_started.elapsed();
    let storage_plan_started = Instant::now();
    let storage = parse_storage(data, raw, width, height)?;
    let storage_plan = storage_plan_started.elapsed();

    Ok(DngImage {
        byte_order: tiff.byte_order(),
        width,
        height,
        sample_count,
        stored_bits_per_sample,
        output_bits_per_sample,
        compression,
        parse_timings: DngParseTimings {
            tiff_header,
            ifd_walk,
            raw_ifd_select,
            metadata: metadata_elapsed,
            storage_plan,
            total: total_started.elapsed(),
        },
        metadata: DngMetadata {
            dng_version,
            backward_version,
            make,
            model,
            camera_model,
            orientation,
            cfa,
            black_level,
            white_level,
            active_area,
            crop,
            linearization_table,
            color_matrix_1,
            color_matrix_2,
            calibration_illuminant_1,
            calibration_illuminant_2,
            as_shot_neutral,
        },
        storage,
    })
}

#[derive(Debug)]
pub(crate) struct DngImage<'a> {
    pub(crate) byte_order: ByteOrder,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) sample_count: usize,
    pub(crate) stored_bits_per_sample: u8,
    pub(crate) output_bits_per_sample: u8,
    pub(crate) compression: Compression,
    pub(crate) parse_timings: DngParseTimings,
    pub(crate) metadata: DngMetadata,
    storage: Storage<'a>,
}

impl DngImage<'_> {
    /// Decodes the stored CFA plane and applies `LinearizationTable`, if present.
    ///
    /// Segments decode on a bounded worker set (`RRRAH_DNG_DECODE_WORKERS`,
    /// default: available parallelism); the result is bit-identical for any
    /// worker count.
    pub(crate) fn decode_u16(
        &self,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<DngDecodedPixels, DngError> {
        self.decode_u16_with_workers(cancelled, parallel::env_workers())
    }

    /// Same as [`DngImage::decode_u16`] with an explicit segment-decode
    /// worker count (1 = fully sequential).
    pub(crate) fn decode_u16_with_workers(
        &self,
        cancelled: &(dyn Fn() -> bool + Sync),
        workers: usize,
    ) -> Result<DngDecodedPixels, DngError> {
        let total_started = Instant::now();
        if cancelled() {
            return Err(DngError::Cancelled { row: 0 });
        }

        let pixel_unpack_started = Instant::now();
        let mut pixels = match self.compression {
            Compression::Uncompressed => uncompressed::decode(self, cancelled, workers),
            Compression::LosslessJpeg => lossless_storage::decode(self, cancelled, workers),
        }?;
        let pixel_unpack = pixel_unpack_started.elapsed();

        let linearization_started = Instant::now();
        if let Some(table) = self.metadata.linearization_table.as_deref() {
            let last_index = table.len() - 1;
            let width = usize::try_from(self.width)
                .map_err(|_| DngError::ArithmeticOverflow("linearization row width"))?;
            for (row, samples) in pixels.chunks_mut(width).enumerate() {
                if cancelled() {
                    return Err(DngError::Cancelled { row });
                }
                for sample in samples {
                    *sample = table[usize::from(*sample).min(last_index)];
                }
            }
        }
        let linearization = linearization_started.elapsed();

        Ok(DngDecodedPixels {
            pixels,
            timings: DngPixelTimings {
                pixel_unpack,
                linearization,
                total: total_started.elapsed(),
            },
        })
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DngParseTimings {
    pub(crate) tiff_header: Duration,
    pub(crate) ifd_walk: Duration,
    pub(crate) raw_ifd_select: Duration,
    pub(crate) metadata: Duration,
    pub(crate) storage_plan: Duration,
    #[allow(dead_code)]
    pub(crate) total: Duration,
}

#[derive(Debug)]
pub(crate) struct DngDecodedPixels {
    pub(crate) pixels: Vec<u16>,
    pub(crate) timings: DngPixelTimings,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DngPixelTimings {
    pub(crate) pixel_unpack: Duration,
    pub(crate) linearization: Duration,
    #[allow(dead_code)]
    pub(crate) total: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Compression {
    Uncompressed,
    LosslessJpeg,
}

#[derive(Debug)]
enum Storage<'a> {
    Strips {
        rows_per_strip: u32,
        segments: Vec<Segment<'a>>,
    },
    Tiles {
        tile_width: u32,
        tile_height: u32,
        segments: Vec<Segment<'a>>,
    },
}

#[derive(Debug, Clone, Copy)]
struct Segment<'a> {
    offset: u64,
    bytes: &'a [u8],
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DngMetadata {
    pub(crate) dng_version: [u8; 4],
    pub(crate) backward_version: [u8; 4],
    pub(crate) make: String,
    pub(crate) model: String,
    /// The required DNG `UniqueCameraModel`, retained independently of TIFF `Model`.
    pub(crate) camera_model: String,
    pub(crate) orientation: Orientation,
    pub(crate) cfa: CfaPattern,
    pub(crate) black_level: BlackLevel,
    pub(crate) white_level: Vec<u16>,
    pub(crate) active_area: Rect,
    pub(crate) crop: Crop,
    pub(crate) linearization_table: Option<Vec<u16>>,
    /// Row-major camera-native-by-XYZ matrix from `ColorMatrix1`.
    pub(crate) color_matrix_1: Option<Vec<f64>>,
    /// Row-major camera-native-by-XYZ matrix from `ColorMatrix2`.
    pub(crate) color_matrix_2: Option<Vec<f64>>,
    /// EXIF light source code from `CalibrationIlluminant1`.
    pub(crate) calibration_illuminant_1: Option<u16>,
    /// EXIF light source code from `CalibrationIlluminant2`.
    pub(crate) calibration_illuminant_2: Option<u16>,
    /// Camera-neutral coordinates from `AsShotNeutral`.
    pub(crate) as_shot_neutral: Option<Vec<f64>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CfaPattern {
    pub(crate) rows: u16,
    pub(crate) columns: u16,
    pub(crate) cells: Vec<CfaColor>,
    pub(crate) plane_colors: Vec<CfaColor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CfaColor {
    Red,
    Green,
    Blue,
    Cyan,
    Magenta,
    Yellow,
    White,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BlackLevel {
    pub(crate) repeat_rows: u16,
    pub(crate) repeat_columns: u16,
    /// Row-column-sample order, as specified by DNG.
    pub(crate) values: Vec<f64>,
    pub(crate) delta_horizontal: Vec<f64>,
    pub(crate) delta_vertical: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rect {
    pub(crate) top: u32,
    pub(crate) left: u32,
    pub(crate) bottom: u32,
    pub(crate) right: u32,
}

impl Rect {
    pub(crate) const fn width(self) -> u32 {
        self.right - self.left
    }

    pub(crate) const fn height(self) -> u32 {
        self.bottom - self.top
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Crop {
    pub(crate) origin_x: f64,
    pub(crate) origin_y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Orientation {
    Normal,
    HorizontalFlip,
    Rotate180,
    VerticalFlip,
    Transpose,
    Rotate90,
    Transverse,
    Rotate270,
}

#[derive(Debug)]
struct Directory<'a> {
    ifd: Ifd<'a>,
}

impl<'a> Directory<'a> {
    fn entry(&self, tag: u16) -> Result<Option<&Entry<'a>>, DngError> {
        Ok(self.ifd.entry(tag)?)
    }

    fn entry_required(&self, tag: u16) -> Result<&Entry<'a>, DngError> {
        self.entry(tag)?.ok_or(DngError::MissingTag { tag })
    }
}

fn collect_directories<'a>(tiff: &Tiff<'a>) -> Result<Vec<Directory<'a>>, DngError> {
    let mut queue = VecDeque::from([(tiff.first_ifd_offset(), true, 0_usize)]);
    let mut seen = HashSet::new();
    let mut directories = Vec::new();
    while let Some((offset, is_top_level, depth)) = queue.pop_front() {
        if depth > MAX_DIRECTORY_DEPTH {
            return Err(DngError::DirectoryDepthLimit {
                actual: depth,
                limit: MAX_DIRECTORY_DEPTH,
            });
        }
        if !seen.insert(offset) {
            return Err(DngError::DirectoryCycle { offset });
        }
        if directories.len() >= MAX_DIRECTORIES {
            return Err(DngError::DirectoryLimit {
                limit: MAX_DIRECTORIES,
            });
        }
        let ifd = tiff.parse_ifd(offset)?;
        if !is_top_level && ifd.next_ifd_offset != 0 {
            return Err(DngError::SubIfdChain {
                offset,
                next: ifd.next_ifd_offset,
            });
        }
        let sub_offsets = ifd
            .entry(TAG_SUB_IFDS)?
            .map(Entry::unsigned_values)
            .transpose()?
            .unwrap_or_default();
        for sub_offset in sub_offsets {
            if sub_offset == 0 {
                return Err(DngError::ZeroSubIfdOffset { parent: offset });
            }
            queue.push_back((sub_offset, false, depth + 1));
        }
        if is_top_level && ifd.next_ifd_offset != 0 {
            queue.push_back((ifd.next_ifd_offset, true, 0));
        }
        directories.push(Directory { ifd });
    }
    Ok(directories)
}

fn select_raw_ifd<'a>(directories: &'a [Directory<'a>]) -> Result<&'a Directory<'a>, DngError> {
    let mut best: Option<(&Directory<'_>, (u8, u64))> = None;
    let mut tied = false;
    for directory in directories {
        let Some(photometric) = directory.entry(TAG_PHOTOMETRIC_INTERPRETATION)? else {
            continue;
        };
        if photometric.unsigned_scalar()? != PHOTOMETRIC_CFA {
            continue;
        }
        let width = u64::from(required_u32(directory, TAG_IMAGE_WIDTH)?);
        let height = u64::from(required_u32(directory, TAG_IMAGE_LENGTH)?);
        let area = width
            .checked_mul(height)
            .ok_or(DngError::ArithmeticOverflow("raw IFD area"))?;
        let primary = u8::from(optional_u32(directory, TAG_NEW_SUBFILE_TYPE)?.unwrap_or(0) == 0);
        let score = (primary, area);
        match best {
            None => {
                best = Some((directory, score));
                tied = false;
            }
            Some((_, best_score)) if score > best_score => {
                best = Some((directory, score));
                tied = false;
            }
            Some((_, best_score)) if score == best_score => tied = true,
            Some(_) => {}
        }
    }
    if tied {
        return Err(DngError::AmbiguousRawIfd);
    }
    best.map(|(directory, _)| directory)
        .ok_or(DngError::MissingRawIfd)
}

fn parse_cfa(raw: &Directory<'_>) -> Result<CfaPattern, DngError> {
    let dimensions = raw
        .entry_required(TAG_CFA_REPEAT_PATTERN_DIM)?
        .unsigned_values()?;
    if dimensions.len() != 2 {
        return Err(DngError::InvalidTagCount {
            tag: TAG_CFA_REPEAT_PATTERN_DIM,
            expected: 2,
            actual: dimensions.len(),
        });
    }
    let rows =
        u16::try_from(dimensions[0]).map_err(|_| DngError::InvalidCfaDimensions { dimensions: [0, 0] })?;
    let columns = u16::try_from(dimensions[1]).map_err(|_| DngError::InvalidCfaDimensions {
        dimensions: [rows, 0],
    })?;
    let cell_count = usize::from(rows)
        .checked_mul(usize::from(columns))
        .ok_or(DngError::ArithmeticOverflow("CFA cell count"))?;
    if cell_count == 0 || cell_count > MAX_CFA_CELLS {
        return Err(DngError::InvalidCfaDimensions {
            dimensions: [rows, columns],
        });
    }
    let layout = optional_u16(raw, TAG_CFA_LAYOUT)?.unwrap_or(1);
    if layout != 1 {
        return Err(DngError::UnsupportedCfaLayout { actual: layout });
    }

    let plane_codes = raw
        .entry(TAG_CFA_PLANE_COLOR)?
        .map(|entry| {
            if entry.field_type != FieldType::Byte {
                return Err(DngError::InvalidTagType {
                    tag: TAG_CFA_PLANE_COLOR,
                    expected: "BYTE",
                });
            }
            Ok(entry.raw_bytes().to_vec())
        })
        .transpose()?
        .unwrap_or_else(|| vec![0, 1, 2]);
    if plane_codes.is_empty() {
        return Err(DngError::EmptyCfaPlaneColors);
    }
    let plane_colors = plane_codes
        .iter()
        .copied()
        .map(cfa_color)
        .collect::<Result<Vec<_>, _>>()?;

    let pattern = raw.entry_required(TAG_CFA_PATTERN)?;
    if pattern.field_type != FieldType::Byte {
        return Err(DngError::InvalidTagType {
            tag: TAG_CFA_PATTERN,
            expected: "BYTE",
        });
    }
    if pattern.raw_bytes().len() != cell_count {
        return Err(DngError::InvalidTagCount {
            tag: TAG_CFA_PATTERN,
            expected: cell_count,
            actual: pattern.raw_bytes().len(),
        });
    }
    let mut cells = Vec::new();
    cells
        .try_reserve_exact(cell_count)
        .map_err(|_| DngError::AllocationFailed { elements: cell_count })?;
    for &plane_index in pattern.raw_bytes() {
        let color =
            plane_colors
                .get(usize::from(plane_index))
                .copied()
                .ok_or(DngError::InvalidCfaPlaneIndex {
                    index: plane_index,
                    planes: plane_colors.len(),
                })?;
        cells.push(color);
    }
    Ok(CfaPattern {
        rows,
        columns,
        cells,
        plane_colors,
    })
}

fn parse_active_area(raw: &Directory<'_>, width: u32, height: u32) -> Result<Rect, DngError> {
    let values = raw
        .entry(TAG_ACTIVE_AREA)?
        .map(Entry::unsigned_values)
        .transpose()?
        .unwrap_or_else(|| vec![0, 0, u64::from(height), u64::from(width)]);
    if values.len() != 4 {
        return Err(DngError::InvalidTagCount {
            tag: TAG_ACTIVE_AREA,
            expected: 4,
            actual: values.len(),
        });
    }
    let rectangle = Rect {
        top: to_u32(values[0], TAG_ACTIVE_AREA)?,
        left: to_u32(values[1], TAG_ACTIVE_AREA)?,
        bottom: to_u32(values[2], TAG_ACTIVE_AREA)?,
        right: to_u32(values[3], TAG_ACTIVE_AREA)?,
    };
    if rectangle.top >= rectangle.bottom
        || rectangle.left >= rectangle.right
        || rectangle.bottom > height
        || rectangle.right > width
    {
        return Err(DngError::InvalidActiveArea {
            area: rectangle,
            image: [width, height],
        });
    }
    Ok(rectangle)
}

fn parse_default_crop(raw: &Directory<'_>, active: Rect) -> Result<Crop, DngError> {
    let origin = raw
        .entry(TAG_DEFAULT_CROP_ORIGIN)?
        .map(Entry::numeric_values)
        .transpose()?
        .unwrap_or_else(|| vec![0.0, 0.0]);
    let size = raw
        .entry(TAG_DEFAULT_CROP_SIZE)?
        .map(Entry::numeric_values)
        .transpose()?
        .unwrap_or_else(|| vec![f64::from(active.width()), f64::from(active.height())]);
    if origin.len() != 2 {
        return Err(DngError::InvalidTagCount {
            tag: TAG_DEFAULT_CROP_ORIGIN,
            expected: 2,
            actual: origin.len(),
        });
    }
    if size.len() != 2 {
        return Err(DngError::InvalidTagCount {
            tag: TAG_DEFAULT_CROP_SIZE,
            expected: 2,
            actual: size.len(),
        });
    }
    let crop = Crop {
        origin_x: f64::from(active.left) + origin[0],
        origin_y: f64::from(active.top) + origin[1],
        width: size[0],
        height: size[1],
    };
    if crop.origin_x < f64::from(active.left)
        || crop.origin_y < f64::from(active.top)
        || crop.width <= 0.0
        || crop.height <= 0.0
        || crop.origin_x + crop.width > f64::from(active.right)
        || crop.origin_y + crop.height > f64::from(active.bottom)
    {
        return Err(DngError::InvalidDefaultCrop { crop, active });
    }
    Ok(crop)
}

fn parse_black_level(
    raw: &Directory<'_>,
    active: Rect,
    samples_per_pixel: u16,
) -> Result<BlackLevel, DngError> {
    let repeat = raw
        .entry(TAG_BLACK_LEVEL_REPEAT_DIM)?
        .map(Entry::unsigned_values)
        .transpose()?
        .unwrap_or_else(|| vec![1, 1]);
    if repeat.len() != 2 {
        return Err(DngError::InvalidTagCount {
            tag: TAG_BLACK_LEVEL_REPEAT_DIM,
            expected: 2,
            actual: repeat.len(),
        });
    }
    let repeat_rows = to_u16(repeat[0], TAG_BLACK_LEVEL_REPEAT_DIM)?;
    let repeat_columns = to_u16(repeat[1], TAG_BLACK_LEVEL_REPEAT_DIM)?;
    let count = usize::from(repeat_rows)
        .checked_mul(usize::from(repeat_columns))
        .and_then(|value| value.checked_mul(usize::from(samples_per_pixel)))
        .ok_or(DngError::ArithmeticOverflow("black-level value count"))?;
    if count == 0 || count > MAX_CFA_CELLS {
        return Err(DngError::InvalidBlackLevelDimensions {
            rows: repeat_rows,
            columns: repeat_columns,
            samples_per_pixel,
        });
    }
    let values = raw
        .entry(TAG_BLACK_LEVEL)?
        .map(Entry::numeric_values)
        .transpose()?
        .unwrap_or_else(|| vec![0.0; count]);
    if values.len() != count {
        return Err(DngError::InvalidTagCount {
            tag: TAG_BLACK_LEVEL,
            expected: count,
            actual: values.len(),
        });
    }
    if values.iter().any(|value| *value < 0.0) {
        return Err(DngError::NegativeBlackLevel);
    }
    let delta_horizontal = parse_optional_numeric_count(
        raw,
        TAG_BLACK_LEVEL_DELTA_H,
        usize::try_from(active.width()).map_err(|_| DngError::ArithmeticOverflow("active width"))?,
    )?;
    let delta_vertical = parse_optional_numeric_count(
        raw,
        TAG_BLACK_LEVEL_DELTA_V,
        usize::try_from(active.height()).map_err(|_| DngError::ArithmeticOverflow("active height"))?,
    )?;
    if !delta_horizontal.is_empty() || !delta_vertical.is_empty() {
        return Err(DngError::UnsupportedBlackLevelDeltas {
            horizontal: delta_horizontal.len(),
            vertical: delta_vertical.len(),
        });
    }
    Ok(BlackLevel {
        repeat_rows,
        repeat_columns,
        values,
        delta_horizontal,
        delta_vertical,
    })
}

fn parse_white_level(
    raw: &Directory<'_>,
    bits_per_sample: u8,
    samples_per_pixel: u16,
) -> Result<Vec<u16>, DngError> {
    let maximum = (1_u32 << bits_per_sample) - 1;
    let values = raw
        .entry(TAG_WHITE_LEVEL)?
        .map(Entry::unsigned_values)
        .transpose()?
        .unwrap_or_else(|| vec![u64::from(maximum)]);
    let expected = usize::from(samples_per_pixel);
    if values.len() != expected {
        return Err(DngError::InvalidTagCount {
            tag: TAG_WHITE_LEVEL,
            expected,
            actual: values.len(),
        });
    }
    values
        .into_iter()
        .map(|value| {
            let converted = u16::try_from(value).map_err(|_| DngError::WhiteLevelOutOfRange { value })?;
            if converted == 0 {
                return Err(DngError::WhiteLevelOutOfRange { value });
            }
            Ok(converted)
        })
        .collect()
}

fn parse_linearization_table(raw: &Directory<'_>) -> Result<Option<Vec<u16>>, DngError> {
    let Some(entry) = raw.entry(TAG_LINEARIZATION_TABLE)? else {
        return Ok(None);
    };
    if entry.field_type != FieldType::Short {
        return Err(DngError::InvalidTagType {
            tag: TAG_LINEARIZATION_TABLE,
            expected: "SHORT",
        });
    }
    let values = entry.unsigned_values()?;
    if values.is_empty() || values.len() > MAX_LINEARIZATION_ENTRIES {
        return Err(DngError::LinearizationTableSize {
            actual: values.len(),
            maximum: MAX_LINEARIZATION_ENTRIES,
        });
    }
    values
        .into_iter()
        .map(|value| u16::try_from(value).map_err(|_| DngError::LinearizationValueOutOfRange { value }))
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn output_bit_depth(
    stored_bits: u8,
    white_level: &[u16],
    linearization_table: Option<&[u16]>,
) -> Result<u8, DngError> {
    if linearization_table.is_some() {
        return Ok(16);
    }
    let identity_max = (1_u32 << stored_bits) - 1;
    let white_max = white_level.iter().copied().max().map_or(0, u32::from);
    let maximum = identity_max.max(white_max);
    let bits = (u32::BITS - maximum.leading_zeros()).max(1);
    u8::try_from(bits).map_err(|_| DngError::OutputBitsOverflow { maximum })
}

fn parse_optional_matrix(
    raw: &Directory<'_>,
    root: &Ifd<'_>,
    tag: u16,
    expected: usize,
) -> Result<Option<Vec<f64>>, DngError> {
    parse_optional_vector(raw, root, tag, expected)
}

fn parse_optional_vector(
    raw: &Directory<'_>,
    root: &Ifd<'_>,
    tag: u16,
    expected: usize,
) -> Result<Option<Vec<f64>>, DngError> {
    let entry = raw.entry(tag)?.or(root.entry(tag)?);
    let Some(entry) = entry else {
        return Ok(None);
    };
    let values = entry.numeric_values()?;
    if values.len() != expected {
        return Err(DngError::InvalidTagCount {
            tag,
            expected,
            actual: values.len(),
        });
    }
    if tag == TAG_AS_SHOT_NEUTRAL && values.iter().any(|value| *value <= 0.0) {
        return Err(DngError::InvalidAsShotNeutral);
    }
    Ok(Some(values))
}

fn parse_orientation(raw: &Directory<'_>, root: &Ifd<'_>) -> Result<Orientation, DngError> {
    let value = raw
        .entry(TAG_ORIENTATION)?
        .or(root.entry(TAG_ORIENTATION)?)
        .ok_or(DngError::MissingTag { tag: TAG_ORIENTATION })?
        .unsigned_scalar()?;
    match value {
        1 => Ok(Orientation::Normal),
        2 => Ok(Orientation::HorizontalFlip),
        3 => Ok(Orientation::Rotate180),
        4 => Ok(Orientation::VerticalFlip),
        5 => Ok(Orientation::Transpose),
        6 => Ok(Orientation::Rotate90),
        7 => Ok(Orientation::Transverse),
        8 => Ok(Orientation::Rotate270),
        actual => Err(DngError::InvalidOrientation { actual }),
    }
}

fn parse_storage<'a>(
    data: &'a [u8],
    raw: &Directory<'_>,
    width: u32,
    height: u32,
) -> Result<Storage<'a>, DngError> {
    let strip_offsets = raw.entry(TAG_STRIP_OFFSETS)?;
    let strip_counts = raw.entry(TAG_STRIP_BYTE_COUNTS)?;
    let tile_offsets = raw.entry(TAG_TILE_OFFSETS)?;
    let tile_counts = raw.entry(TAG_TILE_BYTE_COUNTS)?;
    match (strip_offsets, strip_counts, tile_offsets, tile_counts) {
        (Some(offsets), Some(counts), None, None) => {
            let rows_per_strip = optional_u32(raw, TAG_ROWS_PER_STRIP)?.unwrap_or(height);
            if rows_per_strip == 0 {
                return Err(DngError::InvalidRowsPerStrip);
            }
            let expected = usize::try_from(div_ceil_u32(height, rows_per_strip))
                .map_err(|_| DngError::ArithmeticOverflow("strip count"))?;
            let segments = parse_segments(data, offsets, counts, expected)?;
            Ok(Storage::Strips {
                rows_per_strip,
                segments,
            })
        }
        (None, None, Some(offsets), Some(counts)) => {
            let tile_width = required_u32(raw, TAG_TILE_WIDTH)?;
            let tile_height = required_u32(raw, TAG_TILE_LENGTH)?;
            if tile_width == 0 || tile_height == 0 {
                return Err(DngError::InvalidTileDimensions {
                    width: tile_width,
                    height: tile_height,
                });
            }
            let columns = div_ceil_u32(width, tile_width);
            let rows = div_ceil_u32(height, tile_height);
            let expected_u32 = columns
                .checked_mul(rows)
                .ok_or(DngError::ArithmeticOverflow("tile count"))?;
            let expected =
                usize::try_from(expected_u32).map_err(|_| DngError::ArithmeticOverflow("tile count"))?;
            let segments = parse_segments(data, offsets, counts, expected)?;
            Ok(Storage::Tiles {
                tile_width,
                tile_height,
                segments,
            })
        }
        _ => Err(DngError::InvalidImageStorage),
    }
}

fn parse_segments<'a>(
    data: &'a [u8],
    offsets: &Entry<'_>,
    counts: &Entry<'_>,
    expected: usize,
) -> Result<Vec<Segment<'a>>, DngError> {
    if expected == 0 || expected > MAX_SEGMENTS {
        return Err(DngError::SegmentLimit {
            actual: expected,
            limit: MAX_SEGMENTS,
        });
    }
    let offsets = offsets.unsigned_values()?;
    let counts = counts.unsigned_values()?;
    if offsets.len() != counts.len() || offsets.len() != expected {
        return Err(DngError::SegmentCountMismatch {
            offsets: offsets.len(),
            byte_counts: counts.len(),
            expected,
        });
    }
    let mut ranges = Vec::new();
    ranges
        .try_reserve_exact(expected)
        .map_err(|_| DngError::AllocationFailed { elements: expected })?;
    for (&offset, &count) in offsets.iter().zip(&counts) {
        let start = usize::try_from(offset).map_err(|_| DngError::SegmentRange { offset, count })?;
        let length = usize::try_from(count).map_err(|_| DngError::SegmentRange { offset, count })?;
        let end = start
            .checked_add(length)
            .ok_or(DngError::SegmentRange { offset, count })?;
        let bytes = data
            .get(start..end)
            .ok_or(DngError::SegmentRange { offset, count })?;
        ranges.push(Segment { offset, bytes });
    }
    let mut ordered = ranges
        .iter()
        .map(|segment| {
            let length = u64::try_from(segment.bytes.len()).unwrap_or(u64::MAX);
            (segment.offset, segment.offset.saturating_add(length))
        })
        .collect::<Vec<_>>();
    ordered.sort_unstable();
    if ordered.windows(2).any(|window| window[0].1 > window[1].0) {
        return Err(DngError::OverlappingSegments);
    }
    Ok(ranges)
}

fn version(entry: &Entry<'_>, tag: u16) -> Result<[u8; 4], DngError> {
    if entry.field_type != FieldType::Byte || entry.raw_bytes().len() != 4 {
        return Err(DngError::InvalidTagType {
            tag,
            expected: "BYTE[4]",
        });
    }
    Ok([
        entry.raw_bytes()[0],
        entry.raw_bytes()[1],
        entry.raw_bytes()[2],
        entry.raw_bytes()[3],
    ])
}

fn required_u32(directory: &Directory<'_>, tag: u16) -> Result<u32, DngError> {
    let value = directory.entry_required(tag)?.unsigned_scalar()?;
    to_u32(value, tag)
}

fn optional_u32(directory: &Directory<'_>, tag: u16) -> Result<Option<u32>, DngError> {
    directory
        .entry(tag)?
        .map(|entry| {
            entry.unsigned_scalar().and_then(|value| {
                u32::try_from(value).map_err(|_| TiffError::ArithmeticOverflow("u32 TIFF value"))
            })
        })
        .transpose()
        .map_err(Into::into)
}

fn optional_u16(directory: &Directory<'_>, tag: u16) -> Result<Option<u16>, DngError> {
    directory
        .entry(tag)?
        .map(|entry| {
            entry.unsigned_scalar().and_then(|value| {
                u16::try_from(value).map_err(|_| TiffError::ArithmeticOverflow("u16 TIFF value"))
            })
        })
        .transpose()
        .map_err(Into::into)
}

/// Like [`optional_u16`], but also falls back to the root IFD — the DNG
/// calibration tags may live in either directory.
fn optional_u16_scoped(raw: &Directory<'_>, root: &Ifd<'_>, tag: u16) -> Result<Option<u16>, DngError> {
    let entry = raw.entry(tag)?.or(root.entry(tag)?);
    entry
        .map(|entry| {
            entry.unsigned_scalar().and_then(|value| {
                u16::try_from(value).map_err(|_| TiffError::ArithmeticOverflow("u16 TIFF value"))
            })
        })
        .transpose()
        .map_err(Into::into)
}

fn optional_ascii(directory: &Directory<'_>, tag: u16) -> Result<Option<String>, DngError> {
    Ok(directory
        .entry(tag)?
        .map(|entry| entry.ascii().map(str::trim).map(str::to_owned))
        .transpose()
        .map_err(DngError::from)?
        .filter(|value| !value.is_empty()))
}

fn parse_optional_numeric_count(
    raw: &Directory<'_>,
    tag: u16,
    expected: usize,
) -> Result<Vec<f64>, DngError> {
    let Some(entry) = raw.entry(tag)? else {
        return Ok(Vec::new());
    };
    let values = entry.numeric_values()?;
    if values.len() != expected {
        return Err(DngError::InvalidTagCount {
            tag,
            expected,
            actual: values.len(),
        });
    }
    Ok(values)
}

fn checked_sample_count(width: u32, height: u32) -> Result<usize, DngError> {
    if width == 0 || height == 0 {
        return Err(DngError::EmptyImage { width, height });
    }
    let count = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or(DngError::ArithmeticOverflow("image sample count"))?;
    if count > MAX_IMAGE_SAMPLES {
        return Err(DngError::ImageSampleLimit {
            actual: count,
            limit: MAX_IMAGE_SAMPLES,
        });
    }
    Ok(count)
}

fn cfa_color(code: u8) -> Result<CfaColor, DngError> {
    match code {
        0 => Ok(CfaColor::Red),
        1 => Ok(CfaColor::Green),
        2 => Ok(CfaColor::Blue),
        3 => Ok(CfaColor::Cyan),
        4 => Ok(CfaColor::Magenta),
        5 => Ok(CfaColor::Yellow),
        6 => Ok(CfaColor::White),
        actual => Err(DngError::UnsupportedCfaColor { actual }),
    }
}

fn to_u32(value: u64, tag: u16) -> Result<u32, DngError> {
    u32::try_from(value).map_err(|_| DngError::TagValueOutOfRange { tag, value })
}

fn to_u16(value: u64, tag: u16) -> Result<u16, DngError> {
    u16::try_from(value).map_err(|_| DngError::TagValueOutOfRange { tag, value })
}

const fn div_ceil_u32(value: u32, divisor: u32) -> u32 {
    value.div_ceil(divisor)
}

#[derive(Debug)]
pub(crate) enum DngError {
    Tiff(TiffError),
    LosslessJpeg(LosslessJpegError),
    MissingIfd0,
    MissingRawIfd,
    AmbiguousRawIfd,
    MissingTag {
        tag: u16,
    },
    InvalidTagType {
        tag: u16,
        expected: &'static str,
    },
    InvalidTagCount {
        tag: u16,
        expected: usize,
        actual: usize,
    },
    TagValueOutOfRange {
        tag: u16,
        value: u64,
    },
    UnsupportedBackwardVersion {
        actual: [u8; 4],
        maximum: [u8; 4],
    },
    DirectoryCycle {
        offset: u64,
    },
    DirectoryLimit {
        limit: usize,
    },
    DirectoryDepthLimit {
        actual: usize,
        limit: usize,
    },
    SubIfdChain {
        offset: u64,
        next: u64,
    },
    ZeroSubIfdOffset {
        parent: u64,
    },
    EmptyImage {
        width: u32,
        height: u32,
    },
    ImageSampleLimit {
        actual: usize,
        limit: usize,
    },
    UnsupportedSamplesPerPixel {
        actual: u16,
    },
    UnsupportedSampleFormat {
        actual: u16,
    },
    UnsupportedPlanarConfiguration {
        actual: u16,
    },
    UnsupportedFillOrder {
        actual: u16,
    },
    UnsupportedBitsPerSample {
        actual: u64,
    },
    UnsupportedCompression {
        actual: u64,
    },
    LosslessJpegPrecision {
        index: usize,
        expected: u8,
        actual: u8,
    },
    LosslessJpegSampleCount {
        index: usize,
        expected: usize,
        actual: usize,
        jpeg_width: u16,
        jpeg_height: u16,
        components: usize,
    },
    UnsupportedPredictor {
        actual: u16,
    },
    InvalidCfaDimensions {
        dimensions: [u16; 2],
    },
    UnsupportedCfaLayout {
        actual: u16,
    },
    EmptyCfaPlaneColors,
    UnsupportedCfaColor {
        actual: u8,
    },
    InvalidCfaPlaneIndex {
        index: u8,
        planes: usize,
    },
    InvalidActiveArea {
        area: Rect,
        image: [u32; 2],
    },
    InvalidDefaultCrop {
        crop: Crop,
        active: Rect,
    },
    InvalidBlackLevelDimensions {
        rows: u16,
        columns: u16,
        samples_per_pixel: u16,
    },
    NegativeBlackLevel,
    WhiteLevelOutOfRange {
        value: u64,
    },
    LinearizationTableSize {
        actual: usize,
        maximum: usize,
    },
    LinearizationValueOutOfRange {
        value: u64,
    },
    UnsupportedBlackLevelDeltas {
        horizontal: usize,
        vertical: usize,
    },
    OutputBitsOverflow {
        maximum: u32,
    },
    InvalidAsShotNeutral,
    InvalidOrientation {
        actual: u64,
    },
    InvalidRowsPerStrip,
    InvalidTileDimensions {
        width: u32,
        height: u32,
    },
    InvalidImageStorage,
    SegmentLimit {
        actual: usize,
        limit: usize,
    },
    SegmentCountMismatch {
        offsets: usize,
        byte_counts: usize,
        expected: usize,
    },
    SegmentRange {
        offset: u64,
        count: u64,
    },
    OverlappingSegments,
    SegmentLength {
        index: usize,
        expected: usize,
        actual: usize,
    },
    TruncatedPackedRow {
        expected: usize,
        actual: usize,
    },
    Cancelled {
        row: usize,
    },
    ArithmeticOverflow(&'static str),
    AllocationFailed {
        elements: usize,
    },
}

impl From<TiffError> for DngError {
    fn from(error: TiffError) -> Self {
        Self::Tiff(error)
    }
}

impl From<LosslessJpegError> for DngError {
    fn from(error: LosslessJpegError) -> Self {
        Self::LosslessJpeg(error)
    }
}

impl fmt::Display for DngError {
    #[allow(clippy::too_many_lines)]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tiff(error) => write!(formatter, "DNG TIFF parse failed: {error}"),
            Self::LosslessJpeg(error) => write!(formatter, "DNG lossless JPEG decode failed: {error}"),
            Self::MissingIfd0 => formatter.write_str("DNG IFD0 is missing"),
            Self::MissingRawIfd => formatter.write_str("DNG contains no CFA raw IFD"),
            Self::AmbiguousRawIfd => formatter.write_str("DNG has multiple equally preferred raw IFDs"),
            Self::MissingTag { tag } => write!(formatter, "DNG required tag {tag} is missing"),
            Self::InvalidTagType { tag, expected } => {
                write!(formatter, "DNG tag {tag} must have type {expected}")
            }
            Self::InvalidTagCount {
                tag,
                expected,
                actual,
            } => write!(
                formatter,
                "DNG tag {tag} has {actual} values, expected {expected}"
            ),
            Self::TagValueOutOfRange { tag, value } => {
                write!(formatter, "DNG tag {tag} value {value} is out of range")
            }
            Self::UnsupportedBackwardVersion { actual, maximum } => write!(
                formatter,
                "DNG backward version {actual:?} is newer than supported {maximum:?}"
            ),
            Self::DirectoryCycle { offset } => {
                write!(formatter, "DNG directory graph cycles at offset {offset}")
            }
            Self::DirectoryLimit { limit } => {
                write!(formatter, "DNG directory count exceeds limit {limit}")
            }
            Self::DirectoryDepthLimit { actual, limit } => {
                write!(formatter, "DNG SubIFD depth {actual} exceeds limit {limit}")
            }
            Self::SubIfdChain { offset, next } => write!(
                formatter,
                "DNG SubIFD at {offset} has unsupported next-IFD chain to {next}"
            ),
            Self::ZeroSubIfdOffset { parent } => {
                write!(formatter, "DNG IFD at {parent} contains a zero SubIFD offset")
            }
            Self::EmptyImage { width, height } => {
                write!(formatter, "DNG raw image geometry is empty: {width}x{height}")
            }
            Self::ImageSampleLimit { actual, limit } => {
                write!(formatter, "DNG image has {actual} samples, limit is {limit}")
            }
            Self::UnsupportedSamplesPerPixel { actual } => {
                write!(formatter, "DNG CFA SamplesPerPixel {actual} is unsupported")
            }
            Self::UnsupportedSampleFormat { actual } => {
                write!(formatter, "DNG SampleFormat {actual} is unsupported")
            }
            Self::UnsupportedPlanarConfiguration { actual } => {
                write!(formatter, "DNG PlanarConfiguration {actual} is unsupported")
            }
            Self::UnsupportedFillOrder { actual } => {
                write!(formatter, "DNG FillOrder {actual} is unsupported")
            }
            Self::UnsupportedBitsPerSample { actual } => {
                write!(formatter, "DNG BitsPerSample {actual} is unsupported")
            }
            Self::UnsupportedCompression { actual } => {
                write!(formatter, "DNG Compression {actual} is unsupported")
            }
            Self::LosslessJpegPrecision {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "DNG lossless JPEG segment {index} has precision {actual}, expected {expected}"
            ),
            Self::LosslessJpegSampleCount {
                index,
                expected,
                actual,
                jpeg_width,
                jpeg_height,
                components,
            } => write!(
                formatter,
                "DNG lossless JPEG segment {index} decoded {actual} samples from {jpeg_width}x{jpeg_height}x{components}, expected {expected}"
            ),
            Self::UnsupportedPredictor { actual } => {
                write!(formatter, "DNG Predictor {actual} is unsupported")
            }
            Self::InvalidCfaDimensions { dimensions } => {
                write!(formatter, "invalid DNG CFA dimensions {dimensions:?}")
            }
            Self::UnsupportedCfaLayout { actual } => {
                write!(formatter, "DNG CFA layout {actual} is unsupported")
            }
            Self::EmptyCfaPlaneColors => formatter.write_str("DNG CFA plane-color list is empty"),
            Self::UnsupportedCfaColor { actual } => {
                write!(formatter, "DNG CFA color code {actual} is unsupported")
            }
            Self::InvalidCfaPlaneIndex { index, planes } => write!(
                formatter,
                "DNG CFA pattern plane index {index} is outside {planes} planes"
            ),
            Self::InvalidActiveArea { area, image } => {
                write!(formatter, "DNG ActiveArea {area:?} is outside image {image:?}")
            }
            Self::InvalidDefaultCrop { crop, active } => {
                write!(formatter, "DNG crop {crop:?} is outside ActiveArea {active:?}")
            }
            Self::InvalidBlackLevelDimensions {
                rows,
                columns,
                samples_per_pixel,
            } => write!(
                formatter,
                "invalid DNG black-level grid {rows}x{columns}x{samples_per_pixel}"
            ),
            Self::NegativeBlackLevel => formatter.write_str("DNG BlackLevel contains a negative value"),
            Self::WhiteLevelOutOfRange { value } => {
                write!(formatter, "DNG WhiteLevel {value} is outside u16")
            }
            Self::LinearizationTableSize { actual, maximum } => write!(
                formatter,
                "DNG LinearizationTable has {actual} entries, maximum is {maximum}"
            ),
            Self::LinearizationValueOutOfRange { value } => {
                write!(formatter, "DNG LinearizationTable value {value} is outside u16")
            }
            Self::UnsupportedBlackLevelDeltas { horizontal, vertical } => write!(
                formatter,
                "DNG BlackLevelDeltaH/V are unsupported ({horizontal} horizontal, {vertical} vertical values)"
            ),
            Self::OutputBitsOverflow { maximum } => {
                write!(
                    formatter,
                    "DNG output maximum {maximum} does not fit supported bit depth"
                )
            }
            Self::InvalidAsShotNeutral => formatter.write_str("DNG AsShotNeutral values must be positive"),
            Self::InvalidOrientation { actual } => {
                write!(formatter, "DNG Orientation {actual} is invalid")
            }
            Self::InvalidRowsPerStrip => formatter.write_str("DNG RowsPerStrip is zero"),
            Self::InvalidTileDimensions { width, height } => {
                write!(formatter, "DNG tile geometry is invalid: {width}x{height}")
            }
            Self::InvalidImageStorage => {
                formatter.write_str("DNG raw IFD must contain exactly one complete strip or tile layout")
            }
            Self::SegmentLimit { actual, limit } => {
                write!(formatter, "DNG has {actual} segments, limit is {limit}")
            }
            Self::SegmentCountMismatch {
                offsets,
                byte_counts,
                expected,
            } => write!(
                formatter,
                "DNG segment tables have {offsets} offsets and {byte_counts} counts, expected {expected}"
            ),
            Self::SegmentRange { offset, count } => {
                write!(formatter, "DNG segment [{offset}, +{count}) is outside the file")
            }
            Self::OverlappingSegments => formatter.write_str("DNG image segments overlap"),
            Self::SegmentLength {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "DNG segment {index} has {actual} bytes, expected {expected}"
            ),
            Self::TruncatedPackedRow { expected, actual } => write!(
                formatter,
                "DNG packed row has {actual} bytes, expected {expected}"
            ),
            Self::Cancelled { row } => {
                write!(formatter, "DNG decoding was cancelled before row {row}")
            }
            Self::ArithmeticOverflow(context) => {
                write!(formatter, "arithmetic overflow while computing {context}")
            }
            Self::AllocationFailed { elements } => {
                write!(formatter, "could not allocate {elements} DNG elements")
            }
        }
    }
}

impl Error for DngError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Tiff(error) => Some(error),
            Self::LosslessJpeg(error) => Some(error),
            _ => None,
        }
    }
}

#[doc(hidden)]
pub mod bench_support {
    //! Measurement hooks for the criterion micro-benchmarks in `benches/`.
    //! This module is not public API and may change without notice.

    use super::{lossless_jpeg, uncompressed};

    /// Decodes a lossless-JPEG payload and returns the frame width, height,
    /// sample count, and an order-sensitive checksum of the decoded samples.
    pub fn decode_lossless_jpeg(bytes: &[u8]) -> Result<(u16, u16, usize, u64), String> {
        let image = lossless_jpeg::decode(bytes, &|| false).map_err(|error| error.to_string())?;
        let checksum = sample_checksum(&image.samples);
        Ok((image.width, image.height, image.samples.len(), checksum))
    }

    /// Same as [`decode_lossless_jpeg`], but forces the byte-at-a-time
    /// entropy refill (the pre-optimization reference path) for A/B timing.
    pub fn decode_lossless_jpeg_bytewise(bytes: &[u8]) -> Result<(u16, u16, usize, u64), String> {
        let image =
            lossless_jpeg::decode_with_refill(bytes, &|| false, false).map_err(|error| error.to_string())?;
        let checksum = sample_checksum(&image.samples);
        Ok((image.width, image.height, image.samples.len(), checksum))
    }

    /// Unpacks one MSB-first packed row into `output`.
    pub fn unpack_msb_row(encoded: &[u8], output: &mut [u16], bits_per_sample: u8) -> Result<(), String> {
        uncompressed::decode_msb_packed(encoded, output, bits_per_sample).map_err(|error| error.to_string())
    }

    /// Same as [`unpack_msb_row`], but uses the byte-at-a-time reference loop
    /// (the pre-optimization path) for A/B timing.
    pub fn unpack_msb_row_bytewise(
        encoded: &[u8],
        output: &mut [u16],
        bits_per_sample: u8,
    ) -> Result<(), String> {
        uncompressed::decode_msb_packed_bytewise(encoded, output, bits_per_sample)
            .map_err(|error| error.to_string())
    }

    // ------------------------------------------------------------------
    // Synthetic segmented-DNG fixtures for the parallel segment-decode
    // benchmarks and the cross-worker bit-identity tests.

    /// How pixel data is segmented inside a synthetic DNG fixture.
    #[derive(Debug, Clone, Copy)]
    pub enum SegmentLayout {
        /// Strip layout (tags 273/278/279) with this `RowsPerStrip` value.
        Strips { rows_per_strip: u32 },
        /// Tiled layout (tags 322/323/324/325); edge tiles are padded with
        /// zero samples to the full tile size, as the DNG spec requires.
        Tiles { tile_width: u32, tile_height: u32 },
    }

    /// Compression used by a synthetic DNG fixture.
    #[derive(Debug, Clone, Copy)]
    pub enum SyntheticCompression {
        /// DNG `Compression = 1`, 16 bits per sample, little-endian rows.
        Uncompressed16,
        /// DNG `Compression = 7`, 12-bit lossless JPEG, predictor 1.
        LosslessJpeg12,
    }

    const ENCODE_PRECISION: u8 = 12;

    /// Encodes `samples` (row-major, `width * height` values) as a
    /// single-component 12-bit lossless-JPEG stream, structurally identical
    /// to one DNG segment produced by a camera.
    pub fn encode_lossless_jpeg(width: u16, height: u16, predictor: u8, samples: &[u16]) -> Vec<u8> {
        assert_eq!(
            samples.len(),
            usize::from(width) * usize::from(height),
            "lossless-JPEG fixture sample count",
        );
        let mut output = Vec::new();
        jpeg_marker(&mut output, 0xd8); // SOI

        let mut dht = vec![0_u8];
        dht.extend_from_slice(&[0, 0, 0, 0, 17, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        dht.extend(0_u8..=16);
        jpeg_segment(&mut output, 0xc4, &dht); // DHT

        let mut frame = vec![ENCODE_PRECISION];
        frame.extend_from_slice(&height.to_be_bytes());
        frame.extend_from_slice(&width.to_be_bytes());
        frame.push(1);
        frame.extend_from_slice(&[1, 0x11, 0]);
        jpeg_segment(&mut output, 0xc3, &frame); // SOF3

        jpeg_segment(&mut output, 0xda, &[1, 1, 0, predictor, 0, 0]); // SOS

        let width_usize = usize::from(width);
        let initial = 1_i32 << (ENCODE_PRECISION - 1);
        let mut bits = BitWriter::new();
        for mcu in 0..samples.len() {
            let x = mcu % width_usize;
            let y = mcu / width_usize;
            let sample = i32::from(samples[mcu]);
            let predicted = if mcu == 0 {
                initial
            } else if y == 0 {
                i32::from(samples[mcu - 1])
            } else if x == 0 {
                i32::from(samples[mcu - width_usize])
            } else {
                let left = i32::from(samples[mcu - 1]);
                let above = i32::from(samples[mcu - width_usize]);
                let upper_left = i32::from(samples[mcu - width_usize - 1]);
                select_predictor(predictor, left, above, upper_left)
            };
            let difference = signed_modulo_difference(sample, predicted);
            let (category, encoded) = category_and_bits(difference);
            bits.write(u32::from(category), 5);
            if category < 16 {
                bits.write(encoded, category);
            }
        }
        bits.pad_ones();
        output.extend_from_slice(&bits.bytes);
        jpeg_marker(&mut output, 0xd9); // EOI
        output
    }

    /// Builds a minimal but fully valid single-IFD DNG: the root directory is
    /// also the raw CFA image. Every segment is an independent coded unit, so
    /// the file exercises the parallel segment decoder.
    ///
    /// # Panics
    ///
    /// Panics when `samples.len() != width * height` or a dimension exceeds
    /// the lossless-JPEG `u16` frame geometry limit.
    pub fn build_segmented_dng(
        width: u32,
        height: u32,
        layout: SegmentLayout,
        compression: SyntheticCompression,
        samples: &[u16],
    ) -> Vec<u8> {
        let width_usize = usize::try_from(width).expect("fixture width");
        let height_usize = usize::try_from(height).expect("fixture height");
        assert_eq!(samples.len(), width_usize * height_usize, "fixture sample count");
        let (bits_per_sample, compression_code) = match compression {
            SyntheticCompression::Uncompressed16 => (16_u16, 1_u16),
            SyntheticCompression::LosslessJpeg12 => (u16::from(ENCODE_PRECISION), 7),
        };

        let payloads = encode_segments(width_usize, height_usize, layout, compression, samples);
        assemble_dng(
            width,
            height,
            bits_per_sample,
            compression_code,
            layout,
            &payloads,
        )
    }

    /// Parses a DNG byte buffer and decodes the CFA plane with an explicit
    /// segment-decode worker count, returning the FNV-1a sample checksum so
    /// benchmarks can assert bit-identical output across worker counts.
    pub fn decode_dng_pixels_with_workers(bytes: &[u8], workers: usize) -> Result<u64, String> {
        let (checksum, _) = decode_dng_pixels_timed(bytes, workers)?;
        Ok(checksum)
    }

    /// Same as [`decode_dng_pixels_with_workers`], but also returns the parse
    /// and pixel-unpack wall times for scaling analysis.
    pub fn decode_dng_pixels_timed(bytes: &[u8], workers: usize) -> Result<(u64, DngDecodeTiming), String> {
        let image = super::parse(bytes).map_err(|error| error.to_string())?;
        let parse = image.parse_timings.total;
        let decoded = image
            .decode_u16_with_workers(&|| false, workers)
            .map_err(|error| error.to_string())?;
        let timing = DngDecodeTiming {
            parse,
            pixel_unpack: decoded.timings.pixel_unpack,
            linearization: decoded.timings.linearization,
        };
        Ok((sample_checksum(&decoded.pixels), timing))
    }

    /// Wall-time breakdown of one [`decode_dng_pixels_timed`] call.
    #[derive(Debug, Clone, Copy)]
    pub struct DngDecodeTiming {
        /// TIFF/DNG parse (sequential).
        pub parse: std::time::Duration,
        /// Segment decode and mosaic assembly (parallel over segments).
        pub pixel_unpack: std::time::Duration,
        /// Linearization-table application (sequential; zero without a table).
        pub linearization: std::time::Duration,
    }

    fn encode_segments(
        width: usize,
        height: usize,
        layout: SegmentLayout,
        compression: SyntheticCompression,
        samples: &[u16],
    ) -> Vec<Vec<u8>> {
        let mut payloads = Vec::new();
        match layout {
            SegmentLayout::Strips { rows_per_strip } => {
                let rows_per_strip = usize::try_from(rows_per_strip).expect("rows per strip");
                let mut first_row = 0;
                while first_row < height {
                    let rows = rows_per_strip.min(height - first_row);
                    let strip = &samples[first_row * width..(first_row + rows) * width];
                    payloads.push(encode_segment(compression, width, rows, strip));
                    first_row += rows;
                }
            }
            SegmentLayout::Tiles {
                tile_width,
                tile_height,
            } => {
                let tile_width = usize::try_from(tile_width).expect("tile width");
                let tile_height = usize::try_from(tile_height).expect("tile height");
                let tile_columns = width.div_ceil(tile_width);
                let tile_rows = height.div_ceil(tile_height);
                for tile_y in 0..tile_rows {
                    for tile_x in 0..tile_columns {
                        let first_x = tile_x * tile_width;
                        let first_y = tile_y * tile_height;
                        // Edge tiles are padded with zeros to the full tile
                        // geometry, as the DNG spec requires.
                        let mut tile = vec![0_u16; tile_width * tile_height];
                        for row in 0..tile_height.min(height - first_y) {
                            let source = (first_y + row) * width + first_x;
                            let length = tile_width.min(width - first_x);
                            tile[row * tile_width..row * tile_width + length]
                                .copy_from_slice(&samples[source..source + length]);
                        }
                        payloads.push(encode_segment(compression, tile_width, tile_height, &tile));
                    }
                }
            }
        }
        payloads
    }

    fn encode_segment(
        compression: SyntheticCompression,
        width: usize,
        height: usize,
        samples: &[u16],
    ) -> Vec<u8> {
        match compression {
            SyntheticCompression::Uncompressed16 => {
                let mut encoded = Vec::with_capacity(samples.len() * 2);
                for &sample in samples {
                    encoded.extend_from_slice(&sample.to_le_bytes());
                }
                encoded
            }
            SyntheticCompression::LosslessJpeg12 => encode_lossless_jpeg(
                u16::try_from(width).expect("lossless-JPEG frame width"),
                u16::try_from(height).expect("lossless-JPEG frame height"),
                1,
                samples,
            ),
        }
    }

    /// IFD entry value: either inline bytes or a placeholder resolved to a
    /// data-area offset during assembly.
    enum Value {
        Inline([u8; 4]),
        Model,
        Offsets,
        Counts,
    }

    /// Assembles the TIFF container around pre-encoded segment payloads.
    /// Layout: header, one IFD, out-of-line tag values, then segment data.
    #[allow(clippy::too_many_lines)]
    fn assemble_dng(
        width: u32,
        height: u32,
        bits_per_sample: u16,
        compression_code: u16,
        layout: SegmentLayout,
        payloads: &[Vec<u8>],
    ) -> Vec<u8> {
        const FIELD_BYTE: u16 = 1;
        const FIELD_ASCII: u16 = 2;
        const FIELD_SHORT: u16 = 3;
        const FIELD_LONG: u16 = 4;

        let model: &[u8] = b"Rrrah Synthetic DNG\0";
        let segment_count = payloads.len();

        let short = |value: u16| Value::Inline(u32::from(value).to_le_bytes());
        let long = |value: u32| Value::Inline(value.to_le_bytes());
        let segment_count_u32 = u32::try_from(segment_count).expect("fixture segment count");
        let model_len = u32::try_from(model.len()).expect("fixture model length");
        let mut entries: Vec<(u16, u16, u32, Value)> = vec![
            (256, FIELD_LONG, 1, long(width)),              // ImageWidth
            (257, FIELD_LONG, 1, long(height)),             // ImageLength
            (258, FIELD_SHORT, 1, short(bits_per_sample)),  // BitsPerSample
            (259, FIELD_SHORT, 1, short(compression_code)), // Compression
            (262, FIELD_SHORT, 1, short(32_803)),           // PhotometricInterpretation (CFA)
            (274, FIELD_SHORT, 1, short(1)),                // Orientation
            (277, FIELD_SHORT, 1, short(1)),                // SamplesPerPixel
        ];
        match layout {
            SegmentLayout::Strips { rows_per_strip } => {
                entries.push((273, FIELD_LONG, segment_count_u32, Value::Offsets));
                entries.push((278, FIELD_LONG, 1, long(rows_per_strip)));
                entries.push((279, FIELD_LONG, segment_count_u32, Value::Counts));
            }
            SegmentLayout::Tiles {
                tile_width,
                tile_height,
            } => {
                entries.push((322, FIELD_LONG, 1, long(tile_width)));
                entries.push((323, FIELD_LONG, 1, long(tile_height)));
                entries.push((324, FIELD_LONG, segment_count_u32, Value::Offsets));
                entries.push((325, FIELD_LONG, segment_count_u32, Value::Counts));
            }
        }
        entries.push((33_421, FIELD_SHORT, 2, Value::Inline([2, 0, 2, 0]))); // CFARepeatPatternDim
        entries.push((33_422, FIELD_BYTE, 4, Value::Inline([0, 1, 1, 2]))); // CFAPattern
        entries.push((50_706, FIELD_BYTE, 4, Value::Inline([1, 7, 1, 0]))); // DNGVersion
        entries.push((50_707, FIELD_BYTE, 4, Value::Inline([1, 1, 0, 0]))); // DNGBackwardVersion
        entries.push((50_708, FIELD_ASCII, model_len, Value::Model)); // UniqueCameraModel
        entries.sort_by_key(|entry| entry.0);

        // Compute positions: IFD at 8, then out-of-line values, then payloads.
        let align = |cursor: usize| cursor.div_ceil(2) * 2;
        let ifd_size = 2 + entries.len() * 12 + 4;
        let mut cursor = align(8 + ifd_size);
        let model_offset = cursor;
        cursor = align(cursor + model.len());
        let inline_arrays = segment_count == 1;
        let offsets_offset = if inline_arrays {
            None
        } else {
            let offset = cursor;
            cursor = align(cursor + segment_count * 4);
            Some(offset)
        };
        let counts_offset = if inline_arrays {
            None
        } else {
            let offset = cursor;
            cursor = align(cursor + segment_count * 4);
            Some(offset)
        };
        let mut segment_offsets = Vec::with_capacity(segment_count);
        for payload in payloads {
            segment_offsets.push(u32::try_from(cursor).expect("fixture segment offset"));
            cursor = align(cursor + payload.len());
        }

        let mut bytes = vec![0_u8; cursor];
        bytes[..8].copy_from_slice(&[b'I', b'I', 42, 0, 8, 0, 0, 0]);
        bytes[model_offset..model_offset + model.len()].copy_from_slice(model);
        if let Some(offset) = offsets_offset {
            for (index, &segment_offset) in segment_offsets.iter().enumerate() {
                bytes[offset + index * 4..offset + index * 4 + 4]
                    .copy_from_slice(&segment_offset.to_le_bytes());
            }
        }
        if let Some(offset) = counts_offset {
            for (index, payload) in payloads.iter().enumerate() {
                let count = u32::try_from(payload.len()).expect("fixture segment size");
                bytes[offset + index * 4..offset + index * 4 + 4].copy_from_slice(&count.to_le_bytes());
            }
        }
        for (payload, &offset) in payloads.iter().zip(&segment_offsets) {
            let start = usize::try_from(offset).expect("fixture segment offset");
            bytes[start..start + payload.len()].copy_from_slice(payload);
        }

        // Write the IFD entries.
        let entry_count = u16::try_from(entries.len()).expect("fixture entry count");
        bytes[8..10].copy_from_slice(&entry_count.to_le_bytes());
        for (index, (tag, field_type, count, value)) in entries.iter().enumerate() {
            let start = 10 + index * 12;
            bytes[start..start + 2].copy_from_slice(&tag.to_le_bytes());
            bytes[start + 2..start + 4].copy_from_slice(&field_type.to_le_bytes());
            bytes[start + 4..start + 8].copy_from_slice(&count.to_le_bytes());
            let offset_u32 = |offset: usize| u32::try_from(offset).expect("fixture value offset");
            let value_bytes = match value {
                Value::Inline(inline) => *inline,
                Value::Model => offset_u32(model_offset).to_le_bytes(),
                Value::Offsets => match offsets_offset {
                    Some(offset) => offset_u32(offset).to_le_bytes(),
                    None => segment_offsets[0].to_le_bytes(),
                },
                Value::Counts => match counts_offset {
                    Some(offset) => offset_u32(offset).to_le_bytes(),
                    None => u32::try_from(payloads[0].len())
                        .expect("fixture segment size")
                        .to_le_bytes(),
                },
            };
            bytes[start + 8..start + 12].copy_from_slice(&value_bytes);
        }
        let next = 10 + entries.len() * 12;
        bytes[next..next + 4].copy_from_slice(&0_u32.to_le_bytes()); // no next IFD
        bytes
    }

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

    fn jpeg_marker(output: &mut Vec<u8>, code: u8) {
        output.extend_from_slice(&[0xff, code]);
    }

    fn jpeg_segment(output: &mut Vec<u8>, code: u8, payload: &[u8]) {
        jpeg_marker(output, code);
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

    fn select_predictor(selection: u8, left: i32, above: i32, upper_left: i32) -> i32 {
        match selection {
            1 => left,
            2 => above,
            3 => upper_left,
            4 => left + above - upper_left,
            5 => left + ((above - upper_left) >> 1),
            6 => above + ((left - upper_left) >> 1),
            7 => (left + above) >> 1,
            _ => unreachable!(),
        }
    }

    /// FNV-1a over the little-endian sample bytes; used to assert
    /// bit-identical output across decoder revisions.
    fn sample_checksum(samples: &[u16]) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for &sample in samples {
            for byte in sample.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_raw_sub_ifd_and_parses_required_metadata() {
        let bytes = synthetic_sub_ifd_dng();
        let image = parse(&bytes).unwrap();

        assert_eq!((image.width, image.height), (3, 2));
        assert_eq!(image.stored_bits_per_sample, 8);
        assert_eq!(image.output_bits_per_sample, 16);
        assert_eq!(image.metadata.make, "Clean");
        assert_eq!(image.metadata.model, "Clean Camera");
        assert_eq!(image.metadata.camera_model, "Clean Camera");
        assert_eq!(image.metadata.orientation, Orientation::Rotate90);
        assert_eq!(
            image.metadata.cfa.cells,
            [CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue]
        );
        assert_eq!(image.metadata.black_level.values, [64.0, 65.0, 66.0, 67.0]);
        assert_eq!(image.metadata.white_level, [4095]);
        assert_eq!(
            image.metadata.active_area,
            Rect {
                top: 0,
                left: 0,
                bottom: 2,
                right: 3
            }
        );
        assert_eq!(image.metadata.as_shot_neutral, Some(vec![0.5, 1.0, 0.75]));
        assert_eq!(image.metadata.color_matrix_1.as_ref().unwrap().len(), 9);
        assert_eq!(image.metadata.color_matrix_2, None);
        assert_eq!(image.metadata.calibration_illuminant_1, Some(17));
        assert_eq!(image.metadata.calibration_illuminant_2, Some(21));
        assert_eq!(
            image.decode_u16(&|| false).unwrap().pixels,
            [0, 17, 34, 51, 68, 85]
        );
    }

    #[test]
    fn rejects_a_sub_ifd_chain() {
        let mut bytes = synthetic_sub_ifd_dng();
        let raw_ifd = 256_usize;
        let entry_count = usize::from(u16::from_le_bytes([bytes[raw_ifd], bytes[raw_ifd + 1]]));
        let next_offset = raw_ifd + 2 + entry_count * 12;
        bytes[next_offset..next_offset + 4].copy_from_slice(&8_u32.to_le_bytes());
        assert!(matches!(parse(&bytes), Err(DngError::SubIfdChain { .. })));
    }

    #[test]
    fn clamps_linearization_indices_to_the_last_table_entry() {
        let bytes = synthetic_sub_ifd_dng();
        let mut image = parse(&bytes).unwrap();
        image.metadata.linearization_table = Some(vec![10, 20]);

        assert_eq!(
            image.decode_u16(&|| false).unwrap().pixels,
            [10, 20, 20, 20, 20, 20]
        );
    }

    #[test]
    fn cancellation_stops_before_pixel_allocation() {
        let bytes = synthetic_sub_ifd_dng();
        let image = parse(&bytes).unwrap();

        assert!(matches!(
            image.decode_u16(&|| true),
            Err(DngError::Cancelled { row: 0 })
        ));
    }

    /// Deterministic photo-like content: smooth gradient plus a mild
    /// pseudo-random component, so lossless-JPEG differences stay short.
    fn fixture_samples(width: usize, height: usize) -> Vec<u16> {
        let mut samples = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let base = 1024 + (x * 3 + y * 5) % 2048;
                let noise = (x * 31 + y * 17 + (x * y) % 23) % 64;
                samples.push(u16::try_from(base + noise).unwrap());
            }
        }
        samples
    }

    fn assert_bit_identical_across_workers(bytes: &[u8]) {
        let reference = parse(bytes)
            .unwrap()
            .decode_u16_with_workers(&|| false, 1)
            .unwrap()
            .pixels;
        for workers in [2, 3, 4, 7, 64] {
            let decoded = parse(bytes)
                .unwrap()
                .decode_u16_with_workers(&|| false, workers)
                .unwrap()
                .pixels;
            assert_eq!(decoded, reference, "workers={workers}");
        }
    }

    #[test]
    fn tiled_lossless_jpeg_is_bit_identical_across_workers() {
        let (width, height) = (800, 600);
        let bytes = bench_support::build_segmented_dng(
            width,
            height,
            bench_support::SegmentLayout::Tiles {
                tile_width: 128,
                tile_height: 128,
            },
            bench_support::SyntheticCompression::LosslessJpeg12,
            &fixture_samples(width as usize, height as usize),
        );
        let image = parse(&bytes).unwrap();
        assert_eq!((image.width, image.height), (width, height));
        assert_bit_identical_across_workers(&bytes);
    }

    #[test]
    fn stripped_lossless_jpeg_is_bit_identical_across_workers() {
        let (width, height) = (800, 600);
        let bytes = bench_support::build_segmented_dng(
            width,
            height,
            bench_support::SegmentLayout::Strips { rows_per_strip: 16 },
            bench_support::SyntheticCompression::LosslessJpeg12,
            &fixture_samples(width as usize, height as usize),
        );
        assert_bit_identical_across_workers(&bytes);
    }

    #[test]
    fn tiled_uncompressed_is_bit_identical_across_workers() {
        let (width, height) = (800, 600);
        let bytes = bench_support::build_segmented_dng(
            width,
            height,
            bench_support::SegmentLayout::Tiles {
                tile_width: 128,
                tile_height: 128,
            },
            bench_support::SyntheticCompression::Uncompressed16,
            &fixture_samples(width as usize, height as usize),
        );
        assert_bit_identical_across_workers(&bytes);
    }

    #[test]
    fn stripped_uncompressed_is_bit_identical_across_workers() {
        let (width, height) = (800, 600);
        let bytes = bench_support::build_segmented_dng(
            width,
            height,
            bench_support::SegmentLayout::Strips { rows_per_strip: 16 },
            bench_support::SyntheticCompression::Uncompressed16,
            &fixture_samples(width as usize, height as usize),
        );
        assert_bit_identical_across_workers(&bytes);
    }

    #[test]
    fn parallel_decode_reports_cancellation() {
        let (width, height) = (800, 600);
        let bytes = bench_support::build_segmented_dng(
            width,
            height,
            bench_support::SegmentLayout::Tiles {
                tile_width: 128,
                tile_height: 128,
            },
            bench_support::SyntheticCompression::LosslessJpeg12,
            &fixture_samples(width as usize, height as usize),
        );
        let image = parse(&bytes).unwrap();
        assert!(matches!(
            image.decode_u16_with_workers(&|| true, 4),
            Err(DngError::Cancelled { .. })
        ));
    }

    #[test]
    fn decode_hook_checksum_matches_across_workers() {
        let (width, height) = (800, 600);
        let bytes = bench_support::build_segmented_dng(
            width,
            height,
            bench_support::SegmentLayout::Strips { rows_per_strip: 16 },
            bench_support::SyntheticCompression::LosslessJpeg12,
            &fixture_samples(width as usize, height as usize),
        );
        let reference = bench_support::decode_dng_pixels_with_workers(&bytes, 1).unwrap();
        for workers in [2, 4, 7] {
            assert_eq!(
                bench_support::decode_dng_pixels_with_workers(&bytes, workers).unwrap(),
                reference,
                "workers={workers}",
            );
        }
    }

    #[allow(clippy::too_many_lines)]
    fn synthetic_sub_ifd_dng() -> Vec<u8> {
        const ROOT_IFD: usize = 8;
        const RAW_IFD: usize = 256;
        const MODEL: usize = 768;
        const BLACK: usize = 800;
        const ACTIVE: usize = 816;
        const LINEAR: usize = 832;
        const MATRIX: usize = 1_344;
        const NEUTRAL: usize = 1_416;
        const PIXELS: usize = 1_440;

        let mut bytes = vec![0_u8; 1_446];
        bytes[..8].copy_from_slice(&[b'I', b'I', 42, 0, 8, 0, 0, 0]);
        let root_entries = [
            (TAG_NEW_SUBFILE_TYPE, 4, 1, 1_u32.to_le_bytes()),
            (TAG_IMAGE_WIDTH, 4, 1, 1_u32.to_le_bytes()),
            (TAG_IMAGE_LENGTH, 4, 1, 1_u32.to_le_bytes()),
            (TAG_PHOTOMETRIC_INTERPRETATION, 3, 1, 2_u32.to_le_bytes()),
            (TAG_ORIENTATION, 3, 1, 6_u32.to_le_bytes()),
            (TAG_SUB_IFDS, 4, 1, u32::try_from(RAW_IFD).unwrap().to_le_bytes()),
            (TAG_DNG_VERSION, 1, 4, [1, 7, 1, 0]),
            (TAG_DNG_BACKWARD_VERSION, 1, 4, [1, 1, 0, 0]),
            (
                TAG_UNIQUE_CAMERA_MODEL,
                2,
                13,
                u32::try_from(MODEL).unwrap().to_le_bytes(),
            ),
            (
                TAG_COLOR_MATRIX_1,
                10,
                9,
                u32::try_from(MATRIX).unwrap().to_le_bytes(),
            ),
            (
                TAG_AS_SHOT_NEUTRAL,
                5,
                3,
                u32::try_from(NEUTRAL).unwrap().to_le_bytes(),
            ),
            (TAG_CALIBRATION_ILLUMINANT_1, 3, 1, 17_u32.to_le_bytes()),
            (TAG_CALIBRATION_ILLUMINANT_2, 3, 1, 21_u32.to_le_bytes()),
        ];
        write_ifd(&mut bytes, ROOT_IFD, &root_entries, 0);
        bytes[MODEL..MODEL + 13].copy_from_slice(b"Clean Camera\0");

        let raw_entries = [
            (TAG_NEW_SUBFILE_TYPE, 4, 1, 0_u32.to_le_bytes()),
            (TAG_IMAGE_WIDTH, 4, 1, 3_u32.to_le_bytes()),
            (TAG_IMAGE_LENGTH, 4, 1, 2_u32.to_le_bytes()),
            (TAG_BITS_PER_SAMPLE, 3, 1, 8_u32.to_le_bytes()),
            (TAG_COMPRESSION, 3, 1, 1_u32.to_le_bytes()),
            (
                TAG_PHOTOMETRIC_INTERPRETATION,
                3,
                1,
                u32::try_from(PHOTOMETRIC_CFA).unwrap().to_le_bytes(),
            ),
            (
                TAG_STRIP_OFFSETS,
                4,
                1,
                u32::try_from(PIXELS).unwrap().to_le_bytes(),
            ),
            (TAG_SAMPLES_PER_PIXEL, 3, 1, 1_u32.to_le_bytes()),
            (TAG_ROWS_PER_STRIP, 4, 1, 2_u32.to_le_bytes()),
            (TAG_STRIP_BYTE_COUNTS, 4, 1, 6_u32.to_le_bytes()),
            (TAG_PLANAR_CONFIGURATION, 3, 1, 1_u32.to_le_bytes()),
            (TAG_CFA_REPEAT_PATTERN_DIM, 3, 2, [2, 0, 2, 0]),
            (TAG_CFA_PATTERN, 1, 4, [0, 1, 1, 2]),
            (TAG_BLACK_LEVEL_REPEAT_DIM, 3, 2, [2, 0, 2, 0]),
            (TAG_BLACK_LEVEL, 3, 4, u32::try_from(BLACK).unwrap().to_le_bytes()),
            (TAG_WHITE_LEVEL, 3, 1, 4095_u32.to_le_bytes()),
            (
                TAG_ACTIVE_AREA,
                4,
                4,
                u32::try_from(ACTIVE).unwrap().to_le_bytes(),
            ),
            (
                TAG_LINEARIZATION_TABLE,
                3,
                256,
                u32::try_from(LINEAR).unwrap().to_le_bytes(),
            ),
        ];
        write_ifd(&mut bytes, RAW_IFD, &raw_entries, 0);
        for (index, value) in [64_u16, 65, 66, 67].into_iter().enumerate() {
            bytes[BLACK + index * 2..BLACK + index * 2 + 2].copy_from_slice(&value.to_le_bytes());
        }
        for (index, value) in [0_u32, 0, 2, 3].into_iter().enumerate() {
            bytes[ACTIVE + index * 4..ACTIVE + index * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        for index in 0..256_u16 {
            let value = index * 17;
            let offset = LINEAR + usize::from(index) * 2;
            bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        for index in 0..9 {
            let offset = MATRIX + index * 8;
            let numerator = i32::from(index % 4 == 0);
            bytes[offset..offset + 4].copy_from_slice(&numerator.to_le_bytes());
            bytes[offset + 4..offset + 8].copy_from_slice(&1_i32.to_le_bytes());
        }
        for (index, (numerator, denominator)) in [(1_u32, 2_u32), (1, 1), (3, 4)].into_iter().enumerate() {
            let offset = NEUTRAL + index * 8;
            bytes[offset..offset + 4].copy_from_slice(&numerator.to_le_bytes());
            bytes[offset + 4..offset + 8].copy_from_slice(&denominator.to_le_bytes());
        }
        bytes[PIXELS..PIXELS + 6].copy_from_slice(&[0, 1, 2, 3, 4, 5]);
        bytes
    }

    fn write_ifd(bytes: &mut [u8], offset: usize, entries: &[(u16, u16, u32, [u8; 4])], next: u32) {
        bytes[offset..offset + 2].copy_from_slice(
            &u16::try_from(entries.len())
                .expect("synthetic entry count")
                .to_le_bytes(),
        );
        for (index, &(tag, field_type, count, value)) in entries.iter().enumerate() {
            let start = offset + 2 + index * 12;
            bytes[start..start + 2].copy_from_slice(&tag.to_le_bytes());
            bytes[start + 2..start + 4].copy_from_slice(&field_type.to_le_bytes());
            bytes[start + 4..start + 8].copy_from_slice(&count.to_le_bytes());
            bytes[start + 8..start + 12].copy_from_slice(&value);
        }
        let next_offset = offset + 2 + entries.len() * 12;
        bytes[next_offset..next_offset + 4].copy_from_slice(&next.to_le_bytes());
    }
}
