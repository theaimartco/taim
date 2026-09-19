use crate::core::Size;
use crate::core::time::Instant;
use crate::core::window::{Event, Id};
use crate::futures::Subscription;
use crate::futures::event;

/// Subscribes to presentation frames of the running application.
pub fn frames() -> Subscription<Instant> {
    event::listen_raw(|event, _status, _window| match event {
        crate::core::Event::Window(Event::RedrawRequested(at)) => Some(at),
        _ => None,
    })
}

/// Subscribes to all window events.
pub fn events() -> Subscription<(Id, Event)> {
    event::listen_with(|event, _status, id| match event {
        crate::core::Event::Window(event) => Some((id, event)),
        _ => None,
    })
}

/// Subscribes to window-open events.
pub fn open_events() -> Subscription<Id> {
    event::listen_with(|event, _status, id| match event {
        crate::core::Event::Window(Event::Opened { .. }) => Some(id),
        _ => None,
    })
}

/// Subscribes to window-closed events.
pub fn close_events() -> Subscription<Id> {
    event::listen_with(|event, _status, id| match event {
        crate::core::Event::Window(Event::Closed) => Some(id),
        _ => None,
    })
}

/// Subscribes to window-resized events.
pub fn resize_events() -> Subscription<(Id, Size)> {
    event::listen_with(|event, _status, id| match event {
        crate::core::Event::Window(Event::Resized(size)) => Some((id, size)),
        _ => None,
    })
}

/// Subscribes to window close requests.
pub fn close_requests() -> Subscription<Id> {
    event::listen_with(|event, _status, id| match event {
        crate::core::Event::Window(Event::CloseRequested) => Some(id),
        _ => None,
    })
}
