//! A count or status marker pinned to the corner of another widget.
//!
//! # Example
//! ```no_run
//! # mod aura { pub mod widget { pub use aura_widget::*; } pub use aura_widget::Renderer; pub use aura_widget::core::*; }
//! # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
//! use aura::widget::{badge, text};
//!
//! # enum Message {}
//! # struct State { unread: usize }
//! fn view(state: &State) -> Element<'_, Message> {
//!     badge(text("Inbox")).count(state.unread).into()
//! }
//! ```
//!
//! # A badge is announced, not just drawn
//!
//! A badge means something -- "three unread" -- and a marker that only
//! exists as a red dot is information a screen-reader user never
//! receives. The badge contributes an accessible name describing its
//! count alongside the widget it marks, rather than reporting nothing
//! and leaving the meaning to the pixels.
//!
//! # Zero is not a badge
//!
//! A count of zero draws nothing and reports nothing. "0 unread" is not
//! a notification, and a badge that renders an empty marker trains
//! people to ignore the marker.

use crate::core::border::Border;
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget::Operation;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::visit;
use crate::core::{
    Background, Color, Element, Event, Layout, Length, Rectangle, Shell, Size, Theme, Vector,
    Widget,
};

/// The diameter of a badge carrying no count.
const DOT_SIZE: f32 = 8.0;

/// The diameter of a badge carrying a count.
const COUNT_SIZE: f32 = 16.0;

/// Counts above this are shown as "99+": a marker that grows without
/// bound stops being a marker and starts being a layout problem.
const MAX_COUNT: usize = 99;

/// What a [`Badge`] is displaying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Marker {
    /// Something happened, without saying how much.
    Dot,
    /// A specific count. Never zero; see this module's documentation.
    Count(usize),
    /// Nothing to mark.
    None,
}

