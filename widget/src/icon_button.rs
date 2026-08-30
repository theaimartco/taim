//! Icon-only buttons: a control whose entire label is a glyph.
//!
//! # Example
//! ```no_run
//! # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::Renderer; pub use taim_widget::core::*; }
//! # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
//! use taim::widget::icon_button;
//!
//! #[derive(Clone)]
//! enum Message {
//!     Closed,
//! }
//!
//! fn view() -> Element<'static, Message> {
//!     icon_button('\u{f62a}', "Close").on_press(Message::Closed).into()
//! }
//! ```
//!
//! # Why the label is not optional
//!
//! An [`IconButton`] carries no visible text, so the `label` it is
//! constructed with is the **only** name assistive technology can
//! announce and the only text a tooltip can show. It is therefore a
//! required constructor argument rather than an optional builder method:
//! a control that is impossible to build without a name cannot ship
//! without one.
//!
//! This is `taim_design::components::icon_button::IconButton`'s contract
//! made real -- that type's `has_accessible_name` states the rule; this
//! widget is where it cannot be broken.

use crate::core::border::Border;
use crate::core::layout;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::text;
use crate::core::widget::tree::{self, Tree};
use crate::core::{
    Color, Element, Event, Layout, Length, Pixels, Rectangle, Shell, Size, Theme, Widget,
    alignment, touch,
};

/// The default edge length of an icon button, which is also the smallest
/// touch target Taim allows.
///
/// Taken from [`touch::MIN_TARGET`] rather than restated, so this widget
/// cannot drift from `taim-design`'s `MIN_TOUCH_TARGET`.
const DEFAULT_SIZE: f32 = touch::MIN_TARGET;

/// A button whose content is a single icon glyph.
#[allow(missing_debug_implementations)]
pub struct IconButton<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: text::Renderer,
    Theme: Catalog,
{
    glyph: char,
    /// The accessible name. Never rendered as text; announced, and shown
    /// by a tooltip.
    label: String,
    on_press: Option<Message>,
    size: f32,
    font: Option<Renderer::Font>,
    class: Theme::Class<'a>,
    last_status: Option<Status>,
}

impl<'a, Message, Theme, Renderer> IconButton<'a, Message, Theme, Renderer>
where
    Renderer: text::Renderer,
    Theme: Catalog,
{
    /// Creates an [`IconButton`] showing `glyph`, named `label`.
    ///
    /// `label` is required: see this module's own documentation.
    pub fn new(glyph: char, label: impl Into<String>) -> Self {
        Self {
            glyph,
            label: label.into(),
            on_press: None,
            size: DEFAULT_SIZE,
            font: None,
            class: Theme::default(),
            last_status: None,
        }
    }

    /// Sets the message produced when the button is pressed.
    ///
    /// Unless this is called, the button is disabled.
    pub fn on_press(mut self, on_press: Message) -> Self {
        self.on_press = Some(on_press);
        self
    }

    /// Sets the message produced when pressed, if `Some`; disables the
    /// button if `None`.
    pub fn on_press_maybe(mut self, on_press: Option<Message>) -> Self {
        self.on_press = on_press;
        self
    }

    /// Sets the edge length of the button.
    ///
    /// Values below the platform minimum touch target are clamped up: an
    /// icon button is frequently the only control in a dense bar, and a
    /// target nobody can reliably hit is worse than a large one.
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into().0.max(DEFAULT_SIZE);
        self
    }

    /// Sets the font the glyph is drawn from.
    pub fn font(mut self, font: impl Into<Renderer::Font>) -> Self {
        self.font = Some(font.into());
        self
    }

    /// Returns the accessible name.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Sets the style of the button.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct State {
    is_pressed: bool,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for IconButton<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: text::Renderer,
    Theme: Catalog,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.size), Length::Fixed(self.size))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, Length::Fixed(self.size), Length::Fixed(self.size))
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        let enabled = self.on_press.is_some();

        Some(
            Node::new(tree.id().clone(), layout.bounds(), Role::Button)
                .with_state(if enabled {
                    SemanticState::FOCUSABLE
                } else {
                    SemanticState::DISABLED
                })
                .with_actions(if enabled {
                    Actions::ACTIVATE | Actions::FOCUS
                } else {
                    Actions::empty()
                })
                // The whole point of the type: an icon-only control always
                // has a name to announce.
                .with_name(self.label.as_str()),
        )
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let is_over = cursor.is_over(layout.bounds());

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. })
                if is_over && self.on_press.is_some() =>
            {
                state.is_pressed = true;
                shell.capture_event();
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. })
                if state.is_pressed =>
            {
                state.is_pressed = false;
                shell.request_redraw();

                // Only a release *over* the control activates it:
                // dragging off a button is how a user cancels a press they
                // thought better of.
                if is_over && let Some(message) = self.on_press.clone() {
                    shell.publish(message);
                    shell.capture_event();
                }
            }
            Event::Touch(touch::Event::FingerLost { .. }) => {
                // A cancelled contact must not activate anything.
                state.is_pressed = false;
            }
            _ => {}
        }

        let status = self.status(*state, is_over);

        if self.last_status.is_some_and(|last| last != status) {
            shell.request_redraw();
        }

        self.last_status = Some(status);
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) && self.on_press.is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
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
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let status = self.last_status.unwrap_or(Status::Active);
        let style = theme.style(&self.class, status);

        if let Some(background) = style.background {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: style.border,
                    ..renderer::Quad::default()
                },
                background,
            );
        }

        renderer.fill_text(
            text::Text {
                content: self.glyph.to_string(),
                font: self.font.unwrap_or_else(|| renderer.default_font()),
                size: Pixels(bounds.height * 0.5),
                line_height: text::LineHeight::default(),
                bounds: bounds.size(),
                align_x: text::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text::Shaping::Basic,
                wrapping: text::Wrapping::default(),
                ellipsis: text::Ellipsis::default(),
                hint_factor: None,
            },
            bounds.center(),
            style.icon,
            *viewport,
        );
    }
}

