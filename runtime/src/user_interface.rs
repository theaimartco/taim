//! Implement your own event loop to drive a user interface.
use crate::core::damage;
use crate::core::event::{self, Event};
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::shell;
use crate::core::widget;
use crate::core::widget::visit;
use crate::core::window;
use crate::core::{
    Clipboard, Element, InputMethod, Layout, Point, Rectangle, Shell, Size, Vector, Window,
};
use crate::diagnostics;
use crate::hit_test;
use crate::invalidation::Aggregator;
use crate::pointer_router::{self, Routing};

use std::hash::{Hash, Hasher};
use taim_gesture::arena::Arena;

/// A set of interactive graphical elements with a specific [`Layout`].
///
/// It can be updated and drawn.
///
/// Taim tries to avoid dictating how to write your event loop. You are in
/// charge of using this type in your system in any way you want.
///
/// # Example
/// The [`integration`] example uses a [`UserInterface`] to integrate Taim in an
/// existing graphical application.
///
/// [`integration`]: https://github.com/theaimartco/taim/tree/master/examples/integration
pub struct UserInterface<'a, Message, Theme, Renderer> {
    root: Element<'a, Message, Theme, Renderer>,
    base: layout::Node,
    state: widget::Tree,
    overlay: Option<Overlay>,
    bounds: Size,
    pointer_router: pointer_router::Router,
    automatic_pointer_routing: bool,
    semantic_tree: taim_accessibility::Tree,
    trace: diagnostics::Trace,
}

struct Overlay {
    layout: layout::Node,
    interaction: mouse::Interaction,
}

