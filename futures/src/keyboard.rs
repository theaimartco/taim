//! Keyboard subscriptions built on TAIM's normalized interaction stream.

use crate::core;
use crate::core::keyboard::Event;
use crate::subscription::Subscription;

/// Returns keyboard events that reached no focused widget or shortcut.
pub fn listen() -> Subscription<Event> {
    crate::event::listen_with(|event, status, _window| {
        if status != core::event::Status::Ignored {
            return None;
        }

        match event {
            core::Event::Keyboard(event) => Some(event),
            _ => None,
        }
    })
}
