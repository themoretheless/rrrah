//! Matrix construction from declared linear RGB chromaticities.
use super::{apply_3x3_f64, bradford_adaptation_f64, invert_3x3_f64, multiply_3x3_f64};

/// Relative-colorimetric linear RGB -> linear sRGB/D65 matrix from
/// `[xr, yr, xg, yg, xb, yb, xw, yw]`. Bradford adapts a different source white.
/// No transfer curve, exposure normalization, clipping or tone mapping is applied.
/// Imaginary primaries are allowed (e.g. ACES AP0); white must be physical.
/// Singular/non-finite bases and white points are refused. Uses only stack storage.
pub fn linear_rgb_to_srgb_from_xy(xy: [f64; 8]) -> Option<[[f64; 3]; 3]> {
    linear_rgb_to_srgb_with_adopted_neutral_from_xy(xy, [xy[6], xy[7]])
}

/// Convert a declared RGB basis while adapting the independently declared
/// adopted neutral. The basis remains normalized to its original white;
/// replacing that white would change the meaning of the source RGB samples.
/// The adopted neutral is a physical xy coordinate with XYZ Y normalized to 1.
pub fn linear_rgb_to_srgb_with_adopted_neutral_from_xy(
    xy: [f64; 8],
    adopted_neutral: [f64; 2],
) -> Option<[[f64; 3]; 3]> {
    let (source, _) = rgb_to_xyz(xy)?;
    let [x, y] = adopted_neutral;
    if !x.is_finite() || !y.is_finite() || x <= 0.0 || y <= 0.0 || x + y >= 1.0 {
        return None;
    }
    let white = [x / y, 1.0, (1.0 - x - y) / y];
    let (destination, d65) = rgb_to_xyz([0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290])?;
    let adapted = if white == d65 {
        source
    } else {
        multiply_3x3_f64(bradford_adaptation_f64(white, d65)?, source)
    };
    let matrix = multiply_3x3_f64(invert_3x3_f64(destination)?, adapted);
    matrix.iter().flatten().all(|v| v.is_finite()).then_some(matrix)
}

fn rgb_to_xyz(xy: [f64; 8]) -> Option<([[f64; 3]; 3], [f64; 3])> {
    if xy.iter().any(|v| !v.is_finite()) || xy[6] <= 0.0 || xy[7] <= 0.0 || xy[6] + xy[7] >= 1.0 {
        return None;
    }
    let mut basis = [[0.0; 3]; 3];
    for c in 0..3 {
        let (x, y) = (xy[c * 2], xy[c * 2 + 1]);
        // Unnormalized XYZ basis also supports primaries with y=0 (XYZ axes).
        basis[0][c] = x;
        basis[1][c] = y;
        basis[2][c] = 1.0 - x - y;
    }
    let white = [xy[6] / xy[7], 1.0, (1.0 - xy[6] - xy[7]) / xy[7]];
    let scale = apply_3x3_f64(invert_3x3_f64(basis)?, white);
    for row in &mut basis {
        for c in 0..3 {
            row[c] *= scale[c];
        }
    }
    // Zero scale collapses a primary even when its unscaled basis is invertible.
    invert_3x3_f64(basis)?;
    Some((basis, white))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_d65_primaries_match_independently_derived_matrices() {
        for (xy, red) in [
            (
                [0.708, 0.292, 0.170, 0.797, 0.131, 0.046, 0.3127, 0.3290],
                [1.6604910021084345, -0.12455047452159074, -0.018150763354905303],
            ),
            (
                [0.680, 0.320, 0.265, 0.690, 0.150, 0.060, 0.3127, 0.3290],
                [1.2249401762805596, -0.04205695470968818, -0.019637554590334436],
            ),
        ] {
            let matrix = linear_rgb_to_srgb_from_xy(xy).unwrap();
            for c in 0..3 {
                assert!((matrix[c][0] - red[c]).abs() < 1e-12);
            }
            let neutral = apply_3x3_f64(matrix, [1.0; 3]);
            for value in neutral {
                assert!((value - 1.0).abs() < 1e-12);
            }
        }
    }
    #[test]
    fn other_white_and_imaginary_primaries_remain_neutral() {
        for xy in [
            [0.713, 0.293, 0.165, 0.830, 0.128, 0.044, 0.32168, 0.33767],
            [0.7347, 0.2653, 0.0, 1.0, 0.0001, -0.0770, 0.32168, 0.33767],
            [1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.3127, 0.3290],
        ] {
            let m = linear_rgb_to_srgb_from_xy(xy).unwrap();
            for value in apply_3x3_f64(m, [1.0; 3]) {
                assert!((value - 1.0).abs() < 1e-12);
            }
            assert!(m.iter().flatten().any(|v| *v < 0.0));
        }
    }
    #[test]
    fn adopted_neutral_preserves_source_basis_and_matches_independent_oracle() {
        for (xy, neutral, reference) in [
            (
                [0.64, 0.33, 0.3, 0.6, 0.15, 0.06, 0.3127, 0.329],
                [0.3333, 0.3333],
                [
                    [0.9340649422558872, -0.12377150261792225, -0.008817569255978652],
                    [0.006091541552094074, 1.0474378754627227, -0.0006772374432571365],
                    [0.002842437298447322, 0.0064507192574145555, 1.0896827943019614],
                ],
            ),
            (
                [0.713, 0.293, 0.165, 0.83, 0.128, 0.044, 0.32168, 0.33767],
                [0.34567, 0.3585],
                [
                    [1.6387969704738357, -0.6721022997867034, -0.09305369763523613],
                    [-0.12694001225172752, 1.1606039662379029, -0.016202014152961075],
                    [-0.02250413142204623, -0.138990854898769, 1.418717700493445],
                ],
            ),
        ] {
            let matrix = linear_rgb_to_srgb_with_adopted_neutral_from_xy(xy, neutral).unwrap();
            for (actual, expected) in matrix.iter().flatten().zip(reference.iter().flatten()) {
                assert!((actual - expected).abs() < 1e-12);
            }
            let (basis, _) = rgb_to_xyz(xy).unwrap();
            let [x, y] = neutral;
            let neutral_rgb = apply_3x3_f64(invert_3x3_f64(basis).unwrap(), [x / y, 1.0, (1.0 - x - y) / y]);
            for value in apply_3x3_f64(matrix, neutral_rgb) {
                assert!((value - 1.0).abs() < 1e-12);
            }
            let mut incorrectly_renormalized = xy;
            incorrectly_renormalized[6] = neutral[0];
            incorrectly_renormalized[7] = neutral[1];
            let wrong = linear_rgb_to_srgb_from_xy(incorrectly_renormalized).unwrap();
            assert!(
                matrix
                    .iter()
                    .flatten()
                    .zip(wrong.iter().flatten())
                    .any(|(a, b)| (a - b).abs() > 0.001)
            );
        }
    }
    #[test]
    fn invalid_adopted_neutral_is_refused() {
        let xy = [0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290];
        for neutral in [
            [0.0, 0.33],
            [0.33, 0.0],
            [0.7, 0.7],
            [f64::NAN, 0.33],
            [0.33, f64::INFINITY],
        ] {
            assert!(linear_rgb_to_srgb_with_adopted_neutral_from_xy(xy, neutral).is_none());
        }
    }

    #[test]
    fn invalid_and_collapsed_bases_are_refused() {
        let valid = [0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290];
        for index in 0..8 {
            let mut xy = valid;
            xy[index] = f64::NAN;
            assert!(linear_rgb_to_srgb_from_xy(xy).is_none());
        }
        for xy in [
            [0.0; 8],
            [0.64, 0.33, 0.64, 0.33, 0.64, 0.33, 0.3127, 0.3290],
            [0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.0, 0.3290],
            [0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 1.0, 1.0],
        ] {
            assert!(linear_rgb_to_srgb_from_xy(xy).is_none());
        }
    }
}