impl<'a, Message, Theme, Renderer> UserInterface<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    /// Builds a user interface for an [`Element`].
    ///
    /// It is able to avoid expensive computations when using a [`Cache`]
    /// obtained from a previous instance of a [`UserInterface`].
    ///
    /// # Example
    /// Imagine we want to build a [`UserInterface`] for
    /// [the counter example that we previously wrote](index.html#usage). Here
    /// is naive way to set up our application loop:
    ///
    /// ```no_run
    /// # mod taim_wgpu {
    /// #     pub type Renderer = ();
    /// # }
    /// #
    /// # pub struct Counter;
    /// #
    /// # impl Counter {
    /// #     pub fn new() -> Self { Counter }
    /// #     pub fn view(&self) -> taim_core::Element<(), (), Renderer> { unimplemented!() }
    /// #     pub fn update(&mut self, _: ()) {}
    /// # }
    /// use taim_runtime::core::shell;
    /// use taim_runtime::core::window;
    /// use taim_runtime::core::Size;
    /// use taim_runtime::user_interface::{self, UserInterface};
    /// use taim_wgpu::Renderer;
    ///
    /// // Initialization
    /// let mut counter = Counter::new();
    /// let mut cache = user_interface::Cache::new();
    /// let mut renderer = Renderer::default();
    /// let mut window = window::Headless; // This should be a proper window, like a `winit` one
    /// let mut waker = shell::Waker::noop();
    /// let mut window_size = Size::new(1024.0, 768.0);
    ///
    /// // Application loop
    /// loop {
    ///     // Process system events here...
    ///
    ///     // Build the user interface
    ///     let user_interface = UserInterface::build(
    ///         counter.view(),
    ///         window_size,
    ///         cache,
    ///         &mut renderer,
    ///     );
    ///
    ///     // Update and draw the user interface here...
    ///     // ...
    ///
    ///     // Obtain the cache for the next iteration
    ///     cache = user_interface.into_cache();
    /// }
    /// ```
    pub fn build<E: Into<Element<'a, Message, Theme, Renderer>>>(
        root: E,
        bounds: Size,
        cache: Cache,
        renderer: &mut Renderer,
    ) -> Self {
        Self::build_ref(root, bounds, cache, renderer)
    }

    /// Builds with shared renderer metrics, without painting.
    pub fn build_ref<E: Into<Element<'a, Message, Theme, Renderer>>>(
        root: E,
        bounds: Size,
        cache: Cache,
        renderer: &Renderer,
    ) -> Self {
        let mut root = root.into();

        let Cache {
            mut state,
            pointer_router,
            automatic_pointer_routing,
            semantic_tree,
            trace,
        } = cache;

        trace.begin(diagnostics::Phase::Diff);
        state.diff(root.as_widget_mut());
        state.mark_dirty(widget::tree::DirtyFlags::PAINT | widget::tree::DirtyFlags::SEMANTICS);
        trace.end(diagnostics::Phase::Diff);

        trace.begin(diagnostics::Phase::Layout);
        let base = root.as_widget_mut().layout(
            &mut state,
            renderer,
            &layout::Limits::new(Size::ZERO, bounds),
        );
        trace.end(diagnostics::Phase::Layout);

        UserInterface {
            root,
            base,
            state,
            overlay: None,
            bounds,
            pointer_router,
            automatic_pointer_routing,
            semantic_tree,
            trace,
        }
    }

    /// Records this [`UserInterface`]'s phases into `trace`.
    ///
    /// Off by default: a [`UserInterface`] built without this carries a
    /// disabled collector, so every span below costs a relaxed atomic
    /// load and nothing else.
    ///
    /// Survives [`UserInterface::relayout`], which rebuilds — a trace that
    /// switched itself off the first time the window resized would be a
    /// puzzle rather than a tool.
    #[must_use]
    pub fn with_trace(mut self, trace: diagnostics::Trace) -> Self {
        self.trace = trace;
        self
    }

    /// The trace this [`UserInterface`] records into.
    pub fn trace(&self) -> &diagnostics::Trace {
        &self.trace
    }

    /// Enables or disables automatic gesture-arena routing during [`Self::update`].
    ///
    /// Automatic routing is enabled by default. Integrations that call
    /// [`Self::route_pointer`] themselves to install recognizers should disable
    /// it before dispatching the same event batch, preventing duplicate arena
    /// delivery.
    pub fn set_automatic_pointer_routing(&mut self, enabled: bool) {
        self.automatic_pointer_routing = enabled;
    }

    /// Updates the [`UserInterface`] by processing each provided [`Event`].
    ///
    /// It returns __messages__ that may have been produced as a result of user
    /// interactions. You should feed these to your __update logic__.
    ///
    /// # Example
    /// Let's allow our [counter](index.html#usage) to change state by
    /// completing [the previous example](#example):
    ///
    /// ```no_run
    /// # mod taim_wgpu {
    /// #     pub type Renderer = ();
    /// # }
    /// #
    /// # pub struct Counter;
    /// #
    /// # impl Counter {
    /// #     pub fn new() -> Self { Counter }
    /// #     pub fn view(&self) -> taim_core::Element<(), (), Renderer> { unimplemented!() }
    /// #     pub fn update(&mut self, _: ()) {}
    /// # }
    /// use taim_runtime::core::mouse;
    /// use taim_runtime::core::shell;
    /// use taim_runtime::core::window;
    /// use taim_runtime::core::Size;
    /// use taim_runtime::user_interface::{self, UserInterface};
    /// use taim_wgpu::Renderer;
    ///
    /// let mut counter = Counter::new();
    /// let mut cache = user_interface::Cache::new();
    /// let mut renderer = Renderer::default();
    /// let mut window = window::Headless; // This should be a proper window, like a `winit` one
    /// let mut waker = shell::Waker::noop();
    /// let mut window_size = Size::new(1024.0, 768.0);
    /// let mut cursor = mouse::Cursor::default();
    ///
    /// // Initialize our event storage
    /// let mut events = Vec::new();
    /// let mut messages = Vec::new();
    ///
    /// loop {
    ///     // Obtain system events...
    ///
    ///     let mut user_interface = UserInterface::build(
    ///         counter.view(),
    ///         window_size,
    ///         cache,
    ///         &mut renderer,
    ///     );
    ///
    ///     // Update the user interface
    ///     let (state, event_statuses) = user_interface.update(
    ///         &window,
    ///         &waker,
    ///         &events,
    ///         cursor,
    ///         &mut renderer,
    ///         &mut messages
    ///     );
    ///
    ///     cache = user_interface.into_cache();
    ///
    ///     // Process the produced messages
    ///     for message in messages.drain(..) {
    ///         counter.update(message);
    ///     }
    /// }
    /// ```
    pub fn update(
        &mut self,
        window: &dyn Window,
        waker: &shell::Waker,
        events: &[Event],
        cursor: mouse::Cursor,
        renderer: &mut Renderer,
        messages: &mut Vec<Message>,
    ) -> (State, Vec<event::Status>) {
        self.update_ref(window, waker, events, cursor, renderer, messages)
    }

    /// Dispatches events using shared renderer metrics, without painting.
    pub fn update_ref(
        &mut self,
        window: &dyn Window,
        waker: &shell::Waker,
        events: &[Event],
        cursor: mouse::Cursor,
        renderer: &Renderer,
        messages: &mut Vec<Message>,
    ) -> (State, Vec<event::Status>) {
        self.trace.begin(diagnostics::Phase::Event);

        let mut outdated = false;
        let mut redraw_request = window::RedrawRequest::Wait;
        let mut input_method = InputMethod::Disabled;
        let mut clipboard = Clipboard::new();
        let mut has_layout_changed = false;
        let mut is_animating = false;
        let viewport = Rectangle::with_size(self.bounds);

        // Pointer routing is part of the normal event path. A caller may
        // still use `route_pointer` to supply recognizers, but ordinary
        // mouse/touch input is never silently bypassing the arena anymore.
        if self.automatic_pointer_routing {
            let timestamp = crate::core::time::Instant::now();
            let _ = self.pointer_router.tick(timestamp);
            for event in events {
                let _ = self.route_pointer_default(event, cursor, timestamp);
            }
        }

        let mut maybe_overlay = self
            .root
            .as_widget_mut()
            .overlay(
                &mut self.state,
                Layout::new(&self.base),
                renderer,
                &viewport,
                Vector::ZERO,
            )
            .map(overlay::Nested::new);

        // The overlay is rebuilt from the current widget state for this
        // update. A previously open overlay must not survive merely because
        // the new tree no longer returns one.
        self.overlay = None;

        let (base_cursor, mut event_statuses, overlay_interaction) = if maybe_overlay.is_some() {
            let bounds = self.bounds;

            let mut overlay = maybe_overlay.as_mut().unwrap();
            let mut layout = overlay.layout(renderer, bounds);
            let mut event_statuses = Vec::with_capacity(events.len());

            for event in events {
                let mut shell = Shell::new(window, waker.clone(), messages);

                overlay.update(event, Layout::new(&layout), cursor, renderer, &mut shell);

                event_statuses.push(shell.event_status());
                redraw_request = redraw_request.min(shell.redraw_request());
                is_animating |= shell.is_animating();
                input_method.merge(shell.input_method());
                clipboard.merge(shell.clipboard_mut());

                if let Some(diff) = shell.is_layout_invalid() {
                    drop(maybe_overlay);
                    has_layout_changed = true;

                    match diff {
                        shell::Diff::Perform => {
                            self.root.as_widget_mut().diff(&mut self.state);
                        }
                        shell::Diff::Skip => {}
                    }

                    self.base = self.root.as_widget_mut().layout(
                        &mut self.state,
                        renderer,
                        &layout::Limits::new(Size::ZERO, self.bounds),
                    );

                    maybe_overlay = self
                        .root
                        .as_widget_mut()
                        .overlay(
                            &mut self.state,
                            Layout::new(&self.base),
                            renderer,
                            &viewport,
                            Vector::ZERO,
                        )
                        .map(overlay::Nested::new);

                    if maybe_overlay.is_none() {
                        break;
                    }

                    overlay = maybe_overlay.as_mut().unwrap();

                    shell.revalidate_layout(|_diff| {
                        layout = overlay.layout(renderer, bounds);
                    });
                }

                if shell.are_widgets_invalid() {
                    outdated = true;
                }
            }

            let (base_cursor, interaction) = if let Some(overlay) = maybe_overlay.as_mut() {
                let interaction = cursor
                    .position()
                    .map(|cursor_position| {
                        overlay.mouse_interaction(
                            Layout::new(&layout),
                            mouse::Cursor::Available(cursor_position),
                            renderer,
                        )
                    })
                    .unwrap_or_default();

                if interaction == mouse::Interaction::None {
                    (cursor, mouse::Interaction::None)
                } else {
                    (mouse::Cursor::Unavailable, interaction)
                }
            } else {
                (cursor, mouse::Interaction::None)
            };

            self.overlay = maybe_overlay.as_ref().map(|_| Overlay {
                layout,
                interaction,
            });

            // An overlay may close or invalidate its layout midway through
            // a batch. Remaining events must still reach the base widget;
            // otherwise `zip` would silently truncate the event stream.
            event_statuses.resize(events.len(), event::Status::Ignored);
            (base_cursor, event_statuses, interaction)
        } else {
            (
                cursor,
                vec![event::Status::Ignored; events.len()],
                mouse::Interaction::None,
            )
        };

        drop(maybe_overlay);

        for (event, status) in events.iter().zip(&mut event_statuses) {
            if matches!(*status, event::Status::Captured) {
                continue;
            }

            let mut shell = Shell::new(window, waker.clone(), messages);

            self.root.as_widget_mut().update(
                &mut self.state,
                event,
                Layout::new(&self.base),
                base_cursor,
                renderer,
                &mut shell,
                &viewport,
            );

            if shell.event_status() == event::Status::Captured {
                self.overlay = None;
            }

            redraw_request = redraw_request.min(shell.redraw_request());
            is_animating |= shell.is_animating();
            input_method.merge(shell.input_method());
            clipboard.merge(shell.clipboard_mut());

            shell.revalidate_layout(|diff| {
                has_layout_changed = true;

                match diff {
                    shell::Diff::Perform => {
                        self.root.as_widget_mut().diff(&mut self.state);
                    }
                    shell::Diff::Skip => {}
                }

                self.base = self.root.as_widget_mut().layout(
                    &mut self.state,
                    renderer,
                    &layout::Limits::new(Size::ZERO, self.bounds),
                );

                if let Some(mut overlay) = self
                    .root
                    .as_widget_mut()
                    .overlay(
                        &mut self.state,
                        Layout::new(&self.base),
                        renderer,
                        &viewport,
                        Vector::ZERO,
                    )
                    .map(overlay::Nested::new)
                {
                    let layout = overlay.layout(renderer, self.bounds);
                    let interaction =
                        overlay.mouse_interaction(Layout::new(&layout), cursor, renderer);

                    self.overlay = Some(Overlay {
                        layout,
                        interaction,
                    });
                }
            });

            if shell.are_widgets_invalid() {
                outdated = true;
            }

            *status = shell.event_status().merge(*status);
        }

        let mouse_interaction = if overlay_interaction == mouse::Interaction::None {
            self.root.as_widget().mouse_interaction(
                &self.state,
                Layout::new(&self.base),
                base_cursor,
                &viewport,
                renderer,
            )
        } else {
            overlay_interaction
        };

        // Aggregate damage from this frame's updates.
        //
        // Both paths go through the `Aggregator` -- including the layout
        // one, which used to short-circuit straight to `Region::Full`.
        // That short-circuit is why a relayout arrived at the caller with
        // no reason attached: the one place in the frame that *knew* the
        // answer was the one place that threw it away.
        let damage = if has_layout_changed {
            let mut aggregator = Aggregator::new();

            // `Layout`, not `Unknown`. A relayout is the most common
            // reason a frame redraws everything, and it is knowable right
            // here.
            aggregator.record(damage::Request::full(damage::Cause::Layout));
            aggregator.attribution()
        } else if redraw_request != window::RedrawRequest::Wait {
            // A redraw was requested but layout didn't change. Still
            // `Unknown`, and honestly so: without per-widget damage
            // reporting there is nothing more specific to say, and naming
            // a cause we do not know would be worse than admitting it.
            // Phase 5+ refines this to per-widget granularity.
            let mut aggregator = Aggregator::new();
            aggregator.record(damage::Request::full(damage::Cause::Unknown));
            aggregator.attribution()
        } else {
            None
        };

        if outdated || has_layout_changed || redraw_request != window::RedrawRequest::Wait {
            self.state.mark_dirty(widget::tree::DirtyFlags::PAINT);
        }

        // Closed here rather than at every early return: `update` has
        // one exit, and a span that leaks because a future edit adds a
        // second would silently charge the rest of the frame to `Event`.
        self.trace.end(diagnostics::Phase::Event);

        (
            if outdated {
                State::Outdated
            } else {
                State::Updated {
                    mouse_interaction,
                    redraw_request,
                    input_method,
                    clipboard,
                    has_layout_changed,
                    damage,
                    is_animating,
                }
            },
            event_statuses,
        )
    }

    /// Draws the [`UserInterface`] with the provided [`Renderer`].
    ///
    /// It returns the current [`mouse::Interaction`]. You should update the
    /// icon of the mouse cursor accordingly in your system.
    ///
    /// [`Renderer`]: crate::core::Renderer
    ///
    /// # Example
    /// We can finally draw our [counter](index.html#usage) by
    /// [completing the last example](#example-1):
    ///
    /// ```no_run
    /// # mod taim_wgpu {
    /// #     pub type Renderer = ();
    /// #     pub type Theme = ();
    /// # }
    /// #
    /// # pub struct Counter;
    /// #
    /// # impl Counter {
    /// #     pub fn new() -> Self { Counter }
    /// #     pub fn view(&self) -> Element<(), (), Renderer> { unimplemented!() }
    /// #     pub fn update(&mut self, _: ()) {}
    /// # }
    /// use taim_runtime::core::mouse;
    /// use taim_runtime::core::renderer;
    /// use taim_runtime::core::shell;
    /// use taim_runtime::core::window;
    /// use taim_runtime::core::{Element, Size};
    /// use taim_runtime::user_interface::{self, UserInterface};
    /// use taim_wgpu::{Renderer, Theme};
    ///
    /// let mut counter = Counter::new();
    /// let mut cache = user_interface::Cache::new();
    /// let mut renderer = Renderer::default();
    /// let mut window = window::Headless; // This should be a proper window, like a `winit` one
    /// let mut waker = shell::Waker::noop();
    /// let mut window_size = Size::new(1024.0, 768.0);
    /// let mut cursor = mouse::Cursor::default();
    /// let mut events = Vec::new();
    /// let mut messages = Vec::new();
    /// let mut theme = Theme::default();
    ///
    /// loop {
    ///     // Obtain system events...
    ///
    ///     let mut user_interface = UserInterface::build(
    ///         counter.view(),
    ///         window_size,
    ///         cache,
    ///         &mut renderer,
    ///     );
    ///
    ///     // Update the user interface
    ///     let event_statuses = user_interface.update(
    ///         &window,
    ///         &waker,
    ///         &events,
    ///         cursor,
    ///         &mut renderer,
    ///         &mut messages
    ///     );
    ///
    ///     // Draw the user interface
    ///     let mouse_interaction = user_interface.draw(&mut renderer, &theme, &renderer::Style::default(), cursor);
    ///
    ///     cache = user_interface.into_cache();
    ///
    ///     for message in messages.drain(..) {
    ///         counter.update(message);
    ///     }
    ///
    ///     // Update mouse cursor icon...
    ///     // Flush rendering operations...
    /// }
    /// ```
    pub fn draw(
        &mut self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        cursor: mouse::Cursor,
    ) {
        // A span, not a `begin`/`end` pair: this method returns early when
        // there is no overlay.
        let _span = self.trace.span(diagnostics::Phase::PaintPreparation);

        let viewport = Rectangle::with_size(self.bounds);
        renderer.reset(viewport);

        let base_cursor = match &self.overlay {
            None
            | Some(Overlay {
                interaction: mouse::Interaction::None,
                ..
            }) => cursor,
            _ => mouse::Cursor::Unavailable,
        };

        let retained_key = {
            let mut hasher = rustc_hash::FxHasher::default();
            self.state.id().hash(&mut hasher);
            hasher.finish()
        };
        let revision = self.state.revision().value();

        if !renderer.replay_retained_layer(retained_key, revision, viewport) {
            renderer.start_retained_layer(retained_key, revision, viewport);
            self.root.as_widget().draw(
                &self.state,
                renderer,
                theme,
                style,
                Layout::new(&self.base),
                base_cursor,
                &viewport,
            );
            renderer.end_retained_layer();
        }

        self.state.clear_dirty(widget::tree::DirtyFlags::PAINT);

        let Self {
            overlay,
            root,
            base,
            ..
        } = self;

        let Some(Overlay { layout, .. }) = overlay.as_ref() else {
            return;
        };

        let overlay = root
            .as_widget_mut()
            .overlay(
                &mut self.state,
                Layout::new(base),
                renderer,
                &viewport,
                Vector::ZERO,
            )
            .map(overlay::Nested::new);

        if let Some(mut overlay) = overlay {
            overlay.draw(renderer, theme, style, Layout::new(layout), cursor);
        }
    }

    /// Applies a [`widget::Operation`] to the [`UserInterface`].
    pub fn operate(&mut self, renderer: &Renderer, operation: &mut dyn widget::Operation) {
        let viewport = Rectangle::with_size(self.bounds);

        self.root.as_widget_mut().operate(
            &mut self.state,
            Layout::new(&self.base),
            renderer,
            operation,
        );

        if let Some(mut overlay) = self
            .root
            .as_widget_mut()
            .overlay(
                &mut self.state,
                Layout::new(&self.base),
                renderer,
                &viewport,
                Vector::ZERO,
            )
            .map(overlay::Nested::new)
        {
            if self.overlay.is_none() {
                self.overlay = Some(Overlay {
                    layout: overlay.layout(renderer, self.bounds),
                    interaction: mouse::Interaction::None,
                });
            }

            overlay.operate(
                Layout::new(&self.overlay.as_ref().unwrap().layout),
                renderer,
                operation,
            );
        }
    }

    /// Assembles the [`semantics::Node`] forest describing this
    /// [`UserInterface`] to assistive technology: the widget tree, plus
    /// any overlay currently on top of it.
    ///
    /// A forest rather than a single node because a root is often
    /// transparent — a plain `column` reports nothing of its own while
    /// several of its children report independently, and there is
    /// nothing above it to gather them under. An overlay contributes its
    /// own top-level nodes for the same reason: a dropdown is not a
    /// child of the widget tree it floats over.
    ///
    /// Subtrees whose `SEMANTICS` dirty flag is clear contribute cached
    /// nodes without their widgets being asked, so calling this per
    /// frame costs a walk, not a rebuild.
    ///
    /// This is the entry point an accessibility adapter drives: feed the
    /// result to `taim_accessibility::Tree`, diff it against the
    /// previous frame's, and hand the changes to the platform.
    ///
    /// [`semantics::Node`]: crate::core::semantics::Node
    pub fn semantics(&mut self, renderer: &Renderer) -> Vec<crate::core::semantics::Node> {
        self.trace.begin(diagnostics::Phase::Semantics);

        let viewport = Rectangle::with_size(self.bounds);

        let mut forest = crate::semantics::build(&mut self.visit());

        if let Some(mut overlay) = self
            .root
            .as_widget_mut()
            .overlay(
                &mut self.state,
                Layout::new(&self.base),
                renderer,
                &viewport,
                Vector::ZERO,
            )
            .map(overlay::Nested::new)
        {
            if self.overlay.is_none() {
                self.overlay = Some(Overlay {
                    layout: overlay.layout(renderer, self.bounds),
                    interaction: mouse::Interaction::None,
                });
            }

            forest.extend(overlay.semantics(
                Layout::new(&self.overlay.as_ref().unwrap().layout),
                renderer,
            ));
        }

        self.trace.end(diagnostics::Phase::Semantics);

        forest
    }

    /// Builds and diffs the semantic tree for the current frame.
    ///
    /// The returned changes are the platform-facing incremental payload. The
    /// previous tree is retained by the interface, so the running backend can
    /// call this after every update/draw cycle without rebuilding or
    /// re-announcing an unchanged UI.
    pub fn semantics_changes(
        &mut self,
        renderer: &Renderer,
    ) -> Vec<taim_accessibility::tree::Change> {
        let forest = self.semantics(renderer);
        let next = taim_accessibility::Tree::from_forest(&forest);
        let changes = self.semantic_tree.diff(&next);
        self.semantic_tree = next;
        changes
    }

    /// Returns the latest retained semantic tree.
    pub fn semantic_tree(&self) -> &taim_accessibility::Tree {
        &self.semantic_tree
    }

    /// Adapts this [`UserInterface`]'s widget tree to
    /// [`visit::Visit`], for running a whole-tree traversal over it.
    ///
    /// [`UserInterface::semantics`] and [`UserInterface::hit_test`] are
    /// this method's two callers, and are what most embedders want. This
    /// is public for one that needs to drive a traversal itself — most
    /// usefully, to query a [`hit_test::Index`] built earlier:
    ///
    /// ```ignore
    /// let index = ui.hit_test_index();       // once per layout pass
    /// let path = index.hit_test(&mut ui.visit(), point);  // per event
    /// ```
    ///
    /// Does not include any overlay: an overlay is not part of the widget
    /// tree (see [`UserInterface::semantics`], which appends it
    /// separately).
    pub fn visit(&mut self) -> visit::Widgets<'_, Message, Theme, Renderer> {
        visit::Widgets::new(
            self.root.as_widget_mut(),
            &mut self.state,
            Layout::new(&self.base),
        )
    }

    /// Finds the deepest widget containing `point`, as a
    /// [`hit_test::Path`].
    ///
    /// This is the exhaustive oracle: it walks the tree. For per-event
    /// queries, build a [`hit_test::Index`] once per layout pass with
    /// [`UserInterface::hit_test_index`] and query that instead — it
    /// agrees with this method by construction, since it narrows *which*
    /// widgets are asked without ever substituting a bounding box for a
    /// widget's own [`Widget::hit_test`].
    ///
    /// The resulting [`hit_test::Path`] is what
    /// [`UserInterface::route_pointer`] expects.
    ///
    /// [`Widget::hit_test`]: crate::core::Widget::hit_test
    pub fn hit_test(&mut self, point: Point) -> Option<hit_test::Path> {
        hit_test::hit_test(&mut self.visit(), point)
    }

    /// Builds a [`hit_test::Index`] over this [`UserInterface`]'s current
    /// layout, for querying many points against without re-walking the
    /// tree for each.
    ///
    /// The [`hit_test::Index`] is a snapshot: rebuild it whenever layout
    /// changes (see [`UserInterface::has_layout_changed`]), or it will
    /// answer against stale bounds.
    pub fn hit_test_index(&mut self) -> hit_test::Index {
        hit_test::Index::build(&mut self.visit())
    }

    /// Routes a single pointer-derived [`Event`] (see
    /// [`Event::to_pointer`]) through this [`UserInterface`]'s
    /// [`pointer_router::Router`], starting a fresh gesture arena at
    /// `path` on an unrecognized `Down` and returning [`Routing::None`]
    /// for any [`Event`] that yields no [`pointer::Pointer`] at all
    /// (see [`Event::to_pointer`]'s own documentation for which events
    /// those are).
    ///
    /// This is deliberately independent of [`UserInterface::update`]:
    /// deciding whether a pointer is captured, still competing, or just
    /// resolved never needs a full-tree [`Widget::update`] traversal --
    /// it only needs this event, the [`hit_test::Path`] the caller
    /// already found (e.g. via [`hit_test::Index::hit_test`], built once
    /// per layout pass rather than walked per event), and whatever
    /// [`Arena`] the caller registers recognizers into for a new
    /// contact. `phase.md`'s "integrate route without full-tree
    /// traversal" is satisfied by this method's own cost being
    /// independent of the widget tree's size, not by changing what
    /// [`UserInterface::update`] itself does.
    ///
    /// A caller still dispatches the event to widgets via
    /// [`UserInterface::update`] exactly as before; this method is
    /// additive routing information (which gesture, if any, now owns the
    /// pointer) for a caller that wants gesture recognition alongside
    /// -- not a replacement for the existing per-widget event path,
    /// since no [`Widget`] yet exposes the recognizers it would
    /// register into an [`Arena`] (a larger, separate `Widget` API
    /// question outside this method's scope).
    ///
    /// [`Arena`]: taim_gesture::arena::Arena
    /// [`hit_test::Path`]: crate::hit_test::Path
    /// [`hit_test::Index::hit_test`]: crate::hit_test::Index::hit_test
    /// [`Widget`]: crate::core::Widget
    /// [`Widget::update`]: crate::core::Widget::update
    pub fn route_pointer(
        &mut self,
        event: &Event,
        cursor: mouse::Cursor,
        timestamp: crate::core::time::Instant,
        start: impl FnOnce() -> Option<(crate::hit_test::Path, taim_gesture::arena::Arena)>,
    ) -> Routing {
        let Some(pointer) = event.to_pointer(cursor.position(), timestamp) else {
            return Routing::None;
        };

        let is_down = pointer.phase == crate::core::pointer::Phase::Down;
        let id = pointer.id;

        let routing = self.pointer_router.route(pointer);

        if is_down
            && matches!(routing, Routing::None)
            && let Some((path, arena)) = start()
        {
            self.pointer_router.start(id, path, arena);

            return self.pointer_router.route(
                event.to_pointer(cursor.position(), timestamp).expect(
                    "already produced a pointer above from the same event and cursor position",
                ),
            );
        }

        routing
    }

    /// Routes a pointer event through the default gesture arena.
    ///
    /// This is the safe baseline for widgets that have not registered a
    /// specialised recognizer yet: the contact is still tracked by the
    /// arena, preserves down/up/cancel ordering, and is released without
    /// being captured by a fabricated winner. Specialised widgets can use
    /// [`Self::route_pointer`] to register their recognizers at the hit path.
    pub fn route_pointer_default(
        &mut self,
        event: &Event,
        cursor: mouse::Cursor,
        timestamp: crate::core::time::Instant,
    ) -> Routing {
        let Some(pointer) = event.to_pointer(cursor.position(), timestamp) else {
            return Routing::None;
        };

        let start = if pointer.phase == crate::core::pointer::Phase::Down {
            self.hit_test(pointer.position)
                .map(|path| (path, Arena::new()))
        } else {
            None
        };

        self.route_pointer(event, cursor, timestamp, move || start)
    }

    /// Advances every still-competing gesture arena tracked by this
    /// [`UserInterface`]'s [`pointer_router::Router`] (see
    /// [`pointer_router::Router::tick`]), resolving any whose
    /// deadline-based recognizers (e.g. a long-press) have now elapsed
    /// with no new pointer event.
    pub fn tick_pointer_router(
        &mut self,
        now: crate::core::time::Instant,
    ) -> Vec<(crate::core::pointer::Id, Routing)> {
        self.pointer_router.tick(now)
    }

    /// Relayouts and returns a new  [`UserInterface`] using the provided
    /// bounds.
    pub fn relayout(self, bounds: Size, renderer: &mut Renderer) -> Self {
        Self::build(
            self.root,
            bounds,
            Cache {
                state: self.state,
                pointer_router: self.pointer_router,
                automatic_pointer_routing: self.automatic_pointer_routing,
                semantic_tree: self.semantic_tree,
                trace: self.trace,
            },
            renderer,
        )
    }

    /// Extract the [`Cache`] of the [`UserInterface`], consuming it in the
    /// process.
    pub fn into_cache(self) -> Cache {
        Cache {
            state: self.state,
            pointer_router: self.pointer_router,
            automatic_pointer_routing: self.automatic_pointer_routing,
            semantic_tree: self.semantic_tree,
            trace: self.trace,
        }
    }
}

