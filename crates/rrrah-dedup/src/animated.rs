//! Complete composited animation frames from the existing GIF/APNG/WebP decoders.
use crate::{
    decode::FileError,
    exact::{ContentSnapshot, SnapshotError},
    pixels::PixelError,
    raster::NormalizedRaster,
    sequence::{Duration, SequenceError},
};
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;

#[derive(Debug, Clone, Copy)]
pub enum AnimationKind {
    Gif,
    Apng,
    Webp,
}
#[derive(Debug, Clone, Copy)]
pub struct AnimationBudget {
    pub max_frames: usize,
    pub max_pixels: u64,
    pub max_file_bytes: u64,
}
#[derive(Debug, thiserror::Error)]
pub enum AnimationError {
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
    #[error("complete page/resource enumeration is not implemented for this container")]
    UnsupportedPages,
    #[error(transparent)]
    Source(#[from] SnapshotError),
    #[error(transparent)]
    Decode(#[from] FileError),
    #[error(transparent)]
    Sequence(#[from] SequenceError),
}
#[derive(Debug)]
pub struct Animation {
    frames: Vec<(NormalizedRaster, Duration)>,
    /// None is infinite; Some(n) is total plays, including the initial play.
    total_plays: Option<u64>,
}
impl Animation {
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }
    /// Versioned whole-timeline candidate key, independent of adjacent identical frame splits.
    /// Zero-duration frames are skipped and repetition semantics are retained.
    /// Equal keys still require direct timeline confirmation; collisions cannot prove identity.
    ///
    /// # Errors
    /// Invalid pixels, bounded rational-sum overflow or cancellation. A failed key
    /// is inconclusive and must not be interpreted as a different presentation.
    pub fn timeline_digest(&self, cancel: impl Fn() -> bool) -> Result<[u8; 32], AnimationError> {
        let mut hash = blake3::Hasher::new();
        hash.update(b"rrrah-animation-timeline-v1");
        match self.total_plays {
            None => {
                hash.update(&[0]);
            }
            Some(plays) => {
                hash.update(&[1]);
                hash.update(&plays.to_le_bytes());
            }
        }
        let mut run: Option<([u8; 32], Duration)> = None;
        for (frame, duration) in &self.frames {
            if cancel() {
                return Err(SequenceError::Pixels(PixelError::Cancelled).into());
            }
            if duration.parts().0 == 0 {
                continue;
            }
            let key = frame
                .view(&cancel)
                .map_err(FileError::from)?
                .pixel_digest(&cancel)
                .map_err(SequenceError::from)?;
            match run {
                Some((previous, elapsed)) if previous == key => {
                    run = Some((key, elapsed.checked_add(*duration)?));
                }
                Some((previous, elapsed)) => {
                    hash.update(&previous);
                    let (numerator, denominator) = elapsed.parts();
                    hash.update(&numerator.to_le_bytes());
                    hash.update(&denominator.to_le_bytes());
                    run = Some((key, *duration));
                }
                None => run = Some((key, *duration)),
            }
        }
        if let Some((key, elapsed)) = run {
            hash.update(&key);
            let (numerator, denominator) = elapsed.parts();
            hash.update(&numerator.to_le_bytes());
            hash.update(&denominator.to_le_bytes());
        }
        if cancel() {
            return Err(SequenceError::Pixels(PixelError::Cancelled).into());
        }
        Ok(*hash.finalize().as_bytes())
    }

    /// Compare every presentation, exact rational duration and repetition.
    /// Different frame segmentation is retained, even for equivalent timelines.
    ///
    /// # Errors
    /// Returns cancellation or invalid normalized storage.
    pub fn same_animation(&self, other: &Self, cancel: impl Fn() -> bool) -> Result<bool, AnimationError> {
        if cancel() {
            return Err(SequenceError::Pixels(PixelError::Cancelled).into());
        }
        if self.total_plays != other.total_plays || self.frames.len() != other.frames.len() {
            return Ok(false);
        }
        for ((a, at), (b, bt)) in self.frames.iter().zip(&other.frames) {
            if at != bt || !a.same_selected_frame(b, &cancel).map_err(FileError::from)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Compare presentation timelines, allowing identical adjacent frame splits.
    /// Zero-duration frames have no presentation; player-specific minimum-delay
    /// clamping must be normalized by the caller before using this interpretation.
    ///
    /// # Errors
    /// Returns cancellation, invalid storage or timing arithmetic overflow.
    pub fn same_timeline(&self, other: &Self, cancel: impl Fn() -> bool) -> Result<bool, AnimationError> {
        use crate::sequence::{Frame, Playback, Sequence};
        let mut sequences = Vec::new();
        for animation in [self, other] {
            let frames = animation
                .frames
                .iter()
                .enumerate()
                .map(|(i, (raster, duration))| {
                    Ok((
                        i,
                        Frame {
                            pixels: raster.view(&cancel).map_err(FileError::from)?,
                            duration: Some(*duration),
                        },
                    ))
                })
                .collect::<Result<Vec<_>, AnimationError>>()?;
            sequences.push(Sequence::new(
                frames.len(),
                Playback::Animation {
                    repetitions: animation.total_plays,
                },
                frames,
                animation.frames.len(),
                &cancel,
            )?);
        }
        Ok(sequences[0].same_timeline(&sequences[1], &cancel)?)
    }
}

/// Decode every composited frame with source mutation checks and bounded retained
/// memory. Kind is explicit; each decoder validates its format. No frame is skipped.
/// Repeated selected-frame decoding currently replays earlier presentations, so
/// performance can be quadratic in frame count. Frame budget is mandatory.
///
/// # Errors
/// Returns frame/resource/source/color/timing/cancellation errors; no partial
/// animation is returned. Request `image_index` is overridden to cover all frames.
pub fn decode_animation(
    request: &DecodeRequest,
    kind: AnimationKind,
    limits: AnimationBudget,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<Animation, AnimationError> {
    let cancelled = || {
        cancel()
            || request
                .cancellation
                .as_ref()
                .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
    };
    let source = ContentSnapshot::read(&request.path, limits.max_file_bytes, cancelled)?;
    let mut frames = Vec::new();
    let mut expected = None;
    let mut plays = None;
    let mut index = 0;
    loop {
        if cancelled() {
            return Err(SequenceError::Pixels(PixelError::Cancelled).into());
        }
        if index >= limits.max_frames {
            return Err(SequenceError::Coverage.into());
        }
        let mut selected = request.clone();
        selected.image_index = index;
        selected.memory_budget = Some(budget.clone());
        let (raster, duration, repetition) = match kind {
            AnimationKind::Gif => {
                let image = rrrah_decode::decode_gif(&selected).map_err(FileError::from)?;
                let plays = match image.repeats {
                    None => Some(1),
                    Some(0) => None,
                    Some(n) => Some(u64::from(n) + 1),
                };
                (
                    image.raster,
                    Duration::new(u64::from(image.delay_ms), 1000)?,
                    plays,
                )
            }
            AnimationKind::Apng => {
                let image = rrrah_decode::decode_apng(&selected).map_err(FileError::from)?;
                (
                    image.raster,
                    Duration::new(
                        u64::from(image.delay_ms_numerator),
                        u64::from(image.delay_ms_denominator) * 1000,
                    )?,
                    (image.num_plays != 0).then_some(u64::from(image.num_plays)),
                )
            }
            AnimationKind::Webp => {
                let image = rrrah_decode::decode_webp_animation(&selected).map_err(FileError::from)?;
                (
                    image.raster,
                    Duration::new(u64::from(image.delay_ms), 1000)?,
                    (image.num_plays != 0).then_some(u64::from(image.num_plays)),
                )
            }
        };
        let count = raster.image_count();
        if count > limits.max_frames || raster.image_index() != index {
            return Err(SequenceError::Coverage.into());
        }
        if let Some(count_before) = expected {
            if count != count_before || plays != repetition {
                return Err(SequenceError::Coverage.into());
            }
        } else {
            expected = Some(count);
            plays = repetition;
        }
        if u64::from(raster.width()) * u64::from(raster.height()) > limits.max_pixels {
            return Err(SequenceError::Pixels(PixelError::Budget).into());
        }
        let linear =
            rrrah_decode::prepare_raster_for_display_with_budget_and_cancel(&raster, Some(budget), cancelled)
                .map_err(FileError::from)?;
        let normalized =
            NormalizedRaster::new(&linear, limits.max_pixels, budget, cancelled).map_err(FileError::from)?;
        frames.push((normalized, duration));
        index += 1;
        if index == count {
            break;
        }
    }
    source.verify(cancelled)?;
    Ok(Animation {
        frames,
        total_plays: plays,
    })
}

#[cfg(test)]
mod timeline_key_tests {
    use super::*;
    fn fixture(name: &str, kind: AnimationKind, budget: &MemoryBudget) -> Animation {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
        decode_animation(
            &DecodeRequest::new(root.join(name)),
            kind,
            AnimationBudget {
                max_frames: 10,
                max_pixels: 100,
                max_file_bytes: 100_000,
            },
            budget,
            || false,
        )
        .unwrap()
    }
    #[test]
    fn rational_sum_overflow_is_inconclusive_despite_direct_timeline_equality() {
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let mut animation = fixture("split.apng", AnimationKind::Apng, &budget);
        animation.frames[0].1 = Duration::new(u64::MAX, 1).unwrap();
        animation.frames[1].1 = Duration::new(u64::MAX, 1).unwrap();
        assert!(animation.same_timeline(&animation, || false).unwrap());
        assert!(matches!(
            animation.timeline_digest(|| false),
            Err(AnimationError::Sequence(SequenceError::Timing))
        ));
        drop(animation);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn zero_duration_timelines_ignore_pixels_but_retain_repetition() {
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let mut a = fixture("base.gif", AnimationKind::Gif, &budget);
        let mut b = fixture("changed-pixels.apng", AnimationKind::Apng, &budget);
        for animation in [&mut a, &mut b] {
            for (_, duration) in &mut animation.frames {
                *duration = Duration::new(0, 1).unwrap();
            }
        }
        assert!(a.same_timeline(&b, || false).unwrap());
        assert_eq!(
            a.timeline_digest(|| false).unwrap(),
            b.timeline_digest(|| false).unwrap()
        );
        b.total_plays = Some(a.total_plays.unwrap_or(0) + 1);
        assert!(!a.same_timeline(&b, || false).unwrap());
        assert_ne!(
            a.timeline_digest(|| false).unwrap(),
            b.timeline_digest(|| false).unwrap()
        );
        drop((a, b));
        assert_eq!(budget.used(), 0);
    }
}
