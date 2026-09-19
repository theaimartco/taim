//! Load and operate on images.
#[cfg(feature = "image")]
mod decode;

#[cfg(feature = "image")]
pub use decode::{Buffer, load};

use crate::core::Rectangle;
use crate::core::image;
use crate::core::svg;

/// A raster or vector image.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq)]
pub enum Image {
    /// A raster image.
    Raster {
        image: image::Image,
        bounds: Rectangle,
        clip_bounds: Rectangle,
    },

    /// A vector image.
    Vector {
        svg: svg::Svg,
        bounds: Rectangle,
        clip_bounds: Rectangle,
    },
}

impl Image {
    /// Returns the bounds of the [`Image`].
    pub fn bounds(&self) -> Rectangle {
        match self {
            Image::Raster { image, bounds, .. } => bounds.rotate(image.rotation),
            Image::Vector { svg, bounds, .. } => bounds.rotate(svg.rotation),
        }
    }

    /// Multiplies this image's opacity.
    pub fn scale_alpha(mut self, factor: f32) -> Self {
        let factor = if factor.is_finite() {
            factor.clamp(0.0, 1.0)
        } else {
            0.0
        };

        if factor == 1.0 {
            return self;
        }

        let scale = |opacity: f32| {
            if opacity.is_finite() {
                opacity.clamp(0.0, 1.0) * factor
            } else {
                0.0
            }
        };

        match &mut self {
            Self::Raster { image, .. } => image.opacity = scale(image.opacity),
            Self::Vector { svg, .. } => svg.opacity = scale(svg.opacity),
        }

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_alpha_factor_makes_images_transparent() {
        let bounds = Rectangle::new(crate::core::Point::ORIGIN, crate::core::Size::UNIT);
        let image = Image::Raster {
            image: image::Image::new(image::Handle::from_rgba(1, 1, vec![255; 4])),
            bounds,
            clip_bounds: bounds,
        }
        .scale_alpha(f32::NAN);

        let Image::Raster { image, .. } = image else {
            unreachable!();
        };

        assert_eq!(image.opacity, 0.0);
    }
}
