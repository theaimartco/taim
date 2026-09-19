use crate::image;
use crate::renderer::{self, Renderer};
use crate::{Background, Rectangle, Size, Transformation};

impl Renderer for () {
    fn start_layer(&mut self, _bounds: Rectangle) {}
    fn end_layer(&mut self) {}
    fn start_transformation(&mut self, _transformation: Transformation) {}
    fn end_transformation(&mut self) {}
    fn fill_quad(&mut self, _quad: renderer::Quad, _background: impl Into<Background>) {}

    fn allocate_image(
        &mut self,
        handle: &image::Handle,
        callback: impl FnOnce(Result<image::Allocation, image::Error>) + Send + 'static,
    ) {
        callback(Ok(image::placeholder(handle, Size::new(100, 100))));
    }

    fn hint(&mut self, _scale_factor: f32) {}

    fn scale_factor(&self) -> Option<f32> {
        None
    }

    fn reset(&mut self, _new_bounds: Rectangle) {}
}
