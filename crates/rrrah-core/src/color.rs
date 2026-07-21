/// IEC 61966-2-1 / D65 linear sRGB to CIE XYZ matrix.
pub const SRGB_TO_XYZ_D65: [[f32; 3]; 3] = [
    [0.412_456_4, 0.357_576_1, 0.180_437_5],
    [0.212_672_9, 0.715_152_2, 0.072_175_0],
    [0.019_333_9, 0.119_192, 0.950_304_1],
];

/// Bradford cone-response matrix used for chromatic adaptation.
pub const BRADFORD: [[f32; 3]; 3] = [
    [0.8951, 0.2664, -0.1614],
    [-0.7502, 1.7135, 0.0367],
    [0.0389, -0.0685, 1.0296],
];

/// Apply a 3x3 matrix to an RGB/XYZ vector.
pub fn apply_3x3(matrix: [[f32; 3]; 3], value: [f32; 3]) -> [f32; 3] {
    [
        matrix[0][0] * value[0] + matrix[0][1] * value[1] + matrix[0][2] * value[2],
        matrix[1][0] * value[0] + matrix[1][1] * value[1] + matrix[1][2] * value[2],
        matrix[2][0] * value[0] + matrix[2][1] * value[1] + matrix[2][2] * value[2],
    ]
}

/// Bradford adaptation from `source_white` to `destination_white` XYZ.
///
/// The function rejects non-finite or non-positive white points and singular
/// calibration matrices. It is evaluated in f32 at runtime; callers building
/// camera profiles should calculate a f64 reference and compare against this.
pub fn bradford_adaptation(source_white: [f32; 3], destination_white: [f32; 3]) -> Option<[[f32; 3]; 3]> {
    if source_white
        .iter()
        .chain(destination_white.iter())
        .any(|v| !v.is_finite() || *v <= 0.0)
    {
        return None;
    }
    let source_cone = apply_3x3(BRADFORD, source_white);
    let destination_cone = apply_3x3(BRADFORD, destination_white);
    if source_cone.iter().any(|v| !v.is_finite() || v.abs() < 1.0e-8) {
        return None;
    }
    let diagonal = [
        destination_cone[0] / source_cone[0],
        destination_cone[1] / source_cone[1],
        destination_cone[2] / source_cone[2],
    ];
    let inverse = invert_3x3(BRADFORD)?;
    Some(multiply_3x3(
        multiply_3x3(
            inverse,
            [
                [diagonal[0], 0.0, 0.0],
                [0.0, diagonal[1], 0.0],
                [0.0, 0.0, diagonal[2]],
            ],
        ),
        BRADFORD,
    ))
}

/// Scene-linear exposure in photographic stops. Non-finite values are not
/// propagated; callers should validate the parameter before dispatch.
pub fn apply_exposure(rgb: [f32; 3], stops: f32) -> [f32; 3] {
    if !stops.is_finite() {
        return rgb;
    }
    let gain = 2.0_f32.powf(stops);
    rgb.map(|channel| channel * gain)
}

