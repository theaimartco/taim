use crate::graphics::color;
use crate::quad::Quad;

use bytemuck::{Pod, Zeroable};

mod backend;

pub use backend::{Layer, Pipeline};

/// A quad filled with a solid color.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct Solid {
    /// The background color data of the quad.
    pub color: color::Packed,

    /// The [`Quad`] data of the [`Solid`].
    pub quad: Quad,
}
