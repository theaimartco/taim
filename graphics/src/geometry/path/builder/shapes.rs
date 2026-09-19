//! Robust construction of common path primitives.

use super::{Builder, is_finite};
use crate::core::border;
use crate::core::{Point, Radians, Size};
use crate::geometry::path::{Arc, arc};

use lyon_path::builder::SvgPathBuilder;
use lyon_path::{ArcFlags, geom, math};

impl Builder {
    /// Adds a clockwise circular arc.
    #[inline]
    pub fn arc(&mut self, arc: Arc) {
        self.ellipse(arc.into());
    }

    /// Joins two segments with a tangent circular arc.
    pub fn arc_to(&mut self, a: Point, b: Point, radius: f32) {
        if !is_finite(a) || !is_finite(b) || !radius.is_finite() {
            return;
        }

        let radius = radius.abs();
        let start = self.raw.current_position();
        let mid = math::Point::new(a.x, a.y);
        let end = math::Point::new(b.x, b.y);

        if start == mid || mid == end || radius <= f32::EPSILON {
            let _ = self.raw.line_to(mid);
            return;
        }

        let double_area =
            start.x * (mid.y - end.y) + mid.x * (end.y - start.y) + end.x * (start.y - mid.y);
        if !double_area.is_finite() || double_area.abs() <= f32::EPSILON {
            let _ = self.raw.line_to(mid);
            return;
        }

        let to_start = (start - mid).normalize();
        let to_end = (end - mid).normalize();
        let half_angle = to_start.dot(to_end).clamp(-1.0, 1.0).acos() * 0.5;
        let tangent = half_angle.tan();
        if !tangent.is_finite() || tangent.abs() <= f32::EPSILON {
            let _ = self.raw.line_to(mid);
            return;
        }

        let adjacent = radius / tangent;
        let arc_start = mid + to_start * adjacent;
        let arc_end = mid + to_end * adjacent;
        let _ = self.raw.line_to(arc_start);
        self.raw.arc_to(
            math::Vector::new(radius, radius),
            math::Angle::radians(0.0),
            ArcFlags {
                large_arc: false,
                sweep: to_start.cross(to_end) < 0.0,
            },
            arc_end,
        );
    }

    /// Adds a clockwise elliptical arc.
    pub fn ellipse(&mut self, arc: arc::Elliptical) {
        if !is_finite(arc.center)
            || !arc.radii.x.is_finite()
            || !arc.radii.y.is_finite()
            || !arc.rotation.0.is_finite()
            || !arc.start_angle.0.is_finite()
            || !arc.end_angle.0.is_finite()
        {
            return;
        }

        let radii = Size::new(arc.radii.x.abs(), arc.radii.y.abs());
        if radii.width <= f32::EPSILON || radii.height <= f32::EPSILON {
            return;
        }

        let arc = geom::Arc {
            center: math::Point::new(arc.center.x, arc.center.y),
            radii: math::Vector::new(radii.width, radii.height),
            x_rotation: math::Angle::radians(arc.rotation.0),
            start_angle: math::Angle::radians(arc.start_angle.0),
            sweep_angle: math::Angle::radians((arc.end_angle - arc.start_angle).0),
        };

        let _ = self.raw.move_to(arc.sample(0.0));
        arc.cast::<f64>().for_each_quadratic_bezier(&mut |curve| {
            let curve = curve.cast::<f32>();
            let _ = self.raw.quadratic_bezier_to(curve.ctrl, curve.to);
        });
    }

    /// Adds an axis-aligned rectangle.
    #[inline]
    pub fn rectangle(&mut self, top_left: Point, size: Size) {
        if !is_drawable(top_left, size) {
            return;
        }

        self.move_to(top_left);
        self.line_to(Point::new(top_left.x + size.width, top_left.y));
        self.line_to(Point::new(
            top_left.x + size.width,
            top_left.y + size.height,
        ));
        self.line_to(Point::new(top_left.x, top_left.y + size.height));
        self.close();
    }

    /// Adds an axis-aligned rounded rectangle.
    pub fn rounded_rectangle(&mut self, top_left: Point, size: Size, radius: border::Radius) {
        if !is_drawable(top_left, size) {
            return;
        }

        let limit = (size.height * 0.5).min(size.width * 0.5);
        let [top_left_radius, top_right, bottom_right, bottom_left] = radius.into();
        let top_left_radius = limit.min(top_left_radius.max(0.0));
        let top_right = limit.min(top_right.max(0.0));
        let bottom_right = limit.min(bottom_right.max(0.0));
        let bottom_left = limit.min(bottom_left.max(0.0));
        let right = top_left.x + size.width;
        let bottom = top_left.y + size.height;

        self.move_to(Point::new(top_left.x + top_left_radius, top_left.y));
        self.line_to(Point::new(right - top_right, top_left.y));
        self.arc_to(
            Point::new(right, top_left.y),
            Point::new(right, top_left.y + top_right),
            top_right,
        );
        self.line_to(Point::new(right, bottom - bottom_right));
        self.arc_to(
            Point::new(right, bottom),
            Point::new(right - bottom_right, bottom),
            bottom_right,
        );
        self.line_to(Point::new(top_left.x + bottom_left, bottom));
        self.arc_to(
            Point::new(top_left.x, bottom),
            Point::new(top_left.x, bottom - bottom_left),
            bottom_left,
        );
        self.line_to(Point::new(top_left.x, top_left.y + top_left_radius));
        self.arc_to(
            top_left,
            Point::new(top_left.x + top_left_radius, top_left.y),
            top_left_radius,
        );
        self.close();
    }

    /// Adds a full circle.
    #[inline]
    pub fn circle(&mut self, center: Point, radius: f32) {
        if !is_finite(center) || !radius.is_finite() || radius <= f32::EPSILON {
            return;
        }

        self.arc(Arc {
            center,
            radius,
            start_angle: Radians::ZERO,
            end_angle: Radians::TAU,
        });
    }
}

#[inline]
fn is_drawable(origin: Point, size: Size) -> bool {
    is_finite(origin)
        && size.width.is_finite()
        && size.height.is_finite()
        && size.width > 0.0
        && size.height > 0.0
}
