//! Fixed-height and variable-height `VirtualList` workloads over a
//! genuinely huge (100,000-row) logical row count.
//!
//! Unlike `ten_thousand_rows` (Phase 1's own hand-rolled visible-window
//! scene, kept as-is per its own doc comment as "a Phase 1 baseline that
//! Phase 7 is expected to match or beat"), this scene drives the real
//! `taim_widget::VirtualList` widget, so its measured cost is exactly
//! what a real application embedding a `VirtualList` would pay.
//!
//! `taim_bench`'s harness (`main.rs`'s `drive_one_frame`) rebuilds a
//! fresh `UserInterface` -- and therefore a fresh `VirtualList` widget
//! `Tree`, with its scroll offset reset to zero -- every single frame
//! (see `user_interface::Cache::new()` at that call site); no widget's
//! internal state persists from one measured frame to the next in this
//! harness. So rather than pretending to track a `VirtualList`'s scroll
//! position across frames from the scene side (which would measure
//! nothing real, since the widget itself never sees it), this scene
//! injects a real `mouse::Event::WheelScrolled` via
//! [`Scene::input_events`] every frame, so what gets measured is the
//! genuine cost of "a `VirtualList` receives one scroll event and
//! re-renders," including an occasional huge one-shot delta to exercise
//! the same jump-to-position path `tests/widgets/virtual_list.rs`
//! proves correct.
use crate::scenes::{Element, Scene};

use taim_core::mouse;
use taim_core::{Event, Length};
use taim_widget::{container, text};

/// Command-line/report identifier for this scene.
pub const NAME: &str = "virtual_list";

/// Total logical row count, matching `phase.md`'s own "100,000-row
/// benchmark" exit-gate language for `VirtualList` exactly.
pub const TOTAL_ROWS: usize = 100_000;

const FIXED_ROW_HEIGHT: f32 = 24.0;

/// Which height mode this run of the scene exercises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Every row has the same, fixed height.
    Fixed,
    /// Row heights alternate between two values, exercising
    /// `VirtualList::variable`'s cumulative-position table.
    Variable,
}

/// What a frame does to the scene's own state (row content churn only --
/// scrolling is driven separately via [`Scene::input_events`], see this
/// module's own doc comment for why).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// Updates the text of one row, simulating a live data change (e.g.
    /// a chat message's unread badge). The row is not necessarily
    /// visible in any given frame (this scene has no persisted scroll
    /// state to check against), matching how a real application cannot
    /// generally know what is on-screen in some other client either.
    UpdateRow(usize),
}

/// A synthetic `VirtualList` scene with `TOTAL_ROWS` rows.
#[derive(Debug)]
pub struct VirtualListScene {
    mode: Mode,
    seed: u64,
    /// Per-row generation counter, bumped by `UpdateRow`; folded into
    /// each row's label so the row's content is provably different
    /// after an update.
    row_generation: Vec<u32>,
}

impl VirtualListScene {
    /// Creates the fixed-height variant of the scene.
    pub fn fixed(seed: u64) -> Self {
        Self::new(Mode::Fixed, seed)
    }

    /// Creates the variable-height variant of the scene.
    pub fn variable(seed: u64) -> Self {
        Self::new(Mode::Variable, seed)
    }

    fn new(mode: Mode, seed: u64) -> Self {
        Self {
            mode,
            seed,
            row_generation: vec![0; TOTAL_ROWS],
        }
    }

    #[cfg(test)]
    fn row_height(&self, index: usize) -> f32 {
        Self::height_of(self.mode, index)
    }

    /// The height of `index` under `mode` -- a free function (not a
    /// `&self` method) so it can be captured by value in the `move`
    /// closures [`VirtualListScene::view`] hands to `VirtualList`,
    /// which must not borrow `self` across frames.
    fn height_of(mode: Mode, index: usize) -> f32 {
        match mode {
            Mode::Fixed => FIXED_ROW_HEIGHT,
            Mode::Variable => {
                if index.is_multiple_of(2) {
                    FIXED_ROW_HEIGHT
                } else {
                    FIXED_ROW_HEIGHT * 2.0
                }
            }
        }
    }

    fn total_extent(&self) -> f32 {
        match self.mode {
            Mode::Fixed => TOTAL_ROWS as f32 * FIXED_ROW_HEIGHT,
            Mode::Variable => {
                // Alternating fixed/double-height rows, closed form:
                // half at `FIXED_ROW_HEIGHT`, half at `2 *
                // FIXED_ROW_HEIGHT`.
                let pairs = TOTAL_ROWS as f32 / 2.0;

                pairs * (FIXED_ROW_HEIGHT + FIXED_ROW_HEIGHT * 2.0)
            }
        }
    }

    /// A small deterministic PRNG (splitmix64), matching
    /// `ten_thousand_rows::TenThousandRows::pseudo_random` exactly so
    /// this scene needs no external `rand` dependency and stays
    /// byte-identical across runs and platforms for a given seed.
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

impl Scene for VirtualListScene {
    type Message = Message;

