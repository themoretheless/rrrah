//! Explicit selected-frame versus complete-sequence pixel equality.
//! Inputs must be fully composited, color/orientation-normalized frames.
//! Container decoders remain responsible for disposal, blend and timing metadata.

use crate::{linear::LinearRgbaView, pixels::PixelError};

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
        let left = u128::from(self.numerator) * u128::from(other.denominator);
        let right = u128::from(other.numerator) * u128::from(self.denominator);
        let mut numerator = left.checked_add(right).ok_or(SequenceError::Timing)?;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playback {
    /// Ordered pages have no playback timing.
    Pages,
    /// The exact repetition semantics must be normalized by the decoder.
    Animation { repetitions: Option<u64> },
}

#[derive(Debug, Clone, Copy)]
pub struct Frame<'a> {
    pub pixels: LinearRgbaView<'a>,
    pub duration: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SequenceError {
    #[error("invalid or missing frame timing")]
    Timing,
    #[error("frames are missing, out of order or exceed the frame budget")]
    Coverage,
    #[error(transparent)]
    Pixels(#[from] PixelError),
}

/// Construction proves complete ordinal coverage; selected-frame equality is
/// deliberately available separately and never implies container equality.
#[derive(Debug)]
pub struct Sequence<'a> {
    frames: Vec<Frame<'a>>,
    playback: Playback,
}

impl<'a> Sequence<'a> {
    /// Accept each decoded frame in ordinal order and require declared coverage.
    /// The caller must supply authoritative container count and decoded frames.
    ///
    /// # Errors
    /// Rejects missing/repeated/out-of-order frames, invalid timing, budget excess
    /// and cancellation. No partial sequence is returned.
    pub fn new(
        declared_count: usize,
        playback: Playback,
        frames: impl IntoIterator<Item = (usize, Frame<'a>)>,
        max_frames: usize,
        cancel: impl Fn() -> bool,
    ) -> Result<Self, SequenceError> {
        if declared_count == 0 || declared_count > max_frames {
            return Err(SequenceError::Coverage);
        }
        let mut admitted = Vec::new();
        for (ordinal, frame) in frames {
            if cancel() {
                return Err(PixelError::Cancelled.into());
            }
            if ordinal != admitted.len() || admitted.len() >= declared_count {
                return Err(SequenceError::Coverage);
            }
            if matches!(playback, Playback::Pages) != frame.duration.is_none() {
                return Err(SequenceError::Timing);
            }
            crate::local::reserve_slot(&mut admitted, declared_count)
                .map_err(|_| SequenceError::Pixels(PixelError::Budget))?;
            admitted.push(frame);
        }
        if cancel() {
            return Err(PixelError::Cancelled.into());
        }
        if admitted.len() != declared_count {
            return Err(SequenceError::Coverage);
        }
        Ok(Self {
            frames: admitted,
            playback,
        })
    }

    /// Compare all ordered frames, normalized rational durations and repetition.
    /// Frame segmentation is retained; equivalent timelines with different frame
    /// segmentation are not established by this exact sequence comparison.
    ///
    /// # Errors
    /// Returns cancellation instead of a partial equality claim.
    pub fn same_sequence(&self, other: &Self, cancel: impl Fn() -> bool) -> Result<bool, SequenceError> {
        if cancel() {
            return Err(PixelError::Cancelled.into());
        }
        if self.playback != other.playback || self.frames.len() != other.frames.len() {
            return Ok(false);
        }
        for (left, right) in self.frames.iter().zip(&other.frames) {
            if left.duration != right.duration || !left.pixels.same_pixels(&right.pixels, &cancel)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Compare the displayed timeline independently of adjacent frame splits.
    /// Zero-duration presentations are skipped; repetition semantics stay exact.
    /// Pages retain ordered-frame equality. Arithmetic overflow is inconclusive.
    ///
    /// # Errors
    /// Returns cancellation or timing arithmetic beyond the rational bounds.
    pub fn same_timeline(&self, other: &Self, cancel: impl Fn() -> bool) -> Result<bool, SequenceError> {
        if cancel() {
            return Err(PixelError::Cancelled.into());
        }
        if self.playback != other.playback {
            return Ok(false);
        }
        if matches!(self.playback, Playback::Pages) {
            return self.same_sequence(other, cancel);
        }
        let mut a = 0;
        let mut b = 0;
        let mut at = self.frames[0].duration.ok_or(SequenceError::Timing)?;
        let mut bt = other.frames[0].duration.ok_or(SequenceError::Timing)?;
        loop {
            if cancel() {
                return Err(PixelError::Cancelled.into());
            }
            while a < self.frames.len() && at.numerator == 0 {
                a += 1;
                if a < self.frames.len() {
                    at = self.frames[a].duration.ok_or(SequenceError::Timing)?;
                }
                if cancel() {
                    return Err(PixelError::Cancelled.into());
                }
            }
            while b < other.frames.len() && bt.numerator == 0 {
                b += 1;
                if b < other.frames.len() {
                    bt = other.frames[b].duration.ok_or(SequenceError::Timing)?;
                }
                if cancel() {
                    return Err(PixelError::Cancelled.into());
                }
            }
            if a == self.frames.len() || b == other.frames.len() {
                return Ok(a == self.frames.len() && b == other.frames.len());
            }
            if !self.frames[a]
                .pixels
                .same_pixels(&other.frames[b].pixels, &cancel)?
            {
                return Ok(false);
            }
            let step = if at.less_or_equal(bt) { at } else { bt };
            at = at.subtract(step)?;
            bt = bt.subtract(step)?;
        }
    }
}
