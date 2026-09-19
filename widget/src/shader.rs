//! A custom shader widget for wgpu applications.
mod program;
mod widget;

pub use program::Program;

use crate::core::Length;
use crate::renderer::wgpu::primitive;

use std::marker::PhantomData;

pub use crate::Action;
pub use crate::graphics::Viewport;
pub use primitive::{Pipeline, Primitive, Storage};

/// A widget which can render custom shaders with Taim's `wgpu` backend.
///
/// Must be initialized with a [`Program`], which describes the internal widget state & how
/// its [`Program::Primitive`]s are drawn.
pub struct Shader<Message, P: Program<Message>> {
    pub(super) width: Length,
    pub(super) height: Length,
    pub(super) program: P,
    pub(super) message: PhantomData<fn(Message)>,
}

impl<Message, P: Program<Message>> Shader<Message, P> {
    /// Create a new custom [`Shader`].
    pub fn new(program: P) -> Self {
        Self {
            width: Length::Fixed(100.0),
            height: Length::Fixed(100.0),
            program,
            message: PhantomData,
        }
    }

    /// Set the `width` of the custom [`Shader`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Set the `height` of the custom [`Shader`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }
}
