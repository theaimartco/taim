//! Runs a [`View`] tree inside the current widget runtime.
//!
//! This is the migration bridge described in `docs/architecture/taim-api.md`
//! (phase 1): a [`Host`] is one inherited-runtime widget whose whole content
//! is a TAIM view tree. It goes away when the runtime drives views directly.
use crate::cx::{DEFAULT_CURSOR, DescribeCx, EventCx, MeasureCx, PaintCx, Requests};
use crate::layout::Constraints;
use crate::state::{Invalidation, Scope};
use crate::{AnyView, Child, Event, Handled, Point, Rectangle, Renderer, Size, Theme, View};
use crate::{mouse, semantics};

use taim_core::layout::{self, Layout};
use taim_core::renderer;
use taim_core::widget::{self, Tree};
use taim_core::{Element, Length, Shell, Widget};

use std::cell::RefCell;
use std::sync::Arc;

/// A window's view tree and the state it owns.
pub struct Root {
    child: Child,
    // Dropped after `child`, so views never outlive the state they read.
    _scope: Scope,
    invalidation: Arc<Invalidation>,
    theme: Theme,
    waker_installed: bool,
    id: widget::Id,
}

impl Root {
    /// Builds the tree by calling `build` once with a fresh scope.
    pub fn new<V: Into<AnyView>>(build: impl FnOnce(&mut Scope) -> V) -> Self {
        let invalidation = Arc::new(Invalidation::default());
        let mut scope = Scope::new(invalidation.clone());
        let view = scope.build(|scope| build(scope).into());

        Self {
            child: Child::new(view),
            _scope: scope,
            invalidation,
            theme: Theme::Light,
            waker_installed: false,
            id: widget::Id::unique(),
        }
    }
}

impl std::fmt::Debug for Root {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Root").finish_non_exhaustive()
    }
}

/// The inherited-runtime widget that shows a [`Root`].
pub struct Host<'a> {
    root: &'a RefCell<Root>,
}

impl<'a> Host<'a> {
    /// Shows `root`.
    pub fn new(root: &'a RefCell<Root>) -> Self {
        Self { root }
    }
}

impl<'a, Message: 'a> From<Host<'a>> for Element<'a, Message, Theme, Renderer> {
    fn from(host: Host<'a>) -> Self {
        Element::new(host)
    }
}

/// Calls `visit` for `child` and every descendant, depth first, with each
/// view's bounds in window coordinates.
fn walk(child: &mut Child, origin: Point, visit: &mut dyn FnMut(&mut dyn View, Rectangle)) {
    let frame = child.frame();
    let bounds = Rectangle::new(
        Point::new(origin.x + frame.x, origin.y + frame.y),
        frame.size(),
    );

    visit(child.view_mut(), bounds);
    child
        .view_mut()
        .children(&mut |grandchild| walk(grandchild, bounds.position(), visit));
}

/// Offers `event` to `child`'s subtree, deepest views first; returns whether
/// someone consumed it.
fn dispatch(
    child: &mut Child,
    origin: Point,
    event: &Event,
    pointer: Option<Point>,
    requests: &mut Requests,
) -> Handled {
    let frame = child.frame();
    let bounds = Rectangle::new(
        Point::new(origin.x + frame.x, origin.y + frame.y),
        frame.size(),
    );

    let mut handled = Handled::No;
    child.view_mut().children(&mut |grandchild| {
        if handled == Handled::No {
            handled = dispatch(grandchild, bounds.position(), event, pointer, requests);
        }
    });

    if handled == Handled::Yes {
        return handled;
    }

    child.view_mut().event(
        &mut EventCx {
            bounds,
            pointer,
            requests,
        },
        event,
    )
}

/// Builds the semantic node for `child`, folding the children of views that
/// do not describe themselves into the nearest described ancestor.
fn describe(child: &mut Child, origin: Point, out: &mut Vec<semantics::Node>) {
    let frame = child.frame();
    let bounds = Rectangle::new(
        Point::new(origin.x + frame.x, origin.y + frame.y),
        frame.size(),
    );

    let own = child.view().describe(&DescribeCx {
        bounds,
        _marker: std::marker::PhantomData,
    });

    let mut nested = Vec::new();
    child
        .view_mut()
        .children(&mut |grandchild| describe(grandchild, bounds.position(), &mut nested));

    match own {
        Some(node) => out.push(nested.into_iter().fold(node, semantics::Node::with_child)),
        None => out.extend(nested),
    }
}

