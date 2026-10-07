// SPDX-License-Identifier: Apache-2.0
// Adapted from storytold/lightcraft, commit 294012742e277d95e59db0072c88bfd3d296f6cc.
// Modified for Rrrah: strict validation, sensor clipping masks, memory admission,
// sequential cancellable kernels and the existing scene-linear display path.
//! DNG opcode lists (`OpcodeList1/2/3`, DNG 1.7 chapter 7). Opcode lists are always big-endian.
//!
//! Parsed: every opcode (unknown ones are kept as [`Opcode::Unknown`]). Applied: `WarpRectilinear`,
//! `FixVignetteRadial`, `FixBadPixelsConstant`, `FixBadPixelsList`, `MapTable`, `MapPolynomial`, `GainMap`,
//! `DeltaPerRow/Column`, `ScalePerRow/Column`. `TrimBounds` and `WarpFisheye` are recorded but not applied.
//!
//! Value convention: list 1 runs on raw sample values (16-bit scale), lists 2 and 3 on values normalised to
//! [0, 1]; table/polynomial/delta opcodes are defined on the normalised range and are rescaled accordingly.

use super::{Cfa, Rgb32f};
use serde::{Deserialize, Serialize};

/// Rectangular area + plane selection + pitch shared by the "area" opcodes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Area {
    pub top: u32,
    pub left: u32,
    pub bottom: u32,
    pub right: u32,
    pub plane: u32,
    pub planes: u32,
    pub row_pitch: u32,
    pub col_pitch: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Opcode {
    /// Per-plane radial (kr0..kr3) + tangential (kt0, kt1) coefficients and the relative optical centre.
    WarpRectilinear {
        planes: Vec<[f64; 6]>,
        center: [f64; 2],
    },
    WarpFisheye {
        planes: Vec<[f64; 4]>,
        center: [f64; 2],
    },
    FixVignetteRadial {
        k: [f64; 5],
        center: [f64; 2],
    },
    FixBadPixelsConstant {
        constant: u32,
        bayer_phase: u32,
    },
    FixBadPixelsList {
        bayer_phase: u32,
        points: Vec<(u32, u32)>,
        rects: Vec<[u32; 4]>,
    },
    TrimBounds {
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
    },
    MapTable {
        area: Area,
        table: Vec<u16>,
    },
    MapPolynomial {
        area: Area,
        coefficients: Vec<f64>,
    },
    GainMap {
        area: Area,
        points_v: u32,
        points_h: u32,
        spacing: [f64; 2],
        origin: [f64; 2],
        map_planes: u32,
        gains: Vec<f32>,
    },
    DeltaPerRow {
        area: Area,
        deltas: Vec<f32>,
    },
    DeltaPerColumn {
        area: Area,
        deltas: Vec<f32>,
    },
    ScalePerRow {
        area: Area,
        scales: Vec<f32>,
    },
    ScalePerColumn {
        area: Area,
        scales: Vec<f32>,
    },
    Unknown {
        id: u32,
        flags: u32,
        params: Vec<u8>,
    },
}

impl Opcode {
    /// Whether [`apply_list`] / [`apply_list3`] implement this opcode.
    pub fn is_applied(&self) -> bool {
        matches!(
            self,
            Self::WarpRectilinear { .. }
                | Self::GainMap { .. }
                | Self::FixVignetteRadial { .. }
                | Self::FixBadPixelsConstant { .. }
                | Self::FixBadPixelsList { .. }
        )
    }
}

/// The three opcode lists of a DNG raw IFD.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OpcodeLists {
    /// Horizontal / vertical DNG DefaultScale; absent means square pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pixel_aspect: Option<f64>,
    pub list1: Vec<Opcode>,
    pub list2: Vec<Opcode>,
    pub list3: Vec<Opcode>,
}

struct Rd<'a> {
    d: &'a [u8],
    p: usize,
}

impl Rd<'_> {
    fn u32(&mut self) -> Option<u32> {
        let v = u32::from_be_bytes(self.d.get(self.p..self.p + 4)?.try_into().ok()?);
        self.p += 4;
        Some(v)
    }
    fn u16(&mut self) -> Option<u16> {
        let v = u16::from_be_bytes(self.d.get(self.p..self.p + 2)?.try_into().ok()?);
        self.p += 2;
        Some(v)
    }
    fn f64(&mut self) -> Option<f64> {
        let v = f64::from_be_bytes(self.d.get(self.p..self.p + 8)?.try_into().ok()?);
        self.p += 8;
        Some(v)
    }
    fn f32(&mut self) -> Option<f32> {
        let v = f32::from_be_bytes(self.d.get(self.p..self.p + 4)?.try_into().ok()?);
        self.p += 4;
        Some(v)
    }
    fn area(&mut self) -> Option<Area> {
        Some(Area {
            top: self.u32()?,
            left: self.u32()?,
            bottom: self.u32()?,
            right: self.u32()?,
            plane: self.u32()?,
            planes: self.u32()?,
            row_pitch: self.u32()?,
            col_pitch: self.u32()?,
        })
    }
    fn remaining(&self) -> usize {
        self.d.len().saturating_sub(self.p)
    }
}

