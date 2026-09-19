//! Lightweight horizontal and vertical separators.
mod implementation;
mod style;

pub use style::{Catalog, Style, StyleFn, default, weak};

use crate::core::{Length, Pixels};

/// Creates a horizontal separator.
pub fn horizontal<'a, Theme>(height: impl Into<Pixels>) -> Rule<'a, Theme>
where
    Theme: Catalog,
{
    Rule::new(height.into().0, false)
}

/// Creates a vertical separator.
pub fn vertical<'a, Theme>(width: impl Into<Pixels>) -> Rule<'a, Theme>
where
    Theme: Catalog,
{
    Rule::new(width.into().0, true)
}

/// A separator line with configurable fill behavior.
pub struct Rule<'a, Theme = crate::Theme>
where
    Theme: Catalog,
{
    pub(super) thickness: Length,
    pub(super) is_vertical: bool,
    pub(super) class: Theme::Class<'a>,
}

impl<'a, Theme> Rule<'a, Theme>
where
    Theme: Catalog,
{
    fn new(thickness: f32, is_vertical: bool) -> Self {
        let thickness = if thickness.is_finite() {
            thickness.max(0.0)
        } else {
            0.0
        };

        Self {
            thickness: Length::Fixed(thickness),
            is_vertical,
            class: Theme::default(),
        }
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
}

/// Controls how much of the available axis a rule fills.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FillMode {
    /// Fills the complete axis.
    Full,
    /// Fills a centered percentage in the `0..=100` range.
    Percent(f32),
    /// Applies equal padding to both ends.
    Padded(u16),
    /// Applies independent start and end padding.
    AsymmetricPadding(u16, u16),
}

impl FillMode {
    /// Resolves this mode to `(offset, extent)` for the available space.
    #[inline]
    pub fn fill(&self, space: f32) -> (f32, f32) {
        let space = if space.is_finite() {
            space.max(0.0)
        } else {
            0.0
        };

        match *self {
            Self::Full => (0.0, space),
            Self::Percent(percent) => {
                let percent = if percent.is_finite() {
                    percent.clamp(0.0, 100.0)
                } else {
                    0.0
                };

                if percent == 100.0 {
                    (0.0, space)
                } else {
                    let extent = (space * percent / 100.0).round();
                    (((space - extent) / 2.0).round(), extent)
                }
            }
            Self::Padded(padding) => {
                let padding = f32::from(padding);
                (padding, (space - padding * 2.0).max(0.0))
            }
            Self::AsymmetricPadding(start, end) => {
                let start = f32::from(start);
                (start, (space - start - f32::from(end)).max(0.0))
            }
        }
    }
}
