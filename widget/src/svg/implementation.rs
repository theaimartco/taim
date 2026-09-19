use crate::core::layout;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::svg;
use crate::core::widget::{Tree, tree};
use crate::core::{
    ContentFit, Element, Event, Layout, Length, Point, Rectangle, Shell, Size, Vector, Widget,
};

use super::{Catalog, Status, Svg};

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Svg<'_, Theme>
where
    Renderer: svg::Renderer,
    Theme: Catalog,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
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
        let measured = renderer.measure_svg(&self.handle);
        let image_size = Size::new(measured.width as f32, measured.height as f32);
        tree.state.downcast_mut::<State>().natural_size = image_size;

        let rotated = self.rotation.apply(image_size);
        let raw = limits.resolve(self.width, self.height, rotated);
        let fitted = self.content_fit.fit(rotated, raw);

        layout::Node::new(Size {
            width: if self.width == Length::Shrink {
                raw.width.min(fitted.width)
            } else {
                raw.width
            },
            height: if self.height == Length::Shrink {
                raw.height.min(fitted.height)
            } else {
                raw.height
            },
        })
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        _event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let current = if cursor.is_over(layout.bounds()) {
            Status::Hovered
        } else {
            Status::Idle
        };
        let state = tree.state.downcast_mut::<State>();

        if state.status != current {
            state.status = current;
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        if self.opacity <= 0.0
            || bounds.width <= 0.0
            || bounds.height <= 0.0
            || !bounds.intersects(viewport)
        {
            return;
        }

        let state = tree.state.downcast_ref::<State>();
        let image_size = state.natural_size;
        let rotated = self.rotation.apply(image_size);

        if rotated.width <= 0.0 || rotated.height <= 0.0 {
            return;
        }

        let fitted = self.content_fit.fit(rotated, bounds.size());
        let scale = Vector::new(fitted.width / rotated.width, fitted.height / rotated.height);
        let final_size = image_size * scale;
        let position = match self.content_fit {
            ContentFit::None => Point::new(
                bounds.x + (rotated.width - fitted.width) / 2.0,
                bounds.y + (rotated.height - fitted.height) / 2.0,
            ),
            _ => Point::new(
                bounds.center_x() - final_size.width / 2.0,
                bounds.center_y() - final_size.height / 2.0,
            ),
        };
        let style = theme.style(&self.class, state.status);

        renderer.draw_svg(
            svg::Svg {
                handle: self.handle.clone(),
                color: style.color,
                rotation: self.rotation.radians(),
                opacity: self.opacity,
            },
            Rectangle::new(position, final_size),
            bounds,
        );
    }
}

#[derive(Debug, Clone, Copy)]
struct State {
    status: Status,
    natural_size: Size<f32>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            status: Status::Idle,
            natural_size: Size::ZERO,
        }
    }
}

impl<'a, Message, Theme, Renderer> From<Svg<'a, Theme>> for Element<'a, Message, Theme, Renderer>
where
    Theme: Catalog + 'a,
    Renderer: svg::Renderer + 'a,
{
    fn from(svg: Svg<'a, Theme>) -> Self {
        Element::new(svg)
    }
}
