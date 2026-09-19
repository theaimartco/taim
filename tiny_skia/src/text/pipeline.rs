use super::{draw, glyph_cache};
use crate::core::alignment;
use crate::core::text::fallback::{FallbackChain, Script};
use crate::core::text::paragraph::Direction;
use crate::core::text::{Alignment, Ellipsis, Shaping, Wrapping};
use crate::core::{Color, Font, Pixels, Point, Rectangle, Transformation};
use crate::graphics::text::cache::{self, Cache as ShapingCache};
use crate::graphics::text::{editor, font_system, paragraph};

use std::borrow::Cow;

#[derive(Debug)]
pub struct Pipeline {
    glyphs: glyph_cache::Cache,
    shaping: ShapingCache,
    swash: cosmic_text::SwashCache,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            glyphs: glyph_cache::Cache::new(),
            shaping: ShapingCache::new(),
            swash: cosmic_text::SwashCache::new(),
        }
    }

    #[allow(dead_code)]
    pub fn load_font(&mut self, bytes: Cow<'static, [u8]>) {
        font_system()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .load_font(bytes);
        self.shaping = ShapingCache::new();
        self.glyphs = glyph_cache::Cache::new();
        self.swash = cosmic_text::SwashCache::new();
    }

    pub fn draw_paragraph(
        &mut self,
        paragraph: &paragraph::Weak,
        position: Point,
        color: Color,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: Option<&tiny_skia::Mask>,
        transformation: Transformation,
    ) {
        let Some(paragraph) = paragraph.upgrade() else {
            return;
        };
        self.draw_buffer(
            paragraph.buffer(),
            position,
            color,
            pixels,
            clip_mask,
            transformation,
        );
    }

    pub fn draw_editor(
        &mut self,
        editor: &editor::Weak,
        position: Point,
        color: Color,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: Option<&tiny_skia::Mask>,
        transformation: Transformation,
    ) {
        let Some(editor) = editor.upgrade() else {
            return;
        };
        self.draw_buffer(
            editor.buffer(),
            position,
            color,
            pixels,
            clip_mask,
            transformation,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_cached(
        &mut self,
        content: &str,
        bounds: Rectangle,
        color: Color,
        size: Pixels,
        line_height: Pixels,
        font: Font,
        align_x: Alignment,
        align_y: alignment::Vertical,
        shaping: Shaping,
        wrapping: Wrapping,
        ellipsis: Ellipsis,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: Option<&tiny_skia::Mask>,
        transformation: Transformation,
    ) {
        let mut font_system = font_system()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let font_system = font_system.raw();
        let (_, entry) = self.shaping.allocate(
            font_system,
            cache::Key {
                bounds: bounds.size(),
                content,
                font,
                size: size.into(),
                line_height: line_height.into(),
                shaping,
                wrapping,
                ellipsis,
                align_x,
                locale: None,
                direction: Direction::Auto,
                script: Script::Latin,
                fallback: FallbackChain::for_script(Script::Latin),
                axes: &[],
            },
        );
        let x = match align_x {
            Alignment::Default | Alignment::Left | Alignment::Justified => bounds.x,
            Alignment::Center => bounds.x - entry.min_bounds.width * 0.5,
            Alignment::Right => bounds.x - entry.min_bounds.width,
        };
        let y = match align_y {
            alignment::Vertical::Top => bounds.y,
            alignment::Vertical::Center => bounds.y - entry.min_bounds.height * 0.5,
            alignment::Vertical::Bottom => bounds.y - entry.min_bounds.height,
        };

        draw::buffer(
            font_system,
            &mut self.glyphs,
            &mut self.swash,
            &entry.buffer,
            Point::new(x, y),
            color,
            pixels,
            clip_mask,
            transformation,
        );
    }

    pub fn draw_raw(
        &mut self,
        buffer: &cosmic_text::Buffer,
        position: Point,
        color: Color,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: Option<&tiny_skia::Mask>,
        transformation: Transformation,
    ) {
        self.draw_buffer(buffer, position, color, pixels, clip_mask, transformation);
    }

    pub fn trim_cache(&mut self) {
        self.shaping.trim();
        self.glyphs.trim();
    }

    pub fn cache_statistics(&self) -> cache::Statistics {
        self.shaping.statistics()
    }

    fn draw_buffer(
        &mut self,
        buffer: &cosmic_text::Buffer,
        position: Point,
        color: Color,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: Option<&tiny_skia::Mask>,
        transformation: Transformation,
    ) {
        let mut font_system = font_system()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        draw::buffer(
            font_system.raw(),
            &mut self.glyphs,
            &mut self.swash,
            buffer,
            position,
            color,
            pixels,
            clip_mask,
            transformation,
        );
    }
}
