use super::{Rectangle, far_edge};
use crate::alignment;
use crate::{Padding, Point, Radians, Size, Vector};

impl Rectangle<f32> {
    /// Recovers an axis-aligned rectangle and rotation from three vertices.
    pub fn with_vertices(top_left: Point, top_right: Point, bottom_left: Point) -> (Self, Radians) {
        let horizontal = top_right - top_left;
        let vertical = bottom_left - top_left;
        let width = horizontal.x.hypot(horizontal.y);
        let height = vertical.x.hypot(vertical.y);
        let rotation = horizontal
            .y
            .atan2(horizontal.x)
            .rem_euclid(std::f32::consts::TAU);
        let center = Point::new(
            (top_right.x + bottom_left.x) * 0.5,
            (top_right.y + bottom_left.y) * 0.5,
        );
        let (sine, cosine) = (-rotation).sin_cos();
        let offset = top_left - center;
        let origin = Point::new(
            center.x + offset.x * cosine - offset.y * sine,
            center.y + offset.x * sine + offset.y * cosine,
        );
        (
            Self::new(origin, Size::new(width, height)),
            Radians(rotation),
        )
    }

    /// Rounds both corners and derives the resulting extent.
    pub fn round(self) -> Self {
        let left = self.x.round();
        let top = self.y.round();
        let right = far_edge(self.x, self.width).round();
        let bottom = far_edge(self.y, self.height).round();
        Self::new(Point::new(left, top), Size::new(right - left, bottom - top))
    }

    /// Converts a rounded nonempty rectangle to unsigned coordinates.
    pub fn snap(self) -> Option<Rectangle<u32>> {
        let rounded = self.round();
        (rounded.width >= 1.0 && rounded.height >= 1.0).then_some(Rectangle {
            x: rounded.x as u32,
            y: rounded.y as u32,
            width: rounded.width as u32,
            height: rounded.height as u32,
        })
    }

    /// Expands each side by the supplied padding.
    pub fn expand(self, padding: impl Into<Padding>) -> Self {
        let padding = padding.into();
        Self::new(
            Point::new(self.x - padding.left, self.y - padding.top),
            Size::new(self.width + padding.x(), self.height + padding.y()),
        )
    }

    /// Moves each side inward by the supplied padding.
    pub fn shrink(self, padding: impl Into<Padding>) -> Self {
        let padding = padding.into();
        Self::new(
            Point::new(self.x + padding.left, self.y + padding.top),
            Size::new(self.width - padding.x(), self.height - padding.y()),
        )
    }

    /// Returns the axis-aligned envelope after center rotation.
    pub fn rotate(self, rotation: Radians) -> Self {
        let size = self.size().rotate(rotation);
        Self::new(
            Point::new(
                self.center_x() - size.width * 0.5,
                self.center_y() - size.height * 0.5,
            ),
            size,
        )
    }

    /// Center-scales this rectangle while keeping its position anchor stable.
    pub fn zoom(self, zoom: f32) -> Self {
        let width = self.width * zoom;
        let height = self.height * zoom;
        Self::new(
            Point::new(
                self.x - (width - self.width) * 0.5,
                self.y - (height - self.height) * 0.5,
            ),
            Size::new(width, height),
        )
    }

    /// Positions `size` at the requested horizontal and vertical anchors.
    pub fn anchor(
        &self,
        size: Size,
        align_x: impl Into<alignment::Horizontal>,
        align_y: impl Into<alignment::Vertical>,
    ) -> Point {
        let x = match align_x.into() {
            alignment::Horizontal::Left => self.x,
            alignment::Horizontal::Center => self.x + (self.width - size.width) * 0.5,
            alignment::Horizontal::Right => far_edge(self.x, self.width) - size.width,
        };
        let y = match align_y.into() {
            alignment::Vertical::Top => self.y,
            alignment::Vertical::Center => self.y + (self.height - size.height) * 0.5,
            alignment::Vertical::Bottom => far_edge(self.y, self.height) - size.height,
        };
        Point::new(x, y)
    }

    /// Finds the two boundary points of a center chord at `angle`.
    pub fn chord(&self, angle: impl Into<Radians>) -> (Point, Point) {
        let angle = angle.into().0 - std::f32::consts::FRAC_PI_2;
        let (sine, cosine) = angle.sin_cos();
        let direction = Vector::new(cosine, sine);
        let horizontal = if cosine.abs() > f32::EPSILON {
            self.width * 0.5 / cosine.abs()
        } else {
            f32::INFINITY
        };
        let vertical = if sine.abs() > f32::EPSILON {
            self.height * 0.5 / sine.abs()
        } else {
            f32::INFINITY
        };
        let distance = horizontal.min(vertical);
        let center = self.center();
        (center - direction * distance, center + direction * distance)
    }
}
