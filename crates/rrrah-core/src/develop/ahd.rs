// SPDX-License-Identifier: Apache-2.0
// Adapted from storytold/lightcraft, commit 294012742e277d95e59db0072c88bfd3d296f6cc.
// Modified for Rrrah: strict validation, sensor clipping masks, memory admission,
// sequential cancellable kernels and the existing scene-linear display path.
//! Adaptive Homogeneity-Directed demosaicing (K. Hirakawa, T. W. Parks, "Adaptive homogeneity-directed
//! demosaicing algorithm", IEEE Trans. Image Processing 14(3), 2005), implemented from the paper:
//!
//! 1. Interpolate green horizontally and vertically (5-tap filter [−¼ ½ ½ ½ −¼] across the missing site,
//!    clamped to the two adjacent greens), giving two candidate green planes.
//! 2. Complete each candidate with red/blue by colour-difference (C − G) interpolation.
//! 3. Convert both candidates to CIELab and measure, per pixel, how many of its 4 neighbours lie within the
//!    adaptive luminance / chrominance tolerances (ε_L, ε_C from the paper) — the homogeneity.
//! 4. Sum homogeneity over a 3×3 window and pick the more homogeneous candidate (average on ties).
//!
//! Work is done in overlapping tiles (bounded memory, sequential tile scratch).

use super::Rgb32f;
use super::{DevelopError, Mosaic};

const TILE: usize = 128;
const MARGIN: usize = 6;

pub(crate) fn ahd(m: &Mosaic, cancelled: &impl Fn() -> bool) -> Result<Rgb32f, DevelopError> {
    let (w, h) = (m.w, m.h);
    let mut out = Rgb32f::new(w, h);
    for y0 in (0..h).step_by(TILE) {
        for x0 in (0..w).step_by(TILE) {
            if cancelled() {
                return Err(DevelopError::Cancelled);
            }
            let (tw, th) = (TILE.min(w - x0), TILE.min(h - y0));
            let px = tile(m, x0, y0, tw, th);
            for y in 0..th {
                out.data[(y0 + y) * w + x0..(y0 + y) * w + x0 + tw]
                    .copy_from_slice(&px[y * tw..(y + 1) * tw]);
            }
        }
    }
    Ok(out)
}

