use crate::{Radians, border};

use super::Handle;

/// A raster image draw request.
#[derive(Debug, Clone, PartialEq)]
pub struct Image<H = Handle> {
    /// The handle of the image.
    pub handle: H,
    /// The sampling filter.
    pub filter_method: FilterMethod,
    /// Rotation around the image center.
    pub rotation: Radians,
    /// Border radius applied to clip bounds.
    pub border_radius: border::Radius,
    /// Opacity in the `0.0..=1.0` range.
    pub opacity: f32,
}

impl Image<Handle> {
    /// Creates an image draw request.
    pub fn new(handle: impl Into<Handle>) -> Self {
        Self {
            handle: handle.into(),
            filter_method: FilterMethod::default(),
            rotation: Radians(0.0),
            border_radius: border::Radius::default(),
            opacity: 1.0,
        }
    }

    /// Sets the sampling filter.
    pub fn filter_method(mut self, filter_method: FilterMethod) -> Self {
        self.filter_method = filter_method;
        self
    }

    /// Sets rotation around the image center.
    pub fn rotation(mut self, rotation: impl Into<Radians>) -> Self {
        self.rotation = rotation.into();
        self
    }

    /// Sets normalized opacity.
    pub fn opacity(mut self, opacity: impl Into<f32>) -> Self {
        let opacity = opacity.into();
        self.opacity = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self
    }
}

impl From<&Handle> for Image {
    fn from(handle: &Handle) -> Self {
        Image::new(handle.clone())
    }
}

/// Image sampling strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FilterMethod {
    /// Bilinear interpolation.
    #[default]
    Linear,
    /// Nearest-neighbor sampling.
    Nearest,
}
