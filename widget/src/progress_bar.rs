//! Horizontal and vertical progress indicators.
mod implementation;
mod style;

pub use style::{Catalog, Style, StyleFn, danger, primary, secondary, success, warning};

use crate::core::Length;

use std::ops::RangeInclusive;

/// A lightweight progress indicator with a precomputed normalized value.
pub struct ProgressBar<'a, Theme = crate::Theme>
where
    Theme: Catalog,
{
    pub(super) progress: f32,
    pub(super) length: Length,
    pub(super) girth: Length,
    pub(super) is_vertical: bool,
    pub(super) class: Theme::Class<'a>,
}

impl<'a, Theme> ProgressBar<'a, Theme>
where
    Theme: Catalog,
{
    /// Default cross-axis extent.
    pub const DEFAULT_GIRTH: f32 = 30.0;

    /// Creates a progress bar and normalizes its value once.
    pub fn new(range: RangeInclusive<f32>, value: f32) -> Self {
        let (start, end) = range.into_inner();
        let progress = if start.is_finite() && end.is_finite() && value.is_finite() && start < end {
            ((value - start) / (end - start)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        Self {
            progress,
            length: Length::Fill,
            girth: Length::Fixed(Self::DEFAULT_GIRTH),
            is_vertical: false,
            class: Theme::default(),
        }
    }

    /// Sets main-axis length.
    pub fn length(mut self, length: impl Into<Length>) -> Self {
        self.length = length.into();
        self
    }

    /// Sets cross-axis girth.
    pub fn girth(mut self, girth: impl Into<Length>) -> Self {
        self.girth = girth.into();
        self
    }

    /// Selects vertical orientation.
    pub fn vertical(mut self, vertical: bool) -> Self {
        self.is_vertical = vertical;
        self
    }

    /// Sets a dynamic style.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
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

    pub(super) fn width(&self) -> Length {
        if self.is_vertical {
            self.girth
        } else {
            self.length
        }
    }

    pub(super) fn height(&self) -> Length {
        if self.is_vertical {
            self.length
        } else {
            self.girth
        }
    }
}
