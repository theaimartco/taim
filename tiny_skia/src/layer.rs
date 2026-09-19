mod damage;
mod lifecycle;
mod recording;

mod item;
pub(crate) use item::{DrawBatch, Quad};

use crate::Primitive;
use crate::core::Rectangle;
use crate::graphics;
use crate::graphics::Image;
use crate::graphics::text::Text;

pub type Stack = graphics::layer::Stack<Layer>;

/// A software display layer split into stable paint-order streams.
#[derive(Debug, Clone)]
pub struct Layer {
    pub(crate) bounds: Rectangle,
    pub(crate) quads: Vec<Quad>,
    pub(crate) primitives: Vec<DrawBatch<Primitive>>,
    pub(crate) images: Vec<DrawBatch<Image>>,
    pub(crate) text: Vec<DrawBatch<Text>>,
}

impl Layer {
    pub fn damage(previous: &Self, current: &Self) -> Vec<Rectangle> {
        damage::between(previous, current)
    }
}
