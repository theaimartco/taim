#![allow(clippy::manual_clamp)]
use crate::length;
use crate::{Length, Size};

/// A set of size constraints for layouting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    min: Size,
    max: Size,
    compression: Size<bool>,
}

/// A compact, normalized, [`Hash`]-and-[`Eq`] representation of a
/// [`Limits`] value, suitable for use as a cache key.
///
/// `f32` implements neither [`Hash`] nor [`Eq`] (`NaN != NaN`, and there
/// are distinct bit patterns for the same mathematical value, e.g. `-0.0`
/// vs `0.0`), so [`Limits`] itself cannot be used directly as a hash map
/// key. [`Key`] fixes both problems by normalizing each component before
/// taking its bit pattern:
///
/// - `-0.0` is folded to `0.0` (they compare equal and must hash equal).
/// - `NaN` is folded to a single canonical bit pattern (a [`Limits`]
///   should never legitimately contain `NaN`, but a cache key must not
///   panic or behave inconsistently if one slips through).
///
/// Two [`Limits`] that are [`PartialEq`] always produce the same [`Key`],
/// and [`Key`] equality is a correct (not just approximate) stand-in for
/// [`Limits`] equality under this normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    min: (u32, u32),
    max: (u32, u32),
    compression: (bool, bool),
}

fn normalize_bits(value: f32) -> u32 {
    if value.is_nan() {
        f32::NAN.to_bits()
    } else if value == 0.0 {
        // Folds `-0.0` and `0.0` to the same bit pattern.
        0.0f32.to_bits()
    } else {
        value.to_bits()
    }
}

impl Limits {
    /// No limits
    pub const NONE: Limits = Limits {
        min: Size::ZERO,
        max: Size::INFINITE,
        compression: Size::new(false, false),
    };

    /// Creates new [`Limits`] with the given minimum and maximum [`Size`].
    pub const fn new(min: Size, max: Size) -> Limits {
        Limits::with_compression(min, max, Size::new(false, false))
    }

    /// Creates new [`Limits`] with the given minimun and maximum [`Size`], and
    /// whether fluid lengths should be compressed to intrinsic dimensions.
    pub const fn with_compression(min: Size, max: Size, compress: Size<bool>) -> Self {
        Limits {
            min,
            max,
            compression: compress,
        }
    }

    /// Returns the minimum [`Size`] of the [`Limits`].
    pub fn min(&self) -> Size {
        self.min
    }

    /// Returns the maximum [`Size`] of the [`Limits`].
    pub fn max(&self) -> Size {
        self.max
    }

    /// Returns the compression of the [`Limits`].
    pub fn compression(&self) -> Size<bool> {
        self.compression
    }

    /// Returns a compact, [`Hash`]-and-[`Eq`] [`Key`] identifying this
    /// [`Limits`], for use as a cache key (e.g. by
    /// [`layout::cache`](crate::layout::cache)).
    ///
    /// Equal [`Limits`] always produce an equal [`Key`]; this holds even
    /// across the `-0.0`/`0.0` and `NaN` edge cases that would otherwise
    /// make a raw bit-for-bit comparison unsound.
    pub fn key(&self) -> Key {
        Key {
            min: (
                normalize_bits(self.min.width),
                normalize_bits(self.min.height),
            ),
            max: (
                normalize_bits(self.max.width),
                normalize_bits(self.max.height),
            ),
            compression: (self.compression.width, self.compression.height),
        }
    }

    /// Applies a width constraint to the current [`Limits`].
    pub fn width(mut self, width: impl Into<Length>) -> Limits {
        match width.into() {
            Length::Shrink => {
                self.compression.width = true;
            }
            Length::Fit | Length::Fluid(_) => {
                self.compression.width = false;
            }
            Length::Fixed(amount) => {
                let new_width = amount.min(self.max.width).max(self.min.width);

                self.min.width = new_width;
                self.max.width = new_width;
                self.compression.width = false;
            }
            Length::Bounded { bounds, sizing } => {
                match bounds {
                    length::Bounds::Min(min) => {
                        self.min.width = min.min(self.max.width).max(self.min.width);
                    }
                    length::Bounds::Max(max) => {
                        self.max.width = max.min(self.max.width).max(self.min.width);
                    }
                    length::Bounds::Both { min, max } => {
                        self.min.width = min.min(self.max.width).max(self.min.width);
                        self.max.width = max.min(self.max.width).max(self.min.width);
                    }
                }

                match sizing {
                    length::Sizing::Shrink => {
                        self.compression.width = true;
                    }
                    length::Sizing::Fit => {
                        self.compression.width = false;
                    }
                    length::Sizing::Fill(_) => {}
                }
            }
            Length::Fill | Length::FillPortion(_) => {}
        }

        self
    }

