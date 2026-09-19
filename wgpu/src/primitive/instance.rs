use super::{Primitive, Storage};

use crate::core::Rectangle;
use crate::graphics::Viewport;
use crate::graphics::futures::{MaybeSend, MaybeSync};

use std::fmt::Debug;

pub(crate) trait Stored: Debug + MaybeSend + MaybeSync + 'static {
    fn prepare(
        &self,
        storage: &mut Storage,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        bounds: &Rectangle,
        viewport: &Viewport,
    );

    fn draw(&self, storage: &Storage, render_pass: &mut wgpu::RenderPass<'_>) -> bool;

    fn render(
        &self,
        storage: &Storage,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    );
}

#[derive(Debug)]
struct Erased<P: Primitive> {
    primitive: P,
}

impl<P: Primitive> Stored for Erased<P> {
    fn prepare(
        &self,
        storage: &mut Storage,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        let pipeline = storage.pipeline_mut::<P>(device, queue, format);
        self.primitive
            .prepare(pipeline, device, queue, bounds, viewport);
    }

    fn draw(&self, storage: &Storage, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        self.primitive.draw(storage.pipeline::<P>(), render_pass)
    }

    fn render(
        &self,
        storage: &Storage,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        self.primitive
            .render(storage.pipeline::<P>(), encoder, target, clip_bounds);
    }
}

/// One type-erased custom primitive and its logical bounds.
#[derive(Debug)]
pub struct Instance {
    pub(crate) bounds: Rectangle,
    pub(crate) primitive: Box<dyn Stored>,
}

impl Instance {
    /// Erases a custom primitive for heterogeneous batching.
    pub fn new(bounds: Rectangle, primitive: impl Primitive) -> Self {
        Self {
            bounds,
            primitive: Box::new(Erased { primitive }),
        }
    }
}
