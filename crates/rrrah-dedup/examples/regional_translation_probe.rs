fn project(m: [[f64; 3]; 3], p: [f64; 2]) -> [f64; 2] {
    let z = m[2][0] * p[0] + m[2][1] * p[1] + m[2][2];
    assert!(z.is_finite() && z.abs() > 1e-12);
    [0, 1].map(|i| (m[i][0] * p[0] + m[i][1] * p[1] + m[i][2]) / z)
}
fn translated(m: [[f64; 3]; 3], delta: [f64; 2]) -> [[f64; 3]; 3] {
    let mut result = m;
    for i in 0..2 {
        for j in 0..3 {
            result[i][j] += delta[i] * m[2][j];
        }
    }
    result
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 4);
    let width: f64 = args[2].parse().unwrap();
    let height: f64 = args[3].parse().unwrap();
    assert!(width.is_finite() && height.is_finite() && width > 0. && height > 0.);
    let values: Vec<f64> = std::fs::read_to_string(&args[1])
        .unwrap()
        .split_whitespace()
        .map(|s| s.parse().unwrap())
        .collect();
    assert_eq!(values.len(), 9);
    assert!(values.iter().all(|v| v.is_finite()));
    let m = std::array::from_fn(|i| std::array::from_fn(|j| values[i * 3 + j]));
    let points: Vec<[f64; 4]> = std::fs::read_to_string(&args[0])
        .unwrap()
        .lines()
        .map(|line| {
            let v: Vec<f64> = line.split_whitespace().map(|s| s.parse().unwrap()).collect();
            assert_eq!(v.len(), 4);
            assert!(v.iter().all(|x| x.is_finite()));
            [v[0], v[1], v[2], v[3]]
        })
        .collect();
    assert!(points.len() <= 28000);
    let mut rows = Vec::new();
    for y in 0..4 {
        for x in 0..4 {
            let rect = [
                f64::from(x) * width / 4.,
                f64::from(y) * height / 4.,
                width / 4.,
                height / 4.,
            ];
            let ids: Vec<_> = points
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    p[2] >= rect[0] && p[2] < rect[0] + rect[2] && p[3] >= rect[1] && p[3] < rect[1] + rect[3]
                })
                .map(|(i, _)| i)
                .collect();
            if ids.len() < 10 {
                continue;
            }
            let mut delta = [0.; 2];
            for &id in &ids {
                let p = points[id];
                let q = project(m, [p[0], p[1]]);
                for i in 0..2 {
                    delta[i] += (p[i + 2] - q[i]) / ids.len() as f64;
                }
            }
            assert!(delta.iter().all(|v| v.is_finite()));
            let model = translated(m, delta);
            let inliers: Vec<_> = ids
                .iter()
                .copied()
                .filter(|&id| {
                    let p = points[id];
                    let q = project(model, [p[0], p[1]]);
                    (q[0] - p[2]).hypot(q[1] - p[3]) <= 2.
                })
                .collect();
            let matrix = if inliers.len() >= 10 {
                format!("{model:?}")
            } else {
                "null".into()
            };
            rows.push(format!("{{\"target_domain\":{rect:?},\"point_indices\":{ids:?},\"translation\":{delta:?},\"matrix\":{matrix},\"inliers\":{inliers:?}}}"));
        }
    }
    println!("{{\"status\":\"ok\",\"regions\":[{}]}}", rows.join(","));
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translated_projective_map_preserves_denominator_and_adds_target_shift() {
        let m = [[1.2, 0.1, 8.], [-0.2, 0.8, 4.], [0.001, -0.002, 1.]];
        let delta = [0.75, -1.25];
        let adjusted = translated(m, delta);
        assert_eq!(adjusted[2], m[2]);
        for p in [[0., 0.], [150., 40.], [90., 180.]] {
            let a = project(m, p);
            let b = project(adjusted, p);
            for i in 0..2 {
                assert!((b[i] - a[i] - delta[i]).abs() < 1e-10);
            }
        }
    }
}
