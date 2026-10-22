//! The `winit` side of the shell: owns the event loop, never the application.
use super::protocol::{NativeWindow, Request, Signal, WindowRequest};
use crate::Error;
use crate::conversion;
use crate::core::theme;
use crate::core::time::Instant;
use crate::futures::futures::Future;
use crate::futures::futures::channel::{mpsc, oneshot};
use crate::futures::futures::task::{Context, Poll, noop_waker_ref};
use crate::runtime::Action;

use std::pin::Pin;
use std::sync::Arc;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};

/// Drives the engine task from `winit` callbacks and carries out its requests.
pub(super) struct Host<Message: 'static, Engine> {
    engine: Pin<Box<Engine>>,
    signals: mpsc::UnboundedSender<Signal<Message>>,
    requests: mpsc::UnboundedReceiver<Request>,
    /// Platform application id (Wayland app-id, X11 class) for new windows.
    application_id: Option<String>,
    /// Theme reported on first resume, handed to the engine exactly once.
    first_theme: Option<oneshot::Sender<theme::Mode>>,
    failure: Option<Error>,
    #[cfg(target_arch = "wasm32")]
    canvas: Option<web_sys::HtmlCanvasElement>,
}

impl<Message, Engine> Host<Message, Engine>
where
    Engine: Future<Output = ()>,
{
    pub(super) fn new(
        engine: Engine,
        signals: mpsc::UnboundedSender<Signal<Message>>,
        requests: mpsc::UnboundedReceiver<Request>,
        application_id: Option<String>,
        first_theme: oneshot::Sender<theme::Mode>,
    ) -> Self {
        Self {
            engine: Box::pin(engine),
            signals,
            requests,
            application_id,
            first_theme: Some(first_theme),
            failure: None,
            #[cfg(target_arch = "wasm32")]
            canvas: None,
        }
    }

    /// The error the engine failed with, if it failed.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(super) fn into_failure(self) -> Option<Error> {
        self.failure
    }

    /// Hands `signal` to the engine and services whatever it asks for.
    fn deliver(&mut self, event_loop: &ActiveEventLoop, signal: Signal<Message>) {
        if event_loop.exiting() {
            return;
        }

        self.signals
            .unbounded_send(signal)
            .expect("the engine outlives the event loop");

        self.pump(event_loop);
    }

    /// Polls the engine until it blocks, executing each request it emits.
    fn pump(&mut self, event_loop: &ActiveEventLoop) {
        let mut context = Context::from_waker(noop_waker_ref());

        loop {
            if let Poll::Ready(()) = self.engine.as_mut().poll(&mut context) {
                event_loop.exit();
                return;
            }

            let Ok(request) = self.requests.try_recv() else {
                return;
            };

            match request {
                Request::Flow(flow) => merge_control_flow(event_loop, flow),
                Request::OpenWindow(request) => {
                    let window = self.open_window(event_loop, request);
                    self.deliver(event_loop, Signal::WindowReady(window));
                }
                Request::AutomaticTabbing(_enabled) => {
                    #[cfg(target_os = "macos")]
                    {
                        use winit::platform::macos::ActiveEventLoopExtMacOS;
                        event_loop.set_allows_automatic_window_tabbing(_enabled);
                    }
                }
                Request::Quit => {
                    self.deliver(event_loop, Signal::Shutdown);
                    event_loop.exit();
                    return;
                }
                Request::Fail(error) => {
                    self.failure = Some(error);
                    event_loop.exit();
                }
            }
        }
    }

    fn open_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        request: WindowRequest,
    ) -> NativeWindow {
        let WindowRequest {
            id,
            settings,
            title,
            scale_factor,
            monitor,
            opened,
        } = request;

        let exit_on_close_request = settings.exit_on_close_request;
        let reveal = settings.visible;

        #[cfg(target_arch = "wasm32")]
        let mount_target = settings.platform_specific.target.clone();

        let attributes = conversion::window_attributes(
            settings,
            &title,
            scale_factor,
            monitor.or_else(|| event_loop.primary_monitor()),
            self.application_id.clone(),
        )
        .with_visible(false);

        #[cfg(target_arch = "wasm32")]
        let attributes = {
            use winit::platform::web::WindowAttributesExtWebSys;
            attributes.with_canvas(self.canvas.take())
        };

        log::info!("Opening window {id:?} with {attributes:#?}");

        // macOS treats `WindowAttributes::position` as the inner origin while
        // every other platform treats it as the outer one. Place the window
        // after creation there so `Settings::position` means the same thing
        // everywhere.
        #[cfg(target_os = "macos")]
        let (attributes, outer_position) = {
            let mut attributes = attributes;
            let position = attributes.position.take();
            (attributes, position)
        };

        let raw = event_loop
            .create_window(attributes)
            .expect("the platform refused to create a window");

        #[cfg(target_os = "macos")]
        if let Some(position) = outer_position {
            raw.set_outer_position(position);
        }

        #[cfg(target_arch = "wasm32")]
        web::mount(&raw, mount_target.as_deref());

        NativeWindow {
            id,
            raw: Arc::new(raw),
            exit_on_close_request,
            reveal,
            opened,
        }
    }
}