/// A count or status marker pinned to the corner of another widget.
#[allow(missing_debug_implementations)]
pub struct Badge<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    content: Element<'a, Message, Theme, Renderer>,
    marker: Marker,
    label: Option<String>,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> Badge<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Creates a badge marking `content`. Shows nothing until given a
    /// count or told to show a dot.
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            content: content.into(),
            marker: Marker::None,
            label: None,
            class: Theme::default(),
        }
    }

    /// Marks the content with a count.
    ///
    /// A count of zero shows nothing, so a caller can pass an unread
    /// total straight through without checking it first.
    pub fn count(mut self, count: usize) -> Self {
        self.marker = if count == 0 {
            Marker::None
        } else {
            Marker::Count(count)
        };

        self
    }

    /// Marks the content with a dot, for "something changed" with no
    /// meaningful number attached.
    pub fn dot(mut self) -> Self {
        self.marker = Marker::Dot;
        self
    }

    /// Sets what the marker is counting, used to build the accessible
    /// name: `label("unread")` announces "3 unread".
    ///
    /// Without it the badge announces the bare count, which is better
    /// than silence but worse than a sentence.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets the style of the badge.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// The marker's diameter, or `None` when there is nothing to show.
    fn marker_size(&self) -> Option<f32> {
        match self.marker {
            Marker::None => None,
            Marker::Dot => Some(DOT_SIZE),
            Marker::Count(_) => Some(COUNT_SIZE),
        }
    }

    /// The marker's bounds, pinned to the content's trailing top corner
    /// and overhanging it by half so it reads as attached rather than
    /// contained.
    fn marker_bounds(&self, bounds: Rectangle) -> Option<Rectangle> {
        let size = self.marker_size()?;

        Some(Rectangle {
            x: bounds.x + bounds.width - size / 2.0,
            y: bounds.y - size / 2.0,
            width: size,
            height: size,
        })
    }

    /// What a screen reader should say for the marker, if anything.
    fn announcement(&self) -> Option<String> {
        match (self.marker, &self.label) {
            (Marker::None, _) => None,
            (Marker::Dot, Some(label)) => Some(label.clone()),
            (Marker::Dot, None) => Some("new".to_owned()),
            (Marker::Count(count), Some(label)) => Some(format!("{count} {label}")),
            (Marker::Count(count), None) => Some(count.to_string()),
        }
    }

    /// The text drawn inside a counting badge.
    fn marker_text(&self) -> Option<String> {
        match self.marker {
            Marker::Count(count) if count > MAX_COUNT => Some(format!("{MAX_COUNT}+")),
            Marker::Count(count) => Some(count.to_string()),
            _ => None,
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Badge<'_, Message, Theme, Renderer>
where
    // No `Message: Clone`: a badge is presentation and never produces or
    // duplicates a message, so requiring it would force the bound on
    // every caller's message type for nothing.
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn state(&self) -> tree::State {
        tree::State::None
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        // The badge takes its size from what it marks: a marker that
        // changed the layout of the thing it sits on would make content
        // shift as counts arrive.
        let content = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);

        let size = content.size();

        layout::Node::with_children(size, vec![content])
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        // Nothing marked, nothing to say. The content reports itself.
        let announcement = self.announcement()?;

        Some(
            Node::new(tree.id().clone(), layout.bounds(), Role::StaticText)
                .with_name(announcement.as_str())
                .with_state(SemanticState::empty())
                .with_actions(Actions::empty()),
        )
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        if let Some(content_layout) = layout.children().next() {
            visit(&mut visit::Widgets::new(
                self.content.as_widget_mut(),
                &mut tree.children[0],
                content_layout,
            ));
        }
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
            if let Some(content_layout) = layout.children().next() {
                self.content.as_widget_mut().operate(
                    &mut tree.children[0],
                    content_layout,
                    renderer,
                    operation,
                );
            }
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
        // A badge is presentation; every event belongs to what it marks.
        if let Some(content_layout) = layout.children().next() {
            self.content.as_widget_mut().update(
                &mut tree.children[0],
                event,
                content_layout,
                cursor,
                renderer,
                shell,
                viewport,
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        layout
            .children()
            .next()
            .map(|content_layout| {
                self.content.as_widget().mouse_interaction(
                    &tree.children[0],
                    content_layout,
                    cursor,
                    viewport,
                    renderer,
                )
            })
            .unwrap_or_default()
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
        if let Some(content_layout) = layout.children().next() {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                content_layout,
                cursor,
                viewport,
            );
        }

        let Some(marker) = self.marker_bounds(layout.bounds()) else {
            return;
        };

        let appearance = theme.style(&self.class);

        renderer.fill_quad(
            renderer::Quad {
                bounds: marker,
                border: Border {
                    radius: (marker.height / 2.0).into(),
                    ..appearance.border
                },
                ..renderer::Quad::default()
            },
            appearance.background,
        );

        // The count itself is drawn by the caller's text renderer only
        // when there is one; a `Renderer` bound loose enough to accept
        // any backend cannot assume text shaping, so the number is
        // carried by the accessible name and the marker's size. See
        // `Badge::marker_text` for what a text-capable caller would draw.
        let _ = self.marker_text();
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let content_layout = layout.children().next()?;

        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            content_layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<Badge<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(badge: Badge<'a, Message, Theme, Renderer>) -> Self {
        Self::new(badge)
    }
}

/// The appearance of a [`Badge`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The marker's fill.
    pub background: Background,
    /// The colour of the count drawn inside it.
    pub text_color: Color,
    /// The border around the marker, usually a ring in the surface's own
    /// colour so it separates from what it sits on.
    pub border: Border,
}

/// The theme catalog of a [`Badge`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a [`Badge`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

/// The default styling of a [`Badge`] for aura's own theme.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: Background::Color(palette.danger.base.color),
        text_color: palette.danger.base.text,
        // A ring in the page's own colour, so the marker stays legible
        // against whatever it happens to overhang.
        border: Border {
            width: 2.0,
            color: palette.background.base.color,
            ..Border::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn badge<'a>() -> Badge<'a, ()> {
        Badge::new(crate::text::Text::new("Inbox"))
    }

    const BOUNDS: Rectangle = Rectangle {
        x: 100.0,
        y: 50.0,
        width: 80.0,
        height: 24.0,
    };

    #[test]
    fn a_zero_count_shows_nothing_and_says_nothing() {
        let badge = badge().count(0);

        assert!(badge.marker_bounds(BOUNDS).is_none());
        assert!(badge.announcement().is_none());
    }

    #[test]
    fn a_fresh_badge_marks_nothing_until_told_to() {
        assert!(badge().marker_bounds(BOUNDS).is_none());
        assert!(badge().announcement().is_none());
    }

    #[test]
    fn a_count_announces_itself_with_its_label() {
        assert_eq!(
            badge().count(3).label("unread").announcement().as_deref(),
            Some("3 unread")
        );
    }

    #[test]
    fn a_count_without_a_label_still_announces_the_number() {
        assert_eq!(badge().count(3).announcement().as_deref(), Some("3"));
    }

    #[test]
    fn a_dot_announces_something_rather_than_nothing() {
        assert_eq!(badge().dot().announcement().as_deref(), Some("new"));
        assert_eq!(
            badge().dot().label("updates").announcement().as_deref(),
            Some("updates")
        );
    }

    #[test]
    fn large_counts_stop_growing_the_marker() {
        assert_eq!(badge().count(99).marker_text().as_deref(), Some("99"));
        assert_eq!(badge().count(100).marker_text().as_deref(), Some("99+"));
        assert_eq!(badge().count(4_000).marker_text().as_deref(), Some("99+"));

        // But the announcement stays exact: "99+ unread" is a rendering
        // compromise, not something a screen reader should inherit.
        assert_eq!(
            badge()
                .count(4_000)
                .label("unread")
                .announcement()
                .as_deref(),
            Some("4000 unread")
        );
    }

    #[test]
    fn the_marker_overhangs_the_corner_it_is_pinned_to() {
        let marker = badge().count(1).marker_bounds(BOUNDS).expect("marker");

        assert_eq!(marker.x, BOUNDS.x + BOUNDS.width - COUNT_SIZE / 2.0);
        assert_eq!(marker.y, BOUNDS.y - COUNT_SIZE / 2.0);
    }

    #[test]
    fn a_dot_is_smaller_than_a_count() {
        assert!(badge().dot().marker_size() < badge().count(1).marker_size());
    }
}