fn parse_one(id: u32, flags: u32, params: &[u8]) -> Option<Opcode> {
    let mut r = Rd { d: params, p: 0 };
    let f = |r: &mut Rd| r.f64().filter(|v| v.is_finite());
    let op = match id {
        1 => {
            let n = r.u32()? as usize;
            if n == 0 || n > 4 {
                return None;
            }
            let mut planes = Vec::with_capacity(n);
            for _ in 0..n {
                planes.push([
                    f(&mut r)?,
                    f(&mut r)?,
                    f(&mut r)?,
                    f(&mut r)?,
                    f(&mut r)?,
                    f(&mut r)?,
                ]);
            }
            Opcode::WarpRectilinear {
                planes,
                center: [f(&mut r)?, f(&mut r)?],
            }
        }
        2 => {
            let n = r.u32()? as usize;
            if n == 0 || n > 4 {
                return None;
            }
            let mut planes = Vec::with_capacity(n);
            for _ in 0..n {
                planes.push([f(&mut r)?, f(&mut r)?, f(&mut r)?, f(&mut r)?]);
            }
            Opcode::WarpFisheye {
                planes,
                center: [f(&mut r)?, f(&mut r)?],
            }
        }
        3 => Opcode::FixVignetteRadial {
            k: [f(&mut r)?, f(&mut r)?, f(&mut r)?, f(&mut r)?, f(&mut r)?],
            center: [f(&mut r)?, f(&mut r)?],
        },
        4 => Opcode::FixBadPixelsConstant {
            constant: r.u32()?,
            bayer_phase: r.u32()?,
        },
        5 => {
            let bayer_phase = r.u32()?;
            let (np, nr) = (r.u32()? as usize, r.u32()? as usize);
            if np.saturating_mul(8).saturating_add(nr.saturating_mul(16)) > r.remaining() {
                return None;
            }
            let points = (0..np)
                .map(|_| Some((r.u32()?, r.u32()?)))
                .collect::<Option<_>>()?;
            let rects = (0..nr)
                .map(|_| Some([r.u32()?, r.u32()?, r.u32()?, r.u32()?]))
                .collect::<Option<_>>()?;
            Opcode::FixBadPixelsList {
                bayer_phase,
                points,
                rects,
            }
        }
        6 => Opcode::TrimBounds {
            top: r.u32()?,
            left: r.u32()?,
            bottom: r.u32()?,
            right: r.u32()?,
        },
        7 => {
            let area = r.area()?;
            let n = r.u32()? as usize;
            if n == 0 || n > 65536 || n * 2 > r.remaining() {
                return None;
            }
            Opcode::MapTable {
                area,
                table: (0..n).map(|_| r.u16()).collect::<Option<_>>()?,
            }
        }
        8 => {
            let area = r.area()?;
            let deg = r.u32()? as usize;
            if deg > 8 {
                return None;
            }
            Opcode::MapPolynomial {
                area,
                coefficients: (0..=deg).map(|_| f(&mut r)).collect::<Option<_>>()?,
            }
        }
        9 => {
            let area = r.area()?;
            let (pv, ph) = (r.u32()?, r.u32()?);
            let spacing = [f(&mut r)?, f(&mut r)?];
            let origin = [f(&mut r)?, f(&mut r)?];
            let mp = r.u32()?;
            let n = (pv as usize).checked_mul(ph as usize)?.checked_mul(mp as usize)?;
            if n == 0 || n.checked_mul(4)? > r.remaining() {
                return None;
            }
            let gains = (0..n).map(|_| r.f32()).collect::<Option<_>>()?;
            Opcode::GainMap {
                area,
                points_v: pv,
                points_h: ph,
                spacing,
                origin,
                map_planes: mp,
                gains,
            }
        }
        10..=13 => {
            let area = r.area()?;
            let n = r.u32()? as usize;
            if n.checked_mul(4)? > r.remaining() {
                return None;
            }
            let v: Vec<f32> = (0..n).map(|_| r.f32()).collect::<Option<_>>()?;
            match id {
                10 => Opcode::DeltaPerRow { area, deltas: v },
                11 => Opcode::DeltaPerColumn { area, deltas: v },
                12 => Opcode::ScalePerRow { area, scales: v },
                _ => Opcode::ScalePerColumn { area, scales: v },
            }
        }
        _ => {
            r.p = params.len();
            Opcode::Unknown {
                id,
                flags,
                params: params.to_vec(),
            }
        }
    };
    (r.remaining() == 0).then_some(op)
}

#[cfg(test)]
fn put_area(o: &mut Vec<u8>, a: &Area) {
    for v in [
        a.top,
        a.left,
        a.bottom,
        a.right,
        a.plane,
        a.planes,
        a.row_pitch,
        a.col_pitch,
    ] {
        o.extend_from_slice(&v.to_be_bytes());
    }
}

#[cfg(test)]
fn params(op: &Opcode) -> (u32, u32, Vec<u8>) {
    let mut o = Vec::new();
    let f64s = |o: &mut Vec<u8>, v: &[f64]| v.iter().for_each(|x| o.extend_from_slice(&x.to_be_bytes()));
    let u32s = |o: &mut Vec<u8>, v: &[u32]| v.iter().for_each(|x| o.extend_from_slice(&x.to_be_bytes()));
    let f32s = |o: &mut Vec<u8>, v: &[f32]| v.iter().for_each(|x| o.extend_from_slice(&x.to_be_bytes()));
    let id = match op {
        Opcode::WarpRectilinear { planes, center } => {
            u32s(&mut o, &[planes.len() as u32]);
            planes.iter().for_each(|p| f64s(&mut o, p));
            f64s(&mut o, center);
            1
        }
        Opcode::WarpFisheye { planes, center } => {
            u32s(&mut o, &[planes.len() as u32]);
            planes.iter().for_each(|p| f64s(&mut o, p));
            f64s(&mut o, center);
            2
        }
        Opcode::FixVignetteRadial { k, center } => {
            f64s(&mut o, k);
            f64s(&mut o, center);
            3
        }
        Opcode::FixBadPixelsConstant {
            constant,
            bayer_phase,
        } => {
            u32s(&mut o, &[*constant, *bayer_phase]);
            4
        }
        Opcode::FixBadPixelsList {
            bayer_phase,
            points,
            rects,
        } => {
            u32s(&mut o, &[*bayer_phase, points.len() as u32, rects.len() as u32]);
            points.iter().for_each(|&(r, c)| u32s(&mut o, &[r, c]));
            rects.iter().for_each(|r| u32s(&mut o, r));
            5
        }
        Opcode::TrimBounds {
            top,
            left,
            bottom,
            right,
        } => {
            u32s(&mut o, &[*top, *left, *bottom, *right]);
            6
        }
        Opcode::MapTable { area, table } => {
            put_area(&mut o, area);
            u32s(&mut o, &[table.len() as u32]);
            table.iter().for_each(|v| o.extend_from_slice(&v.to_be_bytes()));
            7
        }
        Opcode::MapPolynomial { area, coefficients } => {
            put_area(&mut o, area);
            u32s(&mut o, &[coefficients.len().saturating_sub(1) as u32]);
            f64s(&mut o, coefficients);
            8
        }
        Opcode::GainMap {
            area,
            points_v,
            points_h,
            spacing,
            origin,
            map_planes,
            gains,
        } => {
            put_area(&mut o, area);
            u32s(&mut o, &[*points_v, *points_h]);
            f64s(&mut o, spacing);
            f64s(&mut o, origin);
            u32s(&mut o, &[*map_planes]);
            f32s(&mut o, gains);
            9
        }
        Opcode::DeltaPerRow { area, deltas: v }
        | Opcode::DeltaPerColumn { area, deltas: v }
        | Opcode::ScalePerRow { area, scales: v }
        | Opcode::ScalePerColumn { area, scales: v } => {
            put_area(&mut o, area);
            u32s(&mut o, &[v.len() as u32]);
            f32s(&mut o, v);
            match op {
                Opcode::DeltaPerRow { .. } => 10,
                Opcode::DeltaPerColumn { .. } => 11,
                Opcode::ScalePerRow { .. } => 12,
                _ => 13,
            }
        }
        Opcode::Unknown { id, flags, params } => return (*id, *flags, params.clone()),
    };
    (id, 0, o)
}