#[cfg(test)]
mod event_batch_tests {
    use super::*;
    use crate::core::{Length, Widget};
    use std::cell::Cell;
    use std::rc::Rc;

    struct ClosingWidget {
        opened: Rc<Cell<bool>>,
        base_events: Rc<Cell<usize>>,
    }

    struct ClosingOverlay(Rc<Cell<bool>>);

    impl Widget<(), (), ()> for ClosingWidget {
        fn size(&self) -> Size<Length> {
            Size::new(Length::Fill, Length::Fill)
        }

        fn layout(&mut self, _: &mut widget::Tree, _: &(), _: &layout::Limits) -> layout::Node {
            layout::Node::new(Size::new(100.0, 100.0))
        }

        fn draw(
            &self,
            _: &widget::Tree,
            _: &mut (),
            _: &(),
            _: &renderer::Style,
            _: Layout<'_>,
            _: mouse::Cursor,
            _: &Rectangle,
        ) {
        }

        fn update(
            &mut self,
            _: &mut widget::Tree,
            _: &Event,
            _: Layout<'_>,
            _: mouse::Cursor,
            _: &(),
            _: &mut Shell<'_, ()>,
            _: &Rectangle,
        ) {
            self.base_events.set(self.base_events.get() + 1);
        }

        fn overlay<'a>(
            &'a mut self,
            _: &'a mut widget::Tree,
            _: Layout<'a>,
            _: &(),
            _: &Rectangle,
            _: Vector,
        ) -> Option<overlay::Element<'a, (), (), ()>> {
            self.opened
                .get()
                .then(|| overlay::Element::wrap(ClosingOverlay(self.opened.clone())))
        }
    }

    impl overlay::Overlay<(), (), ()> for ClosingOverlay {
        fn layout(&mut self, _: &(), _: Size) -> layout::Node {
            layout::Node::new(Size::new(100.0, 100.0))
        }

        fn draw(&self, _: &mut (), _: &(), _: &renderer::Style, _: Layout<'_>, _: mouse::Cursor) {}

        fn update(
            &mut self,
            _: &Event,
            _: Layout<'_>,
            _: mouse::Cursor,
            _: &(),
            shell: &mut Shell<'_, ()>,
        ) {
            self.0.set(false);
            shell.capture_event();
            shell.invalidate_layout();
        }
    }

    #[test]
    fn closing_overlay_mid_batch_preserves_later_events_and_invalidates_layout() {
        let opened = Rc::new(Cell::new(true));
        let base_events = Rc::new(Cell::new(0));
        let root = Element::new(ClosingWidget {
            opened: opened.clone(),
            base_events: base_events.clone(),
        });
        let mut renderer = ();
        let mut ui =
            UserInterface::build(root, Size::new(100.0, 100.0), Cache::new(), &mut renderer);
        let events = [
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        ];
        let mut messages = Vec::new();
        let (state, statuses) = ui.update(
            &window::Headless,
            &shell::Waker::noop(),
            &events,
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut messages,
        );

        assert_eq!(statuses, [event::Status::Captured, event::Status::Ignored]);
        assert_eq!(base_events.get(), 1);
        assert!(ui.overlay.is_none());
        assert!(matches!(
            state,
            State::Updated {
                has_layout_changed: true,
                ..
            }
        ));
    }

    #[test]
    fn removed_overlay_does_not_survive_an_empty_event_batch() {
        let opened = Rc::new(Cell::new(true));
        let root = Element::new(ClosingWidget {
            opened: opened.clone(),
            base_events: Rc::new(Cell::new(0)),
        });
        let mut renderer = ();
        let mut ui =
            UserInterface::build(root, Size::new(100.0, 100.0), Cache::new(), &mut renderer);
        let mut messages = Vec::new();
        let _ = ui.update(
            &window::Headless,
            &shell::Waker::noop(),
            &[],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut messages,
        );
        assert!(ui.overlay.is_some());
        opened.set(false);
        let _ = ui.update(
            &window::Headless,
            &shell::Waker::noop(),
            &[],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut messages,
        );
        assert!(ui.overlay.is_none());
    }
}

