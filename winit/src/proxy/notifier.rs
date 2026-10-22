//! How the renderer asks the shell for work.
use super::Proxy;
use crate::graphics::shell;
use crate::runtime::{Action, window};

impl<T: Send> shell::Notifier for Proxy<T> {
    fn tick(&self) {
        self.send_action(Action::Tick);
    }

    fn request_redraw(&self) {
        self.send_action(Action::Window(window::Action::RedrawAll));
    }

    fn invalidate_layout(&self) {
        self.send_action(Action::Window(window::Action::RelayoutAll));
    }
}
