// SPDX-License-Identifier: Apache-2.0
// Adapted from storytold/lightcraft, commit 294012742e277d95e59db0072c88bfd3d296f6cc.
// Modified for Rrrah: strict validation, sensor clipping masks, memory admission,
// sequential cancellable kernels and the existing scene-linear display path.
//! Spatial highlight reconstruction on white-balanced camera RGB.
//! Clipping masks are captured from sensor samples before gain maps and WB.
//!
//! - [`reconstruct`]: where only some channels are clipped, rebuild them from the unclipped channels using the
//!   local chromaticity of nearby unclipped pixels (diffused into the clipped region with a coarse-to-fine
//!   normalised-convolution fill). Fully clipped pixels become neutral at the brightest plausible level.

use super::Rgb32f;

// Maximum simultaneously live chromaticity/validity pyramid payload. Sum
// rounded dimensions explicitly: a 1-pixel-wide image does not shrink by 4.
pub(super) fn pyramid_capacity_bytes(mut w: usize, mut h: usize) -> Option<u64> {
    if w == 0 || h == 0 {
        return Some(0);
    }
    let mut total = 0u64;
    loop {
        w = w.div_ceil(2);
        h = h.div_ceil(2);
        let pixels = (w as u64).checked_mul(h as u64)?;
        let bytes =
            pixels.checked_mul((std::mem::size_of::<[f64; 3]>() + std::mem::size_of::<bool>()) as u64)?;
        total = total.checked_add(bytes)?;
        if w == 1 && h == 1 {
            return Some(total);
        }
    }
}

/// Fill `values` (per-pixel vectors) where `valid` is false from valid neighbours, coarse to fine.
fn fill_invalid(w: usize, h: usize, values: &mut [[f64; 3]], valid: &mut [bool]) {
    if w == 0 || h == 0 || valid.iter().all(|&v| v) || !valid.iter().any(|&v| v) {
        return;
    }
    // Build a pyramid of weighted means, fill each level from the next coarser one.
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let mut cv = vec![[0f64; 3]; cw * ch];
    let mut cval = vec![false; cw * ch];
    {
        let (values, valid) = (&*values, &*valid);
        downsample(w, h, &mut cv, &mut cval, |i| valid[i].then(|| values[i]));
    }
    if cw * ch < w * h {
        fill_invalid(cw, ch, &mut cv, &mut cval);
    } else {
        // cannot shrink further (1×1): nothing valid anywhere handled above
        return;
    }
    values
        .chunks_mut(w)
        .zip(valid.chunks_mut(w))
        .enumerate()
        .for_each(|(y, (vrow, okrow))| {
            for x in 0..w {
                if !okrow[x] {
                    vrow[x] = upsample(&cv, cw, ch, x, y);
                    okrow[x] = true;
                }
            }
        });
}

/// One pyramid step: each `2 × 2` block of a `w × h` level becomes the mean of its valid
/// entries (`at(i)`: the value at index `i` when valid) in `cv`, `cval` (`⌈w/2⌉ × ⌈h/2⌉`).
fn downsample(
    w: usize,
    h: usize,
    cv: &mut [[f64; 3]],
    cval: &mut [bool],
    at: impl Fn(usize) -> Option<[f64; 3]> + Sync,
) {
    let cw = w.div_ceil(2);
    cv.chunks_mut(cw)
        .zip(cval.chunks_mut(cw))
        .enumerate()
        .for_each(|(y, (vrow, okrow))| {
            for x in 0..cw {
                let (mut s, mut n) = ([0f64; 3], 0);
                for dy in 0..2 {
                    for dx in 0..2 {
                        let (sx, sy) = (2 * x + dx, 2 * y + dy);
                        if sx < w
                            && sy < h
                            && let Some(v) = at(sy * w + sx)
                        {
                            for c in 0..3 {
                                s[c] += v[c];
                            }
                            n += 1;
                        }
                    }
                }
                if n > 0 {
                    vrow[x] = s.map(|v| v / f64::from(n));
                    okrow[x] = true;
                }
            }
        });
}