/// Reusable data of a specific [`UserInterface`].
#[derive(Debug)]
pub struct Cache {
    state: widget::Tree,
    pointer_router: pointer_router::Router,
    automatic_pointer_routing: bool,
    semantic_tree: taim_accessibility::Tree,
    /// Carried through the rebuild so a trace survives it.
    ///
    /// `build` runs `Diff` and `Layout` before a [`UserInterface`] exists
    /// to hold anything, so those two phases can only be recorded through
    /// something the caller already handed over — and a profiler that
    /// could see every phase *except* the two that run at startup would
    /// be missing exactly the ones a slow first frame is made of.
    trace: diagnostics::Trace,
}

impl Cache {
    /// Creates an empty [`Cache`].
    ///
    /// You should use this to initialize a [`Cache`] before building your first
    /// [`UserInterface`].
    pub fn new() -> Cache {
        Cache {
            state: widget::Tree::empty(),
            pointer_router: pointer_router::Router::new(),
            automatic_pointer_routing: true,
            semantic_tree: taim_accessibility::Tree::new(),
            trace: diagnostics::Trace::new(taim_profiler::Collector::disabled()),
        }
    }

    /// Records the phases of every [`UserInterface`] built from this
    /// [`Cache`] into `trace`.
    ///
    /// Set here rather than only on the [`UserInterface`] because `Diff`
    /// and `Layout` run inside [`UserInterface::build`], before there is a
    /// [`UserInterface`] to attach anything to.
    #[must_use]
    pub fn with_trace(mut self, trace: diagnostics::Trace) -> Self {
        self.trace = trace;
        self
    }