/// Serialise an opcode list (inverse of [`parse_list`]).
#[cfg(test)]
pub fn write_list(list: &[Opcode]) -> Vec<u8> {
    let mut out = (list.len() as u32).to_be_bytes().to_vec();
    for op in list {
        let (id, flags, p) = params(op);
        for v in [id, 0x0103_0000, flags, p.len() as u32] {
            out.extend_from_slice(&v.to_be_bytes());
        }
        out.extend_from_slice(&p);
    }
    out
}

/// Parse a bounded opcode list. Malformed or truncated data is rejected; optional unsupported opcodes are skipped.
pub fn parse_list(d: &[u8]) -> Result<Vec<Opcode>, super::DevelopError> {
    use super::DevelopError::Invalid;
    if d.len() > 1 << 20 {
        return Err(Invalid("opcode list exceeds 1 MiB"));
    }
    let mut r = Rd { d, p: 0 };
    let count = r.u32().ok_or(Invalid("truncated opcode count"))?;
    if count > 4096 {
        return Err(Invalid("too many opcodes"));
    }
    let mut out = Vec::new();
    for _ in 0..count {
        let id = r.u32().ok_or(Invalid("truncated opcode header"))?;
        let version = r.u32().ok_or(Invalid("truncated opcode header"))?;
        let flags = r.u32().ok_or(Invalid("truncated opcode header"))?;
        let size = r.u32().ok_or(Invalid("truncated opcode header"))? as usize;
        let params = d
            .get(r.p..r.p.checked_add(size).ok_or(Invalid("opcode size overflow"))?)
            .ok_or(Invalid("truncated opcode parameters"))?;
        r.p += size;
        if flags & !3 != 0 {
            return Err(Invalid("unknown opcode flags"));
        }
        if version > 0x0107_0100 || !matches!(id, 1 | 3 | 4 | 5 | 9) {
            if flags & 1 != 0 {
                continue;
            }
            return Err(Invalid("unsupported required opcode or minimum version"));
        }
        let op = parse_one(id, flags, params).ok_or(Invalid("malformed opcode parameters"))?;
        out.push(op);
    }
    if r.remaining() != 0 {
        return Err(Invalid("trailing bytes in opcode list"));
    }
    Ok(out)
}

pub(super) fn validate_lists(lists: &OpcodeLists) -> Result<(), super::DevelopError> {
    use super::DevelopError::Invalid;
    if lists.pixel_aspect.is_some_and(|v| !v.is_finite() || v <= 0.0) {
        return Err(Invalid("invalid DNG pixel aspect"));
    }
    for (stage, list) in [(1, &lists.list1), (2, &lists.list2), (3, &lists.list3)] {
        for op in list {
            match op {
                Opcode::FixBadPixelsConstant { bayer_phase, .. }
                | Opcode::FixBadPixelsList { bayer_phase, .. }
                    if stage == 1 && *bayer_phase <= 3 => {}
                Opcode::GainMap {
                    area,
                    points_v,
                    points_h,
                    spacing,
                    origin,
                    map_planes,
                    gains,
                } if stage == 2 => {
                    let count = u64::from(*points_v)
                        .checked_mul(u64::from(*points_h))
                        .and_then(|n| n.checked_mul(u64::from(*map_planes)));
                    if *points_v == 0
                        || *points_h == 0
                        || *map_planes != 1
                        || count != Some(gains.len() as u64)
                        || spacing.iter().any(|p| !p.is_finite() || *p <= 0.0)
                        || origin.iter().any(|p| !p.is_finite())
                        || gains.iter().any(|p| !p.is_finite() || *p <= 0.0)
                        || area.plane != 0
                        || area.planes != 1
                        || area.row_pitch == 0
                        || area.col_pitch == 0
                        || area.top >= area.bottom
                        || area.left >= area.right
                    {
                        return Err(Invalid("invalid single-plane gain map"));
                    }
                }
                Opcode::WarpRectilinear { planes, center }
                    if stage == 3
                        && matches!(planes.len(), 1 | 3)
                        && planes.iter().flatten().all(|v| v.is_finite())
                        && center.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)) => {}
                Opcode::FixVignetteRadial { k, center }
                    if stage == 3
                        && k.iter().all(|p| p.is_finite())
                        && center.iter().all(|p| p.is_finite() && (0.0..=1.0).contains(p)) => {}
                _ => return Err(Invalid("opcode is unsupported at this processing stage")),
            }
        }
    }
    Ok(())
}

fn area_iter(a: &Area, w: usize, h: usize, cpp: usize) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
    let (t, l) = (a.top as usize, a.left as usize);
    let (b, r) = ((a.bottom as usize).min(h), (a.right as usize).min(w));
    let (p0, p1) = (
        (a.plane as usize).min(cpp),
        (a.plane as usize + a.planes.max(1) as usize).min(cpp),
    );
    (t..b.max(t))
        .step_by(a.row_pitch.max(1) as usize)
        .flat_map(move |y| {
            (l..r.max(l))
                .step_by(a.col_pitch.max(1) as usize)
                .flat_map(move |x| (p0..p1.max(p0)).map(move |p| (x, y, p)))
        })
}

