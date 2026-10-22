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
            if value.is_finite() {
                value.clamp(min, max.max(min))
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
        let axis = |axis: Axis, min: f32, max: f32| match axis {
            Axis::Fixed(length) => {
                let length = length.clamp(min, max.max(min));
                (length, length)
            }
            Axis::Fill(_) if max.is_finite() => (max, max),
            Axis::Fill(_) | Axis::Hug => (min, max),
        };

        let (min_width, max_width) = axis(sizing.width, self.min.width, self.max.width);
        let (min_height, max_height) = axis(sizing.height, self.min.height, self.max.height);

        Self {
            min: Size::new(min_width, min_height),
            max: Size::new(max_width, max_height),
        }
    }
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
    fn inset_never_goes_negative() {
        let inset = Constraints::loose(Size::new(10.0, 10.0)).inset(Padding::new(8.0));

        assert_eq!(inset.max, Size::ZERO);
    }
}
