// SPDX-License-Identifier: Apache-2.0
//! Exhaustive CPU & standalone demosaicing library for Bayer 2×2, Quad Bayer 4×4, Nona Bayer 3×3 / 6×6,
//! Hexadeca Bayer 8×8, X-Trans 6×6, CYGM / RGBW / RGBE / EXR half-step CFAs, and arbitrary periodic colour filter arrays.
//!
//! Algorithms & capabilities provided:
//! - [`nearest`]: Zero-interpolation nearest-neighbour CFA lookup (1×1/2×2 diagnostic & fast preview).
//! - [`half_size_superpixel`]: Exact 2×2 Bayer superpixel extraction (1:1 per-cell RGB without interpolation artifacts).
//! - [`bilinear`]: Fast 3×3 bilinear colour interpolation for 2×2 Bayer.
//! - [`smooth_hue`]: Cok (1987) Smooth Hue Transition bilinear ratio/hue interpolation for 2×2 Bayer.
//! - [`laroche_prescott`]: Laroche–Prescott (1994) second-derivative edge-directed gradient interpolation.
//! - [`hamilton_adams`]: Hamilton–Adams (1997) directional Laplacian-corrected colour-difference interpolation.
//! - [`malvar_he_cutler`]: Malvar–He–Cutler (2004) 5×5 gradient-corrected linear filter for 2×2 Bayer.
//! - [`ppg`]: Patterned Pixel Grouping (Chuan-kai Lin) directional demosaicing for 2×2 Bayer.
//! - [`vng`]: Variable Number of Gradients (Chang, Cheung, Panchanathan 1999) 5×5 8-direction thresholded
//!   gradient interpolation for 2×2 Bayer.
//! - [`dlmmse`]: Directional Linear Minimum Mean Square Error (Zhang–Wu 2005) 5-tap directional filter.
//! - [`ahd`]: Adaptive Homogeneity-Directed (Hirakawa–Parks 2005) CIELab homogeneity-selected demosaicing.
//! - [`dcb`]: DCB (Jacek Gozdz) directional colour-difference demosaicing with iterative green/chroma refinement.
//! - [`igv`]: Igv (Ingo Weyrich / RawTherapee) variance-weighted anisotropic Gaussian gradient interpolation for noisy/high-ISO sensors.
//! - [`rcd`]: Ratio-Corrected Demosaicing (Luis Sanz Rodríguez) low-pass directional ratio/difference interpolation.
//! - [`amaze`]: AMaZE (Aliasing Minimization and Zipper Elimination, Emil Martinec) multi-scale cardinal + diagonal
//!   directional ratio/difference interpolation with sub-lattice zipper suppression.
//! - [`dual_demosaic`]: Contrast-guided Dual Demosaicing (`AmazeVng`, `AmazeLmmse`, `RcdVng`, `RcdLmmse`, `DcbVng`)
//!   that blends high-acutance edge solvers on detail with low-variance solvers on flat/noisy regions, plus
//!   `XTransMarkesteijnVng` for 6×6 X-Trans.
//! - [`xtrans_markesteijn`] / [`xtrans_markesteijn_3pass`]: 1-pass and 3-pass Markesteijn 6×6 X-Trans homogeneity-directed
//!   demosaicing with iterative green & colour-difference refinement.
//! - [`quad_bayer`] & [`multi_cell_bayer`]: Dedicated directional demosaicing for 4×4 Quad Bayer (TetraCell),
//!   6×6 Nona Bayer (3×3 sub-clusters), and 8×8 Hexadeca Bayer (4×4 sub-clusters) with intra-cluster detail preservation.
//! - [`generic_cfa`]: Universal edge-aware inverse-distance / directional solver for any valid periodic CFA
//!   (including RGBW, CYGM, or non-standard RGB matrices).
//! - [`suppress_false_color`], [`suppress_green_equilibration`], [`suppress_impulse_cfa`], [`suppress_chromatic_aberration`]:
//!   Pre- and post-demosaic artifact suppression stages that preserve scene-linear HDR and measured sensor fidelity.
//! - [`demosaic_cfa`] & [`demosaic_bayer_superpixel`]: Standalone, decoder-independent public API with `MemoryBudget`
//!   admission and cooperative cancellation.

use super::{Cfa, DevelopError, Mosaic, Rgb32f, par_rows, reflect};

const EPS: f32 = 1e-5;
const TILE: usize = 128;

/// Exhaustive set of demosaicing algorithms for [`super::DevelopOptions`] and standalone [`demosaic_cfa`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DemosaicAlgorithm {
    /// Automatically selects the highest-quality native algorithm for the sensor's CFA layout:
    /// - 2×2 RGB Bayer → [`Self::Ahd`]
    /// - 6×6 X-Trans → [`Self::XTransDirectional`]
    /// - 4×4 Quad Bayer → [`Self::QuadBayer`]
    /// - 6×6 Nona Bayer → [`Self::NonaBayer`]
    /// - 8×8 Hexadeca Bayer → [`Self::HexadecaBayer`]
    /// - 2×2 RGBE → calibrated 4-plane RGBE reconstruction
    /// - Any other periodic CFA (CYGM, RGBW, arbitrary `W×H`) → [`Self::GenericCfa`]
    #[default]
    Auto,
    /// Nearest-neighbour replication (fast zero-interpolation diagnostic / preview for 2×2 Bayer).
    Nearest,
    /// Fast 3×3 bilinear interpolation for 2×2 Bayer.
    Bilinear,
    /// Cok (1987) Smooth Hue Transition bilinear hue/ratio interpolation for 2×2 Bayer.
    SmoothHue,
    /// Laroche–Prescott (1994) second-derivative edge-directed gradient interpolation for 2×2 Bayer.
    LarochePrescott,
    /// Hamilton–Adams (1997) directional Laplacian-corrected colour-difference interpolation for 2×2 Bayer.
    HamiltonAdams,
    /// Malvar–He–Cutler (2004) 5×5 gradient-corrected linear filter for 2×2 Bayer.
    MalvarHeCutler,
    /// Patterned Pixel Grouping (Chuan-kai Lin) directional demosaicing for 2×2 Bayer.
    Ppg,
    /// Variable Number of Gradients (5×5, 8 directions) for 2×2 Bayer.
    Vng,
    /// Adaptive Homogeneity-Directed (Hirakawa–Parks 2005) CIELab homogeneity-selected demosaicing for 2×2 Bayer.
    Ahd,
    /// Linear Minimum Mean Square Error (Zhang–Wu 2005) directional filter for 2×2 Bayer.
    Lmmse,
    /// DCB (Jacek Gozdz) directional demosaicing with iterative green & chroma refinement for 2×2 Bayer.
    Dcb,
    /// IGV (Ingo Weyrich) variance-weighted anisotropic Gaussian gradient interpolation for high-ISO 2×2 Bayer.
    Igv,
    /// Ratio-Corrected Demosaicing (Luis Sanz Rodríguez) low-pass directional interpolation for 2×2 Bayer.
    Rcd,
    /// AMaZE (Aliasing Minimization and Zipper Elimination, Emil Martinec) multi-scale cardinal + diagonal
    /// directional interpolation for 2×2 Bayer.
    Amaze,
    /// Contrast-guided Dual Demosaicing blending [`Self::Amaze`] (edges) and [`Self::Vng`] (flat/noisy areas).
    AmazeVng,
    /// Contrast-guided Dual Demosaicing blending [`Self::Amaze`] (edges) and [`Self::Lmmse`] (flat/noisy areas).
    AmazeLmmse,
    /// Contrast-guided Dual Demosaicing blending [`Self::Rcd`] (edges) and [`Self::Vng`] (flat/noisy areas).
    RcdVng,
    /// Contrast-guided Dual Demosaicing blending [`Self::Rcd`] (edges) and [`Self::Lmmse`] (flat/noisy areas).
    RcdLmmse,
    /// Contrast-guided Dual Demosaicing blending [`Self::Dcb`] (edges) and [`Self::Vng`] (flat/noisy areas).
    DcbVng,
    /// Edge-weighted directional colour-difference interpolation for 6×6 X-Trans.
    XTransDirectional,
    /// 1-pass CIELab homogeneity-directed interpolation for 6×6 X-Trans (Markesteijn).
    XTransMarkesteijn,
    /// 3-pass iterative CIELab homogeneity-directed interpolation for 6×6 X-Trans (Markesteijn 3-pass).
    XTransMarkesteijn3Pass,
    /// Contrast-guided Dual Demosaicing for 6×6 X-Trans blending [`Self::XTransMarkesteijn3Pass`] and smooth directional VNG.
    XTransDual,
    /// Dedicated directional demosaicing for 4×4 Quad Bayer (TetraCell, 2×2 sub-clusters) sensors.
    QuadBayer,
    /// Dedicated directional demosaicing for 6×6 Nona Bayer (Nonacell, 3×3 sub-clusters) sensors.
    NonaBayer,
    /// Dedicated directional demosaicing for 8×8 Hexadeca Bayer (ChameleonCell, 4×4 sub-clusters) sensors.
    HexadecaBayer,
    /// Universal edge-aware solver for arbitrary periodic CFAs (Bayer, X-Trans, Quad/Nona/Hexadeca Bayer, CYGM, RGBW).
    GenericCfa,
}

impl DemosaicAlgorithm {
    /// Canonical lowercase identifier used in CLI flags, telemetry, and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Nearest => "nearest",
            Self::Bilinear => "bilinear",
            Self::SmoothHue => "smooth-hue",
            Self::LarochePrescott => "laroche-prescott",
            Self::HamiltonAdams => "hamilton-adams",
            Self::MalvarHeCutler => "mhc",
            Self::Ppg => "ppg",
            Self::Vng => "vng",
            Self::Ahd => "ahd",
            Self::Lmmse => "lmmse",
            Self::Dcb => "dcb",
            Self::Igv => "igv",
            Self::Rcd => "rcd",
            Self::Amaze => "amaze",
            Self::AmazeVng => "amaze-vng",
            Self::AmazeLmmse => "amaze-lmmse",
            Self::RcdVng => "rcd-vng",
            Self::RcdLmmse => "rcd-lmmse",
            Self::DcbVng => "dcb-vng",
            Self::XTransDirectional => "xtrans-directional",
            Self::XTransMarkesteijn => "xtrans-markesteijn",
            Self::XTransMarkesteijn3Pass => "xtrans-markesteijn-3pass",
            Self::XTransDual => "xtrans-dual",
            Self::QuadBayer => "quad-bayer",
            Self::NonaBayer => "nona-bayer",
            Self::HexadecaBayer => "hexadeca-bayer",
            Self::GenericCfa => "generic",
        }
    }

    /// All selectable algorithms in canonical order.
    pub const ALL: &'static [Self] = &[
        Self::Auto,
        Self::Nearest,
        Self::Bilinear,
        Self::SmoothHue,
        Self::LarochePrescott,
        Self::HamiltonAdams,
        Self::MalvarHeCutler,
        Self::Ppg,
        Self::Vng,
        Self::Ahd,
        Self::Lmmse,
        Self::Dcb,
        Self::Igv,
        Self::Rcd,
        Self::Amaze,
        Self::AmazeVng,
        Self::AmazeLmmse,
        Self::RcdVng,
        Self::RcdLmmse,
        Self::DcbVng,
        Self::XTransDirectional,
        Self::XTransMarkesteijn,
        Self::XTransMarkesteijn3Pass,
        Self::XTransDual,
        Self::QuadBayer,
        Self::NonaBayer,
        Self::HexadecaBayer,
        Self::GenericCfa,
    ];
}

/// Run a tiled Bayer demosaicing kernel in parallel across horizontal tile bands,
/// checking cooperative cancellation between tiles and reusing per-worker tile scratch.
fn run_tiled_bayer<S: Default>(
    m: &Mosaic,
    cancelled: &impl Fn() -> bool,
    tile_fn: impl Fn(&Mosaic, usize, usize, usize, usize, &mut S, &mut [[f32; 3]]) + Sync,
) -> Result<Rgb32f, DevelopError> {
    let (w, h) = (m.w, m.h);
    let mut out = Rgb32f::new(w, h);
    if w == 0 || h == 0 {
        return Ok(out);
    }
    let num_bands = h.div_ceil(TILE);
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(num_bands)
        .max(1);

    if workers <= 1 {
        let mut scratch = S::default();
        let mut px = vec![[0.0f32; 3]; TILE * TILE];
        for y0 in (0..h).step_by(TILE) {
            for x0 in (0..w).step_by(TILE) {
                if cancelled() {
                    return Err(DevelopError::Cancelled);
                }
                let (tw, th) = (TILE.min(w - x0), TILE.min(h - y0));
                tile_fn(m, x0, y0, tw, th, &mut scratch, &mut px[..tw * th]);
                for y in 0..th {
                    out.data[(y0 + y) * w + x0..(y0 + y) * w + x0 + tw]
                        .copy_from_slice(&px[y * tw..(y + 1) * tw]);
                }
            }
        }
    } else {
        use std::sync::Mutex;
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

        let is_cancelled = AtomicBool::new(false);
        let next = AtomicUsize::new(0);
        let slots: Vec<_> = out
            .data
            .chunks_mut(TILE * w)
            .map(|chunk| Mutex::new(Some(chunk)))
            .collect();
        let tile_ref = &tile_fn;

        let do_work = || {
            let mut scratch = S::default();
            let mut px = vec![[0.0f32; 3]; TILE * TILE];
            loop {
                if is_cancelled.load(Ordering::Relaxed) {
                    break;
                }
                let band_idx = next.fetch_add(1, Ordering::Relaxed);
                if band_idx >= num_bands {
                    break;
                }
                let band = slots[band_idx]
                    .lock()
                    .expect("slot lock never poisoned")
                    .take()
                    .expect("each band claimed once");
                let y0 = band_idx * TILE;
                let th = TILE.min(h - y0);
                for x0 in (0..w).step_by(TILE) {
                    if is_cancelled.load(Ordering::Relaxed) {
                        break;
                    }
                    let tw = TILE.min(w - x0);
                    tile_ref(m, x0, y0, tw, th, &mut scratch, &mut px[..tw * th]);
                    for y in 0..th {
                        band[y * w + x0..y * w + x0 + tw]
                            .copy_from_slice(&px[y * tw..(y + 1) * tw]);
                    }
                }
            }
        };

        std::thread::scope(|scope| {
            let shared_worker = &do_work;
            for worker_index in 1..workers {
                let _ = std::thread::Builder::new()
                    .name(format!("rrrah-demosaic-{worker_index}"))
                    .spawn_scoped(scope, shared_worker);
            }
            let mut scratch = S::default();
            let mut px = vec![[0.0f32; 3]; TILE * TILE];
            loop {
                if cancelled() {
                    is_cancelled.store(true, Ordering::Relaxed);
                    break;
                }
                let band_idx = next.fetch_add(1, Ordering::Relaxed);
                if band_idx >= num_bands {
                    break;
                }
                let band = slots[band_idx]
                    .lock()
                    .expect("slot lock never poisoned")
                    .take()
                    .expect("each band claimed once");
                let y0 = band_idx * TILE;
                let th = TILE.min(h - y0);
                for x0 in (0..w).step_by(TILE) {
                    if cancelled() {
                        is_cancelled.store(true, Ordering::Relaxed);
                        break;
                    }
                    let tw = TILE.min(w - x0);
                    tile_ref(m, x0, y0, tw, th, &mut scratch, &mut px[..tw * th]);
                    for y in 0..th {
                        band[y * w + x0..y * w + x0 + tw]
                            .copy_from_slice(&px[y * tw..(y + 1) * tw]);
                    }
                }
            }
        });

        if cancelled() || is_cancelled.load(Ordering::Relaxed) {
            return Err(DevelopError::Cancelled);
        }
    }
    Ok(out)
}

/// Zero-interpolation nearest-neighbour CFA lookup for 2×2 RGB Bayer mosaics.
/// Exact at measured CFA sites and exact on constant colour fields.
pub(super) fn nearest(m: &Mosaic, cancelled: &impl Fn() -> bool) -> Result<Rgb32f, DevelopError> {
    let (w, h) = (m.w, m.h);
    let mut out = Rgb32f::new(w, h);
    if w == 0 || h == 0 {
        return Ok(out);
    }
    par_rows(&mut out.data, w, cancelled, |y, row| {
        let yi = y as isize;
        for (x, px) in row.iter_mut().enumerate() {
            let xi = x as isize;
            let c = m.get(xi, yi);
            let n = m.get(xi, yi - 1);
            let e = m.get(xi + 1, yi);
            let ne = m.get(xi + 1, yi - 1);
            *px = match m.color(xi, yi) {
                0 => [c, e, ne],
                2 => [ne, e, c],
                _ => {
                    if m.color(xi + 1, yi) == 0 {
                        [e, c, n]
                    } else {
                        [n, c, e]
                    }
                }
            };
        }
    })?;
    Ok(out)
}

/// Cok (1987) Smooth Hue Transition demosaicing for 2×2 RGB Bayer mosaics.
///
/// Reconstructs Green via 4-neighbour bilinear interpolation, then interpolates Red and Blue
/// by bilinearly averaging the colour ratios `R / G_safe` and `B / G_safe` (with a positive floor
/// `1e-4` so dark/zero sites smoothly fall back to colour differences) and multiplying by the
/// local Green value. Exact on constant colour fields and preserves measured CFA sites.
pub(super) fn smooth_hue(m: &Mosaic, cancelled: &impl Fn() -> bool) -> Result<Rgb32f, DevelopError> {
    #[derive(Default)]
    struct SmoothHueScratch {
        mos: Vec<f32>,
        col: Vec<u8>,
        g: Vec<f32>,
    }
    const MARGIN: usize = 2;
    run_tiled_bayer(
        m,
        cancelled,
        |m, x0, y0, tw, th, scratch: &mut SmoothHueScratch, out| {
            let (bw, bh) = (tw + 2 * MARGIN, th + 2 * MARGIN);
            let n = bw * bh;
            scratch.mos.resize(n, 0.0);
            scratch.col.resize(n, 0);
            scratch.g.resize(n, 0.0);

            let (ox, oy) = (x0 as isize - MARGIN as isize, y0 as isize - MARGIN as isize);
            for y in 0..bh {
                for x in 0..bw {
                    let idx = y * bw + x;
                    scratch.mos[idx] = m.get(ox + x as isize, oy + y as isize);
                    scratch.col[idx] = m.color(ox + x as isize, oy + y as isize);
                }
            }
            let cl = |x: isize, y: isize| -> usize {
                y.clamp(0, bh as isize - 1) as usize * bw + x.clamp(0, bw as isize - 1) as usize
            };
            for y in 1..bh as isize - 1 {
                for x in 1..bw as isize - 1 {
                    let i = cl(x, y);
                    scratch.g[i] = if scratch.col[i] == 1 {
                        scratch.mos[i]
                    } else {
                        0.25 * (scratch.mos[cl(x, y - 1)]
                            + scratch.mos[cl(x, y + 1)]
                            + scratch.mos[cl(x - 1, y)]
                            + scratch.mos[cl(x + 1, y)])
                    };
                }
            }
            let ratio = |idx: usize| -> f32 {
                (scratch.mos[idx] + 1e-3) / (scratch.g[idx].max(0.0) + 1e-3)
            };
            let from_ratio = |gp: f32, r_avg: f32, lo: f32, hi: f32| -> f32 {
                ((gp.max(0.0) + 1e-3) * r_avg - 1e-3).clamp(lo, hi)
            };
            for y in 0..th {
                let by = (y + MARGIN) as isize;
                for x in 0..tw {
                    let bx = (x + MARGIN) as isize;
                    let i = cl(bx, by);
                    let c = scratch.mos[i];
                    let gp = scratch.g[i];
                    let nw = cl(bx - 1, by - 1);
                    let ne = cl(bx + 1, by - 1);
                    let sw = cl(bx - 1, by + 1);
                    let se = cl(bx + 1, by + 1);
                    let w1 = cl(bx - 1, by);
                    let e1 = cl(bx + 1, by);
                    let n1 = cl(bx, by - 1);
                    let s1 = cl(bx, by + 1);
                    out[y * tw + x] = match scratch.col[i] {
                        0 => {
                            let b_min = scratch.mos[nw]
                                .min(scratch.mos[ne])
                                .min(scratch.mos[sw])
                                .min(scratch.mos[se]);
                            let b_max = scratch.mos[nw]
                                .max(scratch.mos[ne])
                                .max(scratch.mos[sw])
                                .max(scratch.mos[se]);
                            let b_rat = 0.25 * (ratio(nw) + ratio(ne) + ratio(sw) + ratio(se));
                            [c, gp, from_ratio(gp, b_rat, b_min, b_max)]
                        }
                        2 => {
                            let r_min = scratch.mos[nw]
                                .min(scratch.mos[ne])
                                .min(scratch.mos[sw])
                                .min(scratch.mos[se]);
                            let r_max = scratch.mos[nw]
                                .max(scratch.mos[ne])
                                .max(scratch.mos[sw])
                                .max(scratch.mos[se]);
                            let r_rat = 0.25 * (ratio(nw) + ratio(ne) + ratio(sw) + ratio(se));
                            [from_ratio(gp, r_rat, r_min, r_max), gp, c]
                        }
                        _ => {
                            let ch = from_ratio(
                                c,
                                0.5 * (ratio(w1) + ratio(e1)),
                                scratch.mos[w1].min(scratch.mos[e1]),
                                scratch.mos[w1].max(scratch.mos[e1]),
                            );
                            let cv = from_ratio(
                                c,
                                0.5 * (ratio(n1) + ratio(s1)),
                                scratch.mos[n1].min(scratch.mos[s1]),
                                scratch.mos[n1].max(scratch.mos[s1]),
                            );
                            if scratch.col[e1] == 0 {
                                [ch, c, cv]
                            } else {
                                [cv, c, ch]
                            }
                        }
                    };
                }
            }
        },
    )
}

