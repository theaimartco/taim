use crate::core::layout;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::widget::Tree;
use crate::core::{Element, Layout, Length, Rectangle, Size, Widget};

use super::{Catalog, Rule};

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Rule<'_, Theme>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    fn size(&self) -> Size<Length> {
        if self.is_vertical {
            Size::new(self.thickness, Length::Fill)
        } else {
            Size::new(Length::Fill, self.thickness)
        }
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let size = <Self as Widget<(), Theme, Renderer>>::size(self);
        layout::atomic(limits, size.width, size.height)
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
        let container = layout.bounds();

        if container.width <= 0.0 || container.height <= 0.0 || !container.intersects(viewport) {
            return;
        }

        let style = theme.style(&self.class);
        let mut bounds = if self.is_vertical {
            let (offset, extent) = style.fill_mode.fill(container.height);
            Rectangle {
                y: container.y + offset,
                height: extent,
                ..container
            }
        } else {
            let (offset, extent) = style.fill_mode.fill(container.width);
            Rectangle {
                x: container.x + offset,
                width: extent,
                ..container
            }
        };

        if style.snap {
            let scale = renderer.scale_factor().unwrap_or(1.0);
            let unit = if scale.is_finite() && scale > 0.0 {
                scale.recip()
            } else {
                1.0
            };

            bounds.width = bounds.width.max(unit);
            bounds.height = bounds.height.max(unit);
        }

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: crate::core::border::rounded(style.radius),
                snap: style.snap,
                ..renderer::Quad::default()
            },
            style.color,
        );
    }
}

impl<'a, Message, Theme, Renderer> From<Rule<'a, Theme>> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(rule: Rule<'a, Theme>) -> Self {
        Element::new(rule)
    }
}
