//! A window's view tree, as the platform shell drives it.
//!
//! The shell owns the window, the renderer and the frame clock; a [`Root`]
//! owns everything else. Each frame the shell calls, in order:
//!
//! 1. [`Root::handle`] for every input event, collecting a [`Response`];
//! 2. [`Root::layout`] when a response, a state change or a resize asked
//!    for it;
//! 3. [`Root::paint`], then [`Root::is_animating`] to decide whether to
//!    schedule the next frame;
//! 4. [`Root::describe`] to publish the accessibility tree.
use crate::cx::{DEFAULT_CURSOR, Focus, MeasureCx, Requests};
use crate::layout::Constraints;
use crate::state::{Invalidation, Scope};
use crate::tree;
use crate::{AnyView, Child, Color, Event, Handled, Instant, Point, Rectangle, Renderer, Size};
use crate::{Theme, mouse, semantics};

use taim_core::widget;

use std::sync::Arc;

/// A window's view tree and the state it owns.
pub struct Root {
    child: Child,
    // Dropped after `child`, so views never outlive the state they read.
    scope: Scope,
    invalidation: Arc<Invalidation>,
    theme: Theme,
    focus: Focus,
    /// Theme name and scale the shell last applied.
    applied: Option<(String, f32)>,
    id: widget::Id,
    /// The focus path [`Root::wants_text_input`] last answered for.
    text_input: Option<(Option<Vec<u16>>, bool)>,
    /// Edges the platform covers (status bar, navigation bar, notch,
    /// on-screen keyboard); views are laid out inside them.
    insets: crate::Padding,
}

/// What handling one event asked of the shell.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Response {
    /// A view used the event; it should not reach anything else.
    pub captured: bool,
    /// The window must be painted again.
    pub paint: bool,
    /// Sizes may have changed; lay out before painting.
    pub layout: bool,
    /// Something started moving; keep frames coming.
    pub animate: bool,
    /// Text to put on the clipboard.
    pub copy: Option<String>,
    /// A view wants the clipboard's text, delivered back as an event.
    pub paste: bool,
}

impl Root {
    /// Builds the tree by calling `build` once with a fresh scope.
    pub fn new<V: Into<AnyView>>(build: impl FnOnce(&mut Scope) -> V) -> Self {
        let invalidation = Arc::new(Invalidation::default());
        let mut scope = Scope::new(invalidation.clone());
        let view = scope.build(|scope| build(scope).into());

        Self {
            child: Child::new(view),
            scope,
            invalidation,
            theme: Theme::Light,
            focus: Focus::default(),
            applied: None,
            text_input: None,
            insets: crate::Padding::ZERO,
            id: widget::Id::unique(),
        }
    }

    /// Calls `wake` (from any thread) whenever state the tree reads changes.
    pub fn on_change(&self, wake: impl Fn() + Send + Sync + 'static) {
        self.invalidation.set_waker(wake);
    }

    /// What state changes owe the window since the last call: `(paint,
    /// layout)`.
    pub fn take_changes(&self) -> (bool, bool) {
        let owed = self.invalidation.take();
        (
            owed & (Invalidation::PAINT | Invalidation::LAYOUT) != 0,
            owed & Invalidation::LAYOUT != 0,
        )
    }

    /// The theme the application chose, if it chose one.
    pub fn theme(&self) -> Option<Theme> {
        self.scope.settings.theme.as_ref().map(crate::Prop::get)
    }

    /// The window zoom the application chose (1.0 by default).
    pub fn scale(&self) -> f32 {
        self.scope
            .settings
            .scale
            .as_ref()
            .map_or(1.0, crate::Prop::get)
    }