/// Laroche–Prescott (1994) edge-directed second-derivative gradient demosaicing for 2×2 RGB Bayer.
///
/// Uses second-order chromatic Laplacian classifiers `|2*C - C_w2 - C_e2|` vs `|2*C - C_n2 - C_s2|`
/// to interpolate Green along edges and then completes Red and Blue via bilinear colour differences.
pub(super) fn laroche_prescott(
    m: &Mosaic,
    cancelled: &impl Fn() -> bool,
) -> Result<Rgb32f, DevelopError> {
    #[derive(Default)]
    struct LpScratch {
        mos: Vec<f32>,
        col: Vec<u8>,
        g: Vec<f32>,
    }
    const MARGIN: usize = 3;
    run_tiled_bayer(
        m,
        cancelled,
        |m, x0, y0, tw, th, scratch: &mut LpScratch, out| {
            let (bw, bh) = (tw + 2 * MARGIN, th + 2 * MARGIN);
            let n = bw * bh;
            scratch.mos.resize(n, 0.0);
            scratch.col.resize(n, 0);
            scratch.g.resize(n, 0.0);

            let (ox, oy) = (x0 as isize - MARGIN as isize, y0 as isize - MARGIN as isize);
            for y in 0..bh {
                for x in 0..bw {
                    let idx = y * bw + x;
                    scratch.mos[idx] = m.get(ox + x as isize, oy + y as isize);
                    scratch.col[idx] = m.color(ox + x as isize, oy + y as isize);
                }
            }
            let cl = |x: isize, y: isize| -> usize {
                y.clamp(0, bh as isize - 1) as usize * bw + x.clamp(0, bw as isize - 1) as usize
            };
            for y in 2..bh as isize - 2 {
                for x in 2..bw as isize - 2 {
                    let i = cl(x, y);
                    let c = scratch.mos[i];
                    if scratch.col[i] == 1 {
                        scratch.g[i] = c;
                        continue;
                    }
                    let w1 = scratch.mos[cl(x - 1, y)];
                    let e1 = scratch.mos[cl(x + 1, y)];
                    let n1 = scratch.mos[cl(x, y - 1)];
                    let s1 = scratch.mos[cl(x, y + 1)];
                    let w2 = scratch.mos[cl(x - 2, y)];
                    let e2 = scratch.mos[cl(x + 2, y)];
                    let n2 = scratch.mos[cl(x, y - 2)];
                    let s2 = scratch.mos[cl(x, y + 2)];
                    let alpha = (2.0 * c - w2 - e2).abs();
                    let beta = (2.0 * c - n2 - s2).abs();
                    scratch.g[i] = if alpha < beta {
                        0.5 * (w1 + e1)
                    } else if beta < alpha {
                        0.5 * (n1 + s1)
                    } else {
                        0.25 * (w1 + e1 + n1 + s1)
                    };
                }
            }
            for y in 0..th {
                let by = (y + MARGIN) as isize;
                for x in 0..tw {
                    let bx = (x + MARGIN) as isize;
                    let i = cl(bx, by);
                    let c = scratch.mos[i];
                    let gp = scratch.g[i];
                    let nw = cl(bx - 1, by - 1);
                    let ne = cl(bx + 1, by - 1);
                    let sw = cl(bx - 1, by + 1);
                    let se = cl(bx + 1, by + 1);
                    let w1 = cl(bx - 1, by);
                    let e1 = cl(bx + 1, by);
                    let n1 = cl(bx, by - 1);
                    let s1 = cl(bx, by + 1);
                    out[y * tw + x] = match scratch.col[i] {
                        0 => {
                            let b_est = gp
                                + 0.25
                                    * ((scratch.mos[nw] - scratch.g[nw])
                                        + (scratch.mos[ne] - scratch.g[ne])
                                        + (scratch.mos[sw] - scratch.g[sw])
                                        + (scratch.mos[se] - scratch.g[se]));
                            [c, gp, b_est]
                        }
                        2 => {
                            let r_est = gp
                                + 0.25
                                    * ((scratch.mos[nw] - scratch.g[nw])
                                        + (scratch.mos[ne] - scratch.g[ne])
                                        + (scratch.mos[sw] - scratch.g[sw])
                                        + (scratch.mos[se] - scratch.g[se]));
                            [r_est, gp, c]
                        }
                        _ => {
                            let ch = c + 0.5 * ((scratch.mos[w1] - scratch.g[w1]) + (scratch.mos[e1] - scratch.g[e1]));
                            let cv = c + 0.5 * ((scratch.mos[n1] - scratch.g[n1]) + (scratch.mos[s1] - scratch.g[s1]));
                            if scratch.col[e1] == 0 {
                                [ch, c, cv]
                            } else {
                                [cv, c, ch]
                            }
                        }
                    };
                }
            }
        },
    )
}

/// Hamilton–Adams (1997) directional Laplacian-corrected demosaicing for 2×2 RGB Bayer.
///
/// Combines first-order colour gradients and second-order cross-channel Laplacians along
/// horizontal, vertical, and both diagonal axes with local neighbour clamping.
pub(super) fn hamilton_adams(
    m: &Mosaic,
    cancelled: &impl Fn() -> bool,
) -> Result<Rgb32f, DevelopError> {
    #[derive(Default)]
    struct HaScratch {
        mos: Vec<f32>,
        col: Vec<u8>,
        g: Vec<f32>,
    }
    const MARGIN: usize = 3;
    run_tiled_bayer(
        m,
        cancelled,
        |m, x0, y0, tw, th, scratch: &mut HaScratch, out| {
            let (bw, bh) = (tw + 2 * MARGIN, th + 2 * MARGIN);
            let n = bw * bh;
            scratch.mos.resize(n, 0.0);
            scratch.col.resize(n, 0);
            scratch.g.resize(n, 0.0);

            let (ox, oy) = (x0 as isize - MARGIN as isize, y0 as isize - MARGIN as isize);
            for y in 0..bh {
                for x in 0..bw {
                    let idx = y * bw + x;
                    scratch.mos[idx] = m.get(ox + x as isize, oy + y as isize);
                    scratch.col[idx] = m.color(ox + x as isize, oy + y as isize);
                }
            }
            let cl = |x: isize, y: isize| -> usize {
                y.clamp(0, bh as isize - 1) as usize * bw + x.clamp(0, bw as isize - 1) as usize
            };
            for y in 2..bh as isize - 2 {
                for x in 2..bw as isize - 2 {
                    let i = cl(x, y);
                    let c = scratch.mos[i];
                    if scratch.col[i] == 1 {
                        scratch.g[i] = c;
                        continue;
                    }
                    let w1 = scratch.mos[cl(x - 1, y)];
                    let e1 = scratch.mos[cl(x + 1, y)];
                    let n1 = scratch.mos[cl(x, y - 1)];
                    let s1 = scratch.mos[cl(x, y + 1)];
                    let w2 = scratch.mos[cl(x - 2, y)];
                    let e2 = scratch.mos[cl(x + 2, y)];
                    let n2 = scratch.mos[cl(x, y - 2)];
                    let s2 = scratch.mos[cl(x, y + 2)];
                    let grad_h = (w1 - e1).abs() + (2.0 * c - w2 - e2).abs();
                    let grad_v = (n1 - s1).abs() + (2.0 * c - n2 - s2).abs();
                    let gh = (0.5 * (w1 + e1) + 0.25 * (2.0 * c - w2 - e2)).clamp(w1.min(e1), w1.max(e1));
                    let gv = (0.5 * (n1 + s1) + 0.25 * (2.0 * c - n2 - s2)).clamp(n1.min(s1), n1.max(s1));
                    scratch.g[i] = if grad_h < grad_v {
                        gh
                    } else if grad_v < grad_h {
                        gv
                    } else {
                        0.5 * (gh + gv)
                    };
                }
            }
            for y in 0..th {
                let by = (y + MARGIN) as isize;
                for x in 0..tw {
                    let bx = (x + MARGIN) as isize;
                    let i = cl(bx, by);
                    let c = scratch.mos[i];
                    let gp = scratch.g[i];
                    let nw = cl(bx - 1, by - 1);
                    let ne = cl(bx + 1, by - 1);
                    let sw = cl(bx - 1, by + 1);
                    let se = cl(bx + 1, by + 1);
                    let w1 = cl(bx - 1, by);
                    let e1 = cl(bx + 1, by);
                    let n1 = cl(bx, by - 1);
                    let s1 = cl(bx, by + 1);
                    out[y * tw + x] = match scratch.col[i] {
                        0 | 2 => {
                            let d_nw = scratch.mos[nw] - scratch.g[nw];
                            let d_se = scratch.mos[se] - scratch.g[se];
                            let d_ne = scratch.mos[ne] - scratch.g[ne];
                            let d_sw = scratch.mos[sw] - scratch.g[sw];
                            let g_nw_se = (scratch.mos[nw] - scratch.mos[se]).abs()
                                + (2.0 * gp - scratch.g[nw] - scratch.g[se]).abs();
                            let g_ne_sw = (scratch.mos[ne] - scratch.mos[sw]).abs()
                                + (2.0 * gp - scratch.g[ne] - scratch.g[sw]).abs();
                            let opp_min = scratch.mos[nw]
                                .min(scratch.mos[ne])
                                .min(scratch.mos[sw])
                                .min(scratch.mos[se]);
                            let opp_max = scratch.mos[nw]
                                .max(scratch.mos[ne])
                                .max(scratch.mos[sw])
                                .max(scratch.mos[se]);
                            let opp = if g_nw_se < g_ne_sw {
                                gp + 0.5 * (d_nw + d_se)
                            } else if g_ne_sw < g_nw_se {
                                gp + 0.5 * (d_ne + d_sw)
                            } else {
                                gp + 0.25 * (d_nw + d_se + d_ne + d_sw)
                            }
                            .clamp(opp_min, opp_max);
                            if scratch.col[i] == 0 {
                                [c, gp, opp]
                            } else {
                                [opp, gp, c]
                            }
                        }
                        _ => {
                            let ch = (c + 0.5 * ((scratch.mos[w1] - scratch.g[w1]) + (scratch.mos[e1] - scratch.g[e1])))
                                .clamp(scratch.mos[w1].min(scratch.mos[e1]), scratch.mos[w1].max(scratch.mos[e1]));
                            let cv = (c + 0.5 * ((scratch.mos[n1] - scratch.g[n1]) + (scratch.mos[s1] - scratch.g[s1])))
                                .clamp(scratch.mos[n1].min(scratch.mos[s1]), scratch.mos[n1].max(scratch.mos[s1]));
                            if scratch.col[e1] == 0 {
                                [ch, c, cv]
                            } else {
                                [cv, c, ch]
                            }
                        }
                    };
                }
            }
        },
    )
}

/// Fast 3×3 bilinear demosaicing for 2×2 RGB Bayer mosaics.
/// Exact at measured CFA sites and exact on constant colour fields.
pub(super) fn bilinear(m: &Mosaic, cancelled: &impl Fn() -> bool) -> Result<Rgb32f, DevelopError> {
    let (w, h) = (m.w, m.h);
    let mut out = Rgb32f::new(w, h);
    if w == 0 || h == 0 {
        return Ok(out);
    }
    par_rows(&mut out.data, w, cancelled, |y, row| {
        let yi = y as isize;
        let interior_y = y >= 1 && y + 1 < h;
        for (x, px) in row.iter_mut().enumerate() {
            let xi = x as isize;
            let (c, n, s, wst, est, nw, ne, sw, se, own, h_color) =
                if interior_y && x >= 1 && x + 1 < w {
                    let i = y * w + x;
                    (
                        m.data[i],
                        m.data[i - w],
                        m.data[i + w],
                        m.data[i - 1],
                        m.data[i + 1],
                        m.data[i - w - 1],
                        m.data[i - w + 1],
                        m.data[i + w - 1],
                        m.data[i + w + 1],
                        m.cfa.color_at(x, y),
                        m.cfa.color_at(x + 1, y),
                    )
                } else {
                    (
                        m.get(xi, yi),
                        m.get(xi, yi - 1),
                        m.get(xi, yi + 1),
                        m.get(xi - 1, yi),
                        m.get(xi + 1, yi),
                        m.get(xi - 1, yi - 1),
                        m.get(xi + 1, yi - 1),
                        m.get(xi - 1, yi + 1),
                        m.get(xi + 1, yi + 1),
                        m.color(xi, yi),
                        m.cfa.color_at((x + 1) % m.cfa.width, y % m.cfa.height),
                    )
                };
            let axial = 0.25 * (n + s + wst + est);
            let diag = 0.25 * (nw + ne + sw + se);
            *px = match own {
                0 => [c, axial, diag],
                2 => [diag, axial, c],
                _ => {
                    let lr = 0.5 * (wst + est);
                    let ud = 0.5 * (n + s);
                    if h_color == 0 {
                        [lr, c, ud]
                    } else {
                        [ud, c, lr]
                    }
                }
            };
        }
    })?;
    Ok(out)
}

/// Malvar–He–Cutler (2004) 5×5 gradient-corrected linear filter for 2×2 RGB Bayer.
///
/// Reference: H. S. Malvar, L.-W. He, and R. Cutler, "High-quality linear interpolation for
/// demosaicing of Bayer-patterned color images", ICASSP 2004.
///
/// The filter weights sum to 1.0 on the target colour and 0.0 on the cross-channel Laplacian
/// correction terms, so constant colour fields are reproduced accurately while high-frequency
/// luminance gradients are transferred across channels.
pub(super) fn malvar_he_cutler(
    m: &Mosaic,
    cancelled: &impl Fn() -> bool,
) -> Result<Rgb32f, DevelopError> {
    let (w, h) = (m.w, m.h);
    let mut out = Rgb32f::new(w, h);
    if w == 0 || h == 0 {
        return Ok(out);
    }
    par_rows(&mut out.data, w, cancelled, |y, row| {
        let yi = y as isize;
        let interior_y = y >= 2 && y + 2 < h;
        for (x, px) in row.iter_mut().enumerate() {
            let xi = x as isize;
            let sample = |dx: isize, dy: isize| -> f32 {
                if interior_y && x >= 2 && x + 2 < w {
                    m.data[(yi + dy) as usize * w + (xi + dx) as usize]
                } else {
                    m.get(xi + dx, yi + dy)
                }
            };
            let c = sample(0, 0);
            let n1 = sample(0, -1);
            let s1 = sample(0, 1);
            let w1 = sample(-1, 0);
            let e1 = sample(1, 0);
            let n2 = sample(0, -2);
            let s2 = sample(0, 2);
            let w2 = sample(-2, 0);
            let e2 = sample(2, 0);
            let nw = sample(-1, -1);
            let ne = sample(1, -1);
            let sw = sample(-1, 1);
            let se = sample(1, 1);

            let own = m.cfa.color_at(x, y);
            // Clamp cross-channel gradient-corrected estimates to the range of the positive
            // support taps so extreme HDR highlights or negative noise never ring below 0.
            let clamp_support = |val: f32, lo: f32, hi: f32| val.clamp(lo, hi);

            match own {
                0 | 2 => {
                    // G at R or B: (2*(N+S+W+E) + (4*C - N2 - S2 - W2 - E2)) / 8
                    let g_min = n1.min(s1).min(w1).min(e1);
                    let g_max = n1.max(s1).max(w1).max(e1);
                    let g_est = 0.25 * (n1 + s1 + w1 + e1) + 0.125 * (4.0 * c - (n2 + s2 + w2 + e2));
                    let g = clamp_support(g_est, g_min, g_max);

                    // Opposite RB at RB: (2*(NW+NE+SW+SE) + 1.5*(4*C - N2 - S2 - W2 - E2)) / 8
                    let opp_min = nw.min(ne).min(sw).min(se);
                    let opp_max = nw.max(ne).max(sw).max(se);
                    let opp_est =
                        0.25 * (nw + ne + sw + se) + 0.1875 * (4.0 * c - (n2 + s2 + w2 + e2));
                    let opp = clamp_support(opp_est, opp_min, opp_max);

                    if own == 0 {
                        *px = [c, g, opp];
                    } else {
                        *px = [opp, g, c];
                    }
                }
                _ => {
                    // Green site: horizontal neighbours have one RB colour, vertical have the other.
                    // Horizontal interpolation: (4*(W1+E1) + 5*C - (NW+NE+SW+SE) - W2 - E2 + 0.5*(N2+S2)) / 8
                    let h_min = w1.min(e1);
                    let h_max = w1.max(e1);
                    let h_est = 0.5 * (w1 + e1)
                        + 0.125
                            * (5.0 * c - (nw + ne + sw + se) - (w2 + e2) + 0.5 * (n2 + s2));
                    let ch_val = clamp_support(h_est, h_min, h_max);

                    // Vertical interpolation: (4*(N1+S1) + 5*C - (NW+NE+SW+SE) - N2 - S2 + 0.5*(W2+E2)) / 8
                    let v_min = n1.min(s1);
                    let v_max = n1.max(s1);
                    let v_est = 0.5 * (n1 + s1)
                        + 0.125
                            * (5.0 * c - (nw + ne + sw + se) - (n2 + s2) + 0.5 * (w2 + e2));
                    let cv_val = clamp_support(v_est, v_min, v_max);

                    let h_color = m.cfa.color_at((x + 1) % m.cfa.width, y % m.cfa.height);
                    if h_color == 0 {
                        *px = [ch_val, c, cv_val];
                    } else {
                        *px = [cv_val, c, ch_val];
                    }
                }
            }
        }
    })?;
    Ok(out)
}

