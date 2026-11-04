//! The focused editor's temporary composition, drawn above the view tree.
use crate::{Border, Point, Rectangle, Renderer, Size, Theme};
use taim_core::Renderer as _;
use taim_core::input_method::{Preedit, Purpose};
use taim_core::renderer::Quad;
use taim_core::text::{self, Paragraph as _, Renderer as _};
use unicode_segmentation::UnicodeSegmentation;

type Paragraph = <Renderer as text::Renderer>::Paragraph;

#[derive(Default)]
pub(crate) struct Composition {
    // Recorded text primitives may borrow paragraph storage until present.
    paragraph: Option<Paragraph>,
}

impl Composition {
    pub(crate) fn clear(&mut self) {
        self.paragraph = None;
    }

    pub(crate) fn paint(
        &mut self,
        renderer: &mut Renderer,
        theme: &Theme,
        cursor: Rectangle,
        purpose: Purpose,
        preedit: &Preedit,
    ) {
        // Password composition must never be rendered in clear text.
        let content = if purpose == Purpose::Secure {
            "•".repeat(preedit.content.graphemes(true).count())
        } else {
            preedit.content.clone()
        };
        if content.is_empty() {
            return;
        }
        let size = preedit.text_size.unwrap_or_else(|| renderer.default_size());
        self.paragraph = Some(Paragraph::with_text(text::Text {
            content: content.as_str(),
            bounds: Size::new(480.0, f32::INFINITY),
            size,
            line_height: text::LineHeight::default(),
            font: renderer.default_font(),
            align_x: text::Alignment::Left,
            align_y: taim_core::alignment::Vertical::Top,
            shaping: text::Shaping::Advanced,
            wrapping: text::Wrapping::Word,
            ellipsis: text::Ellipsis::None,
            hint_factor: renderer.scale_factor(),
        }));
        let paragraph = self
            .paragraph
            .as_ref()
            .expect("composition was shaped above");
        let measured = paragraph.min_bounds();
        let area = Rectangle::new(
            Point::new(cursor.x, cursor.y),
            Size::new(
                measured.width.max(1.0) + 8.0,
                measured.height.max(1.0) + 6.0,
            ),
        );
        let palette = theme.palette();
        renderer.with_layer(area, |renderer| {
            renderer.fill_quad(
                Quad {
                    bounds: area,
                    border: Border::default()
                        .width(1.0)
                        .color(palette.primary.base.color),
                    ..Quad::default()
                },
                palette.background.base.color,
            );
            renderer.fill_paragraph(
                paragraph,
                Point::new(area.x + 4.0, area.y + 3.0),
                palette.background.base.text,
                area,
            );
        });
    }
}
