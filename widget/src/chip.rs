//! A compact control representing an attribute, filter, or choice.
//!
//! # Example
//! ```no_run
//! # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::Renderer; pub use taim_widget::core::*; }
//! # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
//! use taim::widget::{chip, row, text};
//!
//! #[derive(Clone)]
//! enum Message {
//!     Toggled(usize),
//!     Removed(usize),
//! }
//!
//! # struct State { active: [bool; 2] }
//! fn view(state: &State) -> Element<'_, Message> {
//!     row![
//!         chip(text("Unread")).selected(state.active[0]).on_press(Message::Toggled(0)),
//!         chip(text("Starred")).selected(state.active[1]).on_delete(Message::Removed(1)),
//!     ]
//!     .spacing(8)
//!     .into()
//! }
//! ```
//!
//! # A chip's role follows what it does
//!
//! A chip that can be selected is a two-state control, and reports
//! [`Role::Checkbox`](taim_core::semantics::Role::Checkbox) with the
//! `CHECKED` flag -- the same role a
//! [`toggler`](crate::toggler) uses, for the same reason: platform
//! accessibility APIs model "two states, one control" as a checkbox
//! regardless of how it is drawn. A chip that merely performs an action
//! reports [`Role::Button`](taim_core::semantics::Role::Button), and one
//! that does neither is inert presentation.
//!
//! # The pill is 32px; the target is 48px
//!
//! Chips are drawn as a 32px pill, which is below the touch-target
//! policy. Rather than either shipping an undersized target or drawing a
//! chip twice the size everyone expects, the widget reserves the full
//! 48px of height and centres the visible pill inside it. The extra room
//! is real hit area, not padding.

use crate::core::border::Border;
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::touch;
use crate::core::widget::Operation;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::visit;
use crate::core::{
    Background, Color, Element, Event, Layout, Length, Padding, Point, Rectangle, Shell, Size,
    Theme, Vector, Widget,
};

/// The visible pill's height.
const PILL_HEIGHT: f32 = 32.0;

/// The height the widget actually occupies, so the chip clears the
/// touch-target policy. See this module's own documentation.
const TOUCH_HEIGHT: f32 = 48.0;

/// The width of the trailing region that removes the chip.
const DELETE_WIDTH: f32 = 24.0;

/// The size of the glyph drawn in that region.
const DELETE_GLYPH: f32 = 10.0;

