//! Spatial computing and physical dimension primitives implementing Pillar XI.
//!
//! Provides 3D projective transformations, stereoscopic viewing matrices for
//! spatial/waveguide displays, and physical millimeter units with viewing-distance
//! angular normalization.

use crate::transformation::Transformation;
use crate::Pixels;
use glam::{Mat4, Vec3};
use std::ops::{Add, Div, Mul, Sub};

/// An amount of physical space measured in millimeters (mm).
///
/// Unlike logical pixels, millimeters are invariant across panel pixel densities.
/// A button specified as 8 mm is physically 8 mm on phones, desktop monitors,
/// and spatial headsets.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Mm(pub f32);

impl Mm {
    /// Zero millimeters.
    pub const ZERO: Self = Self(0.0);

    /// Standard inter-pupillary distance (IPD) baseline: 64 mm.
    pub const DEFAULT_IPD: Self = Self(64.0);

    /// Standard touch target minimum size recommended by accessibility guidelines: 9 mm.
    pub const MIN_TOUCH_TARGET: Self = Self(9.0);
}

impl From<f32> for Mm {
    fn from(amount: f32) -> Self {
        Self(amount)
    }
}

impl From<u32> for Mm {
    fn from(amount: u32) -> Self {
        Self(amount as f32)
    }
}

impl From<Mm> for f32 {
    fn from(mm: Mm) -> Self {
        mm.0
    }
}

impl Add for Mm {
    type Output = Mm;
    fn add(self, rhs: Self) -> Self {
        Mm(self.0 + rhs.0)
    }
}

impl Sub for Mm {
    type Output = Mm;
    fn sub(self, rhs: Self) -> Self {
        Mm(self.0 - rhs.0)
    }
}

impl Mul<f32> for Mm {
    type Output = Mm;
    fn mul(self, rhs: f32) -> Self {
        Mm(self.0 * rhs)
    }
}

impl Div<f32> for Mm {
    type Output = Mm;
    fn div(self, rhs: f32) -> Self {
        Mm(self.0 / rhs)
    }
}

/// Physical display and viewing context for transforming physical dimensions to pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicalContext {
    /// Pixel density in dots per inch (DPI). Standard CSS/desktop baseline is 96.0 DPI.
    pub dpi: f32,
    /// Distance from the user's eye to the display plane, in millimeters.
    /// Handheld phone: ~350-400 mm; Desktop monitor: ~600-700 mm; TV: ~2000 mm.
    pub viewing_distance_mm: f32,
    /// Reference distance for 1.0x angular scale (default: 400.0 mm / arm's length).
    pub reference_distance_mm: f32,
}

impl Default for PhysicalContext {
    fn default() -> Self {
        Self {
            dpi: 96.0,
            viewing_distance_mm: 400.0,
            reference_distance_mm: 400.0,
        }
    }
}

impl PhysicalContext {
    /// Desktop monitor profile: 110 DPI at 650 mm viewing distance.
    pub const DESKTOP: Self = Self {
        dpi: 110.0,
        viewing_distance_mm: 650.0,
        reference_distance_mm: 400.0,
    };

    /// Mobile phone flagship profile: 460 DPI (OLED) at 350 mm viewing distance.
    pub const MOBILE: Self = Self {
        dpi: 460.0,
        viewing_distance_mm: 350.0,
        reference_distance_mm: 400.0,
    };

    /// Spatial headset waveguide profile: 1800 DPI at 1000 mm virtual plane.
    pub const SPATIAL: Self = Self {
        dpi: 1800.0,
        viewing_distance_mm: 1000.0,
        reference_distance_mm: 400.0,
    };

    /// Converts physical millimeters to display pixels based on pixel density.
    pub fn to_pixels(&self, mm: Mm) -> Pixels {
        // 1 inch = 25.4 mm
        let inches = mm.0 / 25.4;
        Pixels(inches * self.dpi)
    }

    /// Converts physical millimeters to angularly-scaled pixels preserving visual angle.
    pub fn to_angular_pixels(&self, mm: Mm) -> Pixels {
        let base_pixels = self.to_pixels(mm);
        let angular_factor = (self.viewing_distance_mm / self.reference_distance_mm).max(0.1);
        Pixels(base_pixels.0 * angular_factor)
    }
}

/// A 3D spatial transformation matrix (4x4) supporting perspective and stereo projections.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform3D(pub Mat4);

impl Transform3D {
    /// The identity 3D transformation.
    pub const IDENTITY: Self = Self(Mat4::IDENTITY);

