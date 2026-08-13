//! A transient message anchored to the bottom of the screen.
//!
//! # Example
//! ```no_run
//! # mod aura { pub mod widget { pub use aura_widget::*; } pub use aura_widget::Renderer; pub use aura_widget::core::*; }
//! # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
//! use aura::widget::{button, snackbar, stack, text};
//!
//! #[derive(Clone)]
//! enum Message {
//!     Undo,
//!     Dismissed,
//! }
//!
//! # struct State { notice: Option<String> }
//! fn view(state: &State) -> Element<'_, Message> {
//!     stack![
//!         text("the application"),
//!         snackbar(
//!             text(state.notice.as_deref().unwrap_or_default()),
//!             state.notice.is_some(),
//!         )
//!         .action(button(text("Undo")).on_press(Message::Undo))
//!         .on_dismiss(Message::Dismissed),
//!     ]
//!     .into()
//! }
//! ```
//!
//! # It announces itself
//!
//! A snackbar reports [`Live::Polite`](aura_core::semantics::Live::Polite),
//! so a screen reader speaks it when it appears without interrupting
//! whatever the user was already hearing. A transient message that is
//! only visible is not a message a blind user ever receives, and a
//! snackbar is often the only confirmation an action succeeded.
//!
//! # The timeout produces a message, it does not hide the widget
//!
//! When the timeout elapses the widget publishes
//! [`Snackbar::on_dismiss`]; it does not hide itself. Visibility stays
//! application state, like every other controlled widget here, which is
//! what lets a caller queue several notices and decide what replaces
//! what. Without an `on_dismiss` the timeout is inert, because there
//! would be nobody to tell.

use crate::core::animation::Animation;
use crate::core::border::Border;
use crate::core::keyboard;
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::time::{Duration, Instant};
use crate::core::widget::Operation;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::visit;
use crate::core::{
    Background, Color, Element, Event, Layout, Length, Padding, Point, Rectangle, Shadow, Shell,
    Size, Theme, Vector, Widget,
};

/// How long a snackbar stays up before its timeout fires.
///
/// Four seconds is the usual platform figure: long enough to read a
/// short sentence, short enough not to sit over the content.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(4);

/// The gap between the snackbar and the edges it floats above.
const DEFAULT_MARGIN: f32 = 16.0;

/// The widest a snackbar grows before it stops tracking the window.
const MAX_WIDTH: f32 = 560.0;

/// The smallest a snackbar may be, matching the 48px touch-target floor
/// its action has to satisfy.
const MIN_HEIGHT: f32 = 48.0;

/// How far the snackbar rises as it appears.
const SLIDE_DISTANCE: f32 = 24.0;

/// The widget state of a [`Snackbar`].
#[derive(Debug)]
pub struct State {
    /// Drives the slide and fade between hidden and shown.
    animation: Animation<bool>,
    /// When the currently shown message appeared, used to time the
    /// dismissal. `None` while hidden.
    shown_at: Option<Instant>,
    /// The most recent frame time.
    now: Instant,
}

impl Default for State {
    fn default() -> Self {
        Self {
            animation: Animation::new(false).quick(),
            shown_at: None,
            now: Instant::now(),
        }
    }
}

