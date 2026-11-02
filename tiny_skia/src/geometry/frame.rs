use super::cache::Geometry;
use super::convert;
use crate::Primitive;
use crate::core::text::LineHeight;
use crate::core::{self, Pixels, Point, Radians, Rectangle, Size, Svg, Vector};
use crate::graphics::geometry::fill::Fill;
use crate::graphics::geometry::stroke::Stroke;
use crate::graphics::geometry::{self, Path};
use crate::graphics::{self, Image, Text};

#[derive(Debug)]
pub struct Frame {
    clip_bounds: Rectangle,
    transform: tiny_skia::Transform,
    stack: Vec<tiny_skia::Transform>,
    primitives: Vec<Primitive>,
    images: Vec<Image>,
    text: Vec<Text>,
}

impl Frame {
    pub fn new(bounds: Rectangle) -> Self {
        Self {
            clip_bounds: bounds,
            transform: tiny_skia::Transform::identity(),
            stack: Vec::with_capacity(4),
            primitives: Vec::with_capacity(8),
            images: Vec::with_capacity(2),
            text: Vec::with_capacity(2),
        }
    }

    fn fill_path(&mut self, path: tiny_skia::Path, fill: Fill, anti_alias: bool) {
        let Some(path) = path.transform(self.transform) else {
            log::warn!("TAIM software fill skipped after invalid transform");
            return;
        };
        let mut paint = convert::paint(fill.style);
        paint.anti_alias = anti_alias;
        paint.shader.transform(self.transform);
        self.primitives
            .push(Primitive::fill(path, paint, convert::fill_rule(fill.rule)));
    }

    fn stroke_path(&mut self, path: tiny_skia::Path, stroke: Stroke<'_>) {
        let Some(path) = path.transform(self.transform) else {
            log::warn!("TAIM software stroke skipped after invalid transform");
            return;
        };
        let mut paint = convert::paint(stroke.style);
        paint.shader.transform(self.transform);
        self.primitives
            .push(Primitive::stroke(path, paint, convert::stroke(&stroke)));
    }
}

impl geometry::frame::Backend for Frame {
    type Geometry = Geometry;

    #[inline]
    fn width(&self) -> f32 {
        self.clip_bounds.width
    }

    #[inline]
    fn height(&self) -> f32 {
        self.clip_bounds.height
    }

    #[inline]
    fn size(&self) -> Size {
        self.clip_bounds.size()
    }

    #[inline]
    fn center(&self) -> Point {
        Point::new(self.clip_bounds.width * 0.5, self.clip_bounds.height * 0.5)
    }

    fn fill(&mut self, path: &Path, fill: impl Into<Fill>) {
        if let Some(path) = convert::path(path) {
            self.fill_path(path, fill.into(), true);
        }
    }

    fn fill_rectangle(&mut self, top_left: Point, size: Size, fill: impl Into<Fill>) {
        let Some(rectangle) =
            tiny_skia::Rect::from_xywh(top_left.x, top_left.y, size.width, size.height)
        else {
            return;
        };
        let path = tiny_skia::PathBuilder::from_rect(rectangle);
        self.fill_path(path, fill.into(), self.transform.has_skew());
    }

