//! Domain types and deterministic image math for Rrrah.
//!
//! This crate intentionally knows nothing about a particular RAW decoder, GPU
//! API, cache implementation, or windowing toolkit.
#![allow(clippy::missing_errors_doc)]

mod color;
mod frame;
mod geometry;

pub use color::{
    BRADFORD, SRGB_TO_XYZ_D65, aces_fitted, apply_3x3, apply_exposure, bradford_adaptation,
    camera_to_linear_srgb, invert_3x3, multiply_3x3,
};
pub use frame::{
    CfaColor, CfaPattern, DecodedMosaic, FrameError, LevelGrid, Orientation, Photometric, RawMetadata,
    WhiteLevel,
};
pub use geometry::Rect;

/// Bump whenever a persisted decoded mosaic becomes semantically incompatible.
pub const DECODE_CACHE_ABI: u32 = 1;
