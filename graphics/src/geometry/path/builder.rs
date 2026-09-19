mod shapes;

use crate::geometry::path::Path;

use crate::core::Point;

use lyon_path::builder;
use lyon_path::math;

/// A [`Path`] builder.
///
/// Once a [`Path`] is built, it can no longer be mutated.
pub struct Builder {
    raw: builder::WithSvg<lyon_path::path::BuilderImpl>,
}

impl Builder {
    /// Creates a new [`Builder`].
    pub fn new() -> Builder {
        Self::with_capacity(0, 0)
    }

    /// Creates a builder with capacity for the expected endpoints and control
    /// points, avoiding growth while recording known-size shapes.
    pub fn with_capacity(endpoints: usize, control_points: usize) -> Builder {
        let mut raw = lyon_path::Path::builder().with_svg();
        raw.reserve(endpoints, control_points);
        Builder { raw }
    }

    /// Reserves storage for additional endpoints and control points.
    #[inline]
    pub fn reserve(&mut self, endpoints: usize, control_points: usize) {
        self.raw.reserve(endpoints, control_points);
    }

    /// Moves the starting point of a new sub-path to the given `Point`.
    #[inline]
    pub fn move_to(&mut self, point: Point) {
        if is_finite(point) {
            let _ = self.raw.move_to(math::Point::new(point.x, point.y));
        }
    }

    /// Connects the last point in the [`Path`] to the given `Point` with a
    /// straight line.
    #[inline]
    pub fn line_to(&mut self, point: Point) {
        if is_finite(point) {
            let _ = self.raw.line_to(math::Point::new(point.x, point.y));
        }
    }

    /// Adds a cubic Bézier curve to the [`Path`] given its two control points
    /// and its end point.
    #[inline]
    pub fn bezier_curve_to(&mut self, control_a: Point, control_b: Point, to: Point) {
        if !is_finite(control_a) || !is_finite(control_b) || !is_finite(to) {
            return;
        }

        let _ = self.raw.cubic_bezier_to(
            math::Point::new(control_a.x, control_a.y),
            math::Point::new(control_b.x, control_b.y),
            math::Point::new(to.x, to.y),
        );
    }

    /// Adds a quadratic Bézier curve to the [`Path`] given its control point
    /// and its end point.
    #[inline]
    pub fn quadratic_curve_to(&mut self, control: Point, to: Point) {
        if !is_finite(control) || !is_finite(to) {
            return;
        }

        let _ = self.raw.quadratic_bezier_to(
            math::Point::new(control.x, control.y),
            math::Point::new(to.x, to.y),
        );
    }

    /// Closes the current sub-path in the [`Path`] with a straight line to
    /// the starting point.
    #[inline]
    pub fn close(&mut self) {
        self.raw.close();
    }

    /// Builds the [`Path`] of this [`Builder`].
    #[inline]
    pub fn build(self) -> Path {
        Path {
            raw: self.raw.build(),
        }
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
pub(super) fn is_finite(point: Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}