    fn stroke<'a>(&mut self, path: &Path, stroke: impl Into<Stroke<'a>>) {
        if let Some(path) = convert::path(path) {
            self.stroke_path(path, stroke.into());
        }
    }

    fn stroke_rectangle<'a>(&mut self, top_left: Point, size: Size, stroke: impl Into<Stroke<'a>>) {
        let Some(rectangle) =
            tiny_skia::Rect::from_xywh(top_left.x, top_left.y, size.width, size.height)
        else {
            return;
        };
        self.stroke_path(tiny_skia::PathBuilder::from_rect(rectangle), stroke.into());
    }

    fn fill_text(&mut self, text: impl Into<geometry::Text>) {
        let text = text.into();
        let (scale_x, scale_y) = self.transform.get_scale();

        if !self.transform.has_skew() && scale_x == scale_y && scale_x > 0.0 {
            let transformed = !self.transform.is_identity();
            let position = if transformed {
                let mut position = tiny_skia::Point {
                    x: text.position.x,
                    y: text.position.y,
                };
                self.transform.map_point(&mut position);
                Point::new(position.x, position.y)
            } else {
                text.position
            };
            let size = if transformed {
                Pixels(text.size.0 * scale_y)
            } else {
                text.size
            };
            let line_height = if transformed {
                match text.line_height {
                    LineHeight::Absolute(height) => {
                        LineHeight::Absolute(Pixels(height.0 * scale_y))
                    }
                    relative @ LineHeight::Relative(_) => relative,
                }
            } else {
                text.line_height
            };

            self.text.push(Text::Cached {
                content: text.content,
                bounds: Rectangle::new(
                    position,
                    Size::new(text.max_width * scale_x, f32::INFINITY),
                ),
                color: text.color,
                size,
                line_height: line_height.to_absolute(size),
                font: text.font,
                align_x: text.align_x,
                align_y: text.align_y,
                shaping: text.shaping,
                wrapping: text.wrapping,
                ellipsis: text.ellipsis,
                clip_bounds: self.clip_bounds,
            });
        } else {
            text.draw_with(|path, color| self.fill(&path, color));
        }
    }

    fn stroke_text<'a>(&mut self, text: impl Into<geometry::Text>, stroke: impl Into<Stroke<'a>>) {
        let text = text.into();
        let stroke = stroke.into();
        text.draw_with(|path, _| self.stroke(&path, stroke));
    }

    #[inline]
    fn push_transform(&mut self) {
        self.stack.push(self.transform);
    }

    fn pop_transform(&mut self) {
        if let Some(transform) = self.stack.pop() {
            self.transform = transform;
        } else {
            log::warn!("TAIM software geometry transform underflow ignored");
        }
    }

    fn draft(&mut self, clip_bounds: Rectangle) -> Self {
        Self::new(clip_bounds)
    }

    fn paste(&mut self, mut frame: Self) {
        self.primitives.append(&mut frame.primitives);
        self.text.append(&mut frame.text);
        self.images.append(&mut frame.images);
    }

    #[inline]
    fn translate(&mut self, translation: Vector) {
        self.transform = self.transform.pre_translate(translation.x, translation.y);
    }

    #[inline]
    fn rotate(&mut self, angle: impl Into<Radians>) {
        self.transform = self.transform.pre_rotate(angle.into().0.to_degrees());
    }

    #[inline]
    fn scale(&mut self, scale: f32) {
        self.transform = self.transform.pre_scale(scale, scale);
    }

    #[inline]
    fn scale_nonuniform(&mut self, scale: impl Into<Vector>) {
        let scale = scale.into();
        self.transform = self.transform.pre_scale(scale.x, scale.y);
    }

    fn into_geometry(self) -> Geometry {
        Geometry::Live {
            primitives: self.primitives,
            images: self.images,
            text: self.text,
            clip_bounds: self.clip_bounds,
        }
    }

    fn draw_image(&mut self, bounds: Rectangle, image: impl Into<core::Image>) {
        let mut image = image.into();
        let (bounds, rotation) = transform_rectangle(bounds, self.transform);
        let (scale_x, scale_y) = self.transform.get_scale();
        image.rotation += rotation;
        image.border_radius = image.border_radius * scale_x.abs().min(scale_y.abs());
        self.images.push(graphics::Image::Raster {
            image,
            bounds,
            clip_bounds: self.clip_bounds,
        });
    }

    fn draw_svg(&mut self, bounds: Rectangle, svg: impl Into<Svg>) {
        let mut svg = svg.into();
        let (bounds, rotation) = transform_rectangle(bounds, self.transform);
        svg.rotation += rotation;
        self.images.push(Image::Vector {
            svg,
            bounds,
            clip_bounds: self.clip_bounds,
        });
    }
}

fn transform_rectangle(
    rectangle: Rectangle,
    transform: tiny_skia::Transform,
) -> (Rectangle, Radians) {
    let mut vertices = [
        tiny_skia::Point {
            x: rectangle.x,
            y: rectangle.y,
        },
        tiny_skia::Point {
            x: rectangle.x + rectangle.width,
            y: rectangle.y,
        },
        tiny_skia::Point {
            x: rectangle.x,
            y: rectangle.y + rectangle.height,
        },
    ];
    transform.map_points(&mut vertices);

    Rectangle::with_vertices(
        Point::new(vertices[0].x, vertices[0].y),
        Point::new(vertices[1].x, vertices[1].y),
        Point::new(vertices[2].x, vertices[2].y),
    )
}
