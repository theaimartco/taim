//! Typed subscriptions over TAIM's normalized runtime event stream.

use crate::MaybeSend;
use crate::core::event::{self, Event};
use crate::core::window;
use crate::subscription::{self, Subscription};

/// Returns events that reached no widget.
pub fn listen() -> Subscription<Event> {
    listen_with(unhandled)
}

fn unhandled(event: Event, status: event::Status, _window: window::Id) -> Option<Event> {
    (status == event::Status::Ignored).then_some(event)
}

/// Subscribes to non-redraw interactions and transforms them with `mapper`.
///
/// System-theme and platform events have dedicated subscriptions, keeping
/// regular input filters small and preventing accidental redraw feedback
/// loops.
pub fn listen_with<Message>(
    mapper: fn(Event, event::Status, window::Id) -> Option<Message>,
) -> Subscription<Message>
where
    Message: MaybeSend + 'static,
{
    interaction_stream(InteractionMode::Application, mapper)
}

/// Subscribes to every interaction, including redraw notifications.
///
/// Returning a message for every redraw can create a permanent render loop;
/// callers should filter aggressively.
pub fn listen_raw<Message>(
    mapper: fn(Event, event::Status, window::Id) -> Option<Message>,
) -> Subscription<Message>
where
    Message: MaybeSend + 'static,
{
    interaction_stream(InteractionMode::Raw, mapper)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum InteractionMode {
    Application,
    Raw,
}

fn interaction_stream<Message>(
    mode: InteractionMode,
    mapper: fn(Event, event::Status, window::Id) -> Option<Message>,
) -> Subscription<Message>
where
    Message: MaybeSend + 'static,
{
    subscription::filter_map((mode, mapper), move |incoming| {
        let subscription::Event::Interaction {
            window,
            event,
            status,
        } = incoming
        else {
            return None;
        };

        if mode == InteractionMode::Application
            && matches!(event, Event::Window(window::Event::RedrawRequested(_)))
        {
            return None;
        }

        mapper(event, status, window)
    })
}

/// Subscribes to application URLs emitted by the active TAIM platform adapter.
pub fn listen_url() -> Subscription<String> {
    #[derive(Hash)]
    struct ApplicationUrl;

    subscription::filter_map(ApplicationUrl, |incoming| match incoming {
        subscription::Event::PlatformSpecific(subscription::PlatformSpecific::MacOS(
            subscription::MacOS::ReceivedUrl(url),
        )) => Some(url),
        _ => None,
    })
}
