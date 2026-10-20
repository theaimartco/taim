use super::instance::run_instance;
use super::*;

/// Runs a [`Program`] with the provided settings.
///
/// In TAIM Phase 3, this legacy entry point is an adapter that runs the
/// program through the authoritative [`AppRunner`] execution engine.
pub fn run<P>(program: P) -> Result<(), Error>
where
    P: Program + 'static,
    P::Theme: theme::Base,
{
    run_app(crate::program::compat::ProgramAdapter::new(program))
}

/// Runs a native TAIM [`App`](program::App) with the provided settings.
pub fn run_app<A>(app: A) -> Result<(), Error>
where
    A: program::App + 'static,
    A::Theme: theme::Base,
{
    use winit::event_loop::EventLoop;

    let boot_span = debug::boot();
    let config = app.config();
    let settings = config.settings;
    let window_settings = config.window;

    let event_loop = EventLoop::with_user_event()
        .build()
        .expect("Create event loop");

    let backend_settings = backend::Settings::from(&settings);
    let renderer_settings = renderer::Settings::from(&settings);
    let display_handle = event_loop.owned_display_handle();

    let (proxy, worker) = Proxy::new(event_loop.create_proxy());

    #[cfg(feature = "debug")]
    {
        let proxy = proxy.clone();

        debug::on_hotpatch(move || {
            proxy.send_action(Action::Reload);
        });
    }

    let mut runtime = {
        let executor = A::Executor::new().map_err(Error::ExecutorCreationFailed)?;
        executor.spawn(worker);

        Runtime::new(executor, proxy.clone())
    };

    let (runner, task) = runtime.enter(|| program::AppRunner::new(app));
    let is_daemon = window_settings.is_none();

    let task = if let Some(window_settings) = window_settings {
        let mut task = Some(task);

        let (_id, open) = runtime::window::open(window_settings);

        open.then(move |_| task.take().unwrap_or_else(Task::none))
    } else {
        task
    };

    if let Some(stream) = runtime::task::into_stream(task) {
        let _ = runtime.run(stream);
    }

    runtime.track(subscription::into_recipes(
        runtime.enter(|| runner.subscription().map(Action::Output)),
    ));

    let (event_sender, event_receiver) = mpsc::unbounded();
    let (control_sender, control_receiver) = mpsc::unbounded();
    let (system_theme_sender, system_theme_receiver) = oneshot::channel();

    let instance = Box::pin(run_instance::<A>(
        runner,
        runtime,
        proxy.clone(),
        event_receiver,
        control_sender,
        display_handle,
        is_daemon,
        backend_settings,
        renderer_settings,
        settings.fonts,
        system_theme_receiver,
    ));

    let context = task::Context::from_waker(task::noop_waker_ref());

    struct Runner<Message: 'static, F> {
        instance: std::pin::Pin<Box<F>>,
        context: task::Context<'static>,
        id: Option<String>,
        sender: mpsc::UnboundedSender<Event<Action<Message>>>,
        receiver: mpsc::UnboundedReceiver<Control>,
        error: Option<Error>,
        system_theme: Option<oneshot::Sender<theme::Mode>>,

        #[cfg(target_arch = "wasm32")]
        canvas: Option<web_sys::HtmlCanvasElement>,
    }

    let runner = Runner {
        instance,
        context,
        id: settings.id,
        sender: event_sender,
        receiver: control_receiver,
        error: None,
        system_theme: Some(system_theme_sender),

        #[cfg(target_arch = "wasm32")]
        canvas: None,
    };

    boot_span.finish();

    impl<Message, F> winit::application::ApplicationHandler<Action<Message>> for Runner<Message, F>
    where
        F: Future<Output = ()>,
    {
        fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
            if let Some(sender) = self.system_theme.take() {
                let _ = sender.send(
                    event_loop
                        .system_theme()
                        .map(conversion::theme_mode)
                        .unwrap_or_default(),
                );
            }
        }

        fn new_events(
            &mut self,
            event_loop: &winit::event_loop::ActiveEventLoop,
            cause: winit::event::StartCause,
        ) {
            self.process_event(
                event_loop,
                Event::EventLoopAwakened(winit::event::Event::NewEvents(cause)),
            );
        }

        fn window_event(
            &mut self,
            event_loop: &winit::event_loop::ActiveEventLoop,
            window_id: winit::window::WindowId,
            event: winit::event::WindowEvent,
        ) {
            #[cfg(target_os = "windows")]
            let is_move_or_resize = matches!(
                event,
                winit::event::WindowEvent::Resized(_) | winit::event::WindowEvent::Moved(_)
            );

            self.process_event(
                event_loop,
                Event::EventLoopAwakened(winit::event::Event::WindowEvent { window_id, event }),
            );

            // TODO: Remove when unnecessary
            // On Windows, we emulate an `AboutToWait` event after every `Resized` event
            // since the event loop does not resume during resize interaction.
            // More details: https://github.com/rust-windowing/winit/issues/3272
            #[cfg(target_os = "windows")]
            {
                if is_move_or_resize {
                    self.process_event(
                        event_loop,
                        Event::EventLoopAwakened(winit::event::Event::AboutToWait),
                    );
                }
            }
        }

        fn user_event(
            &mut self,
            event_loop: &winit::event_loop::ActiveEventLoop,
            action: Action<Message>,
        ) {
            self.process_event(
                event_loop,
                Event::EventLoopAwakened(winit::event::Event::UserEvent(action)),
            );
        }

        fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
            self.process_event(
                event_loop,
                Event::EventLoopAwakened(winit::event::Event::AboutToWait),
            );
        }
    }

    impl<Message, F> Runner<Message, F>
    where
        F: Future<Output = ()>,
    {
        fn process_event(
            &mut self,
            event_loop: &winit::event_loop::ActiveEventLoop,
            event: Event<Action<Message>>,
        ) {
            if event_loop.exiting() {
                return;
            }

            self.sender.start_send(event).expect("Send event");

            loop {
                let poll = self.instance.as_mut().poll(&mut self.context);

                match poll {
                    task::Poll::Pending => match self.receiver.try_recv() {
                        Ok(control) => match control {
                            Control::ChangeFlow(flow) => {
                                use winit::event_loop::ControlFlow;

                                match (event_loop.control_flow(), flow) {
                                    (
                                        ControlFlow::WaitUntil(current),
                                        ControlFlow::WaitUntil(new),
                                    ) if current < new => {}
                                    (ControlFlow::WaitUntil(target), ControlFlow::Wait)
                                        if target > Instant::now() => {}
                                    _ => {
                                        event_loop.set_control_flow(flow);
                                    }
                                }
                            }
                            Control::CreateWindow {
                                id,
                                settings,
                                title,
                                scale_factor,
                                monitor,
                                on_open,
                            } => {
                                let exit_on_close_request = settings.exit_on_close_request;

                                let visible = settings.visible;

                                #[cfg(target_arch = "wasm32")]
                                let target = settings.platform_specific.target.clone();

                                let window_attributes = conversion::window_attributes(
                                    settings,
                                    &title,
                                    scale_factor,
                                    monitor.or(event_loop.primary_monitor()),
                                    self.id.clone(),
                                )
                                .with_visible(false);

                                #[cfg(target_arch = "wasm32")]
                                let window_attributes = {
                                    use winit::platform::web::WindowAttributesExtWebSys;
                                    window_attributes.with_canvas(self.canvas.take())
                                };

                                log::info!(
                                    "Window attributes for id `{id:#?}`: {window_attributes:#?}"
                                );

                                // On macOS, the `position` in `WindowAttributes` represents the "inner"
                                // position of the window; while on other platforms it's the "outer" position.
                                // We fix the inconsistency on macOS by positioning the window after creation.
                                #[cfg(target_os = "macos")]
                                let mut window_attributes = window_attributes;

                                #[cfg(target_os = "macos")]
                                let position = window_attributes.position.take();

                                let window = event_loop
                                    .create_window(window_attributes)
                                    .expect("Create window");

                                #[cfg(target_os = "macos")]
                                if let Some(position) = position {
                                    window.set_outer_position(position);
                                }

                                #[cfg(target_arch = "wasm32")]
                                {
                                    use winit::platform::web::WindowExtWebSys;

                                    let canvas = window.canvas().expect("Get window canvas");

                                    let _ = canvas.set_attribute(
                                        "style",
                                        "display: block; width: 100%; height: 100%",
                                    );

                                    let window = web_sys::window().unwrap();
                                    let document = window.document().unwrap();
                                    let body = document.body().unwrap();

                                    let target = target.and_then(|target| {
                                        body.query_selector(&format!("#{target}"))
                                            .ok()
                                            .unwrap_or(None)
                                    });

                                    match target {
                                        Some(node) => {
                                            let _ = node.replace_with_with_node_1(&canvas).expect(
                                                &format!("Could not replace #{}", node.id()),
                                            );
                                        }
                                        None => {
                                            let _ = body
                                                .append_child(&canvas)
                                                .expect("Append canvas to HTML body");
                                        }
                                    };
                                }

                                self.process_event(
                                    event_loop,
                                    Event::WindowCreated {
                                        id,
                                        window: Arc::new(window),
                                        exit_on_close_request,
                                        make_visible: visible,
                                        on_open,
                                    },
                                );
                            }
                            Control::Exit => {
                                self.process_event(event_loop, Event::Exit);
                                event_loop.exit();
                                break;
                            }
                            Control::Crash(error) => {
                                self.error = Some(error);
                                event_loop.exit();
                            }
                            Control::SetAutomaticWindowTabbing(_enabled) => {
                                #[cfg(target_os = "macos")]
                                {
                                    use winit::platform::macos::ActiveEventLoopExtMacOS;
                                    event_loop.set_allows_automatic_window_tabbing(_enabled);
                                }
                            }
                        },
                        _ => {
                            break;
                        }
                    },
                    task::Poll::Ready(_) => {
                        event_loop.exit();
                        break;
                    }
                };
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut runner = runner;
        let _ = event_loop.run_app(&mut runner);

        runner.error.map(Err).unwrap_or(Ok(()))
    }

    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        let _ = event_loop.spawn_app(runner);

        Ok(())
    }
}