    /// Configures automatic pointer routing for interfaces built from this cache.
    #[must_use]
    pub fn with_automatic_pointer_routing(mut self, enabled: bool) -> Self {
        self.automatic_pointer_routing = enabled;
        self
    }
}

impl Default for Cache {
    fn default() -> Cache {
        Cache::new()
    }
}

/// The resulting state after updating a [`UserInterface`].
#[derive(Debug)]
pub enum State {
    /// The [`UserInterface`] is outdated and needs to be rebuilt.
    Outdated,

    /// The [`UserInterface`] is up-to-date and can be reused without
    /// rebuilding.
    Updated {
        /// The current [`mouse::Interaction`] of the user interface.
        mouse_interaction: mouse::Interaction,
        /// The [`window::RedrawRequest`] describing when a redraw should be performed.
        redraw_request: window::RedrawRequest,
        /// The current [`InputMethod`] strategy of the user interface.
        input_method: InputMethod,
        /// The set of [`Clipboard`] requests that the user interface has produced.
        clipboard: Clipboard,
        /// Whether the layout of the [`UserInterface`] has changed.
        has_layout_changed: bool,
        /// The aggregated damage from this frame's updates, **and why**.
        ///
        /// When present, this tells the compositor which parts of the
        /// surface need redrawing, and tells a developer why. The
        /// compositor can use the region for partial presentation if the
        /// backend supports it, and ignore the rest.
        ///
        /// `None` means "no damage was recorded" (either nothing changed,
        /// or damage tracking was not active). A
        /// [`damage::Region::Full`] means the entire surface needs
        /// redrawing — and [`damage::Attribution::fallback`] says whether
        /// that was a caller's request or damage tracking giving up.
        damage: Option<damage::Attribution>,
        /// Whether continuous redraws were requested this update (see
        /// [`crate::core::Shell::request_animation`]), rather than a
        /// single one-shot redraw.
        ///
        /// A frame scheduler (`runtime::frame_scheduler::Scheduler`)
        /// reads this to decide whether to enter its continuous
        /// animating state or schedule a single deadline.
        is_animating: bool,
    },
}

