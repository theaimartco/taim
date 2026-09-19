use crate::image::{self, Image};
use crate::svg;
use crate::{Rectangle, Size};

impl image::Renderer for () {
    type Handle = image::Handle;

    fn load_image(&self, handle: &Self::Handle) -> Result<image::Allocation, image::Error> {
        Ok(image::placeholder(handle, Size::new(100, 100)))
    }

    fn measure_image(&self, _: &Self::Handle) -> Option<Size<u32>> {
        Some(Size::new(100, 100))
    }

    fn draw_image(&mut self, _: Image, _: Rectangle, _: Rectangle) {}
}

impl svg::Renderer for () {
    fn measure_svg(&self, _: &svg::Handle) -> Size<u32> {
        Size::new(0, 0)
    }

    fn draw_svg(&mut self, _: svg::Svg, _: Rectangle, _: Rectangle) {}
}
