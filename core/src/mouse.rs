//! Handle mouse events.
pub mod click;

mod button;
mod cursor;
mod event;
mod interaction;

pub use button::Button;
pub use click::Click;
pub use cursor::Cursor;
pub use event::{Event, ScrollDelta};
pub use interaction::Interaction;

/// Delivery policy for mouse events in a bounded TAIM input queue.
///
/// Cursor positions are absolute samples: a newer finite position can replace
/// the previous sample. Wheel events are *deltas*: replacing one would lose
/// accumulated scroll distance, so they must be delivered individually until
/// a future queue explicitly implements additive delta coalescing.
impl Event {
    /// Whether this event is an absolute sample a newer one may overwrite.
    #[inline]
    pub fn can_replace_previous_without_loss(&self) -> bool {
        matches!(self, Self::CursorMoved { position } if position.x.is_finite() && position.y.is_finite())
    }
}

#[cfg(test)]
mod taim_mouse_delivery_tests {
    use super::*;
    use crate::Point;

    #[test]
    fn finite_cursor_position_is_replaceable_but_wheel_delta_is_not() {
        let move_event = Event::CursorMoved {
            position: Point::new(12.0, 15.0),
        };
        let scroll = Event::WheelScrolled {
            delta: ScrollDelta::Pixels { x: 0.0, y: 4.0 },
        };
        assert!(move_event.can_replace_previous_without_loss());
        assert!(!scroll.can_replace_previous_without_loss());
        assert!(!Event::ButtonPressed(Button::Left).can_replace_previous_without_loss());
        assert!(!Event::CursorLeft.can_replace_previous_without_loss());
    }

    #[test]
    fn invalid_mouse_cursor_sample_is_not_eligible_for_drop_merge() {
        let invalid = Event::CursorMoved {
            position: Point::new(f32::NAN, 0.0),
        };
        assert!(!invalid.can_replace_previous_without_loss());
    }
}
