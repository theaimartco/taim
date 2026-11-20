//! Liquid glass: a glass quad that a finger presses into and that melts into
//! its neighbours.
//!
//! A pipeline of its own so that ordinary quads, which are nearly all of
//! them, carry none of this per-instance data.
use crate::graphics::color;
use crate::quad::Quad;

use bytemuck::{Pod, Zeroable};

mod backend;

pub use backend::{Layer, Pipeline};

/// A glass quad with touch and merging, in physical-independent logical
/// units like [`Quad`].
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct Liquid {
    /// The tint, in linear light.
    pub color: color::Packed,

    /// The pane.
    pub quad: Quad,

    /// `(x, y, strength, radius)` of a press; strength `0` for none.
    pub touch: [f32; 4],

    /// The first neighbour's `(x, y, width, height)`; width `0` for none.
    pub neighbour_a: [f32; 4],

    /// The second neighbour's `(x, y, width, height)`; width `0` for none.
    pub neighbour_b: [f32; 4],

    /// `(radius_a, radius_b, blend, legibility)`.
    pub shape: [f32; 4],
}

impl Liquid {
    /// Packs `liquid` for the quad already transformed into `quad`.
    pub fn new(
        color: color::Packed,
        quad: Quad,
        liquid: &crate::core::material::Liquid,
        transformation: crate::core::Transformation,
        uniform_scale: f32,
    ) -> Self {
        let touch = liquid.touch.map_or([0.0; 4], |touch| {
            let position = touch.position * transformation;
            [
                position.x,
                position.y,
                touch.strength.clamp(0.0, 1.0),
                touch.radius.max(0.0) * uniform_scale,
            ]
        });

        let neighbour = |index: usize| {
            liquid.neighbours[index].map_or(([0.0; 4], 0.0), |neighbour| {
                let bounds = neighbour.bounds * transformation;
                (
                    [bounds.x, bounds.y, bounds.width, bounds.height],
                    neighbour.radius.max(0.0) * uniform_scale,
                )
            })
        };
        let (neighbour_a, radius_a) = neighbour(0);
        let (neighbour_b, radius_b) = neighbour(1);

        Self {
            color,
            quad,
            touch,
            neighbour_a,
            neighbour_b,
            shape: [
                radius_a,
                radius_b,
                liquid.blend.max(0.0) * uniform_scale,
                liquid.legibility.clamp(0.0, 1.0),
            ],
        }
    }
}
