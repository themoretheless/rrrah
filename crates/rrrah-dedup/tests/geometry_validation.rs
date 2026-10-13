use rrrah_dedup::{
    geometry::{Correspondence, GeometryError, ProjectiveTransform},
    geometry_validation::{ProjectiveValidationPolicy, validate_projective_points},
};
use std::cell::Cell;

const IDENTITY: ProjectiveTransform = ProjectiveTransform {
    matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
};
fn points(xy: &[[f64; 2]]) -> Vec<Correspondence> {
    xy.iter()
        .map(|&source| Correspondence {
            source,
            target: source,
        })
        .collect()
}
fn fitting() -> Vec<Correspondence> {
    points(&[[2., 2.], [90., 2.], [2., 90.], [90., 90.]])
}
fn validation() -> Vec<Correspondence> {
    points(&[[20., 20.], [40., 20.], [20., 40.], [40., 40.]])
}
fn policy() -> ProjectiveValidationPolicy {
    ProjectiveValidationPolicy {
        maximum_fitting_points: 4,
        maximum_validation_points: 4,
        maximum_separation_checks: 28,
        maximum_projection_checks: 8,
        minimum_endpoint_separation: 1.,
        source_tolerance: 0.1,
        target_tolerance: 0.1,
    }
}
#[test]
fn unchanged_model_scores_only_declared_validation_and_retains_outliers() {
    let mut fitting = fitting();
    // These endpoints intentionally do not support identity. The scorer must
    // not fit to them, replace the caller model, or select validation inliers.
    for point in &mut fitting {
        point.target[0] += 3.;
    }
    let mut validation = validation();
    validation[3].target[0] += 5.;
    let e = validate_projective_points(
        IDENTITY,
        &fitting,
        &validation,
        (100, 100),
        (100, 100),
        policy(),
        || false,
    )
    .unwrap();
    assert_eq!(
        (
            e.validation_points,
            e.forward_inliers,
            e.reverse_inliers,
            e.joint_inliers
        ),
        (4, 3, 3, 3)
    );
    assert_eq!((e.maximum_forward_error, e.maximum_reverse_error), (5., 5.));
    assert_eq!((e.separation_checks, e.projection_checks), (28, 8));
}
#[test]
fn scale_exposes_distinct_forward_reverse_and_joint_evidence() {
    let model = ProjectiveTransform {
        matrix: [[10., 0., 0.], [0., 10., 0.], [0., 0., 1.]],
    };
    let mut train = fitting();
    let mut held = validation();
    for point in train.iter_mut().chain(&mut held) {
        point.target = point.source.map(|v| v * 10.);
    }
    held[0].target[0] += 0.5;
    let e = validate_projective_points(model, &train, &held, (100, 100), (1000, 1000), policy(), || false)
        .unwrap();
    assert_eq!((e.forward_inliers, e.reverse_inliers, e.joint_inliers), (3, 4, 3));
    assert!((e.maximum_reverse_error - 0.05).abs() < 1e-12);
    assert_eq!(e.maximum_forward_error, 0.5);
}
#[test]
fn aliases_in_either_domain_and_either_set_are_refused() {
    for target in [false, true] {
        for within_validation in [false, true] {
            let train = fitting();
            let mut held = validation();
            let duplicate = if within_validation { held[1] } else { train[0] };
            if target {
                held[0].target = duplicate.target;
            } else {
                held[0].source = duplicate.source;
            }
            assert_eq!(
                validate_projective_points(IDENTITY, &train, &held, (100, 100), (100, 100), policy(), || {
                    false
                }),
                Err(GeometryError::Invalid)
            );
        }
        let mut train = fitting();
        let held = validation();
        if target {
            train[1].target = train[0].target;
        } else {
            train[1].source = train[0].source;
        }
        assert_eq!(
            validate_projective_points(IDENTITY, &train, &held, (100, 100), (100, 100), policy(), || {
                false
            }),
            Err(GeometryError::Invalid)
        );
    }
    let train = fitting();
    let mut held = validation();
    held[0].source = [3., 2.];
    assert_eq!(
        validate_projective_points(IDENTITY, &train, &held, (100, 100), (100, 100), policy(), || {
            false
        }),
        Err(GeometryError::Invalid)
    );
    held[0].source = [3.000001, 2.];
    assert!(
        validate_projective_points(IDENTITY, &train, &held, (100, 100), (100, 100), policy(), || {
            false
        })
        .is_ok()
    );
}
#[test]
fn invalid_endpoints_domains_models_and_policy_never_publish_evidence() {
    for held_set in [false, true] {
        for target in [false, true] {
            for axis in 0..2 {
                for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1., 100.] {
                    let mut train = fitting();
                    let mut held = validation();
                    let point = if held_set { &mut held[0] } else { &mut train[0] };
                    if target {
                        point.target[axis] = invalid;
                    } else {
                        point.source[axis] = invalid;
                    }
                    assert_eq!(
                        validate_projective_points(
                            IDENTITY,
                            &train,
                            &held,
                            (100, 100),
                            (100, 100),
                            policy(),
                            || false
                        ),
                        Err(GeometryError::Invalid)
                    );
                }
            }
        }
    }
    for (source, target) in [((0, 100), (100, 100)), ((100, 100), (100, 0))] {
        assert_eq!(
            validate_projective_points(
                IDENTITY,
                &fitting(),
                &validation(),
                source,
                target,
                policy(),
                || false
            ),
            Err(GeometryError::Invalid)
        );
    }
    for matrix in [
        [[0.; 3]; 3],
        [[1., 0., 0.], [0., 1., 0.], [0.02, 0., -1.]],
        [[f64::NAN, 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    ] {
        assert_eq!(
            validate_projective_points(
                ProjectiveTransform { matrix },
                &fitting(),
                &validation(),
                (100, 100),
                (100, 100),
                policy(),
                || false
            ),
            Err(GeometryError::Invalid)
        );
    }
    for field in 0..3 {
        for invalid in [0., -1., f64::NAN, f64::INFINITY] {
            let mut p = policy();
            match field {
                0 => p.source_tolerance = invalid,
                1 => p.target_tolerance = invalid,
                _ => p.minimum_endpoint_separation = invalid,
            };
            assert_eq!(
                validate_projective_points(
                    IDENTITY,
                    &fitting(),
                    &validation(),
                    (100, 100),
                    (100, 100),
                    p,
                    || false
                ),
                Err(GeometryError::Invalid)
            );
        }
    }
    assert_eq!(
        validate_projective_points(
            IDENTITY,
            &fitting()[..3],
            &validation(),
            (100, 100),
            (100, 100),
            policy(),
            || false
        ),
        Err(GeometryError::Invalid)
    );
    assert_eq!(
        validate_projective_points(
            IDENTITY,
            &fitting(),
            &[],
            (100, 100),
            (100, 100),
            policy(),
            || false
        ),
        Err(GeometryError::Invalid)
    );
}
#[test]
fn exact_work_limits_are_prepaid_and_one_less_refuses() {
    assert!(
        validate_projective_points(
            IDENTITY,
            &fitting(),
            &validation(),
            (100, 100),
            (100, 100),
            policy(),
            || false
        )
        .is_ok()
    );
    for field in 0..4 {
        let mut p = policy();
        match field {
            0 => p.maximum_fitting_points -= 1,
            1 => p.maximum_validation_points -= 1,
            2 => p.maximum_separation_checks -= 1,
            _ => p.maximum_projection_checks -= 1,
        };
        assert_eq!(
            validate_projective_points(
                IDENTITY,
                &fitting(),
                &validation(),
                (100, 100),
                (100, 100),
                p,
                || false
            ),
            Err(GeometryError::Budget)
        );
    }
}
#[test]
fn every_one_shot_cancellation_checkpoint_refuses_and_retry_is_identical() {
    let train = fitting();
    let held = validation();
    let checks = Cell::new(0);
    let expected =
        validate_projective_points(IDENTITY, &train, &held, (100, 100), (100, 100), policy(), || {
            checks.set(checks.get() + 1);
            false
        })
        .unwrap();
    assert!(checks.get() > 28);
    for checkpoint in 1..=checks.get() {
        let calls = Cell::new(0);
        assert_eq!(
            validate_projective_points(IDENTITY, &train, &held, (100, 100), (100, 100), policy(), || {
                calls.set(calls.get() + 1);
                calls.get() == checkpoint
            }),
            Err(GeometryError::Cancelled)
        );
    }
    assert_eq!(
        validate_projective_points(IDENTITY, &train, &held, (100, 100), (100, 100), policy(), || {
            false
        })
        .unwrap(),
        expected
    );
}

fn fitting_policy() -> rrrah_dedup::geometry::JointProjectivePolicy {
    rrrah_dedup::geometry::JointProjectivePolicy {
        target: rrrah_dedup::geometry::GeometryPolicy {
            tolerance: 0.001,
            min_inliers: 4,
            max_points: 4,
            max_hypotheses: 8,
        },
        source_tolerance: 0.001,
        maximum_projection_checks: 72,
    }
}
fn sampling() -> rrrah_dedup::geometry::ProjectiveSamplingPolicy {
    rrrah_dedup::geometry::ProjectiveSamplingPolicy { trials: 8, seed: 173 }
}
#[test]
fn fitting_never_uses_validation_to_select_refine_or_replace_the_training_model() {
    use rrrah_dedup::{
        geometry::verify_projective_sampled_joint_for_domains,
        geometry_validation::fit_and_validate_projective_points,
    };
    let train = fitting();
    let held = validation();
    let direct = verify_projective_sampled_joint_for_domains(
        &train,
        fitting_policy(),
        sampling(),
        (100, 100),
        (100, 100),
        || false,
    )
    .unwrap()
    .unwrap();
    let good = fit_and_validate_projective_points(
        &train,
        &held,
        (100, 100),
        (100, 100),
        fitting_policy(),
        sampling(),
        policy(),
        || false,
    )
    .unwrap()
    .unwrap();
    let mut wrong = held.clone();
    for p in &mut wrong {
        p.target[0] += 11.;
    }
    let bad = fit_and_validate_projective_points(
        &train,
        &wrong,
        (100, 100),
        (100, 100),
        fitting_policy(),
        sampling(),
        policy(),
        || false,
    )
    .unwrap()
    .unwrap();
    for candidate in [&good, &bad] {
        assert_eq!(candidate.fitting.transform, direct.transform);
        assert_eq!(candidate.fitting.inliers, direct.inliers);
        assert_eq!(
            candidate.fitting.normalized_squared_error,
            direct.normalized_squared_error
        );
        assert_eq!(candidate.fitting.hypotheses, direct.hypotheses);
        assert_eq!(candidate.validation.validation_points, 4);
    }
    assert_eq!(good.validation.joint_inliers, 4);
    assert_eq!(
        (
            bad.validation.forward_inliers,
            bad.validation.reverse_inliers,
            bad.validation.joint_inliers
        ),
        (0, 0, 0)
    );
    assert!((bad.validation.maximum_forward_error - 11.).abs() < 1e-8);
}
#[test]
fn malformed_validation_is_refused_even_when_training_has_no_model() {
    use rrrah_dedup::geometry_validation::fit_and_validate_projective_points;
    let train = points(&[[2., 2.], [4., 4.], [6., 6.], [8., 8.]]);
    let mut held = validation();
    assert!(
        fit_and_validate_projective_points(
            &train,
            &held,
            (100, 100),
            (100, 100),
            fitting_policy(),
            sampling(),
            policy(),
            || false
        )
        .unwrap()
        .is_none()
    );
    held[0].source[0] = f64::NAN;
    assert!(matches!(
        fit_and_validate_projective_points(
            &train,
            &held,
            (100, 100),
            (100, 100),
            fitting_policy(),
            sampling(),
            policy(),
            || false
        ),
        Err(GeometryError::Invalid)
    ));
}
#[test]
fn coupled_fit_and_validation_enforce_both_work_budgets() {
    use rrrah_dedup::geometry_validation::fit_and_validate_projective_points;
    let train = fitting();
    let held = validation();
    let mut fit = fitting_policy();
    fit.maximum_projection_checks -= 1;
    assert!(matches!(
        fit_and_validate_projective_points(
            &train,
            &held,
            (100, 100),
            (100, 100),
            fit,
            sampling(),
            policy(),
            || false
        ),
        Err(GeometryError::Budget)
    ));
    let mut validation_policy = policy();
    validation_policy.maximum_projection_checks -= 1;
    assert!(matches!(
        fit_and_validate_projective_points(
            &train,
            &held,
            (100, 100),
            (100, 100),
            fitting_policy(),
            sampling(),
            validation_policy,
            || false
        ),
        Err(GeometryError::Budget)
    ));
}
#[test]
fn coupled_fit_one_shot_cancellation_covers_training_and_validation_then_retry() {
    use rrrah_dedup::geometry_validation::fit_and_validate_projective_points;
    let train = fitting();
    let held = validation();
    let checks = Cell::new(0);
    let expected = fit_and_validate_projective_points(
        &train,
        &held,
        (100, 100),
        (100, 100),
        fitting_policy(),
        sampling(),
        policy(),
        || {
            checks.set(checks.get() + 1);
            false
        },
    )
    .unwrap()
    .unwrap();
    assert!(checks.get() > 100);
    for checkpoint in 1..=checks.get() {
        let calls = Cell::new(0);
        assert!(matches!(
            fit_and_validate_projective_points(
                &train,
                &held,
                (100, 100),
                (100, 100),
                fitting_policy(),
                sampling(),
                policy(),
                || {
                    calls.set(calls.get() + 1);
                    calls.get() == checkpoint
                }
            ),
            Err(GeometryError::Cancelled)
        ));
    }
    let retry = fit_and_validate_projective_points(
        &train,
        &held,
        (100, 100),
        (100, 100),
        fitting_policy(),
        sampling(),
        policy(),
        || false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(retry.fitting.transform, expected.fitting.transform);
    assert_eq!(retry.fitting.inliers, expected.fitting.inliers);
    assert_eq!(retry.validation, expected.validation);
}