    /// Applies a height constraint to the current [`Limits`].
    pub fn height(mut self, height: impl Into<Length>) -> Limits {
        match height.into() {
            Length::Shrink => {
                self.compression.height = true;
            }
            Length::Fit | Length::Fluid(_) => {
                self.compression.height = false;
            }
            Length::Fixed(amount) => {
                let new_height = amount.min(self.max.height).max(self.min.height);

                self.min.height = new_height;
                self.max.height = new_height;
                self.compression.height = false;
            }
            Length::Bounded { bounds, sizing } => {
                match bounds {
                    length::Bounds::Min(min) => {
                        self.min.height = min.min(self.max.height).max(self.min.height);
                    }
                    length::Bounds::Max(max) => {
                        self.max.height = max.min(self.max.height).max(self.min.height);
                    }
                    length::Bounds::Both { min, max } => {
                        self.min.height = min.min(self.max.height).max(self.min.height);
                        self.max.height = max.min(self.max.height).max(self.min.height);
                    }
                }

                match sizing {
                    length::Sizing::Shrink => {
                        self.compression.height = true;
                    }
                    length::Sizing::Fit => {
                        self.compression.height = false;
                    }
                    length::Sizing::Fill(_) => {}
                }
            }
            Length::Fill | Length::FillPortion(_) => {}
        }

        self
    }

    /// Shrinks the current [`Limits`] by the given [`Size`].
    pub fn shrink(&self, size: impl Into<Size>) -> Limits {
        let size = size.into();

        let min = Size::new(
            (self.min().width - size.width).max(0.0),
            (self.min().height - size.height).max(0.0),
        );

        let max = Size::new(
            (self.max().width - size.width).max(0.0),
            (self.max().height - size.height).max(0.0),
        );

        Limits {
            min,
            max,
            compression: self.compression,
        }
    }

    /// Removes the minimum [`Size`] constraint for the current [`Limits`].
    pub fn loose(&self) -> Limits {
        Limits {
            min: Size::ZERO,
            max: self.max,
            compression: self.compression,
        }
    }

    /// Computes the resulting [`Size`] that fits the [`Limits`] given
    /// some width and height requirements and the intrinsic size of
    /// some content.
    pub fn resolve(
        &self,
        width: impl Into<Length>,
        height: impl Into<Length>,
        intrinsic_size: Size,
    ) -> Size {
        Size::new(
            self.resolve_width(width, intrinsic_size.width),
            self.resolve_height(height, intrinsic_size.height),
        )
    }

    /// [Resolves](Self::resolve) only the width of the [`Limits`].
    pub fn resolve_width(&self, width: impl Into<Length>, intrinsic_width: f32) -> f32 {
        match width.into() {
            Length::Fill
            | Length::FillPortion(_)
            | Length::Bounded {
                sizing: length::Sizing::Fill(_),
                ..
            } if !self.compression.width => self.max.width,
            Length::Fixed(amount) => amount.min(self.max.width).max(self.min.width),
            _ => intrinsic_width.min(self.max.width).max(self.min.width),
        }
    }

    /// [Resolves](Self::resolve) only the height of the [`Limits`].
    pub fn resolve_height(&self, height: impl Into<Length>, intrinsic_height: f32) -> f32 {
        match height.into() {
            Length::Fill
            | Length::FillPortion(_)
            | Length::Bounded {
                sizing: length::Sizing::Fill(_),
                ..
            } if !self.compression.height => self.max.height,
            Length::Fixed(amount) => amount.min(self.max.height).max(self.min.height),
            _ => intrinsic_height.min(self.max.height).max(self.min.height),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn hash_of(key: Key) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn equal_limits_produce_equal_keys() {
        let a = Limits::new(Size::new(10.0, 20.0), Size::new(100.0, 200.0));
        let b = Limits::new(Size::new(10.0, 20.0), Size::new(100.0, 200.0));

        assert_eq!(a.key(), b.key());
        assert_eq!(hash_of(a.key()), hash_of(b.key()));
    }

    #[test]
    fn different_limits_produce_different_keys() {
        let a = Limits::new(Size::new(10.0, 20.0), Size::new(100.0, 200.0));
        let b = Limits::new(Size::new(11.0, 20.0), Size::new(100.0, 200.0));

        assert_ne!(a.key(), b.key());
    }

    #[test]
    fn different_compression_produces_different_keys() {
        let a =
            Limits::with_compression(Size::ZERO, Size::new(100.0, 200.0), Size::new(false, false));
        let b =
            Limits::with_compression(Size::ZERO, Size::new(100.0, 200.0), Size::new(true, false));

        assert_ne!(a.key(), b.key());
    }

    #[test]
    fn negative_zero_and_positive_zero_produce_the_same_key() {
        let a = Limits::new(Size::new(-0.0, -0.0), Size::new(100.0, 100.0));
        let b = Limits::new(Size::new(0.0, 0.0), Size::new(100.0, 100.0));

        assert_eq!(a.key(), b.key());
        assert_eq!(hash_of(a.key()), hash_of(b.key()));
    }

    #[test]
    fn nan_does_not_panic_and_is_self_consistent() {
        let a = Limits::new(Size::new(f32::NAN, 0.0), Size::new(100.0, 100.0));
        let b = Limits::new(Size::new(f32::NAN, 0.0), Size::new(100.0, 100.0));

        assert_eq!(a.key(), b.key());
    }

    #[test]
    fn key_is_stable_across_clones() {
        let limits = Limits::new(Size::new(5.0, 5.0), Size::new(50.0, 50.0));

        assert_eq!(limits.key(), limits.key());
    }
}