/// Patterned Pixel Grouping (PPG) directional demosaicing for 2×2 RGB Bayer.
///
/// 1. Interpolates Green at R/B sites along the minimum gradient among 4 cardinal directions (N, E, S, W).
/// 2. Interpolates R/B at Green sites using Hamilton–Adams second-derivative correction on the green plane.
/// 3. Interpolates R at B (and B at R) along the minimum diagonal gradient (NE–SW vs NW–SE) with green
///    colour-difference correction.
pub(super) fn ppg(m: &Mosaic, cancelled: &impl Fn() -> bool) -> Result<Rgb32f, DevelopError> {
    #[derive(Default)]
    struct PpgScratch {
        mos: Vec<f32>,
        col: Vec<u8>,
        g: Vec<f32>,
    }
    const MARGIN: usize = 4;
    run_tiled_bayer(
        m,
        cancelled,
        |m, x0, y0, tw, th, scratch: &mut PpgScratch, out| {
            let (bw, bh) = (tw + 2 * MARGIN, th + 2 * MARGIN);
            let n = bw * bh;
            scratch.mos.resize(n, 0.0);
            scratch.col.resize(n, 0);
            scratch.g.resize(n, 0.0);
            let (ox, oy) = (x0 as isize - MARGIN as isize, y0 as isize - MARGIN as isize);
            for y in 0..bh {
                for x in 0..bw {
                    let idx = y * bw + x;
                    scratch.mos[idx] = m.get(ox + x as isize, oy + y as isize);
                    scratch.col[idx] = m.color(ox + x as isize, oy + y as isize);
                }
            }
            let cl = |x: isize, y: isize| -> usize {
                y.clamp(0, bh as isize - 1) as usize * bw + x.clamp(0, bw as isize - 1) as usize
            };
            // Pass 1: Green plane
            for y in 0..bh as isize {
                for x in 0..bw as isize {
                    let i = cl(x, y);
                    if scratch.col[i] == 1 {
                        scratch.g[i] = scratch.mos[i];
                        continue;
                    }
                    let c = scratch.mos[i];
                    let gn = scratch.mos[cl(x, y - 1)];
                    let gs = scratch.mos[cl(x, y + 1)];
                    let gw = scratch.mos[cl(x - 1, y)];
                    let ge = scratch.mos[cl(x + 1, y)];
                    let cn2 = scratch.mos[cl(x, y - 2)];
                    let cs2 = scratch.mos[cl(x, y + 2)];
                    let cw2 = scratch.mos[cl(x - 2, y)];
                    let ce2 = scratch.mos[cl(x + 2, y)];

                    let dn = (c - cn2).abs() * 2.0 + (gn - gs).abs();
                    let ds = (c - cs2).abs() * 2.0 + (gn - gs).abs();
                    let dw = (c - cw2).abs() * 2.0 + (gw - ge).abs();
                    let de = (c - ce2).abs() * 2.0 + (gw - ge).abs();

                    let min_d = dn.min(ds).min(dw).min(de);
                    let est = if (dn - min_d).abs() <= EPS && (ds - min_d).abs() <= EPS {
                        0.5 * (gn + gs) + 0.25 * (2.0 * c - cn2 - cs2)
                    } else if (dw - min_d).abs() <= EPS && (de - min_d).abs() <= EPS {
                        0.5 * (gw + ge) + 0.25 * (2.0 * c - cw2 - ce2)
                    } else if min_d == dn {
                        (3.0 * gn + gs + c - cn2) * 0.25
                    } else if min_d == ds {
                        (3.0 * gs + gn + c - cs2) * 0.25
                    } else if min_d == dw {
                        (3.0 * gw + ge + c - cw2) * 0.25
                    } else {
                        (3.0 * ge + gw + c - ce2) * 0.25
                    };
                    let g_min = gn.min(gs).min(gw).min(ge);
                    let g_max = gn.max(gs).max(gw).max(ge);
                    scratch.g[i] = est.clamp(g_min, g_max);
                }
            }
            // Pass 2: R and B using completed Green plane
            for y in 0..th {
                let by = (y + MARGIN) as isize;
                for x in 0..tw {
                    let bx = (x + MARGIN) as isize;
                    let i = cl(bx, by);
                    let own = scratch.col[i] as usize;
                    let gp = scratch.g[i];
                    let mut p = [0.0f32; 3];
                    p[1] = gp;
                    let d = |dx: isize, dy: isize| {
                        let j = cl(bx + dx, by + dy);
                        scratch.mos[j] - scratch.g[j]
                    };
                    if own == 1 {
                        let ch = scratch.col[cl(bx + 1, by)] as usize;
                        let cv = scratch.col[cl(bx, by + 1)] as usize;
                        p[ch] = gp + 0.5 * (d(-1, 0) + d(1, 0));
                        p[cv] = gp + 0.5 * (d(0, -1) + d(0, 1));
                    } else {
                        p[own] = scratch.mos[i];
                        // Diagonal gradients along NW-SE vs NE-SW
                        let c_nw = scratch.mos[cl(bx - 1, by - 1)];
                        let c_se = scratch.mos[cl(bx + 1, by + 1)];
                        let c_ne = scratch.mos[cl(bx + 1, by - 1)];
                        let c_sw = scratch.mos[cl(bx - 1, by + 1)];
                        let g_nw = scratch.g[cl(bx - 1, by - 1)];
                        let g_se = scratch.g[cl(bx + 1, by + 1)];
                        let g_ne = scratch.g[cl(bx + 1, by - 1)];
                        let g_sw = scratch.g[cl(bx - 1, by + 1)];

                        let grad_nw_se =
                            (c_nw - c_se).abs() + (2.0 * gp - g_nw - g_se).abs();
                        let grad_ne_sw =
                            (c_ne - c_sw).abs() + (2.0 * gp - g_ne - g_sw).abs();

                        p[2 - own] = if grad_nw_se < grad_ne_sw {
                            gp + 0.5 * (d(-1, -1) + d(1, 1))
                        } else if grad_ne_sw < grad_nw_se {
                            gp + 0.5 * (d(1, -1) + d(-1, 1))
                        } else {
                            gp + 0.25 * (d(-1, -1) + d(1, 1) + d(1, -1) + d(-1, 1))
                        };
                    }
                    out[y * tw + x] = p;
                }
            }
        },
    )
}

/// Variable Number of Gradients (VNG) 5×5 8-direction demosaicing for 2×2 RGB Bayer.
///
/// Reference: E. Chang, S. Cheung, and D. Y. Panchanathan, "Color filter array recovery using a
/// threshold-based variable number of gradients", Proc. SPIE 3650, 1999.
///
/// Computes local directional gradients across all 8 compass directions (N, E, S, W, NE, SE, NW, SW)
/// in a 5×5 window, sets an adaptive threshold `T = 1.5 * min_grad + 0.5 * (max_grad + min_grad) * 0.25`,
/// accumulates colour planes over the subset of directions below `T`, and updates the missing channels
/// via colour differences relative to the measured centre channel while keeping the measured channel exact.
pub(super) fn vng(m: &Mosaic, cancelled: &impl Fn() -> bool) -> Result<Rgb32f, DevelopError> {
    let (w, h) = (m.w, m.h);
    let mut out = Rgb32f::new(w, h);
    if w == 0 || h == 0 {
        return Ok(out);
    }
    par_rows(&mut out.data, w, cancelled, |y, row| {
        let yi = y as isize;
        let interior_y = y >= 2 && y + 2 < h;
        for (x, px) in row.iter_mut().enumerate() {
            let xi = x as isize;
            let mut s = [[0.0f32; 5]; 5];
            if interior_y && x >= 2 && x + 2 < w {
                for dy in -2isize..=2 {
                    let r_off = (yi + dy) as usize * w;
                    for dx in -2isize..=2 {
                        s[(dy + 2) as usize][(dx + 2) as usize] = m.data[r_off + (xi + dx) as usize];
                    }
                }
            } else {
                for dy in -2isize..=2 {
                    for dx in -2isize..=2 {
                        s[(dy + 2) as usize][(dx + 2) as usize] = m.get(xi + dx, yi + dy);
                    }
                }
            }

            // 8 directional gradients in the 5×5 neighborhood
            let g_n = (s[1][2] - s[3][2]).abs()
                + (s[0][2] - s[2][2]).abs()
                + 0.5 * ((s[1][1] - s[3][1]).abs() + (s[1][3] - s[3][3]).abs())
                + 0.5 * ((s[0][1] - s[2][1]).abs() + (s[0][3] - s[2][3]).abs());
            let g_e = (s[2][3] - s[2][1]).abs()
                + (s[2][4] - s[2][2]).abs()
                + 0.5 * ((s[1][3] - s[1][1]).abs() + (s[3][3] - s[3][1]).abs())
                + 0.5 * ((s[1][4] - s[1][2]).abs() + (s[3][4] - s[3][2]).abs());
            let g_s = (s[3][2] - s[1][2]).abs()
                + (s[4][2] - s[2][2]).abs()
                + 0.5 * ((s[3][1] - s[1][1]).abs() + (s[3][3] - s[1][3]).abs())
                + 0.5 * ((s[4][1] - s[2][1]).abs() + (s[4][3] - s[2][3]).abs());
            let g_w = (s[2][1] - s[2][3]).abs()
                + (s[2][0] - s[2][2]).abs()
                + 0.5 * ((s[1][1] - s[1][3]).abs() + (s[3][1] - s[3][3]).abs())
                + 0.5 * ((s[1][0] - s[1][2]).abs() + (s[3][0] - s[3][2]).abs());
            let g_ne = (s[1][3] - s[3][1]).abs()
                + (s[0][4] - s[2][2]).abs()
                + 0.5 * ((s[1][2] - s[2][1]).abs() + (s[2][3] - s[3][2]).abs())
                + 0.5 * ((s[0][3] - s[1][2]).abs() + (s[1][4] - s[2][3]).abs());
            let g_se = (s[3][3] - s[1][1]).abs()
                + (s[4][4] - s[2][2]).abs()
                + 0.5 * ((s[2][3] - s[1][2]).abs() + (s[3][2] - s[2][1]).abs())
                + 0.5 * ((s[3][4] - s[2][3]).abs() + (s[4][3] - s[3][2]).abs());
            let g_nw = (s[1][1] - s[3][3]).abs()
                + (s[0][0] - s[2][2]).abs()
                + 0.5 * ((s[1][2] - s[2][3]).abs() + (s[2][1] - s[3][2]).abs())
                + 0.5 * ((s[0][1] - s[1][2]).abs() + (s[1][0] - s[2][1]).abs());
            let g_sw = (s[3][1] - s[1][3]).abs()
                + (s[4][0] - s[2][2]).abs()
                + 0.5 * ((s[3][2] - s[2][3]).abs() + (s[2][1] - s[1][2]).abs())
                + 0.5 * ((s[4][1] - s[3][2]).abs() + (s[3][0] - s[2][1]).abs());

            let grads = [g_n, g_e, g_s, g_w, g_ne, g_se, g_nw, g_sw];
            let min_g = grads.into_iter().fold(f32::INFINITY, f32::min);
            let max_g = grads.into_iter().fold(0.0f32, f32::max);
            let thresh = (1.5 * min_g + 0.5 * (max_g - min_g) * 0.25).max(EPS);

            let own = m.cfa.color_at(x, y) as usize;
            let c_val = s[2][2];

            // For each of the 8 directions, compute local [R, G, B] estimate
            let h_color = m.cfa.color_at((x + 1) % m.cfa.width, y % m.cfa.height) as usize;
            let v_color = m.cfa.color_at(x % m.cfa.width, (y + 1) % m.cfa.height) as usize;

            let dir_rgb = |dir: usize| -> [f32; 3] {
                let mut rgb = [0.0f32; 3];
                if own == 1 {
                    // Green center: h_color is at E/W, v_color is at N/S
                    let (g_est, h_est, v_est) = match dir {
                        0 => (0.5 * (c_val + s[0][2]), 0.5 * (s[2][1] + s[2][3] + s[0][1] + s[0][3]) * 0.5, s[1][2]),
                        1 => (0.5 * (c_val + s[2][4]), s[2][3], 0.5 * (s[1][2] + s[3][2] + s[1][4] + s[3][4]) * 0.5),
                        2 => (0.5 * (c_val + s[4][2]), 0.5 * (s[2][1] + s[2][3] + s[4][1] + s[4][3]) * 0.5, s[3][2]),
                        3 => (0.5 * (c_val + s[2][0]), s[2][1], 0.5 * (s[1][2] + s[3][2] + s[1][0] + s[3][0]) * 0.5),
                        4 => (s[1][3], s[2][3], s[1][2]),
                        5 => (s[3][3], s[2][3], s[3][2]),
                        6 => (s[1][1], s[2][1], s[1][2]),
                        _ => (s[3][1], s[2][1], s[3][2]),
                    };
                    rgb[1] = g_est;
                    rgb[h_color] = h_est;
                    rgb[v_color] = v_est;
                } else {
                    // R or B center: G is at N/E/S/W, opposite RB is at NW/NE/SW/SE
                    let opp = 2 - own;
                    let (own_est, g_est, opp_est) = match dir {
                        0 => (0.5 * (c_val + s[0][2]), s[1][2], 0.5 * (s[1][1] + s[1][3])),
                        1 => (0.5 * (c_val + s[2][4]), s[2][3], 0.5 * (s[1][3] + s[3][3])),
                        2 => (0.5 * (c_val + s[4][2]), s[3][2], 0.5 * (s[3][1] + s[3][3])),
                        3 => (0.5 * (c_val + s[2][0]), s[2][1], 0.5 * (s[1][1] + s[3][1])),
                        4 => (0.5 * (c_val + s[0][4]), 0.5 * (s[1][2] + s[2][3]), s[1][3]),
                        5 => (0.5 * (c_val + s[4][4]), 0.5 * (s[3][2] + s[2][3]), s[3][3]),
                        6 => (0.5 * (c_val + s[0][0]), 0.5 * (s[1][2] + s[2][1]), s[1][1]),
                        _ => (0.5 * (c_val + s[4][0]), 0.5 * (s[3][2] + s[2][1]), s[3][1]),
                    };
                    rgb[own] = own_est;
                    rgb[1] = g_est;
                    rgb[opp] = opp_est;
                }
                rgb
            };

            let mut sum = [0.0f32; 3];
            let mut count = 0.0f32;
            for (d, &g) in grads.iter().enumerate() {
                if g <= thresh {
                    let est = dir_rgb(d);
                    for c in 0..3 {
                        sum[c] += est[c];
                    }
                    count += 1.0;
                }
            }
            if count == 0.0 {
                let est = dir_rgb(0);
                sum = est;
                count = 1.0;
            }
            let avg = sum.map(|v| v / count);
            let mut out_px = [0.0f32; 3];
            for c in 0..3 {
                if c == own {
                    out_px[c] = c_val;
                } else {
                    out_px[c] = (c_val + (avg[c] - avg[own])).max(0.0);
                }
            }
            *px = out_px;
        }
    })?;
    Ok(out)
}

/// Ratio/Difference Corrected Demosaicing (RCD) for 2×2 RGB Bayer.
///
/// Combines low-pass vertical/horizontal directional discrimination with higher-order second-derivative
/// colour-difference and diagonal adaptive weights (inspired by Luis Sanz Rodríguez's RCD algorithm
/// in RawTherapee / darktable), adapted to preserve unclamped scene-linear HDR and exact measured CFA sites.
pub(super) fn rcd(m: &Mosaic, cancelled: &impl Fn() -> bool) -> Result<Rgb32f, DevelopError> {
    #[derive(Default)]
    struct RcdScratch {
        mos: Vec<f32>,
        col: Vec<u8>,
        vh_dir: Vec<f32>,
        g: Vec<f32>,
        rgb: Vec<[f32; 3]>,
    }
    const MARGIN: usize = 5;
    run_tiled_bayer(
        m,
        cancelled,
        |m, x0, y0, tw, th, scratch: &mut RcdScratch, out| {
            let (bw, bh) = (tw + 2 * MARGIN, th + 2 * MARGIN);
            let n = bw * bh;
            scratch.mos.resize(n, 0.0);
            scratch.col.resize(n, 0);
            scratch.vh_dir.resize(n, 0.5);
            scratch.g.resize(n, 0.0);
            scratch.rgb.resize(n, [0.0; 3]);

            let (ox, oy) = (x0 as isize - MARGIN as isize, y0 as isize - MARGIN as isize);
            for y in 0..bh {
                for x in 0..bw {
                    let idx = y * bw + x;
                    scratch.mos[idx] = m.get(ox + x as isize, oy + y as isize);
                    scratch.col[idx] = m.color(ox + x as isize, oy + y as isize);
                }
            }
            let cl = |x: isize, y: isize| -> usize {
                y.clamp(0, bh as isize - 1) as usize * bw + x.clamp(0, bw as isize - 1) as usize
            };

            // Step 1: Vertical vs Horizontal directional discrimination
            for y in 2..bh as isize - 2 {
                for x in 2..bw as isize - 2 {
                    let i = cl(x, y);
                    let c = scratch.mos[i];
                    let v1 = scratch.mos[cl(x, y - 1)] - scratch.mos[cl(x, y + 1)];
                    let v2 = 2.0 * c - scratch.mos[cl(x, y - 2)] - scratch.mos[cl(x, y + 2)];
                    let h1 = scratch.mos[cl(x - 1, y)] - scratch.mos[cl(x + 1, y)];
                    let h2 = 2.0 * c - scratch.mos[cl(x - 2, y)] - scratch.mos[cl(x + 2, y)];

                    let v_stat = (v1 * v1 + 0.5 * v2 * v2).max(EPS * EPS);
                    let h_stat = (h1 * h1 + 0.5 * h2 * h2).max(EPS * EPS);
                    scratch.vh_dir[i] = v_stat / (v_stat + h_stat);
                }
            }

            // Step 2: Directional Green interpolation at R/B sites
            for y in 0..bh as isize {
                for x in 0..bw as isize {
                    let i = cl(x, y);
                    let c = scratch.mos[i];
                    if scratch.col[i] == 1 {
                        scratch.g[i] = c;
                        continue;
                    }
                    let (l, r) = (scratch.mos[cl(x - 1, y)], scratch.mos[cl(x + 1, y)]);
                    let (u, d) = (scratch.mos[cl(x, y - 1)], scratch.mos[cl(x, y + 1)]);
                    let gh = (l.midpoint(r)
                        + (2.0 * c - scratch.mos[cl(x - 2, y)] - scratch.mos[cl(x + 2, y)]) * 0.25)
                        .clamp(l.min(r), l.max(r));
                    let gv = (u.midpoint(d)
                        + (2.0 * c - scratch.mos[cl(x, y - 2)] - scratch.mos[cl(x, y + 2)]) * 0.25)
                        .clamp(u.min(d), u.max(d));

                    // Smooth directional weight with 4 cardinal neighbours
                    let vh_c = scratch.vh_dir[i];
                    let vh_n = 0.25
                        * (scratch.vh_dir[cl(x - 1, y)]
                            + scratch.vh_dir[cl(x + 1, y)]
                            + scratch.vh_dir[cl(x, y - 1)]
                            + scratch.vh_dir[cl(x, y + 1)]);
                    let w_h = 0.5 * (vh_c + vh_n);
                    scratch.g[i] = w_h * gh + (1.0 - w_h) * gv;
                }
            }

            // Step 3: Diagonal R@B and B@R interpolation using directional colour-difference gradients
            for y in 0..bh as isize {
                for x in 0..bw as isize {
                    let i = cl(x, y);
                    let own = scratch.col[i] as usize;
                    let gp = scratch.g[i];
                    scratch.rgb[i][1] = gp;
                    if own != 1 {
                        scratch.rgb[i][own] = scratch.mos[i];
                        let d = |dx: isize, dy: isize| {
                            let j = cl(x + dx, y + dy);
                            scratch.mos[j] - scratch.g[j]
                        };
                        let d_nw = d(-1, -1);
                        let d_se = d(1, 1);
                        let d_ne = d(1, -1);
                        let d_sw = d(-1, 1);
                        let w_nw_se = 1.0 / (EPS + (d_nw - d_se).abs());
                        let w_ne_sw = 1.0 / (EPS + (d_ne - d_sw).abs());
                        let est = (w_nw_se * 0.5 * (d_nw + d_se) + w_ne_sw * 0.5 * (d_ne + d_sw))
                            / (w_nw_se + w_ne_sw);
                        scratch.rgb[i][2 - own] = gp + est;
                    }
                }
            }

            // Step 4: R and B at Green sites using both cardinal and completed diagonal neighbours
            for y in 0..th {
                let by = (y + MARGIN) as isize;
                for x in 0..tw {
                    let bx = (x + MARGIN) as isize;
                    let i = cl(bx, by);
                    let own = scratch.col[i] as usize;
                    if own != 1 {
                        out[y * tw + x] = scratch.rgb[i];
                        continue;
                    }
                    let gp = scratch.g[i];
                    let mut p = [0.0f32; 3];
                    p[1] = gp;
                    for c in [0usize, 2] {
                        let d = |dx: isize, dy: isize| {
                            let j = cl(bx + dx, by + dy);
                            scratch.rgb[j][c] - scratch.g[j]
                        };
                        let dl = d(-1, 0);
                        let dr = d(1, 0);
                        let du = d(0, -1);
                        let dd = d(0, 1);
                        let wh = 1.0 / (EPS + (dl - dr).abs());
                        let wv = 1.0 / (EPS + (du - dd).abs());
                        p[c] = gp + (wh * 0.5 * (dl + dr) + wv * 0.5 * (du + dd)) / (wh + wv);
                    }
                    out[y * tw + x] = p;
                }
            }
        },
    )
}

