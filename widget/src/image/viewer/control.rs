use crate::core::ContentFit;
use crate::core::image::{self, FilterMethod};
use crate::core::{Length, Pixels};

/// An image view with bounded zoom and pan interaction.
pub struct Viewer<Handle> {
    pub(super) padding: f32,
    pub(super) width: Length,
    pub(super) height: Length,
    pub(super) min_scale: f32,
    pub(super) max_scale: f32,
    pub(super) scale_step: f32,
    pub(super) handle: Handle,
    pub(super) filter_method: FilterMethod,
    pub(super) content_fit: ContentFit,
}

impl<Handle> Viewer<Handle> {
    /// Creates an image viewer.
    pub fn new<T: Into<Handle>>(handle: T) -> Self {
        Self {
            handle: handle.into(),
            padding: 0.0,
            width: Length::Shrink,
            height: Length::Shrink,
            min_scale: 0.25,
            max_scale: 10.0,
            scale_step: 0.10,
            filter_method: FilterMethod::default(),
            content_fit: ContentFit::default(),
        }
    }

    /// Sets image filtering.
    pub fn filter_method(mut self, filter_method: image::FilterMethod) -> Self {
        self.filter_method = filter_method;
        self
    }

    /// Sets how the image fits its available bounds.
    pub fn content_fit(mut self, content_fit: ContentFit) -> Self {
        self.content_fit = content_fit;
        self
    }

    /// Sets padding around the interactive image area.
    pub fn padding(mut self, padding: impl Into<Pixels>) -> Self {
        let padding = padding.into().0;

        if padding.is_finite() {
            self.padding = padding.max(0.0);
        }

        self
    }

    /// Sets the viewer width.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the viewer height.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the maximum zoom scale.
    pub fn max_scale(mut self, max_scale: f32) -> Self {
        if max_scale.is_finite() && max_scale > 0.0 {
            self.max_scale = max_scale.max(self.min_scale);
        }

        self
    }

    /// Sets the minimum zoom scale.
    pub fn min_scale(mut self, min_scale: f32) -> Self {
        if min_scale.is_finite() && min_scale > 0.0 {
            self.min_scale = min_scale.min(self.max_scale);
        }

        self
    }

    /// Sets the proportional zoom step.
    pub fn scale_step(mut self, scale_step: f32) -> Self {
        if scale_step.is_finite() && scale_step > 0.0 {
            self.scale_step = scale_step;
        }

        self
    }
}
