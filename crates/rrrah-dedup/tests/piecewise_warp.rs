#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    geometry::Correspondence,
    piecewise_warp::{PiecewiseError, PiecewisePolicy, PiecewiseWarp, WarpTriangle},
};
fn grid() -> (Vec<Correspondence>, Vec<[usize; 3]>) {
    let points = (0..3)
        .flat_map(|y| {
            (0..3).map(move |x| {
                let x = f64::from(x) * 10.;
                let y = f64::from(y) * 10.;
                Correspondence {
                    source: [x + y * y / 100., y],
                    target: [x, y],
                }
            })
        })
        .collect();
    let mut faces = Vec::new();
    for y in 0..2 {
        for x in 0..2 {
            let a = y * 3 + x;
            faces.push([a, a + 1, a + 4]);
            faces.push([a, a + 4, a + 3]);
        }
    }
    (points, faces)
}
fn policy() -> PiecewisePolicy {
    PiecewisePolicy {
        maximum_points: 9,
        maximum_triangles: 8,
        maximum_pair_tests: 92,
        minimum_twice_area: 1e-6,
    }
}
#[test]
fn curved_grid_is_continuous_invertible_and_shared_credit_is_retained() {
    let (points, faces) = grid();
    let bytes = (8 * std::mem::size_of::<WarpTriangle>()) as u64;
    let budget = MemoryBudget::new(bytes);
    let warp =
        PiecewiseWarp::from_triangles(&points, &faces, (64, 64), (64, 64), policy(), &budget, || false)
            .unwrap();
    assert_eq!(warp.triangles().len(), 8);
    assert_eq!(budget.used(), bytes);
    for (p, q) in [
        ([5., 5.], [5.5, 5.]),
        ([5., 15.], [7.5, 15.]),
        ([10., 10.], [11., 10.]),
        ([15., 5.], [15.5, 5.]),
    ] {
        let mapped = warp.map_target(p, 8, || false).unwrap().unwrap();
        assert!(mapped.iter().zip(q).all(|(a, b)| (*a - b).abs() < 1e-10));
        let inverse = warp.map_source(mapped, 8, || false).unwrap().unwrap();
        assert!(inverse.iter().zip(p).all(|(a, b)| (*a - b).abs() < 1e-10));
    }
    assert_eq!(warp.map_target([30., 30.], 8, || false), Ok(None));
    assert_eq!(
        warp.map_target([5., 5.], 7, || false),
        Err(PiecewiseError::Budget)
    );
    assert_eq!(
        warp.map_target([f64::NAN, 5.], 8, || false),
        Err(PiecewiseError::Invalid)
    );
    let clone = warp.clone();
    drop(warp);
    assert_eq!(budget.used(), bytes);
    drop(clone);
    assert_eq!(budget.used(), 0);
}
#[test]
fn inverted_degenerate_duplicate_overlapping_and_t_junction_faces_refuse() {
    let cases = vec![
        (
            vec![[0., 0.], [10., 0.], [0., 10.]],
            vec![[0., 0.], [0., 10.], [10., 0.]],
            vec![[0, 1, 2]],
        ),
        (
            vec![[0., 0.], [10., 0.], [0., 10.]],
            vec![[0., 0.], [10., 0.], [20., 0.]],
            vec![[0, 1, 2]],
        ),
        (
            vec![[0., 0.], [10., 0.], [0., 10.]],
            vec![[0., 0.], [10., 0.], [0., 10.]],
            vec![[0, 1, 2], [2, 1, 0]],
        ),
        (
            vec![[0., 0.], [20., 0.], [10., 20.], [0., 15.], [20., 15.], [10., 0.]],
            vec![[0., 0.], [20., 0.], [10., 20.], [0., 15.], [20., 15.], [10., 0.]],
            vec![[0, 1, 2], [3, 4, 5]],
        ),
        (
            vec![
                [10., 10.],
                [30., 10.],
                [10., 30.],
                [20., 10.],
                [20., 0.],
                [30., 0.],
            ],
            vec![
                [10., 10.],
                [30., 10.],
                [10., 30.],
                [20., 10.],
                [20., 0.],
                [30., 0.],
            ],
            vec![[0, 1, 2], [3, 4, 5]],
        ),
    ];
    for (source, target, faces) in cases {
        let points: Vec<_> = source
            .into_iter()
            .zip(target)
            .map(|(source, target)| Correspondence { source, target })
            .collect();
        let budget = MemoryBudget::new(10000);
        assert!(matches!(
            PiecewiseWarp::from_triangles(&points, &faces, (64, 64), (64, 64), policy(), &budget, || false),
            Err(PiecewiseError::Invalid)
        ));
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn exact_memory_and_work_admission_and_every_checkpoint_cancel_are_atomic() {
    let (points, faces) = grid();
    let bytes = (8 * std::mem::size_of::<WarpTriangle>()) as u64;
    let budget = MemoryBudget::new(bytes - 1);
    assert!(matches!(
        PiecewiseWarp::from_triangles(&points, &faces, (64, 64), (64, 64), policy(), &budget, || false),
        Err(PiecewiseError::Budget)
    ));
    assert_eq!(budget.used(), 0);
    let budget = MemoryBudget::new(bytes);
    let mut short = policy();
    short.maximum_pair_tests -= 1;
    assert!(matches!(
        PiecewiseWarp::from_triangles(&points, &faces, (64, 64), (64, 64), short, &budget, || false),
        Err(PiecewiseError::Budget)
    ));
    assert_eq!(budget.peak(), 0);
    let calls = std::cell::Cell::new(0);
    let warp = PiecewiseWarp::from_triangles(&points, &faces, (64, 64), (64, 64), policy(), &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let total = calls.get();
    drop(warp);
    for stop in 1..=total {
        calls.set(0);
        assert!(matches!(
            PiecewiseWarp::from_triangles(&points, &faces, (64, 64), (64, 64), policy(), &budget, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(PiecewiseError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    let warp =
        PiecewiseWarp::from_triangles(&points, &faces, (64, 64), (64, 64), policy(), &budget, || false)
            .unwrap();
    calls.set(0);
    warp.map_target([5., 5.], 8, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let total = calls.get();
    for stop in 1..=total {
        calls.set(0);
        assert_eq!(
            warp.map_target([5., 5.], 8, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(PiecewiseError::Cancelled)
        );
        assert_eq!(budget.used(), bytes);
    }
    drop(warp);
    assert_eq!(budget.used(), 0);
}

#[test]
fn source_only_overlap_and_input_aliases_are_rejected() {
    let source = [[0., 0.], [10., 0.], [0., 10.], [2., 2.], [8., 2.], [2., 8.]];
    let target = [[0., 0.], [10., 0.], [0., 10.], [20., 20.], [26., 20.], [20., 26.]];
    let points: Vec<_> = source
        .into_iter()
        .zip(target)
        .map(|(source, target)| Correspondence { source, target })
        .collect();
    let budget = MemoryBudget::new(10000);
    assert!(matches!(
        PiecewiseWarp::from_triangles(
            &points,
            &[[0, 1, 2], [3, 4, 5]],
            (64, 64),
            (64, 64),
            policy(),
            &budget,
            || false
        ),
        Err(PiecewiseError::Invalid)
    ));
    assert_eq!(budget.used(), 0);
    let (mut points, faces) = grid();
    points[8].source = points[0].source;
    assert!(matches!(
        PiecewiseWarp::from_triangles(&points, &faces, (64, 64), (64, 64), policy(), &budget, || false),
        Err(PiecewiseError::Invalid)
    ));
    assert_eq!(budget.used(), 0);
    let (points, faces) = grid();
    let warp =
        PiecewiseWarp::from_triangles(&points, &faces, (64, 64), (64, 64), policy(), &budget, || false)
            .unwrap();
    let calls = std::cell::Cell::new(0);
    assert_eq!(
        warp.map_target([-1., 0.], 8, || {
            calls.set(calls.get() + 1);
            calls.get() == 2
        }),
        Err(PiecewiseError::Cancelled)
    );
    drop(warp);
    assert_eq!(budget.used(), 0);
}