/// Linear Minimum Mean Square Error (LMMSE) directional demosaicing for 2×2 RGB Bayer.
///
/// Reference: L. Zhang and X. Wu, "Color demosaicking via directional linear minimum mean square-error
/// estimation", IEEE Trans. Image Processing 14(12), 2005.
///
/// Estimates horizontal and vertical primary colour-difference signals `(G - R/B)`, applies a 5-tap
/// low-pass smoothing filter along each axis, computes local empirical mean and variance over a 5-sample
/// window to form the minimum mean-square error (MMSE) estimate, and blends horizontal and vertical
/// MMSE estimates inversely proportional to their estimation variances.
pub(super) fn lmmse(m: &Mosaic, cancelled: &impl Fn() -> bool) -> Result<Rgb32f, DevelopError> {
    #[derive(Default)]
    struct LmmseScratch {
        mos: Vec<f32>,
        col: Vec<u8>,
        dh: Vec<f32>,
        dv: Vec<f32>,
        sh: Vec<f32>,
        sv: Vec<f32>,
        g: Vec<f32>,
        rgb: Vec<[f32; 3]>,
    }
    const MARGIN: usize = 6;
    run_tiled_bayer(
        m,
        cancelled,
        |m, x0, y0, tw, th, scratch: &mut LmmseScratch, out| {
            let (bw, bh) = (tw + 2 * MARGIN, th + 2 * MARGIN);
            let n = bw * bh;
            scratch.mos.resize(n, 0.0);
            scratch.col.resize(n, 0);
            scratch.dh.resize(n, 0.0);
            scratch.dv.resize(n, 0.0);
            scratch.sh.resize(n, 0.0);
            scratch.sv.resize(n, 0.0);
            scratch.g.resize(n, 0.0);
            scratch.rgb.resize(n, [0.0; 3]);

            let (ox, oy) = (x0 as isize - MARGIN as isize, y0 as isize - MARGIN as isize);
            for y in 0..bh {
                for x in 0..bw {
                    let idx = y * bw + x;
                    scratch.mos[idx] = m.get(ox + x as isize, oy + y as isize);
                    scratch.col[idx] = m.color(ox + x as isize, oy + y as isize);
                }
            }
            let cl = |x: isize, y: isize| -> usize {
                y.clamp(0, bh as isize - 1) as usize * bw + x.clamp(0, bw as isize - 1) as usize
            };

            // 1. Directional colour difference (G - M)
            for y in 0..bh as isize {
                for x in 0..bw as isize {
                    let i = cl(x, y);
                    let c = scratch.mos[i];
                    let (l, r) = (scratch.mos[cl(x - 1, y)], scratch.mos[cl(x + 1, y)]);
                    let (u, d) = (scratch.mos[cl(x, y - 1)], scratch.mos[cl(x, y + 1)]);
                    let est_h =
                        l.midpoint(r) + (2.0 * c - scratch.mos[cl(x - 2, y)] - scratch.mos[cl(x + 2, y)]) * 0.25;
                    let est_v =
                        u.midpoint(d) + (2.0 * c - scratch.mos[cl(x, y - 2)] - scratch.mos[cl(x, y + 2)]) * 0.25;
                    if scratch.col[i] == 1 {
                        scratch.dh[i] = c - est_h;
                        scratch.dv[i] = c - est_v;
                    } else {
                        scratch.dh[i] = est_h.clamp(l.min(r), l.max(r)) - c;
                        scratch.dv[i] = est_v.clamp(u.min(d), u.max(d)) - c;
                    }
                }
            }

            // 2. Low-pass 5-tap filter along H and V
            for y in 0..bh as isize {
                for x in 0..bw as isize {
                    let i = cl(x, y);
                    scratch.sh[i] = (scratch.dh[cl(x - 2, y)]
                        + 2.0 * scratch.dh[cl(x - 1, y)]
                        + 2.0 * scratch.dh[i]
                        + 2.0 * scratch.dh[cl(x + 1, y)]
                        + scratch.dh[cl(x + 2, y)])
                        * 0.125;
                    scratch.sv[i] = (scratch.dv[cl(x, y - 2)]
                        + 2.0 * scratch.dv[cl(x, y - 1)]
                        + 2.0 * scratch.dv[i]
                        + 2.0 * scratch.dv[cl(x, y + 1)]
                        + scratch.dv[cl(x, y + 2)])
                        * 0.125;
                }
            }

            // 3. LMMSE fusion of horizontal and vertical green estimates at R/B sites
            for y in 0..bh as isize {
                for x in 0..bw as isize {
                    let i = cl(x, y);
                    if scratch.col[i] == 1 {
                        scratch.g[i] = scratch.mos[i];
                        continue;
                    }
                    let mut var_h = 0.0f32;
                    let mut var_v = 0.0f32;
                    for d in -2isize..=2 {
                        let jh = cl(x + d, y);
                        let dh_err = scratch.dh[jh] - scratch.sh[jh];
                        var_h += dh_err * dh_err;

                        let jv = cl(x, y + d);
                        let dv_err = scratch.dv[jv] - scratch.sv[jv];
                        var_v += dv_err * dv_err;
                    }
                    let wh = 1.0 / (EPS * EPS + var_h);
                    let wv = 1.0 / (EPS * EPS + var_v);
                    let d_fused = (wh * scratch.sh[i] + wv * scratch.sv[i]) / (wh + wv);
                    let (l, r, u, d) = (
                        scratch.mos[cl(x - 1, y)],
                        scratch.mos[cl(x + 1, y)],
                        scratch.mos[cl(x, y - 1)],
                        scratch.mos[cl(x, y + 1)],
                    );
                    let g_min = l.min(r).min(u).min(d);
                    let g_max = l.max(r).max(u).max(d);
                    scratch.g[i] = (scratch.mos[i] + d_fused).clamp(g_min, g_max);
                }
            }

            // 4. Complete R and B using the LMMSE green plane
            for y in 0..bh as isize {
                for x in 0..bw as isize {
                    let i = cl(x, y);
                    let own = scratch.col[i] as usize;
                    let gp = scratch.g[i];
                    scratch.rgb[i][1] = gp;
                    if own != 1 {
                        scratch.rgb[i][own] = scratch.mos[i];
                        let d = |dx: isize, dy: isize| {
                            let j = cl(x + dx, y + dy);
                            scratch.mos[j] - scratch.g[j]
                        };
                        let d_nw = d(-1, -1);
                        let d_se = d(1, 1);
                        let d_ne = d(1, -1);
                        let d_sw = d(-1, 1);
                        let w1 = 1.0 / (EPS + (d_nw - d_se).abs());
                        let w2 = 1.0 / (EPS + (d_ne - d_sw).abs());
                        scratch.rgb[i][2 - own] =
                            gp + (w1 * 0.5 * (d_nw + d_se) + w2 * 0.5 * (d_ne + d_sw)) / (w1 + w2);
                    }
                }
            }

            for y in 0..th {
                let by = (y + MARGIN) as isize;
                for x in 0..tw {
                    let bx = (x + MARGIN) as isize;
                    let i = cl(bx, by);
                    let own = scratch.col[i] as usize;
                    if own != 1 {
                        out[y * tw + x] = scratch.rgb[i];
                        continue;
                    }
                    let gp = scratch.g[i];
                    let mut p = [0.0f32; 3];
                    p[1] = gp;
                    let ch = scratch.col[cl(bx + 1, by)] as usize;
                    let cv = scratch.col[cl(bx, by + 1)] as usize;
                    let d = |dx: isize, dy: isize, c: usize| {
                        let j = cl(bx + dx, by + dy);
                        scratch.rgb[j][c] - scratch.g[j]
                    };
                    p[ch] = gp + 0.5 * (d(-1, 0, ch) + d(1, 0, ch));
                    p[cv] = gp + 0.5 * (d(0, -1, cv) + d(0, 1, cv));
                    out[y * tw + x] = p;
                }
            }
        },
    )
}

/// DCB (Jacek Gozdz) directional colour-difference demosaicing with iterative green & chroma refinement
/// for 2×2 RGB Bayer mosaics.
///
/// Reconstructs horizontal and vertical green planes using 5-tap second-order differences, evaluates
/// directional colour-difference smoothness in a 3×3 neighbourhood, refines Green with a second pass
/// weighted by local colour-difference consistency, and completes R/B with diagonal and axial colour-difference
/// refinement while keeping measured CFA sites bit-for-bit exact.
pub(super) fn dcb(m: &Mosaic, cancelled: &impl Fn() -> bool) -> Result<Rgb32f, DevelopError> {
    #[derive(Default)]
    struct DcbScratch {
        mos: Vec<f32>,
        col: Vec<u8>,
        gh: Vec<f32>,
        gv: Vec<f32>,
        g: Vec<f32>,
        rgb: Vec<[f32; 3]>,
    }
    const MARGIN: usize = 6;
    run_tiled_bayer(
        m,
        cancelled,
        |m, x0, y0, tw, th, scratch: &mut DcbScratch, out| {
            let (bw, bh) = (tw + 2 * MARGIN, th + 2 * MARGIN);
            let n = bw * bh;
            scratch.mos.resize(n, 0.0);
            scratch.col.resize(n, 0);
            scratch.gh.resize(n, 0.0);
            scratch.gv.resize(n, 0.0);
            scratch.g.resize(n, 0.0);
            scratch.rgb.resize(n, [0.0; 3]);

            let (ox, oy) = (x0 as isize - MARGIN as isize, y0 as isize - MARGIN as isize);
            for y in 0..bh {
                for x in 0..bw {
                    let idx = y * bw + x;
                    scratch.mos[idx] = m.get(ox + x as isize, oy + y as isize);
                    scratch.col[idx] = m.color(ox + x as isize, oy + y as isize);
                }
            }
            let cl = |x: isize, y: isize| -> usize {
                y.clamp(0, bh as isize - 1) as usize * bw + x.clamp(0, bw as isize - 1) as usize
            };

            // Pass 1: H and V green candidates
            for y in 2..bh as isize - 2 {
                for x in 2..bw as isize - 2 {
                    let i = cl(x, y);
                    let c = scratch.mos[i];
                    if scratch.col[i] == 1 {
                        scratch.gh[i] = c;
                        scratch.gv[i] = c;
                        continue;
                    }
                    let w1 = scratch.mos[cl(x - 1, y)];
                    let e1 = scratch.mos[cl(x + 1, y)];
                    let n1 = scratch.mos[cl(x, y - 1)];
                    let s1 = scratch.mos[cl(x, y + 1)];
                    let w2 = scratch.mos[cl(x - 2, y)];
                    let e2 = scratch.mos[cl(x + 2, y)];
                    let n2 = scratch.mos[cl(x, y - 2)];
                    let s2 = scratch.mos[cl(x, y + 2)];
                    scratch.gh[i] = (0.5 * (w1 + e1) + 0.25 * (2.0 * c - w2 - e2)).clamp(w1.min(e1), w1.max(e1));
                    scratch.gv[i] = (0.5 * (n1 + s1) + 0.25 * (2.0 * c - n2 - s2)).clamp(n1.min(s1), n1.max(s1));
                }
            }

            // Pass 2: Iterative DCB directional green selection & refinement using 3×3 colour-difference variation
            for y in 3..bh as isize - 3 {
                for x in 3..bw as isize - 3 {
                    let i = cl(x, y);
                    if scratch.col[i] == 1 {
                        scratch.g[i] = scratch.mos[i];
                        continue;
                    }
                    let c = scratch.mos[i];
                    let dh = (c - scratch.gh[i] - (scratch.mos[cl(x - 2, y)] - scratch.gh[cl(x - 2, y)])).abs()
                        + (c - scratch.gh[i] - (scratch.mos[cl(x + 2, y)] - scratch.gh[cl(x + 2, y)])).abs()
                        + (scratch.gh[cl(x - 1, y)] - scratch.gh[cl(x + 1, y)]).abs();
                    let dv = (c - scratch.gv[i] - (scratch.mos[cl(x, y - 2)] - scratch.gv[cl(x, y - 2)])).abs()
                        + (c - scratch.gv[i] - (scratch.mos[cl(x, y + 2)] - scratch.gv[cl(x, y + 2)])).abs()
                        + (scratch.gv[cl(x, y - 1)] - scratch.gv[cl(x, y + 1)]).abs();
                    let wh = 1.0 / (EPS + dh * dh);
                    let wv = 1.0 / (EPS + dv * dv);
                    scratch.g[i] = (wh * scratch.gh[i] + wv * scratch.gv[i]) / (wh + wv);
                }
            }

            // Pass 3: R/B at opposite R/B sites using diagonal gradient weights
            for y in 4..bh as isize - 4 {
                for x in 4..bw as isize - 4 {
                    let i = cl(x, y);
                    let own = scratch.col[i] as usize;
                    if own == 1 {
                        continue;
                    }
                    let opp = 2 - own;
                    let gp = scratch.g[i];
                    let nw = cl(x - 1, y - 1);
                    let ne = cl(x + 1, y - 1);
                    let sw = cl(x - 1, y + 1);
                    let se = cl(x + 1, y + 1);
                    let d_nw = scratch.mos[nw] - scratch.g[nw];
                    let d_se = scratch.mos[se] - scratch.g[se];
                    let d_ne = scratch.mos[ne] - scratch.g[ne];
                    let d_sw = scratch.mos[sw] - scratch.g[sw];
                    let w1 = 1.0 / (EPS + (d_nw - d_se).abs() + (scratch.mos[nw] - scratch.mos[se]).abs());
                    let w2 = 1.0 / (EPS + (d_ne - d_sw).abs() + (scratch.mos[ne] - scratch.mos[sw]).abs());
                    let mut p = [0.0f32; 3];
                    p[own] = scratch.mos[i];
                    p[1] = gp;
                    p[opp] = gp + (w1 * 0.5 * (d_nw + d_se) + w2 * 0.5 * (d_ne + d_sw)) / (w1 + w2);
                    scratch.rgb[i] = p;
                }
            }

            // Pass 4: R and B at Green sites with directional colour-difference weights
            for y in 0..th {
                let by = (y + MARGIN) as isize;
                for x in 0..tw {
                    let bx = (x + MARGIN) as isize;
                    let i = cl(bx, by);
                    let own = scratch.col[i] as usize;
                    if own != 1 {
                        out[y * tw + x] = scratch.rgb[i];
                        continue;
                    }
                    let gp = scratch.g[i];
                    let mut p = [0.0f32; 3];
                    p[1] = gp;
                    let ch = scratch.col[cl(bx + 1, by)] as usize;
                    let cv = scratch.col[cl(bx, by + 1)] as usize;
                    let w1 = cl(bx - 1, by);
                    let e1 = cl(bx + 1, by);
                    let n1 = cl(bx, by - 1);
                    let s1 = cl(bx, by + 1);
                    p[ch] = gp + 0.5 * ((scratch.rgb[w1][ch] - scratch.g[w1]) + (scratch.rgb[e1][ch] - scratch.g[e1]));
                    p[cv] = gp + 0.5 * ((scratch.rgb[n1][cv] - scratch.g[n1]) + (scratch.rgb[s1][cv] - scratch.g[s1]));
                    out[y * tw + x] = p;
                }
            }
        },
    )
}

/// IGV (Ingo Weyrich / RawTherapee) variance-weighted anisotropic Gaussian gradient demosaicing
/// for noisy / high-ISO 2×2 RGB Bayer sensors.
///
/// Computes N, S, W, E directional colour-difference estimates and weights them inversely by local
/// directional variance in a 5×5 window so flat/noisy areas receive isotropic low-noise smoothing
/// while edges preserve directional sharpness.
pub(super) fn igv(m: &Mosaic, cancelled: &impl Fn() -> bool) -> Result<Rgb32f, DevelopError> {
    #[derive(Default)]
    struct IgvScratch {
        mos: Vec<f32>,
        col: Vec<u8>,
        g: Vec<f32>,
        rgb: Vec<[f32; 3]>,
    }
    const MARGIN: usize = 6;
    run_tiled_bayer(
        m,
        cancelled,
        |m, x0, y0, tw, th, scratch: &mut IgvScratch, out| {
            let (bw, bh) = (tw + 2 * MARGIN, th + 2 * MARGIN);
            let n = bw * bh;
            scratch.mos.resize(n, 0.0);
            scratch.col.resize(n, 0);
            scratch.g.resize(n, 0.0);
            scratch.rgb.resize(n, [0.0; 3]);

            let (ox, oy) = (x0 as isize - MARGIN as isize, y0 as isize - MARGIN as isize);
            for y in 0..bh {
                for x in 0..bw {
                    let idx = y * bw + x;
                    scratch.mos[idx] = m.get(ox + x as isize, oy + y as isize);
                    scratch.col[idx] = m.color(ox + x as isize, oy + y as isize);
                }
            }
            let cl = |x: isize, y: isize| -> usize {
                y.clamp(0, bh as isize - 1) as usize * bw + x.clamp(0, bw as isize - 1) as usize
            };

            // Step 1: 4-direction (N, S, W, E) variance-weighted Green reconstruction
            for y in 3..bh as isize - 3 {
                for x in 3..bw as isize - 3 {
                    let i = cl(x, y);
                    let c = scratch.mos[i];
                    if scratch.col[i] == 1 {
                        scratch.g[i] = c;
                        continue;
                    }
                    let n1 = scratch.mos[cl(x, y - 1)];
                    let s1 = scratch.mos[cl(x, y + 1)];
                    let w1 = scratch.mos[cl(x - 1, y)];
                    let e1 = scratch.mos[cl(x + 1, y)];
                    let n2 = scratch.mos[cl(x, y - 2)];
                    let s2 = scratch.mos[cl(x, y + 2)];
                    let w2 = scratch.mos[cl(x - 2, y)];
                    let e2 = scratch.mos[cl(x + 2, y)];

                    let gn = n1 + 0.5 * (c - n2);
                    let gs = s1 + 0.5 * (c - s2);
                    let gw = w1 + 0.5 * (c - w2);
                    let ge = e1 + 0.5 * (c - e2);

                    let vn = EPS
                        + (n1 - s1).powi(2)
                        + (c - n2).powi(2)
                        + (scratch.mos[cl(x - 1, y - 1)] - scratch.mos[cl(x + 1, y - 1)]).powi(2);
                    let vs = EPS
                        + (s1 - n1).powi(2)
                        + (c - s2).powi(2)
                        + (scratch.mos[cl(x - 1, y + 1)] - scratch.mos[cl(x + 1, y + 1)]).powi(2);
                    let vw = EPS
                        + (w1 - e1).powi(2)
                        + (c - w2).powi(2)
                        + (scratch.mos[cl(x - 1, y - 1)] - scratch.mos[cl(x - 1, y + 1)]).powi(2);
                    let ve = EPS
                        + (e1 - w1).powi(2)
                        + (c - e2).powi(2)
                        + (scratch.mos[cl(x + 1, y - 1)] - scratch.mos[cl(x + 1, y + 1)]).powi(2);

                    let wn = 1.0 / vn;
                    let ws = 1.0 / vs;
                    let ww = 1.0 / vw;
                    let we = 1.0 / ve;
                    let g_min = n1.min(s1).min(w1).min(e1);
                    let g_max = n1.max(s1).max(w1).max(e1);
                    scratch.g[i] = ((wn * gn + ws * gs + ww * gw + we * ge) / (wn + ws + ww + we)).clamp(g_min, g_max);
                }
            }

            // Step 2: R/B at opposite R/B sites
            for y in 4..bh as isize - 4 {
                for x in 4..bw as isize - 4 {
                    let i = cl(x, y);
                    let own = scratch.col[i] as usize;
                    if own == 1 {
                        continue;
                    }
                    let opp = 2 - own;
                    let gp = scratch.g[i];
                    let nw = cl(x - 1, y - 1);
                    let ne = cl(x + 1, y - 1);
                    let sw = cl(x - 1, y + 1);
                    let se = cl(x + 1, y + 1);
                    let d_nw = scratch.mos[nw] - scratch.g[nw];
                    let d_se = scratch.mos[se] - scratch.g[se];
                    let d_ne = scratch.mos[ne] - scratch.g[ne];
                    let d_sw = scratch.mos[sw] - scratch.g[sw];
                    let w1 = 1.0 / (EPS + (d_nw - d_se).powi(2) + (scratch.mos[nw] - scratch.mos[se]).powi(2));
                    let w2 = 1.0 / (EPS + (d_ne - d_sw).powi(2) + (scratch.mos[ne] - scratch.mos[sw]).powi(2));
                    let mut p = [0.0f32; 3];
                    p[own] = scratch.mos[i];
                    p[1] = gp;
                    p[opp] = gp + (w1 * 0.5 * (d_nw + d_se) + w2 * 0.5 * (d_ne + d_sw)) / (w1 + w2);
                    scratch.rgb[i] = p;
                }
            }

            // Step 3: R/B at Green sites
            for y in 0..th {
                let by = (y + MARGIN) as isize;
                for x in 0..tw {
                    let bx = (x + MARGIN) as isize;
                    let i = cl(bx, by);
                    let own = scratch.col[i] as usize;
                    if own != 1 {
                        out[y * tw + x] = scratch.rgb[i];
                        continue;
                    }
                    let gp = scratch.g[i];
                    let mut p = [0.0f32; 3];
                    p[1] = gp;
                    let ch = scratch.col[cl(bx + 1, by)] as usize;
                    let cv = scratch.col[cl(bx, by + 1)] as usize;
                    let w1 = cl(bx - 1, by);
                    let e1 = cl(bx + 1, by);
                    let n1 = cl(bx, by - 1);
                    let s1 = cl(bx, by + 1);
                    p[ch] = gp + 0.5 * ((scratch.rgb[w1][ch] - scratch.g[w1]) + (scratch.rgb[e1][ch] - scratch.g[e1]));
                    p[cv] = gp + 0.5 * ((scratch.rgb[n1][cv] - scratch.g[n1]) + (scratch.rgb[s1][cv] - scratch.g[s1]));
                    out[y * tw + x] = p;
                }
            }
        },
    )
}

