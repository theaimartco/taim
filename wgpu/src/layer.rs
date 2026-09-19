mod draw;
mod lifecycle;
mod memory;

use crate::core::Rectangle;
use crate::graphics::{self, Mesh};
use crate::image;
use crate::primitive;
use crate::quad;
use crate::text::{self, Text};
use crate::triangle;

pub type Stack = graphics::layer::Stack<Layer>;

#[derive(Debug)]
pub struct Layer {
    pub bounds: Rectangle,
    pub quads: quad::Batch,
    pub triangles: triangle::Batch,
    pub primitives: primitive::Batch,
    pub images: image::Batch,
    pub text: text::Batch,
    pub effects: crate::effects::Graph,
    pending_meshes: Vec<Mesh>,
    pending_text: Vec<Text>,
    mesh_pool: Vec<Vec<Mesh>>,
    text_pool: Vec<Vec<Text>>,
}
