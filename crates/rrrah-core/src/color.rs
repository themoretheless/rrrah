/// IEC 61966-2-1 / D65 linear sRGB to CIE XYZ matrix.
pub const SRGB_TO_XYZ_D65: [[f32; 3]; 3] = [
    [0.412_456_4, 0.357_576_1, 0.180_437_5],
    [0.212_672_9, 0.715_152_2, 0.072_175_0],
    [0.019_333_9, 0.119_192, 0.950_304_1],
];

/// f64 reference copy of [`SRGB_TO_XYZ_D65`] for profile construction.
pub const SRGB_TO_XYZ_D65_F64: [[f64; 3]; 3] = [
    [0.412_456_4, 0.357_576_1, 0.180_437_5],
    [0.212_672_9, 0.715_152_2, 0.072_175_0],
    [0.019_333_9, 0.119_192, 0.950_304_1],
];

/// Rec.709 luma weights used to keep WB exposure-neutral (`EDITOR_MATH.md`).
pub const WB_LUMINANCE_WEIGHTS: [f32; 3] = [0.2126, 0.7152, 0.0722];

/// Bradford cone-response matrix used for chromatic adaptation.
pub const BRADFORD: [[f32; 3]; 3] = [
    [0.8951, 0.2664, -0.1614],
    [-0.7502, 1.7135, 0.0367],
    [0.0389, -0.0685, 1.0296],
];

/// CIE D65 white point in XYZ with Y = 1 (xy 0.31271, 0.32902). The display
/// pipeline references every camera transform to this white point.
pub const XYZ_WHITE_D65: [f64; 3] = [0.95047, 1.0, 1.08883];

/// DNG/EXIF light source code for D65 (`CalibrationIlluminant` tags).
pub const DNG_ILLUMINANT_D65: u16 = 21;

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

/// Hue-preserving ACES tone map for a linear RGB triplet. Mirrors the WGSL
/// `aces_tone_map` in the GPU viewport shader (`raw_view.wgsl`):
///
/// 1. Sub-zero components (out-of-gamut camera colors) are desaturated toward
///    the achromatic axis (`WB_LUMINANCE_WEIGHTS` luma) until the lowest
///    channel reaches the gamut boundary; the hue ratio is kept, saturation is
///    reduced only as far as the negative excursion requires. If the luma axis
///    itself is non-positive, the color cannot be recovered and is clamped.
/// 2. The achromatic max-component norm is tone-mapped with [`aces_fitted`],
///    and the RGB triplet is scaled by `aces(norm) / norm` — one common
///    positive factor, so hue and channel ratios are preserved by
///    construction (CIELAB h° is exactly invariant under linear RGB scaling).
/// 3. Because the norm is the max component and `aces_fitted` never exceeds
///    1, the output stays inside the display gamut `[0, 1]` with no
///    post-tone-map clipping.
///
/// Achromatic inputs reduce to per-channel [`aces_fitted`]: `r == g == b == x`
/// maps to `aces_fitted(x)` per channel, so a gray ramp is unchanged.
pub fn aces_tone_map_rgb(rgb: [f32; 3]) -> [f32; 3] {
    if rgb.iter().any(|value| !value.is_finite()) {
        return [0.0; 3];
    }
    let mut color = rgb;
    let minimum = color.iter().copied().fold(f32::INFINITY, f32::min);
    if minimum < 0.0 {
        let luma = color[0] * WB_LUMINANCE_WEIGHTS[0]
            + color[1] * WB_LUMINANCE_WEIGHTS[1]
            + color[2] * WB_LUMINANCE_WEIGHTS[2];
        if luma > 0.0 {
            // Bring the lowest channel exactly to the gamut boundary:
            // min + t * (luma - min) = 0  →  t = -min / (luma - min).
            let desaturate = (-minimum / (luma - minimum)).clamp(0.0, 1.0);
            color = color.map(|channel| channel + desaturate * (luma - channel));
        } else {
            color = color.map(|channel| channel.max(0.0));
        }
    }
    let norm = color.iter().copied().fold(0.0_f32, f32::max);
    if norm <= 0.0 {
        return [0.0; 3];
    }
    let scale = aces_fitted(norm) / norm;
    color.map(|channel| channel * scale)
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

/// Reproduces the conventional dcraw-style camera-to-linear-sRGB transform.
///
/// Thin f32-compatible wrapper around [`camera_to_linear_srgb_precise`]: the
/// profile is built and inverted in f64, then downcast to f32 for uniform
/// upload. Unlike earlier versions, diverging G1/G2 color planes are refused
/// (`None`) instead of being silently reduced to the first three planes.
// The f64 -> f32 downcast is the intended uniform-upload narrowing.
#[allow(clippy::cast_possible_truncation)]
pub fn camera_to_linear_srgb(xyz_to_camera: [[f32; 3]; 4]) -> Option<[[f32; 3]; 3]> {
    let profile = xyz_to_camera.map(|row| row.map(f64::from));
    camera_to_linear_srgb_precise(profile)
        .ok()
        .map(|matrix| matrix.map(|row| row.map(|value| value as f32)))
}

/// Diagnostic status of the fourth (second green) color plane of a camera
/// profile. DNG-style profiles may carry four CFA planes where plane 1 and
/// plane 3 both describe green filters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GreenPlane {
    /// Plane 3 is all zeros: the profile carries no second green plane.
    Absent,
    /// Plane 3 matches plane 1 within [`GREEN_PLANE_RELATIVE_TOLERANCE`].
    Consistent,
    /// Plane 3 diverges from plane 1; payload is the max absolute element
    /// difference. Reducing such a profile to three planes would silently
    /// pick one of two different filter responses.
    Divergent(f64),
}

