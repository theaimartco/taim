//! Rows in a list: a body of text with optional leading and trailing
//! slots.
//!
//! # Example
//! ```no_run
//! # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::Renderer; pub use taim_widget::core::*; }
//! # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
//! use taim::widget::{list_tile, text};
//!
//! #[derive(Clone)]
//! enum Message {
//!     Opened,
//! }
//!
//! fn view() -> Element<'static, Message> {
//!     list_tile(text("Inbox"))
//!         .on_press(Message::Opened)
//!         .into()
//! }
//! ```
//!
//! # Height comes from line count, not from content
//!
//! A tile's height is a function of how many lines of text it *reserves*
//! ([`Lines`]), not of what it happens to contain. That is what makes a
//! list scannable: rows share a rhythm, and one long label does not shove
//! its neighbours out of alignment. It is also what lets a virtualized
//! list compute positions without measuring every row.
//!
//! This realises `taim_design::components::list_tile::ListTile`, whose
//! `ListLines::height` owns the same numbers.

use crate::core::border::Border;
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget::Operation;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::visit;
use crate::core::{
    Background, Color, Element, Event, Layout, Length, Padding, Point, Rectangle, Shell, Size,
    Theme, Vector, Widget, touch,
};

/// How many lines of text a tile reserves room for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lines {
    /// A single line: a plain label.
    #[default]
    One,
    /// Two lines: a label and a supporting line.
    Two,
    /// Three lines: a label and two supporting lines.
    Three,
}

impl Lines {
    /// The tile height, in logical pixels, this line count needs.
    ///
    /// These are the same numbers `taim_design`'s `ListLines::height`
    /// declares; each step adds one line's worth rather than an arbitrary
    /// value.
    pub const fn height(self) -> f32 {
        match self {
            Self::One => 48.0,
            Self::Two => 64.0,
            Self::Three => 80.0,
        }
    }
}

const DEFAULT_PADDING: Padding = Padding {
    top: 0.0,
    right: 16.0,
    bottom: 0.0,
    left: 16.0,
};

const SLOT_SPACING: f32 = 16.0;

/// A row in a list.
#[allow(missing_debug_implementations)]
pub struct ListTile<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// `[leading?] content [trailing?]` -- contiguous, because
    /// `Tree::diff_children` reconciles a slice, not a scattering of
    /// `Option`s.
    slots: Vec<Element<'a, Message, Theme, Renderer>>,
    has_leading: bool,
    has_trailing: bool,
    lines: Lines,
    on_press: Option<Message>,
    selected: bool,
    padding: Padding,
    class: Theme::Class<'a>,
    last_status: Option<Status>,
}

