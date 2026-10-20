//! Handle events of a user interface.
use crate::clipboard;
use crate::input_method;
use crate::keyboard;
use crate::mouse;
use crate::pointer;
use crate::touch;
use crate::window;

/// A user interface event.
///
/// _**Note:** This type is largely incomplete! If you need to track
/// additional events, feel free to [open an issue] and share your use case!_
///
/// [open an issue]: https://github.com/theaimartco/taim/issues
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A keyboard event
    Keyboard(keyboard::Event),

    /// A mouse event
    Mouse(mouse::Event),

    /// A window event
    Window(window::Event),

    /// A touch event
    Touch(touch::Event),

    /// An input method event
    InputMethod(input_method::Event),

    /// A clipboard event
    Clipboard(clipboard::Event),

    /// A wake event, produced by a [`Waker`](crate::shell::Waker).
    Waken,

    /// A wake event addressed to one reactive source.
    Reactive(u64),
}

/// Whether an [`Event`] may be coalesced (merged with an adjacent event
/// of the same kind, or dropped entirely under backpressure) without
/// losing information a widget or gesture recognizer depends on.
///
/// A high-frequency input source (a mouse reporting hundreds of
/// `CursorMoved` events per second, a touchscreen reporting
/// `FingerMoved` at its own high polling rate) can produce events faster
/// than a slow frame can consume them one at a time; `phase.md`'s own
/// correctness rule -- "high-frequency moves may be coalesced, but
/// down/up/cancel and semantic boundary events may not be dropped" --
/// is the policy [`Event::coalescing`] implements as a pure function of
/// an event's shape, so `runtime::pointer_router` (Phase 8) and any
/// other event-queue consumer share exactly one answer to "is it safe to
/// merge/drop this."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coalescing {
    /// This event may be merged with an adjacent, same-kind event (e.g.
    /// two consecutive `CursorMoved`s collapse to the latest position),
    /// or dropped entirely if a newer event of the same kind has
    /// already been queued -- no semantic information is lost either
    /// way, only intermediate positions a widget was never guaranteed
    /// to see individually.
    Mergeable,

    /// This event must be delivered on its own: it marks a lifecycle
    /// boundary (a press, a release, a cancellation) or a semantic
    /// transition (focus/cursor entering or leaving) that a widget or
    /// gesture recognizer may depend on having seen at all, even if its
    /// payload looks similar to a neighboring event.
    Critical,
}

impl Event {
    /// Classifies this [`Event`] for coalescing purposes; see
    /// [`Coalescing`] for the full contract.
    pub fn coalescing(&self) -> Coalescing {
        match self {
            Event::Mouse(mouse_event) => {
                if mouse_event.can_replace_previous_without_loss() {
                    Coalescing::Mergeable
                } else {
                    Coalescing::Critical
                }
            }
            Event::Touch(touch::Event::FingerMoved { position, .. }) => {
                // Invalid motion is never a valid replacement for a prior
                // finite sample; preserve the original queue semantics.
                if position.x.is_finite() && position.y.is_finite() {
                    Coalescing::Mergeable
                } else {
                    Coalescing::Critical
                }
            },
            Event::Touch(_) => Coalescing::Critical,
            Event::Keyboard(_)
            | Event::Window(_)
            | Event::InputMethod(_)
            | Event::Clipboard(_)
            | Event::Waken
            | Event::Reactive(_) => Coalescing::Critical,
        }
    }

    /// Converts this [`Event`] into a unified [`pointer::Pointer`], if it
    /// is a mouse or touch event at all -- `None` for every other
    /// variant (keyboard, window, input method, clipboard, waken), which
    /// have no pointer identity to normalize.
    ///
    /// A [`touch::Event`] already carries its own position, so it maps
    /// via [`touch::Event::to_pointer`] directly. A [`mouse::Event`]
    /// does not: `ButtonPressed`/`ButtonReleased`/`WheelScrolled` carry
    /// no coordinates of their own (the platform reports the cursor
    /// position separately, out-of-band from the button/wheel event
    /// itself), so `cursor` -- the position a caller is already tracking
    /// alongside these events, e.g. `UserInterface::update`'s own
    /// `mouse::Cursor` parameter -- supplies it. `CursorEntered`/
    /// `CursorLeft` have no single position to report either and also
    /// return `None`: they are boundary notifications, not something a
    /// [`pointer::Pointer`] phase represents.
    pub fn to_pointer(
        &self,
        cursor: Option<crate::Point>,
        timestamp: crate::time::Instant,
    ) -> Option<pointer::Pointer> {
        match self {
            Event::Touch(touch) => {
                let pointer = touch.to_pointer(timestamp);
                // TAIM rejects malformed active coordinates. A malformed
                // terminal coordinate becomes a safe Cancel event so the
                // router releases capture without reporting a successful tap.
                pointer.sanitize_coordinates()
            },
            Event::Mouse(mouse_event) => {
                let (phase, position) = match mouse_event {
                    mouse::Event::ButtonPressed(_) => (pointer::Phase::Down, cursor?),
                    mouse::Event::ButtonReleased(_) => (pointer::Phase::Up, cursor?),
                    mouse::Event::CursorMoved { position } => (pointer::Phase::Move, *position),
                    mouse::Event::CursorEntered
                    | mouse::Event::CursorLeft
                    | mouse::Event::WheelScrolled { .. } => return None,
                };

                pointer::Pointer {
                    id: pointer::Id::MOUSE,
                    device: pointer::DeviceKind::Mouse,
                    phase,
                    position,
                    timestamp,
                    metadata: pointer::Metadata::NONE,
                    captured: false,
                }
                .sanitize_coordinates()
            }
            Event::Keyboard(_)
            | Event::Window(_)
            | Event::InputMethod(_)
            | Event::Clipboard(_)
            | Event::Waken
            | Event::Reactive(_) => None,
        }
    }
}

