use super::{InnerBuilder, Overlay, OverlayInstance, dormant_state, trim_event_buffer};
use crate::core::layout::{self, Layout};
use crate::core::{self, Shell, Size};
use crate::core::{mouse, overlay, renderer};

use std::marker::PhantomData;

impl<Message, Theme, Renderer, Event, S> Drop
    for Overlay<'_, '_, Message, Theme, Renderer, Event, S>
{
    fn drop(&mut self) {
        if let Some(heads) = self.0.take().map(super::Inner::into_heads) {
            *heads.instance.tree.borrow().borrow_mut() = Some(heads.tree);
        }
    }
}

impl<Message, Theme, Renderer, Event, S>
    OverlayInstance<'_, '_, Message, Theme, Renderer, Event, S>
{
    fn with_overlay<T>(
        &self,
        f: impl FnOnce(&mut overlay::Nested<'_, Event, Theme, Renderer>) -> T,
    ) -> Option<T> {
        self.overlay
            .as_ref()?
            .0
            .as_ref()?
            .with_overlay(|overlay| overlay.as_ref().map(|nested| (f)(&mut nested.borrow_mut())))
    }

    fn with_overlay_mut<T>(
        &mut self,
        f: impl FnOnce(&mut overlay::Nested<'_, Event, Theme, Renderer>) -> T,
    ) -> Option<T> {
        self.overlay
            .as_mut()?
            .0
            .as_mut()?
            .with_overlay_mut(|overlay| overlay.as_mut().map(|nested| (f)(nested.get_mut())))
    }
}

impl<Message, Theme, Renderer, Event, S> overlay::Overlay<Message, Theme, Renderer>
    for OverlayInstance<'_, '_, Message, Theme, Renderer, Event, S>
where
    Renderer: core::Renderer,
    S: 'static + Default,
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
        event: &core::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        let mut local_messages = std::mem::take(&mut self.events);
        debug_assert!(local_messages.is_empty());
        let mut local_shell = shell.local(&mut local_messages);

        let _ = self.with_overlay_mut(|overlay| {
            overlay.update(event, layout, cursor, renderer, &mut local_shell);
        });

        if local_shell.is_empty() {
            shell.merge_map(local_shell, |_| None);
        } else {
            let mut inner = self.overlay.take().unwrap().0.take().unwrap().into_heads();
            let mut heads = inner.instance.state.get_mut().take().unwrap().into_heads();

            shell.merge_map(local_shell, |message| {
                heads
                    .component
                    .update(inner.tree.state.downcast_mut(), message)
            });

            *inner.instance.state.borrow_mut() = Some(dormant_state(heads.component));

            self.overlay = Some(Overlay(Some(
                InnerBuilder {
                    instance: inner.instance,
                    tree: inner.tree,
                    types: PhantomData,
                    overlay_builder: |_, _| None,
                }
                .build(),
            )));

            shell.invalidate_layout();
            shell.request_redraw();
        }

        trim_event_buffer(&mut local_messages);
        self.events = local_messages;
    }
}
