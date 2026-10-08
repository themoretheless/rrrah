use super::{DngColorMatrix, XYZ_WHITE_D65, bradford_adaptation_f64, invert_3x3_f64, multiply_3x3_f64};

fn kelvin(code: Option<u16>) -> Option<f64> {
    Some(match code? {
        3 | 17 => 2850.0,
        18 => 4874.0,
        19 => 6774.0,
        20 => 5500.0,
        1 | 4 | 9 | 10 | 21 => 6500.0,
        11 | 22 => 7500.0,
        23 => 5000.0,
        _ => return None,
    })
}

fn temperature(x: f64, y: f64) -> Option<f64> {
    let denominator = 1.5 - x + 6.0 * y;
    if !denominator.is_finite() || denominator <= 0.0 {
        return None;
    }
    let (u, v) = (2.0 * x / denominator, 3.0 * y / denominator);
    let mut previous = 0.0;
    for i in 1..ROBERTSON.len() {
        let [r, a, b, slope] = ROBERTSON[i];
        let distance = (v - b - slope * (u - a)) / (1.0 + slope * slope).sqrt();
        if distance <= 0.0 || i == ROBERTSON.len() - 1 {
            let fraction = if i == 1 {
                1.0
            } else {
                previous / (previous - distance.min(0.0))
            };
            let reciprocal = ROBERTSON[i - 1][0] * (1.0 - fraction) + r * fraction;
            return (reciprocal > 0.0).then(|| 1_000_000.0 / reciprocal);
        }
        previous = distance;
    }
    None
}

/// Resolve a three-channel dual-illuminant DNG ColorMatrix pair using the
/// specified camera neutral. Returns a D65-referenced matrix compatible with
/// the existing green-relative WB stage. No camera-calibration/forward-matrix
/// metadata is accepted by this API; callers must establish that contract.
pub fn resolve_dng_neutral_matrix(
    first: DngColorMatrix,
    second: DngColorMatrix,
    neutral: [f64; 3],
) -> Option<[[f64; 3]; 3]> {
    if neutral.iter().any(|v| !v.is_finite() || *v <= 0.0)
        || first
            .xyz_to_camera
            .iter()
            .flatten()
            .chain(second.xyz_to_camera.iter().flatten())
            .any(|v| !v.is_finite())
    {
        return None;
    }
    let (mut a, mut b) = (first, second);
    let (mut ta, mut tb) = (kelvin(a.illuminant)?, kelvin(b.illuminant)?);
    if ta > tb {
        std::mem::swap(&mut a, &mut b);
        std::mem::swap(&mut ta, &mut tb);
    }
    if ta == tb {
        return None;
    }
    let interpolate = |x: f64, y: f64| -> Option<[[f64; 3]; 3]> {
        let t = temperature(x, y)?;
        let weight = ((1.0 / t - 1.0 / tb) / (1.0 / ta - 1.0 / tb)).clamp(0.0, 1.0);
        Some(std::array::from_fn(|r| {
            std::array::from_fn(|c| a.xyz_to_camera[r][c] * weight + b.xyz_to_camera[r][c] * (1.0 - weight))
        }))
    };
    let mut xy = [0.34567, 0.35850];
    for pass in 0..30 {
        let inverse = invert_3x3_f64(interpolate(xy[0], xy[1])?)?;
        let xyz: [f64; 3] = std::array::from_fn(|r| (0..3).map(|c| inverse[r][c] * neutral[c]).sum());
        let sum: f64 = xyz.iter().sum();
        if sum <= 0.0 || xyz.iter().any(|v| !v.is_finite() || *v <= 0.0) {
            return None;
        }
        let mut next = [xyz[0] / sum, xyz[1] / sum];
        let converged = (next[0] - xy[0]).abs() + (next[1] - xy[1]).abs() < 1e-7;
        if pass == 29 {
            next = [(next[0] + xy[0]) / 2.0, (next[1] + xy[1]) / 2.0];
        }
        xy = next;
        if converged {
            break;
        }
    }
    let white = [xy[0] / xy[1], 1.0, (1.0 - xy[0] - xy[1]) / xy[1]];
    Some(multiply_3x3_f64(
        interpolate(xy[0], xy[1])?,
        bradford_adaptation_f64(XYZ_WHITE_D65, white)?,
    ))
}

