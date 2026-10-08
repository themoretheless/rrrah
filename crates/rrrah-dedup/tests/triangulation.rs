#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    geometry::Correspondence,
    piecewise_warp::{PiecewiseError, PiecewisePolicy, PiecewiseWarp},
    triangulation::{TriangulationPolicy, triangulate_targets},
};
use std::cell::Cell;
fn points() -> Vec<Correspondence> {
    (0..3)
        .flat_map(|y| {
            (0..3).map(move |x| {
                let x = x as f64 * 10.;
                let y = y as f64 * 10.;
                Correspondence {
                    target: [x, y],
                    source: [x + y * y / 100., y],
                }
            })
        })
        .collect()
}
fn policy() -> TriangulationPolicy {
    TriangulationPolicy {
        maximum_points: 9,
        maximum_triangles: 32,
        maximum_predicate_tests: 100000,
    }
}
#[test]
fn grid_topology_is_deterministic_and_passes_source_validation() {
    let p = points();
    let budget = MemoryBudget::new(100000);
    let faces = triangulate_targets(&p, (64, 64), policy(), &budget, || false).unwrap();
    assert_eq!(faces.len(), 8);
    let again = triangulate_targets(&p, (64, 64), policy(), &budget, || false).unwrap();
    assert_eq!(&*faces, &*again);
    let mut used = [false; 9];
    for f in faces.iter() {
        for i in f {
            used[*i] = true;
        }
        let [a, b, c] = f.map(|i| p[i].target);
        assert!((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) > 0.);
    }
    assert!(used.into_iter().all(|v| v));
    let warp = PiecewiseWarp::from_triangles(
        &p,
        &faces,
        (64, 64),
        (64, 64),
        PiecewisePolicy {
            maximum_points: 9,
            maximum_triangles: 8,
            maximum_pair_tests: 92,
            minimum_twice_area: 1e-6,
        },
        &budget,
        || false,
    )
    .unwrap();
    for point in &p {
        let mapped = warp.map_target(point.target, 8, || false).unwrap().unwrap();
        assert!(
            mapped
                .iter()
                .zip(point.source)
                .all(|(a, b)| (*a - b).abs() < 1e-9)
        );
    }
    drop(warp);
    drop(faces);
    drop(again);
    assert_eq!(budget.used(), 0);
}
#[test]
fn invalid_budget_and_every_cancellation_release_credit() {
    let p = points();
    let budget = MemoryBudget::new(100000);
    let calls = Cell::new(0);
    let result = triangulate_targets(&p, (64, 64), policy(), &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    drop(result);
    let total = calls.get();
    for checkpoint in 1..=total {
        let count = Cell::new(0);
        assert!(matches!(
            triangulate_targets(&p, (64, 64), policy(), &budget, || {
                count.set(count.get() + 1);
                count.get() == checkpoint
            }),
            Err(PiecewiseError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    let mut duplicate = p.clone();
    duplicate[1].target = duplicate[0].target;
    assert!(matches!(
        triangulate_targets(&duplicate, (64, 64), policy(), &budget, || false),
        Err(PiecewiseError::Invalid)
    ));
    let line: Vec<_> = (0..3)
        .map(|i| Correspondence {
            source: [i as f64, 0.],
            target: [i as f64, 0.],
        })
        .collect();
    assert!(matches!(
        triangulate_targets(&line, (64, 64), policy(), &budget, || false),
        Err(PiecewiseError::Invalid)
    ));
    let mut limited = policy();
    limited.maximum_predicate_tests = 0;
    assert!(matches!(
        triangulate_targets(&p, (64, 64), limited, &budget, || false),
        Err(PiecewiseError::Budget)
    ));
    assert!(matches!(
        triangulate_targets(&p, (64, 64), policy(), &MemoryBudget::new(1), || false),
        Err(PiecewiseError::Budget)
    ));
    assert_eq!(budget.used(), 0);
}

fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn hull_twice_area(points: &[Correspondence]) -> f64 {
    let mut v: Vec<_> = points.iter().map(|p| p.target).collect();
    v.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    let mut hull = Vec::new();
    for &p in &v {
        while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0. {
            hull.pop();
        }
        hull.push(p);
    }
    let lower = hull.len();
    for &p in v.iter().rev().skip(1) {
        while hull.len() > lower && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0. {
            hull.pop();
        }
        hull.push(p);
    }
    hull.pop();
    hull.iter()
        .zip(hull.iter().cycle().skip(1))
        .take(hull.len())
        .map(|(a, b)| a[0] * b[1] - a[1] * b[0])
        .sum()
}
#[test]
fn seeded_clouds_match_independent_hull_and_empty_circumcircle_oracles() {
    let mut seed = 947281u64;
    for case in 0..24 {
        let n = 12 + case;
        let mut points = Vec::new();
        for _ in 0..n {
            let mut next = || {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                ((seed >> 32) as f64) / u32::MAX as f64 * 80. + 10.
            };
            let target = [next(), next()];
            points.push(Correspondence {
                source: target,
                target,
            });
        }
        let budget = MemoryBudget::new(1000000);
        let faces = triangulate_targets(
            &points,
            (100, 100),
            TriangulationPolicy {
                maximum_points: 100,
                maximum_triangles: 256,
                maximum_predicate_tests: 1000000,
            },
            &budget,
            || false,
        )
        .unwrap();
        let mut used = vec![false; n];
        let mut area = 0.;
        for f in faces.iter() {
            let [a, b, c] = f.map(|i| {
                used[i] = true;
                points[i].target
            });
            let twice = cross(a, b, c);
            assert!(twice > 0.);
            area += twice;
            // Solve the circumcenter independently of the insertion determinant.
            let u = [b[0] - a[0], b[1] - a[1]];
            let v = [c[0] - a[0], c[1] - a[1]];
            let uu = u[0] * u[0] + u[1] * u[1];
            let vv = v[0] * v[0] + v[1] * v[1];
            let center = [
                a[0] + (uu * v[1] - vv * u[1]) / (2. * twice),
                a[1] + (u[0] * vv - v[0] * uu) / (2. * twice),
            ];
            let distance = |p: [f64; 2]| (p[0] - center[0]).powi(2) + (p[1] - center[1]).powi(2);
            let radius = distance(a);
            for (i, p) in points.iter().enumerate() {
                if !f.contains(&i) {
                    assert!(
                        distance(p.target) >= radius - 1e-7 * radius.max(1.),
                        "case {case} face {f:?}"
                    );
                }
            }
        }
        assert!(used.into_iter().all(|v| v));
        assert!(
            (area - hull_twice_area(&points)).abs() < 1e-7,
            "case {case}: lost hull area"
        );
        drop(faces);
        assert_eq!(budget.used(), 0);
    }
}
