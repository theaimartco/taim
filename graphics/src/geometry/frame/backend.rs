//! Backend contract for retained geometry frames.

use crate::core::{Point, Radians, Rectangle, Size, Vector};
use crate::geometry::{Fill, Image, Path, Stroke, Svg, Text};

/// Renderer-owned implementation behind a geometry frame.
#[allow(missing_docs)]
pub trait Backend: Sized {
    type Geometry;

    fn width(&self) -> f32;
    fn height(&self) -> f32;
    fn size(&self) -> Size;
    fn center(&self) -> Point;

    fn push_transform(&mut self);
    fn pop_transform(&mut self);

    fn translate(&mut self, translation: Vector);
    fn rotate(&mut self, angle: impl Into<Radians>);
    fn scale(&mut self, scale: f32);
    fn scale_nonuniform(&mut self, scale: impl Into<Vector>);

    fn draft(&mut self, clip_bounds: Rectangle) -> Self;
    fn paste(&mut self, frame: Self);

    fn stroke<'a>(&mut self, path: &Path, stroke: impl Into<Stroke<'a>>);
    fn stroke_rectangle<'a>(&mut self, top_left: Point, size: Size, stroke: impl Into<Stroke<'a>>);
    fn stroke_text<'a>(&mut self, text: impl Into<Text>, stroke: impl Into<Stroke<'a>>);

    fn fill(&mut self, path: &Path, fill: impl Into<Fill>);
    fn fill_text(&mut self, text: impl Into<Text>);
    fn fill_rectangle(&mut self, top_left: Point, size: Size, fill: impl Into<Fill>);

    fn draw_image(&mut self, bounds: Rectangle, image: impl Into<Image>);
    fn draw_svg(&mut self, bounds: Rectangle, svg: impl Into<Svg>);

    fn into_geometry(self) -> Self::Geometry;
}

#[cfg(debug_assertions)]
impl Backend for () {
    type Geometry = ();

    fn width(&self) -> f32 {
        0.0
    }

    fn height(&self) -> f32 {
        0.0
    }

    fn size(&self) -> Size {
        Size::ZERO
    }

    fn center(&self) -> Point {
        Point::ORIGIN
    }

    fn push_transform(&mut self) {}
    fn pop_transform(&mut self) {}

    fn translate(&mut self, _translation: Vector) {}
    fn rotate(&mut self, _angle: impl Into<Radians>) {}
    fn scale(&mut self, _scale: f32) {}
    fn scale_nonuniform(&mut self, _scale: impl Into<Vector>) {}

    fn draft(&mut self, _clip_bounds: Rectangle) -> Self {}
    fn paste(&mut self, _frame: Self) {}

    fn stroke<'a>(&mut self, _path: &Path, _stroke: impl Into<Stroke<'a>>) {}
    fn stroke_rectangle<'a>(
        &mut self,
        _top_left: Point,
        _size: Size,
        _stroke: impl Into<Stroke<'a>>,
    ) {
    }
    fn stroke_text<'a>(&mut self, _text: impl Into<Text>, _stroke: impl Into<Stroke<'a>>) {}

    fn fill(&mut self, _path: &Path, _fill: impl Into<Fill>) {}
    fn fill_text(&mut self, _text: impl Into<Text>) {}
    fn fill_rectangle(&mut self, _top_left: Point, _size: Size, _fill: impl Into<Fill>) {}

    fn draw_image(&mut self, _bounds: Rectangle, _image: impl Into<Image>) {}
    fn draw_svg(&mut self, _bounds: Rectangle, _svg: impl Into<Svg>) {}

    fn into_geometry(self) -> Self::Geometry {}
}