/// AMaZE (Aliasing Minimization and Zipper Elimination, Emil Martinec) demosaicing for 2×2 RGB Bayer.
///
/// Evaluates multi-scale directional gradients (5×5 and 7×7 cardinal and diagonal stencils), computes
/// directional colour-difference and ratio-corrected green candidates (`N`, `S`, `W`, `E`), blends them
/// using local sub-lattice consistency weights that suppress Nyqist-frequency checkerboards and zippering,
/// and completes R/B via diagonal and cardinal curvature-corrected colour differences.
pub(super) fn amaze(m: &Mosaic, cancelled: &impl Fn() -> bool) -> Result<Rgb32f, DevelopError> {
    #[derive(Default)]
    struct AmazeScratch {
        mos: Vec<f32>,
        col: Vec<u8>,
        dg_h: Vec<f32>,
        dg_v: Vec<f32>,
        g: Vec<f32>,
        rgb: Vec<[f32; 3]>,
    }
    const MARGIN: usize = 8;
    run_tiled_bayer(
        m,
        cancelled,
        |m, x0, y0, tw, th, scratch: &mut AmazeScratch, out| {
            let (bw, bh) = (tw + 2 * MARGIN, th + 2 * MARGIN);
            let n = bw * bh;
            scratch.mos.resize(n, 0.0);
            scratch.col.resize(n, 0);
            scratch.dg_h.resize(n, 0.0);
            scratch.dg_v.resize(n, 0.0);
            scratch.g.resize(n, 0.0);
            scratch.rgb.resize(n, [0.0; 3]);

            let (ox, oy) = (x0 as isize - MARGIN as isize, y0 as isize - MARGIN as isize);
            for y in 0..bh {
                for x in 0..bw {
                    let idx = y * bw + x;
                    scratch.mos[idx] = m.get(ox + x as isize, oy + y as isize);
                    scratch.col[idx] = m.color(ox + x as isize, oy + y as isize);
                }
            }
            let cl = |x: isize, y: isize| -> usize {
                y.clamp(0, bh as isize - 1) as usize * bw + x.clamp(0, bw as isize - 1) as usize
            };

            // Stage 1: Multi-scale Hamilton–Adams + 4th-order directional colour differences (G - C)
            for y in 3..bh as isize - 3 {
                for x in 3..bw as isize - 3 {
                    let i = cl(x, y);
                    let c = scratch.mos[i];
                    if scratch.col[i] == 1 {
                        scratch.dg_h[i] = 0.0;
                        scratch.dg_v[i] = 0.0;
                        continue;
                    }
                    let w1 = scratch.mos[cl(x - 1, y)];
                    let e1 = scratch.mos[cl(x + 1, y)];
                    let n1 = scratch.mos[cl(x, y - 1)];
                    let s1 = scratch.mos[cl(x, y + 1)];
                    let w2 = scratch.mos[cl(x - 2, y)];
                    let e2 = scratch.mos[cl(x + 2, y)];
                    let n2 = scratch.mos[cl(x, y - 2)];
                    let s2 = scratch.mos[cl(x, y + 2)];
                    let w3 = scratch.mos[cl(x - 3, y)];
                    let e3 = scratch.mos[cl(x + 3, y)];
                    let n3 = scratch.mos[cl(x, y - 3)];
                    let s3 = scratch.mos[cl(x, y + 3)];

                    let gh = (0.5 * (w1 + e1) + 0.25 * (2.0 * c - w2 - e2)
                        - 0.0625 * (w1 - w3 + e1 - e3))
                        .clamp(w1.min(e1), w1.max(e1));
                    let gv = (0.5 * (n1 + s1) + 0.25 * (2.0 * c - n2 - s2)
                        - 0.0625 * (n1 - n3 + s1 - s3))
                        .clamp(n1.min(s1), n1.max(s1));
                    scratch.dg_h[i] = gh - c;
                    scratch.dg_v[i] = gv - c;
                }
            }

            // Stage 2: AMaZE sub-lattice directional discrimination & zipper-free Green blending
            for y in 4..bh as isize - 4 {
                for x in 4..bw as isize - 4 {
                    let i = cl(x, y);
                    let c = scratch.mos[i];
                    if scratch.col[i] == 1 {
                        scratch.g[i] = c;
                        continue;
                    }
                    let mut var_h = 0.0f32;
                    let mut var_v = 0.0f32;
                    for d in [-2isize, 0, 2] {
                        let jh1 = cl(x + d, y);
                        let jh2 = cl(x, y);
                        var_h += (scratch.dg_h[jh1] - scratch.dg_h[jh2]).abs();
                        let jv1 = cl(x, y + d);
                        let jv2 = cl(x, y);
                        var_v += (scratch.dg_v[jv1] - scratch.dg_v[jv2]).abs();
                    }
                    let grad_h = (scratch.mos[cl(x - 1, y)] - scratch.mos[cl(x + 1, y)]).abs()
                        + 0.5 * (2.0 * c - scratch.mos[cl(x - 2, y)] - scratch.mos[cl(x + 2, y)]).abs()
                        + var_h;
                    let grad_v = (scratch.mos[cl(x, y - 1)] - scratch.mos[cl(x, y + 1)]).abs()
                        + 0.5 * (2.0 * c - scratch.mos[cl(x, y - 2)] - scratch.mos[cl(x, y + 2)]).abs()
                        + var_v;

                    let wh = 1.0 / (EPS + grad_h * grad_h);
                    let wv = 1.0 / (EPS + grad_v * grad_v);
                    // Smooth spatial colour difference along the winning orientation
                    let d_h = 0.7 * scratch.dg_h[i]
                        + 0.15 * (scratch.dg_h[cl(x - 2, y)] + scratch.dg_h[cl(x + 2, y)]);
                    let d_v = 0.7 * scratch.dg_v[i]
                        + 0.15 * (scratch.dg_v[cl(x, y - 2)] + scratch.dg_v[cl(x, y + 2)]);
                    let g_min = scratch.mos[cl(x - 1, y)]
                        .min(scratch.mos[cl(x + 1, y)])
                        .min(scratch.mos[cl(x, y - 1)])
                        .min(scratch.mos[cl(x, y + 1)]);
                    let g_max = scratch.mos[cl(x - 1, y)]
                        .max(scratch.mos[cl(x + 1, y)])
                        .max(scratch.mos[cl(x, y - 1)])
                        .max(scratch.mos[cl(x, y + 1)]);
                    scratch.g[i] = (c + (wh * d_h + wv * d_v) / (wh + wv)).clamp(g_min, g_max);
                }
            }

            // Stage 3: Diagonal R/B at opposite R/B sites with 2nd-order green curvature correction
            for y in 5..bh as isize - 5 {
                for x in 5..bw as isize - 5 {
                    let i = cl(x, y);
                    let own = scratch.col[i] as usize;
                    if own == 1 {
                        continue;
                    }
                    let opp = 2 - own;
                    let gp = scratch.g[i];
                    let nw = cl(x - 1, y - 1);
                    let ne = cl(x + 1, y - 1);
                    let sw = cl(x - 1, y + 1);
                    let se = cl(x + 1, y + 1);
                    let d_nw = scratch.mos[nw] - scratch.g[nw];
                    let d_se = scratch.mos[se] - scratch.g[se];
                    let d_ne = scratch.mos[ne] - scratch.g[ne];
                    let d_sw = scratch.mos[sw] - scratch.g[sw];
                    let w_nwse = 1.0
                        / (EPS
                            + (d_nw - d_se).abs()
                            + (scratch.mos[nw] - scratch.mos[se]).abs()
                            + 0.5 * (2.0 * gp - scratch.g[nw] - scratch.g[se]).abs());
                    let w_nesw = 1.0
                        / (EPS
                            + (d_ne - d_sw).abs()
                            + (scratch.mos[ne] - scratch.mos[sw]).abs()
                            + 0.5 * (2.0 * gp - scratch.g[ne] - scratch.g[sw]).abs());
                    let mut p = [0.0f32; 3];
                    p[own] = scratch.mos[i];
                    p[1] = gp;
                    p[opp] = gp + (w_nwse * 0.5 * (d_nw + d_se) + w_nesw * 0.5 * (d_ne + d_sw)) / (w_nwse + w_nesw);
                    scratch.rgb[i] = p;
                }
            }

            // Stage 4: R/B at Green sites
            for y in 0..th {
                let by = (y + MARGIN) as isize;
                for x in 0..tw {
                    let bx = (x + MARGIN) as isize;
                    let i = cl(bx, by);
                    let own = scratch.col[i] as usize;
                    if own != 1 {
                        out[y * tw + x] = scratch.rgb[i];
                        continue;
                    }
                    let gp = scratch.g[i];
                    let mut p = [0.0f32; 3];
                    p[1] = gp;
                    let ch = scratch.col[cl(bx + 1, by)] as usize;
                    let cv = scratch.col[cl(bx, by + 1)] as usize;
                    let w1 = cl(bx - 1, by);
                    let e1 = cl(bx + 1, by);
                    let n1 = cl(bx, by - 1);
                    let s1 = cl(bx, by + 1);
                    p[ch] = gp + 0.5 * ((scratch.rgb[w1][ch] - scratch.g[w1]) + (scratch.rgb[e1][ch] - scratch.g[e1]));
                    p[cv] = gp + 0.5 * ((scratch.rgb[n1][cv] - scratch.g[n1]) + (scratch.rgb[s1][cv] - scratch.g[s1]));
                    out[y * tw + x] = p;
                }
            }
        },
    )
}

/// Contrast-guided Dual Demosaicing: blends a high-acutance edge solver (`sharp`) and a low-variance
/// smooth solver (`smooth`) using a 3×3 local luminance contrast mask.
///
/// Pixels on high-frequency edges (`contrast >= threshold`) favour `sharp`, while flat/low-contrast
/// regions (`contrast < threshold`) favour `smooth`, eliminating labyrinth/maze noise on uniform areas
/// while keeping maximum edge sharpness and exact measured CFA sites.
pub(super) fn dual_demosaic(
    m: &Mosaic,
    sharp: Rgb32f,
    smooth: &Rgb32f,
    threshold: f32,
    cancelled: &impl Fn() -> bool,
) -> Result<Rgb32f, DevelopError> {
    let (w, h) = (m.w, m.h);
    let mut out = sharp;
    if w == 0 || h == 0 {
        return Ok(out);
    }
    let thresh = threshold.max(1e-4);
    let sharp_copy = out.data.clone();
    par_rows(&mut out.data, w, cancelled, |y, row| {
        let yi = y as isize;
        for (x, px) in row.iter_mut().enumerate() {
            let xi = x as isize;
            let mut g_min = f32::INFINITY;
            let mut g_max = f32::NEG_INFINITY;
            let mut g_sum = 0.0f32;
            for dy in -1isize..=1 {
                let sy = reflect(yi + dy, h);
                for dx in -1isize..=1 {
                    let sx = reflect(xi + dx, w);
                    let g = sharp_copy[sy * w + sx][1];
                    g_min = g_min.min(g);
                    g_max = g_max.max(g);
                    g_sum += g;
                }
            }
            let mean = (g_sum / 9.0).abs().max(1e-3);
            let rel_contrast = (g_max - g_min) / mean;
            // Smooth S-curve weight in [0, 1] around `thresh`
            let t = (rel_contrast / (2.0 * thresh)).clamp(0.0, 1.0);
            let alpha = t * t * (3.0 - 2.0 * t);
            let a = sharp_copy[y * w + x];
            let b = smooth.data[y * w + x];
            let mut blended = [
                b[0] + alpha * (a[0] - b[0]),
                b[1] + alpha * (a[1] - b[1]),
                b[2] + alpha * (a[2] - b[2]),
            ];
            let own = m.cfa.color_at(x, y) as usize;
            if own < 3 {
                blended[own] = m.data[y * w + x];
            }
            *px = blended;
        }
    })?;
    Ok(out)
}

/// Multi-direction CIELab homogeneity-directed X-Trans 6×6 demosaicing (Markesteijn-inspired).
///
/// Evaluates horizontal and vertical directional green candidates at every non-green X-Trans site
/// (exploiting the invariant that every R/B site in a 6×6 X-Trans pattern has at least one immediate
/// green neighbour horizontally or vertically and diagonal greens within the 3×3 ring), completes
/// red and blue for each candidate using edge-weighted colour differences, measures CIELab homogeneity
/// across a 3×3 window, and selects the most homogeneous direction.
pub(super) fn xtrans_markesteijn(
    m: &Mosaic,
    cancelled: &impl Fn() -> bool,
) -> Result<Rgb32f, DevelopError> {
    #[derive(Default)]
    struct XTransScratch {
        mos: Vec<f32>,
        col: Vec<u8>,
        gh: Vec<f32>,
        gv: Vec<f32>,
        rh: Vec<[f32; 3]>,
        rv: Vec<[f32; 3]>,
        lh: Vec<[f32; 3]>,
        lv: Vec<[f32; 3]>,
        hh: Vec<u8>,
        hv: Vec<u8>,
    }
    const MARGIN: usize = 6;
    run_tiled_bayer(
        m,
        cancelled,
        |m, x0, y0, tw, th, scratch: &mut XTransScratch, out| {
            let (bw, bh) = (tw + 2 * MARGIN, th + 2 * MARGIN);
            let n = bw * bh;
            scratch.mos.resize(n, 0.0);
            scratch.col.resize(n, 0);
            scratch.gh.resize(n, 0.0);
            scratch.gv.resize(n, 0.0);
            scratch.rh.resize(n, [0.0; 3]);
            scratch.rv.resize(n, [0.0; 3]);
            scratch.lh.resize(n, [0.0; 3]);
            scratch.lv.resize(n, [0.0; 3]);
            scratch.hh.resize(n, 0);
            scratch.hv.resize(n, 0);

            let (ox, oy) = (x0 as isize - MARGIN as isize, y0 as isize - MARGIN as isize);
            for y in 0..bh {
                for x in 0..bw {
                    let idx = y * bw + x;
                    scratch.mos[idx] = m.get(ox + x as isize, oy + y as isize);
                    scratch.col[idx] = m.color(ox + x as isize, oy + y as isize);
                }
            }
            let cl = |x: isize, y: isize| -> usize {
                y.clamp(0, bh as isize - 1) as usize * bw + x.clamp(0, bw as isize - 1) as usize
            };

            // 1. Directional green candidates (H-biased and V-biased) for X-Trans
            for y in 0..bh as isize {
                for x in 0..bw as isize {
                    let i = cl(x, y);
                    let c = scratch.mos[i];
                    if scratch.col[i] == 1 {
                        scratch.gh[i] = c;
                        scratch.gv[i] = c;
                        continue;
                    }
                    // Gather green neighbours in 5×5 with directional weights (H-favoured vs V-favoured)
                    let (mut sh, mut wsh) = (0.0f32, 0.0f32);
                    let (mut sv, mut wsv) = (0.0f32, 0.0f32);
                    let (mut g_min, mut g_max) = (f32::INFINITY, f32::NEG_INFINITY);
                    let grad_h = (scratch.mos[cl(x - 1, y)] - scratch.mos[cl(x + 1, y)]).abs();
                    let grad_v = (scratch.mos[cl(x, y - 1)] - scratch.mos[cl(x, y + 1)]).abs();
                    for dy in -2isize..=2 {
                        for dx in -2isize..=2 {
                            if dx == 0 && dy == 0 {
                                continue;
                            }
                            let j = cl(x + dx, y + dy);
                            if scratch.col[j] != 1 {
                                continue;
                            }
                            let gv = scratch.mos[j];
                            if dx.abs() <= 1 && dy.abs() <= 1 {
                                g_min = g_min.min(gv);
                                g_max = g_max.max(gv);
                            }
                            let d2 = (dx * dx + dy * dy) as f32;
                            let base_w = 1.0 / (d2 * d2);
                            let h_bias = if dy == 0 {
                                4.0
                            } else if dx.abs() >= dy.abs() {
                                1.5
                            } else {
                                0.25
                            };
                            let v_bias = if dx == 0 {
                                4.0
                            } else if dy.abs() >= dx.abs() {
                                1.5
                            } else {
                                0.25
                            };
                            let wh = base_w * h_bias / (EPS + grad_h);
                            let wv = base_w * v_bias / (EPS + grad_v);
                            sh += wh * gv;
                            wsh += wh;
                            sv += wv * gv;
                            wsv += wv;
                        }
                    }
                    let est_h = if wsh > 0.0 { sh / wsh } else { c };
                    let est_v = if wsv > 0.0 { sv / wsv } else { c };
                    if g_min <= g_max {
                        scratch.gh[i] = est_h.clamp(g_min, g_max);
                        scratch.gv[i] = est_v.clamp(g_min, g_max);
                    } else {
                        scratch.gh[i] = est_h;
                        scratch.gv[i] = est_v;
                    }
                }
            }

            // 2. Complete R and B for both candidates
            let complete_rb = |g: &[f32], mos: &[f32], col: &[u8], rgb: &mut [[f32; 3]]| {
                for y in 0..bh as isize {
                    for x in 0..bw as isize {
                        let i = cl(x, y);
                        let own = col[i] as usize;
                        let gp = g[i];
                        let mut p = [gp; 3];
                        for c in [0usize, 2] {
                            if c == own {
                                p[c] = mos[i];
                                continue;
                            }
                            let (mut s, mut ws) = (0.0f32, 0.0f32);
                            for dy in -2isize..=2 {
                                for dx in -2isize..=2 {
                                    let j = cl(x + dx, y + dy);
                                    if col[j] as usize != c {
                                        continue;
                                    }
                                    let gq = g[j];
                                    let d2 = (dx * dx + dy * dy) as f32;
                                    let wt = 1.0 / (d2 * (1e-3 + (gq - gp).abs()));
                                    s += wt * (mos[j] - gq);
                                    ws += wt;
                                }
                            }
                            p[c] = if ws > 0.0 { gp + s / ws } else { gp };
                        }
                        rgb[i] = p;
                    }
                }
            };
            complete_rb(&scratch.gh, &scratch.mos, &scratch.col, &mut scratch.rh);
            complete_rb(&scratch.gv, &scratch.mos, &scratch.col, &mut scratch.rv);

            // 3. CIELab homogeneity evaluation
            for i in 0..n {
                scratch.lh[i] = super::ahd::lab(scratch.rh[i]);
                scratch.lv[i] = super::ahd::lab(scratch.rv[i]);
            }
            let dl = |a: &[f32; 3], b: &[f32; 3]| (a[0] - b[0]).abs();
            let dc = |a: &[f32; 3], b: &[f32; 3]| (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2);
            for y in 1..bh as isize - 1 {
                for x in 1..bw as isize - 1 {
                    let i = cl(x, y);
                    let (l, r, u, d) = (cl(x - 1, y), cl(x + 1, y), cl(x, y - 1), cl(x, y + 1));
                    let eps_l = dl(&scratch.lh[i], &scratch.lh[l])
                        .max(dl(&scratch.lh[i], &scratch.lh[r]))
                        .min(dl(&scratch.lv[i], &scratch.lv[u]).max(dl(&scratch.lv[i], &scratch.lv[d])));
                    let eps_c = dc(&scratch.lh[i], &scratch.lh[l])
                        .max(dc(&scratch.lh[i], &scratch.lh[r]))
                        .min(dc(&scratch.lv[i], &scratch.lv[u]).max(dc(&scratch.lv[i], &scratch.lv[d])));
                    let (mut ch, mut cv) = (0u8, 0u8);
                    for j in [l, r, u, d] {
                        if dl(&scratch.lh[i], &scratch.lh[j]) <= eps_l
                            && dc(&scratch.lh[i], &scratch.lh[j]) <= eps_c
                        {
                            ch += 1;
                        }
                        if dl(&scratch.lv[i], &scratch.lv[j]) <= eps_l
                            && dc(&scratch.lv[i], &scratch.lv[j]) <= eps_c
                        {
                            cv += 1;
                        }
                    }
                    scratch.hh[i] = ch;
                    scratch.hv[i] = cv;
                }
            }

            // 4. Select homogeneous candidate
            for y in 0..th {
                let by = y + MARGIN;
                for x in 0..tw {
                    let bx = x + MARGIN;
                    let (mut sh, mut sv) = (0u32, 0u32);
                    for dy in -1isize..=1 {
                        for dx in -1isize..=1 {
                            let j = cl(bx as isize + dx, by as isize + dy);
                            sh += u32::from(scratch.hh[j]);
                            sv += u32::from(scratch.hv[j]);
                        }
                    }
                    let i = by * bw + bx;
                    out[y * tw + x] = if sh > sv {
                        scratch.rh[i]
                    } else if sv > sh {
                        scratch.rv[i]
                    } else {
                        [
                            scratch.rh[i][0].midpoint(scratch.rv[i][0]),
                            scratch.rh[i][1].midpoint(scratch.rv[i][1]),
                            scratch.rh[i][2].midpoint(scratch.rv[i][2]),
                        ]
                    };
                }
            }
        },
    )
}