/// Relative tolerance used to decide whether G1/G2 planes encode the same
/// response (absolute tolerance is `tolerance * max(1, max|G1 element|)`).
pub const GREEN_PLANE_RELATIVE_TOLERANCE: f64 = 1.0e-3;

/// Compare camera plane 1 (G1) against plane 3 (G2).
pub fn diagnose_green_planes(xyz_to_camera: &[[f64; 3]; 4]) -> GreenPlane {
    let g1 = xyz_to_camera[1];
    let g2 = xyz_to_camera[3];
    if g2.iter().all(|value| *value == 0.0) {
        return GreenPlane::Absent;
    }
    let max_divergence = g1
        .iter()
        .zip(g2.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f64, f64::max);
    let scale = g1.iter().map(|v| v.abs()).fold(0.0_f64, f64::max).max(1.0);
    if max_divergence <= GREEN_PLANE_RELATIVE_TOLERANCE * scale {
        GreenPlane::Consistent
    } else {
        GreenPlane::Divergent(max_divergence)
    }
}

/// Why a camera calibration profile cannot produce a color transform.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum CameraProfileError {
    /// A coefficient is NaN or infinite.
    #[error("camera profile contains a non-finite coefficient")]
    NonFiniteCoefficient,
    /// A row of `XYZ_to_camera * sRGB_to_XYZ` sums to zero and cannot be
    /// normalized to preserve neutral camera RGB.
    #[error("camera profile row normalizes to zero")]
    DegenerateRow,
    /// The normalized matrix determinant is below `1e-8` (`EDITOR_MATH.md`).
    #[error("camera profile matrix is singular or near-singular (|det| < 1e-8)")]
    SingularMatrix,
    /// G1/G2 planes encode different filter responses; silent reduction to
    /// three planes is refused. Inspect the source profile and choose a plane.
    #[error("G1/G2 color planes diverge by {max_abs_divergence:.6} (tolerance {tolerance:.6})")]
    GreenPlaneMismatch {
        /// Max absolute element difference between plane 1 and plane 3.
        max_abs_divergence: f64,
        /// Tolerance that was exceeded.
        tolerance: f64,
    },
}

/// Builds the camera-to-linear-sRGB transform entirely in f64.
///
/// `xyz_to_camera` is reduced to its first three camera planes after an
/// explicit G1/G2 consistency check. Each row of
/// `XYZ_to_camera * linear_sRGB_to_XYZ` is normalized before inversion so that
/// neutral camera RGB remains neutral after the transform. Per
/// `EDITOR_MATH.md`, the determinant must stay above `1e-8` and all coefficients
/// must be finite. Downcast the result to f32 only at uniform upload.
pub fn camera_to_linear_srgb_precise(
    xyz_to_camera: [[f64; 3]; 4],
) -> Result<[[f64; 3]; 3], CameraProfileError> {
    if xyz_to_camera.iter().flatten().any(|v| !v.is_finite()) {
        return Err(CameraProfileError::NonFiniteCoefficient);
    }
    if let GreenPlane::Divergent(max_abs_divergence) = diagnose_green_planes(&xyz_to_camera) {
        let scale = xyz_to_camera[1]
            .iter()
            .map(|v| v.abs())
            .fold(0.0_f64, f64::max)
            .max(1.0);
        return Err(CameraProfileError::GreenPlaneMismatch {
            max_abs_divergence,
            tolerance: GREEN_PLANE_RELATIVE_TOLERANCE * scale,
        });
    }
    let xyz_to_camera = [xyz_to_camera[0], xyz_to_camera[1], xyz_to_camera[2]];
    let mut srgb_to_camera = multiply_3x3_f64(xyz_to_camera, SRGB_TO_XYZ_D65_F64);
    for row in &mut srgb_to_camera {
        let sum: f64 = row.iter().sum();
        if !sum.is_finite() || sum.abs() < 1.0e-8 {
            return Err(CameraProfileError::DegenerateRow);
        }
        for value in row {
            *value /= sum;
        }
    }
    invert_3x3_f64(srgb_to_camera).ok_or(CameraProfileError::SingularMatrix)
}