impl<'a, Message, Theme, Renderer> ListTile<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Creates a one-line tile with the given body.
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            slots: vec![content.into()],
            has_leading: false,
            has_trailing: false,
            lines: Lines::One,
            on_press: None,
            selected: false,
            padding: DEFAULT_PADDING,
            class: Theme::default(),
            last_status: None,
        }
    }

    /// Sets how many lines of text this tile reserves room for.
    pub fn lines(mut self, lines: Lines) -> Self {
        self.lines = lines;
        self
    }

    /// Sets the leading slot (an avatar, an icon, a checkbox).
    pub fn leading(mut self, leading: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        if self.has_leading {
            self.slots[0] = leading.into();
        } else {
            self.slots.insert(0, leading.into());
            self.has_leading = true;
        }
        self
    }

    /// Sets the trailing slot (a chevron, a switch, a timestamp).
    pub fn trailing(mut self, trailing: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        if self.has_trailing {
            let last = self.slots.len() - 1;
            self.slots[last] = trailing.into();
        } else {
            self.slots.push(trailing.into());
            self.has_trailing = true;
        }
        self
    }

    /// Makes the tile activatable, producing `on_press` when tapped.
    ///
    /// A tile without this is presentation: it reports as static text and
    /// takes no focus.
    pub fn on_press(mut self, on_press: Message) -> Self {
        self.on_press = Some(on_press);
        self
    }

    /// Marks the tile as the selected member of its list.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Sets the horizontal padding.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the style of the tile.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status, bool) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Whether this tile is interactive.
    pub fn is_interactive(&self) -> bool {
        self.on_press.is_some()
    }

    /// The index of the content slot within [`Self::slots`].
    fn content_index(&self) -> usize {
        usize::from(self.has_leading)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct State {
    is_pressed: bool,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for ListTile<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(&mut self.slots);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(self.lines.height()))
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let height = self.lines.height();
        let bounds = limits.resolve(Length::Fill, Length::Fixed(height), Size::ZERO);

        let padding = self.padding.left + self.padding.right;
        let content = self.content_index();
        let slot_limits = layout::Limits::new(Size::ZERO, Size::new(bounds.width, height));

        // Measure the fixed slots first: the content takes whatever is
        // left, so a long label shrinks rather than shoving a trailing
        // control off the row.
        let mut measured: Vec<Option<layout::Node>> = (0..self.slots.len()).map(|_| None).collect();

        for (index, ((slot, slot_tree), out)) in self
            .slots
            .iter_mut()
            .zip(tree.children.iter_mut())
            .zip(measured.iter_mut())
            .enumerate()
        {
            if index == content {
                continue;
            }

            *out = Some(
                slot.as_widget_mut()
                    .layout(slot_tree, renderer, &slot_limits),
            );
        }

        let fixed: f32 = measured
            .iter()
            .flatten()
            .map(|node| node.size().width)
            .sum();

        let gaps = SLOT_SPACING * (self.slots.len() - 1) as f32;
        let available = (bounds.width - padding - fixed - gaps).max(0.0);

        measured[content] = Some(self.slots[content].as_widget_mut().layout(
            &mut tree.children[content],
            renderer,
            &layout::Limits::new(Size::ZERO, Size::new(available, height)),
        ));

        // Position every slot, vertically centred: a row's contents align
        // to its optical centre regardless of their own heights.
        let mut positioned: Vec<layout::Node> = Vec::with_capacity(self.slots.len());
        let mut x = self.padding.left;

        for (index, node) in measured.into_iter().enumerate() {
            let node = node.expect("every slot was measured");
            let size = node.size();

            let at = if self.has_trailing && index == self.slots.len() - 1 {
                // The trailing slot is anchored to the right edge, not
                // packed after the content: that is what keeps a column of
                // chevrons aligned down a list.
                Point::new(
                    bounds.width - self.padding.right - size.width,
                    (height - size.height) / 2.0,
                )
            } else {
                Point::new(x, (height - size.height) / 2.0)
            };

            x += size.width + SLOT_SPACING;
            positioned.push(node.move_to(at));
        }

        layout::Node::with_children(Size::new(bounds.width, height), positioned)
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        let interactive = self.is_interactive();

        let mut flags = SemanticState::empty();
        if interactive {
            flags |= SemanticState::FOCUSABLE;
        }
        if self.selected {
            flags |= SemanticState::SELECTED;
        }

        Some(
            Node::new(
                tree.id().clone(),
                layout.bounds(),
                if interactive {
                    Role::ListItem
                } else {
                    // A tile nobody can act on is presentation, not a list
                    // item: claiming otherwise would put it in a screen
                    // reader's list navigation for no reason.
                    Role::StaticText
                },
            )
            .with_state(flags)
            .with_actions(if interactive {
                Actions::ACTIVATE | Actions::FOCUS
            } else {
                Actions::empty()
            }),
        )
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        for ((slot, slot_tree), slot_layout) in self
            .slots
            .iter_mut()
            .zip(tree.children.iter_mut())
            .zip(layout.children())
        {
            visit(&mut visit::Widgets::new(
                slot.as_widget_mut(),
                slot_tree,
                slot_layout,
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
            for ((slot, slot_tree), slot_layout) in self
                .slots
                .iter_mut()
                .zip(tree.children.iter_mut())
                .zip(layout.children())
            {
                slot.as_widget_mut()
                    .operate(slot_tree, slot_layout, renderer, operation);
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
        // Children first: a switch in the trailing slot must win its own
        // tap rather than the row swallowing it.
        for ((slot, slot_tree), slot_layout) in self
            .slots
            .iter_mut()
            .zip(tree.children.iter_mut())
            .zip(layout.children())
        {
            slot.as_widget_mut().update(
                slot_tree,
                event,
                slot_layout,
                cursor,
                renderer,
                shell,
                viewport,
            );
        }

        if shell.is_event_captured() || self.on_press.is_none() {
            return;
        }

        let state = tree.state.downcast_mut::<State>();
        let is_over = cursor.is_over(layout.bounds());

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. })
                if is_over =>
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

                if is_over && let Some(message) = self.on_press.clone() {
                    shell.publish(message);
                    shell.capture_event();
                }
            }
            Event::Touch(touch::Event::FingerLost { .. }) => {
                state.is_pressed = false;
            }
            _ => {}
        }

        let status = if !self.is_interactive() {
            Status::Inert
        } else if state.is_pressed {
            Status::Pressed
        } else if is_over {
            Status::Hovered
        } else {
            Status::Active
        };

        if self.last_status.is_some_and(|last| last != status) {
            shell.request_redraw();
        }

        self.last_status = Some(status);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let children = self
            .slots
            .iter()
            .zip(tree.children.iter())
            .zip(layout.children())
            .map(|((slot, slot_tree), slot_layout)| {
                slot.as_widget().mouse_interaction(
                    slot_tree,
                    slot_layout,
                    cursor,
                    viewport,
                    renderer,
                )
            })
            .max()
            .unwrap_or_default();

        if children != mouse::Interaction::None {
            return children;
        }

        if self.is_interactive() && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
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
        let bounds = layout.bounds();
        let status = self.last_status.unwrap_or(if self.is_interactive() {
            Status::Active
        } else {
            Status::Inert
        });

        let appearance = theme.style(&self.class, status, self.selected);

        if let Some(background) = appearance.background {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: appearance.border,
                    ..renderer::Quad::default()
                },
                background,
            );
        }

        // The tile owns its slots' text colour: a selected row tints its
        // whole content, so the inherited style is deliberately replaced
        // rather than extended.
        let text_style = renderer::Style {
            text_color: appearance.text_color,
        };

        for ((slot, slot_tree), slot_layout) in self
            .slots
            .iter()
            .zip(tree.children.iter())
            .zip(layout.children())
        {
            slot.as_widget().draw(
                slot_tree,
                renderer,
                theme,
                &text_style,
                slot_layout,
                cursor,
                viewport,
            );
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
        overlay::from_children(
            &mut self.slots,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<ListTile<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(tile: ListTile<'a, Message, Theme, Renderer>) -> Self {
        Self::new(tile)
    }
}

/// The possible status of a [`ListTile`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The tile can be activated.
    Active,
    /// The tile is being hovered.
    Hovered,
    /// The tile is being pressed.
    Pressed,
    /// The tile is presentation only.
    Inert,
}