fn gain_at(
    points_v: u32,
    points_h: u32,
    spacing: [f64; 2],
    origin: [f64; 2],
    map_planes: u32,
    gains: &[f32],
    rv: f64,
    rh: f64,
    plane: usize,
) -> f32 {
    let (pv, ph) = (points_v as usize, points_h as usize);
    let mp = map_planes as usize;
    let p = plane.min(mp - 1);
    let fy = if spacing[0] > 0.0 {
        ((rv - origin[0]) / spacing[0]).clamp(0.0, (pv - 1) as f64)
    } else {
        0.0
    };
    let fx = if spacing[1] > 0.0 {
        ((rh - origin[1]) / spacing[1]).clamp(0.0, (ph - 1) as f64)
    } else {
        0.0
    };
    let (y0, x0) = (fy.floor() as usize, fx.floor() as usize);
    let (y1, x1) = ((y0 + 1).min(pv - 1), (x0 + 1).min(ph - 1));
    let (ty, tx) = ((fy - y0 as f64) as f32, (fx - x0 as f64) as f32);
    let g = |y: usize, x: usize| gains[(y * ph + x) * mp + p];
    let top = g(y0, x0) + (g(y0, x1) - g(y0, x0)) * tx;
    let bot = g(y1, x0) + (g(y1, x1) - g(y1, x0)) * tx;
    top + (bot - top) * ty
}

/// Radius normaliser: distance from the centre to the farthest image corner.
fn max_radius(cx: f64, cy: f64, w: f64, h: f64) -> f64 {
    [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)]
        .iter()
        .map(|&(x, y)| (x - cx).hypot(y - cy))
        .fold(0.0, f64::max)
        .max(1e-9)
}

/// Apply a list to an interleaved `w × h × cpp` buffer. `cfa` (when the data is a mosaic) is used by the
/// bad-pixel opcodes. `scale` is the value of 1.0 in the buffer: 65535 for list 1 (raw 16-bit values), 1 for
/// lists 2 and 3 (normalised values).
#[allow(clippy::float_cmp)] // Bad-pixel markers are exact integer values in float storage.
pub(super) fn apply_list(
    list: &[Opcode],
    buf: &mut [f32],
    w: usize,
    h: usize,
    cpp: usize,
    cfa: Option<&Cfa>,
    scale: f32,
) -> Result<(), super::DevelopError> {
    for op in list {
        match op {
            Opcode::MapTable { area, table } => {
                let n = table.len();
                for (x, y, p) in area_iter(area, w, h, cpp) {
                    let v = &mut buf[(y * w + x) * cpp + p];
                    let idx = ((*v / scale * 65535.0).round().max(0.0) as usize).min(n - 1);
                    *v = f32::from(table[idx]) / 65535.0 * scale;
                }
            }
            Opcode::MapPolynomial { area, coefficients } => {
                for (x, y, p) in area_iter(area, w, h, cpp) {
                    let v = &mut buf[(y * w + x) * cpp + p];
                    let t = f64::from(*v / scale);
                    let r = coefficients.iter().rev().fold(0.0, |acc, &c| acc * t + c);
                    *v = r as f32 * scale;
                }
            }
            Opcode::GainMap {
                area,
                points_v,
                points_h,
                spacing,
                origin,
                map_planes,
                gains,
            } => {
                for (x, y, p) in area_iter(area, w, h, cpp) {
                    let rv = (y as f64 + 0.5) / h as f64;
                    let rh = (x as f64 + 0.5) / w as f64;
                    let g = gain_at(
                        *points_v,
                        *points_h,
                        *spacing,
                        *origin,
                        *map_planes,
                        gains,
                        rv,
                        rh,
                        p - area.plane as usize,
                    );
                    let value = &mut buf[(y * w + x) * cpp + p];
                    // DNG GainMap ProcessArea bounds the normalized result at 1.
                    *value = (*value * g).min(scale);
                }
            }
            Opcode::DeltaPerRow { area, deltas } | Opcode::DeltaPerColumn { area, deltas } => {
                let per_row = matches!(op, Opcode::DeltaPerRow { .. });
                for (x, y, p) in area_iter(area, w, h, cpp) {
                    let i = if per_row {
                        (y - area.top as usize) / area.row_pitch as usize
                    } else {
                        (x - area.left as usize) / area.col_pitch as usize
                    };
                    if let Some(d) = deltas.get(i) {
                        buf[(y * w + x) * cpp + p] += d * scale;
                    }
                }
            }
            Opcode::ScalePerRow { area, scales } | Opcode::ScalePerColumn { area, scales } => {
                let per_row = matches!(op, Opcode::ScalePerRow { .. });
                for (x, y, p) in area_iter(area, w, h, cpp) {
                    let i = if per_row {
                        (y - area.top as usize) / area.row_pitch as usize
                    } else {
                        (x - area.left as usize) / area.col_pitch as usize
                    };
                    if let Some(s) = scales.get(i) {
                        buf[(y * w + x) * cpp + p] *= s;
                    }
                }
            }
            Opcode::FixVignetteRadial { k, center } => {
                let (cx, cy) = (center[0] * (w - 1) as f64, center[1] * (h - 1) as f64);
                let m = max_radius(cx, cy, (w - 1) as f64, (h - 1) as f64).max(f64::MIN_POSITIVE.sqrt());
                for y in 0..h {
                    for x in 0..w {
                        let r2 = ((x as f64 - cx).powi(2) + (y as f64 - cy).powi(2)) / (m * m);
                        let g = 1.0 + r2 * (k[0] + r2 * (k[1] + r2 * (k[2] + r2 * (k[3] + r2 * k[4]))));
                        for p in 0..cpp {
                            buf[(y * w + x) * cpp + p] *= g as f32;
                        }
                    }
                }
            }
            Opcode::FixBadPixelsConstant { constant, .. } => {
                let marker = *constant as f32;
                let bad = (0..w * h)
                    .map(|i| cpp == 1 && buf[i] == marker)
                    .collect::<Vec<_>>();
                fix_pixels(buf, w, h, cpp, cfa, &bad)?;
            }
            Opcode::FixBadPixelsList { points, rects, .. } => {
                let mut bad = vec![false; w * h];
                for &(y, x) in points {
                    if (x as usize) < w && (y as usize) < h {
                        bad[y as usize * w + x as usize] = true;
                    }
                }
                for r in rects {
                    for y in r[0] as usize..(r[2] as usize).min(h) {
                        for x in r[1] as usize..(r[3] as usize).min(w) {
                            bad[y * w + x] = true;
                        }
                    }
                }
                fix_pixels(buf, w, h, cpp, cfa, &bad)?;
            }
            _ => {}
        }
        for v in buf.iter_mut() {
            *v = v.clamp(0.0, scale);
        }
    }
    Ok(())
}