/// ACES fitted tone map for preview. Input and output are non-negative linear
/// values; the final display transfer function is intentionally separate.
pub fn aces_fitted(value: f32) -> f32 {
    if !value.is_finite() {
        return 0.0;
    }
    let x = value.max(0.0);
    (x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
}

pub fn multiply_3x3(left: [[f32; 3]; 3], right: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut output = [[0.0; 3]; 3];
    for row in 0..3 {
        for column in 0..3 {
            output[row][column] = (0..3).map(|index| left[row][index] * right[index][column]).sum();
        }
    }
    output
}

#[allow(clippy::many_single_char_names)]
pub fn invert_3x3(matrix: [[f32; 3]; 3]) -> Option<[[f32; 3]; 3]> {
    let [[a, b, c], [d, e, f], [g, h, i]] = matrix;
    let determinant = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    if !determinant.is_finite() || determinant.abs() < 1.0e-8 {
        return None;
    }
    let inverse_det = determinant.recip();
    Some([
        [
            (e * i - f * h) * inverse_det,
            (c * h - b * i) * inverse_det,
            (b * f - c * e) * inverse_det,
        ],
        [
            (f * g - d * i) * inverse_det,
            (a * i - c * g) * inverse_det,
            (c * d - a * f) * inverse_det,
        ],
        [
            (d * h - e * g) * inverse_det,
            (b * g - a * h) * inverse_det,
            (a * e - b * d) * inverse_det,
        ],
    ])
}

/// Reproduces the conventional dcraw/rawler camera-to-linear-sRGB transform.
///
/// `xyz_to_camera` is reduced to its first three camera planes. Each row of
/// `XYZ_to_camera * linear_sRGB_to_XYZ` is normalized before inversion so that
/// neutral camera RGB remains neutral after the transform.
pub fn camera_to_linear_srgb(xyz_to_camera: [[f32; 3]; 4]) -> Option<[[f32; 3]; 3]> {
    let xyz_to_camera = [xyz_to_camera[0], xyz_to_camera[1], xyz_to_camera[2]];
    let mut srgb_to_camera = multiply_3x3(xyz_to_camera, SRGB_TO_XYZ_D65);
    for row in &mut srgb_to_camera {
        let sum: f32 = row.iter().sum();
        if !sum.is_finite() || sum.abs() < 1.0e-8 {
            return None;
        }
        for value in row {
            *value /= sum;
        }
    }
    invert_3x3(srgb_to_camera)
}

#[cfg(test)]
mod tests {
    use super::{
        SRGB_TO_XYZ_D65, aces_fitted, apply_3x3, apply_exposure, bradford_adaptation, camera_to_linear_srgb,
        invert_3x3, multiply_3x3,
    };

    fn assert_matrix_close(actual: [[f32; 3]; 3], expected: [[f32; 3]; 3]) {
        for (actual, expected) in actual.into_iter().flatten().zip(expected.into_iter().flatten()) {
            assert!((actual - expected).abs() < 2.0e-5, "{actual} != {expected}");
        }
    }

    #[test]
    fn inverse_round_trip_is_identity() {
        let inverse = invert_3x3(SRGB_TO_XYZ_D65).expect("matrix is invertible");
        assert_matrix_close(
            multiply_3x3(SRGB_TO_XYZ_D65, inverse),
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        );
    }

    #[test]
    fn inverse_srgb_matrix_yields_identity_camera_transform() {
        let inverse = invert_3x3(SRGB_TO_XYZ_D65).expect("matrix is invertible");
        let xyz_to_camera = [inverse[0], inverse[1], inverse[2], [0.0; 3]];
        let transform = camera_to_linear_srgb(xyz_to_camera).expect("transform exists");
        assert_matrix_close(transform, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    }

    #[test]
    fn rejects_singular_calibration() {
        assert!(camera_to_linear_srgb([[0.0; 3]; 4]).is_none());
    }

    #[test]
    fn exposure_is_exactly_one_stop_for_finite_signal() {
        let output = apply_exposure([0.125, 0.18, 0.5], 1.0);
        for (actual, expected) in output.into_iter().zip([0.25, 0.36, 1.0]) {
            assert!((actual - expected).abs() < 1.0e-6);
        }
    }

    #[test]
    fn exposure_does_not_propagate_non_finite_control() {
        let input = [0.125, 0.18, 0.5];
        for stops in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(
                apply_exposure(input, stops)
                    .into_iter()
                    .zip(input)
                    .all(|(actual, expected)| actual.to_bits() == expected.to_bits())
            );
        }
    }

    #[test]
    fn aces_is_bounded_and_monotonic_on_reference_range() {
        let values = [0.0, 0.01, 0.18, 1.0, 10.0, 100.0];
        let mapped = values.map(aces_fitted);
        for pair in mapped.windows(2) {
            assert!(pair[1] >= pair[0]);
        }
        assert!(mapped.iter().all(|value| (0.0..=1.0).contains(value)));
        assert!(aces_fitted(f32::NAN).abs() < f32::EPSILON);
        assert!(aces_fitted(-1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn bradford_preserves_equal_white_point() {
        let d65 = [0.95047, 1.0, 1.08883];
        let adaptation = bradford_adaptation(d65, d65).expect("D65 is valid");
        let adapted = apply_3x3(adaptation, d65);
        for (actual, expected) in adapted.into_iter().zip(d65) {
            assert!((actual - expected).abs() < 2.0e-5);
        }
    }

    #[test]
    fn bradford_rejects_invalid_white_point() {
        assert!(bradford_adaptation([0.0, 1.0, 1.0], [0.95, 1.0, 1.09]).is_none());
    }

    #[test]
    fn camera_transform_rejects_non_finite_and_near_singular_profiles() {
        let mut profile = [[0.0; 3]; 4];
        profile[0][0] = f32::NAN;
        assert!(camera_to_linear_srgb(profile).is_none());

        let profile = [[1.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, 1.0], [0.0; 3]];
        assert!(camera_to_linear_srgb(profile).is_none());
    }
}