impl<Message, Engine> winit::application::ApplicationHandler<Action<Message>>
    for Host<Message, Engine>
where
    Engine: Future<Output = ()>,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(first_theme) = self.first_theme.take() {
            let mode = event_loop
                .system_theme()
                .map(conversion::theme_mode)
                .unwrap_or_default();

            let _ = first_theme.send(mode);
        }
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        self.deliver(event_loop, Signal::Woke(cause));
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        // Windows does not return to the event loop while the user drags a
        // window edge or title bar (rust-windowing/winit#3272). Settle after
        // each geometry change so the content follows the drag.
        #[cfg(target_os = "windows")]
        let settle = matches!(event, WindowEvent::Resized(_) | WindowEvent::Moved(_));

        self.deliver(event_loop, Signal::Window { id, event });

        #[cfg(target_os = "windows")]
        if settle {
            self.deliver(event_loop, Signal::Idle);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, action: Action<Message>) {
        self.deliver(event_loop, Signal::Action(action));
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.deliver(event_loop, Signal::Idle);
    }
}

/// Applies the engine's wait policy without discarding an earlier wake-up
/// that is still armed: a later deadline, or a plain `Wait`, never cancels a
/// pending `WaitUntil` that has not fired yet.
fn merge_control_flow(event_loop: &ActiveEventLoop, requested: ControlFlow) {
    let keep_armed = match (event_loop.control_flow(), requested) {
        (ControlFlow::WaitUntil(armed), ControlFlow::WaitUntil(next)) => armed < next,
        (ControlFlow::WaitUntil(armed), ControlFlow::Wait) => armed > Instant::now(),
        _ => false,
    };

    if !keep_armed {
        event_loop.set_control_flow(requested);
    }
}

/// Whether `event` is the platform's "quit application" shortcut
/// (`⌘ Q` on macOS; nothing elsewhere).
pub fn user_force_quit(event: &WindowEvent, _modifiers: winit::keyboard::ModifiersState) -> bool {
    #[cfg(target_os = "macos")]
    if let WindowEvent::KeyboardInput {
        event:
            winit::event::KeyEvent {
                logical_key: winit::keyboard::Key::Character(key),
                state: winit::event::ElementState::Pressed,
                ..
            },
        ..
    } = event
    {
        return key == "q" && _modifiers.super_key();
    }

    let _ = event;
    false
}

#[cfg(target_arch = "wasm32")]
mod web {
    /// Puts the window's canvas into the page: in place of the element whose
    /// id is `target`, or at the end of `<body>`.
    pub(super) fn mount(window: &winit::window::Window, target: Option<&str>) {
        use winit::platform::web::WindowExtWebSys;

        let canvas = window.canvas().expect("a web window always has a canvas");
        let _ = canvas.set_attribute("style", "display: block; width: 100%; height: 100%");

        let document = web_sys::window()
            .and_then(|window| window.document())
            .expect("running inside a browser document");
        let body = document.body().expect("the document has a body");

        let placeholder =
            target.and_then(|target| body.query_selector(&format!("#{target}")).ok().flatten());

        match placeholder {
            Some(node) => node
                .replace_with_with_node_1(&canvas)
                .unwrap_or_else(|_| panic!("could not replace #{}", node.id())),
            None => {
                let _ = body
                    .append_child(&canvas)
                    .expect("could not append the canvas to <body>");
            }
        }
    }
}
