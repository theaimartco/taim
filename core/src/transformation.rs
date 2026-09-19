mod ops;

use crate::Vector;
use glam::{Mat4, Vec3};

/// A compact 2D affine transform stored in a GPU-compatible 4x4 matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transformation(pub(super) Mat4);

impl Transformation {
    /// The identity transform.
    pub const IDENTITY: Self = Self(Mat4::IDENTITY);

    /// Builds the top-left-origin projection used by render targets.
    pub fn orthographic(width: u32, height: u32) -> Self {
        Self(glam::camera::rh::proj::opengl::orthographic(
            0.0,
            width as f32,
            height as f32,
            0.0,
            -1.0,
            1.0,
        ))
    }

    /// Builds a translation.
    #[inline]
    pub fn translate(x: f32, y: f32) -> Self {
        Self(Mat4::from_translation(Vec3::new(x, y, 0.0)))
    }

    /// Builds a uniform scale.
    #[inline]
    pub fn scale(factor: f32) -> Self {
        Self(Mat4::from_scale(Vec3::new(factor, factor, 1.0)))
    }

    /// Returns the matrix inverse.
    #[inline]
    pub fn inverse(self) -> Self {
        Self(self.0.inverse())
    }

    /// Returns the magnitude of the transformed horizontal unit axis.
    #[inline]
    pub fn scale_factor(&self) -> f32 {
        self.scale_factors().x
    }

    /// Returns both transformed unit-axis magnitudes.
    #[inline]
    pub fn scale_factors(&self) -> Vector {
        Vector::new(
            self.0.x_axis.x.hypot(self.0.x_axis.y),
            self.0.y_axis.x.hypot(self.0.y_axis.y),
        )
    }

    /// Returns the two-dimensional translation component.
    #[inline]
    pub fn translation(&self) -> Vector {
        Vector::new(self.0.w_axis.x, self.0.w_axis.y)
    }
}

impl Default for Transformation {
    fn default() -> Self {
        Self::IDENTITY
    }
}
