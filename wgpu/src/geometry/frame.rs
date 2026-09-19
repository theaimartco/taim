use super::buffers;
use super::cache::Geometry;
use super::{dash, transform};

use crate::core::text::LineHeight;
use crate::core::{self, Pixels, Point, Radians, Rectangle, Size, Svg, Vector};
use crate::graphics::geometry::fill::Fill;
use crate::graphics::geometry::{self, Path, Stroke};
use crate::graphics::mesh::Mesh;
use crate::graphics::{Image, Text};

use lyon::tessellation;
use std::borrow::Cow;

pub struct Frame {
    clip_bounds: Rectangle,
    buffers: buffers::Stack,
    meshes: Vec<Mesh>,
    images: Vec<Image>,
    text: Vec<Text>,
    transforms: transform::Stack,
    fill_tessellator: tessellation::FillTessellator,
    stroke_tessellator: tessellation::StrokeTessellator,
}

impl Frame {
    pub fn new(bounds: Rectangle) -> Self {
        Self {
            clip_bounds: bounds,
            buffers: buffers::Stack::new(),
            meshes: Vec::new(),
            images: Vec::new(),
            text: Vec::new(),
            transforms: transform::Stack::new(),
            fill_tessellator: tessellation::FillTessellator::new(),
            stroke_tessellator: tessellation::StrokeTessellator::new(),
        }
    }

    fn record_fill(&mut self, path: &Path, fill: Fill) {
        let style = self.transforms.current.style(fill.style);
        let transformed;
        let path = if self.transforms.current.is_identity() {
            path
        } else {
            transformed = path.transform(&self.transforms.current.0);
            &transformed
        };
        if let Err(error) = self.buffers.fill_path(
            &mut self.fill_tessellator,
            path,
            &buffers::fill_options(fill.rule),
            &style,
        ) {
            log::warn!("TAIM fill tessellation skipped: {error:?}");
        }
    }

    fn record_stroke(&mut self, path: &Path, stroke: Stroke<'_>) {
        let style = self.transforms.current.style(stroke.style);
        let dashed;
        let path = if stroke.line_dash.segments.is_empty() {
            Cow::Borrowed(path)
        } else {
            dashed = dash::apply(path, stroke.line_dash);
            Cow::Owned(dashed)
        };
        let transformed;
        let path = if self.transforms.current.is_identity() {
            path.as_ref()
        } else {
            transformed = path.transform(&self.transforms.current.0);
            &transformed
        };
        if let Err(error) = self.buffers.stroke_path(
            &mut self.stroke_tessellator,
            path,
            &buffers::stroke_options(stroke.width, stroke.line_cap, stroke.line_join),
            &style,
        ) {
            log::warn!("TAIM stroke tessellation skipped: {error:?}");
        }
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
        self.record_fill(path, fill.into());
    }

    fn fill_rectangle(&mut self, top_left: Point, size: Size, fill: impl Into<Fill>) {
        let fill = fill.into();
        if !self.transforms.current.is_positive_axis_aligned() {
            self.record_fill(&Path::rectangle(top_left, size), fill);
            return;
        }

        let style = self.transforms.current.style(fill.style);
        let top_left = self.transforms.current.point(top_left);
        let size = self
            .transforms
            .current
            .vector(Vector::new(size.width, size.height));
        let rectangle = lyon::math::Box2D::new(
            lyon::math::Point::new(top_left.x, top_left.y),
            lyon::math::Point::new(top_left.x + size.x, top_left.y + size.y),
        );
        if let Err(error) = self.buffers.fill_rectangle(
            &mut self.fill_tessellator,
            &rectangle,
            &buffers::fill_options(fill.rule),
            &style,
        ) {
            log::warn!("TAIM rectangle fill skipped: {error:?}");
        }
    }

