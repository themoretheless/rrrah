//! Budgeted, document-local authoring history. Separate from sensor cache recipes.
use crate::{MemoryBudget, MutableBuffer};
use serde::{Deserialize, Serialize};

pub const EDIT_SCHEMA: u32 = 1;
pub const MAX_CURVE_POINTS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditState {
    pub exposure_stops: f32,
    curve: [[f64; 2]; MAX_CURVE_POINTS],
    curve_len: usize,
}
impl Default for EditState {
    fn default() -> Self {
        let mut curve = [[0.; 2]; MAX_CURVE_POINTS];
        curve[1] = [1., 1.];
        Self {
            exposure_stops: 0.,
            curve,
            curve_len: 2,
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("invalid edit state")]
    Invalid,
    #[error(transparent)]
    Memory(#[from] crate::BufferError),
}
impl EditState {
    pub fn points(&self) -> &[[f64; 2]] {
        &self.curve[..self.curve_len.min(MAX_CURVE_POINTS)]
    }
    pub fn validate(&self) -> Result<(), EditError> {
        if !self.exposure_stops.is_finite()
            || !(-32. ..=32.).contains(&self.exposure_stops)
            || !(2..=MAX_CURVE_POINTS).contains(&self.curve_len)
        {
            return Err(EditError::Invalid);
        }
        let p = self.points();
        if p.iter()
            .any(|p| p.iter().any(|v| !v.is_finite() || !(0. ..=1.).contains(v)))
            || p.windows(2)
                .any(|p| p[1][0] - p[0][0] < 1e-6 || p[1][1] < p[0][1])
        {
            return Err(EditError::Invalid);
        }
        Ok(())
    }
    pub fn with_curve(mut self, points: &[[f64; 2]]) -> Result<Self, EditError> {
        if !(2..=MAX_CURVE_POINTS).contains(&points.len()) {
            return Err(EditError::Invalid);
        }
        self.curve = [[0.; 2]; MAX_CURVE_POINTS];
        self.curve[..points.len()].copy_from_slice(points);
        self.curve_len = points.len();
        self.validate()?;
        Ok(self)
    }
}

/// Fixed-capacity ring: memory is admitted before allocation and remains bounded.
#[derive(Debug)]
pub struct EditSession {
    states: MutableBuffer<EditState>,
    start: usize,
    count: usize,
    cursor: usize,
    gesture: bool,
    gesture_written: bool,
    original: EditState,
}
impl EditSession {
    pub fn new(initial: EditState, capacity: usize, budget: &MemoryBudget) -> Result<Self, EditError> {
        initial.validate()?;
        if !(2..=4096).contains(&capacity) {
            return Err(EditError::Invalid);
        }
        let states = budget.try_buffer(capacity, initial)?;
        Ok(Self {
            states,
            start: 0,
            count: 1,
            cursor: 0,
            gesture: false,
            gesture_written: false,
            original: initial,
        })
    }
    pub fn current(&self) -> EditState {
        self.states[(self.start + self.cursor) % self.states.len()]
    }
    pub fn original(&self) -> EditState {
        self.original
    }
    pub fn history_len(&self) -> usize {
        self.count
    }
    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }
    pub fn can_redo(&self) -> bool {
        self.cursor + 1 < self.count
    }
    pub fn begin_gesture(&mut self) {
        self.gesture = true;
        self.gesture_written = false;
    }
    pub fn end_gesture(&mut self) {
        self.gesture = false;
        self.gesture_written = false;
    }
    pub fn apply(&mut self, state: EditState) -> Result<bool, EditError> {
        state.validate()?;
        if state == self.current() {
            return Ok(false);
        }
        if self.gesture && self.gesture_written {
            let index = (self.start + self.cursor) % self.states.len();
            self.states[index] = state;
            return Ok(true);
        }
        self.count = self.cursor + 1; // New intent discards redo.
        if self.count == self.states.len() {
            self.start = (self.start + 1) % self.states.len();
            self.count -= 1;
        }
        let index = (self.start + self.count) % self.states.len();
        self.states[index] = state;
        self.count += 1;
        self.cursor = self.count - 1;
        self.gesture_written = self.gesture;
        Ok(true)
    }
    pub fn undo(&mut self) -> bool {
        self.end_gesture();
        if !self.can_undo() {
            return false;
        }
        self.cursor -= 1;
        true
    }
    pub fn redo(&mut self) -> bool {
        self.end_gesture();
        if !self.can_redo() {
            return false;
        }
        self.cursor += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gestures_redo_truncation_curves_and_bounded_ring() {
        let budget = MemoryBudget::new(3 * std::mem::size_of::<EditState>() as u64);
        let mut history = EditSession::new(EditState::default(), 3, &budget).unwrap();
        history.begin_gesture();
        for exposure_stops in [0.25, 0.5, 0.75] {
            history
                .apply(EditState {
                    exposure_stops,
                    ..EditState::default()
                })
                .unwrap();
        }
        history.end_gesture();
        assert_eq!(history.history_len(), 2);
        assert!(history.undo());
        assert_eq!(history.current().exposure_stops, 0.);
        assert!(history.redo());
        let curve = history
            .current()
            .with_curve(&[[0., 0.], [0.5, 0.7], [1., 1.]])
            .unwrap();
        history.apply(curve).unwrap();
        assert!(history.undo());
        history
            .apply(EditState {
                exposure_stops: 1.,
                ..EditState::default()
            })
            .unwrap();
        assert!(!history.can_redo());
        for exposure_stops in [2., 3., 4.] {
            history
                .apply(EditState {
                    exposure_stops,
                    ..EditState::default()
                })
                .unwrap();
        }
        assert_eq!(history.history_len(), 3);
        assert_eq!(history.original(), EditState::default());
        assert!(history.undo());
        assert!(history.undo());
        assert!(!history.undo());
        assert_eq!(history.current().exposure_stops, 2.);
        drop(history);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn admission_and_invalid_commands_do_not_mutate_history() {
        assert!(EditSession::new(EditState::default(), 2, &MemoryBudget::new(1)).is_err());
        let mut history = EditSession::new(EditState::default(), 2, &MemoryBudget::new(4096)).unwrap();
        assert!(
            history
                .apply(EditState {
                    exposure_stops: f32::NAN,
                    ..EditState::default()
                })
                .is_err()
        );
        assert_eq!(history.history_len(), 1);
        assert!(EditState::default().with_curve(&[[0., 1.], [1., 0.]]).is_err());
    }
}
