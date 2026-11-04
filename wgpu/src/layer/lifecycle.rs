use super::Layer;
use crate::core::Rectangle;
use crate::graphics;
use crate::image;
use crate::text;
use crate::triangle;

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

    fn flush(&mut self) {
        self.flush_meshes();
        self.flush_text();
    }

    fn resize(&mut self, bounds: Rectangle) {
        self.bounds = bounds;
    }

    fn reset(&mut self) {
        self.bounds = Rectangle::INFINITE;
        self.quads.clear();

        let mut recycled_meshes = Vec::new();
        for item in self.triangles.drain(..) {
            if let triangle::Item::Group { meshes, .. } = item {
                recycled_meshes.push(meshes);
            }
        }
        for meshes in recycled_meshes {
            self.recycle_meshes(meshes);
        }

        let mut recycled_text = Vec::new();
        for item in self.text.drain(..) {
            if let text::Item::Group { text, .. } = item {
                recycled_text.push(text);
            }
        }
        for text in recycled_text {
            self.recycle_text(text);
        }

        self.primitives.clear();
        self.images.clear();
        self.effects = crate::effects::Graph::new();
        self.pending_meshes.clear();
        self.pending_text.clear();
    }

    fn start(&self) -> usize {
        if !self.quads.is_empty() {
            1
        } else if !self.triangles.is_empty() {
            2
        } else if !self.primitives.is_empty() {
            3
        } else if !self.images.is_empty() {
            4
        } else if !self.text.is_empty() {
            5
        } else if !self.effects.is_empty() {
            6
        } else {
            usize::MAX
        }
    }

    fn end(&self) -> usize {
        if !self.effects.is_empty() {
            6
        } else if !self.text.is_empty() {
            5
        } else if !self.images.is_empty() {
            4
        } else if !self.primitives.is_empty() {
            3
        } else if !self.triangles.is_empty() {
            2
        } else if !self.quads.is_empty() {
            1
        } else {
            0
        }
    }

    fn merge(&mut self, layer: &mut Self) {
        self.quads.append(&mut layer.quads);
        self.triangles.append(&mut layer.triangles);
        self.primitives.append(&mut layer.primitives);
        self.images.append(&mut layer.images);
        self.text.append(&mut layer.text);
    }

    fn estimated_bytes(&self) -> usize {
        self.retained_bytes()
    }
}

impl Default for Layer {
    fn default() -> Self {
        Self {
            bounds: Rectangle::INFINITE,
            quads: crate::quad::Batch::default(),
            triangles: Vec::new(),
            primitives: Vec::new(),
            text: Vec::new(),
            #[allow(clippy::default_constructed_unit_structs)]
            images: image::Batch::default(),
            effects: crate::effects::Graph::new(),
            pending_meshes: Vec::new(),
            pending_text: Vec::new(),
            mesh_pool: Vec::new(),
            text_pool: Vec::new(),
        }
    }
}
