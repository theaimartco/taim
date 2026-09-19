use crate::core::{Point, Radians, Rectangle, Vector};
use crate::graphics::geometry::Style;
use crate::graphics::gradient::Gradient;

use lyon::geom::euclid;

#[derive(Debug)]
pub(super) struct Stack {
    previous: Vec<Transform>,
    pub current: Transform,
}

impl Stack {
    pub fn new() -> Self {
        Self {
            previous: Vec::with_capacity(4),
            current: Transform::identity(),
        }
    }

    pub fn push(&mut self) {
        self.previous.push(self.current);
    }

    pub fn pop(&mut self) {
        if let Some(previous) = self.previous.pop() {
            self.current = previous;
        } else {
            log::warn!("TAIM geometry transform stack underflow ignored");
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Transform(pub lyon::math::Transform);

impl Transform {
    fn identity() -> Self {
        Self(lyon::math::Transform::identity())
    }

    pub fn is_identity(self) -> bool {
        self.0 == lyon::math::Transform::identity()
    }

    pub fn is_positive_axis_aligned(self) -> bool {
        self.0.m12.abs() < 2.0 * f32::EPSILON
            && self.0.m21.abs() < 2.0 * f32::EPSILON
            && self.0.m11 >= 0.0
            && self.0.m22 >= 0.0
    }

    pub fn scale(self) -> (f32, f32) {
        (self.0.m11, self.0.m22)
    }

    pub fn point(self, point: Point) -> Point {
        let point = self
            .0
            .transform_point(euclid::Point2D::new(point.x, point.y));
        Point::new(point.x, point.y)
    }

    pub fn vector(self, vector: Vector) -> Vector {
        let vector = self
            .0
            .transform_vector(euclid::Vector2D::new(vector.x, vector.y));
        Vector::new(vector.x, vector.y)
    }

    pub fn style(self, style: Style) -> Style {
        match style {
            Style::Solid(color) => Style::Solid(color),
            Style::Gradient(mut gradient) => {
                match &mut gradient {
                    Gradient::Linear(linear) => {
                        linear.start = self.point(linear.start);
                        linear.end = self.point(linear.end);
                    }
                }
                Style::Gradient(gradient)
            }
        }
    }

    pub fn rectangle(self, rectangle: Rectangle) -> (Rectangle, Radians) {
        let top_left = self.point(rectangle.position());
        let top_right = self.point(rectangle.position() + Vector::new(rectangle.width, 0.0));
        let bottom_left = self.point(rectangle.position() + Vector::new(0.0, rectangle.height));
        Rectangle::with_vertices(top_left, top_right, bottom_left)
    }
}
