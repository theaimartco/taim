use crate::length;
use crate::{Length, Size};

use super::Limits;

impl Limits {
    /// Applies a horizontal length strategy.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        match width.into() {
            Length::Shrink => self.compression.width = true,
            Length::Fit | Length::Fluid(_) => self.compression.width = false,
            Length::Fixed(amount) => {
                let width = amount.min(self.max.width).max(self.min.width);
                self.min.width = width;
                self.max.width = width;
                self.compression.width = false;
            }
            Length::Bounded { bounds, sizing } => {
                apply_bounds(&mut self.min.width, &mut self.max.width, bounds);
                apply_sizing(&mut self.compression.width, sizing);
            }
            Length::Fill | Length::FillPortion(_) => {}
        }
        self
    }

    /// Applies a vertical length strategy.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        match height.into() {
            Length::Shrink => self.compression.height = true,
            Length::Fit | Length::Fluid(_) => self.compression.height = false,
            Length::Fixed(amount) => {
                let height = amount.min(self.max.height).max(self.min.height);
                self.min.height = height;
                self.max.height = height;
                self.compression.height = false;
            }
            Length::Bounded { bounds, sizing } => {
                apply_bounds(&mut self.min.height, &mut self.max.height, bounds);
                apply_sizing(&mut self.compression.height, sizing);
            }
            Length::Fill | Length::FillPortion(_) => {}
        }
        self
    }

    /// Shrinks both axes without allowing negative extents.
    pub fn shrink(&self, size: impl Into<Size>) -> Self {
        let size = size.into();
        Self {
            min: Size::new(
                (self.min.width - size.width).max(0.0),
                (self.min.height - size.height).max(0.0),
            ),
            max: Size::new(
                (self.max.width - size.width).max(0.0),
                (self.max.height - size.height).max(0.0),
            ),
            compression: self.compression,
        }
    }

    /// Removes minimum constraints while preserving maximums and compression.
    #[inline]
    pub fn loose(&self) -> Self {
        Self {
            min: Size::ZERO,
            max: self.max,
            compression: self.compression,
        }
    }

    /// Resolves both axes against intrinsic content size.
    #[inline]
    pub fn resolve(
        &self,
        width: impl Into<Length>,
        height: impl Into<Length>,
        intrinsic: Size,
    ) -> Size {
        Size::new(
            self.resolve_width(width, intrinsic.width),
            self.resolve_height(height, intrinsic.height),
        )
    }

    /// Resolves horizontal extent.
    #[inline]
    pub fn resolve_width(&self, width: impl Into<Length>, intrinsic: f32) -> f32 {
        resolve_axis(
            width.into(),
            intrinsic,
            self.min.width,
            self.max.width,
            self.compression.width,
        )
    }

    /// Resolves vertical extent.
    #[inline]
    pub fn resolve_height(&self, height: impl Into<Length>, intrinsic: f32) -> f32 {
        resolve_axis(
            height.into(),
            intrinsic,
            self.min.height,
            self.max.height,
            self.compression.height,
        )
    }
}

#[inline]
fn apply_bounds(minimum: &mut f32, maximum: &mut f32, bounds: length::Bounds) {
    match bounds {
        length::Bounds::Min(min) => *minimum = min.min(*maximum).max(*minimum),
        length::Bounds::Max(max) => *maximum = max.min(*maximum).max(*minimum),
        length::Bounds::Both { min, max } => {
            *minimum = min.min(*maximum).max(*minimum);
            *maximum = max.min(*maximum).max(*minimum);
        }
    }
}

#[inline]
fn apply_sizing(compression: &mut bool, sizing: length::Sizing) {
    match sizing {
        length::Sizing::Shrink => *compression = true,
        length::Sizing::Fit => *compression = false,
        length::Sizing::Fill(_) => {}
    }
}

#[inline]
fn resolve_axis(length: Length, intrinsic: f32, min: f32, max: f32, compressed: bool) -> f32 {
    match length {
        Length::Fill
        | Length::FillPortion(_)
        | Length::Bounded {
            sizing: length::Sizing::Fill(_),
            ..
        } if !compressed => max,
        Length::Fixed(amount) => amount.min(max).max(min),
        _ => intrinsic.min(max).max(min),
    }
}
