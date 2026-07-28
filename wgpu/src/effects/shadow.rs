//! Bounded shadow effect model.
use crate::core::{Rectangle, Vector};
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    pub offset: Vector,
    pub blur_radius: f32,
    pub spread: f32,
}
impl Shadow {
    pub fn bounds(self, source: Rectangle) -> Rectangle {
        let pad = (self.blur_radius.max(0.0) * 2.0 + self.spread.max(0.0)).ceil();
        Rectangle {
            x: source.x + self.offset.x - pad,
            y: source.y + self.offset.y - pad,
            width: source.width + pad * 2.0,
            height: source.height + pad * 2.0,
        }
    }
}
