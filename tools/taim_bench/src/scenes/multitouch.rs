//! Recorded deterministic pointer streams.
//!
//! Unlike the other scenes, `multitouch` is about the cost of the
//! event/hit-test dispatch path itself, not the view it produces. It
//! plays back a fixed, deterministically generated sequence of
//! multi-finger touch events (press, move, lift) through
//! [`crate::scenes::Scene::input_events`] against a row of interactive
//! buttons, so `UserInterface::update` has real widgets to hit-test
//! against. There is no gesture arena yet (Phase 8's `taim-gesture`), so
//! this only measures raw dispatch cost, not recognizer/competition
//! overhead.

use crate::scenes::{Element, Scene};

use taim_core::touch::{Event as TouchEvent, Finger};
use taim_core::{Event, Length, Point};
use taim_widget::{button, row, text};

/// Command-line/report identifier for this scene.
pub const NAME: &str = "multitouch";

/// Number of simultaneous simulated fingers.
const FINGER_COUNT: u64 = 5;

/// Number of buttons laid out for the fingers to interact with.
const BUTTON_COUNT: usize = 8;

/// How many frames a finger stays down (pressed, then moved this many
/// times) before it lifts and a new press begins.
const TOUCH_DURATION_FRAMES: u64 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// A button was activated; carries its index for observability, but
    /// this scene does not otherwise react to it.
    ButtonPressed(usize),
}

/// A row of buttons driven entirely by a recorded, deterministic
/// multi-finger touch stream.
#[derive(Debug)]
pub struct Multitouch {
    presses: u64,
}

impl Multitouch {
    /// Creates the scene.
    pub fn new() -> Self {
        Self { presses: 0 }
    }

    /// The deterministic position of `finger` at `frame_index`, tracing a
    /// small back-and-forth path across the button row so the same
    /// (finger, frame) pair always produces the same point.
    fn position_of(&self, finger: u64, frame_index: u64) -> Point {
        let cycle = frame_index % TOUCH_DURATION_FRAMES;
        let lane = (finger as f32) * 60.0 + 40.0;
        let sweep = (cycle as f32 / TOUCH_DURATION_FRAMES as f32) * 400.0;

        Point::new(lane + sweep, 40.0)
    }
}

impl Default for Multitouch {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene for Multitouch {
    type Message = Message;

    fn name(&self) -> &'static str {
        NAME
    }

    fn view(&self) -> Element<'_, Self::Message> {
        row((0..BUTTON_COUNT).map(|index| {
            Element::from(
                button(text(format!("btn {index}")))
                    .on_press(Message::ButtonPressed(index))
                    .width(Length::Fixed(80.0)),
            )
        }))
        .spacing(8)
        .into()
    }

    fn update(&mut self, message: Self::Message) {
        match message {
            Message::ButtonPressed(_) => {
                self.presses += 1;
            }
        }
    }

    fn advance(&mut self, _frame_index: u64) -> Vec<Self::Message> {
        // All of this scene's activity comes through `input_events`; the
        // resulting `Message`s (if any button ends up activated by the
        // replayed touch stream) are applied by the harness from
        // `UserInterface::update`'s own message queue, not synthesized
        // here.
        Vec::new()
    }

    fn input_events(&self, frame_index: u64) -> Vec<Event> {
        let mut events = Vec::with_capacity(FINGER_COUNT as usize);

        for finger in 0..FINGER_COUNT {
            let cycle_position = frame_index % TOUCH_DURATION_FRAMES;
            let id = Finger(finger);
            let position = self.position_of(finger, frame_index);

            let metadata = taim_core::pointer::Metadata::NONE;

            let touch = if cycle_position == 0 {
                TouchEvent::FingerPressed {
                    id,
                    position,
                    metadata,
                }
            } else if cycle_position == TOUCH_DURATION_FRAMES - 1 {
                TouchEvent::FingerLifted {
                    id,
                    position,
                    metadata,
                }
            } else {
                TouchEvent::FingerMoved {
                    id,
                    position,
                    metadata,
                }
            };

            events.push(Event::Touch(touch));
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_finger_produces_exactly_one_event_per_frame() {
        let scene = Multitouch::new();

        for frame in 0..(TOUCH_DURATION_FRAMES * 3) {
            let events = scene.input_events(frame);
            assert_eq!(events.len(), FINGER_COUNT as usize);
        }
    }

    #[test]
    fn touch_stream_is_deterministic_for_the_same_frame() {
        let scene = Multitouch::new();

        assert_eq!(scene.input_events(3), scene.input_events(3));
    }

    #[test]
    fn every_finger_eventually_lifts() {
        let scene = Multitouch::new();
        let events = scene.input_events(TOUCH_DURATION_FRAMES - 1);

        assert!(
            events
                .iter()
                .all(|event| matches!(event, Event::Touch(TouchEvent::FingerLifted { .. })))
        );
    }
}
