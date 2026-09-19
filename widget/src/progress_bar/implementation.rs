use crate::core::layout;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::widget::Tree;
use crate::core::{Color, Element, Layout, Length, Rectangle, Size, Widget};

use super::{Catalog, ProgressBar};

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for ProgressBar<'_, Theme>
where
    Theme: Catalog,
    Renderer: crate::core::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(self.width(), self.height())
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, self.width(), self.height())
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        if bounds.width <= 0.0 || bounds.height <= 0.0 || !bounds.intersects(viewport) {
            return;
        }

        let style = theme.style(&self.class);
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: style.border,
                ..renderer::Quad::default()
            },
            style.background,
        );

        let extent = if self.is_vertical {
            bounds.height
        } else {
            bounds.width
        } * self.progress;

        if extent <= 0.0 {
            return;
        }

        let active = if self.is_vertical {
            Rectangle {
                y: bounds.y + bounds.height - extent,
                height: extent,
                ..bounds
            }
        } else {
            Rectangle {
                width: extent,
                ..bounds
            }
        };

        renderer.fill_quad(
            renderer::Quad {
                bounds: active,
                border: crate::core::border::Border {
                    color: Color::TRANSPARENT,
                    ..style.border
                },
                ..renderer::Quad::default()
            },
            style.bar,
        );
    }
}

impl<'a, Message, Theme, Renderer> From<ProgressBar<'a, Theme>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(progress_bar: ProgressBar<'a, Theme>) -> Self {
        Element::new(progress_bar)
    }
}
