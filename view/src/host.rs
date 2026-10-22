//! Runs a [`View`] tree inside the current widget runtime.
//!
//! This is the migration bridge described in `docs/architecture/taim-api.md`
//! (phase 1): a [`Host`] is one inherited-runtime widget whose whole content
//! is a TAIM view tree. It goes away when the runtime drives views directly.
use crate::cx::{DEFAULT_CURSOR, Focus, MeasureCx, Requests};
use crate::layout::Constraints;
use crate::state::{Invalidation, Scope};
use crate::tree;
use crate::{AnyView, Child, Event, Handled, Point, Rectangle, Renderer, Size, Theme};
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
    focus: Focus,
    /// Theme name and scale the runtime last applied.
    applied: Option<(String, f32)>,
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
            focus: Focus::default(),
            applied: None,
            id: widget::Id::unique(),
        }
    }
}

impl Root {
    /// The theme the application chose, if it chose one.
    pub fn theme(&self) -> Option<Theme> {
        self.scope().settings.theme.as_ref().map(crate::Prop::get)
    }

    /// The window zoom the application chose (1.0 by default).
    pub fn scale(&self) -> f32 {
        self.scope()
            .settings
            .scale
            .as_ref()
            .map_or(1.0, crate::Prop::get)
    }

    fn scope(&self) -> &Scope {
        &self._scope
    }

    /// Whether theme or scale changed since the runtime last applied them.
    fn settings_changed(&mut self) -> bool {
        use taim_core::theme::Base as _;

        if let Some(reduced) = &self.scope().settings.reduced_motion {
            crate::state::set_motion_reduced(reduced.get());
        }

        let current = (
            self.theme()
                .map(|theme| theme.name().to_owned())
                .unwrap_or_default(),
            self.scale(),
        );
        let changed = self.applied.as_ref() != Some(&current);
        self.applied = Some(current);
        changed
    }
}

impl std::fmt::Debug for Root {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Root").finish_non_exhaustive()
    }
}

/// A described view, handed to the runtime's semantic walk.
struct Described(semantics::Node);

impl widget::visit::Visit for Described {
    fn semantics(&mut self) -> Option<semantics::Node> {
        let mut own = self.0.clone();
        own.children.clear();
        Some(own)
    }

    fn contains(&self, point: Point) -> bool {
        self.0.bounds.contains(point)
    }

    fn bounds(&self) -> Rectangle {
        self.0.bounds
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut dyn widget::visit::Visit)) {
        for child in &self.0.children {
            visit(&mut Described(child.clone()));
        }
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

impl<'a> From<Host<'a>> for Element<'a, (), Theme, Renderer> {
    fn from(host: Host<'a>) -> Self {
        Element::new(host)
    }
}

