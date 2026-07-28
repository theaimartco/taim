//! Modal dialogs: a surface that owns the screen until it is dismissed.
//!
//! # Example
//! ```no_run
//! # mod aura { pub mod widget { pub use aura_widget::*; } pub use aura_widget::Renderer; pub use aura_widget::core::*; }
//! # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
//! use aura::widget::{dialog, stack, text};
//!
//! #[derive(Clone)]
//! enum Message {
//!     Dismissed,
//! }
//!
//! # struct State { is_open: bool }
//! fn view(state: &State) -> Element<'_, Message> {
//!     let page = text("the application");
//!
//!     if state.is_open {
//!         stack![
//!             page,
//!             dialog(text("Are you sure?")).on_dismiss(Message::Dismissed),
//!         ]
//!         .into()
//!     } else {
//!         page.into()
//!     }
//! }
//! ```
//!
//! # Why a `stack`, and not an overlay
//!
//! aura renders floating content through `overlay::Overlay`, which has no
//! `semantics` hook: anything drawn that way is **invisible to the
//! accessibility engine**. A modal is the one thing that must never be
//! invisible to a screen reader, so [`Dialog`] is an ordinary widget with
//! the modal as its child, layered over the page with
//! [`stack`](crate::stack).
//!
//! The cost is that a dialog covers whatever it is stacked over, not the
//! whole window — so it belongs at the root of a view. The benefit is that
//! it reports real semantics, takes part in focus traversal, and can be
//! focus-trapped.
//!
//! # Focus trapping is the caller's to complete
//!
//! A modal owns focus for as long as it is open: Tab must not escape into
//! the inert page behind it. That is
//! `aura_accessibility::focus::Scope`'s job, and it lives in application
//! state (focus outlives any single view). This widget reports
//! [`Role::Window`](aura_core::semantics::Role::Window) and offers
//! `DISMISS`, which is what a caller pushes a `Scope` *for*; see
//! [`Dialog::is_modal`] and `docs/accessibility/contract.md`.

use crate::core::border::Border;
use crate::core::keyboard;
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

/// The smallest a dialog may be, matching
/// `aura_design::components::dialog::Dialog`'s own minimum.
const MIN_SIZE: Size = Size {
    width: 280.0,
    height: 120.0,
};

const DEFAULT_PADDING: Padding = Padding {
    top: 24.0,
    right: 24.0,
    bottom: 24.0,
    left: 24.0,
};