// Robertson's tabulated reciprocal temperatures, CIE 1960 u/v and isotherm
// slopes. Numerical colorimetric data, not camera-specific tuning.
const ROBERTSON: [[f64; 4]; 31] = [
    [0., 0.18006, 0.26352, -0.24341],
    [10., 0.18066, 0.26589, -0.25479],
    [20., 0.18133, 0.26846, -0.26876],
    [30., 0.18208, 0.27119, -0.28539],
    [40., 0.18293, 0.27407, -0.30470],
    [50., 0.18388, 0.27709, -0.32675],
    [60., 0.18494, 0.28021, -0.35156],
    [70., 0.18611, 0.28342, -0.37915],
    [80., 0.18740, 0.28668, -0.40955],
    [90., 0.18880, 0.28997, -0.44278],
    [100., 0.19032, 0.29326, -0.47888],
    [125., 0.19462, 0.30141, -0.58204],
    [150., 0.19962, 0.30921, -0.70471],
    [175., 0.20525, 0.31647, -0.84901],
    [200., 0.21142, 0.32312, -1.0182],
    [225., 0.21807, 0.32909, -1.2168],
    [250., 0.22511, 0.33439, -1.4512],
    [275., 0.23247, 0.33904, -1.7298],
    [300., 0.24010, 0.34308, -2.0637],
    [325., 0.24702, 0.34655, -2.4681],
    [350., 0.25591, 0.34951, -2.9641],
    [375., 0.26400, 0.35200, -3.5814],
    [400., 0.27218, 0.35407, -4.3633],
    [425., 0.28039, 0.35577, -5.3762],
    [450., 0.28863, 0.35714, -6.7262],
    [475., 0.29685, 0.35823, -8.5955],
    [500., 0.30505, 0.35907, -11.324],
    [525., 0.31320, 0.35968, -15.628],
    [550., 0.32129, 0.36011, -23.325],
    [575., 0.32931, 0.36038, -40.770],
    [600., 0.33724, 0.36051, -116.45],
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hero9_matches_independent_adobe_matrix() {
        let a = DngColorMatrix {
            xyz_to_camera: [
                [1.8331, -0.8166, -0.2478],
                [0.1391, 0.8961, -0.0367],
                [0.0822, 0.0662, 0.2597],
            ],
            illuminant: Some(3),
        };
        let b = DngColorMatrix {
            xyz_to_camera: [
                [1.0344, -0.421, -0.062],
                [-0.2315, 1.0625, 0.1948],
                [0.0093, 0.1058, 0.5541],
            ],
            illuminant: Some(23),
        };
        let neutral = [0.463768, 1.0, 0.565121];
        let m = resolve_dng_neutral_matrix(a, b, neutral).unwrap();
        let converted = super::super::camera_to_linear_srgb_precise([m[0], m[1], m[2], [0.0; 3]]).unwrap();
        let expected = [
            [3.111112788461, -0.142492408390, -0.531465203539],
            [-0.573327995961, 1.582402514522, -0.560077110020],
            [-0.073554291016, -0.516786056631, 2.744364805178],
        ];
        let mut maximum = 0.0_f64;
        for r in 0..3 {
            for c in 0..3 {
                maximum = maximum.max((converted[r][c] / neutral[c] - expected[r][c]).abs());
            }
        }
        println!("maximum independent matrix difference: {maximum}");
        // Adobe's oracle uses a four-decimal PCS sRGB matrix. Compare
        // using that same output space before checking our precise sRGB path.
        let d50 = [0.3457 / 0.35850, 1.0, (1.0 - 0.3457 - 0.35850) / 0.35850];
        let pcs_to_camera = multiply_3x3_f64(m, bradford_adaptation_f64(d50, XYZ_WHITE_D65).unwrap());
        let camera_white: [f64; 3] =
            std::array::from_fn(|r| (0..3).map(|c| pcs_to_camera[r][c] * d50[c]).sum());
        let scale = camera_white.into_iter().fold(0.0_f64, f64::max);
        let camera_to_pcs = invert_3x3_f64(pcs_to_camera.map(|row| row.map(|v| v / scale))).unwrap();
        let mut pcs_srgb = [
            [0.4361, 0.3851, 0.1431],
            [0.2225, 0.7169, 0.0606],
            [0.0139, 0.0971, 0.7141],
        ];
        for r in 0..3 {
            let sum: f64 = pcs_srgb[r].iter().sum();
            for v in &mut pcs_srgb[r] {
                *v *= d50[r] / sum;
            }
        }
        let adobe_space = invert_3x3_f64(pcs_srgb).unwrap();
        let oracle_space = multiply_3x3_f64(adobe_space, camera_to_pcs);
        let same_space_error = (0..3)
            .flat_map(|r| (0..3).map(move |c| (oracle_space[r][c] - expected[r][c]).abs()))
            .fold(0.0_f64, f64::max);
        println!("same output space difference: {same_space_error}");
        assert!(same_space_error < 0.00001);
        assert!(maximum < 0.0005);
        assert_eq!(m, resolve_dng_neutral_matrix(b, a, neutral).unwrap());
        assert!(resolve_dng_neutral_matrix(a, b, [0.0, 1.0, 1.0]).is_none());
        assert!(resolve_dng_neutral_matrix(a, a, neutral).is_none());
    }
}