    /// Creates a 3D perspective projection.
    pub fn perspective(fov_y_radians: f32, aspect_ratio: f32, z_near: f32, z_far: f32) -> Self {
        Self(glam::camera::rh::proj::directx::perspective(
            fov_y_radians,
            aspect_ratio,
            z_near,
            z_far,
        ))
    }

    /// Creates an orthographic 3D projection.
    pub fn orthographic(width: f32, height: f32, z_near: f32, z_far: f32) -> Self {
        Self(glam::camera::rh::proj::opengl::orthographic(
            0.0, width, height, 0.0, z_near, z_far,
        ))
    }

    /// Creates a 3D translation.
    pub fn translate(x: f32, y: f32, z: f32) -> Self {
        Self(Mat4::from_translation(Vec3::new(x, y, z)))
    }

    /// Creates a 3D scale.
    pub fn scale(x: f32, y: f32, z: f32) -> Self {
        Self(Mat4::from_scale(Vec3::new(x, y, z)))
    }

    /// Creates a rotation around the X axis in radians.
    pub fn rotate_x(angle_radians: f32) -> Self {
        Self(Mat4::from_rotation_x(angle_radians))
    }

    /// Creates a rotation around the Y axis in radians.
    pub fn rotate_y(angle_radians: f32) -> Self {
        Self(Mat4::from_rotation_y(angle_radians))
    }

    /// Creates a rotation around the Z axis in radians.
    pub fn rotate_z(angle_radians: f32) -> Self {
        Self(Mat4::from_rotation_z(angle_radians))
    }

    /// Creates a camera view matrix looking from `eye` to `target`.
    pub fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> Self {
        Self(glam::camera::rh::view::look_at_mat4(
            Vec3::from(eye),
            Vec3::from(target),
            Vec3::from(up),
        ))
    }

    /// Generates a stereoscopic per-eye projection pair (Left, Right) for spatial displays.
    ///
    /// Offsets each eye by half the inter-pupillary distance (IPD) in scene space.
    pub fn stereo_pair(
        ipd: Mm,
        fov_y_radians: f32,
        aspect_ratio: f32,
        z_near: f32,
        z_far: f32,
        context: &PhysicalContext,
    ) -> (Self, Self) {
        let half_ipd_px = context.to_pixels(ipd).0 * 0.5;
        let proj = glam::camera::rh::proj::directx::perspective(
            fov_y_radians,
            aspect_ratio,
            z_near,
            z_far,
        );

        let left_eye = Self(proj * Mat4::from_translation(Vec3::new(half_ipd_px, 0.0, 0.0)));
        let right_eye = Self(proj * Mat4::from_translation(Vec3::new(-half_ipd_px, 0.0, 0.0)));

        (left_eye, right_eye)
    }

    /// Extracts the 2D affine component as a [`Transformation`].
    pub fn to_2d(&self) -> Transformation {
        Transformation::translate(self.0.w_axis.x, self.0.w_axis.y)
    }

    /// Inverts this 3D transform.
    pub fn inverse(self) -> Self {
        Self(self.0.inverse())
    }
}

impl Default for Transform3D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mul for Transform3D {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        Self(self.0 * rhs.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_mm_converts_to_pixels_via_dpi() {
        let ctx = PhysicalContext {
            dpi: 254.0, // 254 DPI -> 10 pixels per mm exactly
            viewing_distance_mm: 400.0,
            reference_distance_mm: 400.0,
        };

        assert_eq!(ctx.to_pixels(Mm(25.4)), Pixels(254.0));
        assert_eq!(ctx.to_pixels(Mm(10.0)), Pixels(100.0));
    }

    #[test]
    fn angular_pixels_scale_proportionally_with_viewing_distance() {
        let ctx = PhysicalContext {
            viewing_distance_mm: 800.0,
            reference_distance_mm: 400.0,
            ..Default::default()
        };

        let base = ctx.to_pixels(Mm(10.0));
        let angular = ctx.to_angular_pixels(Mm(10.0));
        assert_eq!(angular.0, base.0 * 2.0);
    }

    #[test]
    fn stereo_pair_offsets_left_and_right_eyes() {
        let ctx = PhysicalContext::default();
        let (left, right) = Transform3D::stereo_pair(
            Mm(64.0),
            std::f32::consts::FRAC_PI_2,
            1.0,
            0.1,
            100.0,
            &ctx,
        );

        assert_ne!(left, right);
        assert_ne!(left, Transform3D::IDENTITY);
        assert_ne!(right, Transform3D::IDENTITY);
    }
}
