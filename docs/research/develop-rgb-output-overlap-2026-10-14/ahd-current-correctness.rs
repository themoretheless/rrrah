struct Rgb32f {width:usize,height:usize,data:Vec<[f64;3]>}
impl Rgb32f {fn new(width:usize,height:usize)->Self {Self{width,height,data:vec![[0.;3];width*height]}}}
#[derive(Debug)] enum DevelopError {Cancelled}
struct Cfa {
    width: usize,
    height: usize,
    cells: Vec<u8>,
}
impl Cfa {
    fn color_at(&self, x: usize, y: usize) -> u8 {
        self.cells[y % self.height * self.width + x % self.width]
    }
}
struct Mosaic<'a> {
    w: usize,
    h: usize,
    data: &'a [f64],
    cfa: &'a Cfa,
}
fn reflect(i: isize, n: usize) -> usize {
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
    fn get(&self, x: isize, y: isize) -> f64 {
        self.data[reflect(y, self.h) * self.w + reflect(x, self.w)]
    }
    fn color(&self, x: isize, y: isize) -> u8 {
        self.cfa.color_at(reflect(x, self.w), reflect(y, self.h))
    }
}
mod demosaic {const TILE:usize=128;pub(super) trait TileScratch: Default {
    const MARGIN: usize;
    const BYTES_PER_PIXEL: usize;
}

// The caller's working reservation includes 8 MiB for tiled worker payloads.
// Keep all concurrently live tile buffers inside that allowance, even on
// machines with many cores or tall, narrow sensors.
pub(super) fn tiled_worker_count<S: TileScratch>(width: usize, height: usize, available: usize) -> usize {
    let padded_width = TILE.min(width) + 2 * S::MARGIN;
    let padded_height = TILE.min(height) + 2 * S::MARGIN;
    let per_worker =
        padded_width * padded_height * S::BYTES_PER_PIXEL + TILE * TILE * std::mem::size_of::<[f64; 3]>();
    let admitted = (8 * 1024 * 1024 / per_worker).max(1);
    available.min(height.div_ceil(TILE)).min(admitted).max(1)
}

}
mod ahd { // SPDX-License-Identifier: Apache-2.0
// Adapted from storytold/lightcraft, commit 294012742e277d95e59db0072c88bfd3d296f6cc.
// Modified for Rrrah: strict validation, sensor clipping masks, memory admission,
// sequential cancellable kernels and the existing scene-linear display path.

use super::Rgb32f;
use super::{DevelopError, Mosaic};

const TILE: usize = 128;
const MARGIN: usize = 6;

#[derive(Default)]
struct AhdScratch {
    mos: Vec<f64>,
    col: Vec<u8>,
    gh: Vec<f64>,
    gv: Vec<f64>,
    rh: Vec<[f64; 3]>,
    rv: Vec<[f64; 3]>,
    lh: Vec<[f64; 3]>,
    lv: Vec<[f64; 3]>,
    hh: Vec<u8>,
    hv: Vec<u8>,
}

impl super::demosaic::TileScratch for AhdScratch {
    const MARGIN: usize = MARGIN;
    const BYTES_PER_PIXEL: usize =
        3 * std::mem::size_of::<f64>() + 4 * std::mem::size_of::<[f64; 3]>() + 3 * std::mem::size_of::<u8>();
}

pub(crate) fn ahd(m: &Mosaic, cancelled: &impl Fn() -> bool) -> Result<Rgb32f, DevelopError> {
    let (w, h) = (m.w, m.h);
    let mut out = Rgb32f::new(w, h);
    if w == 0 || h == 0 {
        return Ok(out);
    }
    let workers = super::demosaic::tiled_worker_count::<AhdScratch>(
        w,
        h,
        std::thread::available_parallelism().map_or(1, usize::from),
    );

    if workers <= 1 {
        let mut scratch = AhdScratch::default();
        let mut px = Vec::with_capacity(TILE * TILE);
        for y0 in (0..h).step_by(TILE) {
            for x0 in (0..w).step_by(TILE) {
                if cancelled() {
                    return Err(DevelopError::Cancelled);
                }
                let (tw, th) = (TILE.min(w - x0), TILE.min(h - y0));
                tile_into(m, x0, y0, tw, th, &mut scratch, &mut px);
                for y in 0..th {
                    out.data[(y0 + y) * w + x0..(y0 + y) * w + x0 + tw]
                        .copy_from_slice(&px[y * tw..(y + 1) * tw]);
                }
            }
        }
    } else {
        use std::sync::Mutex;
        use std::sync::atomic::{AtomicBool, Ordering};

        let is_cancelled = AtomicBool::new(false);
        // Hand out disjoint mutable bands directly from the iterator. This
        // avoids a slot allocation proportional to image height.
        let bands = Mutex::new(out.data.chunks_mut(TILE * w).enumerate());

        let do_work = || {
            let mut scratch = AhdScratch::default();
            let mut px = Vec::with_capacity(TILE * TILE);
            loop {
                if is_cancelled.load(Ordering::Relaxed) {
                    break;
                }
                let Some((band_idx, band)) = bands.lock().expect("band iterator lock never poisoned").next()
                else {
                    break;
                };
                let y0 = band_idx * TILE;
                let th = TILE.min(h - y0);
                for x0 in (0..w).step_by(TILE) {
                    if is_cancelled.load(Ordering::Relaxed) {
                        break;
                    }
                    let tw = TILE.min(w - x0);
                    tile_into(m, x0, y0, tw, th, &mut scratch, &mut px);
                    for y in 0..th {
                        band[y * w + x0..y * w + x0 + tw].copy_from_slice(&px[y * tw..(y + 1) * tw]);
                    }
                }
            }
        };

        std::thread::scope(|scope| {
            let shared_worker = &do_work;
            for worker_index in 1..workers {
                let _ = std::thread::Builder::new()
                    .name(format!("rrrah-ahd-{worker_index}"))
                    .spawn_scoped(scope, shared_worker);
            }
            let mut scratch = AhdScratch::default();
            let mut px = Vec::with_capacity(TILE * TILE);
            loop {
                if cancelled() {
                    is_cancelled.store(true, Ordering::Relaxed);
                    break;
                }
                let Some((band_idx, band)) = bands.lock().expect("band iterator lock never poisoned").next()
                else {
                    break;
                };
                let y0 = band_idx * TILE;
                let th = TILE.min(h - y0);
                for x0 in (0..w).step_by(TILE) {
                    if cancelled() {
                        is_cancelled.store(true, Ordering::Relaxed);
                        break;
                    }
                    let tw = TILE.min(w - x0);
                    tile_into(m, x0, y0, tw, th, &mut scratch, &mut px);
                    for y in 0..th {
                        band[y * w + x0..y * w + x0 + tw].copy_from_slice(&px[y * tw..(y + 1) * tw]);
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

/// Linear camera RGB (treated as sRGB/D65 primaries for the homogeneity metric only) → CIELab.
#[inline]
pub(super) fn lab(p: [f64; 3]) -> [f64; 3] {
    let x = (0.4124 * p[0] + 0.3576 * p[1] + 0.1805 * p[2]) / 0.95047;
    let y = 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
    let z = (0.0193 * p[0] + 0.1192 * p[1] + 0.9505 * p[2]) / 1.088_83;
    #[inline]
    fn f(t: f64) -> f64 {
        if t > 0.008856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    }
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

#[cfg(test)]
pub(super) fn tile(m: &Mosaic, x0: usize, y0: usize, tw: usize, th: usize) -> Vec<[f64; 3]> {
    let mut scratch = AhdScratch::default();
    let mut out = Vec::with_capacity(tw * th);
    tile_into(m, x0, y0, tw, th, &mut scratch, &mut out);
    out
}

fn tile_into(
    m: &Mosaic,
    x0: usize,
    y0: usize,
    tw: usize,
    th: usize,
    scratch: &mut AhdScratch,
    out: &mut Vec<[f64; 3]>,
) {
    let (bw, bh) = (tw + 2 * MARGIN, th + 2 * MARGIN);
    let n = bw * bh;
    let (ox, oy) = (x0 as isize - MARGIN as isize, y0 as isize - MARGIN as isize);
    scratch.mos.resize(n, 0.0);
    scratch.col.resize(n, 0);
    scratch.gh.resize(n, 0.0);
    scratch.gv.resize(n, 0.0);
    scratch.rh.resize(n, [0.0; 3]);
    scratch.rv.resize(n, [0.0; 3]);
    scratch.lh.resize(n, [0.0; 3]);
    scratch.lv.resize(n, [0.0; 3]);
    scratch.hh.clear();
    scratch.hh.resize(n, 0);
    scratch.hv.clear();
    scratch.hv.resize(n, 0);

    for y in 0..bh {
        for x in 0..bw {
            scratch.mos[y * bw + x] = m.get(ox + x as isize, oy + y as isize);
            scratch.col[y * bw + x] = m.color(ox + x as isize, oy + y as isize);
        }
    }
    let cl = |x: isize, y: isize| -> usize {
        y.clamp(0, bh as isize - 1) as usize * bw + x.clamp(0, bw as isize - 1) as usize
    };
    // 1. directional greens
    for y in 0..bh as isize {
        for x in 0..bw as isize {
            let i = cl(x, y);
            let c = scratch.mos[i];
            if scratch.col[i] == 1 {
                scratch.gh[i] = c;
                scratch.gv[i] = c;
                continue;
            }
            let (l, r) = (scratch.mos[cl(x - 1, y)], scratch.mos[cl(x + 1, y)]);
            let est =
                l.midpoint(r) + (2.0 * c - scratch.mos[cl(x - 2, y)] - scratch.mos[cl(x + 2, y)]) * 0.25;
            scratch.gh[i] = est.clamp(l.min(r), l.max(r));
            let (u, d) = (scratch.mos[cl(x, y - 1)], scratch.mos[cl(x, y + 1)]);
            let est =
                u.midpoint(d) + (2.0 * c - scratch.mos[cl(x, y - 2)] - scratch.mos[cl(x, y + 2)]) * 0.25;
            scratch.gv[i] = est.clamp(u.min(d), u.max(d));
        }
    }
    // 2. full candidates
    let complete = |g: &[f64], mos: &[f64], col: &[u8], rgb: &mut [[f64; 3]]| {
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
    };
    complete(&scratch.gh, &scratch.mos, &scratch.col, &mut scratch.rh);
    complete(&scratch.gv, &scratch.mos, &scratch.col, &mut scratch.rv);
    // 3. Lab + homogeneity
    for i in 0..n {
        scratch.lh[i] = lab(scratch.rh[i]);
        scratch.lv[i] = lab(scratch.rv[i]);
    }
    let dl = |a: &[f64; 3], b: &[f64; 3]| (a[0] - b[0]).abs();
    let dc = |a: &[f64; 3], b: &[f64; 3]| (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2);
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
            let mut ch = 0;
            let mut cv = 0;
            for j in [l, r, u, d] {
                if dl(&scratch.lh[i], &scratch.lh[j]) <= eps_l && dc(&scratch.lh[i], &scratch.lh[j]) <= eps_c
                {
                    ch += 1;
                }
                if dl(&scratch.lv[i], &scratch.lv[j]) <= eps_l && dc(&scratch.lv[i], &scratch.lv[j]) <= eps_c
                {
                    cv += 1;
                }
            }
            scratch.hh[i] = ch;
            scratch.hv[i] = cv;
        }
    }
    // 4. choose
    out.clear();
    out.reserve(tw * th);
    for y in MARGIN..MARGIN + th {
        for x in MARGIN..MARGIN + tw {
            let (mut sh, mut sv) = (0u32, 0u32);
            for dy in -1isize..=1 {
                for dx in -1isize..=1 {
                    let j = cl(x as isize + dx, y as isize + dy);
                    sh += u32::from(scratch.hh[j]);
                    sv += u32::from(scratch.hv[j]);
                }
            }
            let i = y * bw + x;
            out.push(if sh > sv {
                scratch.rh[i]
            } else if sv > sh {
                scratch.rv[i]
            } else {
                [
                    scratch.rh[i][0].midpoint(scratch.rv[i][0]),
                    scratch.rh[i][1].midpoint(scratch.rv[i][1]),
                    scratch.rh[i][2].midpoint(scratch.rv[i][2]),
                ]
            });
        }
    }
}}

#[test] fn tall_ahd_matches_full_frame_bits() {
 let cfa=Cfa{width:2,height:2,cells:vec![0,1,1,2]};
 for (w,h) in [(259,141),(128,2048),(7,257),(1,129)] {
  let mut seed=42u32;
  let data:Vec<f64>=(0..w*h).map(|_|{seed=seed.wrapping_mul(1664525).wrapping_add(1013904223); f64::from(seed>>8)/16777216.}).collect();
  let m=Mosaic{w,h,data:&data,cfa:&cfa};
  let tiled=ahd::ahd(&m,&||false).unwrap();let full=ahd::tile(&m,0,0,w,h);
  assert_eq!(tiled.data.len(),full.len());
  for (i,(a,b)) in tiled.data.iter().zip(&full).enumerate(){assert_eq!(a.map(f64::to_bits),b.map(f64::to_bits),"{w}x{h} pixel {i}");}
 }
}
