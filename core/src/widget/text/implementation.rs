use crate::layout;
use crate::mouse;
use crate::renderer;
use crate::semantics;
use crate::text;
use crate::text::paragraph::{self, Paragraph};
use crate::widget::tree::{self, Tree};
use crate::{Element, Layout, Length, Rectangle, Size, Widget};

use super::{Catalog, Format, State, Style, Text};

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Text<'_, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::<Renderer::Paragraph>::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.format.width, self.format.height)
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<semantics::Node> {
        if self.fragment.is_empty() {
            return None;
        }

        Some(
            semantics::Node::new(
                tree.id().clone(),
                layout.bounds(),
                semantics::Role::StaticText,
            )
            .with_name(self.fragment.as_ref()),
        )
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout(
            tree.state.downcast_mut::<State<Renderer::Paragraph>>(),
            renderer,
            limits,
            &self.fragment,
            self.format,
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
        let bounds = layout.bounds();
        if self.fragment.is_empty() || !bounds.intersects(viewport) {
            return;
        }

        draw(
            renderer,
            defaults,
            bounds,
            tree.state
                .downcast_ref::<State<Renderer::Paragraph>>()
                .raw(),
            theme.style(&self.class),
            viewport,
        );
    }

    fn operate(
        &mut self,
        _tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn super::super::Operation,
    ) {
        operation.text(None, layout.bounds(), &self.fragment);
    }
}

/// Shapes and measures a plain text paragraph.
pub fn layout<Renderer>(
    paragraph: &mut paragraph::Plain<Renderer::Paragraph>,
    renderer: &Renderer,
    limits: &layout::Limits,
    content: &str,
    format: Format<Renderer::Font>,
) -> layout::Node
where
    Renderer: text::Renderer,
{
    layout::sized(limits, format.width, format.height, |limits| {
        let _ = paragraph.update(text::Text {
            content,
            bounds: limits.max(),
            size: format.size.unwrap_or_else(|| renderer.default_size()),
            line_height: format.line_height,
            font: format.font.unwrap_or_else(|| renderer.default_font()),
            align_x: format.align_x,
            align_y: format.align_y,
            shaping: format.shaping,
            wrapping: format.wrapping,
            ellipsis: format.ellipsis,
            hint_factor: renderer.scale_factor(),
        });

        paragraph.min_bounds()
    })
}

/// Draws a previously shaped paragraph.
pub fn draw<Renderer>(
    renderer: &mut Renderer,
    inherited: &renderer::Style,
    bounds: Rectangle,
    paragraph: &Renderer::Paragraph,
    appearance: Style,
    viewport: &Rectangle,
) where
    Renderer: text::Renderer,
{
    if !bounds.intersects(viewport) {
        return;
    }

    renderer.fill_paragraph(
        paragraph,
        bounds.anchor(
            paragraph.min_bounds(),
            paragraph.align_x(),
            paragraph.align_y(),
        ),
        appearance.color.unwrap_or(inherited.text_color),
        *viewport,
    );
}

impl<'a, Message, Theme, Renderer> From<Text<'a, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Theme: Catalog + 'a,
    Renderer: text::Renderer + 'a,
{
    fn from(text: Text<'a, Theme, Renderer>) -> Self {
        Element::new(text)
    }
}

impl<'a, Theme, Renderer> From<&'a str> for Text<'a, Theme, Renderer>
where
    Theme: Catalog + 'a,
    Renderer: text::Renderer,
{
    fn from(content: &'a str) -> Self {
        Self::new(content)
    }
}

impl<'a, Message, Theme, Renderer> From<&'a str> for Element<'a, Message, Theme, Renderer>
where
    Theme: Catalog + 'a,
    Renderer: text::Renderer + 'a,
{
    fn from(content: &'a str) -> Self {
        Text::from(content).into()
    }
}
