//! Retained vector-image widgets.
mod implementation;
mod style;

pub use style::{Catalog, Style, StyleFn};

use crate::core::svg;
use crate::core::{ContentFit, Length, Rotation};

use std::path::PathBuf;

pub use svg::Handle;

/// A smoothly scalable vector image.
pub struct Svg<'a, Theme = crate::Theme>
where
    Theme: Catalog,
{
    pub(super) handle: Handle,
    pub(super) width: Length,
    pub(super) height: Length,
    pub(super) content_fit: ContentFit,
    pub(super) class: Theme::Class<'a>,
    pub(super) rotation: Rotation,
    pub(super) opacity: f32,
}

impl<'a, Theme> Svg<'a, Theme>
where
    Theme: Catalog,
{
    /// Creates an SVG widget from a handle.
    pub fn new(handle: impl Into<Handle>) -> Self {
        Self {
            handle: handle.into(),
            width: Length::Fill,
            height: Length::Shrink,
            content_fit: ContentFit::Contain,
            class: Theme::default(),
            rotation: Rotation::default(),
            opacity: 1.0,
        }
    }

    /// Creates an SVG widget from a file path.
    #[must_use]
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self::new(Handle::from_path(path))
    }

    /// Sets width.
    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets height.
    #[must_use]
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets content-fit behavior.
    #[must_use]
    pub fn content_fit(mut self, content_fit: ContentFit) -> Self {
        self.content_fit = content_fit;
        self
    }

    /// Sets a status-aware dynamic style.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets a theme class.
    #[cfg(feature = "advanced")]
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    /// Sets rotation around the SVG center.
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
}

/// Current pointer status of an SVG widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Pointer is outside the widget.
    Idle,
    /// Pointer is inside the widget.
    Hovered,
}
