//! Explicit sRGB, straight-alpha RGBA8 input for normalized pixel evidence.
//!
//! Transparent RGB is ignored for pixel equality. Perceptual fingerprints use
//! area-averaged linear-light luminance over both black and white backgrounds;
//! two backgrounds retain alpha information. Equal perceptual hashes are only
//! candidates. The decoder adapter must normalize orientation/color first.

/// Included in every pixel digest and future cache recipe.
pub const PIXEL_RECIPE_VERSION: u32 = 1;
const EDGE: usize = 9;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PixelError {
    #[error("invalid RGBA8 dimensions or byte count")]
    Layout,
    #[error("pixel count exceeds the supplied resource budget")]
    Budget,
    #[error("operation cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Copy)]
pub struct Rgba8View<'a> {
    width: u32,
    height: u32,
    bytes: &'a [u8],
}

/// Fingerprints for the eight square symmetries. Each entry contains row/column
/// dHash against black, then row/column dHash against white.
#[derive(Debug, Clone, PartialEq)]
pub struct Fingerprint {
    pub variants: [[u64; 4]; 8],
    pub mean_linear_rgb: [f64; 3],
    pub luminance_stddev: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comparison {
    pub distance: u32,
    /// Rotation/reflection variant of the right image (identity = 0).
    pub right_transform: usize,
    /// Both inputs contain enough luminance variation for global hash evidence.
    /// This is an information check, not a calibrated duplicate decision.
    pub informative: bool,
}

impl Fingerprint {
    /// Compare to the best transform. Always expose low-information inputs.
    /// A caller must qualify thresholds with positive and negative fixtures.
    pub fn compare(&self, other: &Self, allow_transforms: bool) -> Comparison {
        let count = if allow_transforms { 8 } else { 1 };
        let (distance, right_transform) = (0..count)
            .map(|index| {
                let distance = self.variants[0]
                    .iter()
                    .zip(other.variants[index])
                    .map(|(&a, b)| (a ^ b).count_ones())
                    .sum();
                (distance, index)
            })
            .min()
            .unwrap_or((0, 0));
        Comparison {
            distance,
            right_transform,
            informative: self.luminance_stddev > 0.001 && other.luminance_stddev > 0.001,
        }
    }
}

impl<'a> Rgba8View<'a> {
    /// Validate layout before any read; storage is borrowed and never copied.
    ///
    /// # Errors
    /// Rejects empty/overflowing layout, wrong byte count, or budget overflow.
    pub fn new(width: u32, height: u32, bytes: &'a [u8], max_pixels: u64) -> Result<Self, PixelError> {
        let pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or(PixelError::Layout)?;
        if width == 0 || height == 0 {
            return Err(PixelError::Layout);
        }
        if pixels > max_pixels {
            return Err(PixelError::Budget);
        }
        let length = pixels
            .checked_mul(4)
            .and_then(|size| usize::try_from(size).ok())
            .ok_or(PixelError::Layout)?;
        if length != bytes.len() {
            return Err(PixelError::Layout);
        }
        Ok(Self { width, height, bytes })
    }

    /// Digest canonical pixels with dimensions and recipe version. Only RGB
    /// under alpha zero is canonicalized; visible RGB and alpha remain exact.
    ///
    /// # Errors
    /// Returns cancellation without yielding a partial digest.
    pub fn pixel_digest(&self, cancel: impl Fn() -> bool) -> Result<[u8; 32], PixelError> {
        let mut hash = blake3::Hasher::new();
        hash.update(b"rrrah-normalized-srgb-rgba8");
        hash.update(&PIXEL_RECIPE_VERSION.to_le_bytes());
        hash.update(&self.width.to_le_bytes());
        hash.update(&self.height.to_le_bytes());
        for (index, pixel) in self.bytes.as_chunks::<4>().0.iter().enumerate() {
            if index.is_multiple_of(4096) && cancel() {
                return Err(PixelError::Cancelled);
            }
            hash.update(if pixel[3] == 0 { &[0, 0, 0, 0] } else { pixel });
        }
        if cancel() {
            return Err(PixelError::Cancelled);
        }
        Ok(*hash.finalize().as_bytes())
    }

    /// Byte-confirm normalized pixel equality, independently of digest equality.
    ///
    /// # Errors
    /// Returns cancellation rather than an equality claim.
    pub fn same_pixels(&self, other: &Self, cancel: impl Fn() -> bool) -> Result<bool, PixelError> {
        if cancel() {
            return Err(PixelError::Cancelled);
        }
        if self.width != other.width || self.height != other.height {
            return Ok(false);
        }
        for (index, (a, b)) in self
            .bytes
            .as_chunks::<4>()
            .0
            .iter()
            .zip(other.bytes.as_chunks::<4>().0.iter())
            .enumerate()
        {
            if index.is_multiple_of(4096) && cancel() {
                return Err(PixelError::Cancelled);
            }
            if a[3] != b[3] || (a[3] != 0 && a[..3] != b[..3]) {
                return Ok(false);
            }
        }
        if cancel() {
            return Err(PixelError::Cancelled);
        }
        Ok(true)
    }