/// Bilinear sample of the `cw × ch` coarse level at fine pixel `(x, y)` (twice the resolution).
fn upsample(cv: &[[f64; 3]], cw: usize, ch: usize, x: usize, y: usize) -> [f64; 3] {
    let fx = ((x as f64 + 0.5) / 2.0 - 0.5).clamp(0.0, (cw - 1) as f64);
    let fy = ((y as f64 + 0.5) / 2.0 - 0.5).clamp(0.0, (ch - 1) as f64);
    let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(cw - 1), (y0 + 1).min(ch - 1));
    let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
    let g = |xx: usize, yy: usize| cv[yy * cw + xx];
    let mut v = [0f64; 3];
    for c in 0..3 {
        let top = g(x0, y0)[c] + (g(x1, y0)[c] - g(x0, y0)[c]) * tx;
        let bot = g(x0, y1)[c] + (g(x1, y1)[c] - g(x0, y1)[c]) * tx;
        v[c] = top + (bot - top) * ty;
    }
    v
}

/// White-balanced chromaticity of an unclipped pixel (`None`: clipped or too dark to tell).
#[inline]
fn chroma_of(p: &[f64; 3], _wb: [f64; 3], clip: f64) -> Option<[f64; 3]> {
    if p[0] >= clip || p[1] >= clip || p[2] >= clip {
        return None;
    }
    let q = *p;
    let s = q[0] + q[1] + q[2];
    (s > 1e-4).then(|| q.map(|v| v.max(0.0) / s))
}

/// Reconstruct partially clipped channels. `clip` is the sensor clip level in normalised units (use slightly
/// below 1.0, e.g. 0.99), `wb` the white-balance multipliers already applied to the camera RGB.
/// Returns the number of pixels that had at least one clipped channel.
///
/// The chromaticity field is diffused coarse to fine from the unclipped pixels. A clipped pixel is
/// never valid at full resolution, so its chromaticity is always the bilinear sample of the
/// half-resolution level: we start the pyramid there, straight from the image, and never build
/// the full-resolution field (same result, a quarter of the memory and far less work).
pub(super) fn reconstruct(img: &mut Rgb32f, wb: [f64; 3], clip: f64, clipped: &[[bool; 3]]) -> usize {
    let (w, h) = (img.width, img.height);
    let count = clipped.iter().filter(|p| p.iter().any(|c| *c)).count();
    if count == 0 {
        return 0;
    }
    // chromaticity (white-balanced ratios to the channel sum) of unclipped pixels, at half resolution
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let mut cv = vec![[1f64 / 3.0; 3]; cw * ch];
    let mut cval = vec![false; cw * ch];
    {
        let data = &img.data;
        downsample(w, h, &mut cv, &mut cval, |i| {
            if clipped[i].iter().any(|c| *c) {
                None
            } else {
                chroma_of(&data[i], wb, f64::MAX)
            }
        });
    }
    if cw * ch < w * h {
        fill_invalid(cw, ch, &mut cv, &mut cval);
    }
    let any_valid = cw * ch < w * h && cval.iter().any(|&v| v);
    let max_level = wb.iter().copied().fold(0.0f64, f64::max) * clip;
    img.data.chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, px) in row.iter_mut().enumerate() {
            let p = *px;
            let cl = clipped[y * w + x];
            if !cl.iter().any(|&b| b) {
                continue;
            }
            let r = if any_valid {
                upsample(&cv, cw, ch, x, y)
            } else {
                [1f64 / 3.0; 3]
            };
            let q = p;
            // estimate the white-balanced channel sum from the unclipped channels
            let (mut sum, mut k) = (0f64, 0usize);
            for c in 0..3 {
                if !cl[c] && r[c] > 1e-3 {
                    sum += q[c] / r[c];
                    k += 1;
                }
            }
            let mut out = q;
            if k == 0 {
                let v = q.iter().copied().fold(0.0f64, f64::max).max(max_level);
                out = [v; 3];
            } else {
                let sum = sum / k as f64;
                for c in 0..3 {
                    if cl[c] {
                        out[c] = q[c].max(sum * r[c]);
                    }
                }
            }
            *px = out;
        }
    });
    count
}

#[cfg(test)]
mod memory_admission_tests {
    #[test]
    fn pyramid_payload_counts_rounded_and_narrow_levels() {
        use super::pyramid_capacity_bytes as bytes;
        assert_eq!(bytes(0, 9), Some(0));
        assert_eq!(bytes(1, 1), Some(25));
        assert_eq!(bytes(1, 9), Some((5 + 3 + 2 + 1) * 25));
        assert_eq!(bytes(9, 1), bytes(1, 9));
        assert_eq!(bytes(9, 9), Some((25 + 9 + 4 + 1) * 25));
        assert_eq!(bytes(8, 8), Some((16 + 4 + 1) * 25));
        assert_eq!(bytes(usize::MAX, usize::MAX), None);
    }
}