#[derive(Debug)]
pub(super) enum Event<Message: 'static> {
    WindowCreated {
        id: window::Id,
        window: Arc<winit::window::Window>,
        exit_on_close_request: bool,
        make_visible: bool,
        on_open: oneshot::Sender<window::Id>,
    },
    EventLoopAwakened(winit::event::Event<Message>),
    Exit,
}

/// Per-window event storage that keeps each window's allocation alive across
/// event-loop turns. Inter-window ordering is intentionally irrelevant: UI
/// dispatch is window-scoped, while insertion order inside each window is
/// preserved exactly.
#[derive(Default)]
pub(super) struct PendingEvents {
    pub(super) windows: FxHashMap<window::Id, Vec<core::Event>>,
    len: usize,
}

/// Fold only adjacent observations that share one pointer/window identity.
///
/// Button transitions, wheel deltas, file drops, compositor timestamps and
/// focus changes are never merged: they can affect gesture state or observers.
#[inline]
fn replaces_previous(previous: &core::Event, new: &core::Event) -> bool {
    match (previous, new) {
        (core::Event::Window(before), core::Event::Window(after)) => {
            after.supersedes_adjacent(before)
        }
        (
            core::Event::Mouse(mouse::Event::CursorMoved { .. }),
            core::Event::Mouse(mouse::Event::CursorMoved { .. }),
        ) => true,
        (
            core::Event::Touch(crate::core::touch::Event::FingerMoved { id: before, .. }),
            core::Event::Touch(crate::core::touch::Event::FingerMoved { id: after, .. }),
        ) => before == after,
        _ => false,
    }
}

