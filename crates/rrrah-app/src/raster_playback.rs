//! Presentation-driven, bounded selected-frame animation scheduling.
use rrrah_core::RasterAnimationTiming;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(crate) struct Playback {
    state: Option<State>,
}
struct State {
    generation: u64,
    index: usize,
    count: usize,
    completed: u64,
    timing: RasterAnimationTiming,
    awaiting_present: bool,
    pending: bool,
    deadline: Option<Instant>,
    paused: bool,
    remaining: Option<Duration>,
}
impl Playback {
    pub fn reset(&mut self) {
        self.state = None;
    }
    /// Prepare a manual frame request within the same source. The new frame
    /// starts a fresh repeat count, while the user's pause choice survives.
    pub fn seek(&mut self) {
        if let Some(s) = self.state.as_mut() {
            s.completed = 0;
            s.deadline = None;
            s.remaining = None;
            s.awaiting_present = false;
            s.pending = true;
        }
    }
    pub fn ready(
        &mut self,
        generation: u64,
        index: usize,
        count: usize,
        timing: Option<RasterAnimationTiming>,
    ) {
        let Some(timing) = timing.filter(|_| count > 1 && index < count) else {
            self.reset();
            return;
        };
        let (completed, paused) = self
            .state
            .as_ref()
            .filter(|s| s.pending && s.generation == generation)
            .map_or((0, false), |s| (s.completed, s.paused));
        self.state = Some(State {
            generation,
            index,
            count,
            completed,
            timing,
            awaiting_present: true,
            pending: false,
            deadline: None,
            paused,
            remaining: None,
        });
    }
    pub fn presented(&mut self, generation: u64, now: Instant) {
        if let Some(s) = self
            .state
            .as_mut()
            .filter(|s| s.generation == generation && s.awaiting_present && !s.pending)
        {
            // Explicit viewer pacing policy prevents zero-delay CPU/decode loops.
            let delay = s.timing.delay().max(Duration::from_millis(10));
            if s.paused {
                s.remaining = Some(delay);
            } else {
                s.deadline = now.checked_add(delay);
            }
            s.awaiting_present = false;
        }
    }
    pub fn is_paused(&self, generation: u64) -> bool {
        self.state
            .as_ref()
            .is_some_and(|s| s.generation == generation && s.paused)
    }
    pub fn toggle_pause(&mut self, generation: u64, now: Instant) -> Option<bool> {
        let s = self.state.as_mut().filter(|s| s.generation == generation)?;
        s.paused = !s.paused;
        if s.paused {
            s.remaining = s.deadline.take().map(|d| d.saturating_duration_since(now));
        } else if !s.awaiting_present && !s.pending {
            let delay = s
                .remaining
                .take()
                .unwrap_or_else(|| s.timing.delay().max(Duration::from_millis(10)));
            s.deadline = now.checked_add(delay);
        }
        Some(s.paused)
    }
    pub fn deadline(&self, generation: u64) -> Option<Instant> {
        self.state
            .as_ref()
            .filter(|s| s.generation == generation && !s.pending && !s.paused)
            .and_then(|s| s.deadline)
    }
    pub fn take_next(&mut self, generation: u64, now: Instant) -> Option<usize> {
        if self.deadline(generation).is_none_or(|deadline| now < deadline) {
            return None;
        }
        let s = self.state.as_mut()?;
        s.deadline = None;
        let next = if s.index == s.count - 1 {
            s.completed = s.completed.saturating_add(1);
            if s.timing.num_plays() != 0 && s.completed >= u64::from(s.timing.num_plays()) {
                self.reset();
                return None;
            }
            0
        } else {
            s.index + 1
        };
        // No further deadline until a load completes and is actually presented.
        s.pending = true;
        Some(next)
    }
    /// Whether the pass containing the requested next frame can wrap again.
    pub fn repeat_after_current_cycle(&self, generation: u64) -> Option<bool> {
        self.state
            .as_ref()
            .filter(|s| s.generation == generation)
            .map(|s| {
                s.timing.num_plays() == 0 || s.completed.saturating_add(1) < u64::from(s.timing.num_plays())
            })
    }
    pub fn submitted(&mut self, generation: u64) {
        if let Some(s) = self.state.as_mut() {
            s.generation = generation;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_infinite_rational_and_zero_delay_follow_presented_frames() {
        let mut cases = 0;
        for count in 2..=5 {
            for plays in 0..=3 {
                for (n, d) in [(0, 1), (30, 1), (1000, 60)] {
                    let timing = RasterAnimationTiming::new(n, d, plays).unwrap();
                    let mut clock = Playback::default();
                    let mut now = Instant::now();
                    let mut generation = 1;
                    let mut index = 0;
                    let steps = if plays == 0 {
                        count * 4
                    } else {
                        count * plays as usize
                    };
                    for step in 0..steps {
                        clock.ready(generation, index, count, Some(timing));
                        assert!(clock.deadline(generation).is_none());
                        assert!(
                            clock
                                .take_next(generation, now + Duration::from_secs(10))
                                .is_none()
                        );
                        clock.presented(generation, now);
                        let deadline = clock.deadline(generation).unwrap();
                        assert_eq!(
                            deadline.duration_since(now),
                            timing.delay().max(Duration::from_millis(10))
                        );
                        assert!(
                            clock
                                .take_next(generation, deadline - Duration::from_nanos(1))
                                .is_none()
                        );
                        assert!(clock.take_next(generation + 1, deadline).is_none());
                        let next = clock.take_next(generation, deadline);
                        if plays != 0 && step + 1 == steps {
                            assert!(next.is_none());
                            assert!(clock.deadline(generation).is_none());
                            break;
                        }
                        assert_eq!(next, Some((index + 1) % count));
                        assert_eq!(
                            clock.repeat_after_current_cycle(generation),
                            Some(plays == 0 || (step + 1) / count + 1 < plays as usize)
                        );
                        assert!(
                            clock
                                .take_next(generation, deadline + Duration::from_secs(30))
                                .is_none()
                        );
                        generation += 1;
                        clock.submitted(generation);
                        assert!(clock.deadline(generation).is_none());
                        index = next.unwrap();
                        now = deadline + Duration::from_secs(1);
                    }
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 48);
    }
    #[test]
    fn new_source_manual_reset_and_nonanimation_never_inherit_loop_state() {
        let mut clock = Playback::default();
        let now = Instant::now();
        let timing = RasterAnimationTiming::new(0, 1, 2).unwrap();
        clock.ready(1, 1, 2, Some(timing));
        clock.presented(1, now);
        assert_eq!(clock.take_next(1, now + Duration::from_secs(1)), Some(0));
        clock.submitted(2);
        clock.reset();
        clock.ready(3, 1, 2, Some(timing));
        clock.presented(3, now);
        assert_eq!(clock.take_next(3, now + Duration::from_secs(1)), Some(0));
        for (count, index, timing) in [
            (2, 0, None),
            (1, 0, Some(timing)),
            (0, 0, Some(timing)),
            (2, 2, Some(timing)),
        ] {
            clock.ready(4, index, count, timing);
            clock.presented(4, now);
            assert!(clock.deadline(4).is_none());
        }
    }
}

#[cfg(test)]
mod pause_tests {
    use super::*;
    #[test]
    fn running_pause_preserves_remaining_time_and_stale_generation_is_inert() {
        let timing = RasterAnimationTiming::new(100, 1, 2).unwrap();
        let now = Instant::now();
        let mut clock = Playback::default();
        clock.ready(1, 0, 2, Some(timing));
        clock.presented(1, now);
        assert_eq!(clock.toggle_pause(99, now), None);
        assert_eq!(clock.toggle_pause(1, now + Duration::from_millis(30)), Some(true));
        assert!(clock.is_paused(1));
        assert!(clock.deadline(1).is_none());
        assert!(clock.take_next(1, now + Duration::from_secs(10)).is_none());
        assert_eq!(clock.toggle_pause(1, now + Duration::from_secs(10)), Some(false));
        let deadline = now + Duration::from_secs(10) + Duration::from_millis(70);
        assert_eq!(clock.deadline(1), Some(deadline));
        assert_eq!(clock.take_next(1, deadline), Some(1));
    }
    #[test]
    fn pause_before_presentation_and_during_pending_load_preserves_cycle_count() {
        let timing = RasterAnimationTiming::new(1000, 60, 2).unwrap();
        let now = Instant::now();
        let delay = timing.delay();
        let mut clock = Playback::default();
        clock.ready(1, 1, 2, Some(timing));
        assert_eq!(clock.toggle_pause(1, now), Some(true));
        clock.presented(1, now);
        assert!(clock.deadline(1).is_none());
        clock.toggle_pause(1, now + Duration::from_secs(1));
        assert_eq!(clock.deadline(1), Some(now + Duration::from_secs(1) + delay));
        assert_eq!(clock.take_next(1, now + Duration::from_secs(2)), Some(0));
        clock.submitted(2);
        assert_eq!(clock.toggle_pause(2, now), Some(true));
        clock.ready(2, 0, 2, Some(timing));
        clock.presented(2, now);
        assert!(clock.is_paused(2));
        assert!(clock.deadline(2).is_none());
        clock.toggle_pause(2, now);
        assert_eq!(clock.take_next(2, now + delay), Some(1));
        clock.submitted(3);
        clock.ready(3, 1, 2, Some(timing));
        clock.presented(3, now);
        assert_eq!(clock.take_next(3, now + delay), None);
        assert_eq!(clock.toggle_pause(3, now), None);
        clock.reset();
        assert!(!clock.is_paused(3));
    }
}

#[cfg(test)]
mod seek_tests {
    use super::*;
    #[test]
    fn manual_seek_preserves_pause_and_restarts_repeat_count() {
        let timing = RasterAnimationTiming::new(100, 1, 2).unwrap();
        let now = Instant::now();
        let mut clock = Playback::default();
        clock.ready(1, 1, 2, Some(timing));
        clock.presented(1, now);
        assert_eq!(clock.take_next(1, now + Duration::from_millis(100)), Some(0));
        clock.submitted(2);
        clock.ready(2, 0, 2, Some(timing));
        clock.presented(2, now);
        assert_eq!(clock.toggle_pause(2, now), Some(true));
        clock.seek();
        clock.submitted(3);
        assert!(clock.is_paused(3));
        assert!(clock.deadline(3).is_none());
        clock.ready(3, 1, 2, Some(timing));
        clock.presented(3, now);
        assert!(clock.is_paused(3));
        clock.toggle_pause(3, now);
        assert_eq!(clock.take_next(3, now + Duration::from_millis(100)), Some(0));
        clock.reset();
        assert!(!clock.is_paused(3));
    }
}