/// 3-pass iterative Markesteijn demosaicing for 6×6 Fujifilm X-Trans sensors.
///
/// Extends the 1-pass Markesteijn solver with 3 iterative passes of directional green and
/// colour-difference (`R - G`, `B - G`) refinement across each candidate before evaluating
/// 5×5 CIELab homogeneity, plus a post-selection colour-difference smoothing pass that pins
/// measured CFA channels bit-for-bit.
pub(super) fn xtrans_markesteijn_3pass(
    m: &Mosaic,
    cancelled: &impl Fn() -> bool,
) -> Result<Rgb32f, DevelopError> {
    #[derive(Default)]
    struct XTrans3Scratch {
        mos: Vec<f32>,
        col: Vec<u8>,
        gh: Vec<f32>,
        gv: Vec<f32>,
        rh: Vec<[f32; 3]>,
        rv: Vec<[f32; 3]>,
        lh: Vec<[f32; 3]>,
        lv: Vec<[f32; 3]>,
        hh: Vec<u8>,
        hv: Vec<u8>,
    }
    const MARGIN: usize = 8;
    run_tiled_bayer(
        m,
        cancelled,
        |m, x0, y0, tw, th, scratch: &mut XTrans3Scratch, out| {
            let (bw, bh) = (tw + 2 * MARGIN, th + 2 * MARGIN);
            let n = bw * bh;
            scratch.mos.resize(n, 0.0);
            scratch.col.resize(n, 0);
            scratch.gh.resize(n, 0.0);
            scratch.gv.resize(n, 0.0);
            scratch.rh.resize(n, [0.0; 3]);
            scratch.rv.resize(n, [0.0; 3]);
            scratch.lh.resize(n, [0.0; 3]);
            scratch.lv.resize(n, [0.0; 3]);
            scratch.hh.resize(n, 0);
            scratch.hv.resize(n, 0);

            let (ox, oy) = (x0 as isize - MARGIN as isize, y0 as isize - MARGIN as isize);
            for y in 0..bh {
                for x in 0..bw {
                    let idx = y * bw + x;
                    scratch.mos[idx] = m.get(ox + x as isize, oy + y as isize);
                    scratch.col[idx] = m.color(ox + x as isize, oy + y as isize);
                }
            }
            let cl = |x: isize, y: isize| -> usize {
                y.clamp(0, bh as isize - 1) as usize * bw + x.clamp(0, bw as isize - 1) as usize
            };

            // Pass 1: Initial directional green candidates (H-biased and V-biased)
            for y in 0..bh as isize {
                for x in 0..bw as isize {
                    let i = cl(x, y);
                    let c = scratch.mos[i];
                    if scratch.col[i] == 1 {
                        scratch.gh[i] = c;
                        scratch.gv[i] = c;
                        continue;
                    }
                    let (mut sh, mut wsh) = (0.0f32, 0.0f32);
                    let (mut sv, mut wsv) = (0.0f32, 0.0f32);
                    let (mut g_min, mut g_max) = (f32::INFINITY, f32::NEG_INFINITY);
                    let grad_h = (scratch.mos[cl(x - 1, y)] - scratch.mos[cl(x + 1, y)]).abs();
                    let grad_v = (scratch.mos[cl(x, y - 1)] - scratch.mos[cl(x, y + 1)]).abs();
                    for dy in -2isize..=2 {
                        for dx in -2isize..=2 {
                            if dx == 0 && dy == 0 {
                                continue;
                            }
                            let j = cl(x + dx, y + dy);
                            if scratch.col[j] != 1 {
                                continue;
                            }
                            let gv = scratch.mos[j];
                            if dx.abs() <= 1 && dy.abs() <= 1 {
                                g_min = g_min.min(gv);
                                g_max = g_max.max(gv);
                            }
                            let d2 = (dx * dx + dy * dy) as f32;
                            let base_w = 1.0 / (d2 * d2);
                            let h_bias = if dy == 0 {
                                4.5
                            } else if dx.abs() >= dy.abs() {
                                1.5
                            } else {
                                0.2
                            };
                            let v_bias = if dx == 0 {
                                4.5
                            } else if dy.abs() >= dx.abs() {
                                1.5
                            } else {
                                0.2
                            };
                            let wh = base_w * h_bias / (EPS + grad_h);
                            let wv = base_w * v_bias / (EPS + grad_v);
                            sh += wh * gv;
                            wsh += wh;
                            sv += wv * gv;
                            wsv += wv;
                        }
                    }
                    let est_h = if wsh > 0.0 { sh / wsh } else { c };
                    let est_v = if wsv > 0.0 { sv / wsv } else { c };
                    if g_min <= g_max {
                        scratch.gh[i] = est_h.clamp(g_min, g_max);
                        scratch.gv[i] = est_v.clamp(g_min, g_max);
                    } else {
                        scratch.gh[i] = est_h;
                        scratch.gv[i] = est_v;
                    }
                }
            }

            let complete_rb = |g: &[f32], mos: &[f32], col: &[u8], rgb: &mut [[f32; 3]]| {
                for y in 0..bh as isize {
                    for x in 0..bw as isize {
                        let i = cl(x, y);
                        let own = col[i] as usize;
                        let gp = g[i];
                        let mut p = [gp; 3];
                        for c in [0usize, 2] {
                            if c == own {
                                p[c] = mos[i];
                                continue;
                            }
                            let (mut s, mut ws) = (0.0f32, 0.0f32);
                            for dy in -2isize..=2 {
                                for dx in -2isize..=2 {
                                    let j = cl(x + dx, y + dy);
                                    if col[j] as usize != c {
                                        continue;
                                    }
                                    let gq = g[j];
                                    let d2 = (dx * dx + dy * dy) as f32;
                                    let wt = 1.0 / (d2 * (1e-3 + (gq - gp).abs()));
                                    s += wt * (mos[j] - gq);
                                    ws += wt;
                                }
                            }
                            p[c] = if ws > 0.0 { gp + s / ws } else { gp };
                        }
                        rgb[i] = p;
                    }
                }
            };

            // Passes 2 & 3: Iteratively refine green at R/B sites using local colour-difference consistency
            for _iter in 0..2 {
                complete_rb(&scratch.gh, &scratch.mos, &scratch.col, &mut scratch.rh);
                complete_rb(&scratch.gv, &scratch.mos, &scratch.col, &mut scratch.rv);
                for y in 1..bh as isize - 1 {
                    for x in 1..bw as isize - 1 {
                        let i = cl(x, y);
                        let own = scratch.col[i] as usize;
                        if own == 1 {
                            continue;
                        }
                        let c = scratch.mos[i];
                        // Gather 3×3 green minimum/maximum so iterative refinement never overshoots constant fields
                        let (mut g_min, mut g_max) = (f32::INFINITY, f32::NEG_INFINITY);
                        for dy in -1isize..=1 {
                            for dx in -1isize..=1 {
                                let j = cl(x + dx, y + dy);
                                if scratch.col[j] == 1 {
                                    let gv = scratch.mos[j];
                                    g_min = g_min.min(gv);
                                    g_max = g_max.max(gv);
                                }
                            }
                        }
                        let l = cl(x - 1, y);
                        let r = cl(x + 1, y);
                        let u = cl(x, y - 1);
                        let d = cl(x, y + 1);
                        // H refinement: estimate G = C - avg_H(C - G)
                        let dh = 0.5
                            * ((scratch.rh[l][own] - scratch.rh[l][1])
                                + (scratch.rh[r][own] - scratch.rh[r][1]));
                        // V refinement: estimate G = C - avg_V(C - G)
                        let dv = 0.5
                            * ((scratch.rv[u][own] - scratch.rv[u][1])
                                + (scratch.rv[d][own] - scratch.rv[d][1]));
                        let new_gh = 0.5 * scratch.gh[i] + 0.5 * (c - dh);
                        let new_gv = 0.5 * scratch.gv[i] + 0.5 * (c - dv);
                        if g_min <= g_max {
                            scratch.gh[i] = new_gh.clamp(g_min, g_max);
                            scratch.gv[i] = new_gv.clamp(g_min, g_max);
                        } else {
                            scratch.gh[i] = new_gh;
                            scratch.gv[i] = new_gv;
                        }
                    }
                }
            }
            complete_rb(&scratch.gh, &scratch.mos, &scratch.col, &mut scratch.rh);
            complete_rb(&scratch.gv, &scratch.mos, &scratch.col, &mut scratch.rv);

            // CIELab homogeneity evaluation over a 5×5 window
            for i in 0..n {
                scratch.lh[i] = super::ahd::lab(scratch.rh[i]);
                scratch.lv[i] = super::ahd::lab(scratch.rv[i]);
            }
            let dl = |a: &[f32; 3], b: &[f32; 3]| (a[0] - b[0]).abs();
            let dc = |a: &[f32; 3], b: &[f32; 3]| (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2);
            for y in 2..bh as isize - 2 {
                for x in 2..bw as isize - 2 {
                    let i = cl(x, y);
                    let (l, r, u, d) = (cl(x - 1, y), cl(x + 1, y), cl(x, y - 1), cl(x, y + 1));
                    let eps_l = dl(&scratch.lh[i], &scratch.lh[l])
                        .max(dl(&scratch.lh[i], &scratch.lh[r]))
                        .min(dl(&scratch.lv[i], &scratch.lv[u]).max(dl(&scratch.lv[i], &scratch.lv[d])));
                    let eps_c = dc(&scratch.lh[i], &scratch.lh[l])
                        .max(dc(&scratch.lh[i], &scratch.lh[r]))
                        .min(dc(&scratch.lv[i], &scratch.lv[u]).max(dc(&scratch.lv[i], &scratch.lv[d])));
                    let (mut ch, mut cv) = (0u8, 0u8);
                    for dy in -1isize..=1 {
                        for dx in -1isize..=1 {
                            if dx == 0 && dy == 0 {
                                continue;
                            }
                            let j = cl(x + dx, y + dy);
                            if dl(&scratch.lh[i], &scratch.lh[j]) <= eps_l
                                && dc(&scratch.lh[i], &scratch.lh[j]) <= eps_c
                            {
                                ch += 1;
                            }
                            if dl(&scratch.lv[i], &scratch.lv[j]) <= eps_l
                                && dc(&scratch.lv[i], &scratch.lv[j]) <= eps_c
                            {
                                cv += 1;
                            }
                        }
                    }
                    scratch.hh[i] = ch;
                    scratch.hv[i] = cv;
                }
            }

            // Select homogeneous candidate over 5×5 window
            for y in 0..th {
                let by = y + MARGIN;
                for x in 0..tw {
                    let bx = x + MARGIN;
                    let (mut sh, mut sv) = (0u32, 0u32);
                    for dy in -2isize..=2 {
                        for dx in -2isize..=2 {
                            let j = cl(bx as isize + dx, by as isize + dy);
                            sh += u32::from(scratch.hh[j]);
                            sv += u32::from(scratch.hv[j]);
                        }
                    }
                    let i = by * bw + bx;
                    out[y * tw + x] = if sh > sv {
                        scratch.rh[i]
                    } else if sv > sh {
                        scratch.rv[i]
                    } else {
                        [
                            scratch.rh[i][0].midpoint(scratch.rv[i][0]),
                            scratch.rh[i][1].midpoint(scratch.rv[i][1]),
                            scratch.rh[i][2].midpoint(scratch.rv[i][2]),
                        ]
                    };
                }
            }
        },
    )
}

/// Contrast-guided dual X-Trans demosaicing: blends 3-pass Markesteijn on high-contrast
/// edges with smooth directional colour-difference interpolation on flat/low-contrast areas.
pub(super) fn xtrans_dual(
    m: &Mosaic,
    contrast_threshold: f32,
    cancelled: &impl Fn() -> bool,
) -> Result<Rgb32f, DevelopError> {
    let sharp = xtrans_markesteijn_3pass(m, cancelled)?;
    let smooth = super::xtrans::directional(m, cancelled)?;
    dual_demosaic(m, sharp, &smooth, contrast_threshold, cancelled)
}

/// Check whether a CFA is a $(2k \times 2k)$ Clustered Bayer array composed of four $(k \times k)$
/// homogeneous sub-clusters containing 1 Red cluster, 2 Green clusters, and 1 Blue cluster:
/// - $k = 2$: 4×4 Quad Bayer (TetraCell)
/// - $k = 3$: 6×6 Nona Bayer (Nonacell)
/// - $k = 4$: 8×8 Hexadeca Bayer (ChameleonCell)
#[must_use]
pub(super) fn is_clustered_bayer(cfa: &Cfa, cluster: usize) -> bool {
    if cluster < 2 {
        return false;
    }
    let period = 2 * cluster;
    if cfa.width != period || cfa.height != period || cfa.cells.len() != period * period {
        return false;
    }
    let mut counts = [0usize; 3];
    for by in [0, cluster] {
        for bx in [0, cluster] {
            let c00 = cfa.color_at(bx, by);
            if c00 > 2 {
                return false;
            }
            for dy in 0..cluster {
                for dx in 0..cluster {
                    if cfa.color_at(bx + dx, by + dy) != c00 {
                        return false;
                    }
                }
            }
            counts[c00 as usize] += 1;
        }
    }
    counts == [1, 2, 1]
}

/// Check whether a CFA is a 4×4 Quad Bayer (TetraCell) layout.
#[must_use]
pub(super) fn is_quad_bayer(cfa: &Cfa) -> bool {
    is_clustered_bayer(cfa, 2)
}

/// Check whether a CFA is a 6×6 Nona Bayer (Nonacell) layout.
#[must_use]
pub(super) fn is_nona_bayer(cfa: &Cfa) -> bool {
    is_clustered_bayer(cfa, 3)
}

/// Check whether a CFA is an 8×8 Hexadeca Bayer (ChameleonCell) layout.
#[must_use]
pub(super) fn is_hexadeca_bayer(cfa: &Cfa) -> bool {
    is_clustered_bayer(cfa, 4)
}

/// Dedicated directional demosaicing for $(2k \times 2k)$ Clustered Bayer sensors:
/// - `cluster == 2`: 4×4 Quad Bayer (TetraCell, 2×2 sub-clusters)
/// - `cluster == 3`: 6×6 Nona Bayer (Nonacell, 3×3 sub-clusters)
/// - `cluster == 4`: 8×8 Hexadeca Bayer (ChameleonCell, 4×4 sub-clusters)
///
/// Exploits the $k \times k$ sub-cell structure to preserve intra-cluster high-frequency luminance
/// detail while interpolating missing colours across adjacent $k \times k$ super-pixels using
/// edge-directed colour differences. Measured photosite values remain bit-for-bit exact.
pub(super) fn clustered_bayer(
    m: &Mosaic,
    cluster: usize,
    cancelled: &impl Fn() -> bool,
) -> Result<Rgb32f, DevelopError> {
    let (w, h) = (m.w, m.h);
    let mut g = vec![0.0f32; w * h];
    if w == 0 || h == 0 {
        return Ok(Rgb32f::new(w, h));
    }
    let radius = (cluster as isize + 1).clamp(3, 6);
    // Pass 1: Reconstruct Green plane using directional green search up to `radius`
    par_rows(&mut g, w, cancelled, |y, row| {
        let yi = y as isize;
        for (x, out_g) in row.iter_mut().enumerate() {
            let xi = x as isize;
            if m.color(xi, yi) == 1 {
                *out_g = m.get(xi, yi);
                continue;
            }
            let c = m.get(xi, yi);
            let (mut sh, mut wsh) = (0.0f32, 0.0f32);
            let (mut sv, mut wsv) = (0.0f32, 0.0f32);
            for d in -radius..=radius {
                if d != 0 && m.color(xi + d, yi) == 1 {
                    let wt = 1.0 / (d.abs() as f32);
                    sh += wt * m.get(xi + d, yi);
                    wsh += wt;
                }
                if d != 0 && m.color(xi, yi + d) == 1 {
                    let wt = 1.0 / (d.abs() as f32);
                    sv += wt * m.get(xi, yi + d);
                    wsv += wt;
                }
            }
            let gh = if wsh > 0.0 { sh / wsh } else { c };
            let gv = if wsv > 0.0 { sv / wsv } else { c };
            let grad_h = (m.get(xi - 1, yi) - m.get(xi + 1, yi)).abs()
                + (m.get(xi - cluster as isize, yi) - m.get(xi + cluster as isize, yi)).abs();
            let grad_v = (m.get(xi, yi - 1) - m.get(xi, yi + 1)).abs()
                + (m.get(xi, yi - cluster as isize) - m.get(xi, yi + cluster as isize)).abs();
            let wh = 1.0 / (EPS + grad_h);
            let wv = 1.0 / (EPS + grad_v);
            *out_g = (wh * gh + wv * gv) / (wh + wv);
        }
    })?;

    // Pass 2: Reconstruct R and B via colour difference (C - G) over a `(2*radius+1)×(2*radius+1)` window
    let mut out = Rgb32f::new(w, h);
    let gg = |x: isize, y: isize| g[reflect(y, h) * w + reflect(x, w)];
    par_rows(&mut out.data, w, cancelled, |y, row| {
        let yi = y as isize;
        for (x, px) in row.iter_mut().enumerate() {
            let xi = x as isize;
            let own = m.color(xi, yi) as usize;
            let gp = gg(xi, yi);
            px[1] = gp;
            for c in [0usize, 2] {
                if c == own {
                    px[c] = m.get(xi, yi);
                    continue;
                }
                let (mut s, mut ws) = (0.0f32, 0.0f32);
                for dy in -radius..=radius {
                    for dx in -radius..=radius {
                        if m.color(xi + dx, yi + dy) as usize != c {
                            continue;
                        }
                        let gq = gg(xi + dx, yi + dy);
                        let d2 = (dx * dx + dy * dy) as f32;
                        let wt = 1.0 / (d2 * (1e-3 + (gq - gp).abs()));
                        s += wt * (m.get(xi + dx, yi + dy) - gq);
                        ws += wt;
                    }
                }
                px[c] = if ws > 0.0 { gp + s / ws } else { gp };
            }
        }
    })?;
    Ok(out)
}

/// Dedicated directional demosaicing for 4×4 Quad Bayer (TetraCell) sensors.
pub(super) fn quad_bayer(m: &Mosaic, cancelled: &impl Fn() -> bool) -> Result<Rgb32f, DevelopError> {
    clustered_bayer(m, 2, cancelled)
}

/// Dedicated directional demosaicing for 6×6 Nona Bayer (Nonacell) sensors.
pub(super) fn nona_bayer(m: &Mosaic, cancelled: &impl Fn() -> bool) -> Result<Rgb32f, DevelopError> {
    clustered_bayer(m, 3, cancelled)
}

/// Dedicated directional demosaicing for 8×8 Hexadeca Bayer (ChameleonCell) sensors.
pub(super) fn hexadeca_bayer(m: &Mosaic, cancelled: &impl Fn() -> bool) -> Result<Rgb32f, DevelopError> {
    clustered_bayer(m, 4, cancelled)
}

/// Convert a general CFA cell code (`CfaColor as u8`) into an RGB weight vector or channel index.
/// For standard RGB/RGBW/CYGM patterns:
/// - `0` (Red), `1` (Green), `2` (Blue) map directly to channels `0, 1, 2`.
/// - `3` (Cyan ≈ G+B), `4` (Magenta ≈ R+B), `5` (Yellow ≈ R+G), `6` (White ≈ R+G+B), `7` (Emerald ≈ G).
#[inline]
fn decode_complementary_to_rgb(planes: &[f32; 8]) -> [f32; 3] {
    // CYGM sensors have Cyan (3), Magenta (4), Yellow (5), and Green (1) without primary Red (0) or Blue (2).
    let has_rb = planes[0] > 0.0 || planes[2] > 0.0;
    let has_cygm = planes[3] > 0.0 || planes[4] > 0.0 || planes[5] > 0.0;
    if has_cygm && !has_rb {
        // Pure CYGM: C = G + B, Y = R + G, G = G, M = R + B
        let c = planes[3];
        let m = planes[4];
        let y = planes[5];
        let g = if planes[1] > 0.0 {
            planes[1]
        } else {
            (c + y - m).max(0.0) * 0.5
        };
        let r = (y + m - c).max(0.0) * 0.5;
        let b = (c + m - y).max(0.0) * 0.5;
        [r, g, b]
    } else if planes[6] > 0.0 && has_rb {
        // RGBW sensor: blend panchromatic White (6) luminance detail into interpolated RGB
        let rgb_lum = (planes[0] + planes[1] + planes[2]) / 3.0;
        let scale = if rgb_lum > EPS {
            0.5 + 0.5 * (planes[6] / (rgb_lum * 3.0)).clamp(0.5, 2.0)
        } else {
            1.0
        };
        [planes[0] * scale, planes[1] * scale, planes[2] * scale]
    } else {
        [planes[0], planes[1], planes[2]]
    }
}

/// Universal edge-aware demosaicing for any periodic CFA pattern (`width × height`),
/// including arbitrary RGB lattices, RGBW (panchromatic White sites), and CYGM complementary arrays.
pub(super) fn generic_cfa(m: &Mosaic, cancelled: &impl Fn() -> bool) -> Result<Rgb32f, DevelopError> {
    // If all cells are RGB (0, 1, 2) and Green (1) is present in every 5×5 window, use the
    // high-precision X-Trans directional colour-difference solver.
    let only_rgb = m.cfa.cells.iter().all(|&c| c <= 2);
    let has_all_rgb = [0u8, 1, 2].into_iter().all(|c| m.cfa.cells.contains(&c));
    if only_rgb && has_all_rgb && m.cfa.width <= 6 && m.cfa.height <= 6 {
        return super::xtrans::directional(m, cancelled);
    }

    let (w, h) = (m.w, m.h);
    let mut out = Rgb32f::new(w, h);
    if w == 0 || h == 0 {
        return Ok(out);
    }
    let radius = (m.cfa.width.max(m.cfa.height) as isize).clamp(2, 6);
    par_rows(&mut out.data, w, cancelled, |y, row| {
        let yi = y as isize;
        for (x, px) in row.iter_mut().enumerate() {
            let xi = x as isize;
            let center_val = m.get(xi, yi);
            let center_col = m.color(xi, yi) as usize;
            let mut sums = [0.0f32; 8];
            let mut weights = [0.0f32; 8];
            if center_col < 8 {
                sums[center_col] = center_val;
                weights[center_col] = 1.0;
            }
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let c = m.color(xi + dx, yi + dy) as usize;
                    if c >= 8 || c == center_col {
                        continue;
                    }
                    let v = m.get(xi + dx, yi + dy);
                    let d2 = (dx * dx + dy * dy) as f32;
                    let wt = 1.0 / (d2 * (1e-3 + (v - center_val).abs()));
                    sums[c] += wt * v;
                    weights[c] += wt;
                }
            }
            let mut planes = [0.0f32; 8];
            for c in 0..8 {
                if weights[c] > 0.0 {
                    planes[c] = sums[c] / weights[c];
                }
            }
            let mut rgb = decode_complementary_to_rgb(&planes);
            if center_col < 3 {
                rgb[center_col] = center_val;
            }
            *px = rgb;
        }
    })?;
    Ok(out)
}