    /// Extract a bounded feature grid. Every source pixel contributes to its
    /// normalized footprint; large sources are not sparsely point-sampled.
    ///
    /// # Errors
    /// Returns cancellation at most 4096 source contributions after a request.
    pub fn fingerprint(&self, cancel: impl Fn() -> bool) -> Result<Fingerprint, PixelError> {
        fingerprint_from_linear(
            self.width,
            self.height,
            |x, y| {
                let index = usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x)).ok()?;
                let pixel = self.bytes.as_chunks::<4>().0.get(index)?;
                Some([
                    linear(pixel[0]),
                    linear(pixel[1]),
                    linear(pixel[2]),
                    f64::from(pixel[3]) / 255.0,
                ])
            },
            cancel,
        )
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
pub(crate) fn fingerprint_from_linear(
    width: u32,
    height: u32,
    sample: impl Fn(u32, u32) -> Option<[f64; 4]>,
    cancel: impl Fn() -> bool,
) -> Result<Fingerprint, PixelError> {
    if cancel() {
        return Err(PixelError::Cancelled);
    }
    let mut black = [0.0; EDGE * EDGE];
    let mut white = [0.0; EDGE * EDGE];
    let mut mean_rgb = [0.0; 3];
    let mut contributions = 0_usize;
    for y in 0..EDGE {
        for x in 0..EDGE {
            let left = x as f64 * f64::from(width) / EDGE as f64;
            let right = (x + 1) as f64 * f64::from(width) / EDGE as f64;
            let top = y as f64 * f64::from(height) / EDGE as f64;
            let bottom = (y + 1) as f64 * f64::from(height) / EDGE as f64;
            let area = (right - left) * (bottom - top);
            for sy in top.floor() as u32..(bottom.ceil() as u32).min(height) {
                for sx in left.floor() as u32..(right.ceil() as u32).min(width) {
                    if contributions.is_multiple_of(4096) && cancel() {
                        return Err(PixelError::Cancelled);
                    }
                    contributions += 1;
                    let weight = (right.min(f64::from(sx + 1)) - left.max(f64::from(sx)))
                        * (bottom.min(f64::from(sy + 1)) - top.max(f64::from(sy)))
                        / area;
                    let pixel = sample(sx, sy).ok_or(PixelError::Layout)?;
                    let alpha = pixel[3];
                    let rgb = [pixel[0], pixel[1], pixel[2]];
                    let luminance = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
                    black[y * EDGE + x] += luminance * alpha * weight;
                    white[y * EDGE + x] += (luminance * alpha + 1.0 - alpha) * weight;
                    for channel in 0..3 {
                        mean_rgb[channel] += rgb[channel] * alpha * weight / (EDGE * EDGE) as f64;
                    }
                }
            }
        }
    }
    if cancel() {
        return Err(PixelError::Cancelled);
    }
    let mean = black.iter().sum::<f64>() / (EDGE * EDGE) as f64;
    let variance = black.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (EDGE * EDGE) as f64;
    let mut variants = [[0; 4]; 8];
    for (transform, hashes) in variants.iter_mut().enumerate() {
        let b = difference_hash(&black, transform);
        let w = difference_hash(&white, transform);
        *hashes = [b[0], b[1], w[0], w[1]];
    }
    Ok(Fingerprint {
        variants,
        mean_linear_rgb: mean_rgb,
        luminance_stddev: variance.sqrt(),
    })
}

fn linear(value: u8) -> f64 {
    let normalized = f64::from(value) / 255.0;
    if normalized <= 0.04045 {
        normalized / 12.92
    } else {
        ((normalized + 0.055) / 1.055).powf(2.4)
    }
}

fn transformed(grid: &[f64; EDGE * EDGE], x: usize, y: usize, transform: usize) -> f64 {
    let last = EDGE - 1;
    let (x, y) = match transform {
        0 => (x, y),
        1 => (y, last - x),
        2 => (last - x, last - y),
        3 => (last - y, x),
        4 => (last - x, y),
        5 => (y, x),
        6 => (x, last - y),
        7 => (last - y, last - x),
        _ => unreachable!(),
    };
    grid[y * EDGE + x]
}

fn difference_hash(grid: &[f64; EDGE * EDGE], transform: usize) -> [u64; 2] {
    let mut hashes = [0_u64; 2];
    for y in 0..8 {
        for x in 0..8 {
            let current = transformed(grid, x, y, transform);
            let bit = 1 << (y * 8 + x);
            if current < transformed(grid, x + 1, y, transform) {
                hashes[0] |= bit;
            }
            if current < transformed(grid, x, y + 1, transform) {
                hashes[1] |= bit;
            }
        }
    }
    hashes
}
