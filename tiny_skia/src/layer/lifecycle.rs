use super::{DrawBatch, Layer, Quad};
use crate::Primitive;
use crate::core::Rectangle;
use crate::graphics;
use crate::graphics::Image;
use crate::graphics::text::Text;

const STREAM_RETAIN_LIMIT: usize = 256 * 1024;

impl Default for Layer {
    fn default() -> Self {
        Self {
            bounds: Rectangle::INFINITE,
            quads: Vec::with_capacity(16),
            primitives: Vec::with_capacity(4),
            images: Vec::with_capacity(4),
            text: Vec::with_capacity(8),
        }
    }
}

impl graphics::Layer for Layer {
    fn with_bounds(bounds: Rectangle) -> Self {
        Self {
            bounds,
            ..Self::default()
        }
    }

    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn flush(&mut self) {}

    fn resize(&mut self, bounds: Rectangle) {
        self.bounds = bounds;
    }

    fn reset(&mut self) {
        self.bounds = Rectangle::INFINITE;
        clear_bounded(&mut self.quads);
        clear_bounded(&mut self.primitives);
        clear_bounded(&mut self.images);
        clear_bounded(&mut self.text);
    }

    fn start(&self) -> usize {
        if !self.quads.is_empty() {
            1
        } else if !self.primitives.is_empty() {
            2
        } else if !self.images.is_empty() {
            3
        } else if !self.text.is_empty() {
            4
        } else {
            usize::MAX
        }
    }

    fn end(&self) -> usize {
        if !self.text.is_empty() {
            4
        } else if !self.images.is_empty() {
            3
        } else if !self.primitives.is_empty() {
            2
        } else if !self.quads.is_empty() {
            1
        } else {
            0
        }
    }

    fn merge(&mut self, layer: &mut Self) {
        self.quads.append(&mut layer.quads);
        self.primitives.append(&mut layer.primitives);
        self.images.append(&mut layer.images);
        self.text.append(&mut layer.text);
    }

    fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.quads.capacity() * std::mem::size_of::<Quad>()
            + batch_bytes::<Primitive>(&self.primitives)
            + batch_bytes::<Image>(&self.images)
            + batch_bytes::<Text>(&self.text)
    }
}

fn batch_bytes<T>(batches: &[DrawBatch<T>]) -> usize {
    std::mem::size_of_val(batches) + batches.iter().map(DrawBatch::retained_bytes).sum::<usize>()
}

fn clear_bounded<T>(values: &mut Vec<T>) {
    values.clear();
    if values.capacity().saturating_mul(std::mem::size_of::<T>()) > STREAM_RETAIN_LIMIT {
        *values = Vec::new();
    }
}
