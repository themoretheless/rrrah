//! Canonical equality for explicitly normalized linear-sRGB straight-alpha data.
//! HDR values are retained. Unknown primaries and ICC samples must be transformed
//! by the decoder before constructing this view; no implicit tone map is used.

use crate::pixels::PixelError;

/// Borrowed, validated RGBA32 linear-sRGB pixels. Equality remains exact after
/// color normalization: approximate color matches require separate evidence.
#[derive(Debug, Clone, Copy)]
pub struct LinearRgbaView<'a> {
    width: u32,
    height: u32,
    samples: &'a [f32],
}

impl<'a> LinearRgbaView<'a> {
    /// Extract the same feature recipe directly from normalized linear samples.
    /// HDR/negative values are retained, with no tone mapping or quantization.
    ///
    /// # Errors
    /// Returns cancellation without a partial fingerprint.
    pub fn fingerprint(&self, cancel: impl Fn() -> bool) -> Result<crate::pixels::Fingerprint, PixelError> {
        crate::pixels::fingerprint_from_linear(
            self.width,
            self.height,
            |x, y| {
                self.pixel(x, y)
                    .map(|rgba| rgba.map(|bits| f64::from(f32::from_bits(bits))))
            },
            cancel,
        )
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Read canonical scene-linear RGBA without clipping HDR or negative RGB.
    /// Returns `None` outside the validated dimensions.
    pub fn rgba(&self, x: u32, y: u32) -> Option<[f32; 4]> {
        self.pixel(x, y).map(|pixel| pixel.map(f32::from_bits))
    }

    pub(crate) fn pixel(&self, x: u32, y: u32) -> Option<[u32; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x)).ok()?;
        self.samples.as_chunks::<4>().0.get(index).map(canonical)
    }

    /// Validate dimensions, allocation budget, finite samples and alpha.
    ///
    /// # Errors
    /// Returns layout/budget errors or cancellation. Non-finite visible RGB and
    /// alpha outside [0,1] are invalid. Invisible RGB is deliberately ignored.
    pub fn new(
        width: u32,
        height: u32,
        samples: &'a [f32],
        max_pixels: u64,
        cancel: impl Fn() -> bool,
    ) -> Result<Self, PixelError> {
        let pixels = u64::from(width) * u64::from(height);
        if width == 0 || height == 0 {
            return Err(PixelError::Layout);
        }
        if pixels > max_pixels {
            return Err(PixelError::Budget);
        }
        if pixels.checked_mul(4).and_then(|n| usize::try_from(n).ok()) != Some(samples.len()) {
            return Err(PixelError::Layout);
        }
        for (index, rgba) in samples.as_chunks::<4>().0.iter().enumerate() {
            if index.is_multiple_of(4096) && cancel() {
                return Err(PixelError::Cancelled);
            }
            if !rgba[3].is_finite()
                || !(0.0..=1.0).contains(&rgba[3])
                || (rgba[3] != 0.0 && rgba[..3].iter().any(|v| !v.is_finite()))
            {
                return Err(PixelError::Layout);
            }
        }
        if cancel() {
            return Err(PixelError::Cancelled);
        }
        Ok(Self {
            width,
            height,
            samples,
        })
    }

    /// Versioned digest; signed zero and invisible RGB are canonicalized.
    ///
    /// # Errors
    /// Returns cancellation instead of a partial digest.
    pub fn pixel_digest(&self, cancel: impl Fn() -> bool) -> Result<[u8; 32], PixelError> {
        let mut hash = blake3::Hasher::new();
        hash.update(b"rrrah-linear-srgb-rgba32-v1");
        hash.update(&self.width.to_le_bytes());
        hash.update(&self.height.to_le_bytes());
        for (index, rgba) in self.samples.as_chunks::<4>().0.iter().enumerate() {
            if index.is_multiple_of(4096) && cancel() {
                return Err(PixelError::Cancelled);
            }
            for bits in canonical(rgba) {
                hash.update(&bits.to_le_bytes());
            }
        }
        if cancel() {
            return Err(PixelError::Cancelled);
        }
        Ok(*hash.finalize().as_bytes())
    }

    /// Confirm canonical equality directly, independently of hash collisions.
    ///
    /// # Errors
    /// Returns cancellation without making an equality claim.
    pub fn same_pixels(&self, other: &Self, cancel: impl Fn() -> bool) -> Result<bool, PixelError> {
        if cancel() {
            return Err(PixelError::Cancelled);
        }
        if (self.width, self.height) != (other.width, other.height) {
            return Ok(false);
        }
        for (index, (a, b)) in self
            .samples
            .as_chunks::<4>()
            .0
            .iter()
            .zip(other.samples.as_chunks::<4>().0)
            .enumerate()
        {
            if index.is_multiple_of(4096) && cancel() {
                return Err(PixelError::Cancelled);
            }
            if canonical(a) != canonical(b) {
                return Ok(false);
            }
        }
        if cancel() {
            return Err(PixelError::Cancelled);
        }
        Ok(true)
    }
}

fn canonical(rgba: &[f32; 4]) -> [u32; 4] {
    if rgba[3] == 0.0 {
        [0; 4]
    } else {
        rgba.map(|v| if v == 0.0 { 0 } else { v.to_bits() })
    }
}
