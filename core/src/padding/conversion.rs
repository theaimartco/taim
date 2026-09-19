//! Allocation-free padding conversions.

use super::{Padding, valid};
use crate::{Pixels, Size};

impl From<u16> for Padding {
    #[inline]
    fn from(value: u16) -> Self {
        Self::new(f32::from(value))
    }
}

impl From<[u16; 2]> for Padding {
    #[inline]
    fn from([vertical, horizontal]: [u16; 2]) -> Self {
        Self {
            top: f32::from(vertical),
            right: f32::from(horizontal),
            bottom: f32::from(vertical),
            left: f32::from(horizontal),
        }
    }
}

impl From<f32> for Padding {
    #[inline]
    fn from(value: f32) -> Self {
        Self::new(value)
    }
}

impl From<[f32; 2]> for Padding {
    #[inline]
    fn from([vertical, horizontal]: [f32; 2]) -> Self {
        let vertical = valid(vertical);
        let horizontal = valid(horizontal);
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

impl From<Padding> for Size {
    #[inline]
    fn from(padding: Padding) -> Self {
        Self::new(padding.x(), padding.y())
    }
}

impl From<Pixels> for Padding {
    #[inline]
    fn from(pixels: Pixels) -> Self {
        Self::new(pixels.0)
    }
}
