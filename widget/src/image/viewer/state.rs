use crate::core::image;
use crate::core::{ContentFit, Point, Rectangle, Size, Vector};

/// Retained zoom, image-size, and drag state.
#[derive(Debug, Clone, Copy)]
pub struct State {
    pub(super) scale: f32,
    pub(super) image_size: Size,
    pub(super) starting_offset: Vector,
    pub(super) current_offset: Vector,
    pub(super) cursor_grabbed_at: Option<Point>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            scale: 1.0,
            image_size: Size::ZERO,
            starting_offset: Vector::ZERO,
            current_offset: Vector::ZERO,
            cursor_grabbed_at: None,
        }
    }
}

impl State {
    /// Creates default viewer state.
    pub fn new() -> Self {
        Self::default()
    }

    pub(super) fn offset(&self, bounds: Rectangle, image_size: Size) -> Vector {
        let limit = pan_limit(bounds.size(), image_size);

        Vector::new(
            self.current_offset.x.clamp(-limit.x, limit.x),
            self.current_offset.y.clamp(-limit.y, limit.y),
        )
    }

    /// Returns whether an active pointer drag exists.
    pub fn is_cursor_grabbed(&self) -> bool {
        self.cursor_grabbed_at.is_some()
    }
}

/// Returns the fitted, zoomed image size.
pub fn scaled_image_size<Renderer>(
    renderer: &Renderer,
    handle: &<Renderer as image::Renderer>::Handle,
    state: &State,
    bounds: Size,
    content_fit: ContentFit,
) -> Size
where
    Renderer: image::Renderer,
{
    let measured = renderer.measure_image(handle).unwrap_or_default();
    let image_size = Size::new(measured.width as f32, measured.height as f32);

    fitted_size(image_size, state.scale, bounds, content_fit)
}

pub(super) fn fitted_size(
    image_size: Size,
    scale: f32,
    bounds: Size,
    content_fit: ContentFit,
) -> Size {
    content_fit.fit(image_size, bounds) * scale
}

pub(super) fn pan_limit(bounds: Size, image_size: Size) -> Vector {
    Vector::new(
        ((image_size.width - bounds.width) / 2.0).max(0.0).round(),
        ((image_size.height - bounds.height) / 2.0).max(0.0).round(),
    )
}