/// Replace listed pixels with the mean of the nearest same-colour neighbours that are not themselves bad.
fn fix_pixels(
    buf: &mut [f32],
    w: usize,
    h: usize,
    cpp: usize,
    cfa: Option<&Cfa>,
    bad: &[bool],
) -> Result<(), super::DevelopError> {
    if !bad.iter().any(|p| *p) {
        return Ok(());
    }
    let source = buf.to_vec();
    let step = match cfa {
        Some(c) if c.width == 2 && c.height == 2 => 2,
        Some(c) => c.width.max(c.height) as isize,
        None => 1,
    };
    for (i, marked) in bad.iter().enumerate() {
        if !marked {
            continue;
        }
        let (x, y) = (i % w, i / w);
        for p in 0..cpp {
            let mut sum = 0.0_f64;
            let mut n = 0;
            for (dx, dy) in [
                (-step, 0),
                (step, 0),
                (0, -step),
                (0, step),
                (-step, -step),
                (step, step),
                (-step, step),
                (step, -step),
            ] {
                let (nx, ny) = (x as isize + dx, y as isize + dy);
                if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                    continue;
                }
                let j = ny as usize * w + nx as usize;
                if bad[j] {
                    continue;
                }
                sum += f64::from(source[j * cpp + p]);
                n += 1;
            }
            if n == 0 {
                return Err(super::DevelopError::Invalid(
                    "bad pixel has no valid same-phase neighbours",
                ));
            }
            buf[i * cpp + p] = (sum / f64::from(n)) as f32;
        }
    }
    Ok(())
}

/// Apply `OpcodeList3` (on the demosaiced RGB image, values normalised to [0, 1]).
#[cfg(test)]
pub(super) fn apply_list3(
    list: &[Opcode],
    img: &mut Rgb32f,
    cancelled: &impl Fn() -> bool,
) -> Result<(), super::DevelopError> {
    apply_list3_with_aspect(list, img, cancelled, 1.0)
}

