use crate::core::image;
use crate::core::layout;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::widget::{Tree, tree};
use crate::core::{Element, Layout, Length, Rectangle, Size, Widget};

use super::{Image, geometry};

impl<Message, Theme, Renderer, Handle> Widget<Message, Theme, Renderer> for Image<Handle>
where
    Renderer: image::Renderer<Handle = Handle>,
    Handle: Clone,
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
        let size = renderer.measure_image(&self.handle).unwrap_or_default();
        tree.state.downcast_mut::<State>().natural_size = size;

        geometry::layout_with_size(
            limits,
            size,
            self.width,
            self.height,
            self.crop,
            self.content_fit,
            self.rotation,
            self.expand,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        if self.opacity <= 0.0 || !bounds.intersects(viewport) {
            return;
        }

        geometry::draw_with_size(
            renderer,
            bounds,
            &self.handle,
            tree.state.downcast_ref::<State>().natural_size,
            self.crop,
            self.border_radius,
            self.content_fit,
            self.filter_method,
            self.rotation,
            self.opacity,
            self.scale,
        );
    }
}

#[derive(Debug, Default)]
struct State {
    natural_size: Size<u32>,
}

impl<'a, Message, Theme, Renderer, Handle> From<Image<Handle>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: image::Renderer<Handle = Handle>,
    Handle: Clone + 'a,
{
    fn from(image: Image<Handle>) -> Self {
        Element::new(image)
    }
}