impl<Message> Widget<Message, Theme, Renderer> for Host<'_> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let mut root = self.root.borrow_mut();
        let root = &mut *root;
        let available = limits.max();

        let mut cx = MeasureCx {
            renderer,
            theme: &root.theme,
        };
        let _ = root.child.measure(&mut cx, Constraints::loose(available));
        root.child.place(Point::ORIGIN);

        layout::Node::new(available)
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let mut root = self.root.borrow_mut();
        if root.theme != *theme {
            root.theme = theme.clone();
        }

        let root = &mut *root;
        let pointer = cursor.position();
        let theme = &root.theme;
        walk(&mut root.child, layout.position(), &mut |view, bounds| {
            view.paint(&mut PaintCx {
                renderer: &mut *renderer,
                theme,
                bounds,
                pointer,
                text_color: style.text_color,
            });
        });
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let mut root = self.root.borrow_mut();

        if !root.waker_installed {
            let waker = shell.waker().clone();
            root.invalidation.set_waker(move || waker.wake());
            root.waker_installed = true;
        }

        let mut requests = Requests::default();
        let handled = dispatch(
            &mut root.child,
            layout.position(),
            event,
            cursor.position(),
            &mut requests,
        );

        if handled == Handled::Yes {
            shell.capture_event();
        }

        let owed = root.invalidation.take();
        if requests.layout || owed & Invalidation::LAYOUT != 0 {
            shell.invalidate_layout();
            shell.request_redraw();
        } else if requests.paint || owed & Invalidation::PAINT != 0 {
            shell.request_redraw();
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let Some(pointer) = cursor.position() else {
            return DEFAULT_CURSOR;
        };

        let mut root = self.root.borrow_mut();
        let mut shown = DEFAULT_CURSOR;

        // Depth-first order visits descendants after ancestors, so the last
        // match is the innermost view under the pointer.
        walk(&mut root.child, layout.position(), &mut |view, bounds| {
            if bounds.contains(pointer)
                && let Some(cursor) = view.cursor()
            {
                shown = cursor;
            }
        });

        shown
    }

    fn semantics(&self, _tree: &Tree, layout: Layout<'_>) -> Option<semantics::Node> {
        let mut root = self.root.borrow_mut();
        let mut nodes = Vec::new();
        describe(&mut root.child, layout.position(), &mut nodes);

        let window = semantics::Node::new(root.id.clone(), layout.bounds(), semantics::Role::Group);
        Some(nodes.into_iter().fold(window, semantics::Node::with_child))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views;
    use crate::views::{button, column, row, text};

    fn tree_shape(root: &mut Root) -> Vec<Rectangle> {
        let mut frames = Vec::new();
        walk(&mut root.child, Point::ORIGIN, &mut |_, bounds| {
            frames.push(bounds);
        });
        frames
    }

    #[test]
    fn roots_build_once_and_own_their_state() {
        let root = Root::new(|cx| {
            let count = cx.signal(0);
            column(views![
                text(move || format!("{}", count.get())),
                row(views![
                    button("+").on_press(move || count.update(|n| *n += 1))
                ]),
            ])
        });

        let mut root = root;
        // column, text, row, button, button label
        assert_eq!(tree_shape(&mut root).len(), 5);
    }

    #[test]
    fn clicking_a_button_runs_its_handler_and_invalidates_layout() {
        let mut root = Root::new(|cx| {
            let count = cx.signal(0);
            column(views![
                text(move || format!("{}", count.get())),
                button("+").on_press(move || count.update(|n| *n += 1)),
            ])
        });

        // Lay the button out by hand at a known place: 100×30 at (0, 40).
        root.child.place(Point::ORIGIN);
        let mut index = 0;
        root.child.view_mut().children(&mut |child| {
            if index == 1 {
                child.place(Point::new(0.0, 40.0));
            }
            index += 1;
        });
        set_sizes(&mut root.child, Size::new(100.0, 30.0));

        let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        let release = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
        let over_button = Some(Point::new(10.0, 50.0));

        let mut requests = Requests::default();
        assert_eq!(
            dispatch(
                &mut root.child,
                Point::ORIGIN,
                &press,
                over_button,
                &mut requests
            ),
            Handled::Yes
        );
        assert_eq!(
            dispatch(
                &mut root.child,
                Point::ORIGIN,
                &release,
                over_button,
                &mut requests
            ),
            Handled::Yes
        );
        assert!(requests.paint);

        // A press elsewhere is not the button's.
        let elsewhere = Some(Point::new(10.0, 5.0));
        assert_eq!(
            dispatch(
                &mut root.child,
                Point::ORIGIN,
                &press,
                elsewhere,
                &mut requests
            ),
            Handled::No
        );
    }

    /// Gives every view in the tree the same size, for routing tests that
    /// do not need a renderer.
    fn set_sizes(child: &mut Child, size: Size) {
        child.frame_mut().width = size.width;
        child.frame_mut().height = size.height;
        child
            .view_mut()
            .children(&mut |grandchild| set_sizes(grandchild, size));
    }
}
