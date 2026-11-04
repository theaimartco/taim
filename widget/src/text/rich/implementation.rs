use super::{Rich, State};
use crate::core::alignment;
use crate::core::layout;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::text::{Paragraph, Span};
use crate::core::widget::text::{
    self, Alignment, Catalog, Ellipsis, LineHeight, Shaping, Wrapping,
};
use crate::core::widget::tree::{self, Tree};
use crate::core::{
    self, Element, Event, Layout, Length, Pixels, Point, Rectangle, Shell, Size, Vector, Widget,
};

impl<Link, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Rich<'_, Link, Message, Theme, Renderer>
where
    Link: Clone + 'static,
    Theme: Catalog,
    Renderer: core::text::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Link, Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::<Link, _> {
            spans: Vec::new(),
            hovered_link: None,
            span_pressed: None,
            paragraph: Renderer::Paragraph::default(),
        })
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout(
            tree.state
                .downcast_mut::<State<Link, Renderer::Paragraph>>(),
            renderer,
            limits,
            self.width,
            self.height,
            self.spans.as_ref().as_ref(),
            self.line_height,
            self.size,
            self.font,
            self.align_x,
            self.align_y,
            self.wrapping,
            self.ellipsis,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        defaults: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        if !layout.bounds().intersects(viewport) {
            return;
        }

        let state = tree
            .state
            .downcast_ref::<State<Link, Renderer::Paragraph>>();
        let style = theme.style(&self.class);

        for (index, span) in self.spans.as_ref().as_ref().iter().enumerate() {
            let is_hovered_link = self.on_link_click.is_some() && Some(index) == state.hovered_link;

            if span.highlight.is_none()
                && !span.underline
                && !span.strikethrough
                && !is_hovered_link
            {
                continue;
            }

            let translation = layout.position() - Point::ORIGIN;
            let regions = state.paragraph.span_bounds(index);

            if let Some(highlight) = span.highlight {
                for bounds in &regions {
                    let bounds = Rectangle::new(
                        bounds.position() - Vector::new(span.padding.left, span.padding.top),
                        bounds.size() + Size::new(span.padding.x(), span.padding.y()),
                    );

                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: bounds + translation,
                            border: highlight.border,
                            ..Default::default()
                        },
                        highlight.background,
                    );
                }
            }

            if !span.underline && !span.strikethrough && !is_hovered_link {
                continue;
            }

            let size = span.size.or(self.size).unwrap_or(renderer.default_size());
            let line_height = span
                .line_height
                .unwrap_or(self.line_height)
                .to_absolute(size);
            let color = span.color.or(style.color).unwrap_or(defaults.text_color);
            let baseline = translation + Vector::new(0.0, size.0 + (line_height.0 - size.0) / 2.0);

            if span.underline || is_hovered_link {
                for bounds in &regions {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: Rectangle::new(
                                bounds.position() + baseline - Vector::new(0.0, size.0 * 0.08),
                                Size::new(bounds.width, 1.0),
                            ),
                            ..Default::default()
                        },
                        color,
                    );
                }
            }

            if span.strikethrough {
                for bounds in &regions {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: Rectangle::new(
                                bounds.position() + baseline - Vector::new(0.0, size.0 / 2.0),
                                Size::new(bounds.width, 1.0),
                            ),
                            ..Default::default()
                        },
                        color,
                    );
                }
            }
        }

        text::draw(
            renderer,
            defaults,
            layout.bounds(),
            &state.paragraph,
            style,
            viewport,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let Some(on_link_clicked) = &self.on_link_click else {
            return;
        };

        let state = tree
            .state
            .downcast_mut::<State<Link, Renderer::Paragraph>>();
        let previous_hover = state.hovered_link;

        state.hovered_link = cursor.position_in(layout.bounds()).and_then(|position| {
            let span = state.paragraph.hit_span(position)?;
            self.spans
                .as_ref()
                .as_ref()
                .get(span)?
                .link
                .as_ref()
                .map(|_| span)
        });

        if previous_hover != state.hovered_link {
            shell.request_redraw();
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if state.hovered_link.is_some() => {
                    state.span_pressed = state.hovered_link;
                    shell.capture_event();
                }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.span_pressed == state.hovered_link
                    && let Some(link) = state
                        .span_pressed
                        .and_then(|span| self.spans.as_ref().as_ref().get(span))
                        .and_then(|span| span.link.clone())
                    {
                        shell.publish(on_link_clicked(link));
                    }

                state.span_pressed = None;
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree
            .state
            .downcast_ref::<State<Link, Renderer::Paragraph>>();

        if state.hovered_link.is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn layout<Link, Renderer>(
    state: &mut State<Link, Renderer::Paragraph>,
    renderer: &Renderer,
    limits: &layout::Limits,
    width: Length,
    height: Length,
    spans: &[Span<'_, Link, Renderer::Font>],
    line_height: LineHeight,
    size: Option<Pixels>,
    font: Option<Renderer::Font>,
    align_x: Alignment,
    align_y: alignment::Vertical,
    wrapping: Wrapping,
    ellipsis: Ellipsis,
) -> layout::Node
where
    Link: Clone,
    Renderer: core::text::Renderer,
{
    layout::sized(limits, width, height, |limits| {
        let bounds = limits.max();
        let size = size.unwrap_or_else(|| renderer.default_size());
        let font = font.unwrap_or_else(|| renderer.default_font());

        let text_with_spans = || core::Text {
            content: spans,
            bounds,
            size,
            line_height,
            font,
            align_x,
            align_y,
            shaping: Shaping::Advanced,
            wrapping,
            ellipsis,
            hint_factor: renderer.scale_factor(),
        };

        if state.spans != spans {
            state.paragraph = Renderer::Paragraph::with_spans(text_with_spans());
            state.spans.clear();
            state
                .spans
                .extend(spans.iter().cloned().map(Span::to_static));
        } else {
            match state.paragraph.compare(core::Text {
                content: (),
                bounds,
                size,
                line_height,
                font,
                align_x,
                align_y,
                shaping: Shaping::Advanced,
                wrapping,
                ellipsis,
                hint_factor: renderer.scale_factor(),
            }) {
                core::text::Difference::None => {}
                core::text::Difference::Bounds => state.paragraph.resize(bounds),
                core::text::Difference::Shape => {
                    state.paragraph = Renderer::Paragraph::with_spans(text_with_spans());
                }
            }
        }

        state.paragraph.min_bounds()
    })
}

impl<'a, Link, Message, Theme, Renderer> From<Rich<'a, Link, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Link: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: core::text::Renderer + 'a,
{
    fn from(text: Rich<'a, Link, Message, Theme, Renderer>) -> Self {
        Self::new(text)
    }
}