    /// Applies the application's motion preference and reports whether its
    /// theme or zoom changed since the last call.
    pub fn settings_changed(&mut self) -> bool {
        use taim_core::theme::Base as _;

        if let Some(reduced) = &self.scope.settings.reduced_motion {
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

    /// Sets the theme views paint with.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    /// Sets the edges of the window the platform covers: system bars, a
    /// display cutout, the on-screen keyboard. The next [`Root::layout`]
    /// keeps every view inside them, as a safe area. Returns whether they
    /// changed.
    pub fn set_insets(&mut self, insets: crate::Padding) -> bool {
        let usable = |edge: f32| if edge.is_finite() { edge.max(0.0) } else { 0.0 };
        let insets = crate::Padding {
            top: usable(insets.top),
            right: usable(insets.right),
            bottom: usable(insets.bottom),
            left: usable(insets.left),
        };

        let changed = insets != self.insets;
        self.insets = insets;
        changed
    }

    /// The edges views are kept inside of; see [`Root::set_insets`].
    pub fn insets(&self) -> crate::Padding {
        self.insets
    }

    /// Lays the tree out to fill `size`, inside the platform's insets.
    pub fn layout(&mut self, renderer: &Renderer, size: Size) {
        // A rebuilt subtree can put a different view at the focused path.
        self.text_input = None;
        let mut cx = MeasureCx {
            renderer,
            theme: &self.theme,
        };
        let insets = self.insets;
        let inner = Size::new(
            (size.width - insets.left - insets.right).max(0.0),
            (size.height - insets.top - insets.bottom).max(0.0),
        );
        let _ = self.child.measure(&mut cx, Constraints::loose(inner));
        self.child.place(Point::new(insets.left, insets.top));
    }

    /// Paints the tree as of `now`.
    pub fn paint(
        &mut self,
        renderer: &mut Renderer,
        text_color: Color,
        pointer: Option<Point>,
        now: Instant,
    ) {
        let mut painter = tree::Painter {
            renderer,
            theme: &self.theme,
            pointer,
            text_color,
            focus: self.focus.current.as_deref(),
            now,
        };

        tree::paint(
            &mut self.child,
            Point::ORIGIN,
            Some(Rectangle::INFINITE),
            &mut Vec::new(),
            &mut painter,
        );
    }

    /// Whether anything is still moving at `now`.
    pub fn is_animating(&mut self, now: Instant) -> bool {
        tree::any_animating(&mut self.child, now)
    }

    /// Whether the focused view edits text, so the platform should show its
    /// on-screen keyboard.
    ///
    /// Cheap to ask after every event: the tree is only searched when focus
    /// has moved since the last answer.
    pub fn wants_text_input(&mut self) -> bool {
        if let Some((path, wants)) = &self.text_input
            && *path == self.focus.current
        {
            return *wants;
        }

        let wants = self.focused_takes_text();
        self.text_input = Some((self.focus.current.clone(), wants));
        wants
    }

    fn focused_takes_text(&mut self) -> bool {
        let Some(focused) = self.focus.current.clone() else {
            return false;
        };

        let mut wants = false;
        tree::walk(
            &mut self.child,
            Point::ORIGIN,
            &mut Vec::new(),
            &mut |view, _, path| {
                if path == focused.as_slice() {
                    wants = view.takes_text();
                }
            },
        );
        wants
    }

    /// Routes one event: keyboard input to the focused view, everything
    /// else to every view, topmost first.
    pub fn handle(&mut self, event: &Event, pointer: Option<Point>, now: Instant) -> Response {
        self.handle_input(event, pointer, now, false)
    }

    /// Like [`Root::handle`], saying whether the pointer is a finger. The
    /// shell turns the first finger on the screen into pointer events so
    /// every view works with touch; views that behave differently for a
    /// finger (dragging to scroll) read this through
    /// [`EventCx::is_touch`](crate::EventCx::is_touch).
    pub fn handle_input(
        &mut self,
        event: &Event,
        pointer: Option<Point>,
        now: Instant,
        touch: bool,
    ) -> Response {
        let mut requests = Requests::default();
        let everything = Some(Rectangle::INFINITE);
        self.focus.taken = false;

        let handled = {
            let mut router = tree::Router {
                now,
                pointer,
                touch,
                requests: &mut requests,
                focus: &mut self.focus,
            };

            if tree::is_focused_input(event) {
                match router.focus.current.clone() {
                    Some(target) => tree::deliver(
                        &mut self.child,
                        Point::ORIGIN,
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
                    &mut self.child,
                    Point::ORIGIN,
                    everything,
                    &mut Vec::new(),
                    event,
                    &mut router,
                )
            }
        };

        let mut captured = handled == Handled::Yes;

        // Back that the focused view did not use goes to the topmost view
        // that can close something: a dialog, a sheet, a drawer.
        if !captured && is_back(event) {
            let mut router = tree::Router {
                now,
                pointer,
                touch,
                requests: &mut requests,
                focus: &mut self.focus,
            };

            captured = tree::broadcast(
                &mut self.child,
                Point::ORIGIN,
                everything,
                &mut Vec::new(),
                event,
                &mut router,
            ) == Handled::Yes;
        }

        // Clicking anything that does not take focus clears it.
        if tree::is_press(event) && !self.focus.taken && self.focus.current.take().is_some() {
            requests.paint = true;
        }

        // Tab moves focus when the focused view did not use it.
        if !captured && let Some(backwards) = tree::tab_press(event) {
            let order = tree::focusables(&mut self.child);
            self.focus.current = tree::tab(&order, self.focus.current.as_deref(), backwards);
            requests.paint = true;
            captured = true;
        }

        Response {
            captured,
            paint: requests.paint || requests.layout || requests.animate,
            layout: requests.layout,
            animate: requests.animate,
            copy: requests.copy,
            paste: requests.paste,
        }
    }

    /// The cursor the view under `pointer` wants.
    pub fn cursor(&mut self, pointer: Option<Point>) -> mouse::Interaction {
        pointer
            .and_then(|pointer| tree::cursor(&mut self.child, Point::ORIGIN, pointer))
            .unwrap_or(DEFAULT_CURSOR)
    }

    /// The window's semantic tree: one group of `size` holding every
    /// described view.
    pub fn describe(&mut self, size: Size) -> semantics::Node {
        let mut nodes = Vec::new();
        tree::describe(
            &mut self.child,
            Point::ORIGIN,
            &mut Vec::new(),
            self.focus.current.as_deref(),
            &mut nodes,
        );

        let mut window = semantics::Node::new(
            self.id.clone(),
            Rectangle::new(Point::ORIGIN, size),
            semantics::Role::Group,
        );
        window.children = nodes;
        window
    }
}

impl std::fmt::Debug for Root {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Root").finish_non_exhaustive()
    }
}

/// Whether `event` is a press asking to go back: the Android back button or
/// gesture, a browser/mouse back key, or Escape.
///
/// A back press no view used is the platform's to handle; on Android that
/// leaves the application, like the root route of a navigator.
pub fn is_back(event: &Event) -> bool {
    use crate::keyboard::{self, key::Named};

    matches!(
        event,
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(Named::Escape | Named::BrowserBack | Named::GoBack),
            ..
        })
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cx::{Focus, Requests};
    use crate::layout::Constraints;
    use crate::tree;
    use crate::views;
    use crate::views::{button, column, row, text};
    use crate::{Child, Event, Handled, Point, Rectangle, Size, View, mouse, semantics};

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
                    touch: false,
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

    fn key(named: crate::keyboard::key::Named) -> Event {
        use crate::keyboard;

        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(named),
            modified_key: keyboard::Key::Named(named),
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        })
    }

    #[test]
    fn back_closes_the_open_dialog_and_only_then_reaches_the_platform() {
        use crate::keyboard::key::Named;
        use crate::views::dialog;
        use std::cell::Cell;
        use std::rc::Rc;

        let handle = Rc::new(Cell::new(None));
        let mut root = Root::new({
            let handle = handle.clone();
            move |cx| {
                let open = cx.signal(true);
                handle.set(Some(open));
                column(views![text("page"), dialog(open, text("Saved"))])
            }
        });
        let open = handle.get().expect("built");
        let now = crate::Instant::now();

        let first = root.handle(&key(Named::BrowserBack), None, now);
        assert!(first.captured, "an open dialog takes the back press");
        assert!(!open.get(), "and closes");

        let second = root.handle(&key(Named::BrowserBack), None, now);
        assert!(
            !second.captured,
            "with nothing left to close, back belongs to the platform"
        );
    }

    #[test]
    fn only_back_keys_count_as_back() {
        use crate::keyboard::key::Named;

        assert!(is_back(&key(Named::BrowserBack)));
        assert!(is_back(&key(Named::GoBack)));
        assert!(is_back(&key(Named::Escape)));
        assert!(!is_back(&key(Named::Enter)));
    }
}
