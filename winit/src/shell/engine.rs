//! The application side of the shell: one cooperative task that owns the
//! application, its windows and the async runtime.
use super::frame;
use super::inbox::Inbox;
use super::protocol::{NativeWindow, Request, Signal};
use super::views::{self, Views};
use crate::clipboard::Clipboard;
use crate::conversion;
use crate::core::time::Instant;
use crate::core::{self, Renderer as _, backend, renderer, theme, window};
use crate::debug;
use crate::frame_gate::{self, DirtyFlags, FrameGate, PlatformEvent};
use crate::futures::futures::channel::{mpsc, oneshot};
use crate::futures::futures::{StreamExt, task};
use crate::futures::{Runtime, subscription};
use crate::graphics::{Compositor, Shell, compositor};
use crate::program::{self, App, AppRunner};
use crate::runtime::{self, Action, user_interface};
use crate::{Proxy, window::Manager};

use std::borrow::Cow;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::ControlFlow;

pub(super) type AppRuntime<A> =
    Runtime<<A as App>::Executor, Proxy<<A as App>::Message>, Action<<A as App>::Message>>;

/// The renderer backend an application gets when it does not choose one.
pub(super) type DefaultCompositor<A> = <<A as App>::Renderer as compositor::Default>::Compositor;

/// Everything the engine owns except the application and its interfaces,
/// which live on the [`drive`] task so interfaces can borrow the application.
pub(super) struct Engine<A: App, C: Compositor<Renderer = A::Renderer>>
where
    A::Theme: theme::Base,
{
    pub(super) runtime: AppRuntime<A>,
    pub(super) proxy: Proxy<A::Message>,
    requests: mpsc::UnboundedSender<Request>,

    pub(super) compositor: Option<C>,
    pub(super) windows: Manager<A, C>,
    pub(super) frames: FrameGate,

    pub(super) inbox: Inbox,
    pub(super) messages: Vec<A::Message>,
    /// Actions produced synchronously by the last update, run once the
    /// interfaces have been rebuilt.
    pub(super) staged: Vec<Action<A::Message>>,
    /// Outputs produced synchronously by the last update, fed back into it.
    outputs: Vec<A::Message>,
    /// Proxy actions handled since the last idle turn; their queue slots are
    /// released together.
    handled_actions: usize,

    pub(super) clipboard: Clipboard,
    pub(super) system_theme: theme::Mode,
    pub(super) renderer_settings: renderer::Settings,
    pub(super) opening_window: bool,

    is_daemon: bool,
    display: winit::event_loop::OwnedDisplayHandle,
    backend_settings: backend::Settings,
    fonts: Vec<Cow<'static, [u8]>>,
}

/// Static configuration the engine starts from.
pub(super) struct Setup {
    pub is_daemon: bool,
    pub display: winit::event_loop::OwnedDisplayHandle,
    pub backend_settings: backend::Settings,
    pub renderer_settings: renderer::Settings,
    pub fonts: Vec<Cow<'static, [u8]>>,
}