impl PendingEvents {
    #[inline]
    pub(super) fn push(&mut self, window: window::Id, event: core::Event) {
        let queue = self.windows.entry(window).or_default();
        if let Some(previous) = queue.last_mut() {
            if replaces_previous(previous, &event) {
                // Newest location/dimensions win. Unlike dropping events under
                // a global queue cap, this cannot cross a semantic boundary.
                *previous = event;
                return;
            }
        }
        queue.push(event);
        self.len = self.len.saturating_add(1);
    }

    #[inline]
    pub(super) fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Marks the current batch consumed. Per-window vectors stay allocated:
    /// event bursts are common during resize and shrinking here would force
    /// the next frame to grow the exact same buffers again.
    #[inline]
    pub(super) fn finish_batch(&mut self) {
        self.len = 0;
    }

    pub(super) fn retain_windows(&mut self, mut is_active: impl FnMut(window::Id) -> bool) {
        self.windows.retain(|id, _events| is_active(*id));

        let target = self.windows.len().max(8);
        if self.windows.capacity() > target.saturating_mul(4) {
            self.windows.shrink_to(target);
        }
    }
}

#[derive(Debug)]
pub(super) enum Control {
    ChangeFlow(winit::event_loop::ControlFlow),
    Exit,
    Crash(Error),
    CreateWindow {
        id: window::Id,
        settings: window::Settings,
        title: String,
        monitor: Option<winit::monitor::MonitorHandle>,
        on_open: oneshot::Sender<window::Id>,
        scale_factor: f32,
    },
    SetAutomaticWindowTabbing(bool),
}

