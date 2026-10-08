#[derive(Debug, thiserror::Error)]
pub enum ThumbnailError {
    #[error(transparent)]
    Memory(#[from] crate::BufferError),
    #[error(transparent)]
    Frame(#[from] crate::FrameError),
    #[error("RAW thumbnail cancelled")]
    Cancelled,
    #[error("RAW thumbnail development failed")]
    Develop,
}
// Bounded CPU RAW previews using the display color contract.
use crate::{DecodedMosaic, Photometric, aces_tone_map_rgb, apply_3x3, camera_to_linear_srgb};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub(crate) fn develop(mosaic: &DecodedMosaic, edge: u32) -> Vec<u8> {
    develop_with_cancel(mosaic, edge, &|| false)
}
pub(crate) fn develop_with_cancel(
    mosaic: &DecodedMosaic,
    edge: u32,
    cancelled: &dyn Fn() -> bool,
) -> Vec<u8> {
    if cancelled() {
        return Vec::new();
    }
    let metadata = &mosaic.metadata;
    if metadata.photometric != Photometric::Cfa || metadata.validate().is_err() {
        return Vec::new();
    }
    if metadata.cfa.as_ref().is_some_and(|cfa| cfa.rgbe_quad().is_ok()) {
        return develop_rgbe_thumbnail(mosaic, edge, cancelled);
    }
    let Some(cfa) = metadata.cfa.as_ref().and_then(|cfa| cfa.bayer_quad().ok()) else {
        return develop_xtrans_thumbnail(mosaic, edge, cancelled);
    };
    let Ok(black) = metadata.black_level.bayer_quad() else {
        return Vec::new();
    };
    let Ok(white) = metadata
        .white_level
        .bayer_quad(metadata.cfa.as_ref().expect("validated CFA"))
    else {
        return Vec::new();
    };
    if (0..4).any(|phase| white[phase] <= black[phase]) {
        return Vec::new();
    }
    let Some(matrix) = camera_to_linear_srgb(metadata.xyz_to_camera) else {
        return Vec::new();
    };
    let (out_w, out_h) = dimensions(metadata.display_dimensions(), edge);
    let Some(size) = usize::try_from(out_w)
        .ok()
        .and_then(|w| usize::try_from(out_h).ok().and_then(|h| w.checked_mul(h)))
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return Vec::new();
    };
    let mut output = vec![0_u8; size];
    let crop = metadata.effective_crop();
    let first_green = usize::from(cfa[0] != 1);
    let sensor_sample = |x: i64, y: i64| {
        let x = x.clamp(0, i64::from(metadata.width) - 1) as usize;
        let y = y.clamp(0, i64::from(metadata.height) - 1) as usize;
        let phase = (y % 2) * 2 + x % 2;
        (f32::from(mosaic.pixels[y * metadata.width as usize + x]) - black[phase]).max(0.0)
            / (white[phase] - black[phase])
    };
    let sample = |x: i64, y: i64| {
        let phase = (y.clamp(0, i64::from(metadata.height) - 1) % 2) * 2
            + x.clamp(0, i64::from(metadata.width) - 1) % 2;
        let color = cfa[phase as usize] as usize;
        let gain = if color == 1 && phase as usize != first_green {
            metadata.white_balance[3]
        } else {
            metadata.white_balance[color]
        };
        sensor_sample(x, y) * gain
    };

    let row_bytes = out_w as usize * 4;
    let chunk_rows = 16_usize;
    let num_chunks = (out_h as usize).div_ceil(chunk_rows);
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(num_chunks)
        .max(1);

    if workers <= 1 {
        for y in 0..out_h {
            if cancelled() {
                return Vec::new();
            }
            let y_usize = y as usize;
            render_thumbnail_row(
                metadata,
                cfa,
                crop,
                out_w,
                out_h,
                matrix,
                &sample,
                &sensor_sample,
                y,
                &mut output[y_usize * row_bytes..(y_usize + 1) * row_bytes],
            );
        }
    } else {
        let is_cancelled = AtomicBool::new(false);
        let next = AtomicUsize::new(0);
        let slots: Vec<_> = output
            .chunks_mut(chunk_rows * row_bytes)
            .map(|chunk| Mutex::new(Some(chunk)))
            .collect();

        let do_work = || loop {
            if is_cancelled.load(Ordering::Relaxed) {
                break;
            }
            let idx = next.fetch_add(1, Ordering::Relaxed);
            if idx >= num_chunks {
                break;
            }
            let mut guard = slots[idx].lock().unwrap();
            let chunk = guard.take().unwrap();
            let start_y = (idx * chunk_rows) as u32;
            let end_y = ((idx + 1) * chunk_rows).min(out_h as usize) as u32;
            for y in start_y..end_y {
                let row_in_chunk = (y - start_y) as usize;
                render_thumbnail_row(
                    metadata,
                    cfa,
                    crop,
                    out_w,
                    out_h,
                    matrix,
                    &sample,
                    &sensor_sample,
                    y,
                    &mut chunk[row_in_chunk * row_bytes..(row_in_chunk + 1) * row_bytes],
                );
            }
        };

        std::thread::scope(|scope| {
            let shared_worker = &do_work;
            for worker_index in 1..workers {
                let _ = std::thread::Builder::new()
                    .name(format!("rrrah-thumb-{worker_index}"))
                    .spawn_scoped(scope, shared_worker);
            }
            loop {
                if cancelled() {
                    is_cancelled.store(true, Ordering::Relaxed);
                    break;
                }
                let idx = next.fetch_add(1, Ordering::Relaxed);
                if idx >= num_chunks {
                    break;
                }
                let mut guard = slots[idx].lock().unwrap();
                let chunk = guard.take().unwrap();
                let start_y = (idx * chunk_rows) as u32;
                let end_y = ((idx + 1) * chunk_rows).min(out_h as usize) as u32;
                for y in start_y..end_y {
                    let row_in_chunk = (y - start_y) as usize;
                    render_thumbnail_row(
                        metadata,
                        cfa,
                        crop,
                        out_w,
                        out_h,
                        matrix,
                        &sample,
                        &sensor_sample,
                        y,
                        &mut chunk[row_in_chunk * row_bytes..(row_in_chunk + 1) * row_bytes],
                    );
                }
            }
        });

        if is_cancelled.load(Ordering::Relaxed) || cancelled() {
            return Vec::new();
        }
    }
    output
}

#[inline]
fn render_thumbnail_row(
    metadata: &crate::RawMetadata,
    cfa: [u32; 4],
    crop: crate::Rect,
    out_w: u32,
    out_h: u32,
    matrix: [[f32; 3]; 3],
    sample: &impl Fn(i64, i64) -> f32,
    sensor_sample: &impl Fn(i64, i64) -> f32,
    y: u32,
    row_out: &mut [u8],
) {
    for x in 0..out_w {
        let uv = metadata
            .orientation
            .map_display_uv([(x as f32 + 0.5) / out_w as f32, (y as f32 + 0.5) / out_h as f32]);
        let sx =
            i64::from(crop.x) + (uv[0] * crop.width as f32).floor().min((crop.width - 1) as f32) as i64;
        let sy =
            i64::from(crop.y) + (uv[1] * crop.height as f32).floor().min((crop.height - 1) as f32) as i64;
        let (axis_w, axis_h) = if metadata.orientation.swaps_dimensions() {
            (out_h, out_w)
        } else {
            (out_w, out_h)
        };
        let footprint = [
            crop.width as f32 / axis_w as f32,
            crop.height as f32 / axis_h as f32,
        ];
        let rgb = if footprint[0] >= 2.0 || footprint[1] >= 2.0 {
            let center = [
                crop.x as f32 + uv[0] * crop.width as f32,
                crop.y as f32 + uv[1] * crop.height as f32,
            ];
            area_camera_rgb(
                cfa,
                [
                    center[0] - footprint[0] * 0.5,
                    center[1] - footprint[1] * 0.5,
                    center[0] + footprint[0] * 0.5,
                    center[1] + footprint[1] * 0.5,
                ],
                sample,
                sensor_sample,
            )
        } else {
            developed_camera_rgb(cfa, sx, sy, sample, sensor_sample)
        };
        let linear = aces_tone_map_rgb(apply_3x3(matrix, rgb));
        let x_usize = x as usize;
        row_out[x_usize * 4] = srgb_byte(linear[0]);
        row_out[x_usize * 4 + 1] = srgb_byte(linear[1]);
        row_out[x_usize * 4 + 2] = srgb_byte(linear[2]);
        row_out[x_usize * 4 + 3] = 255;
    }
}

pub(crate) fn dimensions((width, height): (u32, u32), edge: u32) -> (u32, u32) {
    let longest = width.max(height);
    let edge = edge.max(1);
    let scaled = |value: u32| {
        if longest <= edge {
            value
        } else {
            u32::try_from((u64::from(value) * u64::from(edge)).div_ceil(u64::from(longest)))
                .expect("bounded thumbnail dimension")
                .max(1)
        }
    };
    (scaled(width), scaled(height))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn srgb_byte(value: f32) -> u8 {
    let encoded = if value <= 0.003_130_8 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (encoded.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[allow(clippy::cast_possible_truncation)] // An average of finite f32 samples remains in their range.
fn mean_four(values: [f32; 4]) -> f32 {
    let sum = values[0] + values[1] + values[2] + values[3];
    if sum.is_finite() {
        sum * 0.25
    } else {
        (values.into_iter().map(f64::from).sum::<f64>() * 0.25) as f32
    }
}

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
fn bilinear(cfa: [u32; 4], x: i64, y: i64, sample: impl Fn(i64, i64) -> f32) -> [f32; 3] {
    let center = sample(x, y);
    let north = sample(x, y - 1);
    let south = sample(x, y + 1);
    let west = sample(x - 1, y);
    let east = sample(x + 1, y);
    let phase = ((y % 2) * 2 + x % 2) as usize;
    match cfa[phase] {
        0 | 2 => {
            let axial = mean_four([north, south, west, east]);
            let diagonal = mean_four([
                sample(x - 1, y - 1),
                sample(x + 1, y - 1),
                sample(x - 1, y + 1),
                sample(x + 1, y + 1),
            ]);
            if cfa[phase] == 0 {
                [center, axial, diagonal]
            } else {
                [diagonal, axial, center]
            }
        }
        _ => {
            let horizontal_red = cfa[((y % 2) * 2 + (x + 1) % 2) as usize] == 0;
            if horizontal_red {
                [west.midpoint(east), center, north.midpoint(south)]
            } else {
                [north.midpoint(south), center, west.midpoint(east)]
            }
        }
    }
}

#[cfg(test)]
mod numerical_tests {
    use super::*;
    #[test]
    fn bilinear_average_does_not_overflow_finite_hdr_samples() {
        for value in [f32::MAX, -f32::MAX] {
            for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                assert_eq!(bilinear([0, 1, 1, 2], x, y, |_, _| value), [value; 3]);
            }
        }
        assert_eq!(mean_four([f32::MAX, f32::MAX, -f32::MAX, -f32::MAX]), 0.0);
    }
}

fn developed_camera_rgb(
    cfa: [u32; 4],
    x: i64,
    y: i64,
    sample: impl Fn(i64, i64) -> f32,
    sensor: impl Fn(i64, i64) -> f32,
) -> [f32; 3] {
    let rgb = bilinear(cfa, x, y, sample);
    let quad_x = x - x % 2;
    let quad_y = y - y % 2;
    let clipped = [(0, 0), (1, 0), (0, 1), (1, 1)]
        .into_iter()
        .any(|(dx, dy)| sensor(quad_x + dx, quad_y + dy) >= 1.0);
    crate::reconstruct_clipped_camera_highlight(rgb, clipped)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn area_camera_rgb(
    cfa: [u32; 4],
    bounds: [f32; 4],
    sample: impl Fn(i64, i64) -> f32,
    sensor: impl Fn(i64, i64) -> f32,
) -> [f32; 3] {
    let start_x = (bounds[0].floor().max(0.0) as i64) / 2 * 2;
    let start_y = (bounds[1].floor().max(0.0) as i64) / 2 * 2;
    let mut sum = [0.0; 3];
    let mut total = 0.0;
    for y in (start_y..bounds[3].ceil() as i64).step_by(2) {
        for x in (start_x..bounds[2].ceil() as i64).step_by(2) {
            let weight = ((x + 2) as f32).min(bounds[2]) - (x as f32).max(bounds[0]);
            let weight =
                weight.max(0.0) * (((y + 2) as f32).min(bounds[3]) - (y as f32).max(bounds[1])).max(0.0);
            let mut rgb = [0.0; 3];
            let mut clipped = false;
            for (phase, (dx, dy)) in [(0, 0), (1, 0), (0, 1), (1, 1)].into_iter().enumerate() {
                let channel = cfa[phase] as usize;
                rgb[channel] += sample(x + dx, y + dy) * if channel == 1 { 0.5 } else { 1.0 };
                clipped |= sensor(x + dx, y + dy) >= 1.0;
            }
            let rgb = crate::reconstruct_clipped_camera_highlight(rgb, clipped);
            for channel in 0..3 {
                sum[channel] += rgb[channel] * weight;
            }
            total += weight;
        }
    }
    sum.map(|value| value / total.max(f32::MIN_POSITIVE))
}

/// Small X-Trans previews average linear camera samples by CFA color. They do
/// not run full-resolution AHD or the spatial quality-development pipeline.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn develop_xtrans_thumbnail(mosaic: &DecodedMosaic, edge: u32, cancelled: &dyn Fn() -> bool) -> Vec<u8> {
    let md = &mosaic.metadata;
    let Some(cfa) = md.cfa.as_ref() else {
        return Vec::new();
    };
    if cfa.width != 6 || cfa.height != 6 || cfa.cells.iter().any(|c| *c as u8 > 2) {
        return Vec::new();
    }
    let Some(matrix) = camera_to_linear_srgb(md.xyz_to_camera) else {
        return Vec::new();
    };
    let (w, h) = dimensions(md.display_dimensions(), edge);
    let Some(len) = (w as usize)
        .checked_mul(h as usize)
        .and_then(|n| n.checked_mul(4))
    else {
        return Vec::new();
    };
    let mut output = Vec::new();
    if output.try_reserve_exact(len).is_err() {
        return output;
    }
    output.resize(len, 0);
    let crop = md.effective_crop();
    let (aw, ah) = if md.orientation.swaps_dimensions() {
        (h, w)
    } else {
        (w, h)
    };
    let footprint = [
        (f64::from(crop.width) / f64::from(aw)).max(6.0),
        (f64::from(crop.height) / f64::from(ah)).max(6.0),
    ];
    let row_bytes = w as usize * 4;
    let chunk_rows = 16_usize;
    let num_chunks = (h as usize).div_ceil(chunk_rows);
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(num_chunks)
        .max(1);

    if workers <= 1 {
        for y in 0..h {
            if cancelled() {
                return Vec::new();
            }
            let y_usize = y as usize;
            if !render_xtrans_row(
                mosaic,
                cfa,
                crop,
                w,
                h,
                matrix,
                footprint,
                y,
                &mut output[y_usize * row_bytes..(y_usize + 1) * row_bytes],
            ) {
                return Vec::new();
            }
        }
    } else {
        let is_cancelled = AtomicBool::new(false);
        let has_error = AtomicBool::new(false);
        let next = AtomicUsize::new(0);
        let slots: Vec<_> = output
            .chunks_mut(chunk_rows * row_bytes)
            .map(|chunk| Mutex::new(Some(chunk)))
            .collect();

        let do_work = || loop {
            if is_cancelled.load(Ordering::Relaxed) || has_error.load(Ordering::Relaxed) {
                break;
            }
            let idx = next.fetch_add(1, Ordering::Relaxed);
            if idx >= num_chunks {
                break;
            }
            let mut guard = slots[idx].lock().unwrap();
            let chunk = guard.take().unwrap();
            let start_y = (idx * chunk_rows) as u32;
            let end_y = ((idx + 1) * chunk_rows).min(h as usize) as u32;
            for y in start_y..end_y {
                let row_in_chunk = (y - start_y) as usize;
                if !render_xtrans_row(
                    mosaic,
                    cfa,
                    crop,
                    w,
                    h,
                    matrix,
                    footprint,
                    y,
                    &mut chunk[row_in_chunk * row_bytes..(row_in_chunk + 1) * row_bytes],
                ) {
                    has_error.store(true, Ordering::Relaxed);
                    break;
                }
            }
        };

        std::thread::scope(|scope| {
            let shared_worker = &do_work;
            for worker_index in 1..workers {
                let _ = std::thread::Builder::new()
                    .name(format!("rrrah-xtrans-thumb-{worker_index}"))
                    .spawn_scoped(scope, shared_worker);
            }
            loop {
                if cancelled() {
                    is_cancelled.store(true, Ordering::Relaxed);
                    break;
                }
                let idx = next.fetch_add(1, Ordering::Relaxed);
                if idx >= num_chunks {
                    break;
                }
                let mut guard = slots[idx].lock().unwrap();
                let chunk = guard.take().unwrap();
                let start_y = (idx * chunk_rows) as u32;
                let end_y = ((idx + 1) * chunk_rows).min(h as usize) as u32;
                for y in start_y..end_y {
                    let row_in_chunk = (y - start_y) as usize;
                    if !render_xtrans_row(
                        mosaic,
                        cfa,
                        crop,
                        w,
                        h,
                        matrix,
                        footprint,
                        y,
                        &mut chunk[row_in_chunk * row_bytes..(row_in_chunk + 1) * row_bytes],
                    ) {
                        has_error.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            }
        });

        if is_cancelled.load(Ordering::Relaxed) || has_error.load(Ordering::Relaxed) || cancelled() {
            return Vec::new();
        }
    }
    output
}

#[inline]
fn render_xtrans_row(
    mosaic: &DecodedMosaic,
    cfa: &crate::CfaPattern,
    crop: crate::Rect,
    w: u32,
    h: u32,
    matrix: [[f32; 3]; 3],
    footprint: [f64; 2],
    y: u32,
    row_out: &mut [u8],
) -> bool {
    let md = &mosaic.metadata;
    for x in 0..w {
        let uv = md
            .orientation
            .map_display_uv([(x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32]);
        let center = [
            f64::from(crop.x) + f64::from(uv[0]) * f64::from(crop.width),
            f64::from(crop.y) + f64::from(uv[1]) * f64::from(crop.height),
        ];
        let bounds = [
            (center[0] - footprint[0] * 0.5).max(0.0),
            (center[1] - footprint[1] * 0.5).max(0.0),
            (center[0] + footprint[0] * 0.5).min(f64::from(md.width)),
            (center[1] + footprint[1] * 0.5).min(f64::from(md.height)),
        ];
        let mut sum = [0.0_f64; 3];
        let mut weights = [0.0_f64; 3];
        for sy in bounds[1].floor() as u32..bounds[3].ceil() as u32 {
            for sx in bounds[0].floor() as u32..bounds[2].ceil() as u32 {
                let color = cfa.color_at(sy, sx).expect("validated CFA") as usize;
                let weight = ((f64::from(sx) + 1.0).min(bounds[2]) - (f64::from(sx)).max(bounds[0]))
                    .max(0.0)
                    * ((f64::from(sy) + 1.0).min(bounds[3]) - (f64::from(sy)).max(bounds[1])).max(0.0);
                let Some(black) = md.black_level.at(sy, sx, 0) else {
                    return false;
                };
                let white = match md.white_level.0.len() {
                    1 => md.white_level.0[0],
                    3 => md.white_level.0[color],
                    _ => return false,
                };
                if white <= black {
                    return false;
                }
                let sample = (f64::from(mosaic.pixels[sy as usize * md.width as usize + sx as usize])
                    - f64::from(black))
                .max(0.0)
                    / f64::from(white - black)
                    * f64::from(md.white_balance[color]);
                sum[color] += sample * weight;
                weights[color] += weight;
            }
        }
        if weights.iter().any(|v| *v <= 0.0) {
            return false;
        }
        let rgb = std::array::from_fn(|c| (sum[c] / weights[c]) as f32);
        let linear = aces_tone_map_rgb(apply_3x3(matrix, rgb));
        let x_usize = x as usize;
        row_out[x_usize * 4] = srgb_byte(linear[0]);
        row_out[x_usize * 4 + 1] = srgb_byte(linear[1]);
        row_out[x_usize * 4 + 2] = srgb_byte(linear[2]);
        row_out[x_usize * 4 + 3] = 255;
    }
    true
}

/// Four-plane preview, with bounded output only. CFA cell area averaging retains
/// E independently and extends missing odd-edge sites using their own phase.
fn develop_rgbe_thumbnail(mosaic: &DecodedMosaic, edge: u32, cancelled: &dyn Fn() -> bool) -> Vec<u8> {
    let m = &mosaic.metadata;
    let Ok(calibration) = crate::rgbe::Calibration::new(m) else {
        return Vec::new();
    };
    let Some(sensor_count) = (m.width as usize).checked_mul(m.height as usize) else {
        return Vec::new();
    };
    if mosaic.pixels.len() != sensor_count {
        return Vec::new();
    }
    let (w, h) = dimensions(m.display_dimensions(), edge);
    let Some(bytes) = (w as usize)
        .checked_mul(h as usize)
        .and_then(|n| n.checked_mul(4))
    else {
        return Vec::new();
    };
    let mut output = Vec::new();
    if output.try_reserve_exact(bytes).is_err() {
        return output;
    }
    output.resize(bytes, 0);
    let crop = m.effective_crop();
    let (axis_w, axis_h) = if m.orientation.swaps_dimensions() {
        (h, w)
    } else {
        (w, h)
    };
    let footprint = [
        f64::from(crop.width) / f64::from(axis_w),
        f64::from(crop.height) / f64::from(axis_h),
    ];
    let row_bytes = w as usize * 4;
    let chunk_rows = 16_usize;
    let num_chunks = (h as usize).div_ceil(chunk_rows);
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(num_chunks)
        .max(1);

    if workers <= 1 {
        for y in 0..h {
            if cancelled() {
                return Vec::new();
            }
            let y_usize = y as usize;
            if !render_rgbe_row(
                mosaic,
                &calibration,
                crop,
                w,
                h,
                footprint,
                y,
                &mut output[y_usize * row_bytes..(y_usize + 1) * row_bytes],
            ) {
                return Vec::new();
            }
        }
    } else {
        let is_cancelled = AtomicBool::new(false);
        let has_error = AtomicBool::new(false);
        let next = AtomicUsize::new(0);
        let slots: Vec<_> = output
            .chunks_mut(chunk_rows * row_bytes)
            .map(|chunk| Mutex::new(Some(chunk)))
            .collect();

        let do_work = || loop {
            if is_cancelled.load(Ordering::Relaxed) || has_error.load(Ordering::Relaxed) {
                break;
            }
            let idx = next.fetch_add(1, Ordering::Relaxed);
            if idx >= num_chunks {
                break;
            }
            let mut guard = slots[idx].lock().unwrap();
            let chunk = guard.take().unwrap();
            let start_y = (idx * chunk_rows) as u32;
            let end_y = ((idx + 1) * chunk_rows).min(h as usize) as u32;
            for y in start_y..end_y {
                let row_in_chunk = (y - start_y) as usize;
                if !render_rgbe_row(
                    mosaic,
                    &calibration,
                    crop,
                    w,
                    h,
                    footprint,
                    y,
                    &mut chunk[row_in_chunk * row_bytes..(row_in_chunk + 1) * row_bytes],
                ) {
                    has_error.store(true, Ordering::Relaxed);
                    break;
                }
            }
        };

        std::thread::scope(|scope| {
            let shared_worker = &do_work;
            for worker_index in 1..workers {
                let _ = std::thread::Builder::new()
                    .name(format!("rrrah-rgbe-thumb-{worker_index}"))
                    .spawn_scoped(scope, shared_worker);
            }
            loop {
                if cancelled() {
                    is_cancelled.store(true, Ordering::Relaxed);
                    break;
                }
                let idx = next.fetch_add(1, Ordering::Relaxed);
                if idx >= num_chunks {
                    break;
                }
                let mut guard = slots[idx].lock().unwrap();
                let chunk = guard.take().unwrap();
                let start_y = (idx * chunk_rows) as u32;
                let end_y = ((idx + 1) * chunk_rows).min(h as usize) as u32;
                for y in start_y..end_y {
                    let row_in_chunk = (y - start_y) as usize;
                    if !render_rgbe_row(
                        mosaic,
                        &calibration,
                        crop,
                        w,
                        h,
                        footprint,
                        y,
                        &mut chunk[row_in_chunk * row_bytes..(row_in_chunk + 1) * row_bytes],
                    ) {
                        has_error.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            }
        });

        if is_cancelled.load(Ordering::Relaxed) || has_error.load(Ordering::Relaxed) || cancelled() {
            return Vec::new();
        }
    }
    output
}

#[inline]
fn render_rgbe_row(
    mosaic: &DecodedMosaic,
    calibration: &crate::rgbe::Calibration,
    crop: crate::Rect,
    w: u32,
    h: u32,
    footprint: [f64; 2],
    y: u32,
    row_out: &mut [u8],
) -> bool {
    let m = &mosaic.metadata;
    let sample = |x: u32, y: u32| mosaic.pixels[(y as usize) * m.width as usize + x as usize];
    for x in 0..w {
        let uv = m
            .orientation
            .map_display_uv([(x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32]);
        let center = [
            f64::from(crop.x) + f64::from(uv[0]) * f64::from(crop.width),
            f64::from(crop.y) + f64::from(uv[1]) * f64::from(crop.height),
        ];
        let rgb = if footprint[0] >= 2. || footprint[1] >= 2. {
            let lower = [
                (center[0] - footprint[0] * 0.5).max(f64::from(crop.x)),
                (center[1] - footprint[1] * 0.5).max(f64::from(crop.y)),
            ];
            let upper = [
                (center[0] + footprint[0] * 0.5).min(f64::from(crop.x + crop.width)),
                (center[1] + footprint[1] * 0.5).min(f64::from(crop.y + crop.height)),
            ];
            let mut sum = [0f64; 4];
            let mut total = 0.;
            for sy in ((lower[1].floor() as u32 / 2 * 2)..upper[1].ceil() as u32).step_by(2) {
                for sx in ((lower[0].floor() as u32 / 2 * 2)..upper[0].ceil() as u32).step_by(2) {
                    let weight = (f64::from(sx + 2).min(upper[0]) - f64::from(sx).max(lower[0])).max(0.)
                        * (f64::from(sy + 2).min(upper[1]) - f64::from(sy).max(lower[1])).max(0.);
                    for (phase, channel) in calibration.quad().into_iter().enumerate() {
                        let px = (phase % 2) as u32;
                        let py = (phase / 2) as u32;
                        let last_x = px + (m.width - 1 - px) / 2 * 2;
                        let last_y = py + (m.height - 1 - py) / 2 * 2;
                        let xx = (sx + px).min(last_x);
                        let yy = (sy + py).min(last_y);
                        let Some(normalized) = calibration.normalize_site(xx, yy, sample(xx, yy)) else {
                            return false;
                        };
                        sum[channel as usize] += f64::from(normalized) * weight;
                    }
                    total += weight;
                }
            }
            calibration.linear_rgb(sum.map(|v| v / total))
        } else {
            calibration.reconstruct_linear_rgb(
                [
                    center[0].floor().min(f64::from(crop.x + crop.width - 1)) as u32,
                    center[1].floor().min(f64::from(crop.y + crop.height - 1)) as u32,
                ],
                sample,
            )
        };
        let Some(rgb) = rgb else {
            return false;
        };
        let rgb = rgb.map(|v| v as f32);
        if rgb.iter().any(|v| !v.is_finite()) {
            return false;
        }
        let aces = aces_tone_map_rgb(rgb);
        let x_usize = x as usize;
        row_out[x_usize * 4] = srgb_byte(aces[0]);
        row_out[x_usize * 4 + 1] = srgb_byte(aces[1]);
        row_out[x_usize * 4 + 2] = srgb_byte(aces[2]);
        row_out[x_usize * 4 + 3] = 255;
    }
    true
}
