use crate::core::event;
use crate::core::time::Instant;
use crate::core::window;

/// A runtime action that can be performed by some widgets.
#[derive(Debug, Clone)]
pub struct Action<Message> {
    message_to_publish: Option<Message>,
    redraw_request: window::RedrawRequest,
    event_status: event::Status,
}

impl<Message> Action<Message> {
    fn new() -> Self {
        Self {
            message_to_publish: None,
            redraw_request: window::RedrawRequest::Wait,
            event_status: event::Status::Ignored,
        }
    }

    /// Creates a new "capturing" [`Action`]. A capturing [`Action`]
    /// will make other widgets consider it final and prevent further
    /// processing.
    ///
    /// Prevents "event bubbling".
    pub fn capture() -> Self {
        Self {
            event_status: event::Status::Captured,
            ..Self::new()
        }
    }

    /// Creates a new [`Action`] that publishes the given `Message` for
    /// the application to handle.
    ///
    /// Publishing a `Message` always produces a redraw.
    pub fn publish(message: Message) -> Self {
        Self {
            message_to_publish: Some(message),
            // An event-driven message may change the application state even
            // when it does not invalidate the widget tree directly.
            redraw_request: window::RedrawRequest::NextFrame,
            ..Self::new()
        }
    }

    /// Creates a new [`Action`] that requests a redraw to happen as
    /// soon as possible; without publishing any `Message`.
    pub fn request_redraw() -> Self {
        Self {
            redraw_request: window::RedrawRequest::NextFrame,
            ..Self::new()
        }
    }

    /// Creates a new [`Action`] that requests a redraw to happen at
    /// the given [`Instant`]; without publishing any `Message`.
    ///
    /// This can be useful to efficiently animate content, like a
    /// blinking caret on a text input.
    pub fn request_redraw_at(at: Instant) -> Self {
        Self {
            redraw_request: window::RedrawRequest::At(at),
            ..Self::new()
        }
    }

    /// Marks the [`Action`] as "capturing". See [`Self::capture`].
    pub fn and_capture(mut self) -> Self {
        self.event_status = event::Status::Captured;
        self
    }

    /// Combine this action with another frame deadline without losing
    /// an already pending message or event-capture decision.
    ///
    /// The earliest frame wins; publishing always selects NextFrame.
    /// This is used by TAIM's single shared per-window frame clock.
    #[must_use]
    pub fn and_request_redraw_at(mut self, at: Instant) -> Self {
        self.redraw_request =
            self.redraw_request.merge(window::RedrawRequest::At(at));
        self
    }

    /// Converts the [`Action`] into its internal parts.
    ///
    /// This method is meant to be used by runtimes, libraries, or internal
    /// widget implementations.
    pub fn into_inner(self) -> (Option<Message>, window::RedrawRequest, event::Status) {
        (
            self.message_to_publish,
            self.redraw_request,
            self.event_status,
        )
    }
}

#[cfg(test)]
mod taim_action_tests {
    use super::*;

    #[test]
    fn canvas_published_messages_request_an_immediate_frame() {
        let (message, redraw, status) = Action::publish(17_u32).into_inner();

        assert_eq!(message, Some(17));
        assert_eq!(redraw, window::RedrawRequest::NextFrame);
        assert_eq!(status, event::Status::Ignored);
    }

    #[test]
    fn capture_keeps_publication_and_frame_intent() {
        let (message, redraw, status) = Action::publish("changed")
            .and_capture()
            .into_inner();

        assert_eq!(message, Some("changed"));
        assert_eq!(redraw, window::RedrawRequest::NextFrame);
        assert_eq!(status, event::Status::Captured);
    }

    #[test]
    fn deadline_merge_never_overrides_an_immediate_message() {
        let future = Instant::now() + crate::core::time::Duration::from_secs(10);
        let (_, redraw, _) = Action::publish(1_u8)
            .and_request_redraw_at(future)
            .into_inner();

        assert_eq!(redraw, window::RedrawRequest::NextFrame);
    }

    #[test]
    fn multiple_deadline_requests_select_the_earliest() {
        let now = Instant::now();
        let soon = now + crate::core::time::Duration::from_millis(10);
        let late = now + crate::core::time::Duration::from_secs(2);

        let (_, redraw, _) = Action::<()>::request_redraw_at(late)
            .and_request_redraw_at(soon)
            .into_inner();

        assert_eq!(redraw, window::RedrawRequest::At(soon));
    }
}
