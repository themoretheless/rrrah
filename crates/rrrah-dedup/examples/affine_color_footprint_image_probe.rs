use rrrah_dedup::{
    affine_color::{AffineColorPolicy, RgbPair, fit_affine_color, validate_affine_color},
    animated::AnimationBudget,
    geometry::ProjectiveTransform,
    linear::LinearRgbaView,
};
fn pixel(view: &LinearRgbaView<'_>, x: u32, y: u32) -> Option<[f64; 3]> {
    let p = view.rgba(x, y)?.map(f64::from);
    assert_eq!(p[3], 1.);
    assert!(p[..3].iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
    Some([p[0], p[1], p[2]])
}
fn interpolate(view: &LinearRgbaView<'_>, p: [f64; 2]) -> Option<[f64; 3]> {
    let (w, h) = view.dimensions();
    if p.iter().any(|v| !v.is_finite() || *v < 0.) || p[0] + 1. >= f64::from(w) || p[1] + 1. >= f64::from(h) {
        return None;
    }
    let (x, y) = (p[0].floor() as u32, p[1].floor() as u32);
    let (tx, ty) = (p[0] - f64::from(x), p[1] - f64::from(y));
    let a = pixel(view, x, y)?;
    let b = pixel(view, x + 1, y)?;
    let c = pixel(view, x, y + 1)?;
    let d = pixel(view, x + 1, y + 1)?;
    Some(std::array::from_fn(|i| {
        let top = a[i] + (b[i] - a[i]) * tx;
        let bottom = c[i] + (d[i] - c[i]) * tx;
        top + (bottom - top) * ty
    }))
}
fn pairs(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    transform: ProjectiveTransform,
    domain: [u32; 4],
    opposite_domain: Option<[u32; 4]>,
    radius: i64,
) -> (Vec<RgbPair>, Vec<RgbPair>, u64) {
    let [x, y, w, h] = domain;
    let (tw, th) = target.dimensions();
    assert!(x.checked_add(w).unwrap() <= tw && y.checked_add(h).unwrap() <= th);
    let sites = u64::from(w) * u64::from(h);
    assert!(sites <= 20000);
    assert!((1..=16).contains(&radius));
    let taps = ((2 * radius + 1).pow(2)) as u64;
    let reads = sites.checked_mul(taps * 5).unwrap();
    assert!(reads <= 64000000);
    let inverse = transform.inverse().unwrap();
    let mut training = Vec::new();
    let mut held = Vec::new();
    training.try_reserve_exact(sites as usize).unwrap();
    held.try_reserve_exact(sites as usize).unwrap();
    let mut work = 0;
    for py in y..y + h {
        for px in x..x + w {
            if let Some([ox, oy, ow, oh]) = opposite_domain {
                let center = inverse.apply([f64::from(px), f64::from(py)]).unwrap();
                if center[0] < f64::from(ox)
                    || center[1] < f64::from(oy)
                    || center[0] >= f64::from(ox + ow)
                    || center[1] >= f64::from(oy + oh)
                {
                    continue;
                }
            }

            let mut aa = [0.; 3];
            let mut bb = [0.; 3];
            let mut valid = true;
            'window: for dy in -radius..=radius {
                for dx in -radius..=radius {
                    work += 5;
                    assert!(work <= reads);
                    let qx = i64::from(px) + dx;
                    let qy = i64::from(py) + dy;
                    if qx < 0 || qy < 0 || qx >= i64::from(tw) || qy >= i64::from(th) {
                        valid = false;
                        break 'window;
                    }
                    let Some(mapped) = inverse.apply([qx as f64, qy as f64]) else {
                        valid = false;
                        break 'window;
                    };
                    let Some(a) = interpolate(source, mapped) else {
                        valid = false;
                        break 'window;
                    };
                    let b = pixel(target, qx as u32, qy as u32).unwrap();
                    for i in 0..3 {
                        aa[i] += a[i] / taps as f64;
                        bb[i] += b[i] / taps as f64;
                    }
                }
            }
            if valid {
                let pair = RgbPair {
                    source: aa,
                    target: bb,
                };
                if ((px - x) / 8 + (py - y) / 8) % 2 == 0 {
                    training.push(pair);
                } else {
                    held.push(pair);
                }
            }
        }
    }
    (training, held, work)
}
fn reverse_radius(model: ProjectiveTransform, domain: [u32; 4]) -> i64 {
    let [x, y, w, h] = domain;
    let p = [f64::from(x) + f64::from(w) / 2., f64::from(y) + f64::from(h) / 2.];
    let m = model.matrix;
    let q = model.apply(p).unwrap();
    let den = m[2][0] * p[0] + m[2][1] * p[1] + m[2][2];
    let j: [[f64; 2]; 2] = std::array::from_fn(|i| std::array::from_fn(|k| (m[i][k] - q[i] * m[2][k]) / den));
    let scale = (j[0][0] * j[1][1] - j[0][1] * j[1][0]).abs().sqrt();
    assert!(scale.is_finite() && scale > 0.);
    let radius = (8. / scale).ceil() as i64;
    assert!((1..=16).contains(&radius));
    radius
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 3);
    let text = std::fs::read_to_string(&args[2]).unwrap();
    let v: Vec<f64> = text.split_whitespace().map(|v| v.parse().unwrap()).collect();
    assert_eq!(v.len(), 9);
    let model = ProjectiveTransform {
        matrix: std::array::from_fn(|i| std::array::from_fn(|j| v[i * 3 + j])),
    };
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let policy = AnimationBudget {
        max_frames: 1,
        max_pixels: 6400000,
        max_file_bytes: 5242880,
    };
    let a = rrrah_dedup::decode::decode_selected_frame_bounded(
        &rrrah_decode::DecodeRequest::new(&args[0]),
        policy,
        &budget,
        || false,
    )
    .unwrap();
    let b = rrrah_dedup::decode::decode_selected_frame_bounded(
        &rrrah_decode::DecodeRequest::new(&args[1]),
        policy,
        &budget,
        || false,
    )
    .unwrap();
    let av = a.view(|| false).unwrap();
    let bv = b.view(|| false).unwrap();
    let mut rows = Vec::new();
    for (label, source, target, transform, domain) in [
        ("forward", &av, &bv, model, [240, 60, 80, 60]),
        (
            "reverse",
            &bv,
            &av,
            model.inverse().unwrap(),
            [2115, 922, 103, 133],
        ),
    ] {
        let radius = if label == "forward" {
            8
        } else {
            reverse_radius(model, [2115, 922, 103, 133])
        };
        let (training, held, work) = pairs(
            source,
            target,
            transform,
            domain,
            Some(if label == "forward" {
                [2115, 922, 103, 133]
            } else {
                [240, 60, 80, 60]
            }),
            radius,
        );
        let fit = fit_affine_color(
            &training,
            AffineColorPolicy {
                minimum_samples: 1000,
                maximum_samples: 20000,
                minimum_variance: 1e-5,
                minimum_relative_pivot: 1e-6,
                maximum_coefficient: 5.,
                maximum_offset: 0.1,
            },
            || false,
        );
        let evidence = match fit {
            Ok(m) => match validate_affine_color(m, &held, 0.03, 20000, || false) {
                Ok(e) => format!(
                    "{{\"matrix\":{:?},\"offset\":{:?},\"samples\":{},\"matched\":{},\"squared_error\":{}}}",
                    m.matrix, m.offset, e.samples, e.matched, e.squared_error
                ),
                Err(e) => format!("{{\"validation_error\":\"{e:?}\"}}"),
            },
            Err(e) => format!("{{\"fit_error\":\"{e:?}\"}}"),
        };
        rows.push(format!("{{\"direction\":\"{label}\",\"domain\":{domain:?},\"training_samples\":{},\"heldout_samples\":{},\"filter_radius\":{radius},\"pixel_reads\":{work},\"evidence\":{evidence}}}",training.len(),held.len()));
    }
    drop(a);
    drop(b);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"ok\",\"directions\":[{}],\"managed_used\":0,\"managed_peak\":{},\"scope\":\"One fixed ROI pair, native opaque display-range decoder and mapped box forward radius8, reverse geometry-derived radius; separate checker training/heldout, coefficients5/offset0.1, tolerance0.03. Pair vectors bounded separately, outside decode credit. Centers restricted to opposite ROI; filter taps retain whole-image support. Diagnostic only, no copy decision.\"}}",
        rows.join(","),
        budget.peak()
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn radius_is_derived_from_area_scale() {
        for (scale, expected) in [(1., 8), (0.5, 16), (2., 4)] {
            assert_eq!(
                reverse_radius(
                    ProjectiveTransform {
                        matrix: [[scale, 0., 0.], [0., scale, 0.], [0., 0., 1.]]
                    },
                    [10, 20, 40, 60]
                ),
                expected
            );
        }
    }
    #[test]
    fn mapped_box_mean_matches_independent_bilinear_polynomial() {
        let mut data = Vec::new();
        for y in 0..64 {
            for x in 0..64 {
                data.extend([x as f32 / 128., y as f32 / 128., (x * y) as f32 / 4096., 1.]);
            }
        }
        let view = LinearRgbaView::new(64, 64, &data, 4096, || false).unwrap();
        let transform = ProjectiveTransform {
            matrix: [[1., 0., 0.5], [0., 1., -0.25], [0., 0., 1.]],
        };
        let (train, held, work) = pairs(&view, &view, transform, [16, 16, 16, 16], None, 14);
        assert_eq!(train.len(), 128);
        assert_eq!(held.len(), 128);
        assert_eq!(work, 256 * 841 * 5);
        for (values, parity) in [(&train, 0), (&held, 1)] {
            let mut index = 0;
            for y in 16..32 {
                for x in 16..32 {
                    if ((x - 16) / 8 + (y - 16) / 8) % 2 != parity {
                        continue;
                    }
                    let px = f64::from(x);
                    let py = f64::from(y);
                    let source = [
                        (px - 0.5) / 128.,
                        (py + 0.25) / 128.,
                        (px - 0.5) * (py + 0.25) / 4096.,
                    ];
                    let target = [px / 128., py / 128., px * py / 4096.];
                    for i in 0..3 {
                        assert!((values[index].source[i] - source[i]).abs() < 1e-13);
                        assert!((values[index].target[i] - target[i]).abs() < 1e-13);
                    }
                    index += 1;
                }
            }
            assert_eq!(index, values.len());
        }
        assert!(interpolate(&view, [-0.01, 16.]).is_none());
        assert!(interpolate(&view, [63., 16.]).is_none());
    }
    #[test]
    fn opposite_center_clipping_keeps_whole_image_filter_support() {
        let mut data = Vec::new();
        for y in 0..64 {
            for x in 0..64 {
                data.extend([x as f32 / 128., y as f32 / 128., (x * y) as f32 / 4096., 1.]);
            }
        }
        let view = LinearRgbaView::new(64, 64, &data, 4096, || false).unwrap();
        let transform = ProjectiveTransform {
            matrix: [[1., 0., 0.5], [0., 1., -0.25], [0., 0., 1.]],
        };
        let (train, held, work) = pairs(&view, &view, transform, [16, 16, 16, 16], Some([20, 20, 8, 8]), 8);
        assert_eq!(train.len(), 32);
        assert_eq!(held.len(), 32);
        assert_eq!(work, 64 * 289 * 5);
        for (rows, parity) in [(&train, 0u32), (&held, 1u32)] {
            for pair in rows {
                let x = (pair.target[0] * 128.).round() as u32;
                let y = (pair.target[1] * 128.).round() as u32;
                assert!((21..29).contains(&x) && (20..28).contains(&y));
                assert_eq!(((x - 16) / 8 + (y - 16) / 8) % 2, parity);
                assert!(
                    (pair.source[2] - (f64::from(x) - 0.5) * (f64::from(y) + 0.25) / 4096.).abs() < 1e-13
                );
            }
        }
    }
}
