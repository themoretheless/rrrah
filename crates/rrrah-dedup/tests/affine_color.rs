use rrrah_dedup::affine_color::*;
fn policy() -> AffineColorPolicy {
    AffineColorPolicy {
        minimum_samples: 4,
        maximum_samples: 1024,
        minimum_variance: 1e-6,
        minimum_relative_pivot: 1e-6,
        maximum_coefficient: 5.,
        maximum_offset: 0.1,
    }
}
fn truth() -> AffineColorModel {
    AffineColorModel {
        matrix: [[0.9, 0.06, 0.04], [0.03, 0.92, 0.05], [0.08, 0.04, 0.88]],
        offset: [0.01, -0.01, 0.02],
    }
}
fn pairs(values: &[f64]) -> Vec<RgbPair> {
    let mut output = Vec::new();
    for &a in values {
        for &b in values {
            for &c in values {
                let source = [a, b, c];
                output.push(RgbPair {
                    source,
                    target: truth().apply(source),
                });
            }
        }
    }
    output
}
#[test]
fn independent_mixed_channel_transform_and_disjoint_validation() {
    let training = pairs(&[0.1, 0.3, 0.6]);
    let held = pairs(&[0.2, 0.5]);
    assert!(training.iter().all(|a| held.iter().all(|b| a.source != b.source)));
    let fitted = fit_affine_color(&training, policy(), || false).unwrap();
    for i in 0..3 {
        assert!((fitted.offset[i] - truth().offset[i]).abs() < 1e-11);
        for j in 0..3 {
            assert!((fitted.matrix[i][j] - truth().matrix[i][j]).abs() < 1e-11);
        }
    }
    let evidence = validate_affine_color(fitted, &held, 0.03, 1024, || false).unwrap();
    assert_eq!(evidence.matched, 8);
    let negative: Vec<_> = held
        .iter()
        .map(|p| RgbPair {
            source: p.source,
            target: p.target.map(|v| 0.9 - v),
        })
        .collect();
    assert_eq!(
        validate_affine_color(fitted, &negative, 0.03, 1024, || false)
            .unwrap()
            .matched,
        0
    );
    let mut edited = held.clone();
    edited[3].target[1] += 0.1;
    assert_eq!(
        validate_affine_color(fitted, &edited, 0.03, 1024, || false)
            .unwrap()
            .matched,
        7
    );
}
#[test]
fn uniform_rank_deficient_and_target_singular_inputs_refuse() {
    let uniform = vec![
        RgbPair {
            source: [0.5; 3],
            target: [0.4; 3]
        };
        8
    ];
    assert_eq!(
        fit_affine_color(&uniform, policy(), || false),
        Err(AffineColorError::Uninformative)
    );
    let gray: Vec<_> = (1..9)
        .map(|i| {
            let v = f64::from(i) / 10.;
            RgbPair {
                source: [v; 3],
                target: [v; 3],
            }
        })
        .collect();
    assert_eq!(
        fit_affine_color(&gray, policy(), || false),
        Err(AffineColorError::IllConditioned)
    );
    let singular: Vec<_> = pairs(&[0.1, 0.3, 0.6])
        .iter()
        .map(|p| RgbPair {
            source: p.source,
            target: [p.source[0]; 3],
        })
        .collect();
    assert_eq!(
        fit_affine_color(&singular, policy(), || false),
        Err(AffineColorError::IllConditioned)
    );
}
#[test]
fn coefficient_offset_range_and_preflight_work_bounds() {
    let training = pairs(&[0.1, 0.3, 0.6]);
    let mut tight = policy();
    tight.maximum_coefficient = 0.5;
    assert_eq!(
        fit_affine_color(&training, tight, || false),
        Err(AffineColorError::Bounds)
    );
    tight = policy();
    tight.maximum_offset = 0.005;
    assert_eq!(
        fit_affine_color(&training, tight, || false),
        Err(AffineColorError::Bounds)
    );
    tight = policy();
    tight.maximum_samples = training.len() - 1;
    assert_eq!(
        fit_affine_color(&training, tight, || false),
        Err(AffineColorError::Budget)
    );
    let mut invalid = training.clone();
    invalid[0].source[0] = f64::NAN;
    assert_eq!(
        fit_affine_color(&invalid, policy(), || false),
        Err(AffineColorError::Invalid)
    );
    invalid[0].source[0] = -0.01;
    assert_eq!(
        fit_affine_color(&invalid, policy(), || false),
        Err(AffineColorError::Invalid)
    );
    assert_eq!(
        validate_affine_color(truth(), &training, 0.03, training.len() - 1, || false),
        Err(AffineColorError::Budget)
    );
}
#[test]
fn every_fit_and_validation_checkpoint_cancels_without_partial_evidence() {
    use std::cell::Cell;
    let training = pairs(&[0.1, 0.3, 0.6]);
    let calls = Cell::new(0);
    fit_affine_color(&training, policy(), || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let total = calls.get();
    for stop in 1..=total {
        let n = Cell::new(0);
        assert_eq!(
            fit_affine_color(&training, policy(), || {
                n.set(n.get() + 1);
                n.get() >= stop
            }),
            Err(AffineColorError::Cancelled)
        );
    }
    calls.set(0);
    validate_affine_color(truth(), &training, 0.03, 1024, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let total = calls.get();
    for stop in 1..=total {
        let n = Cell::new(0);
        assert_eq!(
            validate_affine_color(truth(), &training, 0.03, 1024, || {
                n.set(n.get() + 1);
                n.get() >= stop
            }),
            Err(AffineColorError::Cancelled)
        );
    }
}
