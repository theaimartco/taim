//! Simultaneous motion stress scene.
//!
//! The scene cycles through 1, 10, 100, and 1,000 independent, retargetable
//! Taim Motion transitions. Time is simulated from the benchmark's fixed
//! 60 Hz sample cadence so every replay has identical positions; production
//! code receives the same elapsed time from its shared frame clock.

use taim_motion::{Easing, Timestamp, Transition};

use crate::scenes::{Element, Scene};

use taim_widget::{container, row, space};

/// Command-line/report identifier.
pub const NAME: &str = "motion_stress";

const LEVELS: [usize; 4] = [1, 10, 100, 1_000];
const LEVEL_FRAMES: u64 = 45;

/// Messages emitted by the deterministic driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// Samples motion at the next shared-clock timestamp.
    Tick,
}

/// A retained collection of independent transitions.
#[derive(Debug)]
pub struct MotionStress {
    transitions: Vec<Transition<f32>>,
    frame: u64,
}

impl MotionStress {
    /// Creates all stress levels once, avoiding allocation churn during frames.
    pub fn new() -> Self {
        let now = Timestamp::from_seconds(0.0);
        Self {
            transitions: (0..1_000).map(|_| Transition::new(0.0, now)).collect(),
            frame: 0,
        }
    }

    fn active_count(&self) -> usize {
        LEVELS[((self.frame / LEVEL_FRAMES) as usize) % LEVELS.len()]
    }

    fn timestamp(&self) -> Timestamp {
        Timestamp::from_seconds(self.frame as f64 / 60.0)
    }
}

impl Default for MotionStress {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene for MotionStress {
    type Message = Message;

    fn name(&self) -> &'static str {
        NAME
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let now = self.timestamp();
        let cards = self.transitions[..self.active_count()]
            .iter()
            .enumerate()
            .map(|(index, transition)| {
                // `Transition::value_at` updates only completion bookkeeping, so a
                // cloned value is used for this immutable `view` method.
                let mut transition = *transition;
                let phase = transition.value_at(now);
                let width = 6.0 + phase * 10.0 + (index % 3) as f32;
                Element::from(space().width(width).height(12.0))
            });

        container(row(cards).spacing(2).wrap()).padding(12).into()
    }

    fn update(&mut self, message: Self::Message) {
        match message {
            Message::Tick => {
                self.frame = self.frame.wrapping_add(1);
                let now = self.timestamp();
                let target = if (self.frame / 30).is_multiple_of(2) {
                    1.0
                } else {
                    0.0
                };
                let active_count = self.active_count();

                for transition in &mut self.transitions[..active_count] {
                    transition.retarget(target, now, 0.18, Easing::EaseInOut);
                    let _ = transition.value_at(now);
                }
            }
        }
    }

    fn advance(&mut self, _frame_index: u64) -> Vec<Self::Message> {
        vec![Message::Tick]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visits_each_required_stress_level() {
        let mut scene = MotionStress::new();
        let mut seen = Vec::new();

        for _ in 0..(LEVELS.len() as u64 * LEVEL_FRAMES) {
            seen.push(scene.active_count());
            scene.frame += 1;
        }

        for expected in LEVELS {
            assert!(seen.contains(&expected));
        }
    }
}
