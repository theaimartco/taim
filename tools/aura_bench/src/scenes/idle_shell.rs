//! Static shell / no-redraw case.
//!
//! The simplest possible scene: a representative application shell (a
//! header, a sidebar, and a content area) that never changes. Every
//! frame after the first should be a no-op once Phase 2-4 land clean-
//! subtree reuse and idle detection; Phase 1 records this scene's
//! baseline cost precisely so those later phases have something to
//! compare against (see `phase.md`'s idle-scene exit gates in section 3).

use crate::scenes::{Element, Scene};

use aura_core::Length;
use aura_widget::{column, container, row, text};

/// Command-line/report identifier for this scene.
pub const NAME: &str = "idle_shell";

/// The `idle_shell` scene's (empty) message type: nothing ever changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {}

/// A static application shell that never mutates.
#[derive(Debug, Default)]
pub struct IdleShell;

impl Scene for IdleShell {
    type Message = Message;

    fn name(&self) -> &'static str {
        NAME
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let header = container(text("VYOM Aura")).width(Length::Fill).padding(16);

        let sidebar = container(column![
            text("Overview"),
            text("Activity"),
            text("Settings"),
        ])
        .width(Length::Fixed(240.0))
        .padding(16);

        let content = container(text(
            "This shell never changes between frames. Once incremental \
             invalidation lands (Phase 2-4), a clean frame here should do \
             zero layout and zero paint work.",
        ))
        .width(Length::Fill)
        .padding(16);

        column![header, row![sidebar, content]].into()
    }

    fn update(&mut self, message: Self::Message) {
        match message {}
    }

    fn advance(&mut self, _frame_index: u64) -> Vec<Self::Message> {
        Vec::new()
    }
}
