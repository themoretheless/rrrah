#![allow(dead_code)]
#[derive(Debug)] pub enum SequenceError { Timing }
/// Exact rational duration in seconds, independent of container timebase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Duration {
    numerator: u64,
    denominator: u64,
}

impl Duration {
    /// # Errors
    /// Rejects a zero denominator.
    pub fn new(numerator: u64, denominator: u64) -> Result<Self, SequenceError> {
        if denominator == 0 {
            return Err(SequenceError::Timing);
        }
        let mut a = numerator;
        let mut b = denominator;
        while b != 0 {
            (a, b) = (b, a % b);
        }
        Ok(Self {
            numerator: numerator / a,
            denominator: denominator / a,
        })
    }
    #[cfg(feature = "decode")]
    pub(crate) fn parts(self) -> (u64, u64) {
        (self.numerator, self.denominator)
    }

    #[cfg(feature = "decode")]
    pub(crate) fn checked_add(self, other: Self) -> Result<Self, SequenceError> {
        // Cancel the denominator GCD before cross multiplication. The final
        // reduced duration can fit u64 even when the unreduced sum exceeds u128.
        let (mut common, mut remainder) = (self.denominator, other.denominator);
        while remainder != 0 {
            (common, remainder) = (remainder, common % remainder);
        }
        let left_scale = other.denominator / common;
        let right_scale = self.denominator / common;
        let left = u128::from(self.numerator) * u128::from(left_scale);
        let right = u128::from(other.numerator) * u128::from(right_scale);
        let mut numerator = left.checked_add(right).ok_or(SequenceError::Timing)?;
        let mut denominator = u128::from(self.denominator) * u128::from(left_scale);
        let (mut a, mut b) = (numerator, denominator);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        numerator /= a;
        denominator /= a;
        Self::new(
            u64::try_from(numerator).map_err(|_| SequenceError::Timing)?,
            u64::try_from(denominator).map_err(|_| SequenceError::Timing)?,
        )
    }

    fn subtract(self, other: Self) -> Result<Self, SequenceError> {
        let left = u128::from(self.numerator) * u128::from(other.denominator);
        let right = u128::from(other.numerator) * u128::from(self.denominator);
        let mut numerator = left.checked_sub(right).ok_or(SequenceError::Timing)?;
        let mut denominator = u128::from(self.denominator) * u128::from(other.denominator);
        let (mut a, mut b) = (numerator, denominator);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        numerator /= a;
        denominator /= a;
        Self::new(
            u64::try_from(numerator).map_err(|_| SequenceError::Timing)?,
            u64::try_from(denominator).map_err(|_| SequenceError::Timing)?,
        )
    }

    fn less_or_equal(self, other: Self) -> bool {
        u128::from(self.numerator) * u128::from(other.denominator)
            <= u128::from(other.numerator) * u128::from(self.denominator)
    }
}

#[cfg(all(test, feature = "decode"))]
mod duration_sum_tests {
    use super::*;

    #[test]
    fn large_shared_denominator_reduces_before_overflow() {
        let value = Duration::new(u64::MAX - 2, u64::MAX - 1).unwrap();
        assert_eq!(
            value.checked_add(value).unwrap(),
            Duration::new(u64::MAX - 2, (u64::MAX - 1) / 2).unwrap()
        );
    }

    #[test]
    fn sum_retains_exact_time_and_refuses_unrepresentable_result() {
        assert_eq!(
            Duration::new(1, 6)
                .unwrap()
                .checked_add(Duration::new(1, 3).unwrap())
                .unwrap(),
            Duration::new(1, 2).unwrap()
        );
        assert_eq!(
            Duration::new(0, 1)
                .unwrap()
                .checked_add(Duration::new(u64::MAX, 1).unwrap())
                .unwrap(),
            Duration::new(u64::MAX, 1).unwrap()
        );
        assert!(matches!(
            Duration::new(u64::MAX, 1)
                .unwrap()
                .checked_add(Duration::new(1, 1).unwrap()),
            Err(SequenceError::Timing)
        ));
    }
}