/// Pre-demosaic Green Equilibration on a 2×2 Bayer CFA mosaic.
///
/// Equalizes G1/G2 crosstalk imbalance between even-row and odd-row green photosites while
/// preserving real local edges. `strength` in `0.0..=1.0` controls the maximum relative
/// threshold below which G1/G2 differences are treated as sensor crosstalk rather than image detail.
pub(super) fn equilibrate_bayer_greens(
    samples: &mut [f32],
    w: usize,
    h: usize,
    cfa: &Cfa,
    strength: f32,
    cancelled: &impl Fn() -> bool,
) -> Result<(), DevelopError> {
    let strength = strength.clamp(0.0, 1.0);
    if strength <= 0.0 || w < 5 || h < 5 {
        return Ok(());
    }
    let src = samples.to_vec();
    let get = |x: isize, y: isize| src[reflect(y, h) * w + reflect(x, w)];
    par_rows(samples, w, cancelled, |y, row| {
        let yi = y as isize;
        for (x, out) in row.iter_mut().enumerate() {
            if cfa.color_at(x, y) != 1 {
                continue;
            }
            let xi = x as isize;
            let g0 = get(xi, yi);
            // 4 diagonal opposite-phase green neighbours vs 4 same-phase green neighbours at distance 2
            let opp_avg = 0.25
                * (get(xi - 1, yi - 1)
                    + get(xi + 1, yi - 1)
                    + get(xi - 1, yi + 1)
                    + get(xi + 1, yi + 1));
            let same_avg =
                0.25 * (get(xi - 2, yi) + get(xi + 2, yi) + get(xi, yi - 2) + get(xi, yi + 2));
            let diff = opp_avg - 0.5 * (g0 + same_avg);
            let local_grad = (get(xi - 1, yi - 1) - get(xi + 1, yi + 1)).abs()
                + (get(xi + 1, yi - 1) - get(xi - 1, yi + 1)).abs();
            let limit = strength * 0.25 * g0.max(opp_avg).max(1e-4);
            if diff.abs() < limit && local_grad < limit * 2.0 {
                *out = (g0 + 0.5 * diff).max(0.0);
            }
        }
    })
}

/// Pre-demosaic CFA Impulse / Hot-Pixel Denoise.
///
/// Detects isolated salt-and-pepper spikes on same-colour CFA neighbours within a 5×5 window
/// and replaces outliers exceeding `threshold` times the local same-colour interquartile range.
pub(super) fn suppress_cfa_impulse_noise(
    samples: &mut [f32],
    w: usize,
    h: usize,
    cfa: &Cfa,
    threshold: f32,
    cancelled: &impl Fn() -> bool,
) -> Result<(), DevelopError> {
    if threshold <= 0.0 || w < 5 || h < 5 {
        return Ok(());
    }
    let src = samples.to_vec();
    let get = |x: isize, y: isize| src[reflect(y, h) * w + reflect(x, w)];
    par_rows(samples, w, cancelled, |y, row| {
        let yi = y as isize;
        let mut ring = [0.0f32; 24];
        for (x, out) in row.iter_mut().enumerate() {
            let xi = x as isize;
            let own = cfa.color_at(x, y);
            let center = get(xi, yi);
            let mut count = 0usize;
            for dy in -2isize..=2 {
                for dx in -2isize..=2 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let sx = reflect(xi + dx, w);
                    let sy = reflect(yi + dy, h);
                    if cfa.color_at(sx, sy) == own {
                        ring[count] = src[sy * w + sx];
                        count += 1;
                    }
                }
            }
            if count < 4 {
                continue;
            }
            let slice = &mut ring[..count];
            slice.sort_unstable_by(|a, b| a.total_cmp(b));
            let med = slice[count / 2];
            let q1 = slice[count / 4];
            let q3 = slice[(count * 3) / 4];
            let spread = (q3 - q1).max(med.abs() * 0.02 + 1e-4);
            if (center - med).abs() > threshold * spread {
                *out = med;
            }
        }
    })
}

/// Post-demosaic edge-preserving median colour-difference filter for false-colour and zipper suppression.
///
/// Reference: Lu & Tan (2003) / Freeman (1988) median filtering of `(R - G)` and `(B - G)` colour
/// differences. Crucially, the measured CFA channel at each pixel `(x, y)` is pinned to its exact
/// sensor value `m.data[y * w + x]` on every pass so sharpness and sensor fidelity are never degraded.
pub(super) fn suppress_false_color(
    rgb: &mut Rgb32f,
    m: &Mosaic,
    passes: u8,
    cancelled: &impl Fn() -> bool,
) -> Result<(), DevelopError> {
    let (w, h) = (rgb.width, rgb.height);
    if passes == 0 || w == 0 || h == 0 {
        return Ok(());
    }
    let passes = passes.min(4);
    let mut prev = vec![[0.0f32; 3]; w * h];
    for _ in 0..passes {
        if cancelled() {
            return Err(DevelopError::Cancelled);
        }
        prev.copy_from_slice(&rgb.data);
        let src = &prev;
        par_rows(&mut rgb.data, w, cancelled, |y, row| {
            let yi = y as isize;
            let mut dr = [0.0f32; 9];
            let mut db = [0.0f32; 9];
            for (x, px) in row.iter_mut().enumerate() {
                let xi = x as isize;
                let mut k = 0;
                for dy in -1isize..=1 {
                    let sy = reflect(yi + dy, h);
                    for dx in -1isize..=1 {
                        let sx = reflect(xi + dx, w);
                        let p = src[sy * w + sx];
                        dr[k] = p[0] - p[1];
                        db[k] = p[2] - p[1];
                        k += 1;
                    }
                }
                dr.sort_unstable_by(|a, b| a.total_cmp(b));
                db.sort_unstable_by(|a, b| a.total_cmp(b));
                let med_rg = dr[4];
                let med_bg = db[4];

                let own = m.cfa.color_at(x, y) as usize;
                let exact = m.data[y * w + x];
                match own {
                    0 => {
                        let g = (exact - med_rg).max(0.0);
                        let b = (g + med_bg).max(0.0);
                        *px = [exact, g, b];
                    }
                    2 => {
                        let g = (exact - med_bg).max(0.0);
                        let r = (g + med_rg).max(0.0);
                        *px = [r, g, exact];
                    }
                    1 => {
                        let r = (exact + med_rg).max(0.0);
                        let b = (exact + med_bg).max(0.0);
                        *px = [r, exact, b];
                    }
                    _ => {
                        let g = px[1];
                        *px = [(g + med_rg).max(0.0), g, (g + med_bg).max(0.0)];
                    }
                }
            }
        })?;
    }
    Ok(())
}

/// Post-demosaic Chromatic Aberration / High-Contrast Fringe Suppression.
///
/// Attenuates narrow purple/magenta/cyan colour-difference spikes along steep luminance edges
/// by clamping interpolated `(R - G)` and `(B - G)` differences toward the low-gradient
/// neighbourhood average while keeping the measured CFA channel bit-for-bit exact.
pub(super) fn suppress_chromatic_fringe(
    rgb: &mut Rgb32f,
    m: &Mosaic,
    cancelled: &impl Fn() -> bool,
) -> Result<(), DevelopError> {
    let (w, h) = (rgb.width, rgb.height);
    if w < 5 || h < 5 {
        return Ok(());
    }
    let src = rgb.data.clone();
    let get = |x: isize, y: isize| src[reflect(y, h) * w + reflect(x, w)];
    par_rows(&mut rgb.data, w, cancelled, |y, row| {
        let yi = y as isize;
        for (x, px) in row.iter_mut().enumerate() {
            let xi = x as isize;
            let p0 = get(xi, yi);
            let lum_grad = (get(xi - 1, yi)[1] - get(xi + 1, yi)[1]).abs()
                + (get(xi, yi - 1)[1] - get(xi, yi + 1)[1]).abs();
            // Only act on steep luminance edges where axial/lateral fringes appear
            if lum_grad < 0.05 * p0[1].max(0.05) {
                continue;
            }
            let (mut sum_rg, mut sum_bg, mut wsum) = (0.0f32, 0.0f32, 0.0f32);
            for dy in -2isize..=2 {
                for dx in -2isize..=2 {
                    let q = get(xi + dx, yi + dy);
                    let q_grad = (get(xi + dx - 1, yi + dy)[1] - get(xi + dx + 1, yi + dy)[1]).abs()
                        + (get(xi + dx, yi + dy - 1)[1] - get(xi + dx, yi + dy + 1)[1]).abs();
                    // Prefer neighbours in flatter zones adjacent to the edge
                    let wt = 1.0 / (1e-3 + q_grad);
                    sum_rg += wt * (q[0] - q[1]);
                    sum_bg += wt * (q[2] - q[1]);
                    wsum += wt;
                }
            }
            if wsum <= 0.0 {
                continue;
            }
            let avg_rg = sum_rg / wsum;
            let avg_bg = sum_bg / wsum;
            let cur_rg = p0[0] - p0[1];
            let cur_bg = p0[2] - p0[1];
            let new_rg = 0.5 * cur_rg + 0.5 * avg_rg;
            let new_bg = 0.5 * cur_bg + 0.5 * avg_bg;

            let own = m.cfa.color_at(x, y) as usize;
            let exact = m.data[y * w + x];
            match own {
                0 => {
                    let g = (exact - new_rg).max(0.0);
                    let b = (g + new_bg).max(0.0);
                    *px = [exact, g, b];
                }
                2 => {
                    let g = (exact - new_bg).max(0.0);
                    let r = (g + new_rg).max(0.0);
                    *px = [r, g, exact];
                }
                1 => {
                    let r = (exact + new_rg).max(0.0);
                    let b = (exact + new_bg).max(0.0);
                    *px = [r, exact, b];
                }
                _ => {
                    let g = p0[1];
                    *px = [(g + new_rg).max(0.0), g, (g + new_bg).max(0.0)];
                }
            }
        }
    })
}

/// Configuration for the standalone public demosaicing API (`demosaic_f32` / `demosaic_u16`).
#[derive(Debug, Clone, PartialEq)]
pub struct DemosaicConfig {
    /// Demosaicing algorithm to execute. Defaults to [`DemosaicAlgorithm::Auto`].
    pub algorithm: DemosaicAlgorithm,
    /// Contrast threshold (`0.0..=1.0`) for dual demosaicing algorithms (`AmazeVng`, `AmazeLmmse`,
    /// `RcdVng`, `RcdLmmse`, `DcbVng`, `XTransDual`). Higher values route more pixels to the
    /// low-noise smooth solver. Defaults to `0.04`.
    pub dual_contrast_threshold: f32,
    /// Post-demosaic median colour-difference refinement passes (`0..=4`) to suppress
    /// false colour and zippering while preserving exact measured CFA samples. Defaults to `0`.
    pub false_color_passes: u8,
    /// Pre-demosaic Bayer G1/G2 green equilibration strength (`0.0..=1.0`). Defaults to `0.0`.
    pub green_equilibration: f32,
    /// Pre-demosaic CFA impulse / hot-pixel outlier threshold (`0.0` disables). Defaults to `0.0`.
    pub impulse_threshold: f32,
    /// Post-demosaic high-contrast chromatic fringe suppression. Defaults to `false`.
    pub suppress_fringe: bool,
}

impl Default for DemosaicConfig {
    fn default() -> Self {
        Self {
            algorithm: DemosaicAlgorithm::Auto,
            dual_contrast_threshold: 0.04,
            false_color_passes: 0,
            green_equilibration: 0.0,
            impulse_threshold: 0.0,
            suppress_fringe: false,
        }
    }
}

/// Dispatch any [`DemosaicAlgorithm`] on a prepared internal [`Mosaic`].
pub(super) fn dispatch_demosaic(
    source: &Mosaic,
    algo: DemosaicAlgorithm,
    dual_contrast_threshold: f32,
    is_bayer_2x2: bool,
    cancelled: &impl Fn() -> bool,
) -> Result<Rgb32f, DevelopError> {
    match algo {
        DemosaicAlgorithm::Auto => {
            if is_bayer_2x2 {
                super::ahd::ahd(source, cancelled)
            } else if is_quad_bayer(source.cfa) {
                quad_bayer(source, cancelled)
            } else if is_nona_bayer(source.cfa) {
                nona_bayer(source, cancelled)
            } else if is_hexadeca_bayer(source.cfa) {
                hexadeca_bayer(source, cancelled)
            } else {
                super::xtrans::directional(source, cancelled)
            }
        }
        DemosaicAlgorithm::Nearest => nearest(source, cancelled),
        DemosaicAlgorithm::Bilinear => bilinear(source, cancelled),
        DemosaicAlgorithm::SmoothHue => smooth_hue(source, cancelled),
        DemosaicAlgorithm::LarochePrescott => laroche_prescott(source, cancelled),
        DemosaicAlgorithm::HamiltonAdams => hamilton_adams(source, cancelled),
        DemosaicAlgorithm::MalvarHeCutler => malvar_he_cutler(source, cancelled),
        DemosaicAlgorithm::Ppg => ppg(source, cancelled),
        DemosaicAlgorithm::Vng => vng(source, cancelled),
        DemosaicAlgorithm::Rcd => rcd(source, cancelled),
        DemosaicAlgorithm::Ahd => super::ahd::ahd(source, cancelled),
        DemosaicAlgorithm::Lmmse => lmmse(source, cancelled),
        DemosaicAlgorithm::Dcb => dcb(source, cancelled),
        DemosaicAlgorithm::Igv => igv(source, cancelled),
        DemosaicAlgorithm::Amaze => amaze(source, cancelled),
        DemosaicAlgorithm::AmazeVng => {
            let sharp = amaze(source, cancelled)?;
            let smooth = vng(source, cancelled)?;
            dual_demosaic(source, sharp, &smooth, dual_contrast_threshold, cancelled)
        }
        DemosaicAlgorithm::AmazeLmmse => {
            let sharp = amaze(source, cancelled)?;
            let smooth = lmmse(source, cancelled)?;
            dual_demosaic(source, sharp, &smooth, dual_contrast_threshold, cancelled)
        }
        DemosaicAlgorithm::RcdVng => {
            let sharp = rcd(source, cancelled)?;
            let smooth = vng(source, cancelled)?;
            dual_demosaic(source, sharp, &smooth, dual_contrast_threshold, cancelled)
        }
        DemosaicAlgorithm::RcdLmmse => {
            let sharp = rcd(source, cancelled)?;
            let smooth = lmmse(source, cancelled)?;
            dual_demosaic(source, sharp, &smooth, dual_contrast_threshold, cancelled)
        }
        DemosaicAlgorithm::DcbVng => {
            let sharp = dcb(source, cancelled)?;
            let smooth = vng(source, cancelled)?;
            dual_demosaic(source, sharp, &smooth, dual_contrast_threshold, cancelled)
        }
        DemosaicAlgorithm::XTransDirectional => super::xtrans::directional(source, cancelled),
        DemosaicAlgorithm::XTransMarkesteijn => xtrans_markesteijn(source, cancelled),
        DemosaicAlgorithm::XTransMarkesteijn3Pass => xtrans_markesteijn_3pass(source, cancelled),
        DemosaicAlgorithm::XTransDual => xtrans_dual(source, dual_contrast_threshold, cancelled),
        DemosaicAlgorithm::QuadBayer => quad_bayer(source, cancelled),
        DemosaicAlgorithm::NonaBayer => nona_bayer(source, cancelled),
        DemosaicAlgorithm::HexadecaBayer => hexadeca_bayer(source, cancelled),
        DemosaicAlgorithm::GenericCfa => generic_cfa(source, cancelled),
    }
}

/// Standalone scene-linear `f32` demosaicing entry point.
///
/// Reconstructs full-resolution 3-channel RGB (`[f32; 3]` per pixel, row-major) directly from
/// a single-channel CFA buffer `samples` without requiring camera metadata or DNG opcodes.
/// All memory allocations are pre-admitted against `budget` when provided.
pub fn demosaic_f32(
    samples: &[f32],
    width: u32,
    height: u32,
    cfa_pattern: & crate::CfaPattern,
    config: &DemosaicConfig,
    budget: Option<&crate::MemoryBudget>,
    cancelled: &impl Fn() -> bool,
) -> Result<rrrah_memory::PixelBuffer<[f32; 3]>, DevelopError> {
    if cancelled() {
        return Err(DevelopError::Cancelled);
    }
    if config.false_color_passes > 4 {
        return Err(DevelopError::Invalid(
            "false_color_passes must be between 0 and 4",
        ));
    }
    let cfa = super::quality_cfa(cfa_pattern, config.algorithm)?;
    let (w, h) = (width as usize, height as usize);
    let n = w
        .checked_mul(h)
        .ok_or(DevelopError::Invalid("dimension overflow"))?;
    if samples.len() != n {
        return Err(DevelopError::Invalid("sensor sample count mismatch"));
    }
    if samples.iter().any(|v| !v.is_finite()) {
        return Err(DevelopError::Invalid("non-finite sensor samples"));
    }
    let out_bytes = (n as u64)
        .checked_mul(12)
        .ok_or(DevelopError::Invalid("output size overflow"))?;
    let scratch_bytes = (n as u64)
        .checked_mul(16)
        .and_then(|v| v.checked_add(8 * 1024 * 1024))
        .ok_or(DevelopError::Invalid("scratch size overflow"))?;
    let managed = budget
        .cloned()
        .unwrap_or_else(|| crate::MemoryBudget::new(out_bytes + scratch_bytes));
    let scratch_credit = managed.try_reserve(scratch_bytes)?;
    let out_credit = managed.try_reserve(out_bytes)?;

    let is_bayer = cfa_pattern.bayer_quad().is_ok();
    let mut conditioned: Option<Vec<f32>> = None;
    if config.impulse_threshold > 0.0 || (is_bayer && config.green_equilibration > 0.0) {
        let mut buf = samples.to_vec();
        if config.impulse_threshold > 0.0 {
            suppress_cfa_impulse_noise(&mut buf, w, h, &cfa, config.impulse_threshold, cancelled)?;
        }
        if is_bayer && config.green_equilibration > 0.0 {
            equilibrate_bayer_greens(&mut buf, w, h, &cfa, config.green_equilibration, cancelled)?;
        }
        conditioned = Some(buf);
    }
    let active_samples = conditioned.as_deref().unwrap_or(samples);
    let source = Mosaic {
        w,
        h,
        data: active_samples,
        cfa: &cfa,
    };
    let mut rgb = dispatch_demosaic(
        &source,
        config.algorithm,
        config.dual_contrast_threshold,
        is_bayer,
        cancelled,
    )?;
    if config.false_color_passes > 0 {
        suppress_false_color(&mut rgb, &source, config.false_color_passes, cancelled)?;
    }
    if config.suppress_fringe {
        suppress_chromatic_fringe(&mut rgb, &source, cancelled)?;
    }
    drop(conditioned);
    drop(scratch_credit);
    Ok(out_credit.try_adopt(rgb.data)?.into())
}

/// Standalone `u16` demosaicing entry point.
///
/// Normalizes integer sensor samples by `(sample - black) / (white - black)` and runs
/// [`demosaic_f32`] with full memory-budget accounting.
pub fn demosaic_u16(
    samples: &[u16],
    width: u32,
    height: u32,
    black_level: f32,
    white_level: f32,
    cfa_pattern: &crate::CfaPattern,
    config: &DemosaicConfig,
    budget: Option<&crate::MemoryBudget>,
    cancelled: &impl Fn() -> bool,
) -> Result<rrrah_memory::PixelBuffer<[f32; 3]>, DevelopError> {
    let range = white_level - black_level;
    if !range.is_finite() || range <= 0.0 {
        return Err(DevelopError::Invalid(
            "white-minus-black range must be finite and positive",
        ));
    }
    let inv = 1.0 / range;
    let normalized: Vec<f32> = samples
        .iter()
        .map(|&s| ((f32::from(s) - black_level).max(0.0)) * inv)
        .collect();
    demosaic_f32(
        &normalized,
        width,
        height,
        cfa_pattern,
        config,
        budget,
        cancelled,
    )
}