impl State {
    /// Returns whether the layout of the [`UserInterface`] has changed.
    pub fn has_layout_changed(&self) -> bool {
        match self {
            State::Outdated => true,
            State::Updated {
                has_layout_changed, ..
            } => *has_layout_changed,
        }
    }

    /// Returns this frame's damage and its causes, if any.
    ///
    /// Prefer this to [`State::damage_region`] anywhere the answer might
    /// be [`damage::Region::Full`]: "redraw everything" is not a finding,
    /// and "redraw everything because the layout changed" is.
    ///
    /// [`State::Outdated`] attributes to [`damage::Cause::Layout`]: an
    /// outdated interface is one whose view must be rebuilt, which is a
    /// structural change by definition.
    pub fn damage(&self) -> Option<damage::Attribution> {
        match self {
            State::Outdated => Some(damage::Attribution {
                region: damage::Region::Full,
                causes: damage::Causes::LAYOUT,
                fallback: Some(damage::Fallback::Requested(damage::Cause::Layout)),
            }),
            State::Updated { damage, .. } => *damage,
        }
    }

    /// Returns the damage region from this frame's updates, if any.
    ///
    /// The geometry alone, for a compositor that only needs to know what
    /// to present. A caller diagnosing a *cost* wants [`State::damage`].
    pub fn damage_region(&self) -> Option<damage::Region> {
        Some(self.damage()?.region)
    }

    /// Returns whether continuous redraws were requested this update.
    ///
    /// Returns `false` for [`State::Outdated`]: an outdated interface
    /// needs to be rebuilt before anything can be known about whether it
    /// is animating, and defaulting to `false` (rather than assuming
    /// `true`) avoids forcing a continuous redraw loop from a state that
    /// says nothing about animation either way.
    pub fn is_animating(&self) -> bool {
        match self {
            State::Outdated => false,
            State::Updated { is_animating, .. } => *is_animating,
        }
    }
}