    fn name(&self) -> &'static str {
        match self.mode {
            Mode::Fixed => "virtual_list",
            Mode::Variable => "virtual_list_variable",
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let generations = &self.row_generation;
        let mode = self.mode;

        let list = match mode {
            Mode::Fixed => taim_widget::virtual_list::VirtualList::new(
                TOTAL_ROWS,
                FIXED_ROW_HEIGHT,
                move |index| {
                    let generation = generations[index];

                    Element::from(text(format!(
                        "row {index} of {TOTAL_ROWS} (rev {generation})"
                    )))
                },
            ),
            Mode::Variable => taim_widget::virtual_list::VirtualList::variable(
                TOTAL_ROWS,
                move |index| Self::height_of(Mode::Variable, index),
                move |index| {
                    let generation = generations[index];

                    Element::from(text(format!(
                        "row {index} of {TOTAL_ROWS} (rev {generation})"
                    )))
                },
            ),
        }
        .width(Length::Fill)
        .height(Length::Fill);

        container(list).padding(8).into()
    }

    fn update(&mut self, message: Self::Message) {
        match message {
            Message::UpdateRow(index) => {
                if let Some(generation) = self.row_generation.get_mut(index) {
                    *generation = generation.wrapping_add(1);
                }
            }
        }
    }

    fn advance(&mut self, frame_index: u64) -> Vec<Self::Message> {
        // Every tenth frame updates one row's content; scrolling itself
        // is driven separately via `input_events` (see this module's
        // own doc comment for why it cannot be modeled as scene state).
        if frame_index % 10 == 4 {
            let target = (self.pseudo_random(frame_index) as usize) % TOTAL_ROWS;

            vec![Message::UpdateRow(target)]
        } else {
            Vec::new()
        }
    }

    fn input_events(&self, frame_index: u64) -> Vec<Event> {
        // Every tenth frame (a different one than the row-update case)
        // injects one huge, one-shot scroll delta -- a "jump to
        // position" -- covering close to the full extent of the list,
        // to exercise the same large-delta path
        // `tests/widgets/virtual_list.rs`'s
        // `scrolling_to_the_end_brings_the_last_row_into_view` proves
        // correct. Every other frame gets an ordinary, small scroll
        // delta, simulating steady wheel input.
        let delta_y = if frame_index % 10 == 9 {
            -(self.total_extent() / 2.0)
        } else {
            -FIXED_ROW_HEIGHT
        };

        vec![Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Pixels { x: 0.0, y: delta_y },
        })]
    }
}

/// The variable-height variant of the scene, registered under its own
/// name/constructor in `main.rs`'s `dispatch!` table -- which maps one
/// scene *name* to one concrete *type*, so the two height modes need
/// distinct types even though they share every bit of logic via
/// [`VirtualListScene`] composition.
#[derive(Debug)]
pub struct VirtualListVariable(VirtualListScene);

/// Command-line/report identifier for the variable-height variant.
pub const VARIABLE_NAME: &str = "virtual_list_variable";

impl VirtualListVariable {
    /// Creates the variable-height variant, seeded for deterministic
    /// jump/update patterns.
    pub fn new(seed: u64) -> Self {
        Self(VirtualListScene::variable(seed))
    }
}

impl Scene for VirtualListVariable {
    type Message = Message;

    fn name(&self) -> &'static str {
        VARIABLE_NAME
    }

    fn view(&self) -> Element<'_, Self::Message> {
        self.0.view()
    }

    fn update(&mut self, message: Self::Message) {
        self.0.update(message);
    }

    fn advance(&mut self, frame_index: u64) -> Vec<Self::Message> {
        self.0.advance(frame_index)
    }

    fn input_events(&self, frame_index: u64) -> Vec<Event> {
        self.0.input_events(frame_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_is_deterministic_for_a_fixed_seed() {
        let mut a = VirtualListScene::fixed(7);
        let mut b = VirtualListScene::fixed(7);

        for frame in 0..50 {
            assert_eq!(a.advance(frame), b.advance(frame));
        }
    }

    #[test]
    fn update_row_bumps_exactly_that_rows_generation() {
        let mut scene = VirtualListScene::fixed(1);

        scene.update(Message::UpdateRow(42));

        assert_eq!(scene.row_generation[42], 1);
        assert_eq!(scene.row_generation[0], 0);
    }

    #[test]
    fn variable_mode_total_extent_accounts_for_alternating_heights() {
        let scene = VirtualListScene::variable(1);

        // Half the rows at FIXED_ROW_HEIGHT, half at double that.
        let expected = (TOTAL_ROWS as f32 / 2.0) * (FIXED_ROW_HEIGHT + FIXED_ROW_HEIGHT * 2.0);

        assert_eq!(scene.total_extent(), expected);
    }

    #[test]
    fn variable_mode_row_heights_alternate() {
        let scene = VirtualListScene::variable(1);

        assert_eq!(scene.row_height(0), FIXED_ROW_HEIGHT);
        assert_eq!(scene.row_height(1), FIXED_ROW_HEIGHT * 2.0);
        assert_eq!(scene.row_height(2), FIXED_ROW_HEIGHT);
    }

    #[test]
    fn fixed_mode_row_heights_are_uniform() {
        let scene = VirtualListScene::fixed(1);

        for index in [0, 1, 2, 99, 99_999] {
            assert_eq!(scene.row_height(index), FIXED_ROW_HEIGHT);
        }
    }

    #[test]
    fn input_events_produce_exactly_one_wheel_event_per_frame() {
        let scene = VirtualListScene::fixed(1);

        for frame in 0..30 {
            let events = scene.input_events(frame);
            assert_eq!(events.len(), 1);
        }
    }
}
