//! Apply a local theme to a transparent widget subtree.
mod implementation;

use crate::core::{Background, Color, Element};

/// A transparent wrapper that replaces the theme of its content.
pub struct Themer<'a, Message, Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
{
    pub(super) content: Element<'a, Message, Theme, Renderer>,
    pub(super) theme: Option<Theme>,
    pub(super) text_color: Option<fn(&Theme) -> Color>,
    pub(super) background: Option<fn(&Theme) -> Background>,
}

impl<'a, Message, Theme, Renderer> Themer<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    /// Creates a themed subtree. `None` derives a theme from the inherited mode.
    pub fn new(
        theme: Option<Theme>,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        Self {
            content: content.into(),
            theme,
            text_color: None,
            background: None,
        }
    }

    /// Sets a theme-derived default text color.
    pub fn text_color(mut self, resolve: fn(&Theme) -> Color) -> Self {
        self.text_color = Some(resolve);
        self
    }

    /// Sets a theme-derived background.
    pub fn background(mut self, resolve: fn(&Theme) -> Background) -> Self {
        self.background = Some(resolve);
        self
    }
}
