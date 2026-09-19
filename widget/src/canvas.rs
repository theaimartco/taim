//! Interactive retained 2D drawing surfaces.
mod implementation;
mod program;

pub use program::{DrawContext, Program, UpdateContext};

pub use crate::Action;
pub use crate::core::event::Event;
pub use crate::graphics::cache::Group;
pub use crate::graphics::geometry::{
    Fill, Gradient, Image, LineCap, LineDash, LineJoin, Path, Stroke, Style, Text, fill, gradient,
    path, stroke,
};

use crate::core::Length;
use crate::core::mouse;
use crate::graphics::geometry;

use std::marker::PhantomData;

/// Retained geometry cache.
pub type Cache<Renderer = crate::Renderer> = geometry::Cache<Renderer>;

/// Renderer-specific geometry.
pub type Geometry<Renderer = crate::Renderer> = <Renderer as geometry::Renderer>::Geometry;

/// Renderer-specific geometry frame.
pub type Frame<Renderer = crate::Renderer> = geometry::Frame<Renderer>;

/// An interactive 2D drawing widget driven by a [`Program`].
#[derive(Debug)]
pub struct Canvas<P, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: geometry::Renderer,
    P: Program<Message, Theme, Renderer>,
{
    pub(super) width: Length,
    pub(super) height: Length,
    pub(super) program: P,
    pub(super) message: PhantomData<Message>,
    pub(super) theme: PhantomData<Theme>,
    pub(super) renderer: PhantomData<Renderer>,
}

impl<P, Message, Theme, Renderer> Canvas<P, Message, Theme, Renderer>
where
    P: Program<Message, Theme, Renderer>,
    Renderer: geometry::Renderer,
{
    const DEFAULT_SIZE: f32 = 100.0;

    /// Creates a canvas driven by `program`.
    pub fn new(program: P) -> Self {
        Self {
            width: Length::Fixed(Self::DEFAULT_SIZE),
            height: Length::Fixed(Self::DEFAULT_SIZE),
            program,
            message: PhantomData,
            theme: PhantomData,
            renderer: PhantomData,
        }
    }

    /// Sets canvas width.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets canvas height.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }
}
