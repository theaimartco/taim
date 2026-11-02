//! Build and draw geometry.
pub mod fill;
pub mod frame;
pub mod path;
pub mod stroke;

mod cache;
mod style;
mod text;

pub use cache::Cache;
pub use fill::Fill;
pub use frame::Frame;
pub use path::Path;
pub use stroke::{LineCap, LineDash, LineJoin, Stroke};
pub use style::Style;
pub use text::Text;

pub use crate::core::{Image, Svg};
pub use crate::gradient::{self, Gradient};

use crate::cache::Cached;
use crate::core::{self, Rectangle};

/// A renderer capable of drawing some [`Self::Geometry`].
pub trait Renderer: core::Renderer {
    /// The kind of geometry this renderer can draw.
    type Geometry: Cached;

    /// The kind of [`Frame`] this renderer supports.
    type Frame: frame::Backend<Geometry = Self::Geometry>;

    /// Creates a new [`Self::Frame`].
    fn new_frame(&self, bounds: Rectangle) -> Self::Frame;

    /// Create a frame only when TAIM's explicit geometry contract accepts
    /// the requested rectangle. Useful for untrusted plugin layout values.
    fn try_new_frame(
        &self,
        bounds: Rectangle,
    ) -> Result<Self::Frame, crate::validation::BoundsError> {
        crate::validation::validate_frame_bounds(bounds)?;
        Ok(self.new_frame(bounds))
    }

    /// Draws the given [`Self::Geometry`].
    fn draw_geometry(&mut self, geometry: Self::Geometry);
}

#[cfg(debug_assertions)]
impl Renderer for () {
    type Geometry = ();
    type Frame = ();

    fn new_frame(&self, _bounds: Rectangle) -> Self::Frame {}

    fn draw_geometry(&mut self, _geometry: Self::Geometry) {}
}

#[cfg(all(test, debug_assertions))]
mod taim_geometry_frame_tests {
    use super::*;
    use crate::core::{Point, Size};

    #[test]
    fn invalid_geometry_does_not_enter_the_render_backend() {
        let rect = Rectangle::new(Point::new(f32::NAN, 0.0), Size::new(10.0, 10.0));
        assert!(matches!(
            ().try_new_frame(rect),
            Err(crate::validation::BoundsError::NonFinite)
        ));
        let valid = Rectangle::new(Point::ORIGIN, Size::new(10.0, 10.0));
        assert!(().try_new_frame(valid).is_ok());
    }
}
