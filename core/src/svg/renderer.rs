use super::{Handle, Svg};
use crate::{Rectangle, Size};

/// A [`Renderer`] that can render vector graphics.
///
/// [renderer]: crate::renderer
pub trait Renderer: crate::Renderer {
    /// Returns the default dimensions of an SVG for the given [`Handle`].
    fn measure_svg(&self, handle: &Handle) -> Size<u32>;

    /// Draws an SVG with the given [`Handle`], an optional [`Color`] filter, and inside the provided `bounds`.
    fn draw_svg(&mut self, svg: Svg, bounds: Rectangle, clip_bounds: Rectangle);
}