/// A transient message anchored to the bottom of the screen.
#[allow(missing_debug_implementations)]
pub struct Snackbar<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    content: Element<'a, Message, Theme, Renderer>,
    action: Option<Element<'a, Message, Theme, Renderer>>,
    is_visible: bool,
    timeout: Option<Duration>,
    padding: Padding,
    margin: f32,
    on_dismiss: Option<Message>,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> Snackbar<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Creates a snackbar showing `content`, visible or not as given.
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        is_visible: bool,
    ) -> Self {
        Self {
            content: content.into(),
            action: None,
            is_visible,
            timeout: Some(DEFAULT_TIMEOUT),
            padding: Padding::new(16.0),
            margin: DEFAULT_MARGIN,
            on_dismiss: None,
            class: Theme::default(),
        }
    }

    /// Sets the action control shown at the trailing edge.
    ///
    /// The action is an ordinary element -- usually a
    /// [`button`](crate::button) -- so it carries its own role, focus,
    /// and touch target rather than the snackbar claiming an action it
    /// has no focusable target for.
    pub fn action(mut self, action: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        self.action = Some(action.into());
        self
    }

    /// Sets how long the snackbar stays up before [`Snackbar::on_dismiss`]
    /// fires.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Removes the timeout, leaving the snackbar up until the caller
    /// takes it down.
    ///
    /// Appropriate when the snackbar carries the only route out of a
    /// state -- an action the user must acknowledge -- and inappropriate
    /// otherwise, since a message that never leaves stops being
    /// transient and starts being furniture.
    pub fn without_timeout(mut self) -> Self {
        self.timeout = None;
        self
    }

    /// Sets the interior padding.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the gap between the snackbar and the edges it floats above.
    pub fn margin(mut self, margin: impl Into<crate::core::Pixels>) -> Self {
        self.margin = margin.into().0.max(0.0);
        self
    }

    /// Produces `on_dismiss` when the timeout elapses or Escape is
    /// pressed.
    pub fn on_dismiss(mut self, on_dismiss: Message) -> Self {
        self.on_dismiss = Some(on_dismiss);
        self
    }

    /// Sets the style of the snackbar.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Where the snackbar sits, given how far it has appeared.
    fn surface_bounds(&self, available: Size, progress: f32) -> Rectangle {
        let width = (available.width - 2.0 * self.margin)
            .min(MAX_WIDTH)
            .max(0.0);

        let height = MIN_HEIGHT.max(self.padding.top + self.padding.bottom);

        // Rises into place as it fades in; the movement is what makes it
        // read as arriving rather than as having always been there.
        let hidden = (1.0 - progress) * SLIDE_DISTANCE;

        Rectangle {
            x: (available.width - width) / 2.0,
            y: available.height - self.margin - height + hidden,
            width,
            height,
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Snackbar<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State {
            animation: Animation::new(self.is_visible).quick(),
            shown_at: None,
            ..State::default()
        })
    }

    fn diff(&mut self, tree: &mut Tree) {
        // Two child slots, and the action's presence can change between
        // views, so the tree is rebuilt from whatever is here now.
        let mut children: Vec<&mut Element<'_, Message, Theme, Renderer>> = vec![&mut self.content];

        if let Some(action) = &mut self.action {
            children.push(action);
        }

        tree.diff_children_custom(
            &mut children,
            |child_tree, child| child.as_widget_mut().diff(child_tree),
            |child| Tree::new(child.as_widget()),
        );
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let available = limits.resolve(Length::Fill, Length::Fill, Size::ZERO);

        let state = tree.state.downcast_ref::<State>();
        let progress = state.animation.interpolate(0.0, 1.0, state.now);
        let surface = self.surface_bounds(available, progress);

        let inner_height = (surface.height - self.padding.top - self.padding.bottom).max(0.0);

        // The action keeps its intrinsic width and the message takes the
        // rest: a long message must not squeeze the control that acts on
        // it below its touch target.
        let action = self.action.as_mut().map(|action| {
            action.as_widget_mut().layout(
                &mut tree.children[1],
                renderer,
                &layout::Limits::new(
                    Size::ZERO,
                    Size::new(surface.width / 2.0, inner_height),
                ),
            )
        });

        let action_width = action.as_ref().map_or(0.0, |node| node.size().width);

        let content_width =
            (surface.width - self.padding.left - self.padding.right - action_width).max(0.0);

        let content = self
            .content
            .as_widget_mut()
            .layout(
                &mut tree.children[0],
                renderer,
                &layout::Limits::new(Size::ZERO, Size::new(content_width, inner_height)),
            )
            .move_to(Point::new(
                surface.x + self.padding.left,
                surface.y + self.padding.top,
            ));

        let mut children = vec![
            layout::Node::new(Size::new(surface.width, surface.height))
                .move_to(Point::new(surface.x, surface.y)),
            content,
        ];

        if let Some(action) = action {
            let size = action.size();

            children.push(action.move_to(Point::new(
                surface.x + surface.width - self.padding.right - size.width,
                surface.y + (surface.height - size.height) / 2.0,
            )));
        }

        layout::Node::with_children(available, children)
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Live, Node, Role, State as SemanticState};

        let state = tree.state.downcast_ref::<State>();

        // A hidden snackbar is absent, not merely transparent: leaving it
        // in the tree would have a screen reader announce a message that
        // is not on screen.
        if state.animation.interpolate(0.0, 1.0, state.now) <= 0.0 {
            return None;
        }

        let bounds = layout
            .children()
            .next()
            .map(|surface| surface.bounds())
            .unwrap_or_else(|| layout.bounds());

        Some(
            Node::new(tree.id().clone(), bounds, Role::Group)
                .with_state(SemanticState::empty())
                .with_actions(if self.on_dismiss.is_some() {
                    Actions::DISMISS
                } else {
                    Actions::empty()
                })
                // Polite, not assertive: a snackbar confirms something
                // that already happened. Interrupting speech for it would
                // make routine confirmations feel like errors.
                .with_live(Live::Polite),
        )
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        let mut children = layout.children();
        let _surface = children.next();

        if let Some(content_layout) = children.next() {
            visit(&mut visit::Widgets::new(
                self.content.as_widget_mut(),
                &mut tree.children[0],
                content_layout,
            ));
        }

        if let (Some(action), Some(action_layout)) = (&mut self.action, children.next()) {
            visit(&mut visit::Widgets::new(
                action.as_widget_mut(),
                &mut tree.children[1],
                action_layout,
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
            let mut children = layout.children();
            let _surface = children.next();

            if let Some(content_layout) = children.next() {
                self.content.as_widget_mut().operate(
                    &mut tree.children[0],
                    content_layout,
                    renderer,
                    operation,
                );
            }

            if let (Some(action), Some(action_layout)) = (&mut self.action, children.next()) {
                action.as_widget_mut().operate(
                    &mut tree.children[1],
                    action_layout,
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
        let _surface = children.next();
        let content_layout = children.next();
        let action_layout = children.next();

        {
            let state = tree.state.downcast_mut::<State>();

            if state.animation.value() != self.is_visible {
                state.animation.go_mut(self.is_visible, state.now);

                // The timeout clock starts when the message appears, not
                // when the widget was built.
                state.shown_at = self.is_visible.then_some(state.now);

                shell.request_redraw();
            }

            if let Event::Window(crate::core::window::Event::RedrawRequested(now)) = event {
                state.now = *now;

                if state.animation.is_animating(*now) {
                    shell.request_redraw();
                }
            }
        }

        // The action sees events first: its button must win its own
        // click before the snackbar considers anything.
        if let (Some(action), Some(action_layout)) = (&mut self.action, action_layout) {
            action.as_widget_mut().update(
                &mut tree.children[1],
                event,
                action_layout,
                cursor,
                renderer,
                shell,
                viewport,
            );

            if shell.is_event_captured() {
                return;
            }
        }

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

            if shell.is_event_captured() {
                return;
            }
        }

        let Some(on_dismiss) = self.on_dismiss.clone() else {
            return;
        };

        if !self.is_visible {
            return;
        }

        match event {
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. })
                if *key == keyboard::Key::Named(keyboard::key::Named::Escape) =>
            {
                shell.publish(on_dismiss);
                shell.capture_event();
            }

            Event::Window(crate::core::window::Event::RedrawRequested(now)) => {
                let Some(timeout) = self.timeout else {
                    return;
                };

                let state = tree.state.downcast_ref::<State>();

                let Some(shown_at) = state.shown_at else {
                    return;
                };

                let elapsed = now.saturating_duration_since(shown_at);

                if elapsed >= timeout {
                    shell.publish(on_dismiss);
                } else {
                    // Ask to be woken exactly when the timeout expires
                    // rather than every frame: a snackbar sitting on
                    // screen must not defeat the framework's zero idle
                    // work guarantee.
                    shell.request_redraw_at(*now + (timeout - elapsed));
                }
            }

            _ => {}
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
        let state = tree.state.downcast_ref::<State>();

        if state.animation.interpolate(0.0, 1.0, state.now) <= 0.0 {
            return mouse::Interaction::None;
        }

        let mut children = layout.children();
        let _surface = children.next();
        let _content = children.next();

        if let (Some(action), Some(action_layout)) = (&self.action, children.next()) {
            return action.as_widget().mouse_interaction(
                &tree.children[1],
                action_layout,
                cursor,
                viewport,
                renderer,
            );
        }

        mouse::Interaction::None
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
        let state = tree.state.downcast_ref::<State>();
        let progress = state.animation.interpolate(0.0, 1.0, state.now);

        if progress <= 0.0 {
            return;
        }

        let appearance = theme.style(&self.class);
        let mut children = layout.children();

        if let Some(surface) = children.next() {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: surface.bounds(),
                    border: appearance.border,
                    shadow: appearance.shadow,
                    ..renderer::Quad::default()
                },
                fade(appearance.background, progress),
            );
        }

        let text_style = renderer::Style {
            text_color: Color {
                a: appearance.text_color.a * progress,
                ..appearance.text_color
            },
        };

        if let Some(content_layout) = children.next() {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                &text_style,
                content_layout,
                cursor,
                viewport,
            );
        }

        if let (Some(action), Some(action_layout)) = (&self.action, children.next()) {
            action.as_widget().draw(
                &tree.children[1],
                renderer,
                theme,
                &renderer::Style {
                    text_color: Color {
                        a: appearance.action_color.a * progress,
                        ..appearance.action_color
                    },
                },
                action_layout,
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

/// Scales a background's alpha by `progress`, so the surface fades in
/// with its contents rather than popping in behind them.
fn fade(background: Background, progress: f32) -> Background {
    match background {
        Background::Color(color) => Background::Color(Color {
            a: color.a * progress,
            ..color
        }),
        other => other,
    }
}

impl<'a, Message, Theme, Renderer> From<Snackbar<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(snackbar: Snackbar<'a, Message, Theme, Renderer>) -> Self {
        Self::new(snackbar)
    }
}