pub fn multiply_3x3_f64(left: [[f64; 3]; 3], right: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut output = [[0.0; 3]; 3];
    for row in 0..3 {
        for column in 0..3 {
            output[row][column] = (0..3).map(|index| left[row][index] * right[index][column]).sum();
        }
    }
    output
}

#[allow(clippy::many_single_char_names)]
pub fn invert_3x3_f64(matrix: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
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

/// Bradford adaptation from `source_white` to `destination_white` XYZ in f64.
///
/// Reference-precision counterpart of [`bradford_adaptation`] for building
/// camera profiles; downcast to f32 only at uniform upload.
pub fn bradford_adaptation_f64(source_white: [f64; 3], destination_white: [f64; 3]) -> Option<[[f64; 3]; 3]> {
    if source_white
        .iter()
        .chain(destination_white.iter())
        .any(|v| !v.is_finite() || *v <= 0.0)
    {
        return None;
    }
    let bradford = BRADFORD.map(|row| row.map(f64::from));
    let source_cone = apply_3x3_f64(bradford, source_white);
    let destination_cone = apply_3x3_f64(bradford, destination_white);
    if source_cone.iter().any(|v| !v.is_finite() || v.abs() < 1.0e-8) {
        return None;
    }
    let diagonal = [
        destination_cone[0] / source_cone[0],
        destination_cone[1] / source_cone[1],
        destination_cone[2] / source_cone[2],
    ];
    let inverse = invert_3x3_f64(bradford)?;
    Some(multiply_3x3_f64(
        multiply_3x3_f64(
            inverse,
            [
                [diagonal[0], 0.0, 0.0],
                [0.0, diagonal[1], 0.0],
                [0.0, 0.0, diagonal[2]],
            ],
        ),
        bradford,
    ))
}

fn apply_3x3_f64(matrix: [[f64; 3]; 3], value: [f64; 3]) -> [f64; 3] {
    [
        matrix[0][0] * value[0] + matrix[0][1] * value[1] + matrix[0][2] * value[2],
        matrix[1][0] * value[0] + matrix[1][1] * value[1] + matrix[1][2] * value[2],
        matrix[2][0] * value[0] + matrix[2][1] * value[1] + matrix[2][2] * value[2],
    ]
}

/// Converts xy chromaticity to an XYZ white point with Y = 1.
///
/// Returns `None` for non-finite input or a non-positive y.
pub fn xy_chromaticity_to_xyz(xy: [f64; 2]) -> Option<[f64; 3]> {
    let [x, y] = xy;
    if !x.is_finite() || !y.is_finite() || y <= 0.0 {
        return None;
    }
    Some([x / y, 1.0, (1.0 - x - y) / y])
}

/// XYZ white point (Y = 1) of a DNG `CalibrationIlluminant` light source code.
///
/// Covers the CIE-standardized illuminants (A, B, C, D50/D55/D65/D75) and the
/// EXIF aliases the DNG specification equates with daylight or tungsten.
/// Codes without a standardized chromaticity (the fluorescent family, ISO
/// studio tungsten, "other", unknown) return `None` so callers keep the
/// legacy un-adapted behavior instead of guessing a white point.
pub fn dng_illuminant_white(code: u16) -> Option<[f64; 3]> {
    // xy chromaticities per CIE 15:2004.
    let xy = match code {
        // Daylight, Flash, Fine weather, Cloudy weather, D65.
        1 | 4 | 9 | 10 | DNG_ILLUMINANT_D65 => [0.31271, 0.32902],
        // Tungsten (incandescent), Standard illuminant A.
        3 | 17 => [0.44757, 0.40745],
        // Shade, D75.
        11 | 22 => [0.29902, 0.31485],
        // Standard illuminant B.
        18 => [0.34842, 0.35161],
        // Standard illuminant C.
        19 => [0.31006, 0.31616],
        // D55.
        20 => [0.33242, 0.34743],
        // D50.
        23 => [0.34567, 0.35850],
        _ => return None,
    };
    xy_chromaticity_to_xyz(xy)
}

/// One DNG color matrix (camera-native rows by XYZ columns, as stored in
/// `ColorMatrix1`/`ColorMatrix2`) paired with its calibration illuminant code.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DngColorMatrix {
    pub xyz_to_camera: [[f64; 3]; 3],
    pub illuminant: Option<u16>,
}