/// Linear camera RGB (treated as sRGB/D65 primaries for the homogeneity metric only) → CIELab.
#[inline]
fn lab(p: [f32; 3]) -> [f32; 3] {
    let x = (0.4124 * p[0] + 0.3576 * p[1] + 0.1805 * p[2]) / 0.95047;
    let y = 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
    let z = (0.0193 * p[0] + 0.1192 * p[1] + 0.9505 * p[2]) / 1.088_83;
    #[inline]
    fn f(t: f32) -> f32 {
        if t > 0.008856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    }
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

pub(super) fn tile(m: &Mosaic, x0: usize, y0: usize, tw: usize, th: usize) -> Vec<[f32; 3]> {
    let (bw, bh) = (tw + 2 * MARGIN, th + 2 * MARGIN);
    let n = bw * bh;
    let (ox, oy) = (x0 as isize - MARGIN as isize, y0 as isize - MARGIN as isize);
    let mut mos = vec![0f32; n];
    let mut col = vec![0u8; n];
    for y in 0..bh {
        for x in 0..bw {
            mos[y * bw + x] = m.get(ox + x as isize, oy + y as isize);
            col[y * bw + x] = m.color(ox + x as isize, oy + y as isize);
        }
    }
    let cl = |x: isize, y: isize| -> usize {
        y.clamp(0, bh as isize - 1) as usize * bw + x.clamp(0, bw as isize - 1) as usize
    };
    // 1. directional greens
    let mut gh = vec![0f32; n];
    let mut gv = vec![0f32; n];
    for y in 0..bh as isize {
        for x in 0..bw as isize {
            let i = cl(x, y);
            let c = mos[i];
            if col[i] == 1 {
                gh[i] = c;
                gv[i] = c;
                continue;
            }
            let (l, r) = (mos[cl(x - 1, y)], mos[cl(x + 1, y)]);
            let est = l.midpoint(r) + (2.0 * c - mos[cl(x - 2, y)] - mos[cl(x + 2, y)]) * 0.25;
            gh[i] = est.clamp(l.min(r), l.max(r));
            let (u, d) = (mos[cl(x, y - 1)], mos[cl(x, y + 1)]);
            let est = u.midpoint(d) + (2.0 * c - mos[cl(x, y - 2)] - mos[cl(x, y + 2)]) * 0.25;
            gv[i] = est.clamp(u.min(d), u.max(d));
        }
    }
    // 2. full candidates
    let complete = |g: &[f32]| -> Vec<[f32; 3]> {
        let mut rgb = vec![[0f32; 3]; n];
        for y in 0..bh as isize {
            for x in 0..bw as isize {
                let i = cl(x, y);
                let own = col[i] as usize;
                let gvv = g[i];
                let d = |dx: isize, dy: isize| {
                    let j = cl(x + dx, y + dy);
                    mos[j] - g[j]
                };
                let p = &mut rgb[i];
                p[1] = gvv;
                if own == 1 {
                    let ch = col[cl(x + 1, y)] as usize;
                    let cv = col[cl(x, y + 1)] as usize;
                    p[ch] = gvv + d(-1, 0).midpoint(d(1, 0));
                    p[cv] = gvv + d(0, -1).midpoint(d(0, 1));
                } else {
                    p[own] = mos[i];
                    p[2 - own] = gvv + (d(-1, -1) + d(1, -1) + d(-1, 1) + d(1, 1)) * 0.25;
                }
            }
        }
        rgb
    };
    let rh = complete(&gh);
    let rv = complete(&gv);
    // 3. Lab + homogeneity
    let lh: Vec<[f32; 3]> = rh.iter().map(|&p| lab(p)).collect();
    let lv: Vec<[f32; 3]> = rv.iter().map(|&p| lab(p)).collect();
    let dl = |a: &[f32; 3], b: &[f32; 3]| (a[0] - b[0]).abs();
    let dc = |a: &[f32; 3], b: &[f32; 3]| (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2);
    let mut hh = vec![0u8; n];
    let mut hv = vec![0u8; n];
    for y in 1..bh as isize - 1 {
        for x in 1..bw as isize - 1 {
            let i = cl(x, y);
            let (l, r, u, d) = (cl(x - 1, y), cl(x + 1, y), cl(x, y - 1), cl(x, y + 1));
            let eps_l = dl(&lh[i], &lh[l])
                .max(dl(&lh[i], &lh[r]))
                .min(dl(&lv[i], &lv[u]).max(dl(&lv[i], &lv[d])));
            let eps_c = dc(&lh[i], &lh[l])
                .max(dc(&lh[i], &lh[r]))
                .min(dc(&lv[i], &lv[u]).max(dc(&lv[i], &lv[d])));
            let mut ch = 0;
            let mut cv = 0;
            for j in [l, r, u, d] {
                if dl(&lh[i], &lh[j]) <= eps_l && dc(&lh[i], &lh[j]) <= eps_c {
                    ch += 1;
                }
                if dl(&lv[i], &lv[j]) <= eps_l && dc(&lv[i], &lv[j]) <= eps_c {
                    cv += 1;
                }
            }
            hh[i] = ch;
            hv[i] = cv;
        }
    }
    // 4. choose
    let mut out = Vec::with_capacity(tw * th);
    for y in MARGIN..MARGIN + th {
        for x in MARGIN..MARGIN + tw {
            let (mut sh, mut sv) = (0u32, 0u32);
            for dy in -1isize..=1 {
                for dx in -1isize..=1 {
                    let j = cl(x as isize + dx, y as isize + dy);
                    sh += u32::from(hh[j]);
                    sv += u32::from(hv[j]);
                }
            }
            let i = y * bw + x;
            out.push(if sh > sv {
                rh[i]
            } else if sv > sh {
                rv[i]
            } else {
                [
                    rh[i][0].midpoint(rv[i][0]),
                    rh[i][1].midpoint(rv[i][1]),
                    rh[i][2].midpoint(rv[i][2]),
                ]
            });
        }
    }
    out
}
