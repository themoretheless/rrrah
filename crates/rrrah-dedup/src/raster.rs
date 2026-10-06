//! Adapter to rrrah's oriented decoded raster and color pipeline.

use crate::{linear::LinearRgbaView, pixels::PixelError};
use rrrah_core::{DecodedRaster, MemoryBudget, RasterError, RasterPixels};

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error(transparent)]
    Raster(#[from] RasterError),
    #[error(transparent)]
    Pixels(#[from] PixelError),
    #[error("unsupported normalized pixel representation")]
    Representation,
}

/// Owned normalized data, retaining HDR and source frame-selection metadata.
/// Exact equality applies to the selected decoded frame only.
#[derive(Debug)]
pub struct NormalizedRaster {
    linear: DecodedRaster,
    max_pixels: u64,
}

impl NormalizedRaster {
    /// Normalize through rrrah-core; ICC/unknown primaries return explicit errors.
    /// The budget admits conversion allocations. Caller also owns source memory.
    ///
    /// # Errors
    /// Returns resource, color, sample or cancellation errors. The core conversion
    /// checks cancellation at least every 4096 pixels.
    pub fn new(
        source: &DecodedRaster,
        max_pixels: u64,
        budget: &MemoryBudget,
        cancel: impl Fn() -> bool,
    ) -> Result<Self, AdapterError> {
        if cancel() {
            return Err(PixelError::Cancelled.into());
        }
        if u64::from(source.width()) * u64::from(source.height()) > max_pixels {
            return Err(PixelError::Budget.into());
        }
        let linear = source.to_linear_srgb_with_budget_and_cancel(Some(budget), &cancel)?;
        let normalized = Self { linear, max_pixels };
        normalized.view(&cancel)?;
        Ok(normalized)
    }

    /// # Errors
    /// Returns cancellation or invalid normalized storage.
    pub fn view(&self, cancel: impl Fn() -> bool) -> Result<LinearRgbaView<'_>, AdapterError> {
        let RasterPixels::Rgba32Float(samples) = self.linear.pixels() else {
            return Err(AdapterError::Representation);
        };
        Ok(LinearRgbaView::new(
            self.linear.width(),
            self.linear.height(),
            samples,
            self.max_pixels,
            cancel,
        )?)
    }

    pub fn image_index(&self) -> usize {
        self.linear.image_index()
    }

    pub fn image_count(&self) -> usize {
        self.linear.image_count()
    }

    /// Canonical selected-frame candidate key, including scale and cursor hotspot.
    /// Digest equality requires direct confirmation; container count/index are excluded.
    ///
    /// # Errors
    /// Invalid normalized storage or cancellation, without a partial digest.
    pub fn selected_frame_digest(&self, cancel: impl Fn() -> bool) -> Result<[u8; 32], AdapterError> {
        let pixels = self.view(&cancel)?.pixel_digest(&cancel)?;
        let mut hash = blake3::Hasher::new();
        hash.update(b"rrrah-selected-frame-v1");
        hash.update(&pixels);
        hash.update(&self.linear.sample_scale().to_bits().to_le_bytes());
        match self.linear.hotspot() {
            None => {
                hash.update(&[0]);
            }
            Some((x, y)) => {
                hash.update(&[1]);
                hash.update(&x.to_le_bytes());
                hash.update(&y.to_le_bytes());
            }
        }
        if cancel() {
            return Err(PixelError::Cancelled.into());
        }
        Ok(*hash.finalize().as_bytes())
    }

    /// Confirm selected-frame equality, including sample scale and cursor hotspot.
    /// Container count/index do not turn this result into whole-file equality.
    ///
    /// # Errors
    /// Returns invalid storage or cancellation without an equality claim.
    pub fn same_selected_frame(&self, other: &Self, cancel: impl Fn() -> bool) -> Result<bool, AdapterError> {
        if cancel() {
            return Err(PixelError::Cancelled.into());
        }
        if self.linear.sample_scale().to_bits() != other.linear.sample_scale().to_bits()
            || self.linear.hotspot() != other.linear.hotspot()
        {
            return Ok(false);
        }
        Ok(self.view(&cancel)?.same_pixels(&other.view(&cancel)?, cancel)?)
    }
}
