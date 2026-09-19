//! Position one widget at explicit coordinates inside a parent region.
mod implementation;

use crate::core;
use crate::core::{Element, Length, Pixels, Point};

/// A transparent positioning wrapper.
pub struct Pin<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: core::Renderer,
{
    pub(super) content: Element<'a, Message, Theme, Renderer>,
    pub(super) width: Length,
    pub(super) height: Length,
    pub(super) position: Point,
}

impl<'a, Message, Theme, Renderer> Pin<'a, Message, Theme, Renderer>
where
    Renderer: core::Renderer,
{
    /// Creates a full-size pin containing one widget.
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            content: content.into(),
            width: Length::Fill,
            height: Length::Fill,
            position: Point::ORIGIN,
        }
    }

    /// Sets parent width behavior.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets parent height behavior.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets finite child coordinates; invalid axes fall back to zero.
    pub fn position(mut self, position: impl Into<Point>) -> Self {
        let position = position.into();
        self.position = Point::new(finite(position.x), finite(position.y));
        self
    }

    /// Sets the child X coordinate.
    pub fn x(mut self, x: impl Into<Pixels>) -> Self {
        self.position.x = finite(x.into().0);
        self
    }

    /// Sets the child Y coordinate.
    pub fn y(mut self, y: impl Into<Pixels>) -> Self {
        self.position.y = finite(y.into().0);
        self
    }
}

#[inline]
fn finite(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}
