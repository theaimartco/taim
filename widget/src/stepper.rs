//! A numeric value with decrement and increment controls.
//!
//! # Example
//! ```no_run
//! # mod aura { pub mod widget { pub use aura_widget::*; } pub use aura_widget::Renderer; pub use aura_widget::core::*; }
//! # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
//! use aura::widget::stepper;
//!
//! #[derive(Clone)]
//! enum Message {
//!     QuantityChanged(i32),
//! }
//!
//! # struct State { quantity: i32 }
//! fn view(state: &State) -> Element<'_, Message> {
//!     stepper(state.quantity)
//!         .range(1..=10)
//!         .on_change(Message::QuantityChanged)
//!         .into()
//! }
//! ```
//!
//! # A stepper is a slider that moves in whole steps
//!
//! It reports [`Role::Slider`](aura_core::semantics::Role::Slider) with
//! `INCREMENT` and `DECREMENT`, which is how platform accessibility APIs
//! model "a value you can nudge either way". The two buttons are the
//! visual form; the value is the control.
//!
//! # A control at its limit is disabled, not silent
//!
//! At the top of its range the increment button stops offering
//! `INCREMENT` and draws as disabled, rather than staying lit and doing
//! nothing when pressed. A control that looks available and is not is
//! the thing users retry.

use crate::core::border::Border;
use crate::core::layout;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::touch;
use crate::core::widget::tree::{self, Tree};
use crate::core::{
    Background, Color, Element, Event, Layout, Length, Point, Rectangle, Shell, Size, Theme,
    Widget,
};

use std::ops::RangeInclusive;

/// The height of the control, and the width of each button, so both
/// clear the touch-target policy.
const BUTTON_SIZE: f32 = 48.0;

/// The width reserved for the value between the two buttons.
const VALUE_WIDTH: f32 = 56.0;

/// The arm length of the drawn plus and minus glyphs.
const GLYPH_ARM: f32 = 14.0;

/// The stroke weight of those glyphs.
const GLYPH_STROKE: f32 = 2.0;

/// Which control a press landed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Decrement,
    Increment,
}

