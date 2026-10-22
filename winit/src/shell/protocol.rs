//! Messages exchanged between the [`host`](super::host) and the
//! [`engine`](super::engine).
use crate::Error;
use crate::core::window;
use crate::futures::futures::channel::oneshot;
use crate::runtime::Action;

use std::sync::Arc;

/// Something the platform reported, forwarded from the host to the engine.
#[derive(Debug)]
pub(super) enum Signal<Message: 'static> {
    /// The event loop woke up for the given reason.
    Woke(winit::event::StartCause),
    /// An action arrived through the [`Proxy`](crate::Proxy).
    Action(Action<Message>),
    /// A native window reported an event.
    Window {
        id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    },
    /// The host finished creating a window the engine asked for.
    WindowReady(NativeWindow),
    /// The platform queue is drained; the engine may batch and render.
    Idle,
    /// The host is shutting down; the engine must stop.
    Shutdown,
}

/// A freshly created native window, not yet known to the engine.
#[derive(Debug)]
pub(super) struct NativeWindow {
    pub id: window::Id,
    pub raw: Arc<winit::window::Window>,
    pub exit_on_close_request: bool,
    /// Windows are created hidden and revealed once their first interface
    /// exists, so they never flash an empty surface.
    pub reveal: bool,
    pub opened: oneshot::Sender<window::Id>,
}

/// Something only the host can do, asked for by the engine.
#[derive(Debug)]
pub(super) enum Request {
    /// Change how the event loop waits for the next event.
    Flow(winit::event_loop::ControlFlow),
    /// Create a native window.
    OpenWindow(WindowRequest),
    /// macOS: allow or forbid automatic window tabbing.
    AutomaticTabbing(bool),
    /// Leave the event loop normally.
    Quit,
    /// Leave the event loop, reporting `Error` from the run call.
    Fail(Error),
}

/// Everything the host needs to create one native window.
#[derive(Debug)]
pub(super) struct WindowRequest {
    pub id: window::Id,
    pub settings: window::Settings,
    pub title: String,
    pub scale_factor: f32,
    pub monitor: Option<winit::monitor::MonitorHandle>,
    pub opened: oneshot::Sender<window::Id>,
}
