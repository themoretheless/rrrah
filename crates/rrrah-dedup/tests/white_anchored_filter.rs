use rrrah_dedup::{
    geometry::Transform,
    linear::LinearRgbaView,
    warp::{
        ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricFitMode, PhotometricPolicy, WarpError,
        WarpPolicy, verify_filtered_photometric_white_anchored as verify,
        verify_reflected_filtered_photometric_white_anchored as reflected,
    },
};
use std::cell::Cell;

const ID: Transform = Transform {
    a: 1.0,
    b: 0.0,
    translation: [0.0; 2],
};
const FIT: PhotometricPolicy = PhotometricPolicy {
    residual: WarpPolicy {
        tolerance: 1e-6,
        max_source_pixels: 126,
    },
    minimum_samples: 8,
    minimum_variance: 1e-5,
    minimum_gain: 0.9,
    maximum_gain: 1.1,
    maximum_offset: 0.1,
};
const FILTER: ColorFilterPolicy = ColorFilterPolicy {
    filter: FilterPolicy {
        radius: 1,
        max_sample_pairs: 2268,
    },
    color_space: FilterColorSpace::LinearSrgb,
};
fn pixels() -> Vec<f32> {
    (0_u8..7)
        .flat_map(|y| {
            (0_u8..9).flat_map(move |x| {
                let a = f32::from(x) / 12.0 + 0.1;
                let b = f32::from(y) / 10.0 + 0.1;
                [a, b, a * b, 1.0]
            })
        })
        .collect()
}

#[test]
fn analytic_window_fit_and_reflection_keep_all_interior_samples() {
    let a = pixels();
    let b: Vec<_> = a
        .chunks_exact(4)
        .flat_map(|p| [0.98 * p[0] + 0.02, 0.99 * p[1] + 0.01, 1.01 * p[2] - 0.01, 1.0])
        .collect();
    let av = LinearRgbaView::new(9, 7, &a, 63, || false).unwrap();
    let bv = LinearRgbaView::new(9, 7, &b, 63, || false).unwrap();
    let direct = verify(&av, &bv, ID, FIT, FILTER, || false).unwrap();
    assert_eq!(direct.fit_mode, PhotometricFitMode::WhiteAnchoredLeastSquares);
    let mut mirror = vec![0.0; b.len()];
    for y in 0..7 {
        for x in 0..9 {
            mirror[(y * 9 + x) * 4..(y * 9 + x + 1) * 4]
                .copy_from_slice(&b[(y * 9 + 8 - x) * 4..(y * 9 + 9 - x) * 4]);
        }
    }
    let mv = LinearRgbaView::new(9, 7, &mirror, 63, || false).unwrap();
    let flipped = reflected(
        &av,
        &mv,
        Transform {
            translation: [8.0, 0.0],
            ..ID
        },
        FIT,
        FILTER,
        || false,
    )
    .unwrap();
    for evidence in [direct, flipped] {
        for model in [evidence.evidence.forward, evidence.evidence.reverse] {
            assert_eq!(model.fitted_samples, 35);
            assert_eq!(model.pixels.compared_pixels, 35);
            assert_eq!(model.pixels.matched_pixels, 35);
        }
    }
}

#[test]
fn exact_work_boundary_and_every_cancellation_checkpoint_are_explicit() {
    let a = pixels();
    let av = LinearRgbaView::new(9, 7, &a, 63, || false).unwrap();
    assert_eq!(
        verify(
            &av,
            &av,
            ID,
            FIT,
            ColorFilterPolicy {
                filter: FilterPolicy {
                    max_sample_pairs: 2267,
                    ..FILTER.filter
                },
                ..FILTER
            },
            || false
        ),
        Err(WarpError::Budget)
    );
    let count = Cell::new(0);
    let baseline = verify(&av, &av, ID, FIT, FILTER, || {
        count.set(count.get() + 1);
        false
    })
    .unwrap();
    for stop in 1..=count.get() {
        let calls = Cell::new(0);
        assert_eq!(
            verify(&av, &av, ID, FIT, FILTER, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(WarpError::Cancelled),
            "checkpoint {stop}"
        );
    }
    assert_eq!(verify(&av, &av, ID, FIT, FILTER, || false).unwrap(), baseline);
}