impl Widget<(), Theme, Renderer> for Host<'_> {
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
        let mut painter = tree::Painter {
            renderer,
            theme: &root.theme,
            pointer: cursor.position(),
            text_color: style.text_color,
            focus: root.focus.current.as_deref(),
            now: crate::Instant::now(),
        };

        tree::paint(
            &mut root.child,
            layout.position(),
            Some(Rectangle::INFINITE),
            &mut Vec::new(),
            &mut painter,
        );
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, ()>,
        _viewport: &Rectangle,
    ) {
        let mut root = self.root.borrow_mut();
        let root = &mut *root;

        if !root.waker_installed {
            let waker = shell.waker().clone();
            root.invalidation.set_waker(move || waker.wake());
            root.waker_installed = true;
        }

        let now = match event {
            Event::Window(taim_core::window::Event::RedrawRequested(at)) => *at,
            _ => crate::Instant::now(),
        };

        let mut requests = Requests::default();
        let origin = layout.position();
        let everything = Some(Rectangle::INFINITE);
        root.focus.taken = false;

        let handled = {
            let mut router = tree::Router {
                now,
                pointer: cursor.position(),
                requests: &mut requests,
                focus: &mut root.focus,
            };

            if tree::is_focused_input(event) {
                match router.focus.current.clone() {
                    Some(target) => tree::deliver(
                        &mut root.child,
                        origin,
                        everything,
                        &mut Vec::new(),
                        &target,
                        event,
                        &mut router,
                    ),
                    None => Handled::No,
                }
            } else {
                tree::broadcast(
                    &mut root.child,
                    origin,
                    everything,
                    &mut Vec::new(),
                    event,
                    &mut router,
                )
            }
        };

        // Clicking anything that does not take focus clears it.
        if tree::is_press(event) && !root.focus.taken && root.focus.current.take().is_some() {
            requests.paint = true;
        }

        // Tab moves focus when the focused view did not use it.
        if handled == Handled::No
            && let Some(backwards) = tree::tab_press(event)
        {
            let order = tree::focusables(&mut root.child);
            root.focus.current = tree::tab(&order, root.focus.current.as_deref(), backwards);
            requests.paint = true;
            shell.capture_event();
        }

        if handled == Handled::Yes {
            shell.capture_event();
        }

        if let Some(text) = requests.copy.take() {
            shell.write_clipboard(taim_core::clipboard::Content::Text(text));
        }
        if requests.paste {
            shell.read_clipboard(taim_core::clipboard::Kind::Text);
        }
        // Each frame, keep frames coming while anything is still moving.
        if matches!(
            event,
            Event::Window(taim_core::window::Event::RedrawRequested(_))
        ) && tree::any_animating(&mut root.child, now)
        {
            requests.animate = true;
        }

        if requests.animate {
            shell.request_animation();
            shell.request_redraw();
        }

        // Theme and zoom are applied by the runtime after a message; send
        // an empty one when they change.
        if root.settings_changed() {
            shell.publish(());
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
        tree::cursor(&mut root.child, layout.position(), pointer).unwrap_or(DEFAULT_CURSOR)
    }

    fn semantics(&self, _tree: &Tree, layout: Layout<'_>) -> Option<semantics::Node> {
        // The window's own node; the views are published through
        // `visit_children`, which the runtime walks on every frame.
        let root = self.root.borrow();
        Some(semantics::Node::new(
            root.id.clone(),
            layout.bounds(),
            semantics::Role::Group,
        ))
    }

    fn visit_children(
        &mut self,
        _tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn widget::visit::Visit),
    ) {
        let nodes = {
            let mut root = self.root.borrow_mut();
            let root = &mut *root;
            let mut nodes = Vec::new();
            tree::describe(
                &mut root.child,
                layout.position(),
                &mut Vec::new(),
                root.focus.current.as_deref(),
                &mut nodes,
            );
            nodes
        };

        for node in nodes {
            visit(&mut Described(node));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views;
    use crate::views::{button, column, row, text};
    use crate::{View, mouse};

    fn frames(root: &mut Root) -> Vec<Rectangle> {
        let mut frames = Vec::new();
        tree::walk(
            &mut root.child,
            Point::ORIGIN,
            &mut Vec::new(),
            &mut |_, bounds, _| {
                frames.push(bounds);
            },
        );
        frames
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

    #[test]
    fn roots_build_once_and_own_their_state() {
        let mut root = Root::new(|cx| {
            let count = cx.signal(0);
            column(views![
                text(move || format!("{}", count.get())),
                row(views![
                    button("+").on_press(move || count.update(|n| *n += 1))
                ]),
            ])
        });

        // column, text, row, button, button label
        assert_eq!(frames(&mut root).len(), 5);
    }

    #[test]
    fn presses_reach_only_the_button_under_the_pointer() {
        let mut root = Root::new(|cx| {
            let count = cx.signal(0);
            column(views![
                text(move || format!("{}", count.get())),
                button("+").on_press(move || count.update(|n| *n += 1)),
            ])
        });

        set_sizes(&mut root.child, Size::new(100.0, 30.0));
        let mut index = 0;
        root.child.view_mut().children(&mut |child| {
            if index == 1 {
                child.place(Point::new(0.0, 40.0));
            }
            index += 1;
        });

        let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        let release = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
        let mut requests = Requests::default();
        let mut focus = Focus::default();

        let mut route = |event: &Event, at: Point, root: &mut Root| {
            tree::broadcast(
                &mut root.child,
                Point::ORIGIN,
                Some(Rectangle::INFINITE),
                &mut Vec::new(),
                event,
                &mut tree::Router {
                    now: crate::Instant::now(),
                    pointer: Some(at),
                    requests: &mut requests,
                    focus: &mut focus,
                },
            )
        };

        assert_eq!(
            route(&press, Point::new(10.0, 50.0), &mut root),
            Handled::Yes
        );
        assert_eq!(
            route(&release, Point::new(10.0, 50.0), &mut root),
            Handled::Yes
        );
        assert_eq!(route(&press, Point::new(10.0, 5.0), &mut root), Handled::No);
    }

    #[test]
    fn scroll_offsets_shift_children() {
        struct Scrolled(Child);

        impl View for Scrolled {
            fn measure(&mut self, _: &mut crate::MeasureCx<'_>, _: Constraints) -> Size {
                Size::ZERO
            }
            fn paint(&self, _: &mut crate::PaintCx<'_>) {}
            fn describe(&self, _: &crate::DescribeCx<'_>) -> Option<semantics::Node> {
                None
            }
            fn scroll_offset(&self) -> crate::Vector {
                crate::Vector::new(0.0, 25.0)
            }
            fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
                visit(&mut self.0);
            }
        }

        let mut root = Root::new(|_| Scrolled(Child::new(text("x"))));
        root.child
            .view_mut()
            .children(&mut |child| child.place(Point::new(0.0, 100.0)));

        assert_eq!(frames(&mut root)[1].y, 75.0);
    }
}
