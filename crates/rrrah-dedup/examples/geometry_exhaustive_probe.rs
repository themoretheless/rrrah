use rrrah_dedup::geometry::{
    Correspondence, GeometryPolicy, ProjectiveSamplingPolicy, verify_projective,
    verify_projective_sampled_for_domains,
};

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 5);
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
    let source = (args[1].parse().unwrap(), args[2].parse().unwrap());
    let target = (args[3].parse().unwrap(), args[4].parse().unwrap());
    for trials in [4096, 65536, 262144] {
        let evidence = verify_projective_sampled_for_domains(
            &points,
            GeometryPolicy {
                tolerance: 2.,
                min_inliers: 10,
                max_points: 28000,
                max_hypotheses: trials,
            },
            ProjectiveSamplingPolicy {
                trials,
                seed: 0x1234abcd,
            },
            source,
            target,
            || false,
        )
        .unwrap();
        match evidence {
            Some(e) => println!(
                "{{\"trials\":{trials},\"matrix\":{:?},\"inliers\":{:?}}}",
                e.transform.matrix, e.inliers
            ),
            None => println!("{{\"trials\":{trials},\"matrix\":null,\"inliers\":[]}}"),
        }
    }
    let n = points.len() as u64;
    assert!(n >= 4 && n <= 64);
    let combinations = n * (n - 1) * (n - 2) * (n - 3) / 24;
    let evidence = verify_projective(
        &points,
        GeometryPolicy {
            tolerance: 2.,
            min_inliers: 10,
            max_points: 64,
            max_hypotheses: combinations,
        },
        || false,
    )
    .unwrap();
    match evidence {
        Some(e) => println!(
            "{{\"exhaustive_combinations\":{combinations},\"matrix\":{:?},\"inliers\":{:?}}}",
            e.transform.matrix, e.inliers
        ),
        None => println!("{{\"exhaustive_combinations\":{combinations},\"matrix\":null,\"inliers\":[]}}"),
    }
}
