//! Keep content clear of platform insets (notches, display cutouts,
//! system bars) that would otherwise obscure or crop it.
//!
//! There is, as of this phase, no live platform hook reporting real
//! insets from a running compositor -- `winit`/`wgpu` do not expose a
//! cross-platform "safe area" query today, mirroring the exact gap
//! `docs/architecture/frame-scheduling.md` already documents for
//! per-frame presentation feedback. [`SafeArea`] is the seam such a
//! source is expected to plug into: an application supplies the current
//! [`Insets`] (from whatever platform integration it has, or a constant
//! `Insets::NONE` on platforms without any), and [`SafeArea`] applies
//! them consistently as padding, honoring a [`CutoutPolicy`] per edge.
use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget::visit;
use crate::core::widget::{Operation, Tree};
use crate::core::{Element, Event, Length, Padding, Rectangle, Shell, Size, Vector, Widget};

/// The platform insets that should be avoided, one value per edge, in
/// pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Insets {
    /// The inset from the top edge (e.g. a status bar, a notch).
    pub top: f32,
    /// The inset from the right edge (e.g. a landscape notch/cutout).
    pub right: f32,
    /// The inset from the bottom edge (e.g. a home indicator, a
    /// navigation bar).
    pub bottom: f32,
    /// The inset from the left edge.
    pub left: f32,
}

impl Insets {
    /// No insets on any edge -- the safe area covers the whole surface.
    pub const NONE: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    /// Creates [`Insets`] that are equal on every edge.
    pub const fn all(amount: f32) -> Self {
        Self {
            top: amount,
            right: amount,
            bottom: amount,
            left: amount,
        }
    }
}

impl From<Insets> for Padding {
    fn from(insets: Insets) -> Self {
        Padding {
            top: insets.top,
            right: insets.right,
            bottom: insets.bottom,
            left: insets.left,
        }
    }
}

/// How a [`SafeArea`] should treat each edge's [`Insets`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutoutPolicy {
    /// Avoid every edge's inset: the content is padded away from all
    /// four edges by their respective [`Insets`] value.
    ///
    /// The correct default for ordinary content (text, controls) that
    /// would be unreadable or unreachable if drawn under a cutout.
    AvoidAll,

    /// Avoid only the top and bottom insets, letting content extend
    /// under any left/right cutout.
    ///
    /// Useful for content that is naturally centered and unlikely to
    /// have anything important near the left/right edges (e.g. a
    /// single-column reading view), while still respecting a status bar
    /// or home indicator.
    AvoidVertical,

    /// Avoid only the left and right insets.
    AvoidHorizontal,

    /// Ignore every inset and let content extend into the full surface,
    /// including under any cutout.
    ///
    /// Intended for content that is itself cutout-aware (e.g. a
    /// full-bleed background image/video) or that deliberately wants
    /// edge-to-edge presentation.
    Ignore,
}

impl CutoutPolicy {
    /// Applies this policy to `insets`, zeroing out any edge the policy
    /// does not avoid.
    pub fn apply(self, insets: Insets) -> Insets {
        match self {
            CutoutPolicy::AvoidAll => insets,
            CutoutPolicy::AvoidVertical => Insets {
                left: 0.0,
                right: 0.0,
                ..insets
            },
            CutoutPolicy::AvoidHorizontal => Insets {
                top: 0.0,
                bottom: 0.0,
                ..insets
            },
            CutoutPolicy::Ignore => Insets::NONE,
        }
    }
}

/// A container that pads its content away from platform [`Insets`],
/// according to a [`CutoutPolicy`].
pub struct SafeArea<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
    insets: Insets,
    policy: CutoutPolicy,
}

impl<'a, Message, Theme, Renderer> SafeArea<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    /// Wraps `content` in a [`SafeArea`] using the given [`Insets`] and
    /// [`CutoutPolicy::AvoidAll`].
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>, insets: Insets) -> Self {
        Self {
            content: content.into(),
            insets,
            policy: CutoutPolicy::AvoidAll,
        }
    }

    /// Sets the [`CutoutPolicy`] used to decide which edges are
    /// actually avoided.
    pub fn policy(mut self, policy: CutoutPolicy) -> Self {
        self.policy = policy;
        self
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for SafeArea<'_, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let padding = Padding::from(self.policy.apply(self.insets));

        layout::padded(limits, Length::Fill, Length::Fill, padding, |limits| {
            self.content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, limits)
        })
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        visit(&mut visit::Widgets::new(
            self.content.as_widget_mut(),
            &mut tree.children[0],
            layout.children().next().unwrap(),
        ));
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout.children().next().unwrap(),
                renderer,
                operation,
            );
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout.children().next().unwrap(),
            cursor,
            renderer,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout.children().next().unwrap(),
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<SafeArea<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: crate::core::Renderer + 'a,
{
    fn from(safe_area: SafeArea<'a, Message, Theme, Renderer>) -> Self {
        Self::new(safe_area)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avoid_all_keeps_every_edge() {
        let insets = Insets {
            top: 10.0,
            right: 20.0,
            bottom: 30.0,
            left: 40.0,
        };

        assert_eq!(CutoutPolicy::AvoidAll.apply(insets), insets);
    }

    #[test]
    fn avoid_vertical_zeroes_left_and_right() {
        let insets = Insets::all(10.0);
        let applied = CutoutPolicy::AvoidVertical.apply(insets);

        assert_eq!(applied.top, 10.0);
        assert_eq!(applied.bottom, 10.0);
        assert_eq!(applied.left, 0.0);
        assert_eq!(applied.right, 0.0);
    }

    #[test]
    fn avoid_horizontal_zeroes_top_and_bottom() {
        let insets = Insets::all(10.0);
        let applied = CutoutPolicy::AvoidHorizontal.apply(insets);

        assert_eq!(applied.top, 0.0);
        assert_eq!(applied.bottom, 0.0);
        assert_eq!(applied.left, 10.0);
        assert_eq!(applied.right, 10.0);
    }

    #[test]
    fn ignore_zeroes_every_edge() {
        let insets = Insets::all(10.0);

        assert_eq!(CutoutPolicy::Ignore.apply(insets), Insets::NONE);
    }

    #[test]
    fn insets_convert_to_matching_padding() {
        let insets = Insets {
            top: 1.0,
            right: 2.0,
            bottom: 3.0,
            left: 4.0,
        };

        let padding = Padding::from(insets);

        assert_eq!(padding.top, 1.0);
        assert_eq!(padding.right, 2.0);
        assert_eq!(padding.bottom, 3.0);
        assert_eq!(padding.left, 4.0);
    }
}
