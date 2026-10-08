use rrrah_dedup::geometry::{
    Correspondence, GeometryPolicy, ProjectiveSamplingPolicy, verify_projective_sampled,
};
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 3);
    let width: f64 = args[1].parse().unwrap();
    let height: f64 = args[2].parse().unwrap();
    assert!(width > 0. && height > 0. && width.is_finite() && height.is_finite());
    let text = std::fs::read_to_string(&args[0]).unwrap();
    let points: Vec<_> = text
        .lines()
        .map(|line| {
            let p: Vec<f64> = line.split_whitespace().map(|v| v.parse().unwrap()).collect();
            assert_eq!(p.len(), 4);
            Correspondence {
                source: [p[0], p[1]],
                target: [p[2], p[3]],
            }
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
            let ids: Vec<usize> = points
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    p.target[0] >= rect[0]
                        && p.target[0] < rect[0] + rect[2]
                        && p.target[1] >= rect[1]
                        && p.target[1] < rect[1] + rect[3]
                })
                .map(|(i, _)| i)
                .collect();
            let subset: Vec<_> = ids.iter().map(|&i| points[i]).collect();
            let evidence = if subset.len() < 10 {
                None
            } else {
                verify_projective_sampled(
                    &subset,
                    GeometryPolicy {
                        tolerance: 2.,
                        min_inliers: 10,
                        max_points: 28000,
                        max_hypotheses: 4096,
                    },
                    ProjectiveSamplingPolicy {
                        trials: 4096,
                        seed: 0x1234abcd,
                    },
                    || false,
                )
                .unwrap()
            };
            let (matrix, inliers) = evidence.map_or(("null".to_string(), Vec::new()), |e| {
                (
                    format!("{:?}", e.transform.matrix),
                    e.inliers.iter().map(|&i| ids[i]).collect(),
                )
            });
            rows.push(format!("{{\"target_domain\":{rect:?},\"point_indices\":{ids:?},\"matrix\":{matrix},\"inliers\":{inliers:?}}}"));
        }
    }
    println!("{{\"status\":\"ok\",\"regions\":[{}]}}", rows.join(","));
}
