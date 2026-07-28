//! Transforms, opacity, and layout animation stress.
//!
//! There is no motion engine yet (Phase 9's `aura-motion`), so this scene
//! drives its own per-frame interpolation deterministically from the
//! frame index rather than wall-clock time -- this keeps the scene
//! reproducible and matches phase.md's requirement that motion later be
//! "based on elapsed monotonic time, not frame count" without having to
//! anticipate that API here.

use crate::scenes::{Element, Scene};

use aura_core::Length;
use aura_widget::{container, opaque, row, space};

/// Command-line/report identifier for this scene.
pub const NAME: &str = "animated_cards";

/// Number of simultaneously animating cards.
const CARD_COUNT: usize = 24;

/// Total frames in one animation loop before it repeats.
const LOOP_LENGTH: u64 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// Advances every card's animation phase by one frame.
    Tick,
}

/// A grid of cards whose size and opacity oscillate every frame.
#[derive(Debug)]
pub struct AnimatedCards {
    frame: u64,
}

impl AnimatedCards {
    /// Creates the scene at frame zero.
    pub fn new() -> Self {
        Self { frame: 0 }
    }

    fn phase_of(&self, card_index: usize) -> f32 {
        let offset = (card_index as u64 * (LOOP_LENGTH / CARD_COUNT.max(1) as u64)) % LOOP_LENGTH;
        let position = (self.frame + offset) % LOOP_LENGTH;

        // Deterministic pseudo-sine via a triangle wave: avoids pulling
        // in a math dependency purely for a benchmark stub and keeps the
        // computation trivially reproducible bit-for-bit.
        let normalized = position as f32 / LOOP_LENGTH as f32;

        if normalized < 0.5 {
            normalized * 2.0
        } else {
            2.0 - normalized * 2.0
        }
    }
}

impl Default for AnimatedCards {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene for AnimatedCards {
    type Message = Message;

    fn name(&self) -> &'static str {
        NAME
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let cards = (0..CARD_COUNT).map(|index| {
            let phase = self.phase_of(index);
            let size = 60.0 + phase * 40.0;

            let card = container(space().width(size).height(size)).style(
                move |theme: &aura_core::Theme| {
                    let palette = theme.palette();

                    container::Style::default()
                        .background(palette.primary.base.color)
                        .border(aura_core::border::rounded(8))
                },
            );

            Element::from(opaque(card))
        });

        container(
            aura_widget::scrollable(row(cards).spacing(12).wrap())
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .padding(16)
        .into()
    }

    fn update(&mut self, message: Self::Message) {
        match message {
            Message::Tick => {
                self.frame = self.frame.wrapping_add(1);
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
    fn phase_stays_within_unit_range() {
        let mut scene = AnimatedCards::new();

        for frame in 0..(LOOP_LENGTH * 3) {
            scene.frame = frame;

            for index in 0..CARD_COUNT {
                let phase = scene.phase_of(index);
                assert!((0.0..=1.0).contains(&phase), "phase {phase} out of range");
            }
        }
    }

    #[test]
    fn advance_ticks_every_frame() {
        let mut scene = AnimatedCards::new();
        assert_eq!(scene.advance(0), vec![Message::Tick]);
    }
}
