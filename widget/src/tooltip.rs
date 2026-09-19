//! Delayed, viewport-aware informational overlays.
mod implementation;

use crate::container;
use crate::core::text;
use crate::core::time::Duration;
use crate::core::{Element, Pixels};

/// Displays a tooltip above one content widget.
pub struct Tooltip<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Theme: container::Catalog,
    Renderer: text::Renderer,
{
    pub(super) content: Element<'a, Message, Theme, Renderer>,
    pub(super) tooltip: Element<'a, Message, Theme, Renderer>,
    pub(super) position: Position,
    pub(super) gap: f32,
    pub(super) padding: f32,
    pub(super) snap_within_viewport: bool,
    pub(super) delay: Duration,
    pub(super) class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> Tooltip<'a, Message, Theme, Renderer>
where
    Theme: container::Catalog,
    Renderer: text::Renderer,
{
    const DEFAULT_PADDING: f32 = 5.0;

    /// Creates a tooltip around content.
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        tooltip: impl Into<Element<'a, Message, Theme, Renderer>>,
        position: Position,
    ) -> Self {
        Self {
            content: content.into(),
            tooltip: tooltip.into(),
            position,
            gap: 0.0,
            padding: Self::DEFAULT_PADDING,
            snap_within_viewport: true,
            delay: Duration::ZERO,
            class: Theme::default(),
        }
    }

    /// Sets nonnegative gap between anchor and tooltip.
    pub fn gap(mut self, gap: impl Into<Pixels>) -> Self {
        let gap = gap.into().0;
        self.gap = if gap.is_finite() { gap.max(0.0) } else { 0.0 };
        self
    }

    /// Sets nonnegative tooltip padding.
    pub fn padding(mut self, padding: impl Into<Pixels>) -> Self {
        let padding = padding.into().0;
        self.padding = if padding.is_finite() {
            padding.max(0.0)
        } else {
            Self::DEFAULT_PADDING
        };
        self
    }

    /// Sets hover delay.
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// Enables or disables viewport snapping.
    pub fn snap_within_viewport(mut self, snap: bool) -> Self {
        self.snap_within_viewport = snap;
        self
    }

    /// Sets a container style.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> container::Style + 'a) -> Self
    where
        Theme::Class<'a>: From<container::StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as container::StyleFn<'a, Theme>).into();
        self
    }

    /// Sets a container theme class.
    #[cfg(feature = "advanced")]
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }
}

/// Tooltip placement relative to its anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Position {
    /// Above the anchor.
    #[default]
    Top,
    /// Below the anchor.
    Bottom,
    /// Left of the anchor.
    Left,
    /// Right of the anchor.
    Right,
    /// Follow pointer coordinates.
    FollowCursor,
}