/// Runs the application until the host shuts down or the last window closes.
pub(super) async fn drive<A>(
    mut runner: AppRunner<A>,
    runtime: AppRuntime<A>,
    proxy: Proxy<A::Message>,
    mut signals: mpsc::UnboundedReceiver<Signal<A::Message>>,
    requests: mpsc::UnboundedSender<Request>,
    first_theme: oneshot::Receiver<theme::Mode>,
    setup: Setup,
) where
    A: App + 'static,
    A::Theme: theme::Base,
{
    let mut engine = Engine::<A, DefaultCompositor<A>>::new(runtime, proxy, requests, setup);
    engine.detect_system_theme(first_theme);

    let mut views = Views::new();

    'signals: loop {
        let _ = engine.runtime.poll_tasks();

        let signal = match signals.try_recv() {
            Ok(signal) => signal,
            Err(_) => match signals.next().await {
                Some(signal) => signal,
                None => break,
            },
        };

        match signal {
            Signal::WindowReady(window) => engine.attach(&runner, &mut views, window).await,
            Signal::Woke(StartCause::Init) => engine.redraw_all(),
            Signal::Woke(StartCause::ResumeTimeReached { .. }) => engine.on_deadline(),
            Signal::Woke(_) => {}
            Signal::Action(action) => {
                engine.perform(action, &runner, &mut views);
                engine.handled_actions += 1;
            }
            Signal::Window {
                id,
                event: WindowEvent::RedrawRequested,
            } => {
                log::debug!("RedrawRequested for {id:?}");

                let Some(mut redraw) = engine.begin_redraw(&mut views, id) else {
                    continue;
                };

                let mut passes = 0;
                let outcome = loop {
                    let Some((outcome, settled)) = engine.deliver_redraw(&mut views, &redraw)
                    else {
                        continue 'signals;
                    };

                    if settled {
                        break outcome;
                    }

                    if passes == 2 {
                        log::warn!("three redraw passes in a row kept invalidating layout");
                        break outcome;
                    }
                    passes += 1;

                    if engine.messages.is_empty() {
                        continue;
                    }

                    let caches = views.recycle();
                    engine.update(&mut runner);
                    views = Views::rebuild(&runner, &mut engine.windows, caches, &mut engine.proxy);
                    engine.after_redraw_update(&runner, &mut views, redraw.id);

                    if engine.compositor.is_none() || !engine.follow_resize(&mut views, &mut redraw)
                    {
                        continue 'signals;
                    }
                };

                engine.finish_redraw(&mut views, &redraw, outcome);
            }
            Signal::Window { id, event } => engine.on_window_event(&runner, &mut views, id, event),
            Signal::Idle => {
                if engine.handled_actions > 0 {
                    engine.proxy.free_slots(engine.handled_actions);
                    engine.handled_actions = 0;
                }

                if engine.is_quiescent() {
                    continue;
                }

                let interfaces_stale = engine.dispatch_input(&mut views);

                if !engine.messages.is_empty() || interfaces_stale {
                    let caches = views.recycle();
                    engine.update(&mut runner);

                    // Native intent handlers report what they invalidated;
                    // compatibility programs are repainted after every batch.
                    let explicit = runner.controls_invalidation();
                    let delta = runner.context_mut().take_invalidation();

                    views = Views::rebuild(&runner, &mut engine.windows, caches, &mut engine.proxy);
                    engine.run_staged(&runner, &mut views);
                    engine.invalidate_after_batch(explicit, interfaces_stale, delta);
                }

                engine.schedule_wake();
            }
            Signal::Shutdown => break,
        }
    }

    drop(views);
}

