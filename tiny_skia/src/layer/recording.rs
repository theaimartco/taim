use super::{DrawBatch, Layer, Quad};
use crate::Primitive;
use crate::core::renderer;
use crate::core::{self, Background, Color, Point, Rectangle, Svg, Transformation};
use crate::graphics::text::{Editor, Paragraph, Text};
use crate::graphics::{self, Image};

use std::sync::Arc;

impl Layer {
    pub fn draw_quad(
        &mut self,
        primitive: renderer::Quad,
        background: Background,
        transformation: Transformation,
    ) {
        self.quads.push(Quad {
            primitive,
            background,
            transformation,
        });
    }

    pub fn draw_paragraph(
        &mut self,
        paragraph: &Paragraph,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        let text = Text::Paragraph {
            paragraph: paragraph.downgrade(),
            position,
            color,
            clip_bounds,
            transformation: Transformation::IDENTITY,
        };
        self.text.push(DrawBatch::one(
            text,
            clip_bounds * transformation,
            transformation,
        ));
    }

    pub fn draw_editor(
        &mut self,
        editor: &Editor,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        let text = Text::Editor {
            editor: editor.downgrade(),
            position,
            color,
            clip_bounds,
            transformation: Transformation::IDENTITY,
        };
        self.text.push(DrawBatch::one(
            text,
            clip_bounds * transformation,
            transformation,
        ));
    }

    pub fn draw_text(
        &mut self,
        text: core::Text,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        let text = Text::Cached {
            content: text.content,
            bounds: Rectangle::new(position, text.bounds),
            color,
            size: text.size,
            line_height: text.line_height.to_absolute(text.size),
            font: text.font,
            align_x: text.align_x,
            align_y: text.align_y,
            shaping: text.shaping,
            wrapping: text.wrapping,
            ellipsis: text.ellipsis,
            clip_bounds,
        };
        self.text.push(DrawBatch::one(
            text,
            clip_bounds * transformation,
            transformation,
        ));
    }

    pub fn draw_text_raw(&mut self, raw: graphics::text::Raw, transformation: Transformation) {
        let clip_bounds = raw.clip_bounds;
        self.text.push(DrawBatch::one(
            Text::Raw {
                raw,
                transformation: Transformation::IDENTITY,
            },
            clip_bounds * transformation,
            transformation,
        ));
    }

    pub fn draw_text_group(
        &mut self,
        text: Vec<Text>,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        if text.is_empty() {
            return;
        }
        self.text.push(DrawBatch::owned(
            text,
            clip_bounds * transformation,
            transformation,
        ));
    }

    pub fn draw_text_cache(
        &mut self,
        text: Arc<[Text]>,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        if text.is_empty() {
            return;
        }
        self.text.push(DrawBatch::shared(
            text,
            clip_bounds * transformation,
            transformation,
        ));
    }

    pub fn draw_image(&mut self, image: Image, transformation: Transformation) {
        let clip_bounds = match &image {
            Image::Raster { clip_bounds, .. } | Image::Vector { clip_bounds, .. } => *clip_bounds,
        };
        self.images.push(DrawBatch::one(
            image,
            clip_bounds * transformation,
            transformation,
        ));
    }

    pub fn draw_image_group(
        &mut self,
        images: Vec<Image>,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        if images.is_empty() {
            return;
        }
        self.images.push(DrawBatch::owned(
            images,
            clip_bounds * transformation,
            transformation,
        ));
    }

    pub fn draw_image_cache(
        &mut self,
        images: Arc<[Image]>,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        if images.is_empty() {
            return;
        }
        self.images.push(DrawBatch::shared(
            images,
            clip_bounds * transformation,
            transformation,
        ));
    }

    pub fn draw_raster(
        &mut self,
        image: core::Image,
        bounds: Rectangle,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        self.draw_image(
            Image::Raster {
                image,
                bounds,
                clip_bounds,
            },
            transformation,
        );
    }

    pub fn draw_svg(
        &mut self,
        svg: Svg,
        bounds: Rectangle,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        self.draw_image(
            Image::Vector {
                svg,
                bounds,
                clip_bounds,
            },
            transformation,
        );
    }

    pub fn draw_primitive_group(
        &mut self,
        primitives: Vec<Primitive>,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        if primitives.is_empty() {
            return;
        }
        self.primitives.push(DrawBatch::owned(
            primitives,
            clip_bounds * transformation,
            transformation,
        ));
    }

    pub fn draw_primitive_cache(
        &mut self,
        primitives: Arc<[Primitive]>,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        if primitives.is_empty() {
            return;
        }
        self.primitives.push(DrawBatch::shared(
            primitives,
            clip_bounds * transformation,
            transformation,
        ));
    }
}
