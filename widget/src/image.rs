//! Raster image widgets with retained sizing and viewport-aware drawing.
mod geometry;
mod implementation;

pub mod viewer;

pub use geometry::{draw, layout};
pub use viewer::Viewer;

use crate::core::border;
use crate::core::image;
use crate::core::{ContentFit, Length, Rectangle, Rotation};

pub use image::{FilterMethod, Handle};

/// Creates an interactive image viewer.
pub fn viewer<Handle>(handle: Handle) -> Viewer<Handle> {
    Viewer::new(handle)
}

/// A retained frame that draws a raster image while preserving aspect ratio.
pub struct Image<Handle = image::Handle> {
    pub(super) handle: Handle,
    pub(super) width: Length,
    pub(super) height: Length,
    pub(super) crop: Option<Rectangle<u32>>,
    pub(super) border_radius: border::Radius,
    pub(super) content_fit: ContentFit,
    pub(super) filter_method: FilterMethod,
    pub(super) rotation: Rotation,
    pub(super) opacity: f32,
    pub(super) scale: f32,
    pub(super) expand: bool,
}

impl<Handle> Image<Handle> {
    /// Creates an image widget.
    pub fn new(handle: impl Into<Handle>) -> Self {
        Self {
            handle: handle.into(),
            width: Length::Shrink,
            height: Length::Shrink,
            crop: None,
            border_radius: border::Radius::default(),
            content_fit: ContentFit::default(),
            filter_method: FilterMethod::default(),
            rotation: Rotation::default(),
            opacity: 1.0,
            scale: 1.0,
            expand: false,
        }
    }

    /// Sets the widget width.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the widget height.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Enables intrinsic aspect-preserving expansion.
    pub fn expand(mut self, expand: bool) -> Self {
        self.expand = expand;
        self
    }

    /// Sets how image content fits its bounds.
    pub fn content_fit(mut self, content_fit: ContentFit) -> Self {
        self.content_fit = content_fit;
        self
    }

    /// Sets the sampling filter.
    pub fn filter_method(mut self, filter_method: FilterMethod) -> Self {
        self.filter_method = filter_method;
        self
    }

    /// Sets rotation around the image center.
    pub fn rotation(mut self, rotation: impl Into<Rotation>) -> Self {
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

    /// Sets a positive finite center scale.
    pub fn scale(mut self, scale: impl Into<f32>) -> Self {
        let scale = scale.into();
        self.scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        self
    }

    /// Selects a source-pixel crop region.
    pub fn crop(mut self, region: Rectangle<u32>) -> Self {
        self.crop = Some(region);
        self
    }

    /// Sets clip corner radii.
    pub fn border_radius(mut self, border_radius: impl Into<border::Radius>) -> Self {
        self.border_radius = border_radius.into();
        self
    }
}
