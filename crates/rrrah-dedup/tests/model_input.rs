#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    linear::LinearRgbaView,
    model_input::{ModelInputError, ModelInputPolicy, prepare_gray_model_input},
};
use std::cell::Cell;
const POLICY: ModelInputPolicy = ModelInputPolicy {
    max_side: 16,
    max_source_pixels: 1000,
    max_output_pixels: 1000,
    max_pixel_reads: 10000,
};
fn samples() -> Vec<f32> {
    (0..11)
        .flat_map(|y| {
            (0..17).flat_map(move |x| {
                let v = if (x + 2 * y) % 3 == 0 { 1. } else { 0. };
                [v, v, v, 1.]
            })
        })
        .collect()
}

#[test]
fn nonzero_odd_crop_matches_independent_continuous_area_and_keeps_full_source_cap() {
    use rrrah_dedup::model_input::prepare_gray_model_region_input;
    let mut data = samples();
    // Valid HDR outside the inference crop must never be read by this recipe.
    data[0] = 8.;
    let image = LinearRgbaView::new(17, 11, &data, 1000, || false).unwrap();
    let budget = MemoryBudget::new(64);
    let input = prepare_gray_model_region_input(&image, [3, 2, 11, 7], POLICY, &budget, || false).unwrap();
    assert_eq!(input.dimensions, [8, 8]);
    assert_eq!(input.original, [11, 7]);
    for y in 0..8 {
        for x in 0..8 {
            let left = 3. + f64::from(x) * 11. / 8.;
            let right = 3. + f64::from(x + 1) * 11. / 8.;
            let top = 2. + f64::from(y) * 7. / 8.;
            let bottom = 2. + f64::from(y + 1) * 7. / 8.;
            let mut white = 0.;
            for sy in 2..9 {
                for sx in 3..14 {
                    if (sx + 2 * sy) % 3 == 0 {
                        white += (right.min(f64::from(sx + 1)) - left.max(f64::from(sx))).max(0.)
                            * (bottom.min(f64::from(sy + 1)) - top.max(f64::from(sy))).max(0.);
                    }
                }
            }
            assert_eq!(
                input.pixels[(8 * y + x) as usize],
                (white / ((right - left) * (bottom - top)) * 255.).round() as u8
            );
        }
    }
    let clone = input.clone();
    drop(input);
    assert_eq!(budget.used(), 64);
    drop(clone);
    assert_eq!(budget.used(), 0);
    assert_eq!(
        prepare_gray_model_region_input(
            &image,
            [3, 2, 11, 7],
            ModelInputPolicy {
                max_source_pixels: 186,
                ..POLICY
            },
            &budget,
            || false
        )
        .unwrap_err(),
        ModelInputError::Budget
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn crop_bounds_overflow_and_every_cancellation_checkpoint_release_payload() {
    use rrrah_dedup::model_input::prepare_gray_model_region_input;
    let data = samples();
    let image = LinearRgbaView::new(17, 11, &data, 1000, || false).unwrap();
    let budget = MemoryBudget::new(64);
    for rectangle in [[0, 0, 0, 7], [3, 2, 15, 7], [3, 8, 11, 4], [u32::MAX, 0, 2, 7]] {
        assert_eq!(
            prepare_gray_model_region_input(&image, rectangle, POLICY, &budget, || false).unwrap_err(),
            ModelInputError::Invalid
        );
        assert_eq!((budget.used(), budget.peak()), (0, 0));
    }
    let calls = Cell::new(0);
    drop(
        prepare_gray_model_region_input(&image, [3, 2, 11, 7], POLICY, &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap(),
    );
    for at in 1..=calls.get() {
        let current = Cell::new(0);
        assert_eq!(
            prepare_gray_model_region_input(&image, [3, 2, 11, 7], POLICY, &budget, || {
                current.set(current.get() + 1);
                current.get() == at
            })
            .unwrap_err(),
            ModelInputError::Cancelled
        );
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn odd_edges_match_independent_continuous_area_reference_and_credit_tracks_owners() {
    let samples = samples();
    let image = LinearRgbaView::new(17, 11, &samples, 1000, || false).unwrap();
    let budget = MemoryBudget::new(128);
    let output = prepare_gray_model_input(&image, POLICY, &budget, || false).unwrap();
    assert_eq!(output.dimensions, [16, 8]);
    assert_eq!(output.original, [17, 11]);
    assert_eq!(budget.used(), 128);
    for y in 0..8 {
        for x in 0..16 {
            let (left, right) = (f64::from(x) * 17. / 16., f64::from(x + 1) * 17. / 16.);
            let (top, bottom) = (f64::from(y) * 11. / 8., f64::from(y + 1) * 11. / 8.);
            let mut white = 0.;
            for sy in 0..11 {
                for sx in 0..17 {
                    if (sx + 2 * sy) % 3 == 0 {
                        white += (right.min(f64::from(sx + 1)) - left.max(f64::from(sx))).max(0.)
                            * (bottom.min(f64::from(sy + 1)) - top.max(f64::from(sy))).max(0.);
                    }
                }
            }
            let expected = (white / ((right - left) * (bottom - top)) * 255.).round() as u8;
            assert_eq!(output.pixels[(y * 16 + x) as usize], expected, "{x},{y}");
        }
    }
    let clone = output.clone();
    drop(output);
    assert_eq!(budget.used(), 128);
    drop(clone);
    assert_eq!(budget.used(), 0);
}
#[test]
fn every_one_shot_cancellation_checkpoint_releases_all_output_credit() {
    let samples = samples();
    let image = LinearRgbaView::new(17, 11, &samples, 1000, || false).unwrap();
    let budget = MemoryBudget::new(128);
    let calls = Cell::new(0);
    drop(
        prepare_gray_model_input(&image, POLICY, &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap(),
    );
    assert_eq!(budget.used(), 0);
    for at in 1..=calls.get() {
        let current = Cell::new(0);
        assert!(
            matches!(
                prepare_gray_model_input(&image, POLICY, &budget, || {
                    current.set(current.get() + 1);
                    current.get() == at
                }),
                Err(ModelInputError::Cancelled)
            ),
            "{at}"
        );
        assert_eq!(current.get(), at);
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn invalid_caps_and_unsupported_pixels_refuse_atomically() {
    let samples = samples();
    let image = LinearRgbaView::new(17, 11, &samples, 1000, || false).unwrap();
    let budget = MemoryBudget::new(128);
    for policy in [
        ModelInputPolicy {
            max_source_pixels: 186,
            ..POLICY
        },
        ModelInputPolicy {
            max_output_pixels: 127,
            ..POLICY
        },
        ModelInputPolicy {
            max_pixel_reads: 0,
            ..POLICY
        },
    ] {
        assert!(matches!(
            prepare_gray_model_input(&image, policy, &budget, || false),
            Err(ModelInputError::Budget)
        ));
        assert_eq!(budget.used(), 0);
    }
    let short = MemoryBudget::new(127);
    assert!(matches!(
        prepare_gray_model_input(&image, POLICY, &short, || false),
        Err(ModelInputError::Budget)
    ));
    assert_eq!(short.used(), 0);
    assert!(matches!(
        prepare_gray_model_input(
            &image,
            ModelInputPolicy {
                max_side: 7,
                ..POLICY
            },
            &budget,
            || false
        ),
        Err(ModelInputError::Invalid)
    ));
    for (channel, value) in [(3, 0.5), (0, 2.), (0, -0.1)] {
        let mut samples = samples.clone();
        samples[(17 * 11 - 1) * 4 + channel] = value;
        let image = LinearRgbaView::new(17, 11, &samples, 1000, || false).unwrap();
        assert!(matches!(
            prepare_gray_model_input(&image, POLICY, &budget, || false),
            Err(ModelInputError::UnsupportedPixels)
        ));
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn tiny_inputs_are_explicitly_upsampled_and_linear_gray_is_srgb_encoded() {
    let samples = [0.21404114, 0.21404114, 0.21404114, 1.];
    let image = LinearRgbaView::new(1, 1, &samples, 1, || false).unwrap();
    let budget = MemoryBudget::new(64);
    let output = prepare_gray_model_input(&image, POLICY, &budget, || false).unwrap();
    assert_eq!(output.dimensions, [8, 8]);
    assert!(output.pixels.iter().all(|v| *v == 128));
    drop(output);
    assert_eq!(budget.used(), 0);
}

#[test]
fn projected_hdr_alpha_odd_area_matches_independent_reference_and_credit() {
    use rrrah_dedup::{
        model_input::prepare_gray_model_input_with_projection, model_projection::ModelInputProjection,
    };
    let data: Vec<f32> = (0..11)
        .flat_map(|y| {
            (0..17).flat_map(move |x| {
                if (x + 2 * y) % 3 == 0 {
                    [4., 4., 4., 0.25]
                } else {
                    [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.]
                }
            })
        })
        .collect();
    let view = LinearRgbaView::new(17, 11, &data, 1000, || false).unwrap();
    let budget = MemoryBudget::new(128);
    assert_eq!(
        prepare_gray_model_input(&view, POLICY, &budget, || false).unwrap_err(),
        ModelInputError::UnsupportedPixels
    );
    let projection = ModelInputProjection::CompositeReinhardV1 {
        exposure: 1.,
        background: [0.; 3],
    };
    let input =
        prepare_gray_model_input_with_projection(&view, POLICY, projection, &budget, || false).unwrap();
    assert_eq!(input.dimensions, [16, 8]);
    assert_eq!(budget.used(), 128);
    assert_eq!(budget.peak(), 128);
    let encoded = 1.055 * 0.5_f64.powf(1. / 2.4) - 0.055;
    for y in 0..8 {
        for x in 0..16 {
            let (l, r) = (f64::from(x) * 17. / 16., f64::from(x + 1) * 17. / 16.);
            let (t, b) = (f64::from(y) * 11. / 8., f64::from(y + 1) * 11. / 8.);
            let mut area = 0.;
            for sy in 0..11 {
                for sx in 0..17 {
                    if (sx + 2 * sy) % 3 == 0 {
                        area += (r.min(f64::from(sx + 1)) - l.max(f64::from(sx))).max(0.)
                            * (b.min(f64::from(sy + 1)) - t.max(f64::from(sy))).max(0.);
                    }
                }
            }
            assert_eq!(
                input.pixels[(y * 16 + x) as usize],
                (encoded * area / ((r - l) * (b - t)) * 255.).round() as u8
            );
        }
    }
    assert_eq!(view.rgba(0, 0).unwrap(), [4., 4., 4., 0.25]);
    let copy = input.clone();
    drop(input);
    assert_eq!(budget.used(), 128);
    drop(copy);
    assert_eq!(budget.used(), 0);
}

#[test]
fn projected_preflight_every_cancellation_and_memory_refusal_release_payload() {
    use rrrah_dedup::{
        model_input::prepare_gray_model_input_with_projection, model_projection::ModelInputProjection,
    };
    let data = vec![4., 4., 4., 0.5].repeat(64);
    let view = LinearRgbaView::new(8, 8, &data, 64, || false).unwrap();
    let projection = ModelInputProjection::CompositeReinhardV1 {
        exposure: 1.,
        background: [0.; 3],
    };
    let budget = MemoryBudget::new(64);
    let calls = Cell::new(0);
    drop(
        prepare_gray_model_input_with_projection(&view, POLICY, projection, &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap(),
    );
    for stop in 1..=calls.get() {
        let at = Cell::new(0);
        assert_eq!(
            prepare_gray_model_input_with_projection(&view, POLICY, projection, &budget, || {
                at.set(at.get() + 1);
                at.get() == stop
            })
            .unwrap_err(),
            ModelInputError::Cancelled
        );
        assert_eq!(budget.used(), 0);
    }
    let short = MemoryBudget::new(63);
    assert_eq!(
        prepare_gray_model_input_with_projection(&view, POLICY, projection, &short, || false).unwrap_err(),
        ModelInputError::Budget
    );
    assert_eq!((short.used(), short.peak()), (0, 0));
    for exposure in [0., -1., f64::INFINITY, f64::NAN] {
        let fresh = MemoryBudget::new(64);
        let invalid = ModelInputProjection::CompositeReinhardV1 {
            exposure,
            background: [0.; 3],
        };
        assert_eq!(
            prepare_gray_model_input_with_projection(&view, POLICY, invalid, &fresh, || false).unwrap_err(),
            ModelInputError::Invalid
        );
        assert_eq!((fresh.used(), fresh.peak()), (0, 0));
    }
}

#[test]
fn projected_collisions_are_rejected_by_original_rgba_warp() {
    use rrrah_dedup::{
        geometry::ProjectiveTransform,
        model_input::prepare_gray_model_input_with_projection,
        model_projection::ModelInputProjection,
        warp::{WarpPolicy, verify_projective_bidirectional},
    };
    let projection = ModelInputProjection::CompositeReinhardV1 {
        exposure: 1.,
        background: [0.; 3],
    };
    for (a, b) in [
        ([0., 0., 0., 0.25], [0., 0., 0., 1.]),
        ([4., 4., 4., 1.], [4.01, 4.01, 4.01, 1.]),
        ([-1., -1., -1., 1.], [-2., -2., -2., 1.]),
    ] {
        let left = a.repeat(64);
        let right = b.repeat(64);
        let left = LinearRgbaView::new(8, 8, &left, 64, || false).unwrap();
        let right = LinearRgbaView::new(8, 8, &right, 64, || false).unwrap();
        let budget = MemoryBudget::new(128);
        let li =
            prepare_gray_model_input_with_projection(&left, POLICY, projection, &budget, || false).unwrap();
        let ri =
            prepare_gray_model_input_with_projection(&right, POLICY, projection, &budget, || false).unwrap();
        assert_eq!(&*li.pixels, &*ri.pixels);
        let proof = verify_projective_bidirectional(
            &left,
            &right,
            ProjectiveTransform {
                matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
            },
            WarpPolicy {
                tolerance: 0.,
                max_source_pixels: 128,
            },
            || false,
        )
        .unwrap();
        assert_eq!(proof.forward.compared_pixels, 64);
        assert_eq!(proof.reverse.compared_pixels, 64);
        assert_eq!(proof.forward.matched_pixels, 0);
        assert_eq!(proof.reverse.matched_pixels, 0);
        drop((li, ri));
        assert_eq!(budget.used(), 0);
    }
}
