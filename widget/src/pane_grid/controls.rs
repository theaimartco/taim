//! TAIM adaptive pane controls.
//!
//! A title bar must make one consistent control-selection decision for
//! drawing, pointer hit-testing, layout, accessibility and overlays. The
//! selection depends on measured bounds, not on the input device or its
//! event phase: a hidden control cannot continue receiving keyboard input.

use crate::container;
use crate::core::{self, Element};

/// Actual control/title presentation given a pane's available width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Presentation {
    /// Full controls and the title are both visible.
    FullWithTitle,
    /// Compact controls replace full controls; the title remains visible.
    CompactWithTitle,
    /// Full controls take priority when no compact variant exists.
    FullWithoutTitle,
}

/// Full and optional compact representations of a pane's command surface.
pub struct Controls<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Theme: container::Catalog,
    Renderer: core::Renderer,
{
    pub(super) full: Element<'a, Message, Theme, Renderer>,
    pub(super) compact: Option<Element<'a, Message, Theme, Renderer>>,
}

impl<'a, Message, Theme, Renderer> Controls<'a, Message, Theme, Renderer>
where
    Theme: container::Catalog,
    Renderer: core::Renderer,
{
    /// Construct controls that remain fully visible even in narrow panes.
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            full: content.into(),
            compact: None,
        }
    }

    /// Provide a second compact controls view for narrow pane titles.
    pub fn dynamic(
        full: impl Into<Element<'a, Message, Theme, Renderer>>,
        compact: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        Self {
            full: full.into(),
            compact: Some(compact.into()),
        }
    }

    /// Resolve title/control visibility once from measured bounds.
    ///
    /// When the full controls cannot coexist with the title, the compact
    /// controls win if present. Otherwise full controls win and the title is
    /// suppressed. NaN/overflow measurement results never masquerade as
    /// fitting; infinity on the available-width side denotes no constraint.
    pub(crate) fn presentation(
        &self,
        title_width: f32,
        controls_width: f32,
        available_width: f32,
    ) -> Presentation {
        select_presentation(
            title_width,
            controls_width,
            available_width,
            self.compact.is_some(),
        )
    }
}

impl<'a, Message, Theme, Renderer> From<Element<'a, Message, Theme, Renderer>>
    for Controls<'a, Message, Theme, Renderer>
where
    Theme: container::Catalog,
    Renderer: core::Renderer,
{
    fn from(value: Element<'a, Message, Theme, Renderer>) -> Self {
        Self::new(value)
    }
}

/// Pure visibility contract, tested without renderer or window dependencies.
fn select_presentation(
    title_width: f32,
    full_width: f32,
    available_width: f32,
    compact_available: bool,
) -> Presentation {
    let total = title_width + full_width;
    if total.is_finite() && total <= available_width {
        Presentation::FullWithTitle
    } else if compact_available {
        Presentation::CompactWithTitle
    } else {
        Presentation::FullWithoutTitle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitting_layout_keeps_title_and_full_controls() {
        assert_eq!(
            select_presentation(60.0, 30.0, 100.0, true),
            Presentation::FullWithTitle
        );
        assert_eq!(
            select_presentation(60.0, 40.0, 100.0, false),
            Presentation::FullWithTitle
        );
    }

    #[test]
    fn compact_variant_wins_when_full_layout_overflows() {
        assert_eq!(
            select_presentation(60.0, 41.0, 100.0, true),
            Presentation::CompactWithTitle
        );
        assert_eq!(
            select_presentation(60.0, 41.0, 100.0, false),
            Presentation::FullWithoutTitle
        );
    }

    #[test]
    fn invalid_measurements_cannot_reveal_hidden_title_overlays() {
        assert_eq!(
            select_presentation(f32::NAN, 20.0, 100.0, false),
            Presentation::FullWithoutTitle
        );
        assert_eq!(
            select_presentation(40.0, f32::INFINITY, 100.0, true),
            Presentation::CompactWithTitle
        );
        assert_eq!(
            select_presentation(40.0, 20.0, f32::INFINITY, false),
            Presentation::FullWithTitle
        );
    }
}