/// Validated linear RGB primaries/white and optional adopted neutral, held inline.
/// The inline representation makes swap serialization allocation-free and keeps
/// exact source coordinates through clones and disk round trips.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RgbChromaticities([u8; 80]);
impl RgbChromaticities {
    pub fn new(xy: [f64; 8]) -> Option<Self> {
        linear_rgb_to_srgb_from_xy(xy)?;
        let mut bytes = [0u8; 80];
        for (chunk, value) in bytes.chunks_exact_mut(8).zip(xy) {
            chunk.copy_from_slice(&value.to_le_bytes());
        }
        Some(Self(bytes))
    }
    pub fn from_le_bytes(bytes: [u8; 64]) -> Option<Self> {
        Self::new(std::array::from_fn(|i| {
            f64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap())
        }))
    }
    /// Coordinate-only legacy representation; optional adopted neutral is separate.
    pub fn as_le_bytes(&self) -> &[u8; 64] {
        self.0[..64].try_into().unwrap()
    }
    pub fn with_adopted_neutral(mut self, neutral: [f64; 2]) -> Option<Self> {
        linear_rgb_to_srgb_with_adopted_neutral_from_xy(self.xy(), neutral)?;
        for (chunk, value) in self.0[64..].chunks_exact_mut(8).zip(neutral) {
            chunk.copy_from_slice(&value.to_le_bytes());
        }
        Some(self)
    }
    pub fn adopted_neutral(&self) -> Option<[f64; 2]> {
        self.0[64..].iter().any(|v| *v != 0).then(|| {
            std::array::from_fn(|i| f64::from_le_bytes(self.0[64 + i * 8..72 + i * 8].try_into().unwrap()))
        })
    }
    pub fn color_metadata_bytes(&self) -> &[u8] {
        &self.0[..if self.adopted_neutral().is_some() { 80 } else { 64 }]
    }
    pub fn from_color_bytes(bytes: [u8; 80]) -> Option<Self> {
        Self::from_le_bytes(bytes[..64].try_into().unwrap())?.with_adopted_neutral(std::array::from_fn(|i| {
            f64::from_le_bytes(bytes[64 + i * 8..72 + i * 8].try_into().unwrap())
        }))
    }
    pub fn to_linear_srgb_matrix(&self) -> Option<[[f64; 3]; 3]> {
        let xy = self.xy();
        linear_rgb_to_srgb_with_adopted_neutral_from_xy(xy, self.adopted_neutral().unwrap_or([xy[6], xy[7]]))
    }
    pub fn xy(&self) -> [f64; 8] {
        std::array::from_fn(|i| f64::from_le_bytes(self.0[i * 8..i * 8 + 8].try_into().unwrap()))
    }
}