/// The status of an [`Event`] after being processed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The [`Event`] was **NOT** handled by any widget.
    Ignored,

    /// The [`Event`] was handled and processed by a widget.
    Captured,
}

impl Status {
    /// Merges two [`Status`] into one.
    ///
    /// `Captured` takes precedence over `Ignored`:
    ///
    /// ```
    /// use taim_core::event::Status;
    ///
    /// assert_eq!(Status::Ignored.merge(Status::Ignored), Status::Ignored);
    /// assert_eq!(Status::Ignored.merge(Status::Captured), Status::Captured);
    /// assert_eq!(Status::Captured.merge(Status::Ignored), Status::Captured);
    /// assert_eq!(Status::Captured.merge(Status::Captured), Status::Captured);
    /// ```
    pub fn merge(self, b: Self) -> Self {
        match self {
            Status::Ignored => b,
            Status::Captured => Status::Captured,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_moves_coalesce_but_wheel_deltas_are_critical() {
        let moved = Event::Mouse(mouse::Event::CursorMoved {
            position: crate::Point::ORIGIN,
        });
        let scrolled = Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Pixels { x: 0.0, y: 1.0 },
        });

        assert_eq!(moved.coalescing(), Coalescing::Mergeable);
        assert_eq!(scrolled.coalescing(), Coalescing::Critical);
    }

    #[test]
    fn finger_moved_is_mergeable() {
        let event = Event::Touch(touch::Event::FingerMoved {
            id: touch::Finger(0),
            position: crate::Point::ORIGIN,
            metadata: crate::pointer::Metadata::NONE,
        });

        assert_eq!(event.coalescing(), Coalescing::Mergeable);
    }

    #[test]
    fn invalid_touch_motion_cannot_replace_valid_queued_motion() {
        let invalid = Event::Touch(touch::Event::FingerMoved {
            id: touch::Finger(17),
            position: crate::Point::new(f32::NAN, 10.0),
            metadata: crate::pointer::Metadata::NONE,
        });
        assert_eq!(invalid.coalescing(), Coalescing::Critical);
    }

    #[test]
    fn button_and_finger_lifecycle_events_are_critical() {
        let pressed = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        let released = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
        let entered = Event::Mouse(mouse::Event::CursorEntered);
        let left = Event::Mouse(mouse::Event::CursorLeft);

        for event in [pressed, released, entered, left] {
            assert_eq!(event.coalescing(), Coalescing::Critical);
        }

        let finger_pressed = Event::Touch(touch::Event::FingerPressed {
            id: touch::Finger(0),
            position: crate::Point::ORIGIN,
            metadata: crate::pointer::Metadata::NONE,
        });
        let finger_lifted = Event::Touch(touch::Event::FingerLifted {
            id: touch::Finger(0),
            position: crate::Point::ORIGIN,
            metadata: crate::pointer::Metadata::NONE,
        });
        let finger_lost = Event::Touch(touch::Event::FingerLost {
            id: touch::Finger(0),
            position: crate::Point::ORIGIN,
            metadata: crate::pointer::Metadata::NONE,
        });

        for event in [finger_pressed, finger_lifted, finger_lost] {
            assert_eq!(event.coalescing(), Coalescing::Critical);
        }
    }

    #[test]
    fn every_non_pointer_event_kind_is_critical() {
        assert_eq!(
            Event::Keyboard(keyboard::Event::ModifiersChanged(
                keyboard::Modifiers::default()
            ))
            .coalescing(),
            Coalescing::Critical
        );
        assert_eq!(Event::Waken.coalescing(), Coalescing::Critical);
        assert_eq!(Event::Reactive(7).coalescing(), Coalescing::Critical);
    }