impl<Message, Theme, Renderer> IconButton<'_, Message, Theme, Renderer>
where
    Renderer: text::Renderer,
    Theme: Catalog,
{
    fn status(&self, state: State, is_over: bool) -> Status {
        if self.on_press.is_none() {
            Status::Disabled
        } else if state.is_pressed {
            Status::Pressed
        } else if is_over {
            Status::Hovered
        } else {
            Status::Active
        }
    }
}

impl<'a, Message, Theme, Renderer> From<IconButton<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a + Catalog,
    Renderer: 'a + text::Renderer,
{
    fn from(button: IconButton<'a, Message, Theme, Renderer>) -> Self {
        Self::new(button)
    }
}

/// The possible status of an [`IconButton`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The button can be pressed.
    Active,
    /// The button is being hovered.
    Hovered,
    /// The button is being pressed.
    Pressed,
    /// The button cannot be pressed.
    Disabled,
}

/// The appearance of an [`IconButton`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The fill behind the glyph, if any.
    pub background: Option<crate::core::Background>,
    /// The glyph colour.
    pub icon: Color,
    /// The border around the button.
    pub border: Border,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            background: None,
            icon: Color::BLACK,
            border: Border::default(),
        }
    }
}

/// The theme catalog of an [`IconButton`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class with the given status.
    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style;
}

/// A styling function for an [`IconButton`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, Status) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style {
        class(self, status)
    }
}

/// The default styling of an [`IconButton`] for taim's own theme.
pub fn default(theme: &Theme, status: Status) -> Style {
    let palette = theme.palette();
    let text = palette.background.base.text;

    Style {
        // At rest an icon button is content, not chrome: it draws no fill
        // at all, so a row of them reads as icons rather than a wall of
        // boxes. Hover and press are the only times it takes a surface.
        background: match status {
            Status::Hovered => Some(crate::core::Background::Color(
                palette.background.weak.color,
            )),
            Status::Pressed => Some(crate::core::Background::Color(
                palette.background.strong.color,
            )),
            Status::Active | Status::Disabled => None,
        },
        icon: match status {
            Status::Disabled => Color {
                a: text.a * 0.5,
                ..text
            },
            _ => text,
        },
        border: Border {
            // A pill, resolved from the control's own height.
            radius: (DEFAULT_SIZE / 2.0).into(),
            ..Border::default()
        },
    }
}
