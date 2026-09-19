//! Make elements float above the normal widget layer.
mod implementation;
mod style;

pub use style::{Catalog, Style, StyleFn};

use crate::core::{Element, Rectangle, Vector};

/// A widget that can transform its content above other widgets.
pub struct Float<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Theme: Catalog,
{
    pub(super) content: Element<'a, Message, Theme, Renderer>,
    pub(super) scale: f32,
    pub(super) translate: Option<Box<dyn Fn(Rectangle, Rectangle) -> Vector + 'a>>,
    pub(super) class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> Float<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
{
    /// Creates a new [`Float`] widget with the given content.
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            content: content.into(),
            scale: 1.0,
            translate: None,
            class: Theme::default(),
        }
    }

    /// Sets the scale applied to the content.
    ///
    /// Non-positive or non-finite values are normalized to `1.0` so the
    /// overlay transformation always remains invertible.
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        self
    }

    /// Sets the translation logic applied to the content.
    pub fn translate(mut self, translate: impl Fn(Rectangle, Rectangle) -> Vector + 'a) -> Self {
        self.translate = Some(Box::new(translate));
        self
    }

    /// Sets the style of the [`Float`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class of the [`Float`].
    #[cfg(feature = "advanced")]
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    pub(super) fn is_floating(&self, bounds: Rectangle, viewport: Rectangle) -> bool {
        self.scale != 1.0
            || self
                .translate
                .as_ref()
                .is_some_and(|translate| translate(bounds, viewport) != Vector::ZERO)
    }
}
