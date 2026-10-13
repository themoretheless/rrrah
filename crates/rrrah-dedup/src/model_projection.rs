//! Explicit lossy model-input projections; never original-pixel evidence.
use crate::model_input::ModelInputError;

/// Projection applied before sRGB encoding and grayscale area averaging.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModelInputProjection {
    /// Existing exact opaque unit-range linear-sRGB input recipe.
    OpaqueSdr,
    /// Composite straight-alpha linear RGB on an explicit nonnegative linear
    /// background, clip negative channels, apply exposure and x/(1+x).
    /// These operations are proposal-only. Original RGBA must be verified
    /// independently; projected equality never proves a duplicate.
    CompositeReinhardV1 { exposure: f64, background: [f64; 3] },
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct ValidatedProjection(ModelInputProjection);
impl ModelInputProjection {
    pub(crate) fn validate(self) -> Result<ValidatedProjection, ModelInputError> {
        if let Self::CompositeReinhardV1 { exposure, background } = self {
            if !exposure.is_finite() || exposure <= 0. || background.iter().any(|v| !v.is_finite() || *v < 0.)
            {
                return Err(ModelInputError::Invalid);
            }
        }
        Ok(ValidatedProjection(self))
    }
}
impl ValidatedProjection {
    pub(crate) fn rgb(self, rgba: [f32; 4]) -> Result<[f64; 3], ModelInputError> {
        let alpha = f64::from(rgba[3]);
        if !alpha.is_finite()
            || !(0. ..=1.).contains(&alpha)
            || (alpha != 0. && rgba[..3].iter().any(|v| !v.is_finite()))
        {
            return Err(ModelInputError::UnsupportedPixels);
        }
        match self.0 {
            ModelInputProjection::OpaqueSdr => {
                if alpha != 1. || rgba[..3].iter().any(|v| !(0. ..=1.).contains(v)) {
                    return Err(ModelInputError::UnsupportedPixels);
                }
                Ok([f64::from(rgba[0]), f64::from(rgba[1]), f64::from(rgba[2])])
            }
            ModelInputProjection::CompositeReinhardV1 { exposure, background } => {
                Ok(std::array::from_fn(|i| {
                    // Invisible RGB is ignored before any arithmetic, including NaN.
                    let visible = if alpha == 0. {
                        background[i]
                    } else {
                        f64::from(rgba[i]) * alpha + background[i] * (1. - alpha)
                    };
                    let x = visible.max(0.) * exposure;
                    // Stable at very small values and at product overflow to +inf.
                    if x <= 1. { x / (1. + x) } else { 1. - 1. / (1. + x) }
                }))
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn projection(exposure: f64, background: [f64; 3]) -> ValidatedProjection {
        ModelInputProjection::CompositeReinhardV1 { exposure, background }
            .validate()
            .unwrap()
    }
    #[test]
    fn opaque_sdr_preserves_values_and_refuses_alpha_hdr() {
        let p = ModelInputProjection::OpaqueSdr.validate().unwrap();
        assert_eq!(p.rgb([0., 0.25, 1., 1.]).unwrap(), [0., 0.25, 1.]);
        for rgba in [[0., 0., 0., 0.5], [2., 0., 0., 1.], [-0.1, 0., 0., 1.]] {
            assert_eq!(p.rgb(rgba), Err(ModelInputError::UnsupportedPixels));
        }
    }
    #[test]
    fn neutral_reinhard_and_linear_alpha_compositing() {
        assert_eq!(projection(1., [0.; 3]).rgb([1., 1., 1., 1.]).unwrap(), [0.5; 3]);
        assert_eq!(projection(1., [0.; 3]).rgb([3., 3., 3., 1.]).unwrap(), [0.75; 3]);
        assert_eq!(projection(1., [0.; 3]).rgb([4., 4., 4., 0.25]).unwrap(), [0.5; 3]);
        let output = projection(1., [2.; 3]).rgb([4., 4., 4., 0.25]).unwrap();
        assert!(output.iter().all(|v| (*v - 5. / 7.).abs() <= f64::EPSILON));
    }
    #[test]
    fn invisible_rgb_is_ignored_and_background_is_explicit() {
        let rgba = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.];
        assert_eq!(projection(1., [0.; 3]).rgb(rgba).unwrap(), [0.; 3]);
        assert_eq!(projection(1., [1.; 3]).rgb(rgba).unwrap(), [0.5; 3]);
    }
    #[test]
    fn invalid_configuration_is_refused() {
        for exposure in [0., -1., f64::NAN, f64::INFINITY] {
            assert!(matches!(
                ModelInputProjection::CompositeReinhardV1 {
                    exposure,
                    background: [0.; 3]
                }
                .validate(),
                Err(ModelInputError::Invalid)
            ));
        }
        for channel in [-1., f64::NAN, f64::INFINITY] {
            assert!(matches!(
                ModelInputProjection::CompositeReinhardV1 {
                    exposure: 1.,
                    background: [channel, 0., 0.]
                }
                .validate(),
                Err(ModelInputError::Invalid)
            ));
        }
    }
    #[test]
    fn projected_equality_can_hide_alpha_and_hdr_differences() {
        let p = projection(1., [0.; 3]);
        assert_eq!(p.rgb([0., 0., 0., 0.25]), p.rgb([0., 0., 0., 1.]));
        let encode = |x: f64| {
            if x <= 0.0031308 {
                12.92 * x
            } else {
                1.055 * x.powf(1. / 2.4) - 0.055
            }
        };
        let gray = |x: f32| (encode(p.rgb([x, x, x, 1.]).unwrap()[0]) * 255.).round() as u8;
        assert_eq!(gray(4.), gray(4.01));
        assert_ne!(4_f32.to_bits(), 4.01_f32.to_bits());
    }

    #[test]
    fn extreme_finite_parameters_remain_bounded() {
        for exposure in [f64::from_bits(1), 1., f64::MAX] {
            for background in [[0.; 3], [f64::MAX; 3]] {
                for rgba in [[f32::MAX, f32::MIN, 0., 1.], [1., 2., 3., 0.5], [0., 0., 0., 0.]] {
                    let rgb = projection(exposure, background).rgb(rgba).unwrap();
                    assert!(rgb.iter().all(|v| v.is_finite() && (0. ..=1.).contains(v)));
                }
            }
        }
    }
    #[test]
    fn visible_nonfinite_channels_and_invalid_alpha_are_refused() {
        let p = projection(1., [0.; 3]);
        for rgba in [
            [f32::NAN, 0., 0., 1.],
            [0., 0., 0., f32::NAN],
            [0., 0., 0., -0.1],
            [0., 0., 0., 1.1],
        ] {
            assert_eq!(p.rgb(rgba), Err(ModelInputError::UnsupportedPixels));
        }
    }
}