/// Exact half-resolution 2×2 Bayer superpixel extractor (no spatial interpolation).
///
/// Collapses each 2×2 Bayer quad into one RGB pixel `(R, 0.5 * (G1 + G2), B)` of dimensions
/// `(width / 2, height / 2)`, eliminating all demosaicing artifacts and moiré for fast previews
/// or half-resolution workflows.
pub fn demosaic_bayer_superpixel(
    samples: &[f32],
    width: u32,
    height: u32,
    cfa_pattern: &crate::CfaPattern,
    budget: Option<&crate::MemoryBudget>,
    cancelled: &impl Fn() -> bool,
) -> Result<(u32, u32, rrrah_memory::PixelBuffer<[f32; 3]>), DevelopError> {
    if cancelled() {
        return Err(DevelopError::Cancelled);
    }
    let quad = cfa_pattern
        .bayer_quad()
        .map_err(|_| DevelopError::Invalid("superpixel demosaic requires a 2x2 RGB Bayer CFA"))?;
    let (w, h) = (width as usize, height as usize);
    if samples.len() != w.checked_mul(h).ok_or(DevelopError::Invalid("dimension overflow"))? {
        return Err(DevelopError::Invalid("sensor sample count mismatch"));
    }
    let (ow, oh) = (w / 2, h / 2);
    if ow == 0 || oh == 0 {
        return Err(DevelopError::Invalid(
            "superpixel demosaic requires dimensions >= 2x2",
        ));
    }
    let out_bytes = (ow * oh) as u64 * 12;
    let managed = budget
        .cloned()
        .unwrap_or_else(|| crate::MemoryBudget::new(out_bytes));
    let out_credit = managed.try_reserve(out_bytes)?;
    let mut out = vec![[0.0f32; 3]; ow * oh];
    par_rows(&mut out, ow, cancelled, |oy, row| {
        let sy = oy * 2;
        for (ox, px) in row.iter_mut().enumerate() {
            let sx = ox * 2;
            let mut sums = [0.0f32; 3];
            let (mut s00, mut s10, mut s01, mut s11) = (
                samples[sy * w + sx],
                samples[sy * w + sx + 1],
                samples[(sy + 1) * w + sx],
                samples[(sy + 1) * w + sx + 1],
            );
            for (val, c) in [
                (&mut s00, quad[0]),
                (&mut s10, quad[1]),
                (&mut s01, quad[2]),
                (&mut s11, quad[3]),
            ] {
                sums[c as usize] += *val;
            }
            *px = [sums[0], sums[1] * 0.5, sums[2]];
        }
    })?;
    Ok((ow as u32, oh as u32, out_credit.try_adopt(out)?.into()))
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CfaColor, CfaPattern, DecodedMosaic, LevelGrid, MemoryBudget, Orientation, Photometric,
        RasterPixels, RawMetadata, WhiteLevel,
        develop::{DevelopOptions, OpcodeLists, develop_raw},
    };

    fn all_bayer_cfas() -> [Cfa; 4] {
        [
            Cfa {
                width: 2,
                height: 2,
                cells: vec![0, 1, 1, 2], // RGGB
            },
            Cfa {
                width: 2,
                height: 2,
                cells: vec![2, 1, 1, 0], // BGGR
            },
            Cfa {
                width: 2,
                height: 2,
                cells: vec![1, 0, 2, 1], // GRBG
            },
            Cfa {
                width: 2,
                height: 2,
                cells: vec![1, 2, 0, 1], // GBRG
            },
        ]
    }

    fn xtrans_cfa() -> Cfa {
        Cfa {
            width: 6,
            height: 6,
            cells: vec![
                1, 2, 1, 1, 0, 1, 0, 1, 0, 2, 1, 2, 1, 2, 1, 1, 0, 1, 1, 0, 1, 1, 2, 1, 2, 1, 2, 0, 1, 0, 1,
                0, 1, 1, 2, 1,
            ],
        }
    }

    fn quad_bayer_cfa() -> Cfa {
        Cfa {
            width: 4,
            height: 4,
            cells: vec![
                0, 0, 1, 1, // R R G G
                0, 0, 1, 1, // R R G G
                1, 1, 2, 2, // G G B B
                1, 1, 2, 2, // G G B B
            ],
        }
    }

    fn nona_bayer_cfa() -> Cfa {
        let mut cells = Vec::with_capacity(36);
        for y in 0..6 {
            for x in 0..6 {
                let c = match (x >= 3, y >= 3) {
                    (false, false) => 0,
                    (true, false) | (false, true) => 1,
                    (true, true) => 2,
                };
                cells.push(c);
            }
        }
        Cfa {
            width: 6,
            height: 6,
            cells,
        }
    }

    fn hexadeca_bayer_cfa() -> Cfa {
        let mut cells = Vec::with_capacity(64);
        for y in 0..8 {
            for x in 0..8 {
                let c = match (x >= 4, y >= 4) {
                    (false, false) => 0,
                    (true, false) | (false, true) => 1,
                    (true, true) => 2,
                };
                cells.push(c);
            }
        }
        Cfa {
            width: 8,
            height: 8,
            cells,
        }
    }

    #[test]
    fn all_bayer_algorithms_preserve_constant_hdr_color_and_exact_sites_across_phases() {
        // Cross 128x128 tile boundaries and odd dimensions.
        let (w, h) = (139, 133);
        let color = [1.85_f32, 0.42, 3.15]; // Scene-linear HDR > 1.0
        for cfa in all_bayer_cfas() {
            let data: Vec<f32> = (0..w * h)
                .map(|i| color[cfa.color_at(i % w, i / w) as usize])
                .collect();
            let m = Mosaic {
                w,
                h,
                data: &data,
                cfa: &cfa,
            };
            for algo in [
                DemosaicAlgorithm::Nearest,
                DemosaicAlgorithm::Bilinear,
                DemosaicAlgorithm::SmoothHue,
                DemosaicAlgorithm::LarochePrescott,
                DemosaicAlgorithm::HamiltonAdams,
                DemosaicAlgorithm::MalvarHeCutler,
                DemosaicAlgorithm::Ppg,
                DemosaicAlgorithm::Vng,
                DemosaicAlgorithm::Rcd,
                DemosaicAlgorithm::Ahd,
                DemosaicAlgorithm::Lmmse,
                DemosaicAlgorithm::Dcb,
                DemosaicAlgorithm::Igv,
                DemosaicAlgorithm::Amaze,
                DemosaicAlgorithm::AmazeVng,
                DemosaicAlgorithm::AmazeLmmse,
                DemosaicAlgorithm::RcdVng,
                DemosaicAlgorithm::RcdLmmse,
                DemosaicAlgorithm::DcbVng,
                DemosaicAlgorithm::GenericCfa,
            ] {
                let mut out = dispatch_demosaic(&m, algo, 0.04, true, &|| false).unwrap();
                for (i, p) in out.data.iter().enumerate() {
                    for c in 0..3 {
                        assert!(
                            (p[c] - color[c]).abs() < 5e-5,
                            "{algo:?} pixel {i} ch {c}: got {p:?}, want {color:?}"
                        );
                    }
                    let own = cfa.color_at(i % w, i / w) as usize;
                    assert_eq!(
                        p[own].to_bits(),
                        data[i].to_bits(),
                        "{algo:?} modified measured site at {i}"
                    );
                }
                suppress_false_color(&mut out, &m, 2, &|| false).unwrap();
                suppress_chromatic_fringe(&mut out, &m, &|| false).unwrap();
                for (i, p) in out.data.iter().enumerate() {
                    for c in 0..3 {
                        assert!(
                            (p[c] - color[c]).abs() < 5e-5,
                            "{algo:?}+post pixel {i} ch {c}: got {p:?}, want {color:?}"
                        );
                    }
                    let own = cfa.color_at(i % w, i / w) as usize;
                    assert_eq!(p[own].to_bits(), data[i].to_bits());
                }
            }
        }
    }

    #[test]
    fn xtrans_and_clustered_bayer_families_preserve_constant_hdr_and_measured_sites() {
        let (w, h) = (140, 134);
        let color = [2.4_f32, 0.75, 1.35];
        for (name, cfa, algo) in [
            ("xtrans_dir", xtrans_cfa(), DemosaicAlgorithm::XTransDirectional),
            ("xtrans_1pass", xtrans_cfa(), DemosaicAlgorithm::XTransMarkesteijn),
            (
                "xtrans_3pass",
                xtrans_cfa(),
                DemosaicAlgorithm::XTransMarkesteijn3Pass,
            ),
            ("xtrans_dual", xtrans_cfa(), DemosaicAlgorithm::XTransDual),
            ("quad_bayer", quad_bayer_cfa(), DemosaicAlgorithm::QuadBayer),
            ("nona_bayer", nona_bayer_cfa(), DemosaicAlgorithm::NonaBayer),
            (
                "hexadeca_bayer",
                hexadeca_bayer_cfa(),
                DemosaicAlgorithm::HexadecaBayer,
            ),
        ] {
            let data: Vec<f32> = (0..w * h)
                .map(|i| color[cfa.color_at(i % w, i / w) as usize])
                .collect();
            let m = Mosaic {
                w,
                h,
                data: &data,
                cfa: &cfa,
            };
            let out = dispatch_demosaic(&m, algo, 0.04, false, &|| false).unwrap();
            for (i, p) in out.data.iter().enumerate() {
                for c in 0..3 {
                    assert!(
                        (p[c] - color[c]).abs() < 1e-5,
                        "{name} pixel {i} ch {c}: got {p:?}, want {color:?}"
                    );
                }
                let own = cfa.color_at(i % w, i / w) as usize;
                assert_eq!(p[own].to_bits(), data[i].to_bits());
            }
        }
    }

    #[test]
    fn cygm_complementary_cfa_reconstructs_primary_rgb_via_generic_solver() {
        let cfa = Cfa {
            width: 2,
            height: 2,
            // Cyan (3) = G+B, Yellow (5) = R+G, Green (1) = G, Magenta (4) = R+B
            cells: vec![3, 5, 1, 4],
        };
        let (r, g, b) = (0.4_f32, 0.6_f32, 0.8_f32);
        let cygm_vals = |code: u8| match code {
            3 => g + b,
            5 => r + g,
            1 => g,
            4 => r + b,
            _ => unreachable!(),
        };
        let (w, h) = (16, 16);
        let data: Vec<f32> = (0..w * h)
            .map(|i| cygm_vals(cfa.color_at(i % w, i / w)))
            .collect();
        let m = Mosaic {
            w,
            h,
            data: &data,
            cfa: &cfa,
        };
        let out = generic_cfa(&m, &|| false).unwrap();
        for p in &out.data {
            assert!((p[0] - r).abs() < 1e-5, "R: {:?}", p);
            assert!((p[1] - g).abs() < 1e-5, "G: {:?}", p);
            assert!((p[2] - b).abs() < 1e-5, "B: {:?}", p);
        }
    }

    #[test]
    fn directional_algorithms_outperform_bilinear_on_sharp_step_edges_and_stripes() {
        // Construct a synthetic vertical high-contrast step + fine alternating horizontal frequency
        // where bilinear produces severe zippering while directional/gradient-corrected methods excel.
        let (w, h) = (64, 64);
        let cfa = &all_bayer_cfas()[0];
        let mut gt = vec![[0.0f32; 3]; w * h];
        let mut data = vec![0.0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                // Vertical edge at x == 32 plus horizontal high-frequency modulation
                let base = if x < 32 { 0.2f32 } else { 0.8f32 };
                let wave = 0.15 * ((x as f32 * 1.3).sin() + (y as f32 * 0.4).cos());
                let lum = (base + wave).clamp(0.05, 0.95);
                let rgb = [lum * 0.9, lum, lum * 0.7];
                gt[y * w + x] = rgb;
                data[y * w + x] = rgb[cfa.color_at(x, y) as usize];
            }
        }
        let m = Mosaic {
            w,
            h,
            data: &data,
            cfa,
        };
        let mse = |img: &Rgb32f| -> f64 {
            let mut sum = 0.0f64;
            let mut count = 0usize;
            for y in 4..h - 4 {
                for x in 4..w - 4 {
                    let a = img.data[y * w + x];
                    let b = gt[y * w + x];
                    for c in 0..3 {
                        let d = f64::from(a[c] - b[c]);
                        sum += d * d;
                        count += 1;
                    }
                }
            }
            sum / count as f64
        };
        let mse_bil = mse(&bilinear(&m, &|| false).unwrap());
        let mse_lp = mse(&laroche_prescott(&m, &|| false).unwrap());
        let mse_ha = mse(&hamilton_adams(&m, &|| false).unwrap());
        let mse_mhc = mse(&malvar_he_cutler(&m, &|| false).unwrap());
        let mse_ppg = mse(&ppg(&m, &|| false).unwrap());
        let mse_rcd = mse(&rcd(&m, &|| false).unwrap());
        let mse_lmmse = mse(&lmmse(&m, &|| false).unwrap());
        let mse_dcb = mse(&dcb(&m, &|| false).unwrap());
        let mse_igv = mse(&igv(&m, &|| false).unwrap());
        let mse_amaze = mse(&amaze(&m, &|| false).unwrap());
        assert!(mse_lp < mse_bil, "LP ({mse_lp}) should beat bilinear ({mse_bil})");
        assert!(mse_ha < mse_bil, "HA ({mse_ha}) should beat bilinear ({mse_bil})");
        assert!(mse_mhc < mse_bil, "MHC ({mse_mhc}) should beat bilinear ({mse_bil})");
        assert!(mse_ppg < mse_bil, "PPG ({mse_ppg}) should beat bilinear ({mse_bil})");
        assert!(mse_rcd < mse_bil, "RCD ({mse_rcd}) should beat bilinear ({mse_bil})");
        assert!(mse_lmmse < mse_bil, "LMMSE ({mse_lmmse}) should beat bilinear ({mse_bil})");
        assert!(mse_dcb < mse_bil, "DCB ({mse_dcb}) should beat bilinear ({mse_bil})");
        assert!(mse_igv < mse_bil, "IGV ({mse_igv}) should beat bilinear ({mse_bil})");
        assert!(mse_amaze < mse_bil, "AMaZE ({mse_amaze}) should beat bilinear ({mse_bil})");
    }

    #[test]
    fn standalone_demosaic_apis_and_conditioning_stages_work_under_memory_budget() {
        let cfa_pat = CfaPattern {
            width: 2,
            height: 2,
            cells: vec![CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue],
        };
        let (w, h) = (32u32, 32u32);
        let color = [0.6_f32, 0.5, 0.4];
        let mut samples: Vec<f32> = (0..(w * h) as usize)
            .map(|i| {
                let x = i % w as usize;
                let y = i / w as usize;
                let c = cfa_pat.cells[(y % 2) * 2 + (x % 2)] as usize;
                color[c]
            })
            .collect();
        // Inject one hot-pixel spike at (10, 10)
        samples[10 * w as usize + 10] = 50.0;

        let budget = MemoryBudget::new(32 * 1024 * 1024);
        let cfg = DemosaicConfig {
            algorithm: DemosaicAlgorithm::AmazeVng,
            dual_contrast_threshold: 0.04,
            false_color_passes: 2,
            green_equilibration: 0.5,
            impulse_threshold: 3.0,
            suppress_fringe: true,
        };
        let rgb = demosaic_f32(&samples, w, h, &cfa_pat, &cfg, Some(&budget), &|| false).unwrap();
        assert_eq!(rgb.len(), (w * h) as usize);
        // The hot pixel at (10, 10) should have been suppressed back to color
        for p in rgb.iter() {
            for c in 0..3 {
                assert!((p[c] - color[c]).abs() < 1e-4, "got {p:?}, want {color:?}");
            }
        }
        drop(rgb);
        assert_eq!(budget.used(), 0);

        // Test u16 entry point and superpixel entry point
        let u16_samples = vec![2000u16; (w * h) as usize];
        let rgb16 = demosaic_u16(
            &u16_samples,
            w,
            h,
            0.0,
            4000.0,
            &cfa_pat,
            &DemosaicConfig {
                algorithm: DemosaicAlgorithm::RcdLmmse,
                ..Default::default()
            },
            Some(&budget),
            &|| false,
        )
        .unwrap();
        assert!(rgb16.iter().all(|p| (p[0] - 0.5).abs() < 1e-5));
        drop(rgb16);
        assert_eq!(budget.used(), 0);

        let (sw, sh, super_rgb) =
            demosaic_bayer_superpixel(&samples[..], w, h, &cfa_pat, Some(&budget), &|| false)
                .unwrap();
        assert_eq!((sw, sh), (w / 2, h / 2));
        assert_eq!(super_rgb.len(), (sw * sh) as usize);
        drop(super_rgb);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn cancellation_and_cfa_mismatch_validation_across_all_algorithms() {
        let (w, h) = (140, 140);
        let bayer = &all_bayer_cfas()[0];
        let data = vec![0.5f32; w * h];
        let m = Mosaic {
            w,
            h,
            data: &data,
            cfa: bayer,
        };
        for algo in [
            DemosaicAlgorithm::Nearest,
            DemosaicAlgorithm::Bilinear,
            DemosaicAlgorithm::SmoothHue,
            DemosaicAlgorithm::LarochePrescott,
            DemosaicAlgorithm::HamiltonAdams,
            DemosaicAlgorithm::MalvarHeCutler,
            DemosaicAlgorithm::Ppg,
            DemosaicAlgorithm::Vng,
            DemosaicAlgorithm::Rcd,
            DemosaicAlgorithm::Ahd,
            DemosaicAlgorithm::Lmmse,
            DemosaicAlgorithm::Dcb,
            DemosaicAlgorithm::Igv,
            DemosaicAlgorithm::Amaze,
            DemosaicAlgorithm::AmazeVng,
            DemosaicAlgorithm::GenericCfa,
        ] {
            let calls = std::cell::Cell::new(0usize);
            let cancel = || {
                calls.set(calls.get() + 1);
                calls.get() > 1
            };
            let res = dispatch_demosaic(&m, algo, 0.04, true, &cancel);
            assert!(matches!(res, Err(DevelopError::Cancelled)), "{algo:?}");
        }

        // Verify develop_raw dispatches all Bayer algorithms and rejects CFA mismatches cleanly
        let md = RawMetadata {
            make: "test".into(),
            model: "demosaic".into(),
            width: 8,
            height: 8,
            components_per_pixel: 1,
            bits_per_sample: 14,
            photometric: Photometric::Cfa,
            cfa: Some(CfaPattern {
                width: 2,
                height: 2,
                cells: vec![CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue],
            }),
            black_level: LevelGrid {
                width: 1,
                height: 1,
                components: 1,
                values: vec![0.0],
            },
            white_level: WhiteLevel(vec![1000.0]),
            white_balance: [1.0; 4],
            xyz_to_camera: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]],
            active_area: None,
            crop_area: None,
            orientation: Orientation::Normal,
        };
        let mosaic = DecodedMosaic::new(md, std::sync::Arc::new(vec![500u16; 64])).unwrap();
        let budget = MemoryBudget::new(16 * 1024 * 1024);
        for algo in [
            DemosaicAlgorithm::Auto,
            DemosaicAlgorithm::Nearest,
            DemosaicAlgorithm::Bilinear,
            DemosaicAlgorithm::SmoothHue,
            DemosaicAlgorithm::LarochePrescott,
            DemosaicAlgorithm::HamiltonAdams,
            DemosaicAlgorithm::MalvarHeCutler,
            DemosaicAlgorithm::Ppg,
            DemosaicAlgorithm::Vng,
            DemosaicAlgorithm::Rcd,
            DemosaicAlgorithm::Ahd,
            DemosaicAlgorithm::Lmmse,
            DemosaicAlgorithm::Dcb,
            DemosaicAlgorithm::Igv,
            DemosaicAlgorithm::Amaze,
            DemosaicAlgorithm::AmazeVng,
            DemosaicAlgorithm::AmazeLmmse,
            DemosaicAlgorithm::RcdVng,
            DemosaicAlgorithm::RcdLmmse,
            DemosaicAlgorithm::DcbVng,
            DemosaicAlgorithm::GenericCfa,
        ] {
            let opts = DevelopOptions {
                demosaic: algo,
                false_color_passes: 1,
                green_equilibration: 0.25,
                impulse_threshold: 3.0,
                suppress_fringe: true,
                ..Default::default()
            };
            let raster = develop_raw(&mosaic, &opts, &OpcodeLists::default(), Some(&budget), &|| false)
                .unwrap();
            let RasterPixels::Rgba32Float(px) = raster.pixels() else {
                panic!()
            };
            assert!(px.chunks_exact(4).all(|p| (p[0] - 0.5).abs() < 1e-5));
            drop(raster);
            assert_eq!(budget.used(), 0);
        }

        // Non-Bayer algorithms on a 2x2 Bayer sensor must fail explicitly
        for non_bayer in [
            DemosaicAlgorithm::XTransDirectional,
            DemosaicAlgorithm::XTransMarkesteijn,
            DemosaicAlgorithm::XTransMarkesteijn3Pass,
            DemosaicAlgorithm::XTransDual,
            DemosaicAlgorithm::QuadBayer,
            DemosaicAlgorithm::NonaBayer,
            DemosaicAlgorithm::HexadecaBayer,
        ] {
            assert!(matches!(
                develop_raw(
                    &mosaic,
                    &DevelopOptions {
                        demosaic: non_bayer,
                        ..Default::default()
                    },
                    &OpcodeLists::default(),
                    Some(&budget),
                    &|| false
                ),
                Err(DevelopError::Invalid(_))
            ));
            assert_eq!(budget.used(), 0);
        }
    }
}


