//! Parametric precision corpus: f32 vs f64 matrix inversion.
//!
//! Prints a comparison table (run with `--nocapture`) that feeds
//! docs/experiments/d.md. Round-trip error is `max |M * M^-1 - I|` evaluated
//! in f64 for both paths so the f32 path is measured against true arithmetic.

use rrrah_core::{
    BRADFORD, SRGB_TO_XYZ_D65_F64, camera_to_linear_srgb, camera_to_linear_srgb_precise,
    display_wb_gains, invert_3x3, invert_3x3_f64, multiply_3x3_f64,
};

const IDENTITY: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn round_trip_error_f64(matrix: [[f64; 3]; 3], inverse: [[f64; 3]; 3]) -> f64 {
    let product = multiply_3x3_f64(matrix, inverse);
    let mut error = 0.0_f64;
    for row in 0..3 {
        for column in 0..3 {
            error = error.max((product[row][column] - IDENTITY[row][column]).abs());
        }
    }
    error
}

#[allow(clippy::many_single_char_names)]
fn determinant_f64(m: [[f64; 3]; 3]) -> f64 {
    let [[a, b, c], [d, e, f], [g, h, i]] = m;
    a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g)
}

struct Case {
    name: String,
    matrix: [[f64; 3]; 3],
    well_conditioned: bool,
}

/// Realistic `XYZ->camera` calibration matrices in the style of dcraw / Adobe
/// DNG `ColorMatrix` coefficients (mixed signs, row sums near one).
fn realistic_camera_matrices() -> Vec<(&'static str, [[f64; 3]; 3])> {
    vec![
        (
            "camera A (DSLR daylight)",
            [
                [1.0507, -0.2546, -0.0893],
                [-0.4923, 1.3619, 0.1304],
                [-0.0584, 0.1073, 0.7525],
            ],
        ),
        (
            "camera B (mirrorless tungsten)",
            [
                [0.6803, -0.1029, -0.1364],
                [-0.8513, 1.6292, 0.2445],
                [-0.1574, 0.3519, 0.5896],
            ],
        ),
        (
            "camera C (compact, wide gamut)",
            [
                [1.3982, -0.5441, -0.0741],
                [-0.3088, 1.0985, 0.1902],
                [0.0217, -0.0412, 0.8811],
            ],
        ),
        (
            "camera D (Foveon-like, strong cross-talk)",
            [
                [1.8491, -1.1379, 0.2200],
                [-0.4566, 1.8270, -0.3387],
                [0.1035, -0.3798, 1.2116],
            ],
        ),
    ]
}

/// Near-singular family: rows r1, r2, 0.6*r1 + 0.4*r2 + eps * n. The base is
/// rank 2, so the condition number grows like 1/eps.
fn near_singular(eps: f64) -> [[f64; 3]; 3] {
    let r1 = [0.9216, -0.2127, -0.0518];
    let r2 = [-0.3854, 1.1805, 0.1479];
    let n = [1.0, -0.7, 0.4];
    let mut r3 = [0.0; 3];
    for k in 0..3 {
        r3[k] = 0.6f64.mul_add(r1[k], 0.4 * r2[k]) + eps * n[k];
    }
    [r1, r2, r3]
}

fn corpus() -> Vec<Case> {
    let mut cases = vec![
        Case {
            name: "sRGB->XYZ D65".to_string(),
            matrix: SRGB_TO_XYZ_D65_F64,
            well_conditioned: true,
        },
        Case {
            name: "Bradford cone response".to_string(),
            matrix: BRADFORD.map(|row| row.map(f64::from)),
            well_conditioned: true,
        },
    ];
    for (name, matrix) in realistic_camera_matrices() {
        cases.push(Case {
            name: name.to_string(),
            matrix,
            well_conditioned: true,
        });
    }
    for exponent in 3..=8 {
        let eps = 10.0_f64.powi(-exponent);
        cases.push(Case {
            name: format!("near-singular eps=1e-{exponent}"),
            matrix: near_singular(eps),
            well_conditioned: false,
        });
    }
    cases
}