/// A numeric value with decrement and increment controls.
#[allow(missing_debug_implementations)]
pub struct Stepper<'a, Message, Theme = crate::Theme>
where
    Theme: Catalog,
{
    value: i32,
    range: RangeInclusive<i32>,
    step: i32,
    label: Option<String>,
    on_change: Option<Box<dyn Fn(i32) -> Message + 'a>>,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme> Stepper<'a, Message, Theme>
where
    Theme: Catalog,
{
    /// Creates a stepper showing `value`.
    pub fn new(value: i32) -> Self {
        Self {
            value,
            range: i32::MIN..=i32::MAX,
            step: 1,
            label: None,
            on_change: None,
            class: Theme::default(),
        }
    }

    /// Sets the range the value is held within.
    ///
    /// An inverted range (`10..=1`) is normalised rather than rejected:
    /// a stepper is presentation, and a swapped pair from application
    /// state should not take the window down.
    pub fn range(mut self, range: RangeInclusive<i32>) -> Self {
        let (start, end) = (*range.start(), *range.end());

        self.range = if start <= end {
            start..=end
        } else {
            end..=start
        };

        self
    }

    /// Sets how much one press changes the value. Zero or negative
    /// values are coerced to one, since a step that never moves would
    /// make both buttons inert.
    pub fn step(mut self, step: i32) -> Self {
        self.step = step.max(1);
        self
    }

    /// Sets what the value describes, used as the accessible name.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Produces a message with the new value when either button is
    /// pressed.
    pub fn on_change(mut self, on_change: impl Fn(i32) -> Message + 'a) -> Self {
        self.on_change = Some(Box::new(on_change));
        self
    }

    /// Sets the style of the stepper.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// The value clamped into the configured range.
    ///
    /// The caller's value is not assumed to be in range: it is
    /// application state and may predate a narrowed range.
    pub fn clamped(&self) -> i32 {
        self.value.clamp(*self.range.start(), *self.range.end())
    }

    /// Whether the value can still be decremented.
    fn can_decrement(&self) -> bool {
        self.on_change.is_some() && self.clamped() > *self.range.start()
    }

    /// Whether the value can still be incremented.
    fn can_increment(&self) -> bool {
        self.on_change.is_some() && self.clamped() < *self.range.end()
    }

    /// The value one step below the current one, held in range.
    ///
    /// Saturating: a step that would overflow `i32` clamps to the range
    /// bound instead of wrapping to the opposite end.
    fn decremented(&self) -> i32 {
        self.clamped()
            .saturating_sub(self.step)
            .clamp(*self.range.start(), *self.range.end())
    }

    /// The value one step above the current one, held in range.
    fn incremented(&self) -> i32 {
        self.clamped()
            .saturating_add(self.step)
            .clamp(*self.range.start(), *self.range.end())
    }

    /// The bounds of one of the two buttons.
    fn button_bounds(&self, bounds: Rectangle, part: Part) -> Rectangle {
        let x = match part {
            Part::Decrement => bounds.x,
            Part::Increment => bounds.x + bounds.width - BUTTON_SIZE,
        };

        Rectangle {
            x,
            y: bounds.y,
            width: BUTTON_SIZE,
            height: bounds.height,
        }
    }

    /// Which button, if either, a position landed on.
    fn part_at(&self, bounds: Rectangle, position: Point) -> Option<Part> {
        for part in [Part::Decrement, Part::Increment] {
            if self.button_bounds(bounds, part).contains(position) {
                return Some(part);
            }
        }

        None
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Stepper<'_, Message, Theme>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn state(&self) -> tree::State {
        tree::State::None
    }

    fn size(&self) -> Size<Length> {
        Size::new(
            Length::Fixed(BUTTON_SIZE * 2.0 + VALUE_WIDTH),
            Length::Fixed(BUTTON_SIZE),
        )
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let size = Size::new(BUTTON_SIZE * 2.0 + VALUE_WIDTH, BUTTON_SIZE);

        layout::Node::new(limits.resolve(
            Length::Fixed(size.width),
            Length::Fixed(size.height),
            size,
        ))
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        let enabled = self.on_change.is_some();

        let mut actions = if enabled {
            Actions::FOCUS | Actions::SET_VALUE
        } else {
            Actions::empty()
        };

        // Only offered where they can actually do something: a control
        // at the top of its range must not advertise an increment that
        // does nothing.
        if self.can_decrement() {
            actions |= Actions::DECREMENT;
        }

        if self.can_increment() {
            actions |= Actions::INCREMENT;
        }

        let node = Node::new(tree.id().clone(), layout.bounds(), Role::Slider)
            .with_state(if enabled {
                SemanticState::FOCUSABLE
            } else {
                SemanticState::DISABLED
            })
            .with_actions(actions);

        Some(match &self.label {
            Some(label) => node.with_name(label.as_str()),
            None => node,
        })
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let Some(on_change) = &self.on_change else {
            return;
        };

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

        let Some(part) = self.part_at(bounds, position) else {
            return;
        };

        let (allowed, next) = match part {
            Part::Decrement => (self.can_decrement(), self.decremented()),
            Part::Increment => (self.can_increment(), self.incremented()),
        };

        // A press on a disabled button is still swallowed: it landed on
        // this control, and letting it fall through to whatever is
        // underneath would be worse than doing nothing.
        if allowed {
            shell.publish(on_change(next));
        }

        shell.capture_event();
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();

        let Some(position) = cursor.position_over(bounds) else {
            return mouse::Interaction::None;
        };

        match self.part_at(bounds, position) {
            Some(Part::Decrement) if self.can_decrement() => mouse::Interaction::Pointer,
            Some(Part::Increment) if self.can_increment() => mouse::Interaction::Pointer,
            Some(_) => mouse::Interaction::NotAllowed,
            None => mouse::Interaction::None,
        }
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let appearance = theme.style(&self.class);
        let bounds = layout.bounds();

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    radius: (bounds.height / 2.0).into(),
                    ..appearance.border
                },
                ..renderer::Quad::default()
            },
            appearance.background,
        );

        for (part, enabled) in [
            (Part::Decrement, self.can_decrement()),
            (Part::Increment, self.can_increment()),
        ] {
            let button = self.button_bounds(bounds, part);

            let color = if enabled {
                appearance.glyph_color
            } else {
                appearance.disabled_glyph_color
            };

            let centre = Point::new(
                button.x + button.width / 2.0,
                button.y + button.height / 2.0,
            );

            // The horizontal bar of both glyphs.
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: centre.x - GLYPH_ARM / 2.0,
                        y: centre.y - GLYPH_STROKE / 2.0,
                        width: GLYPH_ARM,
                        height: GLYPH_STROKE,
                    },
                    ..renderer::Quad::default()
                },
                Background::Color(color),
            );

            // The vertical bar, which is what makes a plus out of a minus.
            if part == Part::Increment {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: centre.x - GLYPH_STROKE / 2.0,
                            y: centre.y - GLYPH_ARM / 2.0,
                            width: GLYPH_STROKE,
                            height: GLYPH_ARM,
                        },
                        ..renderer::Quad::default()
                    },
                    Background::Color(color),
                );
            }
        }
    }
}

impl<'a, Message, Theme, Renderer> From<Stepper<'a, Message, Theme>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(stepper: Stepper<'a, Message, Theme>) -> Self {
        Self::new(stepper)
    }
}

