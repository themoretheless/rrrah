//! Byte-accounted memory caching and an atomic decoded-mosaic disk cache.
#![allow(
    clippy::missing_errors_doc,
    clippy::format_collect,
    clippy::cast_possible_truncation
)]

mod disk;
mod weighted_lru;

pub use disk::{CacheError, CacheKey, CacheLoad, DiskMosaicCache, SourceFingerprint};
pub use weighted_lru::WeightedLru;