/// A modal dialog.
#[allow(missing_debug_implementations)]
pub struct Dialog<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    content: Element<'a, Message, Theme, Renderer>,
    on_dismiss: Option<Message>,
    dismiss_on_scrim: bool,
    padding: Padding,
    max_width: f32,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> Dialog<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Creates a dialog around `content`.
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            content: content.into(),
            on_dismiss: None,
            dismiss_on_scrim: true,
            padding: DEFAULT_PADDING,
            max_width: 560.0,
            class: Theme::default(),
        }
    }

    /// Makes the dialog dismissible, producing `on_dismiss` on Escape or a
    /// scrim tap.
    ///
    /// A dialog without this cannot be dismissed by the user at all --
    /// appropriate only when the content itself provides the only way out
    /// (a required choice), and a deliberate decision either way.
    pub fn on_dismiss(mut self, on_dismiss: Message) -> Self {
        self.on_dismiss = Some(on_dismiss);
        self
    }

    /// Sets whether tapping the scrim dismisses the dialog.
    ///
    /// Escape always does (when [`Self::on_dismiss`] is set); this only
    /// governs the scrim, because a destructive confirmation may
    /// reasonably want a mis-tap outside it to do nothing.
    pub fn dismiss_on_scrim(mut self, dismiss: bool) -> Self {
        self.dismiss_on_scrim = dismiss;
        self
    }

    /// Sets the interior padding.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the widest the dialog surface may grow.
    pub fn max_width(mut self, max_width: impl Into<crate::core::Pixels>) -> Self {
        self.max_width = max_width.into().0.max(MIN_SIZE.width);
        self
    }

    /// Sets the style of the dialog.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Whether this dialog is modal, and therefore needs a focus trap.
    ///
    /// Always `true`: a dialog that did not own focus would let Tab wander
    /// into the page it is covering, which is visually inert but still
    /// reachable. A caller pushes an
    /// `aura_accessibility::focus::Scope { trapped: true }` for as long as
    /// the dialog is in the view.
    pub const fn is_modal(&self) -> bool {
        true
    }

    /// Whether the user can dismiss this dialog.
    pub const fn is_dismissible(&self) -> bool {
        self.on_dismiss.is_some()
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Dialog<'_, Message, Theme, Renderer>
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
        // The scrim fills whatever it is stacked over.
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let scrim = limits.resolve(Length::Fill, Length::Fill, Size::ZERO);

        let surface_limits = layout::Limits::new(
            MIN_SIZE,
            Size::new(
                self.max_width.min(scrim.width),
                // Never taller than the space available: a dialog that
                // overflowed the window would put its own actions off
                // screen, which is how a modal becomes inescapable.
                scrim.height,
            ),
        )
        .shrink(self.padding);

        let content =
            self.content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, &surface_limits);

        let surface = Size::new(
            (content.size().width + self.padding.left + self.padding.right)
                .clamp(MIN_SIZE.width, self.max_width.min(scrim.width)),
            (content.size().height + self.padding.top + self.padding.bottom)
                .clamp(MIN_SIZE.height.min(scrim.height), scrim.height),
        );

        // Centred in the scrim: a modal has no anchor to align to, and
        // centring is what makes it read as "this, now" rather than as
        // part of the page behind it.
        let origin = Point::new(
            (scrim.width - surface.width) / 2.0,
            (scrim.height - surface.height) / 2.0,
        );

        let content = content.move_to(Point::new(
            origin.x + self.padding.left,
            origin.y + self.padding.top,
        ));

        // Two children: the surface rectangle, then the content. The
        // surface node carries no widget -- it exists so `draw` and
        // hit-testing know where the dialog itself is, as distinct from
        // the scrim.
        layout::Node::with_children(
            scrim,
            vec![layout::Node::new(surface).move_to(origin), content],
        )
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State};

        // The node's bounds are the *surface*, not the scrim: a screen
        // reader pointing at the dialog must point at the dialog, not at
        // the whole window.
        let bounds = layout
            .children()
            .next()
            .map(|surface| surface.bounds())
            .unwrap_or_else(|| layout.bounds());

        Some(
            Node::new(tree.id().clone(), bounds, Role::Window)
                .with_state(State::empty())
                .with_actions(if self.is_dismissible() {
                    Actions::DISMISS
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
        if let Some(content_layout) = layout.children().nth(1) {
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
            if let Some(content_layout) = layout.children().nth(1) {
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
        let mut children = layout.children();
        let surface = children.next().map(|node| node.bounds());
        let content_layout = children.next();

        // The content sees events first: a button inside the dialog must
        // win its own click.
        if let Some(content_layout) = content_layout {
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

        let Some(on_dismiss) = self.on_dismiss.clone() else {
            // A non-dismissible modal still swallows everything below it:
            // the page behind is inert, not merely covered.
            self.swallow(event, layout, cursor, shell);
            return;
        };

        match event {
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. })
                if *key == keyboard::Key::Named(keyboard::key::Named::Escape) =>
            {
                shell.publish(on_dismiss);
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. })
                if self.dismiss_on_scrim =>
            {
                let on_scrim = cursor.is_over(layout.bounds())
                    && !surface.is_some_and(|surface| cursor.is_over(surface));

                if on_scrim {
                    shell.publish(on_dismiss);
                    shell.capture_event();
                } else {
                    self.swallow(event, layout, cursor, shell);
                }
            }
            _ => self.swallow(event, layout, cursor, shell),
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
            .nth(1)
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
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let appearance = theme.style(&self.class);
        let mut children = layout.children();

        // The scrim: what makes the page behind read as unavailable.
        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                ..renderer::Quad::default()
            },
            appearance.scrim,
        );

        if let Some(surface) = children.next() {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: surface.bounds(),
                    border: appearance.border,
                    ..renderer::Quad::default()
                },
                appearance.background,
            );
        }

        if let Some(content_layout) = children.next() {
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
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let content_layout = layout.children().nth(1)?;

        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            content_layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<Message, Theme, Renderer> Dialog<'_, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Captures a pointer event that landed anywhere on the dialog,
    /// including the scrim.
    ///
    /// This is what makes the page behind genuinely inert rather than
    /// merely hidden: a click that fell on the scrim must not reach the
    /// button it happens to be covering.
    fn swallow(
        &self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        shell: &mut Shell<'_, Message>,
    ) {
        let is_pointer = matches!(event, Event::Mouse(_) | Event::Touch(_));

        if is_pointer && cursor.is_over(layout.bounds()) {
            shell.capture_event();
        }
    }
}

impl<'a, Message, Theme, Renderer> From<Dialog<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(dialog: Dialog<'a, Message, Theme, Renderer>) -> Self {
        Self::new(dialog)
    }
}

/// The appearance of a [`Dialog`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The scrim covering the page behind.
    pub scrim: Background,
    /// The dialog surface's fill.
    pub background: Background,
    /// The colour the dialog's content inherits.
    pub text_color: Color,
    /// The border around the surface.
    pub border: Border,
}

/// The theme catalog of a [`Dialog`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a [`Dialog`].
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

/// The default styling of a [`Dialog`] for aura's own theme.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        // A scrim is a dimming, not a blur: the page must read as
        // unavailable even with every expensive effect disabled.
        scrim: Background::Color(Color {
            a: 0.4,
            ..Color::BLACK
        }),
        background: Background::Color(palette.background.base.color),
        text_color: palette.background.base.text,
        border: Border {
            radius: 20.0.into(),
            ..Border::default()
        },
    }
}
