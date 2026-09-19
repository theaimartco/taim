use crate::core::{Size, Transformation};

/// A viewing region for displaying computer graphics.
#[derive(Debug, Clone)]
pub struct Viewport {
    physical_size: Size<u32>,
    logical_size: Size<f32>,
    scale_factor: f32,
    projection: Transformation,
}

impl Viewport {
    /// Creates a new [`Viewport`] with the given physical dimensions and scale
    /// factor.
    pub fn with_physical_size(size: Size<u32>, scale_factor: f32) -> Viewport {
        let scale_factor = valid_scale(scale_factor);
        let logical_per_pixel = scale_factor.recip();

        Viewport {
            physical_size: size,
            logical_size: Size::new(
                size.width as f32 * logical_per_pixel,
                size.height as f32 * logical_per_pixel,
            ),
            scale_factor,
            projection: Transformation::orthographic(size.width, size.height),
        }
    }

    /// Updates the viewing region in place, returning whether anything changed.
    ///
    /// A scale-only update preserves the projection matrix because it is
    /// defined in physical pixels. This avoids rebuilding it when a user or
    /// platform changes only the logical scale.
    pub fn update(&mut self, size: Size<u32>, scale_factor: f32) -> bool {
        let scale_factor = valid_scale(scale_factor);
        if self.physical_size == size && self.scale_factor == scale_factor {
            return false;
        }

        if self.physical_size != size {
            self.physical_size = size;
            self.projection = Transformation::orthographic(size.width, size.height);
        }
        self.scale_factor = scale_factor;
        let logical_per_pixel = scale_factor.recip();
        self.logical_size = Size::new(
            size.width as f32 * logical_per_pixel,
            size.height as f32 * logical_per_pixel,
        );
        true
    }

    /// Returns the physical size of the [`Viewport`].
    pub fn physical_size(&self) -> Size<u32> {
        self.physical_size
    }

    /// Returns the physical width of the [`Viewport`].
    pub fn physical_width(&self) -> u32 {
        self.physical_size.width
    }

    /// Returns the physical height of the [`Viewport`].
    pub fn physical_height(&self) -> u32 {
        self.physical_size.height
    }

    /// Returns the logical size of the [`Viewport`].
    pub fn logical_size(&self) -> Size<f32> {
        self.logical_size
    }

    /// Returns the scale factor of the [`Viewport`].
    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    /// Returns the projection transformation of the [`Viewport`].
    pub fn projection(&self) -> Transformation {
        self.projection
    }
}

#[inline]
fn valid_scale(scale_factor: f32) -> f32 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_only_update_keeps_physical_projection() {
        let mut viewport = Viewport::with_physical_size(Size::new(800, 600), 1.0);
        let projection = viewport.projection();

        assert!(viewport.update(Size::new(800, 600), 2.0));
        assert_eq!(viewport.logical_size(), Size::new(400.0, 300.0));
        assert_eq!(viewport.projection(), projection);
        assert!(!viewport.update(Size::new(800, 600), 2.0));
    }
}
