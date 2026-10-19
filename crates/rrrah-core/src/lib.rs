//! Domain types and deterministic image math for Rrrah.
//!
//! This crate intentionally knows nothing about a particular RAW decoder, GPU
//! API, cache implementation, or windowing toolkit.
#![allow(clippy::missing_errors_doc)]

pub use rrrah_memory::{BufferError, MemoryBudget, MutableBuffer, PixelBuffer, Reservation, SharedBuffer};

pub use rrrah_color::{
    BRADFORD, CameraProfileError, DNG_ILLUMINANT_D65, DngColorMatrix, GREEN_PLANE_RELATIVE_TOLERANCE,
    GreenPlane, RgbChromaticities, SRGB_TO_XYZ_D65, SRGB_TO_XYZ_D65_F64, WB_LUMINANCE_WEIGHTS, XYZ_WHITE_D65,
    aces_fitted, aces_fitted_f64, aces_tone_map_rgb, aces_tone_map_rgb_f64, apply_3x3, apply_3x3_f64,
    apply_exposure, apply_exposure_f64, bradford_adaptation, bradford_adaptation_f64, camera_to_linear_srgb,
    camera_to_linear_srgb_precise, camera4_to_linear_srgb_precise, diagnose_green_planes, display_wb_gains,
    dng_illuminant_white, green_relative_wb_gains, invert_3x3, invert_3x3_f64, linear_rgb_to_srgb_from_xy,
    linear_rgb_to_srgb_with_adopted_neutral_from_xy, luminance_normalize_wb_gains, multiply_3x3,
    multiply_3x3_f64, reconstruct_clipped_camera_highlight, reconstruct_clipped_camera_highlight_f64,
    resolve_dng_neutral_matrix, select_dng_xyz_to_camera, xy_chromaticity_to_xyz,
};

pub use rrrah_demosaic::{
    CfaColor, CfaError, CfaPattern, DemosaicAlgorithm, DemosaicConfig, FrameError, demosaic_bayer_superpixel,
    demosaic_bayer_superpixel_f64, demosaic_f32, demosaic_f64, demosaic_u16, demosaic_u16_f64,
};

pub use rrrah_develop::{
    self as develop, DECODE_CROP_AS_METADATA, DECODE_FULL_SENSOR_RAW, DECODE_IMAGE_INDEX_IN_KEY,
    DECODE_INTEGER_U16, DECODE_SENSOR_COORDINATES, DecodedMosaic, DecodedRaster, KNOWN_MOSAIC_DECODE_FLAGS,
    LevelGrid, MOSAIC_RECIPE_MANIFEST_BYTES, MOSAIC_RECIPE_MANIFEST_VERSION_V1, ManifestError,
    MosaicRecipeManifest, Orientation, Photometric, REQUIRED_SENSOR_MOSAIC_DECODE_FLAGS,
    RasterAlphaAssociation, RasterAnimationTiming, RasterColorSpace, RasterError, RasterPixels, RawMetadata,
    Rect, SENSOR_MOSAIC_ARTIFACT_KIND_CODE, ThumbnailError, WhiteLevel, bayer, edit, histogram, rgbe,
};

/// Frozen legacy-v2 cache ABI. New semantic recipes use
/// `MosaicRecipeManifest`; this value must never be bumped in place.
pub const LEGACY_V2_CACHE_ABI: u32 = 1;
