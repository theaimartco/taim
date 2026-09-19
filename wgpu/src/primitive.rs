//! Draw custom primitives.
use crate::core::{self, Rectangle};
use crate::graphics::Viewport;
use crate::graphics::futures::{MaybeSend, MaybeSync};

use std::any::Any;
use std::fmt::Debug;

mod instance;
mod storage;

pub use instance::Instance;
pub use storage::Storage;

/// A batch of primitives.
pub type Batch = Vec<Instance>;

/// A set of methods which allows a [`Primitive`] to be rendered.
pub trait Primitive: Debug + MaybeSend + MaybeSync + 'static {
    /// The shared renderer of this [`Primitive`].
    ///
    /// Normally, this will contain a bunch of [`wgpu`] state; like
    /// a rendering pipeline, buffers, and textures.
    ///
    /// All instances of this [`Primitive`] type will share the same
    /// [`Renderer`].
    type Pipeline: Pipeline + MaybeSend + MaybeSync;

    /// Processes the [`Primitive`], allowing for GPU buffer allocation.
    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &Viewport,
    );

    /// Draws the [`Primitive`] in the given [`wgpu::RenderPass`].
    ///
    /// When possible, this should be implemented over [`render`](Self::render)
    /// since reusing the existing render pass should be considerably more
    /// efficient than issuing a new one.
    ///
    /// The viewport and scissor rect of the render pass provided is set
    /// to the bounds and clip bounds of the [`Primitive`], respectively.
    ///
    /// If you have complex composition needs, then you can leverage
    /// [`render`](Self::render) by returning `false` here.
    ///
    /// By default, it does nothing and returns `false`.
    fn draw(&self, _pipeline: &Self::Pipeline, _render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        false
    }

    /// Renders the [`Primitive`], using the given [`wgpu::CommandEncoder`].
    ///
    /// This will only be called if [`draw`](Self::draw) returns `false`.
    ///
    /// By default, it does nothing.
    fn render(
        &self,
        _pipeline: &Self::Pipeline,
        _encoder: &mut wgpu::CommandEncoder,
        _target: &wgpu::TextureView,
        _clip_bounds: &Rectangle<u32>,
    ) {
    }
}

/// The pipeline of a graphics [`Primitive`].
pub trait Pipeline: Any + MaybeSend + MaybeSync {
    /// Creates the [`Pipeline`] of a [`Primitive`].
    ///
    /// This will only be called once, when the first [`Primitive`] with this kind
    /// of [`Pipeline`] is encountered.
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self
    where
        Self: Sized;

    /// Trims any cached data in the [`Pipeline`].
    ///
    /// This will normally be called at the end of a frame.
    fn trim(&mut self) {}
}

/// A renderer than can draw custom primitives.
pub trait Renderer: core::Renderer {
    /// Draws a custom primitive.
    fn draw_primitive(&mut self, bounds: Rectangle, primitive: impl Primitive);
}