/// The appearance of a [`Stepper`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The control's fill.
    pub background: Background,
    /// The colour of an available button's glyph.
    pub glyph_color: Color,
    /// The colour of a glyph whose button is at its limit.
    pub disabled_glyph_color: Color,
    /// The colour of the value between the buttons.
    pub text_color: Color,
    /// The border around the control.
    pub border: Border,
}

/// The theme catalog of a [`Stepper`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a [`Stepper`].
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

/// The default styling of a [`Stepper`] for aura's own theme.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: Background::Color(palette.background.weak.color),
        glyph_color: palette.background.base.text,
        disabled_glyph_color: Color {
            a: 0.3,
            ..palette.background.base.text
        },
        text_color: palette.background.base.text,
        border: Border::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control<'a>(value: i32) -> Stepper<'a, ()> {
        Stepper::new(value).on_change(|_| ())
    }

    const BOUNDS: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: BUTTON_SIZE * 2.0 + VALUE_WIDTH,
        height: BUTTON_SIZE,
    };

    #[test]
    fn stepping_moves_by_the_configured_step() {
        let stepper = control(10).step(5);

        assert_eq!(stepper.incremented(), 15);
        assert_eq!(stepper.decremented(), 5);
    }

    #[test]
    fn a_zero_step_is_coerced_so_the_buttons_still_move() {
        assert_eq!(control(10).step(0).incremented(), 11);
        assert_eq!(control(10).step(-4).incremented(), 11);
    }

    #[test]
    fn the_value_is_held_inside_its_range() {
        let stepper = control(5).range(1..=10);

        assert_eq!(stepper.clamped(), 5);
        assert_eq!(control(99).range(1..=10).clamped(), 10);
        assert_eq!(control(-99).range(1..=10).clamped(), 1);
    }

    #[test]
    fn an_inverted_range_is_normalised_rather_than_rejected() {
        let stepper = control(5).range(10..=1);

        assert_eq!(*stepper.range.start(), 1);
        assert_eq!(*stepper.range.end(), 10);
    }

    #[test]
    fn a_control_at_its_limit_cannot_step_further() {
        let top = control(10).range(1..=10);
        assert!(!top.can_increment());
        assert!(top.can_decrement());
        assert_eq!(top.incremented(), 10);

        let bottom = control(1).range(1..=10);
        assert!(bottom.can_increment());
        assert!(!bottom.can_decrement());
        assert_eq!(bottom.decremented(), 1);
    }

    #[test]
    fn a_step_that_would_overshoot_lands_on_the_bound() {
        let stepper = control(8).range(1..=10).step(5);

        assert_eq!(stepper.incremented(), 10);
    }

    #[test]
    fn a_step_that_would_overflow_does_not_wrap() {
        // Saturating rather than wrapping: an increment must never come
        // back as a decrement of the opposite sign.
        let stepper = control(i32::MAX).step(10);

        assert_eq!(stepper.incremented(), i32::MAX);

        let stepper = control(i32::MIN).step(10);

        assert_eq!(stepper.decremented(), i32::MIN);
    }

    #[test]
    fn a_stepper_without_a_handler_is_inert() {
        let stepper: Stepper<'_, ()> = Stepper::new(5).range(1..=10);

        assert!(!stepper.can_increment());
        assert!(!stepper.can_decrement());
    }

    #[test]
    fn each_button_owns_a_full_touch_target_at_its_own_end() {
        let stepper = control(5);

        let decrement = stepper.button_bounds(BOUNDS, Part::Decrement);
        let increment = stepper.button_bounds(BOUNDS, Part::Increment);

        assert!(decrement.width >= 48.0 && decrement.height >= 48.0);
        assert!(increment.width >= 48.0 && increment.height >= 48.0);
        assert_eq!(decrement.x, BOUNDS.x);
        assert_eq!(increment.x + increment.width, BOUNDS.x + BOUNDS.width);
        assert!(
            decrement.x + decrement.width <= increment.x,
            "the buttons must not overlap the value between them"
        );
    }

    #[test]
    fn a_press_between_the_buttons_hits_neither() {
        let stepper = control(5);
        let centre = Point::new(BOUNDS.width / 2.0, BOUNDS.height / 2.0);

        assert_eq!(stepper.part_at(BOUNDS, centre), None);
    }

    #[test]
    fn a_press_on_a_button_identifies_it() {
        let stepper = control(5);

        assert_eq!(
            stepper.part_at(BOUNDS, Point::new(10.0, 24.0)),
            Some(Part::Decrement)
        );
        assert_eq!(
            stepper.part_at(BOUNDS, Point::new(BOUNDS.width - 10.0, 24.0)),
            Some(Part::Increment)
        );
    }
}
