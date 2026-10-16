//! Independent f64 oracle for all six GPU Bayer interpolation contracts.
//! We evaluate inverse-gradient averages as barycentric sums, avoiding GPU f32 reciprocal chains.
use rrrah_core::DecodedMosaic;

pub fn weighted_rgb(
    mosaic: &DecodedMosaic,
    x: u32,
    y: u32,
    algorithm: rrrah_gpu::GpuDemosaicAlgorithm,
) -> [f64; 3] {
    use rrrah_gpu::GpuDemosaicAlgorithm::{Rcd, Vng};
    let metadata = &mosaic.metadata;
    let quad = metadata.cfa.as_ref().unwrap().bayer_quad().unwrap();
    let white = metadata
        .white_level
        .bayer_quad(metadata.cfa.as_ref().unwrap())
        .unwrap();
    let first_green = quad.iter().position(|&c| c == 1).unwrap();
    let reflect = |value: i32, size: u32| {
        let period = 2 * (size as i32 - 1);
        let p = value.rem_euclid(period);
        if p >= size as i32 {
            (period - p) as u32
        } else {
            p as u32
        }
    };
    let sample = |sx: i32, sy: i32| {
        let sx = reflect(sx, metadata.width);
        let sy = reflect(sy, metadata.height);
        let phase = ((sy % 2) * 2 + sx % 2) as usize;
        let channel = if quad[phase] == 1 && phase != first_green {
            3
        } else {
            quad[phase] as usize
        };
        let black = f64::from(metadata.black_level.at(sy, sx, 0).unwrap());
        (f64::from(mosaic.pixels[(sy * metadata.width + sx) as usize]) - black).max(0.)
            / (f64::from(white[phase]) - black)
            * f64::from(metadata.white_balance[channel])
    };
    let average = |a: f64, b: f64, da: f64, db: f64| (a * db + b * da) / (da + db);
    let green = |sx: i32, sy: i32| {
        let center = sample(sx, sy);
        let west = sample(sx - 1, sy);
        let east = sample(sx + 1, sy);
        let north = sample(sx, sy - 1);
        let south = sample(sx, sy + 1);
        let horizontal_correction = 2. * center - sample(sx - 2, sy) - sample(sx + 2, sy);
        let vertical_correction = 2. * center - sample(sx, sy - 2) - sample(sx, sy + 2);
        let horizontal =
            ((west + east) / 2. + horizontal_correction / 4.).clamp(west.min(east), west.max(east));
        let vertical =
            ((north + south) / 2. + vertical_correction / 4.).clamp(north.min(south), north.max(south));
        if algorithm == Rcd {
            let dh = 0.00001
                + (west - east).abs()
                + (center - sample(sx - 2, sy)).abs()
                + (center - sample(sx + 2, sy)).abs()
                + ((west - sample(sx - 3, sy)).abs() + (east - sample(sx + 3, sy)).abs()) / 2.;
            let dv = 0.00001
                + (north - south).abs()
                + (center - sample(sx, sy - 2)).abs()
                + (center - sample(sx, sy + 2)).abs()
                + ((north - sample(sx, sy - 3)).abs() + (south - sample(sx, sy + 3)).abs()) / 2.;
            average(horizontal, vertical, dh * dh, dv * dv)
        } else {
            average(
                horizontal,
                vertical,
                0.00001 + (west - east).abs() + horizontal_correction.abs(),
                0.00001 + (north - south).abs() + vertical_correction.abs(),
            )
        }
    };
    let (x, y) = (x as i32, y as i32);
    let center = sample(x, y);
    let own = quad[((y & 1) * 2 + (x & 1)) as usize];
    let camera = if matches!(
        algorithm,
        rrrah_gpu::GpuDemosaicAlgorithm::Bilinear
            | rrrah_gpu::GpuDemosaicAlgorithm::SmoothHue
            | rrrah_gpu::GpuDemosaicAlgorithm::MalvarHeCutler
    ) {
        simple_camera(&sample, x, y, quad, algorithm)
    } else if own == 0 || own == 2 {
        let mut g = green(x, y);
        let nw = sample(x - 1, y - 1);
        let ne = sample(x + 1, y - 1);
        let sw = sample(x - 1, y + 1);
        let se = sample(x + 1, y + 1);
        let north = sample(x, y - 1);
        let south = sample(x, y + 1);
        let west = sample(x - 1, y);
        let east = sample(x + 1, y);
        if algorithm == Vng {
            let n2 = sample(x, y - 2);
            let s2 = sample(x, y + 2);
            let w2 = sample(x - 2, y);
            let e2 = sample(x + 2, y);
            let gradients = [
                (north - south).abs() + (n2 - center).abs() + ((nw - sw).abs() + (ne - se).abs()) / 2.,
                (east - west).abs() + (e2 - center).abs() + ((ne - nw).abs() + (se - sw).abs()) / 2.,
                (south - north).abs() + (s2 - center).abs() + ((sw - nw).abs() + (se - ne).abs()) / 2.,
                (west - east).abs() + (w2 - center).abs() + ((nw - ne).abs() + (sw - se).abs()) / 2.,
            ];
            let low = gradients.into_iter().fold(f64::INFINITY, f64::min);
            let high = gradients.into_iter().fold(0.0f64, f64::max);
            let threshold = 1.5 * low + (high - low) / 2. + 0.00001;
            let estimates = [
                north + (center - n2) / 2.,
                east + (center - e2) / 2.,
                south + (center - s2) / 2.,
                west + (center - w2) / 2.,
            ];
            let mut sum = 0.;
            let mut count = 0.;
            for i in 0..4 {
                if gradients[i] <= threshold {
                    sum += estimates[i];
                    count += 1.;
                }
            }
            g = (sum / count).clamp(
                north.min(south).min(west).min(east),
                north.max(south).max(west).max(east),
            );
        }
        let local_green = |sx: i32, sy: i32, a: f64, b: f64| {
            if algorithm == Rcd {
                green(sx, sy)
            } else {
                (a + b) / 2.
            }
        };
        let differences = [
            nw - local_green(x - 1, y - 1, north, west),
            ne - local_green(x + 1, y - 1, north, east),
            sw - local_green(x - 1, y + 1, south, west),
            se - local_green(x + 1, y + 1, south, east),
        ];
        let diagonal = if algorithm == Vng {
            differences.iter().sum::<f64>() / 4.
        } else {
            average(
                (differences[0] + differences[3]) / 2.,
                (differences[1] + differences[2]) / 2.,
                0.00001 + (nw - se).abs() + (differences[0] - differences[3]).abs(),
                0.00001 + (ne - sw).abs() + (differences[1] - differences[2]).abs(),
            )
        };
        let opposite = (g + diagonal).clamp(nw.min(ne).min(sw).min(se), nw.max(ne).max(sw).max(se));
        if own == 0 {
            [center, g, opposite]
        } else {
            [opposite, g, center]
        }
    } else {
        let west = sample(x - 1, y);
        let east = sample(x + 1, y);
        let north = sample(x, y - 1);
        let south = sample(x, y + 1);
        let horizontal = (center + (west - green(x - 1, y) + east - green(x + 1, y)) / 2.)
            .clamp(west.min(east), west.max(east));
        let vertical = (center + (north - green(x, y - 1) + south - green(x, y + 1)) / 2.)
            .clamp(north.min(south), north.max(south));
        if quad[((y & 1) * 2 + ((x + 1) & 1)) as usize] == 0 {
            [horizontal, center, vertical]
        } else {
            [vertical, center, horizontal]
        }
    };
    let matrix = rrrah_core::camera_to_linear_srgb(metadata.xyz_to_camera).unwrap();
    matrix.map(|row| {
        row.iter()
            .zip(camera)
            .map(|(&coefficient, value)| f64::from(coefficient) * value)
            .sum()
    })
}

