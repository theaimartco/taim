//! Deterministic scene registry.
//!
//! A [`Scene`] is a small state machine that produces an `Element` for
//! the current frame and can mutate its own state between frames to
//! simulate churn, scrolling, editing, or animation. Every scene is
//! seeded so that two runs with the same [`crate::config::Config::seed`]
//! produce byte-identical input sequences.

pub mod animated_cards;
pub mod blur_layers;
pub mod damage_churn;
pub mod effect_ladder;
pub mod idle_shell;
pub mod image_grid;
pub mod motion_stress;
pub mod multitouch;
pub mod ten_thousand_rows;
pub mod text_edit;
pub mod text_heavy;
pub mod virtual_list;

use taim_core::Event;

/// A generic widget with the workspace's default `Theme` and `Renderer`,
/// mirroring the alias the root `taim` facade crate provides (which
/// `taim_bench` does not depend on -- see `Cargo.toml`).
pub type Element<'a, Message> =
    taim_core::Element<'a, Message, taim_core::Theme, taim_renderer::Renderer>;

/// A benchmark scene: something that can be driven for a fixed number of
/// simulated frames and produces a view each time.
pub trait Scene {
    /// The scene's message type, kept internal to the scene so the
    /// harness does not need to know each scene's update logic.
    type Message: Clone + std::fmt::Debug + 'static;

    /// A short, unique, `snake_case` identifier used on the command line
    /// and in reports.
    fn name(&self) -> &'static str;

    /// Produces the current frame's view.
    fn view(&self) -> Element<'_, Self::Message>;

    /// Applies a message produced by driving the view (or a synthetic one
    /// injected by [`Scene::advance`]) to the scene's state.
    fn update(&mut self, message: Self::Message);

    /// Advances the scene's internal state by one simulated frame and
    /// returns the messages that frame's driver should apply, e.g. "the
    /// next row scrolled into view" or "the next character was typed".
    ///
    /// Deterministic: called with the same `frame_index` on two runs with
    /// the same seed, this must return the same messages.
    fn advance(&mut self, frame_index: u64) -> Vec<Self::Message>;

    /// Raw input events to dispatch through `UserInterface::update` for
    /// this frame, in addition to `advance`'s messages.
    ///
    /// Most scenes drive state directly via `advance`/`update` and never
    /// need this; it exists for scenes like `multitouch` that are
    /// specifically about exercising the pointer/event dispatch path
    /// itself, where the events (not their resulting messages) are what
    /// must be recorded and replayed deterministically.
    fn input_events(&self, _frame_index: u64) -> Vec<Event> {
        Vec::new()
    }
}

/// Looks up a scene by its [`Scene::name`], for the command-line `--scene`
/// argument. Returns the names of all registered scenes if `name` does
/// not match any of them, so callers can print a helpful error.
pub fn names() -> &'static [&'static str] {
    &[
        idle_shell::NAME,
        ten_thousand_rows::NAME,
        text_heavy::NAME,
        text_edit::NAME,
        image_grid::NAME,
        blur_layers::NAME,
        animated_cards::NAME,
        multitouch::NAME,
        motion_stress::NAME,
        damage_churn::NAME,
        effect_ladder::NAME,
        virtual_list::NAME,
        virtual_list::VARIABLE_NAME,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scene_name_is_unique_and_snake_case() {
        let mut seen = std::collections::HashSet::new();

        for name in names() {
            assert!(
                name.chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit()),
                "scene name '{name}' is not snake_case"
            );

            assert!(seen.insert(*name), "duplicate scene name '{name}'");
        }
    }
}
