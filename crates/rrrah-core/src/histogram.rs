//! Bounded, scene-linear RGB/luminance histogram. Log bins retain HDR excursions.
pub const BINS: usize = 64;
pub const MAX_SAMPLES: usize = 65_536;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Histogram {
    /// Half-stop bins from -16 through +16 EV relative to 18% grey, R/G/B/Y.
    pub channels: [[u64; BINS]; 4],
    pub negative: [u64; 4],
    pub zero: [u64; 4],
    pub underflow: [u64; 4],
    pub overflow: [u64; 4],
    pub sampled: u64,
    pub invalid: u64,
    pub transparent: u64,
}
impl Default for Histogram {
    fn default() -> Self {
        Self {
            channels: [[0; BINS]; 4],
            negative: [0; 4],
            zero: [0; 4],
            underflow: [0; 4],
            overflow: [0; 4],
            sampled: 0,
            invalid: 0,
            transparent: 0,
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum HistogramError {
    #[error("histogram cancelled")]
    Cancelled,
    #[error("invalid histogram layout, sample limit or exposure")]
    Invalid,
}
impl Histogram {
    /// Uniformly subsamples at most `max_samples` pixels. Nonfinite pixels or
    /// alpha outside [0,1] are counted as invalid; fully transparent RGB is ignored.
    /// Signed samples are recorded separately; exposure is applied in log space.
    // BINS is 64 and the float index is checked in [0,64) before conversion.
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn linear_rgba(
        pixels: &[f32],
        max_samples: usize,
        exposure: f32,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<Self, HistogramError> {
        if cancelled() {
            return Err(HistogramError::Cancelled);
        }
        if !pixels.len().is_multiple_of(4)
            || !(1..=MAX_SAMPLES).contains(&max_samples)
            || !exposure.is_finite()
            || !(-32. ..=32.).contains(&exposure)
        {
            return Err(HistogramError::Invalid);
        }
        let count = pixels.len() / 4;
        let stride = count.div_ceil(max_samples).max(1);
        let mut result = Self::default();
        for (i, p) in pixels.as_chunks::<4>().0.iter().step_by(stride).enumerate() {
            if i % 256 == 0 && cancelled() {
                return Err(HistogramError::Cancelled);
            }
            result.sampled += 1;
            if p.iter().any(|v| !v.is_finite()) || !(0. ..=1.).contains(&p[3]) {
                result.invalid += 1;
                continue;
            }
            if p[3] == 0. {
                result.transparent += 1;
                continue;
            }
            // f64 avoids overflow while forming luminance from finite f32 HDR.
            let values = [
                f64::from(p[0]),
                f64::from(p[1]),
                f64::from(p[2]),
                0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2]),
            ];
            for (channel, value) in values.into_iter().enumerate() {
                if value < 0. {
                    result.negative[channel] += 1;
                } else if value == 0. {
                    result.zero[channel] += 1;
                } else {
                    let position = ((value / 0.18).log2() + f64::from(exposure) + 16.) * 2.;
                    if position < 0. {
                        result.underflow[channel] += 1;
                    } else if position >= BINS as f64 {
                        result.overflow[channel] += 1;
                    } else {
                        result.channels[channel][position.floor() as usize] += 1;
                    }
                }
            }
        }
        if cancelled() {
            return Err(HistogramError::Cancelled);
        }
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn analytic_sdr_hdr_signed_alpha_and_nonfinite_policies() {
        let values = [
            0.18,
            0.36,
            0.72,
            1.,
            -1.,
            0.,
            f32::MAX,
            1.,
            f32::NAN,
            0.,
            0.,
            1.,
            4.,
            4.,
            4.,
            0.,
        ];
        let histogram = Histogram::linear_rgba(&values, 16, 0., || false).unwrap();
        assert_eq!(histogram.sampled, 4);
        assert_eq!(histogram.invalid, 1);
        assert_eq!(histogram.transparent, 1);
        // f32 encoding of 0.18 is slightly above exact 18% grey.
        assert_eq!(histogram.channels[0][32], 1);
        assert_eq!(histogram.channels[1][34], 1);
        assert_eq!(histogram.channels[2][36], 1);
        assert_eq!(histogram.negative[0], 1);
        assert_eq!(histogram.zero[1], 1);
        assert_eq!(histogram.overflow[2], 1);
        let shifted = Histogram::linear_rgba(&values[..4], 16, 1., || false).unwrap();
        assert_eq!(shifted.channels[0][34], 1);
    }
    #[test]
    fn sample_count_and_cancellation_are_bounded() {
        let input = vec![1.; 400_000];
        let histogram = Histogram::linear_rgba(&input, 100, 0., || false).unwrap();
        assert_eq!(histogram.sampled, 100);
        let mut checks = 0;
        assert!(matches!(
            Histogram::linear_rgba(&input, MAX_SAMPLES, 0., || {
                checks += 1;
                checks == 3
            }),
            Err(HistogramError::Cancelled)
        ));
        assert!(Histogram::linear_rgba(&[1.; 3], 1, 0., || false).is_err());
    }
}