/// A compact control representing an attribute, filter, or choice.
#[allow(missing_debug_implementations)]
pub struct Chip<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    content: Element<'a, Message, Theme, Renderer>,
    selected: bool,
    on_press: Option<Message>,
    on_delete: Option<Message>,
    padding: Padding,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> Chip<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Creates a chip around `content`.
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            content: content.into(),
            selected: false,
            on_press: None,
            on_delete: None,
            padding: Padding::new(0.0).left(12.0).right(12.0),
            class: Theme::default(),
        }
    }

    /// Sets whether the chip is selected.
    ///
    /// Selecting a chip that has no [`Chip::on_press`] is legitimate --
    /// a chip can display a state it does not let the user change -- so
    /// this does not imply interactivity on its own.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Produces `on_press` when the chip's body is activated.
    pub fn on_press(mut self, on_press: Message) -> Self {
        self.on_press = Some(on_press);
        self
    }

    /// Adds a trailing region that produces `on_delete`.
    pub fn on_delete(mut self, on_delete: Message) -> Self {
        self.on_delete = Some(on_delete);
        self
    }

    /// Sets the interior padding.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the style of the chip.
    ///
    /// The closure receives the selection state, because a chip's fill
    /// is the main thing that changes with it.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, bool) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Whether the chip accepts input at all.
    fn is_interactive(&self) -> bool {
        self.on_press.is_some() || self.on_delete.is_some()
    }

    /// The pill's bounds inside the widget's own, which is taller so the
    /// touch target clears the policy.
    fn pill_bounds(&self, bounds: Rectangle) -> Rectangle {
        Rectangle {
            x: bounds.x,
            y: bounds.y + (bounds.height - PILL_HEIGHT) / 2.0,
            width: bounds.width,
            height: PILL_HEIGHT,
        }
    }

    /// The trailing region that removes the chip, if it has one.
    fn delete_bounds(&self, bounds: Rectangle) -> Option<Rectangle> {
        let _ = self.on_delete.as_ref()?;

        // The full widget height, not the pill's: the affordance is small
        // but its target should not be.
        Some(Rectangle {
            x: bounds.x + bounds.width - self.padding.right - DELETE_WIDTH,
            y: bounds.y,
            width: DELETE_WIDTH,
            height: bounds.height,
        })
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Chip<'_, Message, Theme, Renderer>
where
    Message: Clone,
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
        Size::new(Length::Shrink, Length::Fixed(TOUCH_HEIGHT))
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let delete_width = if self.on_delete.is_some() {
            DELETE_WIDTH
        } else {
            0.0
        };

        // The content is bounded by the pill, not by the taller touch
        // target: a chip's label sits inside the visible shape, and the
        // extra height exists only to be tappable.
        let available_width =
            (limits.max().width - self.padding.left - self.padding.right - delete_width).max(0.0);

        let content_limits =
            layout::Limits::new(Size::ZERO, Size::new(available_width, PILL_HEIGHT));

        let content =
            self.content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, &content_limits);

        let content_size = content.size();

        let chip = Size::new(
            content_size.width + self.padding.left + self.padding.right + delete_width,
            TOUCH_HEIGHT,
        );

        let content = content.move_to(Point::new(
            self.padding.left,
            (TOUCH_HEIGHT - content_size.height) / 2.0,
        ));

        layout::Node::with_children(chip, vec![content])
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        // Inert presentation reports nothing: a decorative chip in the
        // accessibility tree is noise a screen-reader user has to step
        // through for no reason.
        if !self.is_interactive() && !self.selected {
            return None;
        }

        let mut flags = if self.is_interactive() {
            SemanticState::FOCUSABLE
        } else {
            SemanticState::DISABLED
        };

        if self.selected {
            flags |= SemanticState::CHECKED | SemanticState::SELECTED;
        }

        let mut actions = Actions::empty();

        if self.on_press.is_some() {
            actions |= Actions::ACTIVATE | Actions::FOCUS;
        }

        if self.on_delete.is_some() {
            actions |= Actions::DISMISS | Actions::FOCUS;
        }

        Some(
            Node::new(
                tree.id().clone(),
                layout.bounds(),
                // A chip that can be selected is a two-state control; see
                // this module's own documentation.
                if self.on_press.is_some() {
                    Role::Checkbox
                } else {
                    Role::Button
                },
            )
            .with_state(flags)
            .with_actions(actions),
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

        if shell.is_event_captured() {
            return;
        }

        if !matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | Event::Touch(touch::Event::FingerPressed { .. })
        ) {
            return;
        }

        let bounds = layout.bounds();

        let Some(position) = cursor.position_over(bounds) else {
            return;
        };

        // The delete region wins over the body: it sits inside the chip,
        // so testing the body first would swallow every removal.
        if let Some(delete) = self.delete_bounds(bounds)
            && delete.contains(position)
            && let Some(on_delete) = self.on_delete.clone()
        {
            shell.publish(on_delete);
            shell.capture_event();
            return;
        }

        if let Some(on_press) = self.on_press.clone() {
            shell.publish(on_press);
            shell.capture_event();
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.is_interactive() && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let appearance = theme.style(&self.class, self.selected);
        let bounds = layout.bounds();
        let pill = self.pill_bounds(bounds);

        renderer.fill_quad(
            renderer::Quad {
                bounds: pill,
                border: Border {
                    radius: (PILL_HEIGHT / 2.0).into(),
                    ..appearance.border
                },
                ..renderer::Quad::default()
            },
            appearance.background,
        );

        if let Some(content_layout) = layout.children().next() {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                &renderer::Style {
                    text_color: appearance.text_color,
                },
                content_layout,
                cursor,
                viewport,
            );
        }

        // The removal affordance: two crossed bars, drawn rather than
        // set as text so the chip does not depend on an icon font being
        // present to be usable.
        if let Some(delete) = self.delete_bounds(bounds) {
            let centre = Point::new(
                delete.x + delete.width / 2.0,
                delete.y + delete.height / 2.0,
            );

            for rotated in [false, true] {
                let (width, height) = if rotated {
                    (1.5, DELETE_GLYPH)
                } else {
                    (DELETE_GLYPH, 1.5)
                };

                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: centre.x - width / 2.0,
                            y: centre.y - height / 2.0,
                            width,
                            height,
                        },
                        ..renderer::Quad::default()
                    },
                    Background::Color(appearance.text_color),
                );
            }
        }
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

