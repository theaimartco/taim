//! Describe intrinsic, fixed, bounded, and fluid space requirements.
mod bounds;
mod combine;
mod configure;
mod fingerprint;

pub use bounds::{Bounds, Constraint, Sizing};

use crate::Pixels;

/// The strategy used to resolve space in one dimension.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    /// Resolve to intrinsic content size.
    Fit,
    /// Consume all remaining space.
    Fill,
    /// Consume a weighted portion of remaining space.
    FillPortion(u16),
    /// Compress to the least possible size.
    Shrink,
    /// Use a fixed logical pixel extent.
    Fixed(f32),
    /// Resolve inside explicit bounds.
    Bounded {
        /// Pixel boundaries.
        bounds: Bounds,
        /// Growth strategy inside the boundaries.
        sizing: Sizing,
    },
    /// Consume remaining space subject to an open-ended constraint.
    Fluid(Constraint),
}

impl From<Pixels> for Length {
    fn from(amount: Pixels) -> Self {
        Self::Fixed(f32::from(amount))
    }
}

impl From<f32> for Length {
    fn from(amount: f32) -> Self {
        Self::Fixed(amount)
    }
}

impl From<u32> for Length {
    fn from(units: u32) -> Self {
        Self::Fixed(units as f32)
    }
}