// Direct f64 stencil evaluation, separate from production CPU/GPU implementations.
fn simple_camera(
    sample: &impl Fn(i32, i32) -> f64,
    x: i32,
    y: i32,
    quad: [u32; 4],
    algorithm: rrrah_gpu::GpuDemosaicAlgorithm,
) -> [f64; 3] {
    use rrrah_gpu::GpuDemosaicAlgorithm::{Bilinear, MalvarHeCutler, SmoothHue};
    let c = sample(x, y);
    let own = quad[((y & 1) * 2 + (x & 1)) as usize];
    let horizontal_red = quad[((y & 1) * 2 + ((x + 1) & 1)) as usize] == 0;
    let n = sample(x, y - 1);
    let s = sample(x, y + 1);
    let w = sample(x - 1, y);
    let e = sample(x + 1, y);
    let diagonals = [
        sample(x - 1, y - 1),
        sample(x + 1, y - 1),
        sample(x - 1, y + 1),
        sample(x + 1, y + 1),
    ];
    let sum_d = diagonals.iter().sum::<f64>();
    let range = |v: f64, values: &[f64]| {
        v.clamp(
            values.iter().copied().fold(f64::INFINITY, f64::min),
            values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let smooth_green =
        |sx, sy| (sample(sx - 1, sy) + sample(sx + 1, sy) + sample(sx, sy - 1) + sample(sx, sy + 1)) / 4.;
    if own != 1 {
        let (g, opposite) = match algorithm {
            Bilinear => ((n + s + w + e) / 4., sum_d / 4.),
            MalvarHeCutler => {
                let correction =
                    4. * c - sample(x - 2, y) - sample(x + 2, y) - sample(x, y - 2) - sample(x, y + 2);
                (
                    range((n + s + w + e) / 4. + correction / 8., &[n, s, w, e]),
                    range(sum_d / 4. + 3. * correction / 16., &diagonals),
                )
            }
            SmoothHue => {
                let g = (n + s + w + e) / 4.;
                let offsets = [(-1, -1), (1, -1), (-1, 1), (1, 1)];
                let ratio = offsets
                    .iter()
                    .zip(diagonals)
                    .map(|(&(dx, dy), v)| (v + 0.001) / (smooth_green(x + dx, y + dy) + 0.001))
                    .sum::<f64>()
                    / 4.;
                (g, range((g + 0.001) * ratio - 0.001, &diagonals))
            }
            _ => unreachable!(),
        };
        return if own == 0 {
            [c, g, opposite]
        } else {
            [opposite, g, c]
        };
    }
    let (horizontal, vertical) = match algorithm {
        Bilinear => ((w + e) / 2., (n + s) / 2.),
        MalvarHeCutler => {
            let h2 = sample(x - 2, y) + sample(x + 2, y);
            let v2 = sample(x, y - 2) + sample(x, y + 2);
            (
                range((w + e) / 2. + (5. * c - sum_d - h2 + v2 / 2.) / 8., &[w, e]),
                range((n + s) / 2. + (5. * c - sum_d - v2 + h2 / 2.) / 8., &[n, s]),
            )
        }
        SmoothHue => {
            let h = ((w + 0.001) / (smooth_green(x - 1, y) + 0.001)
                + (e + 0.001) / (smooth_green(x + 1, y) + 0.001))
                / 2.;
            let v = ((n + 0.001) / (smooth_green(x, y - 1) + 0.001)
                + (s + 0.001) / (smooth_green(x, y + 1) + 0.001))
                / 2.;
            (
                range((c + 0.001) * h - 0.001, &[w, e]),
                range((c + 0.001) * v - 0.001, &[n, s]),
            )
        }
        _ => unreachable!(),
    };
    if horizontal_red {
        [horizontal, c, vertical]
    } else {
        [vertical, c, horizontal]
    }
}
