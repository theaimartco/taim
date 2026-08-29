//! Manage colors for shaders.
use crate::core::Color;

use bytemuck::{Pod, Zeroable};

/// A color packed as 4 floats representing RGBA channels.
#[derive(Debug, Clone, Copy, PartialEq, Zeroable, Pod)]
#[repr(C)]
pub struct Packed([f32; 4]);

impl Packed {
    /// Returns the internal components of the [`Packed`] color.
    pub fn components(self) -> [f32; 4] {
        self.0
    }

    /// Multiplies the packed color's alpha channel.
    pub fn scale_alpha(mut self, factor: f32) -> Self {
        self.0[3] *= factor.clamp(0.0, 1.0);
        self
    }

    /// Multiplies the light the packed color emits, leaving alpha alone.
    ///
    /// Not clamped: a value above `1.0` is the whole point, and whether it
    /// survives is a property of the surface it is written to rather than of
    /// this operation.
    ///
    /// Only meaningful on a linear pack. With the `web-colors` feature the
    /// components are sRGB-encoded and scaling them is not a luminance
    /// change, so [`GAMMA_CORRECTION`] gates whether a caller should use it.
    pub fn scale_light(mut self, factor: f32) -> Self {
        self.0[0] *= factor;
        self.0[1] *= factor;
        self.0[2] *= factor;
        self
    }
}

/// A flag that indicates whether the renderer should perform gamma correction.
pub const GAMMA_CORRECTION: bool = internal::GAMMA_CORRECTION;

/// Packs a [`Color`].
pub fn pack(color: impl Into<Color>) -> Packed {
    Packed(internal::pack(color.into()))
}

#[cfg(not(feature = "web-colors"))]
mod internal {
    use crate::core::Color;

    pub const GAMMA_CORRECTION: bool = true;

    pub fn pack(color: Color) -> [f32; 4] {
        color.into_linear()
    }
}

#[cfg(feature = "web-colors")]
mod internal {
    use crate::core::Color;

    pub const GAMMA_CORRECTION: bool = false;

    pub fn pack(color: Color) -> [f32; 4] {
        [color.r, color.g, color.b, color.a]
    }
}
