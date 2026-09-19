//! Space stuff around the perimeter.
mod conversion;

use crate::{Pixels, Size};

/// An amount of space to pad for each side of a box
///
/// You can leverage the `From` trait to build [`Padding`] conveniently:
///
/// ```
/// # use taim_core::Padding;
/// #
/// let padding = Padding::from(20);              // 20px on all sides
/// let padding = Padding::from([10, 20]);        // top/bottom, left/right
/// ```
///
/// Normally, the `padding` method of a widget will ask for an `Into<Padding>`,
/// so you can easily write:
///
/// ```
/// # use taim_core::Padding;
/// #
/// # struct Widget;
/// #
/// impl Widget {
///     # pub fn new() -> Self { Self }
///     #
///     pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
///         // ...
///         self
///     }
/// }
///
/// let widget = Widget::new().padding(20);              // 20px on all sides
/// let widget = Widget::new().padding([10, 20]);        // top/bottom, left/right
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct Padding {
    /// Top padding
    pub top: f32,
    /// Right padding
    pub right: f32,
    /// Bottom padding
    pub bottom: f32,
    /// Left padding
    pub left: f32,
}

/// Create a [`Padding`] that is equal on all sides.
pub fn all(padding: impl Into<Pixels>) -> Padding {
    Padding::new(padding.into().0)
}

/// Create some top [`Padding`].
pub fn top(padding: impl Into<Pixels>) -> Padding {
    Padding::default().top(padding)
}

/// Create some bottom [`Padding`].
pub fn bottom(padding: impl Into<Pixels>) -> Padding {
    Padding::default().bottom(padding)
}

/// Create some left [`Padding`].
pub fn left(padding: impl Into<Pixels>) -> Padding {
    Padding::default().left(padding)
}

/// Create some right [`Padding`].
pub fn right(padding: impl Into<Pixels>) -> Padding {
    Padding::default().right(padding)
}

/// Create some [`Padding`] with equal left and right sides.
pub fn horizontal(padding: impl Into<Pixels>) -> Padding {
    Padding::default().horizontal(padding)
}

/// Create some [`Padding`] with equal top and bottom sides.
pub fn vertical(padding: impl Into<Pixels>) -> Padding {
    Padding::default().vertical(padding)
}

impl Padding {
    /// Padding of zero
    pub const ZERO: Padding = Padding {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    /// Create a [`Padding`] that is equal on all sides.
    pub const fn new(padding: f32) -> Padding {
        let padding = valid(padding);
        Padding {
            top: padding,
            right: padding,
            bottom: padding,
            left: padding,
        }
    }

    /// Creates padding from horizontal and vertical axis values.
    pub const fn axes(horizontal: f32, vertical: f32) -> Self {
        Self {
            top: valid(vertical),
            right: valid(horizontal),
            bottom: valid(vertical),
            left: valid(horizontal),
        }
    }

    /// Creates padding from clockwise side values, starting at the top.
    pub const fn sides(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top: valid(top),
            right: valid(right),
            bottom: valid(bottom),
            left: valid(left),
        }
    }

    /// Sets the [`top`] of the [`Padding`].
    ///
    /// [`top`]: Self::top
    pub fn top(self, top: impl Into<Pixels>) -> Self {
        Self {
            top: valid(top.into().0),
            ..self
        }
    }

    /// Sets the [`bottom`] of the [`Padding`].
    ///
    /// [`bottom`]: Self::bottom
    pub fn bottom(self, bottom: impl Into<Pixels>) -> Self {
        Self {
            bottom: valid(bottom.into().0),
            ..self
        }
    }

    /// Sets the [`left`] of the [`Padding`].
    ///
    /// [`left`]: Self::left
    pub fn left(self, left: impl Into<Pixels>) -> Self {
        Self {
            left: valid(left.into().0),
            ..self
        }
    }

    /// Sets the [`right`] of the [`Padding`].
    ///
    /// [`right`]: Self::right
    pub fn right(self, right: impl Into<Pixels>) -> Self {
        Self {
            right: valid(right.into().0),
            ..self
        }
    }

    /// Sets the [`left`] and [`right`] of the [`Padding`].
    ///
    /// [`left`]: Self::left
    /// [`right`]: Self::right
    pub fn horizontal(self, horizontal: impl Into<Pixels>) -> Self {
        let horizontal = horizontal.into();

        Self {
            left: valid(horizontal.0),
            right: valid(horizontal.0),
            ..self
        }
    }

