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
use crate::{Theme, keyboard, mouse, semantics};

use taim_core::input_method::{self, InputMethod, Preedit};
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
    focus_identity: Option<widget::Id>,
    modal_focus: Vec<ModalFocus>,
    /// Theme name and scale the shell last applied.
    applied: Option<(String, f32)>,
    id: widget::Id,
    preedit: Option<Preedit>,
    composition: crate::ime::Composition,
    system_motion_reduced: bool,
    paste_sequence: u64,
    pending_paste: Option<PendingPaste>,
    /// The focus path [`Root::wants_text_input`] last answered for.
    text_input: Option<(Option<Vec<u16>>, bool)>,
    /// Edges the platform covers (status bar, navigation bar, notch,
    /// on-screen keyboard); views are laid out inside them.
    insets: crate::Padding,
}

/// Opaque single-use identity for one asynchronous clipboard read.
/// It contains no editor text and is scoped to its originating Root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasteToken {
    root: widget::Id,
    sequence: u64,
}
struct PendingPaste {
    sequence: u64,
    target: widget::Id,
    editor: Option<input_method::EditorState>,
    purpose: Option<input_method::Purpose>,
}

struct ModalFocus {
    identity: widget::Id,
    path: Vec<u16>,
    restore: Option<widget::Id>,
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
    /// Capture the focused destination before starting an asynchronous clipboard read.
    /// A newer request supersedes the old one; no secret editor text is retained.
    pub fn begin_paste(&mut self) -> Option<PasteToken> {
        self.pending_paste = None;
        let target = self.focused_identity()?;
        self.paste_sequence = self.paste_sequence.checked_add(1)?;
        let purpose = self.paste_purpose();
        let editor = if purpose == Some(input_method::Purpose::Secure) {
            None
        } else {
            self.editor_state().map(|(_, state)| state)
        };
        self.pending_paste = Some(PendingPaste {
            sequence: self.paste_sequence,
            target,
            editor,
            purpose,
        });
        Some(PasteToken {
            root: self.id.clone(),
            sequence: self.paste_sequence,
        })
    }
    /// Authorize one completion only if its original editor and editing context remain current.
    /// Call before delivering the Clipboard event; rejected old reads cannot cancel a newer read.
    pub fn accept_paste(&mut self, token: &PasteToken) -> bool {
        self.reconcile_focus();
        if token.root != self.id
            || self
                .pending_paste
                .as_ref()
                .is_none_or(|pending| pending.sequence != token.sequence)
        {
            return false;
        }
        let Some(pending) = self.pending_paste.take() else {
            return false;
        };
        if self.focused_identity() != Some(pending.target)
            || self.paste_purpose() != pending.purpose
        {
            return false;
        }
        let editor = if pending.purpose == Some(input_method::Purpose::Secure) {
            None
        } else {
            self.editor_state().map(|(_, state)| state)
        };
        editor == pending.editor
    }
    fn paste_purpose(&mut self) -> Option<input_method::Purpose> {
        match self.input_method() {
            InputMethod::Enabled { purpose, .. } => Some(purpose),
            InputMethod::Disabled => None,
        }
    }

