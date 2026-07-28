//! The application's top bar: a leading slot, a title, and actions.
//!
//! # Example
//! ```no_run
//! # mod aura { pub mod widget { pub use aura_widget::*; } pub use aura_widget::Renderer; pub use aura_widget::core::*; }
//! # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
//! use aura::widget::{app_bar, icon_button, text};
//!
//! #[derive(Clone)]
//! enum Message {
//!     Back,
//! }
//!
//! fn view() -> Element<'static, Message> {
//!     app_bar(text("Inbox"))
//!         .leading(icon_button('<', "Back").on_press(Message::Back))
//!         .into()
//! }
//! ```
//!
//! # A bar is a container, not a control
//!
//! An [`AppBar`] has no actions of its own and takes no focus. Its
//! leading and action slots hold real controls -- an
//! [`IconButton`](crate::icon_button::IconButton), typically -- which
//! carry their own roles, names, focus, and touch targets.
//!
//! Modelling the bar itself as activatable, as
//! `aura_design::components::app_bar::AppBar` originally did, claims an
//! action with no focusable, touch-sized target behind it. That defect
//! was found and fixed in the contract; this widget is built the right
//! way round from the start.

use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget::Operation;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::visit;
use crate::core::{
    Background, Color, Element, Event, Layout, Length, Padding, Point, Rectangle, Shell, Size,
    Theme, Vector, Widget,
};

/// How much vertical room the bar's title occupies, and whether it
/// collapses as content scrolls beneath it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    /// A single row: leading, title, actions.
    #[default]
    Standard,
    /// A display-size title above the action row.
    Large,
    /// Like [`Kind::Large`], but shrinking to [`Kind::Standard`]'s height
    /// as content scrolls under it.
    Collapsing,
}

impl Kind {
    /// The bar's height at rest.
    pub const fn expanded_height(self) -> f32 {
        match self {
            Self::Standard => 64.0,
            Self::Large | Self::Collapsing => 112.0,
        }
    }

    /// The height this kind shrinks to once content scrolls under it.
    /// Non-collapsing kinds keep their full height.
    pub const fn collapsed_height(self) -> f32 {
        match self {
            Self::Standard => 64.0,
            Self::Large => 112.0,
            Self::Collapsing => 64.0,
        }
    }
}

const DEFAULT_PADDING: Padding = Padding {
    top: 0.0,
    right: 8.0,
    bottom: 0.0,
    left: 8.0,
};

const SLOT_SPACING: f32 = 8.0;

