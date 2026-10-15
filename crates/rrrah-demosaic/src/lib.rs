//! Exhaustive 64-bit (`f64`) and 32-bit (`f32`) CFA demosaicing and pre/post-conditioning filters for Rrrah.
//!
//! Includes 28 demosaicing algorithms covering 2×2 Bayer, 6×6 Fujifilm X-Trans, 4×4 Quad Bayer (TetraCell),
//! 6×6 Nona Bayer (Nonacell), 8×8 Hexadeca Bayer (ChameleonCell), CYGM / RGBW / RGBE, and arbitrary periodic CFAs.
#![allow(
    clippy::pedantic,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

pub mod ahd;
pub mod demosaic;
pub mod xtrans;

use rrrah_memory::MemoryBudget;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use demosaic::{
    DemosaicAlgorithm, DemosaicConfig, TileScratch, clustered_bayer, demosaic_bayer_superpixel,
    demosaic_bayer_superpixel_f64, demosaic_f32, demosaic_f64, demosaic_u16, demosaic_u16_f64,
    dispatch_demosaic, equilibrate_bayer_greens, generic_cfa, hexadeca_bayer, is_clustered_bayer,
    is_hexadeca_bayer, is_nona_bayer, is_quad_bayer, nona_bayer, quad_bayer, suppress_cfa_impulse_noise,
    suppress_chromatic_fringe, suppress_false_color, tiled_worker_count, uses_dual_frames,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CfaColor {
    Red = 0,
    Green = 1,
    Blue = 2,
    Cyan = 3,
    Magenta = 4,
    Yellow = 5,
    White = 6,
    Unknown = 255,
    /// Distinct emerald response plane of Sony RGBE sensors. Appended to retain
    /// existing serde enum indices; the cache wire code is explicitly 7.
    Emerald = 7,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CfaPattern {
    pub width: u8,
    pub height: u8,
    pub cells: Vec<CfaColor>,
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum FrameError {
    #[error("frame dimensions or allocation size overflow")]
    DimensionOverflow,
    #[error("frame dimensions must be non-zero")]
    EmptyFrame,
    #[error("unsupported RAW bit depth: {0}")]
    UnsupportedBitDepth(u8),
    #[error("CFA photometric data must have one component and a CFA pattern")]
    InconsistentCfaComponents,
    #[error("invalid CFA cell count: expected {expected}, got {actual}")]
    InvalidCfaLength { expected: usize, actual: usize },
    #[error("unsupported CFA layout {width}x{height}; the GPU fast path currently requires 2x2 RGB Bayer")]
    UnsupportedCfa { width: u8, height: u8 },
    #[error("invalid black-level grid: expected {expected}, got {actual}")]
    InvalidLevelGrid { expected: usize, actual: usize },
    #[error("black-level grid cannot be represented exactly by the GPU Bayer 2x2 correction")]
    UnsupportedBlackLevelGrid,
    #[error("non-finite {0} metadata")]
    NonFiniteMetadata(&'static str),
    #[error("white-balance correction gains must be positive")]
    InvalidWhiteBalance,
    #[error("{0} lies outside the decoded image")]
    InvalidRectangle(&'static str),
    #[error("invalid pixel count: expected {expected}, got {actual}")]
    InvalidPixelCount { expected: usize, actual: usize },
}

pub type CfaError = FrameError;

impl CfaPattern {
    pub fn color_at(&self, row: u32, column: u32) -> Option<CfaColor> {
        if self.width == 0 || self.height == 0 {
            return None;
        }
        let x = column % u32::from(self.width);
        let y = row % u32::from(self.height);
        self.cells.get((y * u32::from(self.width) + x) as usize).copied()
    }

    /// Returns a row-major 2x2 RGB Bayer quad encoded for WGSL.
    pub fn bayer_quad(&self) -> Result<[u32; 4], FrameError> {
        if self.width != 2 || self.height != 2 || self.cells.len() != 4 {
            return Err(FrameError::UnsupportedCfa {
                width: self.width,
                height: self.height,
            });
        }
        let mut encoded = [0_u32; 4];
        for (dst, color) in encoded.iter_mut().zip(&self.cells) {
            *dst = match color {
                CfaColor::Red => 0,
                CfaColor::Green => 1,
                CfaColor::Blue => 2,
                _ => {
                    return Err(FrameError::UnsupportedCfa {
                        width: self.width,
                        height: self.height,
                    });
                }
            };
        }
        let red = encoded.iter().filter(|&&value| value == 0).count();
        let green = encoded.iter().filter(|&&value| value == 1).count();
        let blue = encoded.iter().filter(|&&value| value == 2).count();
        if (red, green, blue) != (1, 2, 1) {
            return Err(FrameError::UnsupportedCfa {
                width: self.width,
                height: self.height,
            });
        }
        Ok(encoded)
    }

    /// Four distinct sensor planes encoded in R/G/B/E order for reconstruction.
    /// This does not treat emerald as a second ordinary green Bayer site.
    pub fn rgbe_quad(&self) -> Result<[u32; 4], FrameError> {
        let unsupported = || FrameError::UnsupportedCfa {
            width: self.width,
            height: self.height,
        };
        if self.width != 2 || self.height != 2 || self.cells.len() != 4 {
            return Err(unsupported());
        }
        let mut result = [0u32; 4];
        let mut seen = [false; 4];
        for (output, color) in result.iter_mut().zip(&self.cells) {
            let code = match color {
                CfaColor::Red => 0,
                CfaColor::Green => 1,
                CfaColor::Blue => 2,
                CfaColor::Emerald => 3,
                _ => return Err(unsupported()),
            };
            if seen[code] {
                return Err(unsupported());
            }
            seen[code] = true;
            *output = code as u32;
        }
        Ok(result)
    }

    pub fn validate(&self) -> Result<(), FrameError> {
        let expected = usize::from(self.width)
            .checked_mul(usize::from(self.height))
            .ok_or(FrameError::DimensionOverflow)?;
        if expected == 0 || expected != self.cells.len() {
            return Err(FrameError::InvalidCfaLength {
                expected,
                actual: self.cells.len(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum DemosaicError {
    #[error("invalid RAW development input: {0}")]
    Invalid(&'static str),
    #[error("RAW development cancelled")]
    Cancelled,
    #[error(transparent)]
    Frame(#[from] FrameError),
    #[error(transparent)]
    Buffer(#[from] rrrah_memory::BufferError),
}

pub(crate) type DevelopError = DemosaicError;

#[derive(Debug, Clone)]
pub struct Rgb32f {
    pub width: usize,
    pub height: usize,
    pub data: Vec<[f64; 3]>,
}

impl Rgb32f {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![[0.0; 3]; width * height],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Cfa {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<u8>,
}

impl Cfa {
    #[inline]
    pub fn color_at(&self, x: usize, y: usize) -> u8 {
        self.cells[y % self.height * self.width + x % self.width]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Mosaic<'a> {
    pub w: usize,
    pub h: usize,
    pub data: &'a [f64],
    pub cfa: &'a Cfa,
}

#[inline]
pub fn reflect(i: isize, n: usize) -> usize {
    if n == 1 {
        return 0;
    }
    let period = 2 * (n as isize - 1);
    let j = i.rem_euclid(period);
    if j >= n as isize {
        (period - j) as usize
    } else {
        j as usize
    }
}

impl Mosaic<'_> {
    #[inline]
    pub fn get(&self, x: isize, y: isize) -> f64 {
        self.data[reflect(y, self.h) * self.w + reflect(x, self.w)]
    }

    #[inline]
    pub fn color(&self, x: isize, y: isize) -> u8 {
        self.cfa.color_at(reflect(x, self.w), reflect(y, self.h))
    }
}

pub fn par_rows<T: Send>(
    data: &mut [T],
    width: usize,
    cancelled: &impl Fn() -> bool,
    f: impl Fn(usize, &mut [T]) + Sync,
) -> Result<(), DemosaicError> {
    if width == 0 || data.is_empty() {
        return Ok(());
    }
    let height = data.len() / width;
    let chunk_rows = 32;
    let num_chunks = height.div_ceil(chunk_rows);
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(num_chunks)
        .max(1);

    if workers <= 1 {
        for (y, row) in data.chunks_mut(width).enumerate() {
            if cancelled() {
                return Err(DemosaicError::Cancelled);
            }
            f(y, row);
        }
        return Ok(());
    }

    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    let is_cancelled = AtomicBool::new(false);
    let next = AtomicUsize::new(0);
    let slots: Vec<_> = data
        .chunks_mut(chunk_rows * width)
        .map(|chunk| Mutex::new(Some(chunk)))
        .collect();
    let f_ref = &f;

    let do_work = || loop {
        if is_cancelled.load(Ordering::Relaxed) {
            break;
        }
        let idx = next.fetch_add(1, Ordering::Relaxed);
        if idx >= slots.len() {
            break;
        }
        let chunk = slots[idx]
            .lock()
            .expect("slot lock never poisoned")
            .take()
            .expect("each chunk claimed once");
        let start_y = idx * chunk_rows;
        for (row_in_chunk, row) in chunk.chunks_mut(width).enumerate() {
            if is_cancelled.load(Ordering::Relaxed) {
                break;
            }
            f_ref(start_y + row_in_chunk, row);
        }
    };

    std::thread::scope(|scope| {
        let shared_worker = &do_work;
        for worker_index in 1..workers {
            let _ = std::thread::Builder::new()
                .name(format!("rrrah-row-{worker_index}"))
                .spawn_scoped(scope, shared_worker);
        }
        loop {
            if cancelled() {
                is_cancelled.store(true, Ordering::Relaxed);
                break;
            }
            let idx = next.fetch_add(1, Ordering::Relaxed);
            if idx >= slots.len() {
                break;
            }
            let chunk = slots[idx]
                .lock()
                .expect("slot lock never poisoned")
                .take()
                .expect("each chunk claimed once");
            let start_y = idx * chunk_rows;
            for (row_in_chunk, row) in chunk.chunks_mut(width).enumerate() {
                if cancelled() {
                    is_cancelled.store(true, Ordering::Relaxed);
                    break;
                }
                f_ref(start_y + row_in_chunk, row);
            }
        }
    });

    if is_cancelled.load(Ordering::Relaxed) || cancelled() {
        return Err(DemosaicError::Cancelled);
    }
    Ok(())
}

pub fn is_valid_xtrans_6x6(pattern: &CfaPattern) -> bool {
    pattern.width == 6
        && pattern.height == 6
        && pattern.cells.len() == 36
        && pattern.cells.iter().all(|c| (*c as u8) <= 2)
        && [0, 1, 2]
            .into_iter()
            .zip([8, 20, 8])
            .all(|(c, n)| pattern.cells.iter().filter(|p| **p as u8 == c).count() == n)
}

pub fn quality_cfa(pattern: &CfaPattern, algo: DemosaicAlgorithm) -> Result<Cfa, DemosaicError> {
    pattern.validate()?;
    let cfa = Cfa {
        width: pattern.width as usize,
        height: pattern.height as usize,
        cells: pattern.cells.iter().map(|c| *c as u8).collect(),
    };
    let bayer = pattern.bayer_quad().is_ok();
    let xtrans = is_valid_xtrans_6x6(pattern);
    let quad_bayer = demosaic::is_quad_bayer(&cfa);
    let nona_bayer = demosaic::is_nona_bayer(&cfa);
    let hexadeca_bayer = demosaic::is_hexadeca_bayer(&cfa);
    let known_cells = cfa.cells.iter().all(|&c| c <= 7);

    match algo {
        DemosaicAlgorithm::Auto => {
            if !bayer && !xtrans && !quad_bayer && !nona_bayer && !hexadeca_bayer {
                return Err(DemosaicError::Invalid(
                    "quality development requires RGB Bayer, Clustered Bayer (Quad/Nona/Hexadeca), or 6x6 X-Trans (or DemosaicAlgorithm::GenericCfa)",
                ));
            }
        }
        DemosaicAlgorithm::Nearest
        | DemosaicAlgorithm::Bilinear
        | DemosaicAlgorithm::SmoothHue
        | DemosaicAlgorithm::LarochePrescott
        | DemosaicAlgorithm::HamiltonAdams
        | DemosaicAlgorithm::MalvarHeCutler
        | DemosaicAlgorithm::Ppg
        | DemosaicAlgorithm::Vng
        | DemosaicAlgorithm::Rcd
        | DemosaicAlgorithm::Ahd
        | DemosaicAlgorithm::Lmmse
        | DemosaicAlgorithm::Dcb
        | DemosaicAlgorithm::Igv
        | DemosaicAlgorithm::Amaze
        | DemosaicAlgorithm::AmazeVng
        | DemosaicAlgorithm::AmazeLmmse
        | DemosaicAlgorithm::RcdVng
        | DemosaicAlgorithm::RcdLmmse
        | DemosaicAlgorithm::DcbVng => {
            if !bayer {
                return Err(DemosaicError::Invalid(
                    "selected Bayer demosaic algorithm requires a 2x2 RGB Bayer CFA",
                ));
            }
        }
        DemosaicAlgorithm::XTransDirectional
        | DemosaicAlgorithm::XTransMarkesteijn
        | DemosaicAlgorithm::XTransMarkesteijn3Pass
        | DemosaicAlgorithm::XTransDual => {
            if !xtrans {
                return Err(DemosaicError::Invalid(
                    "selected X-Trans demosaic algorithm requires a 6x6 RGB X-Trans CFA",
                ));
            }
        }
        DemosaicAlgorithm::QuadBayer => {
            if !quad_bayer {
                return Err(DemosaicError::Invalid(
                    "QuadBayer demosaic algorithm requires a 4x4 Quad Bayer CFA",
                ));
            }
        }
        DemosaicAlgorithm::NonaBayer => {
            if !nona_bayer {
                return Err(DemosaicError::Invalid(
                    "NonaBayer demosaic algorithm requires a 6x6 Nona Bayer (3x3 sub-cluster) CFA",
                ));
            }
        }
        DemosaicAlgorithm::HexadecaBayer => {
            if !hexadeca_bayer {
                return Err(DemosaicError::Invalid(
                    "HexadecaBayer demosaic algorithm requires an 8x8 Hexadeca Bayer (4x4 sub-cluster) CFA",
                ));
            }
        }
        DemosaicAlgorithm::GenericCfa => {
            if !known_cells {
                return Err(DemosaicError::Invalid(
                    "GenericCfa demosaic algorithm requires known CFA colour planes",
                ));
            }
        }
    }
    Ok(cfa)
}