pub(super) fn apply_list3_with_aspect(
    list: &[Opcode],
    img: &mut Rgb32f,
    cancelled: &impl Fn() -> bool,
    pixel_aspect: f64,
) -> Result<(), super::DevelopError> {
    if list.is_empty() {
        return Ok(());
    }
    let (w, h) = (img.width, img.height);
    for op in list {
        match op {
            Opcode::WarpRectilinear { planes, center } => {
                if planes.iter().all(|p| *p == [1.0, 0.0, 0.0, 0.0, 0.0, 0.0]) {
                    continue;
                }
                let source = img.data.clone();
                for y in 0..h {
                    if cancelled() {
                        return Err(super::DevelopError::Cancelled);
                    }
                    for x in 0..w {
                        for c in 0..3 {
                            let coefficients = planes[if planes.len() == 1 { 0 } else { c }];
                            let point = rectilinear_source_point_with_aspect(
                                coefficients,
                                *center,
                                [w as u32, h as u32],
                                [x as f64, y as f64],
                                pixel_aspect,
                            )?;
                            img.data[y * w + x][c] = rectilinear_sample(&source, [w, h], point, c)?;
                        }
                    }
                }
            }
            Opcode::WarpFisheye { .. } | Opcode::TrimBounds { .. } | Opcode::Unknown { .. } => {}
            other => {
                let mut flat: Vec<f32> = img.data.iter().flat_map(|p| *p).collect();
                apply_list(std::slice::from_ref(other), &mut flat, w, h, 3, None, 1.0)?;
                for (d, c) in img.data.iter_mut().zip(flat.as_chunks::<3>().0) {
                    *d = [c[0], c[1], c[2]];
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod gain_map_bounds_tests {
    use super::*;
    #[test]
    fn gain_map_clips_padded_extent_and_saturates_normalized_highlights() {
        let map = Opcode::GainMap {
            area: Area {
                top: 0,
                left: 0,
                bottom: 4,
                right: 3,
                plane: 0,
                planes: 1,
                row_pitch: 1,
                col_pitch: 1,
            },
            points_v: 1,
            points_h: 1,
            spacing: [1.0, 1.0],
            origin: [0.0, 0.0],
            map_planes: 1,
            gains: vec![2.0],
        };
        let mut pixels = [0.75, 0.25, 0.9, 0.1];
        apply_list(&[map], &mut pixels, 2, 2, 1, None, 1.0).unwrap();
        assert_eq!(pixels, [1.0, 0.5, 1.0, 0.2]);
    }
}

#[cfg(test)]
mod gain_map_sdk_tests {
    #[test]
    fn spatial_gains_match_independent_dng_sdk_interpolation() {
        let reference = include_bytes!("../../../../tests/fixtures/dng/gain-map-sdk.f32le");
        assert_eq!(reference.len(), 20 * 23 * 4);
        let gains: Vec<f32> = (0..3)
            .flat_map(|y| {
                (0..4).map(move |x| {
                    (1.0 + 0.13 * y as f64 + 0.07 * x as f64 + 0.03 * x as f64 * y as f64) as f32
                })
            })
            .collect();
        let mut maximum = 0.0f32;
        for (i, bytes) in reference.chunks_exact(4).enumerate() {
            let expected = f32::from_le_bytes(bytes.try_into().unwrap());
            let actual = super::gain_at(
                3,
                4,
                [0.3, 0.2],
                [0.1, -0.05],
                1,
                &gains,
                (i / 23) as f64 / 20.0 + 0.5 / 20.0,
                (i % 23) as f64 / 23.0 + 0.5 / 23.0,
                0,
            );
            maximum = maximum.max((actual - expected).abs());
        }
        assert!(maximum <= 3e-7, "gain-map max absolute error {maximum}");
    }
}

/// DNG rectilinear destination-to-source coordinates for square pixels.
/// Coordinates and center are `[x, y]`; radial coefficients precede two tangential coefficients.
/// Image resampling and non-square pixel aspect admission are separate operations.
#[cfg(test)]
pub fn rectilinear_source_point(
    coefficients: [f64; 6],
    center: [f64; 2],
    extent: [u32; 2],
    point: [f64; 2],
) -> Result<[f64; 2], super::DevelopError> {
    rectilinear_source_point_with_aspect(coefficients, center, extent, point, 1.0)
}

/// DNG pixel aspect is horizontal DefaultScale divided by vertical DefaultScale.
pub fn rectilinear_source_point_with_aspect(
    coefficients: [f64; 6],
    center: [f64; 2],
    extent: [u32; 2],
    point: [f64; 2],
    pixel_aspect: f64,
) -> Result<[f64; 2], super::DevelopError> {
    if !pixel_aspect.is_finite()
        || pixel_aspect <= 0.0
        || extent.contains(&0)
        || coefficients
            .iter()
            .chain(center.iter())
            .chain(point.iter())
            .any(|v| !v.is_finite())
        || center.iter().any(|v| !(0.0..=1.0).contains(v))
    {
        return Err(super::DevelopError::Invalid("invalid rectilinear geometry"));
    }
    let w = f64::from(extent[0]);
    let h = f64::from(extent[1]);
    let cx = center[0] * w;
    let cy = center[1] * h;
    let square_h = (h / pixel_aspect).round();
    if !square_h.is_finite() || square_h > f64::from(i32::MAX) {
        return Err(super::DevelopError::Invalid("rectilinear aspect extent overflow"));
    }
    let radius = max_radius(cx, center[1] * square_h, w, square_h);
    let dx = (point[0] - cx) / radius;
    let dy = (point[1] - cy) / radius;
    let scaled_dy = dy / pixel_aspect;
    let r2 = (dx * dx + scaled_dy * scaled_dy).min(1.0);
    let k = coefficients;
    let radial = k[0] + r2 * (k[1] + r2 * (k[2] + r2 * k[3]));
    let tx = k[5] * (r2 + 2.0 * dx * dx) + 2.0 * k[4] * dx * scaled_dy;
    let ty = k[4] * (r2 + 2.0 * scaled_dy * scaled_dy) + 2.0 * k[5] * dx * scaled_dy;
    let result = [
        cx + radius * (dx * radial + tx),
        cy + radius * (dy * radial + ty * pixel_aspect),
    ];
    if result.iter().any(|v| !v.is_finite()) {
        return Err(super::DevelopError::Invalid("rectilinear coordinate overflow"));
    }
    Ok(result)
}

#[cfg(test)]
mod rectilinear_coordinate_tests {
    use super::rectilinear_source_point;
    #[test]
    fn identity_and_fusion_plane_scales_preserve_the_optical_center() {
        for point in [[0.0, 0.0], [1552.0, 1500.0], [3103.0, 2999.0]] {
            assert_eq!(
                rectilinear_source_point([1.0, 0.0, 0.0, 0.0, 0.0, 0.0], [0.5; 2], [3104, 3000], point)
                    .unwrap(),
                point
            );
        }
        let point = rectilinear_source_point(
            [1.00037422, 0.0, 0.0, 0.0, 0.0, 0.0],
            [0.5; 2],
            [3104, 3000],
            [552.0, 500.0],
        )
        .unwrap();
        assert!((point[0] - 551.62578).abs() < 1e-9);
        assert!((point[1] - 499.62578).abs() < 1e-9);
        assert!(rectilinear_source_point([f64::NAN; 6], [0.5; 2], [3104, 3000], [0.0; 2]).is_err());
    }
}

#[cfg(test)]
mod rectilinear_sdk_tests {
    #[test]
    fn coordinates_match_independent_sdk_radial_and_tangential_evaluation() {
        let reference = include_bytes!("../../../../tests/fixtures/dng/rectilinear-sdk.f64le");
        assert_eq!(reference.len(), 300 * 16);
        for (i, pair) in reference.chunks_exact(16).enumerate() {
            let expected = [
                f64::from_le_bytes(pair[..8].try_into().unwrap()),
                f64::from_le_bytes(pair[8..].try_into().unwrap()),
            ];
            let actual = super::rectilinear_source_point(
                [0.97, 0.05, -0.01, 0.002, 0.003, -0.004],
                [0.4, 0.6],
                [40, 30],
                [(i % 20 * 2) as f64, (i / 20 * 2) as f64],
            )
            .unwrap();
            for c in 0..2 {
                assert!((actual[c] - expected[c]).abs() < 1e-11, "point {i} channel {c}");
            }
        }
    }
}

/// DNG warp's 4x4 bicubic weights for quantized 1/32-pixel y/x phases.
pub fn rectilinear_bicubic_weights(phase_y: u8, phase_x: u8) -> Result<[f32; 16], super::DevelopError> {
    if phase_y >= 32 || phase_x >= 32 {
        return Err(super::DevelopError::Invalid("invalid warp phase"));
    }
    fn kernel(x: f64) -> f32 {
        let x = x.abs();
        let a = -0.75;
        let v = if x >= 2.0 {
            0.0
        } else if x >= 1.0 {
            ((a * x - 5.0 * a) * x + 8.0 * a) * x - 4.0 * a
        } else {
            ((a + 2.0) * x - (a + 3.0)) * x * x + 1.0
        };
        v as f32
    }
    let mut weights = [0.0f32; 16];
    let mut sum = 0.0f64;
    for y in 0..4 {
        for x in 0..4 {
            let value = kernel(x as f64 - 1.0 - f64::from(phase_x) / 32.0)
                * kernel(y as f64 - 1.0 - f64::from(phase_y) / 32.0);
            weights[y * 4 + x] = value;
            sum += f64::from(value);
        }
    }
    let normalize = (1.0 / sum) as f32;
    for w in &mut weights {
        *w *= normalize;
    }
    Ok(weights)
}
#[cfg(test)]
mod bicubic_sdk_tests {
    #[test]
    fn all_quantized_warp_phases_match_sdk_weight_bits() {
        let reference = include_bytes!("../../../../tests/fixtures/dng/bicubic-sdk.f32le");
        assert_eq!(reference.len(), 32 * 32 * 16 * 4);
        for y in 0..32 {
            for x in 0..32 {
                let weights = super::rectilinear_bicubic_weights(y, x).unwrap();
                for (i, w) in weights.iter().enumerate() {
                    let offset = ((usize::from(y) * 32 + usize::from(x)) * 16 + i) * 4;
                    assert_eq!(
                        w.to_bits(),
                        u32::from_le_bytes(reference[offset..offset + 4].try_into().unwrap())
                    );
                }
            }
        }
    }
}

static WARP_WEIGHTS: std::sync::LazyLock<[[f32; 16]; 1024]> = std::sync::LazyLock::new(|| {
    std::array::from_fn(|i| {
        rectilinear_bicubic_weights((i / 32) as u8, (i % 32) as u8).expect("bounded phase")
    })
});
/// Sample normalized RGB with the DNG 32-phase bicubic kernel and repeated edge pixels.
pub fn rectilinear_sample(
    source: &[[f32; 3]],
    extent: [usize; 2],
    point: [f64; 2],
    channel: usize,
) -> Result<f32, super::DevelopError> {
    let [w, h] = extent;
    if w == 0
        || h == 0
        || w.checked_mul(h) != Some(source.len())
        || channel >= 3
        || point.iter().any(|p| !p.is_finite())
    {
        return Err(super::DevelopError::Invalid("invalid warp sampling layout"));
    }
    let px = point[0].clamp(-2.0, w as f64 + 1.0);
    let py = point[1].clamp(-2.0, h as f64 + 1.0);
    let floor_x = px.floor();
    let floor_y = py.floor();
    let bx = floor_x as isize;
    let by = floor_y as isize;
    let fx = ((px - floor_x) * 32.0) as usize;
    let fy = ((py - floor_y) * 32.0) as usize;
    let weights = &WARP_WEIGHTS[fy * 32 + fx];
    let columns: [usize; 4] =
        std::array::from_fn(|x| (bx + x as isize - 1).clamp(0, w as isize - 1) as usize);
    let mut total = 0.0f32;
    for y in 0..4 {
        let row = (by + y as isize - 1).clamp(0, h as isize - 1) as usize * w;
        for x in 0..4 {
            total += weights[y * 4 + x] * source[row + columns[x]][channel];
        }
    }
    Ok(total.clamp(0.0, 1.0))
}

#[cfg(test)]
mod warp_sampling_tests {
    use super::*;
    #[test]
    fn warp_identity_edges_and_cancellation_are_bounded() {
        let source = vec![[0.25, 0.5, 0.75]; 16];
        assert_eq!(
            rectilinear_sample(&source, [4, 4], [-100.0, 100.0], 1).unwrap(),
            0.5
        );
        let mut image = super::super::Rgb32f {
            width: 4,
            height: 4,
            data: source.clone(),
        };
        let opcode = Opcode::WarpRectilinear {
            planes: vec![[1.01, 0.0, 0.0, 0.0, 0.0, 0.0]],
            center: [0.5; 2],
        };
        assert!(matches!(
            apply_list3(&[opcode], &mut image, &|| true),
            Err(super::super::DevelopError::Cancelled)
        ));
        assert_eq!(image.data, source);
    }
}

#[cfg(test)]
mod full_warp_sdk_tests {
    #[test]
    fn full_rgb_warp_matches_independent_sdk_image_opcode() {
        let reference = include_bytes!("../../../../tests/fixtures/dng/warp-image-sdk.f32le");
        qualify(reference, vec![[0.97, 0.05, -0.01, 0.002, 0.003, -0.004]], false);
    }

    #[test]
    fn per_channel_warp_matches_independent_sdk_image_opcode() {
        let reference = include_bytes!("../../../../tests/fixtures/dng/warp-three-plane-sdk.f32le");
        let planes = (0..3)
            .map(|p| {
                let p = f64::from(p);
                [
                    0.97 + 0.01 * p,
                    0.05 - 0.02 * p,
                    -0.01,
                    0.002,
                    0.003 + 0.001 * p,
                    -0.004 + 0.002 * p,
                ]
            })
            .collect();
        qualify(reference, planes, false);
    }

    #[test]
    fn hdr_warp_matches_independent_sdk_clipping() {
        let reference = include_bytes!("../../../../tests/fixtures/dng/warp-hdr-sdk.f32le");
        let planes = (0..3)
            .map(|p| {
                let p = f64::from(p);
                [
                    0.97 + 0.01 * p,
                    0.05 - 0.02 * p,
                    -0.01,
                    0.002,
                    0.003 + 0.001 * p,
                    -0.004 + 0.002 * p,
                ]
            })
            .collect();
        qualify(reference, planes, true);
    }

    #[test]
    fn identity_warp_preserves_hdr_bits() {
        let source = vec![[-0.5, 1.25, 8.0]; 4];
        let mut image = super::super::Rgb32f {
            width: 2,
            height: 2,
            data: source.clone(),
        };
        let op = super::Opcode::WarpRectilinear {
            planes: vec![[1.0, 0.0, 0.0, 0.0, 0.0, 0.0]; 3],
            center: [0.5; 2],
        };
        super::apply_list3(&[op], &mut image, &|| false).unwrap();
        for (a, b) in image.data.iter().flatten().zip(source.iter().flatten()) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }

    fn qualify(reference: &[u8], planes: Vec<[f64; 6]>, hdr: bool) {
        assert_eq!(reference.len(), 32 * 48 * 3 * 4);
        let data: Vec<[f32; 3]> = (0..32)
            .flat_map(|y| {
                (0..48).map(move |x| {
                    std::array::from_fn(|c| (((x * 17 + y * 31 + c * 43) % 251) as f64 / 250.0) as f32)
                })
            })
            .collect();
        let data = if hdr {
            data.into_iter()
                .map(|p: [f32; 3]| p.map(|v| v * 4.0 - 0.5))
                .collect()
        } else {
            data
        };
        let mut image = super::super::Rgb32f {
            width: 48,
            height: 32,
            data,
        };
        let op = super::Opcode::WarpRectilinear {
            planes,
            center: [0.4, 0.6],
        };
        super::apply_list3(&[op], &mut image, &|| false).unwrap();
        let mut maximum = 0.0f32;
        for (actual, b) in image.data.iter().flatten().zip(reference.chunks_exact(4)) {
            let expected = f32::from_le_bytes(b.try_into().unwrap());
            maximum = maximum.max((actual - expected).abs());
        }
        assert!(maximum <= 1e-6, "full SDK warp maximum error {maximum}");
    }
}

#[cfg(test)]
mod parameter_boundary_tests {
    use super::*;

    #[test]
    fn rectilinear_parameters_require_exact_consumption() {
        let valid = write_list(&[Opcode::WarpRectilinear {
            planes: vec![[1.0, 0.0, 0.0, 0.0, 0.0, 0.0]],
            center: [0.5; 2],
        }]);
        assert_eq!(parse_list(&valid).unwrap().len(), 1);
        let mut extra = valid.clone();
        let size = u32::from_be_bytes(extra[16..20].try_into().unwrap());
        extra[16..20].copy_from_slice(&(size + 1).to_be_bytes());
        extra.push(0);
        assert!(parse_list(&extra).is_err());
        for end in 0..valid.len() {
            assert!(parse_list(&valid[..end]).is_err(), "truncation at {end}");
        }
    }

    #[test]
    fn oversized_gain_dimensions_are_rejected_before_collecting() {
        let mut parameters = vec![0u8; 32]; // AreaSpec
        parameters.extend_from_slice(&u32::MAX.to_be_bytes());
        parameters.extend_from_slice(&u32::MAX.to_be_bytes());
        for value in [1.0f64, 1.0, 0.0, 0.0] {
            parameters.extend_from_slice(&value.to_be_bytes());
        }
        parameters.extend_from_slice(&u32::MAX.to_be_bytes());
        let mut blob = Vec::new();
        for value in [1u32, 9, 0x0103_0000, 0, parameters.len() as u32] {
            blob.extend_from_slice(&value.to_be_bytes());
        }
        blob.extend_from_slice(&parameters);
        assert!(parse_list(&blob).is_err());
    }
}

#[cfg(test)]
mod warp_timing_tests {
    #[test]
    #[ignore = "manual fixed-work warp sampling benchmark"]
    fn sampling_fixed_work_timing() {
        let source: Vec<[f32; 3]> = (0..256 * 256)
            .map(|i| [((i * 17) % 251) as f32 / 250.0; 3])
            .collect();
        let mut checksum = 0.0f64;
        for run in 0..6 {
            let start = std::time::Instant::now();
            let mut sum = 0.0f64;
            for i in 0..1_000_000 {
                let point = [
                    (i % 260) as f64 - 2.0 + 0.37,
                    ((i / 260) % 260) as f64 - 2.0 + 0.63,
                ];
                sum += f64::from(
                    super::rectilinear_sample(
                        std::hint::black_box(&source),
                        [256, 256],
                        std::hint::black_box(point),
                        i % 3,
                    )
                    .unwrap(),
                );
            }
            checksum = sum;
            eprintln!(
                "run={run} ms={:.3} checksum={sum:.9}",
                start.elapsed().as_secs_f64() * 1000.0
            );
        }
        assert!(checksum > 0.0);
    }
}

#[cfg(test)]
mod aspect_warp_sdk_tests {
    #[test]
    fn non_square_rgb_warp_matches_complete_sdk_image() {
        let reference = include_bytes!("../../../../tests/fixtures/dng/warp-aspect-sdk.f32le");
        let source: Vec<[f32; 3]> = (0..32)
            .flat_map(|y| {
                (0..48).map(move |x| {
                    std::array::from_fn(|c| (((x * 17 + y * 31 + c * 43) % 251) as f64 / 250.0) as f32)
                })
            })
            .collect();
        let planes = (0..3)
            .map(|c| {
                let p = f64::from(c);
                [
                    0.97 + 0.01 * p,
                    0.05 - 0.02 * p,
                    -0.01,
                    0.002,
                    0.003 + 0.001 * p,
                    -0.004 + 0.002 * p,
                ]
            })
            .collect();
        let mut image = super::super::Rgb32f {
            width: 48,
            height: 32,
            data: source.clone(),
        };
        super::apply_list3_with_aspect(
            &[super::Opcode::WarpRectilinear {
                planes,
                center: [0.4, 0.6],
            }],
            &mut image,
            &|| false,
            1.5,
        )
        .unwrap();
        for (actual, bytes) in image.data.iter().flatten().zip(reference.chunks_exact(4)) {
            assert!((*actual - f32::from_le_bytes(bytes.try_into().unwrap())).abs() <= 1e-6);
        }
        let mut maximum = 0.0f32;
        for y in 0..32 {
            for x in 0..48 {
                for c in 0..3 {
                    let p = c as f64;
                    let k = [
                        0.97 + 0.01 * p,
                        0.05 - 0.02 * p,
                        -0.01,
                        0.002,
                        0.003 + 0.001 * p,
                        -0.004 + 0.002 * p,
                    ];
                    let point = super::rectilinear_source_point_with_aspect(
                        k,
                        [0.4, 0.6],
                        [48, 32],
                        [x as f64, y as f64],
                        1.5,
                    )
                    .unwrap();
                    let actual = super::rectilinear_sample(&source, [48, 32], point, c).unwrap();
                    let offset = ((y * 48 + x) * 3 + c) * 4;
                    let expected = f32::from_le_bytes(reference[offset..offset + 4].try_into().unwrap());
                    maximum = maximum.max((actual - expected).abs());
                }
            }
        }
        assert!(maximum <= 1e-6, "SDK non-square warp error {maximum}");
        for aspect in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(
                super::rectilinear_source_point_with_aspect(
                    [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    [0.5; 2],
                    [48, 32],
                    [1.0; 2],
                    aspect
                )
                .is_err()
            );
        }
    }
}
