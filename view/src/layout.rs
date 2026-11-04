//! Sizing preferences and size constraints.
use crate::{Padding, Size};

/// How a view wants to be sized along one axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Axis {
    /// As small as its content allows.
    Hug,
    /// Exactly this many logical pixels.
    Fixed(f32),
    /// A share of the space left over by its siblings, proportional to
    /// `weight`.
    Fill(u16),
}

impl Axis {
    /// The share weight, or `None` when the axis does not fill.
    pub fn fill_weight(self) -> Option<u16> {
        match self {
            Self::Fill(weight) => Some(weight.max(1)),
            Self::Hug | Self::Fixed(_) => None,
        }
    }
}

/// How a view wants to be sized along both axes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sizing {
    /// Horizontal preference.
    pub width: Axis,
    /// Vertical preference.
    pub height: Axis,
}

impl Sizing {
    /// Content-sized on both axes.
    pub const HUG: Self = Self {
        width: Axis::Hug,
        height: Axis::Hug,
    };

    /// Fills both axes with weight 1.
    pub const FILL: Self = Self {
        width: Axis::Fill(1),
        height: Axis::Fill(1),
    };
}

/// The range of sizes a parent accepts from a child.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Constraints {
    /// Smallest acceptable size.
    pub min: Size,
    /// Largest acceptable size; may be infinite.
    pub max: Size,
}

impl Constraints {
    /// Anything from zero up to `max`.
    pub fn loose(max: Size) -> Self {
        Self {
            min: Size::ZERO,
            max,
        }
    }

    /// Exactly `size`.
    pub fn tight(size: Size) -> Self {
        Self {
            min: size,
            max: size,
        }
    }

    /// Brings `size` into range. Non-finite sizes collapse to the minimum.
    pub fn clamp(&self, size: Size) -> Size {
        let fit = |value: f32, min: f32, max: f32| {
            let (min, max) = bounds(min, max);
            if value.is_finite() {
                value.clamp(min, max)
            } else {
                min
            }
        };

        Size::new(
            fit(size.width, self.min.width, self.max.width),
            fit(size.height, self.min.height, self.max.height),
        )
    }

    /// The constraints left for content inside `padding`.
    pub fn inset(&self, padding: Padding) -> Self {
        let shrink = |size: Size| {
            Size::new(
                (size.width - (padding.left + padding.right)).max(0.0),
                (size.height - (padding.top + padding.bottom)).max(0.0),
            )
        };

        Self {
            min: shrink(self.min),
            max: shrink(self.max),
        }
    }

    /// Same maximum, no minimum.
    pub fn loosen(&self) -> Self {
        Self::loose(self.max)
    }

    /// Applies `sizing`: fixed axes become tight, fill axes take the
    /// maximum when it is finite.
    pub fn with_sizing(&self, sizing: Sizing) -> Self {
        let axis = |axis: Axis, min: f32, max: f32| {
            let (min, max) = bounds(min, max);
            match axis {
                Axis::Fixed(length) => {
                    let length = if length.is_finite() {
                        length.clamp(min, max)
                    } else {
                        min
                    };
                    (length, length)
                }
                Axis::Fill(_) if max.is_finite() => (max, max),
                Axis::Fill(_) | Axis::Hug => (min, max),
            }
        };

        let (min_width, max_width) = axis(sizing.width, self.min.width, self.max.width);
        let (min_height, max_height) = axis(sizing.height, self.min.height, self.max.height);

        Self {
            min: Size::new(min_width, min_height),
            max: Size::new(max_width, max_height),
        }
    }
}

// Public constraints may be assembled directly by custom views. Keep their
// numeric bounds valid before calling f32::clamp, which panics on NaN bounds.
// Infinity is useful only as an unbounded maximum, never as a minimum.
fn bounds(min: f32, max: f32) -> (f32, f32) {
    let min = if min.is_finite() { min.max(0.0) } else { 0.0 };
    let max = if max.is_nan() { min } else { max.max(min) };
    (min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_keeps_sizes_in_range_and_rejects_infinity() {
        let constraints = Constraints {
            min: Size::new(10.0, 10.0),
            max: Size::new(100.0, f32::INFINITY),
        };

        assert_eq!(
            constraints.clamp(Size::new(5.0, 50.0)),
            Size::new(10.0, 50.0)
        );
        assert_eq!(
            constraints.clamp(Size::new(f32::INFINITY, 500.0)),
            Size::new(10.0, 500.0)
        );
    }

    #[test]
    fn sizing_turns_fixed_into_tight_and_fill_into_max() {
        let constraints = Constraints::loose(Size::new(200.0, 100.0));
        let sized = constraints.with_sizing(Sizing {
            width: Axis::Fixed(40.0),
            height: Axis::Fill(1),
        });

        assert_eq!(sized.min, Size::new(40.0, 100.0));
        assert_eq!(sized.max, Size::new(40.0, 100.0));
    }

    #[test]
    fn malformed_public_constraints_always_produce_finite_sizes() {
        let values = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -10.0, 0.0, 50.0];
        for min in values {
            for max in values {
                let constraints = Constraints {
                    min: Size::new(min, min),
                    max: Size::new(max, max),
                };
                for value in values {
                    let size = constraints.clamp(Size::new(value, value));
                    assert!(size.width.is_finite() && size.width >= 0.0);
                    for axis in [Axis::Fixed(value), Axis::Fill(1), Axis::Hug] {
                        let sized = constraints.with_sizing(Sizing {
                            width: axis,
                            height: axis,
                        });
                        assert!(sized.min.width.is_finite() && sized.min.width >= 0.0);
                        assert!(!sized.max.width.is_nan());
                        assert!(sized.max.width >= sized.min.width);
                    }
                }
            }
        }
    }

    #[test]
    fn non_finite_fixed_size_uses_parent_minimum() {
        let constraints = Constraints {
            min: Size::new(10.0, 20.0),
            max: Size::new(100.0, f32::INFINITY),
        };
        let sized = constraints.with_sizing(Sizing {
            width: Axis::Fixed(f32::NAN),
            height: Axis::Fixed(f32::INFINITY),
        });
        assert_eq!(sized.min, constraints.min);
        assert_eq!(sized.max, constraints.min);
    }

    #[test]
    fn inset_never_goes_negative() {
        let inset = Constraints::loose(Size::new(10.0, 10.0)).inset(Padding::new(8.0));

        assert_eq!(inset.max, Size::ZERO);
    }
}