/// The appearance of a [`Snackbar`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The snackbar's fill.
    pub background: Background,
    /// The colour of the message.
    pub text_color: Color,
    /// The colour of the action control's content.
    pub action_color: Color,
    /// The border around the snackbar.
    pub border: Border,
    /// The shadow it casts over the page.
    pub shadow: Shadow,
}

/// The theme catalog of a [`Snackbar`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a [`Snackbar`].
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

/// The default styling of a [`Snackbar`] for aura's own theme.
///
/// A snackbar deliberately inverts the page: it is a notice layered over
/// content, and matching the surface it floats above would make it read
/// as part of the page rather than as something that just arrived.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: Background::Color(palette.background.strong.color),
        text_color: palette.background.strong.text,
        action_color: palette.primary.base.color,
        border: Border {
            radius: 8.0.into(),
            ..Border::default()
        },
        shadow: Shadow {
            color: Color {
                a: 0.3,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 2.0),
            blur_radius: 12.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snackbar<'a>(is_visible: bool) -> Snackbar<'a, ()> {
        Snackbar::new(crate::text::Text::new("Saved"), is_visible)
    }

    const AVAILABLE: Size = Size {
        width: 400.0,
        height: 800.0,
    };

    #[test]
    fn a_snackbar_floats_above_the_bottom_edge_by_its_margin() {
        let bar = snackbar(true);
        let bounds = bar.surface_bounds(AVAILABLE, 1.0);

        assert_eq!(bounds.y + bounds.height, AVAILABLE.height - DEFAULT_MARGIN);
    }

    #[test]
    fn a_hidden_snackbar_sits_lower_than_a_shown_one() {
        let bar = snackbar(false);

        let hidden = bar.surface_bounds(AVAILABLE, 0.0);
        let shown = bar.surface_bounds(AVAILABLE, 1.0);

        assert!(
            hidden.y > shown.y,
            "the snackbar should rise into place, not drop into it"
        );
        assert_eq!(hidden.y - shown.y, SLIDE_DISTANCE);
    }

    #[test]
    fn a_snackbar_is_centred_and_inset_by_its_margin() {
        let bounds = snackbar(true).surface_bounds(AVAILABLE, 1.0);

        assert_eq!(bounds.x, DEFAULT_MARGIN);
        assert_eq!(bounds.width, AVAILABLE.width - 2.0 * DEFAULT_MARGIN);
        assert_eq!(
            bounds.x + bounds.width,
            AVAILABLE.width - DEFAULT_MARGIN,
            "both side margins should match"
        );
    }

    #[test]
    fn a_snackbar_stops_widening_on_a_large_window() {
        let wide = Size::new(2000.0, 900.0);
        let bounds = snackbar(true).surface_bounds(wide, 1.0);

        assert_eq!(bounds.width, MAX_WIDTH);
        assert_eq!(
            bounds.x,
            (wide.width - MAX_WIDTH) / 2.0,
            "a capped snackbar should stay centred"
        );
    }

    #[test]
    fn a_snackbar_always_clears_the_touch_target_floor() {
        // The action inside it has to be tappable, so the bar cannot be
        // shorter than the touch-target policy even with no padding.
        let bar = snackbar(true).padding(Padding::ZERO);

        assert!(bar.surface_bounds(AVAILABLE, 1.0).height >= MIN_HEIGHT);
    }

    #[test]
    fn generous_padding_grows_the_snackbar_past_the_floor() {
        let bar = snackbar(true).padding(Padding::new(40.0));

        assert_eq!(bar.surface_bounds(AVAILABLE, 1.0).height, 80.0);
    }

    #[test]
    fn a_snackbar_narrower_than_its_margins_does_not_go_negative() {
        let tiny = Size::new(8.0, 100.0);

        assert_eq!(snackbar(true).surface_bounds(tiny, 1.0).width, 0.0);
    }

    #[test]
    fn the_timeout_can_be_removed_for_a_notice_that_must_be_acknowledged() {
        assert_eq!(snackbar(true).timeout, Some(DEFAULT_TIMEOUT));
        assert_eq!(snackbar(true).without_timeout().timeout, None);
    }
}