    fn stroke<'a>(&mut self, path: &Path, stroke: impl Into<Stroke<'a>>) {
        self.record_stroke(path, stroke.into());
    }

    fn stroke_rectangle<'a>(&mut self, top_left: Point, size: Size, stroke: impl Into<Stroke<'a>>) {
        let stroke = stroke.into();
        if !self.transforms.current.is_positive_axis_aligned()
            || !stroke.line_dash.segments.is_empty()
        {
            self.record_stroke(&Path::rectangle(top_left, size), stroke);
            return;
        }

        let style = self.transforms.current.style(stroke.style);
        let top_left = self.transforms.current.point(top_left);
        let size = self
            .transforms
            .current
            .vector(Vector::new(size.width, size.height));
        let rectangle = lyon::math::Box2D::new(
            lyon::math::Point::new(top_left.x, top_left.y),
            lyon::math::Point::new(top_left.x + size.x, top_left.y + size.y),
        );
        if let Err(error) = self.buffers.stroke_rectangle(
            &mut self.stroke_tessellator,
            &rectangle,
            &buffers::stroke_options(stroke.width, stroke.line_cap, stroke.line_join),
            &style,
        ) {
            log::warn!("TAIM rectangle stroke skipped: {error:?}");
        }
    }

    fn stroke_text<'a>(&mut self, text: impl Into<geometry::Text>, stroke: impl Into<Stroke<'a>>) {
        let text = text.into();
        let stroke = stroke.into();
        text.draw_with(|glyph, _| self.record_stroke(&glyph, stroke));
    }

    fn fill_text(&mut self, text: impl Into<geometry::Text>) {
        let text = text.into();
        let (scale_x, scale_y) = self.transforms.current.scale();

        if self.transforms.current.is_positive_axis_aligned() && scale_x == scale_y && scale_x > 0.0
        {
            let transformed = !self.transforms.current.is_identity();
            let position = if transformed {
                self.transforms.current.point(text.position)
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
                    relative => relative,
                }
            } else {
                text.line_height
            };
            let max_width = if transformed {
                text.max_width * scale_x
            } else {
                text.max_width
            };

            self.text.push(Text::Cached {
                content: text.content,
                bounds: Rectangle::new(position, Size::new(max_width, f32::INFINITY)),
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
            text.draw_with(|path, color| self.record_fill(&path, color.into()));
        }
    }

    #[inline]
    fn translate(&mut self, translation: Vector) {
        self.transforms.current.0 = self
            .transforms
            .current
            .0
            .pre_translate(lyon::math::Vector::new(translation.x, translation.y));
    }

    #[inline]
    fn rotate(&mut self, angle: impl Into<Radians>) {
        self.transforms.current.0 = self
            .transforms
            .current
            .0
            .pre_rotate(lyon::math::Angle::radians(angle.into().0));
    }

    #[inline]
    fn scale(&mut self, scale: f32) {
        self.transforms.current.0 = self.transforms.current.0.pre_scale(scale, scale);
    }

    #[inline]
    fn scale_nonuniform(&mut self, scale: impl Into<Vector>) {
        let scale = scale.into();
        self.transforms.current.0 = self.transforms.current.0.pre_scale(scale.x, scale.y);
    }

    fn push_transform(&mut self) {
        self.transforms.push();
    }

    fn pop_transform(&mut self) {
        self.transforms.pop();
    }

    fn draft(&mut self, clip_bounds: Rectangle) -> Self {
        Self::new(clip_bounds)
    }

    fn paste(&mut self, mut frame: Self) {
        self.meshes.append(&mut frame.meshes);
        self.meshes
            .extend(frame.buffers.into_meshes(frame.clip_bounds));
        self.images.append(&mut frame.images);
        self.text.append(&mut frame.text);
    }

    fn into_geometry(mut self) -> Geometry {
        self.meshes
            .extend(self.buffers.into_meshes(self.clip_bounds));
        Geometry::Live {
            meshes: self.meshes,
            images: self.images,
            text: self.text,
        }
    }

    fn draw_image(&mut self, bounds: Rectangle, image: impl Into<core::Image>) {
        let mut image = image.into();
        let (bounds, rotation) = self.transforms.current.rectangle(bounds);
        let scale = self.transforms.current.scale();
        image.rotation += rotation;
        image.border_radius = image.border_radius * scale.0.abs().min(scale.1.abs());
        self.images.push(Image::Raster {
            image,
            bounds,
            clip_bounds: self.clip_bounds,
        });
    }

    fn draw_svg(&mut self, bounds: Rectangle, svg: impl Into<Svg>) {
        let mut svg = svg.into();
        let (bounds, rotation) = self.transforms.current.rectangle(bounds);
        svg.rotation += rotation;
        self.images.push(Image::Vector {
            svg,
            bounds,
            clip_bounds: self.clip_bounds,
        });
    }
}
