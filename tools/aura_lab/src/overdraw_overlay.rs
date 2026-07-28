//! How many times each pixel was written.
//!
//! # This panel shows nothing, and the reason is deeper than the last one
//!
//! `gpu_resources` is empty because nothing *counts*. This is empty
//! because nothing *can* count without changing how drawing works.
//!
//! Overdraw is a per-pixel fact: how many times a given pixel was written
//! while producing one frame. Aura's renderers do not know. `tiny_skia`
//! hands spans to a rasteriser; `wgpu` hands vertices to a GPU. Neither
//! keeps a count per pixel, and neither can be asked afterwards — the
//! information is destroyed by the act of drawing, which is what drawing
//! *is*.
//!
//! Getting it needs one of:
//!
//! - **A counting pass.** Render the frame a second time into an R32Uint
//!   target with additive blending and no colour, so every fragment adds
//!   one. Accurate, and it doubles the frame cost — acceptable for a
//!   profiler, but it is a second renderer path to write and keep correct.
//! - **Layer accounting.** Count how many layers claim each region,
//!   from the display list rather than the rasteriser. Cheap and
//!   approximate: it sees a full-screen quad behind an opaque one as two,
//!   which is true, and misses that a glyph only covers 8% of its own
//!   bounding box, which makes text look far worse than it is.
//!
//! The second is tempting because it is easy. It is also the one that
//! would put a red heat map over every paragraph in the application and
//! send a developer optimising text that was never the problem. A wrong
//! answer here is worse than none, because a heat map is *persuasive* —
//! nobody double-checks a picture.
//!
//! So: nothing, and the reason.

use aura::widget::{column, container, text};
use aura::{Element, Length};

/// Why this is empty, and what it would take.
pub const MISSING: &str = "Nothing counts per-pixel writes, and nothing can without a new render \
                           path. Overdraw is destroyed by the act of drawing: tiny_skia hands \
                           spans to a rasteriser and wgpu hands vertices to a GPU, and neither \
                           keeps a per-pixel count. It needs either a second counting pass \
                           (accurate, doubles the frame) or layer accounting (cheap, and wrong \
                           about text in a way that would send you optimising the wrong thing).";

/// Renders the overdraw panel.
pub fn view<'a, Message: 'a>() -> Element<'a, Message> {
    container(
        column![
            text("Overdraw is not measured.").size(16),
            text(MISSING).size(12),
            text(
                "A heat map built from layer bounds would be persuasive and wrong. Nobody \
                 double-checks a picture."
            )
            .size(12),
        ]
        .spacing(8)
        .width(Length::Fill),
    )
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .padding(12)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_says_why_it_is_empty_and_what_it_would_take() {
        // An empty panel with no explanation reads as a broken tool.
        assert!(MISSING.contains("counting pass"), "{MISSING}");
        assert!(MISSING.contains("layer accounting"), "{MISSING}");
    }

    #[test]
    fn the_panel_names_the_trap_rather_than_falling_into_it() {
        // The cheap approximation would put a red heat map over every
        // paragraph and send a developer optimising text that was never
        // the problem.
        assert!(MISSING.contains("wrong about text"), "{MISSING}");
    }

    #[test]
    fn it_renders_without_a_target() {
        // It has nothing to read, so it must not require anything.
        let element: Element<'_, ()> = view();

        let _ = std::hint::black_box(element);
    }
}