impl<A, C> Engine<A, C>
where
    A: App,
    A::Theme: theme::Base,
    C: Compositor<Renderer = A::Renderer> + 'static,
{
    fn new(
        runtime: AppRuntime<A>,
        proxy: Proxy<A::Message>,
        requests: mpsc::UnboundedSender<Request>,
        setup: Setup,
    ) -> Self {
        let mut engine = Self {
            runtime,
            proxy,
            requests,
            compositor: None,
            windows: Manager::new(),
            frames: FrameGate::new(),
            inbox: Inbox::default(),
            messages: Vec::new(),
            staged: Vec::new(),
            outputs: Vec::new(),
            handled_actions: 0,
            clipboard: Clipboard::new(),
            system_theme: theme::Mode::default(),
            renderer_settings: setup.renderer_settings,
            opening_window: !setup.is_daemon,
            is_daemon: setup.is_daemon,
            display: setup.display,
            backend_settings: setup.backend_settings,
            fonts: setup.fonts,
        };

        // Timers and animation deadlines wake the event loop through the
        // proxy, which the frame scheduler turns into renderer ticks.
        let waker = engine.proxy.clone();
        engine
            .runtime
            .scheduler_mut()
            .set_notifier(move || waker.send_action(Action::Tick));

        engine
    }

    pub(super) fn request(&self, request: Request) {
        let _ = self.requests.unbounded_send(request);
    }

    /// Learns the system color scheme: from the desktop portal when
    /// `linux-theme-detection` is enabled (and keeps following it), otherwise
    /// from what the host saw on its first resume.
    fn detect_system_theme(&mut self, _first_theme: oneshot::Receiver<theme::Mode>) {
        #[cfg(all(feature = "linux-theme-detection", target_os = "linux"))]
        {
            let to_mode = |scheme| match scheme {
                mundy::ColorScheme::NoPreference => theme::Mode::None,
                mundy::ColorScheme::Light => theme::Mode::Light,
                mundy::ColorScheme::Dark => theme::Mode::Dark,
            };

            let _ = self.runtime.run(
                mundy::Preferences::stream(mundy::Interest::ColorScheme)
                    .map(move |preferences| {
                        Action::System(runtime::system::Action::NotifyTheme(to_mode(
                            preferences.color_scheme,
                        )))
                    })
                    .boxed(),
            );

            self.system_theme = self
                .runtime
                .enter(|| {
                    mundy::Preferences::once_blocking(
                        mundy::Interest::ColorScheme,
                        core::time::Duration::from_millis(200),
                    )
                })
                .map(|preferences| to_mode(preferences.color_scheme))
                .unwrap_or_default();
        }

        #[cfg(not(all(feature = "linux-theme-detection", target_os = "linux")))]
        {
            let mut first_theme = _first_theme;
            self.system_theme = first_theme.try_recv().ok().flatten().unwrap_or_default();
        }

        log::info!("System theme: {:?}", self.system_theme);
    }

    /// Records a system theme change and tells subscriptions about it.
    pub(super) fn set_system_theme(&mut self, mode: theme::Mode) {
        if mode != self.system_theme {
            self.system_theme = mode;
            self.runtime
                .broadcast(subscription::Event::SystemThemeChanged(mode));
        }
    }

    /// Brings a native window to life: compositor (on first use), state,
    /// renderer, interface, and its first frame.
    async fn attach<'a>(
        &mut self,
        runner: &'a AppRunner<A>,
        views: &mut Views<'a, A>,
        window: NativeWindow,
    ) {
        if self.compositor.is_none() {
            match self.create_compositor(&window).await {
                Ok(compositor) => self.compositor = Some(compositor),
                Err(error) => {
                    self.request(Request::Fail(error.into()));
                    return;
                }
            }
        }

        let NativeWindow {
            id,
            raw,
            exit_on_close_request,
            reveal,
            opened,
        } = window;

        let window_theme = raw.theme().map(conversion::theme_mode).unwrap_or_default();
        self.set_system_theme(window_theme);

        let is_first = self.windows.is_empty();
        let compositor = self.compositor.as_mut().expect("compositor created above");
        let window = self.windows.insert(
            id,
            raw,
            runner,
            compositor,
            self.proxy.clone(),
            self.renderer_settings,
            exit_on_close_request,
            self.system_theme,
        );

        self.frames
            .track_window(id, window.raw.scale_factor() as f32);
        let surface = window.state.physical_size();
        if surface.width == 0 || surface.height == 0 {
            let _ = self
                .frames
                .handle_event(id, PlatformEvent::SurfaceSizeChanged { zero_sized: true });
        }

        window
            .raw
            .set_theme(conversion::window_theme(window.state.theme_mode()));

        debug::theme_changed(|| {
            is_first
                .then(|| theme::Base::seed(window.state.theme()))
                .flatten()
        });

        #[cfg(feature = "hinting")]
        window.renderer.hint(window.state.scale_factor());

        // One light per window, owned by the theme: materials respond to it
        // whether or not metrics are hinted.
        window
            .renderer
            .set_light(theme::Base::light(window.state.theme()));

        let mut interface = views::build(
            runner,
            user_interface::Cache::default(),
            &mut window.renderer,
            window.state.logical_size(),
            id,
            window.state.scale_factor(),
        );
        window.hit_test_index = Some(interface.hit_test_index());
        views.insert(id, interface);

        if reveal {
            window.raw.set_visible(true);
        }

        // A new window's scheduler is idle and would ignore every
        // `RedrawRequested` until the first input. Asking for the first frame
        // through the scheduler moves it to "requested", so the platform's
        // initial redraw reaches `present`.
        window.request_redraw(window::RedrawRequest::NextFrame);

        let opened_event = core::Event::Window(window::Event::Opened {
            position: window.position(),
            size: window.state.logical_size(),
            scale_factor: window.raw.scale_factor() as f32,
        });
        self.inbox.push(id, opened_event);

        let _ = opened.send(id);
        self.opening_window = false;
    }

    /// Creates the compositor against the first window.
    ///
    /// Native targets block on it; the web spawns it and waits, since a
    /// browser cannot block. Either way the result comes back over a channel
    /// so both paths share one shape.
    async fn create_compositor(&mut self, window: &NativeWindow) -> Result<C, backend::Error> {
        let (sender, receiver) = oneshot::channel();

        let creation = {
            let raw = window.raw.clone();
            let settings = self.backend_settings.clone();
            let display = self.display.clone();
            let proxy = self.proxy.clone();
            let fonts = self.fonts.clone();

            async move {
                let mut compositor =
                    C::new(settings, display, raw, Shell::new(proxy.clone())).await;

                if let Ok(compositor) = &mut compositor {
                    for font in fonts {
                        compositor.load_font(font);
                    }
                }

                let _ = sender.send(compositor);

                // Nudge the event loop so the engine is polled again and
                // picks up the result. Any cheap round-trip action will do.
                let (reply, _) = oneshot::channel();
                proxy.send_action(Action::Window(runtime::window::Action::GetLatest(reply)));
            }
        };

        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(creation);

        #[cfg(not(target_arch = "wasm32"))]
        self.runtime.block_on(creation);

        receiver
            .await
            .expect("compositor creation always reports back")
    }

    pub(super) fn redraw_all(&mut self) {
        for (_, window) in self.windows.iter_mut() {
            window.request_redraw(window::RedrawRequest::NextFrame);
        }
    }

    /// A deadline armed by [`schedule_wake`](Self::schedule_wake) fired.
    fn on_deadline(&mut self) {
        let now = Instant::now();

        for (_, window) in self.windows.iter_mut() {
            window.scheduler.tick(now);

            if window.scheduler.should_produce_frame(now) {
                window.raw.request_redraw();
            }
        }

        let flow = self
            .windows
            .redraw_at()
            .map_or(ControlFlow::Wait, ControlFlow::WaitUntil);
        self.request(Request::Flow(flow));
    }

    fn on_window_event<'a>(
        &mut self,
        runner: &'a AppRunner<A>,
        views: &mut Views<'a, A>,
        platform_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        log::trace!("WindowEvent for {platform_id:?}: {event:?}");

        let last_window_gone = !self.is_daemon
            && matches!(event, WindowEvent::Destroyed)
            && !self.opening_window
            && self.windows.is_empty();

        if last_window_gone {
            self.request(Request::Quit);
            return;
        }

        let Some((id, window)) = self.windows.get_mut_alias(platform_id) else {
            return;
        };

        // The frame gate sees the event before any invalidation it causes.
        if let Some(observation) = observe(&event) {
            let _ = self.frames.handle_event(id, observation);
        }

        match &event {
            WindowEvent::Resized(size) => {
                window.set_zero_sized(size.width == 0 || size.height == 0);
                self.frames
                    .invalidate(id, DirtyFlags::LAYOUT_DIRTY | DirtyFlags::PAINT_DIRTY);
            }
            WindowEvent::Occluded(occluded) => window.set_occluded(*occluded),
            _ => {}
        }

        if wants_repaint(&event) {
            window.request_redraw(window::RedrawRequest::NextFrame);
        }

        let closes = matches!(event, WindowEvent::CloseRequested) && window.exit_on_close_request;

        if let WindowEvent::ThemeChanged(theme) = &event {
            self.set_system_theme(conversion::theme_mode(*theme));
        }

        if closes {
            self.perform(
                Action::Window(runtime::window::Action::Close(id)),
                runner,
                views,
            );
        } else {
            self.translate(runner, id, event);
        }
    }

    /// Updates the window state from `event` and queues its TAIM translation.
    fn translate(&mut self, runner: &AppRunner<A>, id: window::Id, event: WindowEvent) {
        let Some(window) = self.windows.get_mut(id) else {
            return;
        };

        window.state.update(runner, &window.raw, &event);

        if let Some(event) =
            conversion::window_event(event, window.state.scale_factor(), window.state.modifiers())
        {
            self.inbox.push(id, event);
        }
    }

    /// Nothing queued, nothing to say, nothing scheduled.
    fn is_quiescent(&self) -> bool {
        self.inbox.is_empty()
            && self.messages.is_empty()
            && self.windows.is_idle()
            && self.frames.is_idle()
    }

    /// Routes queued input through every window's interface.
    ///
    /// Returns whether any interface reported itself outdated and needs a
    /// rebuild regardless of messages.
    fn dispatch_input(&mut self, views: &mut Views<'_, A>) -> bool {
        let mut stale = false;

        for (id, window) in self.windows.iter_mut() {
            let interact_span = debug::interact(id);

            let Some(events) = self.inbox.pending_for(id) else {
                continue;
            };
            let Some(interface) = views.get_mut(id) else {
                continue;
            };

            let cursor = window.state.cursor();
            let now = Instant::now();

            for event in events.iter() {
                // Only a press starts a gesture, so only a press hit-tests.
                let target = event
                    .to_pointer(cursor.position(), now)
                    .filter(|pointer| pointer.phase == core::pointer::Phase::Down)
                    .and_then(|pointer| {
                        window
                            .hit_test_index
                            .as_mut()?
                            .hit_test(&mut interface.visit(), pointer.position)
                    });

                let _ = interface.route_pointer(event, cursor, now, || {
                    target.map(|path| (path, taim_gesture::arena::Arena::new()))
                });
            }

            let (outcome, statuses) = interface.update(
                &window.raw,
                &window.waker,
                events,
                cursor,
                &mut window.renderer,
                &mut self.messages,
            );

            #[cfg(feature = "unconditional-rendering")]
            window.request_redraw(window::RedrawRequest::NextFrame);

            match outcome {
                user_interface::State::Updated {
                    redraw_request: _redraw_request,
                    mouse_interaction,
                    clipboard,
                    is_animating,
                    ..
                } => {
                    window.update_mouse(mouse_interaction);

                    #[cfg(not(feature = "unconditional-rendering"))]
                    window.request_redraw(_redraw_request);

                    if is_animating {
                        window.scheduler.request_animation();
                        window.raw.request_redraw();
                    } else {
                        window.scheduler.stop_animating();
                    }

                    frame::serve_clipboard(&mut self.proxy, &mut self.clipboard, clipboard, id);
                }
                user_interface::State::Outdated => stale = true,
            }

            for (event, status) in events.drain(..).zip(statuses) {
                self.runtime.broadcast(subscription::Event::Interaction {
                    window: id,
                    event,
                    status,
                });
            }

            interact_span.finish();
        }

        // Events for windows without an interface still reach subscriptions.
        let runtime = &mut self.runtime;
        self.inbox.drain_all(|window, event| {
            runtime.broadcast(subscription::Event::Interaction {
                window,
                event,
                status: core::event::Status::Ignored,
            });
        });
        self.inbox.finish_batch();

        let windows = &self.windows;
        self.inbox.retain_windows(|id| windows.get(id).is_some());

        stale
    }

    /// Feeds queued messages to the application until it stops producing
    /// synchronous output, then refreshes its subscriptions.
    pub(super) fn update(&mut self, runner: &mut AppRunner<A>) {
        debug_assert!(self.staged.is_empty());
        debug_assert!(self.outputs.is_empty());

        let mut context = task::Context::from_waker(task::noop_waker_ref());

        while !self.messages.is_empty() {
            for message in self.messages.drain(..) {
                let task = self.runtime.enter(|| runner.update(message));
                let Some(mut stream) = runtime::task::into_stream(task) else {
                    continue;
                };

                // Take whatever the task can produce right now (widget
                // operations, immediate outputs); leave the rest running.
                loop {
                    match self.runtime.enter(|| stream.poll_next_unpin(&mut context)) {
                        task::Poll::Ready(Some(Action::Output(output))) => {
                            self.outputs.push(output);
                        }
                        task::Poll::Ready(Some(action)) => self.staged.push(action),
                        task::Poll::Ready(None) => break,
                        task::Poll::Pending => {
                            let _ = self.runtime.run(stream);
                            break;
                        }
                    }
                }
            }

            self.messages.append(&mut self.outputs);
        }

        let subscription = self.runtime.enter(|| runner.subscription());
        self.runtime
            .track(subscription::into_recipes(subscription.map(Action::Output)));
    }

    /// Runs the actions staged by the last [`update`](Self::update).
    fn run_staged<'a>(&mut self, runner: &'a AppRunner<A>, views: &mut Views<'a, A>) {
        for action in std::mem::take(&mut self.staged) {
            self.perform(action, runner, views);
        }
    }

    /// Marks every window dirty according to what the batch invalidated.
    fn invalidate_after_batch(
        &mut self,
        explicit: bool,
        interfaces_stale: bool,
        delta: program::StateDelta,
    ) {
        let dirty = frame_gate::after_message_batch(explicit, interfaces_stale, delta);
        if dirty.is_empty() {
            return;
        }

        let animate = explicit && delta.frame == program::FrameRequest::Animate;

        for (id, window) in self.windows.iter_mut() {
            self.frames.invalidate(id, dirty);

            if animate {
                window.scheduler.request_animation();
            }

            window.request_redraw(window::RedrawRequest::NextFrame);
        }
    }

    /// Tells the host how long it may sleep.
    fn schedule_wake(&self) {
        let flow = if self.frames.has_immediate_work() {
            ControlFlow::Poll
        } else {
            self.windows
                .redraw_at()
                .map_or(ControlFlow::Wait, ControlFlow::WaitUntil)
        };

        self.request(Request::Flow(flow));
    }
}

/// What the frame gate needs to know about a platform window event.
fn observe(event: &WindowEvent) -> Option<PlatformEvent> {
    Some(match event {
        WindowEvent::Resized(size) => PlatformEvent::SurfaceSizeChanged {
            zero_sized: size.width == 0 || size.height == 0,
        },
        WindowEvent::Focused(focused) => PlatformEvent::FocusChanged(*focused),
        WindowEvent::Occluded(occluded) => PlatformEvent::OcclusionChanged(*occluded),
        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
            PlatformEvent::ScaleFactorChanged(*scale_factor as f32)
        }
        WindowEvent::CursorMoved { position, .. } => PlatformEvent::CursorMoved {
            x: position.x as f32,
            y: position.y as f32,
        },
        WindowEvent::ThemeChanged(_) => PlatformEvent::ThemeChanged,
        _ => return None,
    })
}

/// Platform events after which the window's content must be painted again.
fn wants_repaint(event: &WindowEvent) -> bool {
    matches!(
        event,
        WindowEvent::Resized(_)
            | WindowEvent::Focused(_)
            | WindowEvent::Occluded(false)
            | WindowEvent::ScaleFactorChanged { .. }
            | WindowEvent::ThemeChanged(_)
    )
}