impl<'a, Message, Theme, Renderer> From<Chip<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(chip: Chip<'a, Message, Theme, Renderer>) -> Self {
        Self::new(chip)
    }
}

/// The appearance of a [`Chip`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The pill's fill.
    pub background: Background,
    /// The colour of the chip's content.
    pub text_color: Color,
    /// The border around the pill.
    pub border: Border,
}

/// The theme catalog of a [`Chip`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class, in the given selection state.
    fn style(&self, class: &Self::Class<'_>, selected: bool) -> Style;
}

/// A styling function for a [`Chip`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, bool) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>, selected: bool) -> Style {
        class(self, selected)
    }
}

/// The default styling of a [`Chip`] for taim's own theme.
pub fn default(theme: &Theme, selected: bool) -> Style {
    let palette = theme.palette();

    if selected {
        Style {
            background: Background::Color(palette.primary.base.color),
            text_color: palette.primary.base.text,
            border: Border::default(),
        }
    } else {
        Style {
            background: Background::Color(palette.background.weak.color),
            text_color: palette.background.base.text,
            // An unselected chip carries an outline so selection is not
            // signalled by fill alone; colour is not an accessible
            // distinction on its own.
            border: Border {
                width: 1.0,
                color: palette.background.strong.color,
                ..Border::default()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chip<'a>() -> Chip<'a, ()> {
        Chip::new(crate::text::Text::new("Unread"))
    }

    const BOUNDS: Rectangle = Rectangle {
        x: 10.0,
        y: 20.0,
        width: 120.0,
        height: TOUCH_HEIGHT,
    };

    // The fixture must itself be at least a touch target, or the centring
    // assertion below proves nothing. Constant, so check it at compile time.
    const _: () = assert!(
        BOUNDS.height >= 48.0,
        "the test fixture must occupy a full touch target"
    );

    #[test]
    fn the_pill_is_centred_inside_a_full_touch_target() {
        let pill = chip().pill_bounds(BOUNDS);

        assert_eq!(pill.height, PILL_HEIGHT);
        assert_eq!(
            pill.y - BOUNDS.y,
            BOUNDS.y + BOUNDS.height - (pill.y + pill.height),
            "equal space above and below the pill"
        );
    }

    #[test]
    fn a_chip_without_a_delete_has_no_delete_region() {
        assert!(chip().delete_bounds(BOUNDS).is_none());
    }

    #[test]
    fn the_delete_region_is_a_full_height_target_at_the_trailing_edge() {
        let chip = chip().on_delete(());
        let delete = chip.delete_bounds(BOUNDS).expect("delete region");

        assert_eq!(delete.height, BOUNDS.height, "a small glyph, a real target");
        assert_eq!(
            delete.x + delete.width,
            BOUNDS.x + BOUNDS.width - chip.padding.right
        );
    }

    #[test]
    fn interactivity_follows_the_handlers_that_were_set() {
        assert!(!chip().is_interactive());
        assert!(chip().on_press(()).is_interactive());
        assert!(chip().on_delete(()).is_interactive());

        // Selection alone is a state to display, not a way to interact.
        assert!(!chip().selected(true).is_interactive());
    }
}