    /// Sets the [`top`] and [`bottom`] of the [`Padding`].
    ///
    /// [`top`]: Self::top
    /// [`bottom`]: Self::bottom
    pub fn vertical(self, vertical: impl Into<Pixels>) -> Self {
        let vertical = vertical.into();

        Self {
            top: valid(vertical.0),
            bottom: valid(vertical.0),
            ..self
        }
    }

    /// Returns the total amount of horizontal [`Padding`].
    #[inline]
    pub const fn x(self) -> f32 {
        valid(self.left) + valid(self.right)
    }

    /// Returns the total amount of vertical [`Padding`].
    #[inline]
    pub const fn y(self) -> f32 {
        valid(self.top) + valid(self.bottom)
    }

    /// Returns a finite, non-negative copy safe for layout arithmetic.
    #[inline]
    pub const fn normalized(self) -> Self {
        Self {
            top: valid(self.top),
            right: valid(self.right),
            bottom: valid(self.bottom),
            left: valid(self.left),
        }
    }

    /// Returns whether all sides are zero after normalization.
    #[inline]
    pub fn is_zero(self) -> bool {
        self.x() == 0.0 && self.y() == 0.0
    }

    /// Scales every side by a finite, non-negative factor.
    pub fn scale(self, factor: f32) -> Self {
        let factor = valid(factor);
        Self::sides(
            self.top * factor,
            self.right * factor,
            self.bottom * factor,
            self.left * factor,
        )
    }

    /// Interpolates each side toward `target` for layout animation.
    pub fn lerp(self, target: Self, amount: f32) -> Self {
        let amount = if amount.is_finite() {
            amount.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let blend = |start: f32, end: f32| start + (end - start) * amount;
        Self::sides(
            blend(self.top, target.top),
            blend(self.right, target.right),
            blend(self.bottom, target.bottom),
            blend(self.left, target.left),
        )
    }

    /// Fits the [`Padding`] between the provided `inner` and `outer` [`Size`].
    pub fn fit(self, inner: Size, outer: Size) -> Self {
        let padding = self.normalized();
        let available = Size::new(
            available(outer.width - inner.width),
            available(outer.height - inner.height),
        );
        let (left, right) = fit_axis(padding.left, padding.right, available.width);
        let (top, bottom) = fit_axis(padding.top, padding.bottom, available.height);

        Padding {
            top,
            right,
            bottom,
            left,
        }
    }
}

#[inline]
fn fit_axis(leading: f32, trailing: f32, available: f32) -> (f32, f32) {
    let total = leading + trailing;
    if total <= available || total == 0.0 {
        (leading, trailing)
    } else {
        let scale = available / total;
        (leading * scale, trailing * scale)
    }
}

#[inline]
pub(super) const fn valid(value: f32) -> f32 {
    if value >= 0.0 && value <= f32::MAX {
        value
    } else {
        0.0
    }
}

/// Normalizes available layout space while preserving an unbounded axis.
///
/// Padding values themselves must be finite, but a layout limit may
/// legitimately be positive infinity. Folding that infinity to zero would
/// collapse padding around intrinsically-sized content.
#[inline]
const fn available(value: f32) -> f32 {
    if value.is_nan() || value < 0.0 {
        0.0
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_preserves_padding_on_unbounded_axes() {
        let padding = Padding::new(5.0).fit(Size::ZERO, Size::INFINITE);

        assert_eq!(padding, Padding::new(5.0));
    }

    #[test]
    fn fit_clamps_padding_to_finite_available_space() {
        let padding = Padding::new(5.0).fit(Size::new(8.0, 9.0), Size::new(10.0, 10.0));

        assert_eq!(
            padding,
            Padding {
                top: 0.5,
                right: 1.0,
                bottom: 0.5,
                left: 1.0,
            }
        );
    }

    #[test]
    fn fit_preserves_asymmetric_side_ratio() {
        let padding = Padding::sides(8.0, 2.0, 2.0, 6.0)
            .fit(Size::ZERO, Size::new(4.0, 5.0));

        assert_eq!(padding, Padding::sides(4.0, 1.0, 1.0, 3.0));
    }
}