    #[test]
    fn invalid_active_pointer_samples_cannot_enter_gesture_arena() {
        let invalid = crate::Point::new(f32::NAN, 1.0);
        let down = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        let move_event = Event::Mouse(mouse::Event::CursorMoved { position: invalid });

        assert_eq!(down.to_pointer(Some(invalid), now()), None);
        assert_eq!(move_event.to_pointer(None, now()), None);
        assert_eq!(move_event.coalescing(), Coalescing::Critical);
    }

    #[test]
    fn invalid_terminal_coordinates_still_release_existing_contacts() {
        let invalid = crate::Point::new(f32::INFINITY, 1.0);
        let released_mouse = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
        let mouse_up = released_mouse.to_pointer(Some(invalid), now()).unwrap();
        assert_eq!(mouse_up.phase, pointer::Phase::Cancel);
        assert_eq!(mouse_up.position, crate::Point::ORIGIN);

        let touch_move = Event::Touch(touch::Event::FingerMoved {
            id: touch::Finger(4),
            position: invalid,
            metadata: pointer::Metadata::NONE,
        });
        assert_eq!(touch_move.to_pointer(None, now()), None);

        let touch_release = Event::Touch(touch::Event::FingerLifted {
            id: touch::Finger(4),
            position: invalid,
            metadata: pointer::Metadata::NONE,
        });
        let up = touch_release.to_pointer(None, now()).unwrap();
        assert_eq!(up.phase, pointer::Phase::Cancel);
        assert_eq!(up.position, crate::Point::ORIGIN);
        assert!(up.is_terminal());
    }

    #[test]
    fn status_merge_prefers_captured() {
        assert_eq!(Status::Ignored.merge(Status::Ignored), Status::Ignored);
        assert_eq!(Status::Ignored.merge(Status::Captured), Status::Captured);
        assert_eq!(Status::Captured.merge(Status::Ignored), Status::Captured);
        assert_eq!(Status::Captured.merge(Status::Captured), Status::Captured);
    }

    fn now() -> crate::time::Instant {
        static EPOCH: std::sync::OnceLock<crate::time::Instant> = std::sync::OnceLock::new();
        *EPOCH.get_or_init(crate::time::Instant::now)
    }

    #[test]
    fn touch_events_convert_to_pointer_directly() {
        let event = Event::Touch(touch::Event::FingerPressed {
            id: touch::Finger(3),
            position: crate::Point::new(5.0, 6.0),
            metadata: crate::pointer::Metadata::NONE,
        });

        let converted = event.to_pointer(None, now()).unwrap();

        assert_eq!(converted.phase, pointer::Phase::Down);
        assert_eq!(converted.position, crate::Point::new(5.0, 6.0));
        assert_eq!(converted.device, pointer::DeviceKind::Touch);
    }

    #[test]
    fn mouse_button_events_use_the_supplied_cursor_position() {
        let pressed = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        let cursor = crate::Point::new(10.0, 20.0);

        let converted = pressed.to_pointer(Some(cursor), now()).unwrap();

        assert_eq!(converted.phase, pointer::Phase::Down);
        assert_eq!(converted.position, cursor);
        assert_eq!(converted.id, pointer::Id::MOUSE);
    }

    #[test]
    fn mouse_button_events_with_no_cursor_position_produce_no_pointer() {
        let pressed = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));

        assert_eq!(pressed.to_pointer(None, now()), None);
    }

    #[test]
    fn cursor_moved_carries_its_own_position() {
        let moved = Event::Mouse(mouse::Event::CursorMoved {
            position: crate::Point::new(1.0, 2.0),
        });

        let converted = moved.to_pointer(None, now()).unwrap();

        assert_eq!(converted.phase, pointer::Phase::Move);
        assert_eq!(converted.position, crate::Point::new(1.0, 2.0));
    }

    #[test]
    fn cursor_boundary_and_wheel_events_have_no_pointer_representation() {
        let entered = Event::Mouse(mouse::Event::CursorEntered);
        let left = Event::Mouse(mouse::Event::CursorLeft);
        let scrolled = Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Pixels { x: 0.0, y: 1.0 },
        });

        for event in [entered, left, scrolled] {
            assert_eq!(event.to_pointer(Some(crate::Point::ORIGIN), now()), None);
        }
    }

    #[test]
    fn non_pointer_event_kinds_have_no_pointer_representation() {
        assert_eq!(
            Event::Keyboard(keyboard::Event::ModifiersChanged(
                keyboard::Modifiers::default()
            ))
            .to_pointer(None, now()),
            None
        );
        assert_eq!(Event::Waken.to_pointer(None, now()), None);
        assert_eq!(Event::Reactive(7).to_pointer(None, now()), None);
    }
}