/// The application's top bar.
#[allow(missing_debug_implementations)]
pub struct AppBar<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// `[leading?] title [actions...]` -- contiguous, because
    /// `Tree::diff_children` reconciles a slice.
    slots: Vec<Element<'a, Message, Theme, Renderer>>,
    has_leading: bool,
    action_count: usize,
    kind: Kind,
    collapse: f32,
    padding: Padding,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> AppBar<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Creates a standard bar with the given title.
    pub fn new(title: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            slots: vec![title.into()],
            has_leading: false,
            action_count: 0,
            kind: Kind::Standard,
            collapse: 0.0,
            padding: DEFAULT_PADDING,
            class: Theme::default(),
        }
    }

    /// Sets the bar's kind.
    pub fn kind(mut self, kind: Kind) -> Self {
        self.kind = kind;
        self
    }

    /// Sets the leading slot: a back or menu control.
    pub fn leading(mut self, leading: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        if self.has_leading {
            self.slots[0] = leading.into();
        } else {
            self.slots.insert(0, leading.into());
            self.has_leading = true;
        }
        self
    }

    /// Appends a trailing action.
    pub fn action(mut self, action: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        self.slots.push(action.into());
        self.action_count += 1;
        self
    }

    /// Sets how far a [`Kind::Collapsing`] bar has collapsed, `0.0`
    /// (expanded) to `1.0` (collapsed). Values outside that range clamp.
    ///
    /// A caller drives this from its scrollable's own offset; the bar does
    /// not observe scrolling itself, because it has no way to know which
    /// scrollable it belongs to.
    pub fn collapse(mut self, progress: f32) -> Self {
        self.collapse = progress.clamp(0.0, 1.0);
        self
    }

    /// Sets the horizontal padding.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the style of the bar.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// The bar's current height, interpolated by [`Self::collapse`].
    pub fn height(&self) -> f32 {
        let expanded = self.kind.expanded_height();
        let collapsed = self.kind.collapsed_height();

        expanded + (collapsed - expanded) * self.collapse
    }

    /// The index of the title slot within [`Self::slots`].
    fn title_index(&self) -> usize {
        usize::from(self.has_leading)
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for AppBar<'_, Message, Theme, Renderer>
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

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(&mut self.slots);
    }

    fn size(&self) -> Size<Length> {
        // Width is imposed by the window, not the bar.
        Size::new(Length::Fill, Length::Fixed(self.height()))
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let height = self.height();
        let bounds = limits.resolve(Length::Fill, Length::Fixed(height), Size::ZERO);

        let title = self.title_index();
        let slot_limits = layout::Limits::new(Size::ZERO, Size::new(bounds.width, height));

        // Measure leading and actions first; the title takes what is left,
        // so a long title truncates rather than pushing the actions off
        // the bar.
        let mut measured: Vec<Option<layout::Node>> = (0..self.slots.len()).map(|_| None).collect();

        for (index, ((slot, slot_tree), out)) in self
            .slots
            .iter_mut()
            .zip(tree.children.iter_mut())
            .zip(measured.iter_mut())
            .enumerate()
        {
            if index == title {
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
        let gaps = SLOT_SPACING * (self.slots.len().saturating_sub(1)) as f32;
        let padding = self.padding.left + self.padding.right;
        let available = (bounds.width - padding - fixed - gaps).max(0.0);

        measured[title] = Some(self.slots[title].as_widget_mut().layout(
            &mut tree.children[title],
            renderer,
            &layout::Limits::new(Size::ZERO, Size::new(available, height)),
        ));

        // Actions are packed against the right edge so they stay put as
        // the title's width changes; everything is vertically centred on
        // the action row, which for a Large bar is its bottom 64px.
        let action_row_centre = height - Kind::Standard.expanded_height() / 2.0;
        let first_action = self.slots.len() - self.action_count;

        let mut positioned: Vec<layout::Node> = Vec::with_capacity(self.slots.len());
        let mut x = self.padding.left;
        let mut action_x = bounds.width - self.padding.right;

        // Right-to-left over the actions, so the last-added sits innermost.
        let mut action_offsets: Vec<f32> = Vec::with_capacity(self.action_count);
        for node in measured[first_action..].iter().flatten().rev() {
            action_x -= node.size().width;
            action_offsets.push(action_x);
            action_x -= SLOT_SPACING;
        }
        action_offsets.reverse();

        for (index, node) in measured.into_iter().enumerate() {
            let node = node.expect("every slot was measured");
            let size = node.size();

            let at = if index >= first_action {
                Point::new(
                    action_offsets[index - first_action],
                    action_row_centre - size.height / 2.0,
                )
            } else if index == title && matches!(self.kind, Kind::Large | Kind::Collapsing) {
                // A large title sits on its own row above the actions...
                // and rises to the action row as the bar collapses.
                let expanded_y = height - Kind::Standard.expanded_height() - size.height / 2.0;
                let collapsed_y = action_row_centre - size.height / 2.0;

                Point::new(x, expanded_y + (collapsed_y - expanded_y) * self.collapse)
            } else {
                Point::new(x, action_row_centre - size.height / 2.0)
            };

            if index < first_action {
                x += size.width + SLOT_SPACING;
            }

            positioned.push(node.move_to(at));
        }

        layout::Node::with_children(Size::new(bounds.width, height), positioned)
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State};

        // A container: no actions, no focus. See this module's own
        // documentation for why claiming otherwise would be a defect.
        Some(
            Node::new(tree.id().clone(), layout.bounds(), Role::Group)
                .with_state(State::empty())
                .with_actions(Actions::empty()),
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
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.slots
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
            .unwrap_or_default()
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
        let appearance = theme.style(&self.class);

        if let Some(background) = appearance.background {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    ..renderer::Quad::default()
                },
                background,
            );
        }

        let slot_style = renderer::Style {
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
                &slot_style,
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

impl<'a, Message, Theme, Renderer> From<AppBar<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(bar: AppBar<'a, Message, Theme, Renderer>) -> Self {
        Self::new(bar)
    }
}

/// The appearance of an [`AppBar`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The fill behind the bar.
    pub background: Option<Background>,
    /// The colour the bar's content inherits.
    pub text_color: Color,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            background: None,
            text_color: Color::BLACK,
        }
    }
}

/// The theme catalog of an [`AppBar`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for an [`AppBar`].
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

/// The default styling of an [`AppBar`] for aura's own theme.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: Some(Background::Color(palette.background.base.color)),
        text_color: palette.background.base.text,
    }
}
