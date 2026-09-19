use super::{
    InnerBuilder, Instance, Overlay, OverlayInstance, Tag, dormant_state, trim_event_buffer,
};

use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::visit;
use crate::core::{self, Length, Rectangle, Shell, Size, Vector, Widget};

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

impl<Message, Theme, Renderer, Event, S> Widget<Message, Theme, Renderer>
    for Instance<'_, Message, Theme, Renderer, Event, S>
where
    S: 'static + Default,
    Renderer: core::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Tag<S>>()
    }

    fn state(&self) -> tree::State {
        let state = Rc::new(RefCell::new(Some(Tree::with_state(
            tree::Tag::of::<Tag<S>>(),
            tree::State::new(S::default()),
            vec![Tree::empty()],
        ))));

        *self.tree.borrow_mut() = Rc::clone(&state);
        self.diff_self();

        tree::State::new(state)
    }

    fn diff(&mut self, tree: &mut Tree) {
        let retained = tree.state.downcast_ref::<Rc<RefCell<Option<Tree>>>>();
        *self.tree.borrow_mut() = Rc::clone(retained);
        self.rebuild_element_if_necessary();
    }

    fn size(&self) -> Size<Length> {
        self.with_element(|element| element.as_widget().size())
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let retained = tree.state.downcast_mut::<Rc<RefCell<Option<Tree>>>>();

        self.with_element_mut(|element| {
            element.as_widget_mut().layout(
                &mut retained.borrow_mut().as_mut().unwrap().children[0],
                renderer,
                limits,
            )
        })
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &core::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let mut local_messages = std::mem::take(&mut self.events);
        debug_assert!(local_messages.is_empty());
        let mut local_shell = shell.local(&mut local_messages);
        let retained = tree.state.downcast_mut::<Rc<RefCell<Option<Tree>>>>();

        self.with_element_mut(|element| {
            element.as_widget_mut().update(
                &mut retained.borrow_mut().as_mut().unwrap().children[0],
                event,
                layout,
                cursor,
                renderer,
                &mut local_shell,
                viewport,
            );
        });

        if local_shell.is_empty() {
            shell.merge_map(local_shell, |_| None);
        } else {
            let mut heads = self.state.get_mut().take().unwrap().into_heads();
            let mut tree = retained.borrow_mut();
            let component_state = tree.as_mut().unwrap().state.downcast_mut();

            shell.merge_map(local_shell, |message| {
                heads.component.update(component_state, message)
            });
            drop(tree);

            *self.state.get_mut() = Some(dormant_state(heads.component));

            shell.invalidate_layout();
            shell.request_redraw();
        }

        trim_event_buffer(&mut local_messages);
        self.events = local_messages;
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        let retained = tree.state.downcast_mut::<Rc<RefCell<Option<Tree>>>>();

        self.with_element_mut(|element| {
            visit(&mut visit::Widgets::new(
                element.as_widget_mut(),
                &mut retained.borrow_mut().as_mut().unwrap().children[0],
                layout,
            ));
        });
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.rebuild_element_with_operation(layout, operation);

        let retained = tree.state.downcast_mut::<Rc<RefCell<Option<Tree>>>>();
        self.with_element_mut(|element| {
            element.as_widget_mut().operate(
                &mut retained.borrow_mut().as_mut().unwrap().children[0],
                layout,
                renderer,
                operation,
            );
        });
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let retained = tree.state.downcast_ref::<Rc<RefCell<Option<Tree>>>>();

        self.with_element(|element| {
            let retained = retained.borrow();
            let child = &retained.as_ref().unwrap().children[0];
            let bounds = element.as_widget().paint_bounds(child, layout);

            if bounds.width <= 0.0 || bounds.height <= 0.0 || !bounds.intersects(viewport) {
                return;
            }

            element
                .as_widget()
                .draw(child, renderer, theme, style, layout, cursor, viewport);
        });
    }

    fn paint_bounds(&self, tree: &Tree, layout: Layout<'_>) -> Rectangle {
        let retained = tree.state.downcast_ref::<Rc<RefCell<Option<Tree>>>>();

        self.with_element(|element| {
            let retained = retained.borrow();
            let child = &retained.as_ref().unwrap().children[0];
            element.as_widget().paint_bounds(child, layout)
        })
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let retained = tree.state.downcast_ref::<Rc<RefCell<Option<Tree>>>>();
        self.with_element(|element| {
            let retained = retained.borrow();
            element.as_widget().mouse_interaction(
                &retained.as_ref().unwrap().children[0],
                layout,
                cursor,
                viewport,
                renderer,
            )
        })
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.rebuild_element_if_necessary();

        let state = tree.state.downcast_mut::<Rc<RefCell<Option<Tree>>>>();
        let tree = state.borrow_mut().take().unwrap();

        let overlay = InnerBuilder {
            instance: self,
            tree,
            types: PhantomData,
            overlay_builder: |instance, tree| {
                instance
                    .state
                    .get_mut()
                    .as_mut()
                    .unwrap()
                    .with_element_mut(move |element| {
                        element
                            .as_mut()
                            .unwrap()
                            .as_widget_mut()
                            .overlay(
                                &mut tree.children[0],
                                layout,
                                renderer,
                                viewport,
                                translation,
                            )
                            .map(|overlay| RefCell::new(overlay::Nested::new(overlay)))
                    })
            },
        }
        .build();

        #[allow(clippy::redundant_closure_for_method_calls)]
        if overlay.with_overlay(|overlay| overlay.is_some()) {
            Some(overlay::Element::new(Box::new(OverlayInstance {
                overlay: Some(Overlay(Some(overlay))),
                events: Vec::new(),
            })))
        } else {
            let heads = overlay.into_heads();
            *state.borrow_mut() = Some(heads.tree);

            None
        }
    }
}
