//! The input-method composition overlay.
//!
//! While an input method composes text (pinyin, kana, dead keys…), TAIM
//! draws the uncommitted text in a small opaque box under the caret, with an
//! underline and the IME's selection highlighted. The shaped paragraph is
//! cached and rebuilt only when the composed text or its style changes.
use crate::core::alignment;
use crate::core::input_method;
use crate::core::renderer;
use crate::core::text::{self, Paragraph as _};
use crate::core::{Color, Padding, Pixels, Point, Rectangle, Size, Text, Vector};

/// Height of the composition underline, in logical pixels.
const UNDERLINE: f32 = 2.0;

/// Shown in place of an empty selection so the caret stays visible.
const CARET: &str = "\u{200A}";

pub(super) struct Preedit<Renderer>
where
    Renderer: text::Renderer,
{
    /// Bottom-left corner of the overlay: just below the caret.
    anchor: Point,
    paragraph: Renderer::Paragraph,
    size: Size,
    highlight: Vec<Rectangle>,
    /// What `paragraph` was shaped from.
    shaped: Option<Shaped<Renderer::Font>>,
}

#[derive(PartialEq)]
struct Shaped<Font> {
    spans: Vec<text::Span<'static, (), Font>>,
    size: Pixels,
    font: Font,
    hint_factor: Option<f32>,
}

impl<Renderer> Preedit<Renderer>
where
    Renderer: text::Renderer,
{
    pub(super) fn new() -> Self {
        Self {
            anchor: Point::ORIGIN,
            paragraph: Renderer::Paragraph::default(),
            size: Size::ZERO,
            highlight: Vec::new(),
            shaped: None,
        }
    }

    pub(super) fn update(
        &mut self,
        caret: Rectangle,
        preedit: &input_method::Preedit,
        background: Color,
        renderer: &Renderer,
    ) {
        self.anchor = caret.position() + Vector::new(0.0, caret.height);

        // The selected part is drawn in the background color on top of a
        // highlight quad, which reads as inverted text.
        let inverted = Color {
            a: 1.0,
            ..background
        };
        let spans: Vec<_> = segments(preedit)
            .into_iter()
            .map(|(content, selected)| {
                let span = text::Span::new(content);
                if selected { span.color(inverted) } else { span }
            })
            .collect();

        let size = preedit.text_size.unwrap_or_else(|| renderer.default_size());
        let font = renderer.default_font();
        let hint_factor = renderer.scale_factor();

        let unchanged = self.shaped.as_ref().is_some_and(|shaped| {
            shaped.size == size
                && shaped.font == font
                && shaped.hint_factor == hint_factor
                && shaped.spans.as_slice() == spans.as_slice()
        });
        if unchanged {
            return;
        }

        self.paragraph = Renderer::Paragraph::with_spans(Text {
            content: spans.as_slice(),
            bounds: Size::INFINITE,
            size,
            line_height: text::LineHeight::default(),
            font,
            align_x: text::Alignment::Default,
            align_y: alignment::Vertical::Top,
            shaping: text::Shaping::Advanced,
            wrapping: text::Wrapping::None,
            ellipsis: text::Ellipsis::None,
            hint_factor,
        });
        self.size = self.paragraph.min_bounds();
        self.highlight = if spans.len() == 3 {
            self.paragraph.span_bounds(1)
        } else {
            Vec::new()
        };

        self.shaped = Some(Shaped {
            spans: spans.into_iter().map(text::Span::to_static).collect(),
            size,
            font,
            hint_factor,
        });
    }

    pub(super) fn draw(
        &self,
        renderer: &mut Renderer,
        color: Color,
        background: Color,
        viewport: &Rectangle,
    ) {
        if self.size.width < 1.0 {
            return;
        }

        let bounds = keep_inside(
            Rectangle::new(self.anchor - Vector::new(0.0, self.size.height), self.size),
            viewport,
        );
        let quad = |bounds| renderer::Quad {
            bounds,
            ..renderer::Quad::default()
        };

        renderer.with_layer(bounds, |renderer| {
            let opaque = Color {
                a: 1.0,
                ..background
            };
            renderer.fill_quad(quad(bounds), opaque);
            renderer.fill_paragraph(&self.paragraph, bounds.position(), color, bounds);

            let underline = bounds.shrink(Padding {
                top: (bounds.height - UNDERLINE).max(0.0),
                ..Padding::ZERO
            });
            renderer.fill_quad(quad(underline), color);

            let offset = bounds.position() - Point::ORIGIN;
            for selected in &self.highlight {
                renderer.fill_quad(quad(*selected + offset), color);
            }
        });
    }
}

/// Splits the composed text into `(text, selected)` runs: the whole text,
/// or before / selection / after when the IME reports a valid selection.
fn segments(preedit: &input_method::Preedit) -> Vec<(&str, bool)> {
    let text = preedit.content.as_str();

    let selection = preedit.selection.as_ref().filter(|range| {
        range.start <= range.end
            && range.end <= text.len()
            && text.is_char_boundary(range.start)
            && text.is_char_boundary(range.end)
    });

    match selection {
        Some(range) => {
            let selected = if range.is_empty() {
                CARET
            } else {
                &text[range.clone()]
            };

            vec![
                (&text[..range.start], false),
                (selected, true),
                (&text[range.end..], false),
            ]
        }
        None => vec![(text, false)],
    }
}

/// Moves `bounds` (without resizing it) so it stays inside `viewport` where
/// it fits; the top-left corner wins when it does not.
fn keep_inside(mut bounds: Rectangle, viewport: &Rectangle) -> Rectangle {
    let right = (viewport.x + viewport.width - bounds.width).max(viewport.x);
    let bottom = (viewport.y + viewport.height - bounds.height).max(viewport.y);

    bounds.x = bounds.x.clamp(viewport.x, right);
    bounds.y = bounds.y.clamp(viewport.y, bottom);
    bounds
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preedit(content: &str, selection: Option<std::ops::Range<usize>>) -> input_method::Preedit {
        input_method::Preedit {
            content: content.to_owned(),
            selection,
            text_size: None,
        }
    }

    #[test]
    fn selection_splits_into_three_runs() {
        let composing = preedit("nihao", Some(2..5));
        assert_eq!(
            segments(&composing),
            vec![("ni", false), ("hao", true), ("", false)]
        );
    }

    #[test]
    #[allow(clippy::reversed_empty_ranges)] // a reversed range is one of the cases
    fn invalid_selections_are_ignored() {
        for selection in [Some(3..1), Some(0..99), Some(1..2)] {
            let composing = preedit("é", selection);
            assert_eq!(segments(&composing), vec![("é", false)]);
        }
    }

    #[test]
    fn empty_selection_shows_a_caret() {
        let composing = preedit("ab", Some(1..1));
        assert_eq!(segments(&composing)[1], (CARET, true));
    }

    #[test]
    fn overlay_stays_on_screen() {
        let viewport = Rectangle::new(Point::ORIGIN, Size::new(100.0, 100.0));
        let moved = keep_inside(
            Rectangle::new(Point::new(90.0, -5.0), Size::new(30.0, 10.0)),
            &viewport,
        );

        assert_eq!(moved.position(), Point::new(70.0, 0.0));
    }
}
