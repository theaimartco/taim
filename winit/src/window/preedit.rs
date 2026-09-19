//! Cached input-method preedit overlay.

use crate::core::alignment;
use crate::core::input_method;
use crate::core::renderer;
use crate::core::text;
use crate::core::{Color, Padding, Point, Rectangle, Size, Text, Vector};

pub(super) struct Preedit<Renderer>
where
    Renderer: text::Renderer,
{
    position: Point,
    content: Renderer::Paragraph,
    content_size: Size,
    selection_bounds: Vec<Rectangle>,
    spans: Vec<text::Span<'static, (), Renderer::Font>>,
    paragraph_style: Option<(crate::core::Pixels, Renderer::Font, Option<f32>)>,
}

impl<Renderer> Preedit<Renderer>
where
    Renderer: text::Renderer,
{
    pub(super) fn new() -> Self {
        Self {
            position: Point::ORIGIN,
            content: Renderer::Paragraph::default(),
            content_size: Size::ZERO,
            selection_bounds: Vec::new(),
            spans: Vec::new(),
            paragraph_style: None,
        }
    }

    pub(super) fn update(
        &mut self,
        cursor: Rectangle,
        preedit: &input_method::Preedit,
        background: Color,
        renderer: &Renderer,
    ) {
        self.position = cursor.position() + Vector::new(0.0, cursor.height);
        let background = Color {
            a: 1.0,
            ..background
        };

        let mut stack_spans = [
            text::Span::new(""),
            text::Span::new(""),
            text::Span::new(""),
        ];
        let span_count = if let Some(selection) = preedit.selection.as_ref().filter(|selection| {
            selection.start <= selection.end
                && selection.end <= preedit.content.len()
                && preedit.content.is_char_boundary(selection.start)
                && preedit.content.is_char_boundary(selection.end)
        }) {
            stack_spans[0] = text::Span::new(&preedit.content[..selection.start]);
            stack_spans[1] = text::Span::new(if selection.is_empty() {
                "\u{200A}"
            } else {
                &preedit.content[selection.clone()]
            })
            .color(background);
            stack_spans[2] = text::Span::new(&preedit.content[selection.end..]);
            3
        } else {
            stack_spans[0] = text::Span::new(&preedit.content);
            1
        };

        let spans = &stack_spans[..span_count];
        let paragraph_style = (
            preedit.text_size.unwrap_or_else(|| renderer.default_size()),
            renderer.default_font(),
            renderer.scale_factor(),
        );

        if spans == self.spans.as_slice() && self.paragraph_style.as_ref() == Some(&paragraph_style)
        {
            return;
        }

        use text::Paragraph as _;

        self.content = Renderer::Paragraph::with_spans(Text {
            content: spans,
            bounds: Size::INFINITE,
            size: paragraph_style.0,
            line_height: text::LineHeight::default(),
            font: paragraph_style.1,
            align_x: text::Alignment::Default,
            align_y: alignment::Vertical::Top,
            shaping: text::Shaping::Advanced,
            wrapping: text::Wrapping::None,
            ellipsis: text::Ellipsis::None,
            hint_factor: paragraph_style.2,
        });
        self.content_size = self.content.min_bounds();
        self.selection_bounds = self.content.span_bounds(1);

        self.spans.clear();
        self.spans
            .extend(spans.iter().cloned().map(text::Span::to_static));
        self.paragraph_style = Some(paragraph_style);
    }

    pub(super) fn draw(
        &self,
        renderer: &mut Renderer,
        color: Color,
        background: Color,
        viewport: &Rectangle,
    ) {
        if self.content_size.width < 1.0 {
            return;
        }

        let mut bounds = Rectangle::new(
            self.position - Vector::new(0.0, self.content_size.height),
            self.content_size,
        );
        let max_x = (viewport.x + viewport.width - bounds.width).max(viewport.x);
        let max_y = (viewport.y + viewport.height - bounds.height).max(viewport.y);
        bounds.x = bounds.x.clamp(viewport.x, max_x);
        bounds.y = bounds.y.clamp(viewport.y, max_y);

        renderer.with_layer(bounds, |renderer| {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    ..Default::default()
                },
                Color {
                    a: 1.0,
                    ..background
                },
            );
            renderer.fill_paragraph(&self.content, bounds.position(), color, bounds);

            const UNDERLINE: f32 = 2.0;
            renderer.fill_quad(
                renderer::Quad {
                    bounds: bounds.shrink(Padding {
                        top: (bounds.height - UNDERLINE).max(0.0),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                color,
            );

            let translation = bounds.position() - Point::ORIGIN;
            for span_bounds in &self.selection_bounds {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: *span_bounds + translation,
                        ..Default::default()
                    },
                    color,
                );
            }
        });
    }
}
