//! Reasons another thread wakes the event loop.
use crate::core::{clipboard, theme, window};
use crate::graphics;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use winit::event_loop::EventLoopProxy;

/// Something that happened off the event loop and needs its attention.
#[derive(Debug)]
pub(crate) enum Wake {
    /// State read by a window's views changed.
    Changed(window::Id),
    /// A clipboard read finished.
    Clipboard(window::Id, clipboard::Event),
    /// The desktop's color scheme changed.
    #[cfg_attr(
        not(all(feature = "linux-theme-detection", target_os = "linux")),
        allow(dead_code)
    )]
    SystemTheme(theme::Mode),
    /// The renderer finished background work (an image decoded, a font
    /// loaded) and every window should be laid out and painted again.
    Refresh,
    /// A screen reader started reading a window: publish its whole tree.
    #[cfg(target_os = "android")]
    Describe(window::Id),
    /// A screen reader asked for an action on a node.
    #[cfg(target_os = "android")]
    Accessibility(window::Id, accesskit::ActionRequest),
}

/// Wakes the event loop when a window's state changes, at most once until
/// the event loop has looked at the change.
#[derive(Debug, Clone)]
pub(crate) struct ChangeSignal {
    window: window::Id,
    queued: Arc<AtomicBool>,
    proxy: EventLoopProxy<Wake>,
}

impl ChangeSignal {
    /// The event loop this signal wakes.
    #[cfg(target_os = "android")]
    pub fn proxy(&self) -> EventLoopProxy<Wake> {
        self.proxy.clone()
    }

    pub fn new(window: window::Id, proxy: EventLoopProxy<Wake>) -> Self {
        Self {
            window,
            queued: Arc::new(AtomicBool::new(false)),
            proxy,
        }
    }

    /// Queues a [`Wake::Changed`] unless one is already queued.
    pub fn raise(&self) {
        if !self.queued.swap(true, Ordering::AcqRel) {
            let _ = self.proxy.send_event(Wake::Changed(self.window));
        }
    }

    /// The event loop is handling the queued wake; the next change queues
    /// a new one.
    pub fn lower(&self) {
        self.queued.store(false, Ordering::Release);
    }
}

/// Lets the renderer reach the event loop.
pub(crate) struct RendererSignal(pub EventLoopProxy<Wake>);

impl graphics::shell::Notifier for RendererSignal {
    fn tick(&self) {
        let _ = self.0.send_event(Wake::Refresh);
    }

    fn request_redraw(&self) {
        let _ = self.0.send_event(Wake::Refresh);
    }

    fn invalidate_layout(&self) {
        let _ = self.0.send_event(Wake::Refresh);
    }
}