/// Selects the D65-referenced `XYZ -> camera` matrix from a DNG calibration
/// pair (`first` = `ColorMatrix1`/`CalibrationIlluminant1`, `second` =
/// `ColorMatrix2`/`CalibrationIlluminant2`).
///
/// Selection order:
/// 1. A matrix already calibrated for D65 is used verbatim; the second matrix
///    is checked first because the DNG convention pairs `ColorMatrix2` with
///    the higher-CCT (usually D65) illuminant.
/// 2. A matrix calibrated for another known illuminant is Bradford-adapted to
///    D65: `camera = CM_illum * Bradford(D65 -> illum) * XYZ_D65`, again
///    preferring the second matrix.
/// 3. Without any known illuminant the legacy verbatim `ColorMatrix1`
///    behavior is kept (`ColorMatrix2` only as a last resort).
///
/// All math is f64; downcast to f32 only at uniform upload.
pub fn select_dng_xyz_to_camera(
    first: Option<DngColorMatrix>,
    second: Option<DngColorMatrix>,
) -> Option<[[f64; 3]; 3]> {
    for candidate in [second, first].into_iter().flatten() {
        if candidate.illuminant == Some(DNG_ILLUMINANT_D65) {
            return Some(candidate.xyz_to_camera);
        }
    }
    for candidate in [second, first].into_iter().flatten() {
        if let Some(white) = candidate.illuminant.and_then(dng_illuminant_white)
            && let Some(adaptation) = bradford_adaptation_f64(XYZ_WHITE_D65, white)
        {
            return Some(multiply_3x3_f64(candidate.xyz_to_camera, adaptation));
        }
    }
    first.or(second).map(|candidate| candidate.xyz_to_camera)
}

/// Converts raw camera WB multipliers (as shot, R/G/B) into green-relative
/// scene-linear gains `g_c = (WB_c / WB_G)^-1` per `EDITOR_MATH.md`.
///
/// Returns `None` for non-finite or non-positive multipliers.
pub fn green_relative_wb_gains(camera_wb: [f32; 3]) -> Option<[f32; 3]> {
    if camera_wb.iter().any(|v| !v.is_finite() || *v <= 0.0) {
        return None;
    }
    let green = camera_wb[1];
    Some(camera_wb.map(|channel| green / channel))
}

/// Rescales WB gains so their Rec.709 weighted luminance
/// (`0.2126*gR + 0.7152*gG + 0.0722*gB`) is exactly one, keeping WB
/// exposure-neutral. Pure function, ready for shader-side upload later.
///
/// Returns `None` for non-finite or non-positive gains.
pub fn luminance_normalize_wb_gains(gains: [f32; 3]) -> Option<[f32; 3]> {
    if gains.iter().any(|v| !v.is_finite() || *v <= 0.0) {
        return None;
    }
    let luminance = gains[0] * WB_LUMINANCE_WEIGHTS[0]
        + gains[1] * WB_LUMINANCE_WEIGHTS[1]
        + gains[2] * WB_LUMINANCE_WEIGHTS[2];
    if !luminance.is_finite() || luminance <= 0.0 {
        return None;
    }
    Some(gains.map(|gain| gain / luminance))
}

/// Full WB chain: camera multipliers to green-relative gains, then luminance
/// normalization so applying WB never shifts exposure.
pub fn display_wb_gains(camera_wb: [f32; 3]) -> Option<[f32; 3]> {
    luminance_normalize_wb_gains(green_relative_wb_gains(camera_wb)?)
}