/// The appearance of a [`ListTile`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The fill behind the row, if any.
    pub background: Option<Background>,
    /// The colour text in the row inherits.
    pub text_color: Color,
    /// The border around the row.
    pub border: Border,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            background: None,
            text_color: Color::BLACK,
            border: Border::default(),
        }
    }
}

/// The theme catalog of a [`ListTile`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class with the given status and selection.
    fn style(&self, class: &Self::Class<'_>, status: Status, selected: bool) -> Style;
}

/// A styling function for a [`ListTile`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, Status, bool) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>, status: Status, selected: bool) -> Style {
        class(self, status, selected)
    }
}

/// The default styling of a [`ListTile`] for taim's own theme.
pub fn default(theme: &Theme, status: Status, selected: bool) -> Style {
    let palette = theme.palette();

    let background = if selected {
        Some(Background::Color(palette.primary.weak.color))
    } else {
        match status {
            Status::Hovered => Some(Background::Color(palette.background.weak.color)),
            Status::Pressed => Some(Background::Color(palette.background.strong.color)),
            Status::Active | Status::Inert => None,
        }
    };

    Style {
        background,
        text_color: if selected {
            palette.primary.weak.text
        } else {
            palette.background.base.text
        },
        // Tiles sit flush inside a list: rounding them would break the
        // continuous edge a scannable list depends on.
        border: Border::default(),
    }
}