    /// Current native hierarchy slots after layout. Active Taim modals hide every native slot.
    pub fn native_views(&mut self) -> Vec<crate::NativeViewPlacement> {
        tree::native_views(&mut self.child)
    }
    /// Release Taim keyboard/composition ownership when a native control takes input.
    pub fn release_focus(&mut self, now: Instant) -> Response {
        let mut response = self.handle(
            &Event::Window(taim_core::window::Event::Unfocused),
            None,
            now,
        );
        self.focus.current = None;
        self.focus_identity = None;
        self.preedit = None;
        self.composition.clear();
        response.paint = true;
        response
    }
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
            focus_identity: None,
            modal_focus: Vec::new(),
            applied: None,
            text_input: None,
            insets: crate::Padding::ZERO,
            id: widget::Id::unique(),
            preedit: None,
            composition: crate::ime::Composition::default(),
            system_motion_reduced: false,
            paste_sequence: 0,
            pending_paste: None,
        }
    }

    /// Whether a modal or application navigation can consume a back gesture.
    pub fn handles_back(&mut self) -> bool {
        self.reconcile_focus();
        !self.modal_focus.is_empty()
            || self
                .scope
                .settings
                .back
                .as_ref()
                .is_some_and(|(enabled, _)| enabled.get())
    }

    /// Dismisses the active modal first, then invokes enabled app navigation.
    pub fn back(&mut self, now: Instant) -> Response {
        self.reconcile_focus();
        if !self.modal_focus.is_empty() {
            let key = keyboard::Key::Named(keyboard::key::Named::Escape);
            let response = self.handle(
                &Event::Keyboard(keyboard::Event::KeyPressed {
                    key: key.clone(),
                    modified_key: key,
                    physical_key: keyboard::key::Physical::Unidentified(
                        keyboard::key::NativeCode::Unidentified,
                    ),
                    location: keyboard::Location::Standard,
                    modifiers: keyboard::Modifiers::empty(),
                    text: None,
                    repeat: false,
                }),
                None,
                now,
            );
            if response.captured {
                return response;
            }
        }
        if let Some((enabled, action)) = &self.scope.settings.back
            && enabled.get()
        {
            action();
            return Response {
                captured: true,
                paint: true,
                ..Response::default()
            };
        }
        Response::default()
    }

    /// Builds a scope using a versioned, explicitly opted-in state checkpoint.
    pub fn new_restored<V: Into<AnyView>>(
        snapshot: &str,
        build: impl FnOnce(&mut Scope) -> V,
    ) -> Self {
        Self::new(|cx| {
            cx.restoration.load(snapshot);
            build(cx)
        })
    }

    /// Serializes only values registered with `Scope::restorable`.
    pub fn restoration_snapshot(&self) -> Result<String, crate::RestorationError> {
        self.scope.restoration.snapshot()
    }

    /// Calls `wake` (from any thread) whenever state the tree reads changes.
    pub fn on_change(&self, wake: impl Fn() + Send + Sync + 'static) {
        self.invalidation.set_waker(wake);
    }

    /// What state changes owe the window since the last call: `(paint,
    /// layout)`.
    pub fn take_changes(&self) -> (bool, bool) {
        let more = self.scope.receive();
        let owed = self.invalidation.take();
        if more {
            self.invalidation.mark(Invalidation::LAYOUT);
        }
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
        let scale = self
            .scope
            .settings
            .scale
            .as_ref()
            .map_or(1.0, crate::Prop::get);
        if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        }
    }

    /// Applies the application's motion preference and reports whether its
    /// theme or zoom changed since the last call.
    pub fn settings_changed(&mut self) -> bool {
        let _ = self.scope.receive();
        use taim_core::theme::Base as _;

        crate::state::set_motion_reduced(
            self.scope
                .settings
                .reduced_motion
                .as_ref()
                .map_or(self.system_motion_reduced, crate::Prop::get),
        );

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

    /// Sets the platform preference, overridden by an explicit app preference.
    pub fn set_system_motion_reduced(&mut self, reduced: bool) -> bool {
        if self.system_motion_reduced == reduced {
            return false;
        }
        self.system_motion_reduced = reduced;
        crate::state::set_motion_reduced(
            self.scope
                .settings
                .reduced_motion
                .as_ref()
                .map_or(reduced, crate::Prop::get),
        );
        true
    }

    /// Stable identity of the focused view, without disclosing editor contents.
    pub fn focused_identity(&mut self) -> Option<widget::Id> {
        self.reconcile_focus();
        self.focus_identity.clone()
    }

    /// Semantic node identity at keyboard focus. This differs from the retained
    /// child identity used to track focus through reparenting.
    pub fn focused_semantic_identity(&mut self) -> Option<widget::Id> {
        self.reconcile_focus();
        let focused = self.focus.current.as_deref()?;
        let mut identity = None;
        tree::walk(
            &mut self.child,
            Point::ORIGIN,
            &mut Vec::new(),
            &mut |view, bounds, path| {
                if path == focused {
                    identity = view
                        .describe(&crate::DescribeCx {
                            bounds,
                            focused: true,
                            _marker: std::marker::PhantomData,
                        })
                        .map(|node| node.id);
                }
            },
        );
        identity
    }

    /// Public native-editor context for the focused view and its stable identity.
    pub fn editor_state(&mut self) -> Option<(widget::Id, input_method::EditorState)> {
        self.reconcile_focus();
        let focused = self.focus.current.as_deref()?;
        let mut state = None;
        tree::walk(
            &mut self.child,
            Point::ORIGIN,
            &mut Vec::new(),
            &mut |view, _, path| {
                if path == focused {
                    state = view.editor_state();
                }
            },
        );
        state
            .zip(self.focus_identity.clone())
            .map(|(state, id)| (id, state.normalized()))
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
        self.reconcile_focus();
    }

    fn reconcile_focus(&mut self) {
        let previous = self.focus_identity.clone();
        let scopes = tree::modal_scopes(&mut self.child, &mut Vec::new(), &mut Vec::new());
        let shared = self
            .modal_focus
            .iter()
            .zip(&scopes)
            .take_while(|(old, (id, _))| old.identity == *id)
            .count();
        let modal_changed = shared != self.modal_focus.len() || shared != scopes.len();
        if shared < self.modal_focus.len() {
            self.focus_identity = self.modal_focus[shared].restore.clone();
            self.modal_focus.truncate(shared);
        }
        for (index, (identity, path)) in scopes.into_iter().enumerate() {
            if index < shared {
                self.modal_focus[index].path = path;
            } else {
                self.modal_focus.push(ModalFocus {
                    identity,
                    path: path.clone(),
                    restore: self.focus_identity.clone(),
                });
                tree::cancel_outside_scope(&mut self.child, &path);
                self.keep_focus_within(Some(&path));
            }
        }
        let scope = self.modal_focus.last().map(|scope| scope.path.clone());
        self.keep_focus_within(scope.as_deref());
        if modal_changed || previous != self.focus_identity {
            self.pending_paste = None;
        }
        if previous != self.focus_identity {
            self.invalidation.mark(Invalidation::PAINT);
            self.preedit = None;
        }
    }

    fn keep_focus_within(&mut self, scope: Option<&[u16]>) {
        self.focus.current = self
            .focus_identity
            .as_ref()
            .and_then(|identity| tree::focus_path(&mut self.child, identity, &mut Vec::new()))
            .filter(|path| scope.is_none_or(|scope| path.starts_with(scope)));
        if self.focus.current.is_none()
            && let Some(scope) = scope
        {
            self.focus.current = tree::focusables(&mut self.child)
                .into_iter()
                .find(|path| path.starts_with(scope));
        }
        self.focus_identity = self
            .focus
            .current
            .as_deref()
            .and_then(|path| tree::focus_identity(&mut self.child, path));
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
        self.reconcile_focus();
        if let InputMethod::Enabled {
            cursor,
            purpose,
            preedit: Some(preedit),
        } = self.input_method()
        {
            self.composition
                .paint(renderer, &self.theme, cursor, purpose, &preedit);
        } else {
            self.composition.clear();
        }
    }

    /// The focused view's text input request in window logical coordinates.
    /// Non-text focus disables the platform input method.
    pub fn input_method(&mut self) -> InputMethod {
        let Some(focused) = self.focus.current.as_deref() else {
            return InputMethod::Disabled;
        };
        let mut request = InputMethod::Disabled;
        tree::walk(
            &mut self.child,
            Point::ORIGIN,
            &mut Vec::new(),
            &mut |view, bounds, path| {
                if path == focused {
                    request = view.input_method(bounds);
                }
            },
        );
        if let InputMethod::Enabled { preedit, .. } = &mut request {
            *preedit = self.preedit.clone();
        }
        request
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

    /// The earliest one-shot deadline requested by any view.
    pub fn redraw_at(&mut self) -> Option<Instant> {
        let mut earliest = None;
        tree::walk(
            &mut self.child,
            Point::ORIGIN,
            &mut Vec::new(),
            &mut |view, _, _| {
                if let Some(deadline) = view.redraw_at() {
                    earliest =
                        Some(earliest.map_or(deadline, |previous: Instant| previous.min(deadline)));
                }
            },
        );
        earliest
    }

    /// Routes one event: keyboard input to the focused view, everything
    /// else to every view, topmost first.
    pub fn handle(&mut self, event: &Event, pointer: Option<Point>, now: Instant) -> Response {
        self.handle_inner(None, event, pointer, now, false)
    }

    /// Like [`Root::handle`], saying whether the pointer is a finger, for
    /// shells that turn the first finger into pointer events; views that
    /// behave differently for a finger read
    /// [`EventCx::is_touch`](crate::EventCx::is_touch).
    pub fn handle_input(
        &mut self,
        event: &Event,
        pointer: Option<Point>,
        now: Instant,
        touch: bool,
    ) -> Response {
        self.handle_inner(None, event, pointer, now, touch)
    }

    /// Routes an event with access to the active rendering backend for
    /// custom views that need its text metrics or widget interoperation.
    pub fn handle_with_renderer(
        &mut self,
        renderer: &Renderer,
        event: &Event,
        pointer: Option<Point>,
        now: Instant,
    ) -> Response {
        self.handle_inner(Some(renderer), event, pointer, now, false)
    }

    fn handle_inner(
        &mut self,
        renderer: Option<&Renderer>,
        event: &Event,
        pointer: Option<Point>,
        now: Instant,
        touch: bool,
    ) -> Response {
        // Native finger events are touch too, whichever way the shell reports them.
        let touch = touch || matches!(event, Event::Touch(_));
        self.reconcile_focus();
        if matches!(
            event,
            Event::Keyboard(keyboard::Event::KeyPressed { .. })
                | Event::InputMethod(_)
                | Event::Mouse(mouse::Event::ButtonPressed(_))
                | Event::Touch(taim_core::touch::Event::FingerPressed { .. })
                | Event::Window(taim_core::window::Event::Unfocused)
        ) {
            self.pending_paste = None;
        }
        // Match the core pointer policy: malformed active samples are ignored;
        // terminal touch events still reach routing so they can cancel owners.
        let invalid = |point: Point| !point.x.is_finite() || !point.y.is_finite();
        let invalid_active = match event {
            Event::Mouse(mouse::Event::CursorMoved { position }) => invalid(*position),
            Event::Mouse(mouse::Event::ButtonPressed(_)) => pointer.is_some_and(invalid),
            Event::Touch(
                taim_core::touch::Event::FingerPressed { position, .. }
                | taim_core::touch::Event::FingerMoved { position, .. },
            ) => invalid(*position),
            _ => false,
        };
        if invalid_active {
            return Response::default();
        }
        let pointer = match event {
            Event::Touch(taim_core::touch::Event::FingerLost { .. }) => None,
            Event::Touch(touch) => {
                let point = touch.position();
                (point.x.is_finite() && point.y.is_finite()).then_some(point)
            }
            _ => pointer,
        };
        let mut composition_cleared = false;
        match event {
            Event::InputMethod(input_method::Event::Edit(state))
                if state.composing.is_some() && self.input_method().is_enabled() =>
            {
                let state = state.clone().normalized();
                self.preedit = state.composing.map(|range| Preedit {
                    content: state.content[range].to_owned(),
                    selection: None,
                    text_size: None,
                });
                return Response {
                    captured: true,
                    paint: true,
                    ..Response::default()
                };
            }
            Event::InputMethod(input_method::Event::Edit(_)) => {
                self.preedit = None;
            }
            Event::InputMethod(input_method::Event::Preedit(content, selection))
                if self.input_method().is_enabled() =>
            {
                self.preedit = (!content.is_empty()).then(|| Preedit {
                    content: content.clone(),
                    selection: selection.clone().filter(|range| {
                        range.start <= range.end
                            && range.end <= content.len()
                            && content.is_char_boundary(range.start)
                            && content.is_char_boundary(range.end)
                    }),
                    text_size: None,
                });
                return Response {
                    captured: true,
                    paint: true,
                    ..Response::default()
                };
            }
            Event::InputMethod(input_method::Event::Commit(_) | input_method::Event::Closed)
            | Event::Window(taim_core::window::Event::Unfocused) => {
                composition_cleared = self.preedit.take().is_some();
            }
            // During composition the IME owns editing/navigation keys. The
            // final Commit is the only event allowed to change the buffer.
            Event::Keyboard(keyboard::Event::KeyPressed { .. }) if self.preedit.is_some() => {
                return Response {
                    captured: true,
                    ..Response::default()
                };
            }
            _ => {}
        }
        let previous_focus = self.focus.current.clone();
        let mut requests = Requests {
            paint: composition_cleared,
            ..Requests::default()
        };
        let everything = Some(Rectangle::INFINITE);
        self.focus.taken = false;

        let handled = {
            let mut router = tree::Router {
                renderer,
                now,
                pointer,
                touch,
                requests: &mut requests,
                focus: &mut self.focus,
            };

            if tree::is_focused_input(event) {
                match router
                    .focus
                    .current
                    .clone()
                    .or_else(|| self.modal_focus.last().map(|scope| scope.path.clone()))
                {
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
                renderer,
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
            let order: Vec<_> = tree::focusables(&mut self.child)
                .into_iter()
                .filter(|path| {
                    self.modal_focus
                        .last()
                        .is_none_or(|scope| path.starts_with(&scope.path))
                })
                .collect();
            self.focus.current = tree::tab(&order, self.focus.current.as_deref(), backwards);
            requests.paint = true;
            captured = true;
        }

        self.focus_identity = self
            .focus
            .current
            .as_deref()
            .and_then(|path| tree::focus_identity(&mut self.child, path));
        if self.focus_identity.is_none() && self.focus.current.take().is_some() {
            requests.paint = true;
        }
        if previous_focus != self.focus.current {
            self.preedit = None;
        }

        self.reconcile_focus();

        Response {
            captured,
            paint: requests.paint || requests.layout || requests.animate,
            layout: requests.layout,
            animate: requests.animate,
            copy: requests.copy,
            paste: requests.paste,
        }
    }

    /// Executes a semantic action against the current live view tree.
    /// Stale identities, disabled controls and unadvertised actions are rejected.
    pub fn accessibility_action(&mut self, action: &semantics::Action, now: Instant) -> Response {
        self.accessibility_action_inner(None, action, now)
    }

    /// Executes a semantic action with active renderer metrics available to custom views.
    pub fn accessibility_action_with_renderer(
        &mut self,
        renderer: &Renderer,
        action: &semantics::Action,
        now: Instant,
    ) -> Response {
        self.accessibility_action_inner(Some(renderer), action, now)
    }

    fn accessibility_action_inner(
        &mut self,
        renderer: Option<&Renderer>,
        action: &semantics::Action,
        now: Instant,
    ) -> Response {
        self.pending_paste = None;
        self.reconcile_focus();
        let previous_focus = self.focus.current.clone();
        let mut requests = Requests::default();
        let scope = self.modal_focus.last().map(|scope| scope.path.clone());
        let handled = tree::accessibility_action(
            &mut self.child,
            Point::ORIGIN,
            &mut Vec::new(),
            action,
            scope.as_deref(),
            &mut tree::Router {
                renderer,
                now,
                pointer: None,
                touch: false,
                requests: &mut requests,
                focus: &mut self.focus,
            },
        )
        .unwrap_or(Handled::No);
        self.focus_identity = self
            .focus
            .current
            .as_deref()
            .and_then(|path| tree::focus_identity(&mut self.child, path));
        if self.focus_identity.is_none() {
            self.focus.current = None;
        }
        if previous_focus != self.focus.current {
            self.preedit = None;
            requests.paint = true;
        }
        Response {
            captured: handled == Handled::Yes,
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
        self.reconcile_focus();
        let mut nodes = Vec::new();
        tree::describe_scope(
            &mut self.child,
            Point::ORIGIN,
            &mut Vec::new(),
            self.modal_focus
                .last()
                .map_or(&[], |scope| scope.path.as_slice()),
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

    #[test]
    fn a_busy_mailbox_rearms_the_waker_after_its_drain_budget() {
        use std::cell::{Cell, RefCell};
        use std::rc::Rc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let slot = Rc::new(RefCell::new(None::<crate::Sender<u32>>));
        let received = Rc::new(Cell::new(0));
        let root = Root::new(|cx| {
            let sender = cx.channel({
                let slot = slot.clone();
                let received = received.clone();
                move |value| {
                    received.set(received.get() + 1);
                    if value < 256 {
                        slot.borrow().as_ref().unwrap().send(value + 256).unwrap();
                    }
                }
            });
            *slot.borrow_mut() = Some(sender);
            text("Mailbox")
        });
        let wakes = Arc::new(AtomicUsize::new(0));
        root.on_change({
            let wakes = wakes.clone();
            move || {
                let _ = wakes.fetch_add(1, Ordering::Relaxed);
            }
        });
        for value in 0..256 {
            slot.borrow().as_ref().unwrap().send(value).unwrap();
        }
        assert_eq!(wakes.load(Ordering::Relaxed), 1);
        assert_eq!(root.take_changes(), (true, true));
        assert_eq!(received.get(), 256);
        assert_eq!(
            wakes.load(Ordering::Relaxed),
            2,
            "queued work must get a new event-loop wake"
        );
        let _ = root.take_changes();
        assert_eq!(received.get(), 512);
    }

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
    fn cancellation_reaches_every_view_even_when_a_sibling_captures_it() {
        use std::cell::Cell;
        use std::rc::Rc;
        struct Listener(Rc<Cell<usize>>);
        impl crate::View for Listener {
            fn measure(&mut self, _: &mut crate::MeasureCx<'_>, _: crate::Constraints) -> Size {
                Size::new(10.0, 10.0)
            }
            fn paint(&self, _: &mut crate::PaintCx<'_>) {}
            fn describe(&self, _: &crate::DescribeCx<'_>) -> Option<semantics::Node> {
                None
            }
            fn event(&mut self, _: &mut crate::EventCx<'_>, event: &Event) -> Handled {
                if tree::is_cancellation(event) {
                    self.0.set(self.0.get() + 1);
                    Handled::Yes
                } else {
                    Handled::No
                }
            }
        }
        let count = Rc::new(Cell::new(0));
        let mut root =
            Root::new(|_| column(views![Listener(count.clone()), Listener(count.clone())]));
        let response = root.handle(
            &Event::Window(taim_core::window::Event::Unfocused),
            None,
            Instant::now(),
        );
        assert!(response.captured);
        assert_eq!(count.get(), 2);
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
    fn invalid_zoom_does_not_keep_window_settings_permanently_dirty() {
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -1.0] {
            let mut root = Root::new(|cx| {
                cx.scale(invalid);
                text("Invalid zoom")
            });
            assert_eq!(root.scale(), 1.0);
            assert!(root.settings_changed());
            assert!(!root.settings_changed());
        }
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
                    renderer: None,
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

    #[test]
    fn reverse_child_order_is_the_reverse_of_paint_order() {
        use crate::views::{grid, layers};

        fn order(view: &mut dyn View, reverse: bool) -> Vec<*const Child> {
            let mut seen = Vec::new();
            let mut record = |child: &mut Child| seen.push(child as *const Child);
            if reverse {
                view.children_rev(&mut record);
            } else {
                view.children(&mut record);
            }
            seen
        }

        let mut views: Vec<Box<dyn View>> = vec![
            Box::new(column(views![text("a"), text("b"), text("c")])),
            Box::new(layers(views![text("a"), text("b"), text("c")])),
            Box::new(grid(2, views![text("a"), text("b"), text("c")])),
            // A view with a single child uses the default.
            Box::new(crate::views::container(text("a"))),
        ];

        for view in &mut views {
            let forward = order(view.as_mut(), false);
            let mut backward = order(view.as_mut(), true);
            backward.reverse();

            assert_eq!(forward, backward, "{}", view.debug_name());
        }
    }
}

#[cfg(test)]
mod paste_privacy_tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;
    struct PrivateEditor {
        purpose: Rc<Cell<input_method::Purpose>>,
        reads: Rc<Cell<u32>>,
    }
    impl crate::View for PrivateEditor {
        fn measure(&mut self, _: &mut crate::MeasureCx<'_>, _: Constraints) -> Size {
            Size::new(100.0, 40.0)
        }
        fn paint(&self, _: &mut crate::PaintCx<'_>) {}
        fn describe(&self, _: &crate::DescribeCx<'_>) -> Option<semantics::Node> {
            None
        }
        fn focusable(&self) -> bool {
            true
        }
        fn input_method(&self, bounds: Rectangle) -> InputMethod {
            InputMethod::Enabled {
                cursor: bounds,
                purpose: self.purpose.get(),
                preedit: None,
            }
        }
        fn editor_state(&self) -> Option<input_method::EditorState> {
            self.reads.set(self.reads.get() + 1);
            Some(input_method::EditorState {
                content: "PRIVATE_MUST_NOT_BE_READ".into(),
                selection: 0..0,
                composing: None,
                multiline: false,
            })
        }
    }
    #[test]
    fn secure_paste_never_reads_even_a_custom_widgets_editor_state_and_purpose_changes_expire_lease()
     {
        let purpose = Rc::new(Cell::new(input_method::Purpose::Secure));
        let reads = Rc::new(Cell::new(0));
        let mut root = Root::new(|_| PrivateEditor {
            purpose: purpose.clone(),
            reads: reads.clone(),
        });
        let _ = root.handle(
            &Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Tab),
                modified_key: keyboard::Key::Named(keyboard::key::Named::Tab),
                physical_key: keyboard::key::Physical::Unidentified(
                    keyboard::key::NativeCode::Unidentified,
                ),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::NONE,
                text: None,
                repeat: false,
            }),
            None,
            Instant::now(),
        );
        let token = root.begin_paste().unwrap();
        assert_eq!(reads.get(), 0);
        assert!(root.accept_paste(&token));
        assert_eq!(reads.get(), 0);
        let token = root.begin_paste().unwrap();
        purpose.set(input_method::Purpose::Normal);
        assert!(!root.accept_paste(&token));
        assert_eq!(reads.get(), 0);
    }
}
