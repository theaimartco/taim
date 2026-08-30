//! Benchmark scene for damage tracking performance.
//!
//! This scene exercises the damage tracking system with three
//! workloads that stress different aspects of the damage pipeline:
//!
//! - **Tiny damage**: a single widget updates in a large tree
//!   (measures the cost of damage tracking overhead when most of
//!   the tree is static).
//! - **Scattered damage**: random N widgets update simultaneously
//!   (measures merge heuristic performance and the threshold for
//!   collapsing to full-surface redraw).
//! - **Full-screen damage**: complete tree invalidation (measures
//!   the baseline cost and the effectiveness of the full-surface
//!   fallback path).

use super::Element;
use crate::scenes::Scene;

use taim_core::Length;
use taim_core::widget::Text;

/// The scene's `Scene::name`.
pub const NAME: &str = "damage_churn";

/// How many widgets are in the tree. A non-trivial but not extreme
/// number that exercises damage tracking without making the harness
/// itself the bottleneck.
const WIDGET_COUNT: usize = 200;

/// Number of widgets that change per frame in "scattered" mode.
const SCATTER_COUNT: usize = 10;

/// A benchmark scene that exercises damage tracking with varying
/// amounts of per-frame churn.
pub struct DamageChurn {
    /// Current frame index.
    frame: u64,
    /// Per-widget counters that change on certain frames.
    counters: Vec<u64>,
    /// Which mode we're in for this run's reporting.
    mode: ChurnMode,
}

/// The churn pattern for this scene run.
#[derive(Debug, Clone, Copy)]
enum ChurnMode {
    /// One widget changes per frame.
    Tiny,
    /// `SCATTER_COUNT` random widgets change per frame.
    Scattered,
    /// All widgets change every frame.
    Full,
}

/// Messages produced by this scene.
#[derive(Debug, Clone)]
pub enum Message {
    /// Increment a specific widget's counter.
    Increment(usize),
}

impl DamageChurn {
    /// Creates a new damage churn scene.
    pub fn new() -> Self {
        Self {
            frame: 0,
            counters: vec![0; WIDGET_COUNT],
            mode: ChurnMode::Tiny,
        }
    }
}

impl Scene for DamageChurn {
    type Message = Message;

    fn name(&self) -> &'static str {
        NAME
    }

    fn view(&self) -> Element<'_, Self::Message> {
        // Build a column of Text widgets, each showing its counter.
        // This is intentionally simple: the goal is to measure
        // damage tracking, not widget rendering.
        let children: Vec<Element<'_, Self::Message>> = self
            .counters
            .iter()
            .map(|count| Text::new(format!("{count}")).width(Length::Fill).into())
            .collect();

        taim_widget::Column::with_children(children)
            .spacing(2)
            .width(Length::Fill)
            .into()
    }

    fn update(&mut self, message: Self::Message) {
        match message {
            Message::Increment(index) => {
                if index < self.counters.len() {
                    self.counters[index] += 1;
                }
            }
        }
    }

    fn advance(&mut self, frame_index: u64) -> Vec<Self::Message> {
        self.frame = frame_index;

        // Cycle through modes every 100 frames for varied profiling
        self.mode = match (frame_index / 100) % 3 {
            0 => ChurnMode::Tiny,
            1 => ChurnMode::Scattered,
            _ => ChurnMode::Full,
        };

        match self.mode {
            ChurnMode::Tiny => {
                // One widget changes per frame
                let index = (frame_index as usize) % WIDGET_COUNT;
                vec![Message::Increment(index)]
            }
            ChurnMode::Scattered => {
                // SCATTER_COUNT widgets change per frame, evenly distributed
                (0..SCATTER_COUNT)
                    .map(|i| {
                        let index = ((frame_index as usize) * 37 + i * 23) % WIDGET_COUNT;
                        Message::Increment(index)
                    })
                    .collect()
            }
            ChurnMode::Full => {
                // All widgets change every frame
                (0..WIDGET_COUNT).map(Message::Increment).collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_name_is_snake_case() {
        let scene = DamageChurn::new();
        assert_eq!(scene.name(), NAME);
        assert!(
            NAME.chars()
                .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit()),
            "scene name must be snake_case"
        );
    }

    #[test]
    fn advance_produces_messages_for_each_mode() {
        let mut scene = DamageChurn::new();

        // Tiny mode (frame 0–99)
        let msgs = scene.advance(0);
        assert_eq!(msgs.len(), 1, "tiny mode should produce 1 message");

        // Scattered mode (frame 100–199)
        let msgs = scene.advance(100);
        assert_eq!(
            msgs.len(),
            SCATTER_COUNT,
            "scattered mode should produce {SCATTER_COUNT} messages"
        );

        // Full mode (frame 200–299)
        let msgs = scene.advance(200);
        assert_eq!(
            msgs.len(),
            WIDGET_COUNT,
            "full mode should produce {WIDGET_COUNT} messages"
        );
    }
}