#[test]
// The corpus deliberately narrows f64 references to f32 to compare both paths.
#[allow(clippy::cast_possible_truncation)]
fn f32_vs_f64_inversion_precision_corpus() {
    println!();
    println!(
        "{:<42} {:>10} {:>12} {:>12} {:>12}",
        "case", "|det|", "f32 roundtrip", "f64 roundtrip", "f32 > 1e-5?"
    );
    let mut f32_failures = 0_u32;
    let mut f64_failures = 0_u32;
    for case in corpus() {
        let det = determinant_f64(case.matrix).abs();

        let f32_matrix = case.matrix.map(|row| row.map(|v| v as f32));
        let f32_result = invert_3x3(f32_matrix).map(|inverse_f32| {
            let matrix_f64 = f32_matrix.map(|row| row.map(f64::from));
            let inverse_f64 = inverse_f32.map(|row| row.map(f64::from));
            round_trip_error_f64(matrix_f64, inverse_f64)
        });
        let f64_result = invert_3x3_f64(case.matrix).map(|inverse| round_trip_error_f64(case.matrix, inverse));

        let f32_text = f32_result.map_or_else(|| "rejected".to_string(), |e| format!("{e:.3e}"));
        let f64_text = f64_result.map_or_else(|| "rejected".to_string(), |e| format!("{e:.3e}"));
        let f32_exceeds = f32_result.is_some_and(|e| e > 1.0e-5);
        println!(
            "{:<42} {:>10.3e} {:>12} {:>12} {:>12}",
            case.name,
            det,
            f32_text,
            f64_text,
            if f32_exceeds { "YES" } else { "" }
        );

        if case.well_conditioned {
            // f64 reference must always meet the < 1e-10 contract on healthy
            // matrices; f32 is recorded but only the f64 path is gated here.
            let f64_error = f64_result.expect("f64 path must invert well-conditioned matrices");
            assert!(
                f64_error < 1.0e-10,
                "f64 round trip {} on {} exceeds 1e-10",
                f64_error,
                case.name
            );
            if f32_exceeds {
                f32_failures += 1;
            }
        } else if let (Some(f32_error), Some(f64_error)) = (f32_result, f64_result) {
            // Hypothesis: on near-singular matrices the f32 path degrades far
            // faster than the f64 path.
            assert!(
                f64_error < f32_error,
                "f64 path ({f64_error:e}) should beat f32 path ({f32_error:e}) on {}",
                case.name
            );
            if f32_exceeds {
                f32_failures += 1;
            }
            if f64_error > 1.0e-10 {
                f64_failures += 1;
            }
        }
    }
    println!("f32 round-trip > 1e-5 on {f32_failures} cases; f64 > 1e-10 on {f64_failures} near-singular cases");
}

#[test]
fn wb_without_luminance_normalization_shifts_exposure() {
    println!();
    println!("{:<24} {:>14} {:>14}", "camera WB (as shot)", "luma shift", "stops");
    let samples = [
        ("tungsten [1.9, 1.0, 1.4]", [1.9, 1.0, 1.4]),
        ("shade [2.3, 1.0, 1.2]", [2.3, 1.0, 1.2]),
        ("fluorescent [1.5, 1.0, 1.8]", [1.5, 1.0, 1.8]),
        ("daylight [2.0, 1.0, 1.6]", [2.0, 1.0, 1.6]),
    ];
    for (name, camera_wb) in samples {
        let gains = display_wb_gains(camera_wb).expect("valid multipliers");
        // display_wb_gains includes luminance normalization; recompute the
        // un-normalized green-relative gains to show the exposure shift.
        let raw = camera_wb.map(|c| camera_wb[1] / c);
        let luma = raw[0] * 0.2126 + raw[1] * 0.7152 + raw[2] * 0.0722;
        let stops = luma.log2();
        println!("{name:<24} {luma:>14.4} {stops:>+14.3}");
        assert!(stops.abs() > 0.05, "corpus sample should show a visible shift");
        // Normalized gains undo exactly that shift.
        let normalized_luma = gains[0] * 0.2126 + gains[1] * 0.7152 + gains[2] * 0.0722;
        assert!((normalized_luma - 1.0).abs() < 1.0e-6);
    }
}

#[test]
// Comparing f32 uniforms against the f64 reference requires narrowing casts.
#[allow(clippy::cast_possible_truncation)]
fn precise_and_f32_camera_transforms_agree_on_realistic_profiles() {
    for (name, xyz_to_camera) in realistic_camera_matrices() {
        let profile = [
            xyz_to_camera[0],
            xyz_to_camera[1],
            xyz_to_camera[2],
            [0.0; 3],
        ];
        let precise = camera_to_linear_srgb_precise(profile).expect(name);
        let legacy = camera_to_linear_srgb(profile.map(|row| row.map(|v| v as f32))).expect(name);
        let mut max_diff = 0.0_f64;
        for row in 0..3 {
            for column in 0..3 {
                max_diff = max_diff.max((precise[row][column] - f64::from(legacy[row][column])).abs());
            }
        }
        println!("camera transform f64-vs-f32 max element diff on {name}: {max_diff:.3e}");
        // f32 uniform downcast error alone is ~1e-7 per element; allow margin.
        assert!(max_diff < 1.0e-5, "{name}: {max_diff:e}");
    }
}

// Silence unused warning for the constant when assertions change.
const _: [[f64; 3]; 3] = IDENTITY;
