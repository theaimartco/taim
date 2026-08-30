//! List churn, scroll, update, and jump cases over a large logical row
//! count.
//!
//! This scene renders a bounded visible window itself (there is no
//! `VirtualList` widget yet -- that is Phase 7's `widget/src/virtual_list.rs`)
//! so it can already measure "cost proportional to visible rows" as a
//! Phase 1 baseline that Phase 7 is expected to match or beat.

use crate::scenes::{Element, Scene};

use taim_core::Length;
use taim_widget::{column, container, scrollable, text};

/// Command-line/report identifier for this scene.
pub const NAME: &str = "ten_thousand_rows";

/// Total logical row count, matching phase.md's "100,000 logical rows"
/// virtualization target order of magnitude closely enough to stress
/// list construction cost, while `10_000` keeps this Phase 1 scene's
/// name accurate to what it actually renders per frame today (all rows
/// in the visible window are rebuilt every frame -- there is no
/// incremental reconciliation until Phase 3).
pub const TOTAL_ROWS: usize = 10_000;

/// How many rows are visible (and thus actually rendered) at once.
const VISIBLE_ROWS: usize = 24;

/// What a frame does to the scene, chosen deterministically by
/// [`Scene::advance`] from the frame index and the scene's seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// Scrolls forward by one row.
    ScrollForward,
    /// Updates the text of one currently visible row, simulating a live
    /// data change (e.g. an unread-count badge).
    UpdateRow(usize),
    /// Jumps to an arbitrary offset, simulating "scroll to end"/search.
    JumpTo(usize),
}

/// A synthetic scrollable list of `TOTAL_ROWS` rows.
#[derive(Debug)]
pub struct TenThousandRows {
    seed: u64,
    scroll_offset: usize,
    /// Per-row generation counter, bumped by `UpdateRow`; folded into
    /// each row's label so the row's content is provably different after
    /// an update.
    row_generation: Vec<u32>,
}

impl TenThousandRows {
    /// Creates the scene with `TOTAL_ROWS` rows, seeded for deterministic
    /// jump/update patterns.
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            scroll_offset: 0,
            row_generation: vec![0; TOTAL_ROWS],
        }
    }

    fn visible_range(&self) -> std::ops::Range<usize> {
        let start = self.scroll_offset.min(TOTAL_ROWS.saturating_sub(1));
        let end = (start + VISIBLE_ROWS).min(TOTAL_ROWS);

        start..end
    }

    /// A small deterministic PRNG (splitmix64) so this scene needs no
    /// external `rand` dependency and stays byte-identical across runs
    /// and platforms for a given seed.
    fn pseudo_random(&self, index: u64) -> u64 {
        let mut z = self
            .seed
            .wrapping_add(index)
            .wrapping_add(0x9E3779B97F4A7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

impl Scene for TenThousandRows {
    type Message = Message;

    fn name(&self) -> &'static str {
        NAME
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let rows = self.visible_range().map(|index| {
            let generation = self.row_generation[index];

            Element::from(text(format!(
                "row {index} of {TOTAL_ROWS} (rev {generation})"
            )))
        });

        container(scrollable(column(rows).width(Length::Fill).spacing(4)).height(Length::Fill))
            .padding(8)
            .into()
    }

    fn update(&mut self, message: Self::Message) {
        match message {
            Message::ScrollForward => {
                self.scroll_offset = (self.scroll_offset + 1).min(TOTAL_ROWS - VISIBLE_ROWS);
            }
            Message::UpdateRow(index) => {
                if let Some(generation) = self.row_generation.get_mut(index) {
                    *generation = generation.wrapping_add(1);
                }
            }
            Message::JumpTo(offset) => {
                self.scroll_offset = offset.min(TOTAL_ROWS - VISIBLE_ROWS);
            }
        }
    }

    fn advance(&mut self, frame_index: u64) -> Vec<Self::Message> {
        // A repeating pattern that exercises all three named cases:
        // mostly scrolling, with periodic in-place updates and an
        // occasional large jump -- deterministic given `frame_index`.
        match frame_index % 10 {
            9 => {
                let jump = (self.pseudo_random(frame_index) as usize) % TOTAL_ROWS;
                vec![Message::JumpTo(jump)]
            }
            4 => {
                let visible = self.visible_range();
                let target = visible.start
                    + (self.pseudo_random(frame_index) as usize) % visible.len().max(1);
                vec![Message::UpdateRow(target)]
            }
            _ => vec![Message::ScrollForward],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_is_deterministic_for_a_fixed_seed() {
        let mut a = TenThousandRows::new(7);
        let mut b = TenThousandRows::new(7);

        for frame in 0..50 {
            assert_eq!(a.advance(frame), b.advance(frame));
        }
    }

    #[test]
    fn scroll_offset_never_exceeds_the_last_valid_window() {
        let mut scene = TenThousandRows::new(1);

        for frame in 0..1000 {
            for message in scene.advance(frame) {
                scene.update(message);
            }
        }

        assert!(scene.scroll_offset <= TOTAL_ROWS - VISIBLE_ROWS);
    }
}
