//! Two-axis constraints and resolution for widget measurement.
mod key;
mod resolve;

pub use key::Key;

use crate::Size;

/// Minimum, maximum, and intrinsic-compression constraints for two axes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    pub(super) min: Size,
    pub(super) max: Size,
    pub(super) compression: Size<bool>,
}

impl Limits {
    /// Unbounded limits with no compression.
    pub const NONE: Self = Self {
        min: Size::ZERO,
        max: Size::INFINITE,
        compression: Size::new(false, false),
    };

    /// Creates limits with no intrinsic compression.
    pub const fn new(min: Size, max: Size) -> Self {
        Self::with_compression(min, max, Size::new(false, false))
    }

    /// Creates limits with explicit per-axis intrinsic compression.
    pub const fn with_compression(min: Size, max: Size, compression: Size<bool>) -> Self {
        Self {
            min,
            max,
            compression,
        }
    }

    /// Returns minimum extents.
    #[inline]
    pub const fn min(&self) -> Size {
        self.min
    }

    /// Returns maximum extents.
    #[inline]
    pub const fn max(&self) -> Size {
        self.max
    }

    /// Returns intrinsic-compression flags.
    #[inline]
    pub const fn compression(&self) -> Size<bool> {
        self.compression
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_distinguishes_geometry_and_compression() {
        let base = Limits::new(Size::new(10.0, 20.0), Size::new(100.0, 200.0));
        let width_changed = Limits::new(Size::new(11.0, 20.0), Size::new(100.0, 200.0));
        let compressed = Limits::with_compression(
            Size::new(10.0, 20.0),
            Size::new(100.0, 200.0),
            Size::new(true, false),
        );

        assert_ne!(base.key(), width_changed.key());
        assert_ne!(base.key(), compressed.key());
    }

    #[test]
    fn cache_key_normalizes_zero_and_nan() {
        let positive = Limits::new(Size::new(0.0, f32::NAN), Size::new(100.0, 100.0));
        let negative = Limits::new(Size::new(-0.0, f32::NAN), Size::new(100.0, 100.0));

        assert_eq!(positive.key(), negative.key());
    }
}
