//! Domain types and deterministic image math for Rrrah.
//!
//! This crate intentionally knows nothing about a particular RAW decoder, GPU
//! API, cache implementation, or windowing toolkit.
#![allow(clippy::missing_errors_doc)]

mod color;
mod frame;
mod geometry;
mod recipe;

pub use color::{
    BRADFORD, SRGB_TO_XYZ_D65, aces_fitted, apply_3x3, apply_exposure, bradford_adaptation,
    camera_to_linear_srgb, invert_3x3, multiply_3x3,
};
pub use frame::{
    CfaColor, CfaPattern, DecodedMosaic, FrameError, LevelGrid, Orientation, Photometric, RawMetadata,
    WhiteLevel,
};
pub use geometry::Rect;
pub use recipe::{
    DECODE_CROP_AS_METADATA, DECODE_FULL_SENSOR_RAW, DECODE_IMAGE_INDEX_IN_KEY, DECODE_INTEGER_U16,
    DECODE_SENSOR_COORDINATES, KNOWN_MOSAIC_DECODE_FLAGS, MOSAIC_RECIPE_MANIFEST_BYTES,
    MOSAIC_RECIPE_MANIFEST_VERSION_V1, ManifestError, MosaicRecipeManifest,
    REQUIRED_SENSOR_MOSAIC_DECODE_FLAGS, SENSOR_MOSAIC_ARTIFACT_KIND_CODE,
};

/// Frozen legacy-v2 cache ABI. New semantic recipes use
/// `MosaicRecipeManifest`; this value must never be bumped in place.
pub const LEGACY_V2_CACHE_ABI: u32 = 1;
