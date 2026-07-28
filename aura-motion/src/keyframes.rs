//! Validated, normalized keyframes.

use alloc::vec::Vec;

use crate::tween::Interpolate;

/// A value at normalized timeline progress.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keyframe<T> {
    /// Normalized offset in the inclusive `0.0..=1.0` range.
    pub offset: f32,
    /// Value at this offset.
    pub value: T,
}

/// Validation failures for [`Keyframes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyframesError {
    /// No frames were supplied.
    Empty,
    /// An offset was NaN, infinite, or outside the normalized range.
    InvalidOffset,
    /// Offsets were not strictly increasing.
    Unordered,
}

/// A sorted sequence of normalized keyframes.
#[derive(Debug, Clone, PartialEq)]
pub struct Keyframes<T> {
    frames: Vec<Keyframe<T>>,
}

impl<T> Keyframes<T> {
    /// Validates and stores normalized keyframes.
    pub fn new(frames: Vec<Keyframe<T>>) -> Result<Self, KeyframesError> {
        let Some(first) = frames.first() else {
            return Err(KeyframesError::Empty);
        };

        if !first.offset.is_finite() || !(0.0..=1.0).contains(&first.offset) {
            return Err(KeyframesError::InvalidOffset);
        }

        for pair in frames.windows(2) {
            if !pair[1].offset.is_finite() || !(0.0..=1.0).contains(&pair[1].offset) {
                return Err(KeyframesError::InvalidOffset);
            }
            if pair[0].offset >= pair[1].offset {
                return Err(KeyframesError::Unordered);
            }
        }

        Ok(Self { frames })
    }

    /// Returns the validated frames.
    pub fn frames(&self) -> &[Keyframe<T>] {
        &self.frames
    }
}

impl<T: Interpolate> Keyframes<T> {
    /// Samples a value at normalized progress.
    pub fn sample(&self, progress: f32) -> T {
        let progress = progress.clamp(0.0, 1.0);
        let first = self.frames[0];
        let last = self.frames[self.frames.len() - 1];

        if progress <= first.offset {
            return first.value;
        }
        if progress >= last.offset {
            return last.value;
        }

        for pair in self.frames.windows(2) {
            let from = pair[0];
            let to = pair[1];
            if progress <= to.offset {
                let amount = (progress - from.offset) / (to.offset - from.offset);
                return T::interpolate(from.value, to.value, amount);
            }
        }

        last.value
    }
}
