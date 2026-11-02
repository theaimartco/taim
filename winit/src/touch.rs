//! Which finger drives the pointer.
//!
//! The first finger on the screen becomes the pointer: views receive it as
//! a move, a press and a release, so every view that works with a mouse
//! works with touch. Further fingers are ignored until it lifts.
use winit::event::TouchPhase;

/// What a finger's report means for the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    /// The pointer arrives and presses.
    Press,
    /// The pointer moves.
    Move,
    /// The pointer releases and leaves.
    Release,
    /// The system took the gesture: the pointer leaves, then releases, so
    /// nothing under it activates.
    Cancel,
    /// Not the pointer's finger.
    Ignore,
}

/// Tracks the finger that is the pointer.
#[derive(Debug, Default)]
pub(crate) struct Fingers {
    primary: Option<u64>,
}

impl Fingers {
    pub fn step(&mut self, finger: u64, phase: TouchPhase) -> Step {
        let primary = self.primary == Some(finger);

        match phase {
            TouchPhase::Started if self.primary.is_none() => {
                self.primary = Some(finger);
                Step::Press
            }
            TouchPhase::Moved if primary => Step::Move,
            TouchPhase::Ended if primary => {
                self.primary = None;
                Step::Release
            }
            TouchPhase::Cancelled if primary => {
                self.primary = None;
                Step::Cancel
            }
            _ => Step::Ignore,
        }
    }

    /// Forgets every finger: the window lost its surface.
    pub fn reset(&mut self) {
        self.primary = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_finger_is_the_pointer_from_press_to_release() {
        let mut fingers = Fingers::default();

        assert_eq!(fingers.step(7, TouchPhase::Started), Step::Press);
        assert_eq!(fingers.step(7, TouchPhase::Moved), Step::Move);
        assert_eq!(fingers.step(7, TouchPhase::Ended), Step::Release);
    }

    #[test]
    fn a_second_finger_is_ignored_while_the_first_is_down() {
        let mut fingers = Fingers::default();

        assert_eq!(fingers.step(1, TouchPhase::Started), Step::Press);
        assert_eq!(fingers.step(2, TouchPhase::Started), Step::Ignore);
        assert_eq!(fingers.step(2, TouchPhase::Moved), Step::Ignore);
        assert_eq!(fingers.step(2, TouchPhase::Ended), Step::Ignore);
        assert_eq!(fingers.step(1, TouchPhase::Moved), Step::Move);
    }

    #[test]
    fn after_the_first_lifts_the_next_finger_takes_over() {
        let mut fingers = Fingers::default();

        let _ = fingers.step(1, TouchPhase::Started);
        let _ = fingers.step(2, TouchPhase::Started);
        assert_eq!(fingers.step(1, TouchPhase::Ended), Step::Release);
        assert_eq!(
            fingers.step(2, TouchPhase::Moved),
            Step::Ignore,
            "a finger that landed while another was the pointer stays ignored"
        );
        assert_eq!(fingers.step(3, TouchPhase::Started), Step::Press);
    }

    #[test]
    fn a_cancelled_gesture_cancels_and_frees_the_pointer() {
        let mut fingers = Fingers::default();

        let _ = fingers.step(4, TouchPhase::Started);
        assert_eq!(fingers.step(4, TouchPhase::Cancelled), Step::Cancel);
        assert_eq!(fingers.step(5, TouchPhase::Started), Step::Press);
    }

    #[test]
    fn reset_forgets_a_finger_whose_end_was_never_reported() {
        let mut fingers = Fingers::default();

        let _ = fingers.step(4, TouchPhase::Started);
        fingers.reset();
        assert_eq!(fingers.step(9, TouchPhase::Started), Step::Press);
    }
}
