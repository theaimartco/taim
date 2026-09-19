use super::{InnerBuilder, Internal, Lazy, Overlay};

use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::visit;
use crate::core::widget::{self, Widget};
use crate::core::{self, Element, Event, Length, Rectangle, Shell, Size, Vector};

use rustc_hash::FxHasher;
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

impl<'a, Message, Theme, Renderer, Dependency, View> Widget<Message, Theme, Renderer>
    for Lazy<'a, Message, Theme, Renderer, Dependency, View>
where
    View: Into<Element<'static, Message, Theme, Renderer>> + 'static,
    Dependency: Hash + 'a,
    Message: 'static,
    Theme: 'static,
    Renderer: core::Renderer + 'static,
{
    fn tag(&self) -> tree::Tag {
        struct Tag<T>(T);
        tree::Tag::of::<Tag<View>>()
    }

    fn state(&self) -> tree::State {
        let hash = dependency_hash(&self.dependency);
        let element = Rc::new(RefCell::new(Some((self.view)(&self.dependency).into())));

        let _ = self.element.replace(Some(element.clone()));

        tree::State::new(Internal {
            element,
            hash,
            layer_key: None,
        })
    }

    fn diff(&mut self, tree: &mut Tree) {
        let new_hash = dependency_hash(&self.dependency);
        let (element, changed) = {
            let current = tree
                .state
                .downcast_mut::<Internal<Message, Theme, Renderer>>();
            let changed = current.hash != new_hash;

            if changed {
                current.hash = new_hash;
                current.element = Rc::new(RefCell::new(Some((self.view)(&self.dependency).into())));
            }

            (current.element.clone(), changed)
        };

        let _ = self.element.replace(Some(element));

        // A stable dependency retains the exact same element and tree. Avoid
        // walking and hashing that subtree again; the sole exception is its
        // first reconciliation, when `Tree::new` has not created children yet.
        if changed || tree.children.is_empty() {
            self.with_element_mut(|element| {
                tree.diff_children(std::slice::from_mut(&mut element.as_widget_mut()));
            });

            let layer_key = retained_key(&tree.children[0]);
            tree.state
                .downcast_mut::<Internal<Message, Theme, Renderer>>()
                .layer_key = Some(layer_key);
        }

        if changed {
            tree.mark_dirty(
                tree::DirtyFlags::LAYOUT | tree::DirtyFlags::PAINT | tree::DirtyFlags::SEMANTICS,
            );
        }

        if tree.dirty().contains(tree::DirtyFlags::LAYOUT) {
            for child in &mut tree.children {
                child.mark_dirty(tree::DirtyFlags::LAYOUT);
            }
        }
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
        self.ensure_element(tree);
        let node = self.with_element_mut(|element| {
            tree.children[0].layout_with_cache(*limits, |child| {
                element.as_widget_mut().layout(child, renderer, limits)
            })
        });
        tree.clear_dirty(tree::DirtyFlags::LAYOUT);
        node
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        self.ensure_element(tree);
        self.with_element_mut(|element| {
            visit(&mut visit::Widgets::new(
                element.as_widget_mut(),
                &mut tree.children[0],
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
        self.ensure_element(tree);
        self.with_element_mut(|element| {
            element
                .as_widget_mut()
                .operate(&mut tree.children[0], layout, renderer, operation);
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.ensure_element(tree);
        self.with_element_mut(|element| {
            element.as_widget_mut().update(
                &mut tree.children[0],
                event,
                layout,
                cursor,
                renderer,
                shell,
                viewport,
            );
        });

        if shell.is_layout_invalid().is_some() {
            tree.children[0].mark_dirty(tree::DirtyFlags::LAYOUT);
            tree.mark_dirty(tree::DirtyFlags::LAYOUT | tree::DirtyFlags::PAINT);
        }

        if shell.are_widgets_invalid() {
            tree.children[0].mark_dirty(tree::DirtyFlags::all());
            tree.mark_dirty(tree::DirtyFlags::all());
        }

        if shell.is_semantics_invalid() {
            tree.children[0].mark_dirty(tree::DirtyFlags::SEMANTICS);
            tree.mark_dirty(tree::DirtyFlags::SEMANTICS);
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.ensure_element(tree);
        self.with_element(|element| {
            element.as_widget().mouse_interaction(
                &tree.children[0],
                layout,
                cursor,
                viewport,
                renderer,
            )
        })
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
        self.ensure_element(tree);
        self.with_element(|element| {
            let child = &tree.children[0];
            let bounds = element.as_widget().paint_bounds(child, layout);

            if bounds.width <= 0.0 || bounds.height <= 0.0 || !bounds.intersects(viewport) {
                return;
            }

            let key = tree
                .state
                .downcast_ref::<Internal<Message, Theme, Renderer>>()
                .layer_key
                .unwrap_or_else(|| retained_key(child));
            let revision = child.revision().value();

            if !renderer.replay_retained_layer(key, revision, bounds) {
                renderer.start_retained_layer(key, revision, bounds);
                element
                    .as_widget()
                    .draw(child, renderer, theme, style, layout, cursor, viewport);
                renderer.end_retained_layer();
            }
        });
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.ensure_element(tree);

        let cell = self.element.borrow().as_ref().unwrap().clone();
        let element = cell.borrow_mut().take().unwrap();
        let overlay = InnerBuilder {
            cell,
            element,
            tree: &mut tree.children[0],
            layout,
            overlay_builder: |element, tree, layout| {
                element
                    .as_widget_mut()
                    .overlay(tree, *layout, renderer, viewport, translation)
                    .map(|overlay| RefCell::new(overlay::Nested::new(overlay)))
            },
        }
        .build();

        #[allow(clippy::redundant_closure_for_method_calls)]
        if overlay.with_overlay(|overlay| overlay.is_some()) {
            Some(overlay::Element::new(Box::new(Overlay(Some(overlay)))))
        } else {
            let heads = overlay.into_heads();
            *heads.cell.borrow_mut() = Some(heads.element);

            None
        }
    }
}

#[inline]
fn dependency_hash(dependency: &impl Hash) -> u64 {
    let mut hasher = FxHasher::default();
    dependency.hash(&mut hasher);
    hasher.finish()
}

#[inline]
fn retained_key(tree: &Tree) -> u64 {
    let mut hasher = FxHasher::default();
    tree.id().hash(&mut hasher);
    hasher.finish()
}
