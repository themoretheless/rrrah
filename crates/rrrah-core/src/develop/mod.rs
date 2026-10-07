//! CPU quality development, adapted from Apache-2.0 storytold/lightcraft.
//! Sensor samples stay immutable; operations use separate float scratch.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]
mod ahd;
mod curve;
mod highlight;
mod opcodes;
mod xtrans;
use crate::{CfaPattern, DecodedMosaic, DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
pub use curve::MonotoneCurve;
pub use opcodes::{Area, Opcode, OpcodeLists, parse_list as parse_opcode_list};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DevelopError {
    #[error("invalid RAW development input: {0}")]
    Invalid(&'static str),
    #[error("RAW development cancelled")]
    Cancelled,
    #[error(transparent)]
    Frame(#[from] crate::FrameError),
    #[error(transparent)]
    Buffer(#[from] crate::BufferError),
    #[error(transparent)]
    Raster(#[from] crate::RasterError),
    #[error(transparent)]
    Rgbe(#[from] crate::rgbe::CalibrationError),
}
#[derive(Debug, Clone, PartialEq)]
pub struct DevelopOptions {
    pub recover_highlights: bool,
    /// Display-space luminance curve, evaluated by the renderer after exposure
    /// and tone mapping. `develop_raw` retains scene-linear output.
    pub curve: MonotoneCurve,
}
impl Default for DevelopOptions {
    fn default() -> Self {
        Self {
            recover_highlights: true,
            curve: MonotoneCurve::identity(),
        }
    }
}
#[derive(Debug)]
struct Rgb32f {
    width: usize,
    height: usize,
    data: Vec<[f32; 3]>,
}
impl Rgb32f {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![[0.0; 3]; width * height],
        }
    }
}
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
    data: &'a [f32],
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
    fn get(&self, x: isize, y: isize) -> f32 {
        self.data[reflect(y, self.h) * self.w + reflect(x, self.w)]
    }
    fn color(&self, x: isize, y: isize) -> u8 {
        self.cfa.color_at(reflect(x, self.w), reflect(y, self.h))
    }
}
fn par_rows<T>(data: &mut [T], width: usize, mut f: impl FnMut(usize, &mut [T])) {
    for (y, row) in data.chunks_mut(width).enumerate() {
        f(y, row);
    }
}
fn quality_cfa(pattern: &CfaPattern) -> Result<Cfa, DevelopError> {
    pattern.validate()?;
    let bayer = pattern.bayer_quad().is_ok();
    if !bayer
        && (pattern.width != 6
            || pattern.height != 6
            || pattern.cells.iter().any(|c| *c as u8 > 2)
            || [0, 1, 2]
                .into_iter()
                .zip([8, 20, 8])
                .any(|(c, n)| pattern.cells.iter().filter(|p| **p as u8 == c).count() != n))
    {
        return Err(DevelopError::Invalid(
            "quality development requires RGB Bayer or 6x6 X-Trans",
        ));
    }
    Ok(Cfa {
        width: pattern.width as usize,
        height: pattern.height as usize,
        cells: pattern.cells.iter().map(|c| *c as u8).collect(),
    })
}

/// Develop full sensor data, then crop/orient. Admission covers output plus a
/// conservative scratch bound (not allocator/driver overhead). Cancellation
/// is observed between stages and at rows/tiles of the expensive kernels.
/// RGBE uses calibrated four-plane bilinear reconstruction; Bayer highlight
/// recovery does not apply to this path, and nonempty RGBE opcodes are rejected.
pub fn develop_raw(
    mosaic: &DecodedMosaic,
    options: &DevelopOptions,
    lists: &OpcodeLists,
    budget: Option<&MemoryBudget>,
    cancelled: &impl Fn() -> bool,
) -> Result<DecodedRaster, DevelopError> {
    if cancelled() {
        return Err(DevelopError::Cancelled);
    }
    let md = &mosaic.metadata;
    md.validate()?;
    if md.photometric != crate::Photometric::Cfa {
        return Err(DevelopError::Invalid(
            "quality development requires CFA photometric",
        ));
    }
    if md.components_per_pixel != 1 {
        return Err(DevelopError::Invalid("only single-plane CFA is supported"));
    }
    if md.cfa.as_ref().is_some_and(|cfa| cfa.rgbe_quad().is_ok()) {
        return develop_rgbe(mosaic, lists, budget, cancelled);
    }
    let cfa = quality_cfa(md.cfa.as_ref().ok_or(DevelopError::Invalid("missing CFA"))?)?;
    opcodes::validate_lists(lists)?;
    let matrix = crate::camera_to_linear_srgb(md.xyz_to_camera)
        .ok_or(DevelopError::Invalid("singular camera transform"))?;
    let (w, h) = (md.width as usize, md.height as usize);
    let n = w
        .checked_mul(h)
        .ok_or(DevelopError::Invalid("dimension overflow"))?;
    if n > 128 * 1024 * 1024 {
        return Err(DevelopError::Invalid("quality sensor exceeds 128 Mi samples"));
    }
    if mosaic.pixels.len() != n {
        return Err(DevelopError::Invalid("sensor sample count mismatch"));
    }
    for op in &lists.list1 {
        let phase = match op {
            Opcode::FixBadPixelsConstant { bayer_phase, .. }
            | Opcode::FixBadPixelsList { bayer_phase, .. } => *bayer_phase,
            _ => unreachable!("validated stage 1"),
        };
        let q = md
            .cfa
            .as_ref()
            .and_then(|c| c.bayer_quad().ok())
            .ok_or(DevelopError::Invalid("bad-pixel opcodes require Bayer"))?;
        let red = q
            .iter()
            .position(|c| *c == 0)
            .ok_or(DevelopError::Invalid("missing Bayer red phase"))? as u32;
        if phase != red {
            return Err(DevelopError::Invalid("bad-pixel BayerPhase disagrees with CFA"));
        }
        if let Opcode::FixBadPixelsList { points, rects, .. } = op {
            if points.iter().any(|(y, x)| *y >= md.height || *x >= md.width)
                || rects
                    .iter()
                    .any(|r| r[0] >= r[2] || r[1] >= r[3] || r[2] > md.height || r[3] > md.width)
            {
                return Err(DevelopError::Invalid("bad-pixel area outside sensor"));
            }
        }
    }
    // DNG GainMap AreaSpec is intersected with the destination image.
    // Padded codec extents may legitimately extend beyond the sensor (GoPro).
    // The opcode kernel already clips its area iterator to image bounds.
    if (!lists.list2.is_empty() || !lists.list3.is_empty())
        && md
            .active_area
            .is_some_and(|r| r != crate::Rect::full(md.width, md.height))
    {
        return Err(DevelopError::Invalid(
            "opcode coordinate domains currently require full-sensor ActiveArea",
        ));
    }
    // Peak full-resolution storage: samples (4), clipping (3), RGB (12),
    // and output RGBA (16) = 35 bytes/sample. Other stages peak lower:
    // X-Trans adds green (4); list3 adds flattened RGB (12); highlight
    // reconstruction adds 13 bytes per coarse pyramid pixel. The sum of
    // ceil-halved pyramid areas is bounded by n plus logarithmic edge terms,
    // including one-dimensional sensors. The 8 MiB margin covers these terms,
    // sequential 140x140 AHD tile buffers, and bounded CFA neighbour tables.
    // Sensor input and caller-owned opcode metadata have separate ownership.
    let bytes = (n as u64)
        .checked_mul(35)
        .and_then(|n| n.checked_add(8 * 1024 * 1024))
        .ok_or(DevelopError::Invalid("scratch size overflow"))?;
    let scratch_budget = budget.cloned().unwrap_or_else(|| MemoryBudget::new(bytes));
    let output_bytes = u64::from(md.display_dimensions().0) * u64::from(md.display_dimensions().1) * 16;
    let scratch_credit = scratch_budget.try_reserve(bytes - output_bytes)?;
    let output_credit = scratch_budget.try_reserve(output_bytes)?;
    let mut samples: Vec<f32> = mosaic.pixels.iter().map(|p| f32::from(*p)).collect();
    opcodes::apply_list(&lists.list1, &mut samples, w, h, 1, Some(&cfa), 65535.0)?;
    let quad = md.cfa.as_ref().and_then(|c| c.bayer_quad().ok());
    let first_green = quad.map(|q| usize::from(q[0] != 1));
    for y in 0..h {
        if cancelled() {
            return Err(DevelopError::Cancelled);
        }
        for x in 0..w {
            let c = cfa.color_at(x, y) as usize;
            let black = md
                .black_level
                .at(y as u32, x as u32, 0)
                .ok_or(DevelopError::Invalid("invalid black grid"))?;
            let white = match md.white_level.0.len() {
                1 => md.white_level.0[0],
                3 => md.white_level.0[c],
                4 if quad.is_some() => md.white_level.0[(y % 2) * 2 + x % 2],
                _ => return Err(DevelopError::Invalid("unsupported white grid")),
            };
            if white <= black {
                return Err(DevelopError::Invalid("white level must exceed black"));
            }
            samples[y * w + x] = (samples[y * w + x] - black).max(0.0) / (white - black);
        }
    }
    // Capture clipping in sensor units before gain maps or WB. Corrected HDR
    // above one is not evidence that the sensor clipped.
    let mut clipped = vec![[false; 3]; n];
    for y in 0..h {
        if cancelled() {
            return Err(DevelopError::Cancelled);
        }
        for x in 0..w {
            for dy in -2..=2 {
                for dx in -2..=2 {
                    let sx = reflect(x as isize + dx, w);
                    let sy = reflect(y as isize + dy, h);
                    let c = cfa.color_at(sx, sy) as usize;
                    clipped[y * w + x][c] |= samples[sy * w + sx] >= 0.999;
                }
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            clipped[y * w + x][cfa.color_at(x, y) as usize] = samples[y * w + x] >= 0.999;
        }
    }
    opcodes::apply_list(&lists.list2, &mut samples, w, h, 1, Some(&cfa), 1.0)?;
    // Per-photosite WB, including G2, before demosaic. Do not clamp HDR.
    for y in 0..h {
        for x in 0..w {
            let c = cfa.color_at(x, y) as usize;
            let gain = if c == 1 && first_green.is_some_and(|g| g != (y % 2) * 2 + x % 2) {
                md.white_balance[3]
            } else {
                md.white_balance[c]
            };
            samples[y * w + x] *= gain;
        }
    }
    if samples.iter().any(|p| !p.is_finite()) {
        return Err(DevelopError::Invalid("non-finite corrected sensor samples"));
    }
    let source = Mosaic {
        w,
        h,
        data: &samples,
        cfa: &cfa,
    };
    let mut rgb = if quad.is_some() {
        ahd::ahd(&source, cancelled)?
    } else {
        xtrans::directional(&source, cancelled)?
    };
    // Corrected sensor samples are no longer needed after demosaicing.
    drop(samples);
    if cancelled() {
        return Err(DevelopError::Cancelled);
    }
    if !lists.list3.is_empty() {
        let wb = [md.white_balance[0], md.white_balance[1], md.white_balance[2]];
        for p in &mut rgb.data {
            for c in 0..3 {
                p[c] /= wb[c];
            }
        }
        opcodes::apply_list3(&lists.list3, &mut rgb, cancelled)?;
        for p in &mut rgb.data {
            for c in 0..3 {
                p[c] *= wb[c];
            }
        }
    }
    if options.recover_highlights {
        highlight::reconstruct(
            &mut rgb,
            [
                md.white_balance[0],
                md.white_balance[1].min(md.white_balance[3]),
                md.white_balance[2],
            ],
            0.999,
            &clipped,
        );
    }
    // The output conversion needs RGB only, not the sensor clipping mask.
    drop(clipped);
    if cancelled() {
        return Err(DevelopError::Cancelled);
    }
    let crop = md.effective_crop();
    let (ow, oh) = md.display_dimensions();
    let output_bytes = u64::from(ow) * u64::from(oh) * 16;
    // Output is already credited inside scratch; transfer credit at the end.
    let mut output = Vec::with_capacity((output_bytes / 4) as usize);
    for y in 0..oh {
        if cancelled() {
            return Err(DevelopError::Cancelled);
        }
        for x in 0..ow {
            let uv = md
                .orientation
                .map_display_uv([(x as f32 + 0.5) / ow as f32, (y as f32 + 0.5) / oh as f32]);
            let sx = crop.x + (uv[0] * crop.width as f32).floor().min((crop.width - 1) as f32) as u32;
            let sy = crop.y + (uv[1] * crop.height as f32).floor().min((crop.height - 1) as f32) as u32;
            let linear = crate::apply_3x3(matrix, rgb.data[sy as usize * w + sx as usize]);
            if linear.iter().any(|p| !p.is_finite()) {
                return Err(DevelopError::Invalid("non-finite developed RGB"));
            }
            output.extend_from_slice(&[linear[0], linear[1], linear[2], 1.0]);
        }
    }
    drop(rgb);
    drop(scratch_credit);
    let pixels: rrrah_memory::PixelBuffer<f32> = output_credit.try_adopt(output)?.into();
    Ok(DecodedRaster::new(
        ow,
        oh,
        RasterPixels::Rgba32Float(pixels),
        RasterColorSpace::LinearSrgb,
    )?)
}

/// Four distinct planes retain their own calibration. This path uses the shared
/// bilinear RGBE reference reconstruction, without Bayer highlight recovery.
/// DNG opcode processing for RGBE is not qualified and is rejected explicitly.
fn develop_rgbe(
    mosaic: &DecodedMosaic,
    lists: &OpcodeLists,
    budget: Option<&MemoryBudget>,
    cancelled: &impl Fn() -> bool,
) -> Result<DecodedRaster, DevelopError> {
    if !lists.list1.is_empty() || !lists.list2.is_empty() || !lists.list3.is_empty() {
        return Err(DevelopError::Invalid("RGBE development opcodes are unsupported"));
    }
    let calibration = crate::rgbe::Calibration::new(&mosaic.metadata)?;
    let md = &mosaic.metadata;
    let n = u64::from(md.width) * u64::from(md.height);
    if n > 128 * 1024 * 1024 || mosaic.pixels.len() as u64 != n {
        return Err(DevelopError::Invalid("invalid RGBE sensor sample count"));
    }
    let (ow, oh) = md.display_dimensions();
    let bytes = u64::from(ow) * u64::from(oh) * 16;
    let managed = budget.cloned().unwrap_or_else(|| MemoryBudget::new(bytes));
    let credit = managed.try_reserve(bytes)?;
    let mut output = Vec::with_capacity((bytes / 4) as usize);
    let crop = md.effective_crop();
    for y in 0..oh {
        if cancelled() {
            return Err(DevelopError::Cancelled);
        }
        for x in 0..ow {
            let uv = md
                .orientation
                .map_display_uv([(x as f32 + 0.5) / ow as f32, (y as f32 + 0.5) / oh as f32]);
            let sx = crop.x + (uv[0] * crop.width as f32).floor().min((crop.width - 1) as f32) as u32;
            let sy = crop.y + (uv[1] * crop.height as f32).floor().min((crop.height - 1) as f32) as u32;
            let rgb = calibration
                .reconstruct_linear_rgb([sx, sy], |px, py| {
                    mosaic.pixels[py as usize * md.width as usize + px as usize]
                })
                .ok_or(DevelopError::Invalid("non-finite RGBE reconstruction"))?;
            let rgb = rgb.map(|v| v as f32);
            if rgb.iter().any(|v| !v.is_finite()) {
                return Err(DevelopError::Invalid("non-finite developed RGBE"));
            }
            output.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 1.0]);
        }
    }
    if cancelled() {
        return Err(DevelopError::Cancelled);
    }
    let pixels: rrrah_memory::PixelBuffer<f32> = credit.try_adopt(output)?.into();
    Ok(DecodedRaster::new(
        ow,
        oh,
        RasterPixels::Rgba32Float(pixels),
        RasterColorSpace::LinearSrgb,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bayer() -> Cfa {
        Cfa {
            width: 2,
            height: 2,
            cells: vec![0, 1, 1, 2],
        }
    }
    fn xtrans() -> Cfa {
        Cfa {
            width: 6,
            height: 6,
            cells: vec![
                1, 2, 1, 1, 0, 1, 0, 1, 0, 2, 1, 2, 1, 2, 1, 1, 0, 1, 1, 0, 1, 1, 2, 1, 2, 1, 2, 0, 1, 0, 1,
                0, 1, 1, 2, 1,
            ],
        }
    }
    #[test]
    fn constant_color_and_measured_sites_survive_quality_demosaic() {
        for cfa in [bayer(), xtrans()] {
            // Cross both AHD tile boundaries and exercise X-Trans borders.
            let (w, h) = (139, 133);
            let color = [0.23, 0.41, 0.67];
            let data = (0..w * h)
                .map(|i| color[cfa.color_at(i % w, i / w) as usize])
                .collect::<Vec<_>>();
            let m = Mosaic {
                w,
                h,
                data: &data,
                cfa: &cfa,
            };
            let out = if cfa.width == 2 {
                ahd::ahd(&m, &|| false)
            } else {
                xtrans::directional(&m, &|| false)
            }
            .unwrap();
            for (i, p) in out.data.iter().enumerate() {
                for c in 0..3 {
                    assert!((p[c] - color[c]).abs() < 2e-6, "pixel {i}, channel {c}: {p:?}");
                }
                let own = cfa.color_at(i % w, i / w) as usize;
                assert_eq!(p[own].to_bits(), data[i].to_bits());
            }
        }
    }
    #[test]
    fn quality_cancellation_is_checked_inside_tiles_and_rows() {
        let (w, h) = (260, 150);
        let data = vec![0.5; w * h];
        for cfa in [bayer(), xtrans()] {
            let calls = std::cell::Cell::new(0);
            let cancel = || {
                calls.set(calls.get() + 1);
                calls.get() > 2
            };
            let m = Mosaic {
                w,
                h,
                data: &data,
                cfa: &cfa,
            };
            let out = if cfa.width == 2 {
                ahd::ahd(&m, &cancel)
            } else {
                xtrans::directional(&m, &cancel)
            };
            assert!(matches!(out, Err(DevelopError::Cancelled)));
        }
    }
    #[test]
    fn spatial_highlights_preserve_unclipped_channels_and_use_nearby_color() {
        let mut img = Rgb32f {
            width: 9,
            height: 9,
            data: vec![[0.8, 0.4, 0.2]; 81],
        };
        img.data[40] = [1.0, 0.75, 0.375];
        let mut masks = vec![[false; 3]; 81];
        masks[40][0] = true;
        let original = img.data.clone();
        assert_eq!(highlight::reconstruct(&mut img, [1.0; 3], 1.0, &masks), 1);
        assert!((img.data[40][0] - 1.5).abs() < 1e-5);
        assert_eq!(&img.data[40][1..], &original[40][1..]);
        for i in 0..81 {
            if i != 40 {
                assert_eq!(img.data[i], original[i]);
            }
        }
        // Bright corrected HDR, with no sensor clipping, must remain exact.
        masks.fill([false; 3]);
        img.data[40] = [4.0, 2.0, 1.5];
        let before = img.data.clone();
        assert_eq!(highlight::reconstruct(&mut img, [1.0; 3], 1.0, &masks), 0);
        assert_eq!(img.data, before);
    }
    #[test]
    fn fully_clipped_image_stays_finite_and_neutral_after_wb() {
        let wb = [2.0, 1.0, 1.5];
        let mut img = Rgb32f {
            width: 3,
            height: 3,
            data: vec![[2.0, 1.0, 1.5]; 9],
        };
        highlight::reconstruct(&mut img, wb, 1.0, &[[true; 3]; 9]);
        for p in img.data {
            assert_eq!(p, [2.0; 3]);
        }
    }
    #[test]
    fn monotone_curves_have_no_overshoot_and_reject_bad_points() {
        let pts = [(0.0, 0.0), (0.001, 0.4), (0.4, 0.5), (0.6, 0.5), (1.0, 1.0)];
        let c = MonotoneCurve::new(&pts).unwrap();
        for (x, y) in pts {
            assert!((c.eval(x) - y).abs() < 1e-12);
        }
        let mut last = 0.0;
        for i in 0..=10000 {
            let x = f64::from(i) / 10000.0;
            let y = c.eval(x);
            assert!(y >= last - 1e-12 && y <= 1.0);
            last = y;
            if (0.4..=0.6).contains(&x) {
                assert!((y - 0.5).abs() < 1e-12);
            }
        }
        for p in [
            vec![],
            vec![(0.0, 0.0), (0.0, 1.0)],
            vec![(0.0, 1.0), (1.0, 0.0)],
            vec![(0.0, 0.0), (f64::NAN, 1.0)],
        ] {
            assert!(MonotoneCurve::new(&p).is_err());
        }
        assert_eq!(c.eval(f64::NEG_INFINITY), 0.0);
        assert_eq!(c.eval(f64::INFINITY), 1.0);
        assert!(c.eval(f64::NAN).is_nan());
    }
    fn gain() -> Opcode {
        Opcode::GainMap {
            area: Area {
                top: 0,
                left: 0,
                bottom: 4,
                right: 4,
                plane: 0,
                planes: 1,
                row_pitch: 1,
                col_pitch: 1,
            },
            points_v: 2,
            points_h: 2,
            spacing: [0.5; 2],
            origin: [0.0; 2],
            map_planes: 1,
            gains: vec![1.0, 2.0, 3.0, 4.0],
        }
    }
    #[test]
    fn opcode_wire_is_strict_and_gain_interpolates() {
        let encoded = opcodes::write_list(&[gain()]);
        let parsed = parse_opcode_list(&encoded).unwrap();
        for n in 0..encoded.len() {
            assert!(parse_opcode_list(&encoded[..n]).is_err(), "length {n}");
        }
        let mut giant = encoded.clone();
        // GainMap points at parameter byte 32/36; multiplication must never panic.
        giant[52..56].copy_from_slice(&u32::MAX.to_be_bytes());
        giant[56..60].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(parse_opcode_list(&giant).is_err());
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(parse_opcode_list(&trailing).is_err());
        let mut required = encoded.clone();
        required[4..8].copy_from_slice(&999_u32.to_be_bytes());
        assert!(parse_opcode_list(&required).is_err());
        required[12..16].copy_from_slice(&1_u32.to_be_bytes());
        assert!(parse_opcode_list(&required).unwrap().is_empty());
        let lists = OpcodeLists {
            list2: parsed.clone(),
            ..Default::default()
        };
        opcodes::validate_lists(&lists).unwrap();
        let mut data = vec![0.5; 16];
        opcodes::apply_list(&parsed, &mut data, 4, 4, 1, None, 1.0).unwrap();
        assert_eq!(data[0], 0.875);
        assert_eq!(data[5], 1.0);
        assert_eq!(data[10], 1.0);
        let mut bad = lists.clone();
        if let Opcode::GainMap { gains, .. } = &mut bad.list2[0] {
            gains[0] = f32::NAN;
        }
        assert!(opcodes::validate_lists(&bad).is_err());
        assert!(
            opcodes::validate_lists(&OpcodeLists {
                list1: parsed,
                ..Default::default()
            })
            .is_err()
        );
    }
    #[test]
    fn bad_pixels_use_same_phase_and_radial_vignette_is_spatial() {
        let mut samples = (0..64)
            .map(|i| [100.0, 200.0, 200.0, 300.0][(i / 8 % 2) * 2 + i % 2])
            .collect::<Vec<_>>();
        samples[18] = 0.0;
        let ops = [Opcode::FixBadPixelsConstant {
            constant: 0,
            bayer_phase: 0,
        }];
        opcodes::apply_list(&ops, &mut samples, 8, 8, 1, Some(&bayer()), 65535.0).unwrap();
        assert_eq!(samples[18], 100.0);
        let mut img = Rgb32f {
            width: 5,
            height: 5,
            data: vec![[0.5; 3]; 25],
        };
        opcodes::apply_list3(
            &[Opcode::FixVignetteRadial {
                k: [1.0, 0.0, 0.0, 0.0, 0.0],
                center: [0.5; 2],
            }],
            &mut img,
            &|| false,
        )
        .unwrap();
        assert_eq!(img.data[12], [0.5; 3]);
        assert!(img.data[0][0] > 0.5);
        assert_eq!(img.data[0], img.data[24]);
    }
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;
    fn sensor() -> DecodedMosaic {
        use crate::{CfaColor, LevelGrid, Orientation, Photometric, RawMetadata, WhiteLevel};
        let md = RawMetadata {
            make: "synthetic".into(),
            model: "quality".into(),
            width: 8,
            height: 6,
            components_per_pixel: 1,
            bits_per_sample: 16,
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
            white_balance: [2.0, 1.0, 1.5, 0.5],
            xyz_to_camera: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]],
            active_area: None,
            crop_area: None,
            orientation: Orientation::Normal,
        };
        let data = (0..48)
            .map(|i| [200, 400, 800, 300][(i / 8 % 2) * 2 + i % 2])
            .collect::<Vec<_>>();
        DecodedMosaic::new(md, std::sync::Arc::new(data)).unwrap()
    }
    #[test]
    fn rgbe_full_development_preserves_emerald_color_crop_and_budget() {
        use crate::CfaColor;
        let mut m = sensor();
        m.metadata.cfa.as_mut().unwrap().cells =
            vec![CfaColor::Emerald, CfaColor::Red, CfaColor::Blue, CfaColor::Green];
        m.metadata.white_balance = [1.; 4];
        m.metadata.xyz_to_camera = [
            [0.7924, -0.1910, -0.0777],
            [-0.8226, 1.5459, 0.2998],
            [-0.1517, 0.2199, 0.6818],
            [-0.7242, 1.1401, 0.3481],
        ];
        m.pixels = std::sync::Arc::new(
            (0..48)
                .map(|i| [1600_u16, 200, 300, 400][(i / 8 % 2) * 2 + i % 2])
                .collect::<Vec<_>>(),
        )
        .into();
        m.metadata.crop_area = Some(crate::Rect::new(1, 1, 6, 4));
        m.metadata.orientation = crate::Orientation::Rotate90;
        // Independent LibRaw F828 rgb_cam[3][4], with a nonzero Emerald plane.
        let matrix = [
            [1.63739419, -0.252761811, -0.003541585058, -0.38109079],
            [0.06718456, 0.8223665357, -0.5305757523, 0.6410246491],
            [-0.0008971288335, -0.3551472425, 1.415327907, -0.05928355828],
        ];
        let expected = matrix.map(|row| {
            row.into_iter()
                .zip([0.2, 0.4, 0.3, 1.6])
                .map(|(a, b)| a * b)
                .sum::<f64>()
        });
        let budget = MemoryBudget::new(4 * 6 * 16);
        let out = develop_raw(
            &m,
            &DevelopOptions::default(),
            &OpcodeLists::default(),
            Some(&budget),
            &|| false,
        )
        .unwrap();
        assert_eq!((out.width(), out.height()), (4, 6));
        let RasterPixels::Rgba32Float(pixels) = out.pixels() else {
            panic!()
        };
        for pixel in pixels.chunks_exact(4) {
            for c in 0..3 {
                assert!((f64::from(pixel[c]) - expected[c]).abs() < 2e-6);
            }
            assert_eq!(pixel[3], 1.);
        }
        assert!(pixels.iter().any(|v| *v < 0.));
        assert!(pixels.iter().any(|v| *v > 1.));
        assert_eq!(budget.peak(), 4 * 6 * 16);
        drop(out);
        assert_eq!(budget.used(), 0);
        let too_small = MemoryBudget::new(4 * 6 * 16 - 1);
        assert!(matches!(
            develop_raw(
                &m,
                &DevelopOptions::default(),
                &OpcodeLists::default(),
                Some(&too_small),
                &|| false
            ),
            Err(DevelopError::Buffer(_))
        ));
        assert_eq!(too_small.used(), 0);
        let checkpoints = std::cell::Cell::new(0);
        assert!(matches!(
            develop_raw(
                &m,
                &DevelopOptions::default(),
                &OpcodeLists::default(),
                Some(&budget),
                &|| {
                    checkpoints.set(checkpoints.get() + 1);
                    checkpoints.get() == 3
                }
            ),
            Err(DevelopError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
        let ops = OpcodeLists {
            list3: vec![Opcode::FixVignetteRadial {
                k: [0.; 5],
                center: [0.5; 2],
            }],
            ..Default::default()
        };
        assert!(matches!(
            develop_raw(&m, &DevelopOptions::default(), &ops, Some(&budget), &|| false),
            Err(DevelopError::Invalid(_))
        ));
    }
    #[test]
    fn scene_linear_output_retains_g2_hdr_and_managed_ownership() {
        let mut m = sensor();
        let original = m.pixels.to_vec();
        // Nontrivial orientation/crop are applied after full-sensor demosaic.
        m.metadata.orientation = crate::Orientation::Rotate90;
        m.metadata.crop_area = Some(crate::Rect::new(1, 1, 6, 4));
        let budget = MemoryBudget::new(8 * 1024 * 1024 + 48 * 35);
        let out = develop_raw(
            &m,
            &DevelopOptions::default(),
            &OpcodeLists::default(),
            Some(&budget),
            &|| false,
        )
        .unwrap();
        assert_eq!((out.width(), out.height()), (4, 6));
        assert_eq!(budget.used(), 4 * 6 * 16);
        assert_eq!(budget.peak(), 8 * 1024 * 1024 + 48 * 35);
        let expected = crate::apply_3x3(
            crate::camera_to_linear_srgb(m.metadata.xyz_to_camera).unwrap(),
            [0.4, 0.4, 0.45],
        );
        let RasterPixels::Rgba32Float(pixels) = out.pixels() else {
            panic!()
        };
        for p in pixels.chunks_exact(4) {
            for c in 0..3 {
                assert!((p[c] - expected[c]).abs() < 1e-6);
            }
        }
        assert_eq!(&*m.pixels, original.as_slice());
        drop(out);
        assert_eq!(budget.used(), 0);
        // Admission is checked before allocations and all refused credit releases.
        let short = MemoryBudget::new(8 * 1024 * 1024 + 48 * 35 - 1);
        assert!(
            develop_raw(
                &m,
                &DevelopOptions::default(),
                &OpcodeLists::default(),
                Some(&short),
                &|| false
            )
            .is_err()
        );
        assert_eq!(short.used(), 0);
    }
    #[test]
    fn xtrans_has_a_sensor_developed_thumbnail_and_linear_view() {
        let mut m = sensor();
        m.metadata.width = 12;
        m.metadata.height = 12;
        m.metadata.white_balance = [1.0; 4];
        let colors = [
            crate::CfaColor::Red,
            crate::CfaColor::Green,
            crate::CfaColor::Blue,
        ];
        let pattern = [
            1_u8, 2, 1, 1, 0, 1, 0, 1, 0, 2, 1, 2, 1, 2, 1, 1, 0, 1, 1, 0, 1, 1, 2, 1, 2, 1, 2, 0, 1, 0, 1,
            0, 1, 1, 2, 1,
        ];
        m.metadata.cfa = Some(CfaPattern {
            width: 6,
            height: 6,
            cells: pattern.iter().map(|c| colors[*c as usize]).collect(),
        });
        let pixels = (0..144)
            .map(|i| [200_u16, 400, 600][pattern[i / 12 % 6 * 6 + i % 12 % 6] as usize])
            .collect::<Vec<_>>();
        m.pixels = std::sync::Arc::new(pixels).into();
        let thumbnail = m.thumbnail_rgba8(3);
        assert_eq!(thumbnail.len(), 36);
        assert!(thumbnail.chunks_exact(4).all(|p| p == &thumbnail[..4]));
        let view = develop_raw(
            &m,
            &DevelopOptions::default(),
            &OpcodeLists::default(),
            None,
            &|| false,
        )
        .unwrap();
        assert_eq!((view.width(), view.height()), (12, 12));
    }

    #[test]
    fn gain_map_does_not_turn_unclipped_hdr_into_clipping() {
        let m = sensor();
        let ops = OpcodeLists {
            list2: vec![Opcode::GainMap {
                area: Area {
                    top: 0,
                    left: 0,
                    bottom: 6,
                    right: 8,
                    plane: 0,
                    planes: 1,
                    row_pitch: 1,
                    col_pitch: 1,
                },
                points_v: 1,
                points_h: 1,
                spacing: [1.0; 2],
                origin: [0.0; 2],
                map_planes: 1,
                gains: vec![4.0],
            }],
            ..Default::default()
        };
        let on = develop_raw(&m, &DevelopOptions::default(), &ops, None, &|| false).unwrap();
        let off = develop_raw(
            &m,
            &DevelopOptions {
                recover_highlights: false,
                ..Default::default()
            },
            &ops,
            None,
            &|| false,
        )
        .unwrap();
        let (RasterPixels::Rgba32Float(a), RasterPixels::Rgba32Float(b)) = (on.pixels(), off.pixels()) else {
            panic!()
        };
        for (x, y) in a.iter().zip(b.iter()) {
            assert!((x - y).abs() < 2e-6);
        }
        assert!(a.iter().any(|v| *v > 1.0));
    }
    #[test]
    fn random_ahd_tiles_equal_a_single_full_frame_evaluation() {
        let cfa = Cfa {
            width: 2,
            height: 2,
            cells: vec![0, 1, 1, 2],
        };
        let (w, h) = (259, 141);
        let mut seed = 42_u32;
        let data = (0..w * h)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed >> 8) as f32 / 16777216.0
            })
            .collect::<Vec<_>>();
        let m = Mosaic {
            w,
            h,
            data: &data,
            cfa: &cfa,
        };
        let tiled = ahd::ahd(&m, &|| false).unwrap();
        let full = ahd::tile(&m, 0, 0, w, h);
        assert!(
            tiled
                .data
                .iter()
                .zip(&full)
                .all(|(a, b)| a.map(f32::to_bits) == b.map(f32::to_bits))
        );
    }
}