#[cfg(test)]
// Tests intentionally narrow f64 references to f32 to compare both paths.
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use super::{
        CameraProfileError, DNG_ILLUMINANT_D65, DngColorMatrix, GreenPlane, SRGB_TO_XYZ_D65,
        SRGB_TO_XYZ_D65_F64, WB_LUMINANCE_WEIGHTS, XYZ_WHITE_D65, aces_fitted, aces_tone_map_rgb, apply_3x3, apply_exposure,
        bradford_adaptation, bradford_adaptation_f64, camera_to_linear_srgb, camera_to_linear_srgb_precise,
        diagnose_green_planes, display_wb_gains, dng_illuminant_white, green_relative_wb_gains, invert_3x3,
        invert_3x3_f64, luminance_normalize_wb_gains, multiply_3x3, multiply_3x3_f64,
        select_dng_xyz_to_camera, xy_chromaticity_to_xyz,
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
    fn aces_tone_map_rgb_reduces_to_per_channel_aces_on_grays() {
        for value in [0.0_f32, 0.02, 0.18, 0.5, 1.0, 4.0] {
            let mapped = aces_tone_map_rgb([value; 3]);
            let expected = aces_fitted(value);
            for channel in mapped {
                // One f32 divide/multiply round-trip away from the scalar
                // curve: far below the 8-bit output quantization.
                assert!(
                    (channel - expected).abs() <= expected.abs() * 1.0e-6 + 1.0e-9,
                    "gray {value}: {channel} vs scalar {expected}"
                );
            }
        }
    }

    #[test]
    fn aces_tone_map_rgb_preserves_channel_ratios_for_in_gamut_colors() {
        for color in [
            [1.0_f32, 0.5, 0.0],
            [0.25, 1.0, 0.5],
            [0.8, 0.1, 0.9],
            [2.5, 1.25, 0.75],
        ] {
            let mapped = aces_tone_map_rgb(color);
            let norm = color.iter().copied().fold(0.0_f32, f32::max);
            let expected_scale = aces_fitted(norm) / norm;
            for (mapped, original) in mapped.into_iter().zip(color) {
                let expected = original * expected_scale;
                assert!(
                    (mapped - expected).abs() <= expected.abs() * 1.0e-6 + 1.0e-9,
                    "{color:?}: {mapped} vs uniformly scaled {expected}"
                );
            }
            assert!(mapped.iter().all(|value| (0.0..=1.0).contains(value)));
        }
    }

    #[test]
    fn aces_tone_map_rgb_desaturates_negative_components_to_gamut() {
        // Out-of-gamut camera color: the blue channel is sub-zero. The mapper
        // must pull the color to the gamut boundary without zeroing the
        // channel outright (old clamp behavior) while keeping output bounded.
        let mapped = aces_tone_map_rgb([0.8, 0.4, -0.2]);
        let minimum = mapped.iter().copied().fold(f32::INFINITY, f32::min);
        assert!(minimum >= -1.0e-6, "negative channel survived: {mapped:?}");
        assert!(mapped.iter().all(|value| *value <= 1.0));
        assert!(mapped[2].abs() <= 1.0e-6, "desaturation lands exactly on the boundary: {mapped:?}");
        // Degenerate case: no recoverable achromatic axis falls back to clamp.
        assert_eq!(aces_tone_map_rgb([-0.5, -0.1, -0.2]), [0.0; 3]);
        assert_eq!(aces_tone_map_rgb([f32::NAN, 0.0, 0.0]), [0.0; 3]);
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

    fn realistic_profile(g2: [f64; 3]) -> [[f64; 3]; 4] {
        [
            [1.0507, -0.2546, -0.0893],
            [-0.4923, 1.3619, 0.1304],
            [-0.0584, 0.1073, 0.7525],
            g2,
        ]
    }

    #[test]
    fn f64_inverse_round_trip_meets_reference_contract() {
        let inverse = invert_3x3_f64(SRGB_TO_XYZ_D65_F64).expect("matrix is invertible");
        let product = multiply_3x3_f64(SRGB_TO_XYZ_D65_F64, inverse);
        let mut max_error = 0.0_f64;
        for (row_index, row) in product.iter().enumerate() {
            for (column_index, value) in row.iter().enumerate() {
                let expected = if row_index == column_index { 1.0 } else { 0.0 };
                max_error = max_error.max((value - expected).abs());
            }
        }
        assert!(max_error < 1.0e-10, "f64 round trip error {max_error:e}");
    }

    #[test]
    fn precise_transform_matches_f32_wrapper_on_healthy_profile() {
        let profile = realistic_profile([0.0; 3]);
        let precise = camera_to_linear_srgb_precise(profile).expect("healthy profile");
        let wrapped =
            camera_to_linear_srgb(profile.map(|row| row.map(|v| v as f32))).expect("healthy profile");
        for (precise, wrapped) in precise.into_iter().flatten().zip(wrapped.into_iter().flatten()) {
            assert!(
                (precise - f64::from(wrapped)).abs() < 1.0e-5,
                "{precise} != {wrapped}"
            );
        }
    }

    #[test]
    fn green_plane_diagnosis_classifies_absent_consistent_divergent() {
        let g1 = [-0.4923, 1.3619, 0.1304];
        assert_eq!(
            diagnose_green_planes(&realistic_profile([0.0; 3])),
            GreenPlane::Absent
        );
        assert_eq!(
            diagnose_green_planes(&realistic_profile(g1)),
            GreenPlane::Consistent
        );

        let mut noisy = g1;
        noisy[0] += 1.0e-4; // well below 1e-3 * scale
        assert_eq!(
            diagnose_green_planes(&realistic_profile(noisy)),
            GreenPlane::Consistent
        );

        let mut different = g1;
        different[1] *= 0.9; // a genuinely different green filter response
        match diagnose_green_planes(&realistic_profile(different)) {
            GreenPlane::Divergent(divergence) => assert!(divergence > 0.1),
            other => panic!("expected Divergent, got {other:?}"),
        }
    }

    #[test]
    fn divergent_green_planes_are_refused_not_silently_reduced() {
        let g1 = [-0.4923, 1.3619, 0.1304];
        let mut g2 = g1;
        g2[1] *= 0.9;
        let result = camera_to_linear_srgb_precise(realistic_profile(g2));
        match result {
            Err(CameraProfileError::GreenPlaneMismatch {
                max_abs_divergence,
                tolerance,
            }) => {
                assert!(max_abs_divergence > tolerance);
            }
            other => panic!("expected GreenPlaneMismatch, got {other:?}"),
        }
        // The legacy f32 wrapper refuses too instead of silently dropping G2.
        assert!(camera_to_linear_srgb(realistic_profile(g2).map(|row| row.map(|v| v as f32))).is_none());
    }

    #[test]
    fn consistent_green_planes_are_accepted() {
        let g1 = [-0.4923, 1.3619, 0.1304];
        assert!(camera_to_linear_srgb_precise(realistic_profile(g1)).is_ok());
    }

    #[test]
    fn precise_transform_reports_singular_and_non_finite() {
        let mut profile = realistic_profile([0.0; 3]);
        profile[2] = profile[0]; // duplicate row -> singular
        assert_eq!(
            camera_to_linear_srgb_precise(profile),
            Err(CameraProfileError::SingularMatrix)
        );

        let mut profile = realistic_profile([0.0; 3]);
        profile[0][1] = f64::NAN;
        assert_eq!(
            camera_to_linear_srgb_precise(profile),
            Err(CameraProfileError::NonFiniteCoefficient)
        );
    }

    #[test]
    fn bradford_f64_round_trips_white_point_to_reference_precision() {
        let d65 = [0.95047, 1.0, 1.08883];
        let d50 = [0.96422, 1.0, 0.82521];
        let adaptation = bradford_adaptation_f64(d65, d50).expect("valid white points");
        let adapted = apply_3x3(adaptation.map(|row| row.map(|v| v as f32)), d65.map(|v| v as f32));
        // f64-built adaptation maps the source white exactly onto the target.
        let adapted_f64 = {
            let m = adaptation;
            let v = d65;
            [
                m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
                m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
                m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
            ]
        };
        for (actual, expected) in adapted_f64.into_iter().zip(d50) {
            assert!((actual - expected).abs() < 1.0e-10, "{actual:e} != {expected:e}");
        }
        for (actual, expected) in adapted.into_iter().zip(d50.map(|v| v as f32)) {
            assert!((actual - expected).abs() < 2.0e-5);
        }
    }

    #[test]
    fn wb_gains_become_exposure_neutral_after_luminance_normalization() {
        let camera_wb = [1.9, 1.0, 1.4];
        let gains = green_relative_wb_gains(camera_wb).expect("valid multipliers");
        // g_c = (WB_c / WB_G)^-1: green stays 1, red/blue move above 1.
        assert!((gains[1] - 1.0).abs() < 1.0e-7);
        assert!(gains[0] < 1.0 && gains[2] < 1.0);

        let luminance_before = gains[0] * WB_LUMINANCE_WEIGHTS[0]
            + gains[1] * WB_LUMINANCE_WEIGHTS[1]
            + gains[2] * WB_LUMINANCE_WEIGHTS[2];
        let normalized = luminance_normalize_wb_gains(gains).expect("valid gains");
        let luminance_after = normalized[0] * WB_LUMINANCE_WEIGHTS[0]
            + normalized[1] * WB_LUMINANCE_WEIGHTS[1]
            + normalized[2] * WB_LUMINANCE_WEIGHTS[2];
        assert!((luminance_after - 1.0).abs() < 1.0e-6);
        for (before, after) in gains.into_iter().zip(normalized) {
            assert!((after - before / luminance_before).abs() < 1.0e-6);
        }

        // The combined helper must equal the two-step chain.
        let combined = display_wb_gains(camera_wb).expect("valid multipliers");
        for (combined, stepped) in combined.into_iter().zip(normalized) {
            assert!((combined - stepped).abs() < 1.0e-6);
        }
    }

    #[test]
    fn wb_gains_reject_invalid_input() {
        assert!(green_relative_wb_gains([0.0, 1.0, 1.0]).is_none());
        assert!(green_relative_wb_gains([1.0, f32::NAN, 1.0]).is_none());
        assert!(luminance_normalize_wb_gains([1.0, -1.0, 1.0]).is_none());
        assert!(luminance_normalize_wb_gains([1.0, 1.0, f32::INFINITY]).is_none());
    }

    /// Realistic `XYZ -> camera` matrix calibrated under illuminant A
    /// (tungsten), matching the corpus profile "camera B".
    fn illuminant_a_matrix() -> [[f64; 3]; 3] {
        [
            [0.6803, -0.1029, -0.1364],
            [-0.8513, 1.6292, 0.2445],
            [-0.1574, 0.3519, 0.5896],
        ]
    }

    fn apply_3x3_ref(matrix: [[f64; 3]; 3], value: [f64; 3]) -> [f64; 3] {
        [
            matrix[0][0] * value[0] + matrix[0][1] * value[1] + matrix[0][2] * value[2],
            matrix[1][0] * value[0] + matrix[1][1] * value[1] + matrix[1][2] * value[2],
            matrix[2][0] * value[0] + matrix[2][1] * value[1] + matrix[2][2] * value[2],
        ]
    }

    fn assert_matrix_close_f64(actual: [[f64; 3]; 3], expected: [[f64; 3]; 3], tolerance: f64) {
        for (actual, expected) in actual.into_iter().flatten().zip(expected.into_iter().flatten()) {
            assert!((actual - expected).abs() < tolerance, "{actual} != {expected}");
        }
    }

    #[test]
    fn illuminant_white_points_match_cie_chromaticities() {
        let d65 = dng_illuminant_white(DNG_ILLUMINANT_D65).expect("D65 is standardized");
        for (actual, expected) in d65.into_iter().zip(XYZ_WHITE_D65) {
            assert!((actual - expected).abs() < 1.0e-3, "{actual} != {expected}");
        }
        let standard_a = dng_illuminant_white(17).expect("illuminant A is standardized");
        // CIE A: xy (0.44757, 0.40745) -> XYZ (1.09847, 1, 0.35582).
        for (actual, expected) in standard_a.into_iter().zip([1.09847, 1.0, 0.35582]) {
            assert!((actual - expected).abs() < 1.0e-3, "{actual} != {expected}");
        }
        // Tungsten (3) aliases illuminant A; daylight aliases D65.
        assert_eq!(dng_illuminant_white(3), Some(standard_a));
        assert_eq!(dng_illuminant_white(1), Some(d65));
        // Codes without a standardized chromaticity stay unmapped.
        for code in [0, 2, 14, 24, 255] {
            assert_eq!(dng_illuminant_white(code), None);
        }
        assert_eq!(xy_chromaticity_to_xyz([0.3, 0.0]), None);
        assert_eq!(xy_chromaticity_to_xyz([f64::NAN, 0.3]), None);
    }

    #[test]
    fn d65_color_matrix_is_used_verbatim_without_adaptation() {
        let cm_a = illuminant_a_matrix();
        let cm_d65 = [
            [1.0507, -0.2546, -0.0893],
            [-0.4923, 1.3619, 0.1304],
            [-0.0584, 0.1073, 0.7525],
        ];
        // ColorMatrix2 calibrated for D65 wins verbatim over ColorMatrix1/A.
        let selected = select_dng_xyz_to_camera(
            Some(DngColorMatrix {
                xyz_to_camera: cm_a,
                illuminant: Some(17),
            }),
            Some(DngColorMatrix {
                xyz_to_camera: cm_d65,
                illuminant: Some(DNG_ILLUMINANT_D65),
            }),
        );
        assert_eq!(selected, Some(cm_d65));
        // ColorMatrix1 calibrated for D65 is verbatim too.
        let selected = select_dng_xyz_to_camera(
            Some(DngColorMatrix {
                xyz_to_camera: cm_a,
                illuminant: Some(DNG_ILLUMINANT_D65),
            }),
            None,
        );
        assert_eq!(selected, Some(cm_a));
    }

    #[test]
    fn illuminant_a_color_matrix_is_bradford_adapted_to_d65_reference() {
        let cm_a = illuminant_a_matrix();
        let selected = select_dng_xyz_to_camera(
            Some(DngColorMatrix {
                xyz_to_camera: cm_a,
                illuminant: Some(17),
            }),
            None,
        )
        .expect("illuminant A matrix must be adaptable");
        // Reference: CM_A * Bradford(D65 -> A), computed with an independent
        // implementation (numpy, CIE xy->XYZ white points). Tolerance covers
        // the f32 storage of the shared BRADFORD constant (~1e-8 per element,
        // amplified by the matrix inverse).
        let reference = [
            [0.815_030_534_3, -0.023_578_057_5, -0.142_567_505_8],
            [-0.791_628_272_9, 1.405_426_588_6, 0.117_618_770_4],
            [-0.151_635_898_1, 0.325_776_033_8, 0.190_244_484_5],
        ];
        assert_matrix_close_f64(selected, reference, 1.0e-6);
        // The adaptation is a real correction, not a near-identity no-op.
        let max_shift = selected
            .into_iter()
            .flatten()
            .zip(cm_a.into_iter().flatten())
            .map(|(adapted, raw)| (adapted - raw).abs())
            .fold(0.0_f64, f64::max);
        assert!(
            max_shift > 1.0e-2,
            "adaptation shift {max_shift:e} is implausibly small"
        );
    }

    #[test]
    fn adapted_matrix_maps_a_neutral_scene_to_d65_white() {
        let cm_a = illuminant_a_matrix();
        let a_white = dng_illuminant_white(17).expect("standardized");
        // A scene that is neutral under illuminant A: camera response through
        // the un-adapted matrix.
        let camera = apply_3x3_ref(cm_a, a_white);
        let selected = select_dng_xyz_to_camera(
            Some(DngColorMatrix {
                xyz_to_camera: cm_a,
                illuminant: Some(17),
            }),
            None,
        )
        .expect("adaptable");
        let camera_to_xyz = invert_3x3_f64(selected).expect("well conditioned");
        let xyz_d65 = apply_3x3_ref(camera_to_xyz, camera);
        for (actual, expected) in xyz_d65.into_iter().zip(XYZ_WHITE_D65) {
            assert!((actual - expected).abs() < 1.0e-9, "{actual:e} != {expected:e}");
        }
        // Without adaptation the same scene lands on the A white point,
        // visibly shifted from D65 (the bug this closes).
        let unadapted = apply_3x3_ref(invert_3x3_f64(cm_a).expect("well conditioned"), camera);
        let delta = unadapted
            .into_iter()
            .zip(XYZ_WHITE_D65)
            .map(|(actual, expected)| (actual - expected).abs())
            .fold(0.0_f64, f64::max);
        assert!(
            delta > 0.1,
            "un-adapted white should sit far from D65, delta {delta:e}"
        );
    }

    #[test]
    fn bradford_adaptation_composition_round_trips() {
        let a_white = dng_illuminant_white(17).expect("standardized");
        let forward = bradford_adaptation_f64(a_white, XYZ_WHITE_D65).expect("valid whites");
        let backward = bradford_adaptation_f64(XYZ_WHITE_D65, a_white).expect("valid whites");
        assert_matrix_close_f64(
            multiply_3x3_f64(forward, backward),
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            1.0e-10,
        );
    }

    #[test]
    fn unknown_illuminants_keep_legacy_verbatim_selection() {
        let cm_a = illuminant_a_matrix();
        // No illuminant tags at all: ColorMatrix1 verbatim (legacy behavior).
        let selected = select_dng_xyz_to_camera(
            Some(DngColorMatrix {
                xyz_to_camera: cm_a,
                illuminant: None,
            }),
            None,
        );
        assert_eq!(selected, Some(cm_a));
        // Unmapped codes (e.g. 255 "other") also keep the verbatim matrix.
        let cm_other = [[1.1, -0.2, -0.1], [-0.5, 1.4, 0.1], [-0.1, 0.1, 0.8]];
        let selected = select_dng_xyz_to_camera(
            Some(DngColorMatrix {
                xyz_to_camera: cm_a,
                illuminant: Some(255),
            }),
            Some(DngColorMatrix {
                xyz_to_camera: cm_other,
                illuminant: None,
            }),
        );
        assert_eq!(selected, Some(cm_a));
        // A known non-D65 illuminant on ColorMatrix2 is preferred over an
        // unmapped ColorMatrix1 and gets adapted.
        let selected = select_dng_xyz_to_camera(
            Some(DngColorMatrix {
                xyz_to_camera: cm_a,
                illuminant: None,
            }),
            Some(DngColorMatrix {
                xyz_to_camera: cm_other,
                illuminant: Some(17),
            }),
        )
        .expect("adaptable");
        let expected = multiply_3x3_f64(
            cm_other,
            bradford_adaptation_f64(XYZ_WHITE_D65, dng_illuminant_white(17).expect("standardized"))
                .expect("valid whites"),
        );
        assert_matrix_close_f64(selected, expected, 1.0e-12);
    }
}
