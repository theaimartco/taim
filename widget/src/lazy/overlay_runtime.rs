use super::Overlay;

use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::{self, Event, Shell, Size};

impl<Message, Theme, Renderer> Drop for Overlay<'_, Message, Theme, Renderer> {
    fn drop(&mut self) {
        let heads = self.0.take().unwrap().into_heads();
        *heads.cell.borrow_mut() = Some(heads.element);
    }
}

impl<Message, Theme, Renderer> Overlay<'_, Message, Theme, Renderer> {
    fn with_overlay<T>(
        &self,
        f: impl FnOnce(&mut overlay::Nested<'_, Message, Theme, Renderer>) -> T,
    ) -> Option<T> {
        self.0
            .as_ref()
            .unwrap()
            .with_overlay(|overlay| overlay.as_ref().map(|nested| f(&mut nested.borrow_mut())))
    }

    fn with_overlay_mut<T>(
        &mut self,
        f: impl FnOnce(&mut overlay::Nested<'_, Message, Theme, Renderer>) -> T,
    ) -> Option<T> {
        self.0
            .as_mut()
            .unwrap()
            .with_overlay_mut(|overlay| overlay.as_mut().map(|nested| f(nested.get_mut())))
    }
}

impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for Overlay<'_, Message, Theme, Renderer>
where
    Renderer: core::Renderer,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        self.with_overlay(|overlay| overlay.layout(renderer, bounds))
            .unwrap_or_default()
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let _ = self.with_overlay(|overlay| {
            overlay.draw(renderer, theme, style, layout, cursor);
        });
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.with_overlay(|overlay| overlay.mouse_interaction(layout, cursor, renderer))
            .unwrap_or_default()
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        let _ = self.with_overlay_mut(|overlay| {
            overlay.update(event, layout, cursor, renderer, shell);
        });
    }
}