#[cfg(test)]
mod taim_event_batch_tests {
    use super::*;
    use crate::core::{Point, Size, mouse, touch};

    fn mouse_move(x: f32) -> core::Event {
        core::Event::Mouse(mouse::Event::CursorMoved {
            position: Point::new(x, 0.0),
        })
    }

    fn finger_move(id: u64, x: f32) -> core::Event {
        core::Event::Touch(touch::Event::FingerMoved {
            id: touch::Finger(id),
            position: Point::new(x, 2.0),
            metadata: core::pointer::Metadata::NONE,
        })
    }

    #[test]
    fn adjacent_cursor_moves_keep_last_position_without_extra_allocation() {
        let id = window::Id::unique();
        let mut queue = PendingEvents::default();
        queue.push(id, mouse_move(1.0));
        queue.push(id, mouse_move(2.0));
        queue.push(id, mouse_move(3.0));
        assert_eq!(queue.len, 1);
        assert_eq!(queue.windows[&id], vec![mouse_move(3.0)]);
    }

    #[test]
    fn press_and_release_are_gesture_boundaries() {
        let id = window::Id::unique();
        let mut queue = PendingEvents::default();
        queue.push(id, mouse_move(1.0));
        queue.push(
            id,
            core::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        );
        queue.push(id, mouse_move(2.0));
        queue.push(
            id,
            core::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        );
        queue.push(id, mouse_move(3.0));
        assert_eq!(queue.windows[&id].len(), 5);
        assert_eq!(queue.len, 5);
    }

    #[test]
    fn different_fingers_must_not_coalesce() {
        let id = window::Id::unique();
        let mut queue = PendingEvents::default();
        queue.push(id, finger_move(1, 1.0));
        queue.push(id, finger_move(2, 2.0));
        queue.push(id, finger_move(1, 3.0));
        queue.push(id, finger_move(1, 4.0));
        assert_eq!(
            queue.windows[&id],
            vec![
                finger_move(1, 1.0),
                finger_move(2, 2.0),
                finger_move(1, 4.0)
            ]
        );
    }

    #[test]
    fn adjacent_resize_coalesces_but_close_request_does_not() {
        let id = window::Id::unique();
        let mut queue = PendingEvents::default();
        queue.push(
            id,
            core::Event::Window(window::Event::Resized(Size::new(10.0, 10.0))),
        );
        queue.push(
            id,
            core::Event::Window(window::Event::Resized(Size::new(20.0, 30.0))),
        );
        assert_eq!(queue.len, 1);
        queue.push(id, core::Event::Window(window::Event::CloseRequested));
        queue.push(id, core::Event::Window(window::Event::CloseRequested));
        assert_eq!(queue.len, 3);
        assert_eq!(
            queue.windows[&id][0],
            core::Event::Window(window::Event::Resized(Size::new(20.0, 30.0)))
        );
    }

    #[test]
    fn separate_windows_keep_independent_tail_events() {
        let a = window::Id::unique();
        let b = window::Id::unique();
        let mut queue = PendingEvents::default();
        queue.push(a, mouse_move(1.0));
        queue.push(b, mouse_move(2.0));
        queue.push(a, mouse_move(3.0));
        assert_eq!(queue.windows[&a], vec![mouse_move(3.0)]);
        assert_eq!(queue.windows[&b], vec![mouse_move(2.0)]);
        assert_eq!(queue.len, 2);
        queue.windows.get_mut(&a).unwrap().clear();
        queue.windows.get_mut(&b).unwrap().clear();
        queue.finish_batch();
        assert!(queue.is_empty());
    }
}
