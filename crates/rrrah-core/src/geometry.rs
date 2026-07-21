use serde::{Deserialize, Serialize};

/// An integer rectangle in raw-sensor coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub const fn full(width: u32, height: u32) -> Self {
        Self::new(0, 0, width, height)
    }

    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn checked_end(self) -> Option<(u32, u32)> {
        Some((self.x.checked_add(self.width)?, self.y.checked_add(self.height)?))
    }

    pub fn fits_within(self, width: u32, height: u32) -> bool {
        self.checked_end()
            .is_some_and(|(right, bottom)| right <= width && bottom <= height)
    }
}

#[cfg(test)]
mod tests {
    use super::Rect;

    #[test]
    fn rejects_overflow_and_out_of_bounds() {
        assert!(!Rect::new(u32::MAX, 0, 2, 1).fits_within(u32::MAX, 1));
        assert!(!Rect::new(9, 0, 2, 1).fits_within(10, 1));
        assert!(Rect::new(8, 0, 2, 1).fits_within(10, 1));
    }
}
